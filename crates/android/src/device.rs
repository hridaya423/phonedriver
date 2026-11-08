//! `AndroidDevice`: the `Device` port over the on-device agent.

use async_trait::async_trait;
use phonedriver_core::device::{
    App, AppId, AppOp, Button, Capabilities, ClipboardOp, Device, DeviceEvent, DeviceInfo,
    DisplayMode, FrameRef, Gesture, Image, ImageFormat, Key, Quality, Result, ScreenState,
    ShellReply, ShellRequest, TextInput, Url,
};
use phonedriver_core::error::{Error, Op};
use phonedriver_core::geometry::{Rect, Size};
use phonedriver_core::raw::{NodeFlags, RawTree};
use phonedriver_video::frame_store::FrameStore;
use phonedriver_video::image::{encode_png, scaled_size};
use phonedriver_video::stream::VideoStream;
use serde_json::{Value, json};
use tokio::sync::{broadcast, watch};

use crate::adb::Adb;
use crate::agent::{Agent, AgentLog, Launched};
use crate::control::Control;
use crate::map;

const FIRST_FRAME_WAIT: std::time::Duration = std::time::Duration::from_secs(2);
const EVENT_BACKLOG: usize = 64;

pub struct AndroidDevice {
    adb: Adb,
    serial: String,
    info: DeviceInfo,
    hidden_display: Option<u32>,
    density: f32,
    control: Control,
    video: VideoStream,
    events: broadcast::Sender<DeviceEvent>,
    log: AgentLog,
    _launched: Launched,
}

impl AndroidDevice {
    /// Deploys and starts the agent on `serial`, connects both streams, and starts decoding video. It returns
    /// once the first frame has been decoded (or after 2 s), so settle and screenshots have a picture from the start.
    /// `info().screen` is the size at connect time; a later rotation arrives as `DeviceEvent::DisplayChanged`.
    /// Dropping the device closes the agent's shell stream, which ends the agent within about a second.
    pub async fn connect(adb: &Adb, serial: &str, display: DisplayMode) -> Result<AndroidDevice> {
        let Agent {
            control,
            video,
            hello,
            log,
            launched,
        } = Agent::start(adb, serial, display).await?;
        let density = hello.density;
        let id = format!("android:{serial}");
        let (events, _) = broadcast::channel(EVENT_BACKLOG);
        let screen_long_pt = hello.display.w.max(hello.display.h) as f32 / density;
        let video = VideoStream::spawn(video, FrameStore::fitted(screen_long_pt));
        let mut first = video.store.subscribe();
        let _ = tokio::time::timeout(FIRST_FRAME_WAIT, first.wait_for(|f| f.seq > 0)).await;
        Ok(AndroidDevice {
            adb: adb.clone(),
            serial: serial.to_string(),
            info: DeviceInfo {
                id: id.clone(),
                model: hello.model,
                os: format!("Android API {}", hello.api),
                screen: Size {
                    w: hello.display.w as f32 / density,
                    h: hello.display.h as f32 / density,
                },
                density,
                capabilities: Capabilities::all(),
            },
            hidden_display: hello.hidden_display,
            density,
            control: Control::new(id, control, density, events.clone()),
            video,
            events,
            log,
            _launched: launched,
        })
    }

    /// The last lines the agent printed, for the daemon's log.
    pub fn agent_log(&self) -> Vec<String> {
        self.log.recent()
    }

    async fn pm(&self, cmd: &str) -> Result<String> {
        let out = self.adb.shell(&self.serial, cmd).await?.stdout_text();
        let last = out
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        if last.trim() == "Success" {
            Ok(out)
        } else {
            Err(Error::Transport {
                detail: format!("`{cmd}` said: {}", last.trim()),
            })
        }
    }

    async fn packages(&self) -> Result<Vec<String>> {
        let out = self.adb.shell(&self.serial, "pm list packages").await?;
        Ok(out
            .stdout_text()
            .lines()
            .filter_map(|l| l.strip_prefix("package:").map(|p| p.trim().to_string()))
            .collect())
    }

    async fn simple(&self, op: Op, req: Value) -> Result<Value> {
        self.control.request(op, req).await
    }

    async fn press(&self, op: Op, keycode: i32) -> Result<()> {
        self.simple(
            op,
            json!({"op": "key", "keycode": keycode, "action": "press"}),
        )
        .await?;
        Ok(())
    }
}

#[async_trait]
impl Device for AndroidDevice {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn hidden_display(&self) -> Option<u32> {
        self.hidden_display
    }

    async fn tree(&self) -> Result<RawTree> {
        let reply = self.simple(Op::Tree, json!({"op": "tree"})).await?;
        map::raw_tree(&reply, self.density)
    }

    fn frames(&self) -> Option<watch::Receiver<FrameRef>> {
        Some(self.video.store.subscribe())
    }

