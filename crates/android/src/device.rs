//! `AndroidDevice`: the `Device` port over the on-device agent.

use async_trait::async_trait;
use phone_use_core::device::{
    App, AppId, Button, Capabilities, ClipboardOp, Device, DeviceEvent, DeviceInfo, FrameRef,
    Gesture, Image, ImageFormat, Key, Quality, Result, TextInput, Url,
};
use phone_use_core::error::{Error, Op};
use phone_use_core::geometry::{Rect, Size};
use phone_use_core::raw::{NodeFlags, RawTree};
use phone_use_video::image::{encode_png, scaled_size};
use phone_use_video::stream::VideoStream;
use serde_json::{Value, json};
use tokio::sync::{broadcast, watch};

use crate::adb::Adb;
use crate::agent::{Agent, AgentLog, Launched};
use crate::control::Control;
use crate::map;

const FIRST_FRAME_WAIT: std::time::Duration = std::time::Duration::from_secs(2);
const EVENT_BACKLOG: usize = 64;

pub struct AndroidDevice {
    info: DeviceInfo,
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
    pub async fn connect(adb: &Adb, serial: &str) -> Result<AndroidDevice> {
        let Agent {
            control,
            video,
            hello,
            log,
            launched,
        } = Agent::start(adb, serial).await?;
        let density = hello.density;
        let id = format!("android:{serial}");
        let (events, _) = broadcast::channel(EVENT_BACKLOG);
        let video = VideoStream::spawn(video, density);
        let mut first = video.store.subscribe();
        let _ = tokio::time::timeout(FIRST_FRAME_WAIT, first.wait_for(|f| f.seq > 0)).await;
        Ok(AndroidDevice {
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

    /// Wakes the screen (architecture 6.3: before any action on a dark screen).
    pub async fn wake(&self) -> Result<()> {
        self.control
            .request(Op::Button, json!({"op": "wake"}))
            .await?;
        Ok(())
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
        let scale = 1.0 / self.density;
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

    async fn open_url(&self, url: &Url) -> Result<()> {
        self.simple(Op::OpenUrl, json!({"op": "open_url", "url": url.0}))
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

    async fn foreground(&self) -> Result<Option<AppId>> {
        let reply = self.simple(Op::Foreground, json!({"op": "state"})).await?;
        Ok(reply["foreground"].as_str().map(|p| AppId(p.to_string())))
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