    async fn screenshot(&self, _q: Quality) -> Result<Image> {
        let store = &self.video.store;
        if store.latest().is_none() {
            let mut rx = store.subscribe();
            let first = tokio::time::timeout(FIRST_FRAME_WAIT, rx.wait_for(|f| f.seq > 0)).await;
            if first.is_err() {
                return Err(Error::Timeout {
                    waited_ms: FIRST_FRAME_WAIT.as_millis() as u32,
                    condition: "the first video frame".into(),
                });
            }
        }
        if self.video.ended() {
            return Err(Error::DeviceGone {
                id: self.info.id.clone(),
            });
        }
        let frame = store.latest().ok_or_else(|| Error::Transport {
            detail: "no video frame".into(),
        })?;
        let scale = 1.0 / store.pixels_per_point(&frame);
        let (width, height) = scaled_size(frame.width, frame.height, scale);
        let bytes = tokio::task::spawn_blocking(move || encode_png(&frame, scale))
            .await
            .map_err(|e| Error::Transport {
                detail: format!("the screenshot task failed: {e}"),
            })?
            .map_err(|e| Error::Transport {
                detail: e.to_string(),
            })?;
        Ok(Image {
            width,
            height,
            format: ImageFormat::Png,
            bytes,
        })
    }

    fn events(&self) -> broadcast::Receiver<DeviceEvent> {
        self.events.subscribe()
    }

    async fn gesture(&self, g: Gesture) -> Result<()> {
        self.simple(Op::Gesture, map::gesture_request(&g, self.density))
            .await?;
        Ok(())
    }

    async fn key(&self, k: Key) -> Result<()> {
        self.press(Op::Key, map::keycode(k)).await
    }

    async fn button(&self, b: Button) -> Result<()> {
        match map::button_keycode(b) {
            Some(code) => self.press(Op::Button, code).await,
            None => {
                self.simple(Op::Button, json!({"op": "notifications"}))
                    .await?;
                Ok(())
            }
        }
    }

    async fn open_url(&self, url: &Url, app: Option<&AppId>) -> Result<()> {
        self.simple(
            Op::OpenUrl,
            json!({"op": "open_url", "url": url.0, "package": app.map(|a| a.0.as_str())}),
        )
        .await?;
        Ok(())
    }

    async fn text(&self, t: TextInput) -> Result<()> {
        self.simple(Op::Text, map::text_request(&t)).await?;
        Ok(())
    }

    async fn clipboard(&self, op: ClipboardOp) -> Result<Option<String>> {
        match op {
            ClipboardOp::Get => {
                let reply = self
                    .simple(Op::Clipboard, json!({"op": "clipboard_get"}))
                    .await?;
                Ok(reply["text"].as_str().map(str::to_string))
            }
            ClipboardOp::Set(text) => {
                self.simple(Op::Clipboard, json!({"op": "clipboard_set", "text": text}))
                    .await?;
                Ok(None)
            }
        }
    }

    async fn apps(&self) -> Result<Vec<App>> {
        let reply = self.simple(Op::Apps, json!({"op": "apps"})).await?;
        Ok(reply["apps"]
            .as_array()
            .map(|apps| {
                apps.iter()
                    .filter_map(|a| {
                        Some(App {
                            id: AppId(a["package"].as_str()?.to_string()),
                            name: a["label"].as_str()?.to_string(),
                            system: a["system"].as_bool().unwrap_or(false),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn launch(&self, app: &AppId) -> Result<()> {
        self.simple(Op::Launch, json!({"op": "launch", "package": app.0}))
            .await?;
        Ok(())
    }

    async fn grant(&self, app: &AppId, permissions: &[String]) -> Result<()> {
        let package_ok = !app.0.is_empty()
            && app
                .0
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_');
        if !package_ok {
            return Err(Error::Transport {
                detail: format!("{:?} is not a package name", app.0),
            });
        }
        for name in permissions {
            let permission = permission_name(name)?;
            let out = self
                .adb
                .shell(&self.serial, &format!("pm grant {} {permission}", app.0))
                .await?;
            let text = format!(
                "{}{}",
                out.stdout_text(),
                String::from_utf8_lossy(&out.stderr)
            );
            if out.exit_code != 0 || text.contains("Exception") || text.contains("Error") {
                let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
                return Err(Error::Transport {
                    detail: format!("`pm grant {} {permission}` said: {}", app.0, first.trim()),
                });
            }
        }
        Ok(())
    }

    async fn foreground(&self) -> Result<Option<AppId>> {
        let reply = self.simple(Op::Foreground, json!({"op": "state"})).await?;
        Ok(reply["foreground"].as_str().map(|p| AppId(p.to_string())))
    }

    async fn screen_state(&self) -> Result<ScreenState> {
        let reply = self.simple(Op::Foreground, json!({"op": "power"})).await?;
        Ok(ScreenState {
            on: reply["screen_on"].as_bool().unwrap_or(true),
            locked: reply["locked"].as_bool().unwrap_or(false),
            secure: reply["secure_keyguard"].as_bool().unwrap_or(false),
            focused: reply["focused"].as_bool().unwrap_or(true),
        })
    }

    async fn install(&self, path: &std::path::Path) -> Result<Option<AppId>> {
        let bytes = std::fs::read(path).map_err(|e| Error::Transport {
            detail: format!("cannot read {}: {e}", path.display()),
        })?;
        let before = self.packages().await?;
        let remote = "/data/local/tmp/phonedriver-install.apk";
        self.adb.push(&self.serial, &bytes, remote, 0o644).await?;
        let installed = self.pm(&format!("pm install -r {remote}")).await;
        let _ = self.adb.shell(&self.serial, &format!("rm {remote}")).await;
        installed?;
        let after = self.packages().await?;
        Ok(after.into_iter().find(|p| !before.contains(p)).map(AppId))
    }

    async fn app_op(&self, op: AppOp, app: &AppId) -> Result<()> {
        let id = &app.0;
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
        {
            return Err(Error::Transport {
                detail: format!("{id:?} is not a package name"),
            });
        }
        match op {
            AppOp::Stop => {
                self.adb
                    .shell(&self.serial, &format!("am force-stop {id}"))
                    .await?;
            }
            AppOp::ClearData => {
                self.pm(&format!("pm clear {id}")).await?;
            }
            AppOp::Uninstall => {
                self.pm(&format!("pm uninstall {id}")).await?;
            }
        }
        Ok(())
    }

    async fn shell(&self, req: &ShellRequest) -> Result<ShellReply> {
        let reply = self
            .simple(
                Op::Shell,
                json!({"op": "shell", "command": req.command, "timeout_ms": req.timeout.as_millis() as u64}),
            )
            .await?;
        Ok(ShellReply {
            exit: reply["exit"].as_i64().unwrap_or(-1) as i32,
            stdout: reply["stdout"].as_str().unwrap_or_default().to_string(),
            stderr: reply["stderr"].as_str().unwrap_or_default().to_string(),
            timed_out: reply["timed_out"].as_bool().unwrap_or(false),
        })
    }

    async fn wake(&self) -> Result<()> {
        self.simple(Op::Button, json!({"op": "wake"})).await?;
        Ok(())
    }

    fn secure_content_hidden(&self, tree: &RawTree) -> bool {
        tree.windows
            .iter()
            .filter(|w| w.kind == "application")
            .any(|w| {
                let b = w.bounds;
                let middle = Rect {
                    x: b.x,
                    y: b.y + b.h * 0.25,
                    w: b.w,
                    h: b.h * 0.5,
                };
                let shows_text = tree.nodes.iter().any(|n| {
                    n.window == w.id
                        && n.label.is_some()
                        && n.flags.contains(NodeFlags::VISIBLE_TO_USER)
                });
                shows_text && self.video.store.region_is_black(middle)
            })
    }

    async fn capture_tree(&self) -> Result<Option<String>> {
        let mut reply = self.simple(Op::Tree, json!({"op": "tree"})).await?;
        if let Some(map) = reply.as_object_mut() {
            map.remove("id");
            map.remove("ok");
            map.insert("density".into(), self.density.into());
        }
        Ok(Some(reply.to_string()))
    }
}

/// `notifications`, `camera`, `location`... become `android.permission.POST_NOTIFICATIONS` and so on; a name with a
/// dot is taken as written.
fn permission_name(name: &str) -> Result<String> {
    let full = match name {
        "notifications" => "android.permission.POST_NOTIFICATIONS".to_string(),
        "location" => "android.permission.ACCESS_FINE_LOCATION".to_string(),
        n if n.contains('.') => n.to_string(),
        n => format!("android.permission.{}", n.to_ascii_uppercase()),
    };
    if full
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
    {
        Ok(full)
    } else {
        Err(Error::Transport {
            detail: format!("{name:?} is not a permission name"),
        })
    }
}

#[cfg(test)]
mod permission_tests {
    use super::permission_name;

    #[test]
    fn short_names_expand_and_full_names_pass() {
        assert_eq!(
            permission_name("notifications").unwrap(),
            "android.permission.POST_NOTIFICATIONS"
        );
        assert_eq!(
            permission_name("camera").unwrap(),
            "android.permission.CAMERA"
        );
        assert_eq!(
            permission_name("android.permission.READ_CONTACTS").unwrap(),
            "android.permission.READ_CONTACTS"
        );
        assert!(permission_name("camera; rm -rf /").is_err());
    }
}
