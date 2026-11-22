//! `AndroidDevice`: the `Device` port over the on-device agent.

use async_trait::async_trait;
use phonedriver_core::device::{
    App, AppId, AppOp, Button, Capabilities, ClipboardOp, Device, DeviceEvent, DeviceInfo,
    DisplayMode, FrameRef, Gesture, Image, ImageFormat, Key, LogReply, LogRequest, Quality,
    Recorded, Result, ScreenState, ShellReply, ShellRequest, TextInput, Url, WebPage,
};
use phonedriver_core::error::{Error, Op};
use phonedriver_core::geometry::{Rect, Size};
use phonedriver_core::network::{LogWait, LogWaited, NetworkView};
use phonedriver_core::raw::{NodeFlags, RawTree};
use phonedriver_core::settings::{SettingOp, SettingReply, SettingsIndex};
use phonedriver_video::frame_store::FrameStore;
use phonedriver_video::image::{encode_png, scaled_size};
use phonedriver_video::stream::VideoStream;
use serde_json::{Value, json};
use tokio::sync::{broadcast, watch};

use crate::adb::{Adb, ShellOutput, ShellPacket};
use crate::agent::{Agent, AgentLog, Launched};
use crate::console;
use crate::control::Control;
use crate::devtools;
use crate::logs;
use crate::map;
use crate::network;
use crate::settings;

const FIRST_FRAME_WAIT: std::time::Duration = std::time::Duration::from_secs(2);
/// A screenshot proves the stream is current within this: one control round trip and the barrier's way back.
const BARRIER_WAIT: std::time::Duration = std::time::Duration::from_millis(1500);
/// The finger `press fingerprint` touches the sensor with, which is the one to enroll with, and one that is
/// never enrolled.
const ENROLLED_FINGER: u32 = 1;
const OTHER_FINGER: u32 = 2;
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
    _launched: std::sync::Arc<Launched>,
    settings: tokio::sync::OnceCell<std::sync::Arc<SettingsIndex>>,
}

impl AndroidDevice {
    /// Deploys and starts the agent on `serial`, connects both streams, and starts decoding video. It returns
    /// once the first frame has been decoded (or after 2 s), so settle and screenshots have a picture from the start.
    /// `info().screen` is the size at connect time; a later rotation arrives as `DeviceEvent::DisplayChanged`.
    /// Dropping the device closes the agent's shell stream, which ends the agent within about a second.
    /// With a `lane`, the device works on a hidden display of its own beside the device's other lanes, and its id is
    /// `android:<serial>+<lane>` (decision D73).
    pub async fn connect(
        adb: &Adb,
        serial: &str,
        display: DisplayMode,
        lane: Option<&str>,
    ) -> Result<AndroidDevice> {
        let Agent {
            control,
            video,
            hello,
            log,
            launched,
        } = Agent::start(adb, serial, display, lane).await?;
        let density = hello.density;
        let id = match lane {
            Some(lane) => format!("android:{serial}+{lane}"),
            None => format!("android:{serial}"),
        };
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
            settings: tokio::sync::OnceCell::new(),
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
        listed_packages(&out)
    }

    /// A connection to the emulator's console and the token it may ask for, or why there is none. `op` is what
    /// a refusal names.
    async fn console(
        &self,
        op: Op,
        phone: &'static str,
    ) -> Result<(tokio::net::TcpStream, Option<String>)> {
        let port = console::port(&self.serial).ok_or(Error::Unsupported { op, reason: phone })?;
        let token = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .and_then(|home| {
                let file = std::path::Path::new(&home).join(".emulator_console_auth_token");
                std::fs::read_to_string(file).ok()
            });
        let stream = if self.adb.is_local() {
            console::connect(&format!("127.0.0.1:{port}"))
                .await
                .map_err(|detail| Error::Transport { detail })?
        } else {
            // Behind `ADB_SERVER_SOCKET` the console's machine is not this one. The emulator's own network gives
            // its host's loopback the address 10.0.2.2, so `nc` on the device reaches the console.
            self.adb
                .exec_stream(&self.serial, &format!("nc 10.0.2.2 {port}"))
                .await?
        };
        Ok((stream, token))
    }

    /// Touches the emulator's fingerprint sensor through its console (decision D63).
    async fn touch_sensor(&self, finger: u32) -> Result<()> {
        let (stream, token) = self
            .console(
                Op::Button,
                "a phone's fingerprint sensor takes its owner's finger; only an emulator's can be touched from here",
            )
            .await?;
        console::touch_sensor_on(stream, token.as_deref(), finger)
            .await
            .map_err(|detail| Error::Transport { detail })
    }

    async fn simple(&self, op: Op, req: Value) -> Result<Value> {
        self.control
            .request(op, req, std::time::Duration::ZERO)
            .await
    }

    async fn video_caught_up(&self, op: Op) -> Result<()> {
        caught_up(&self.video, &self.info.id, BARRIER_WAIT, |number| {
            self.simple(op, barrier_request(number))
        })
        .await
    }

    async fn press(&self, op: Op, keycode: i32) -> Result<()> {
        self.simple(
            op,
            json!({"op": "key", "keycode": keycode, "action": "press"}),
        )
        .await?;
        Ok(())
    }

    /// Runs one of the Settings app's functions and returns its JSON. Older Android has no `app_function`
    /// service; a build whose Settings lacks the function answers "App function not found".
    async fn settings_function(&self, function: &str, parameters: &Value) -> Result<String> {
        let out = self
            .adb
            .shell(&self.serial, &settings::command(function, parameters))
            .await?;
        let text = out.stdout_text();
        if text.trim_start().starts_with('{') {
            return Ok(text);
        }
        let said = format!("{text}{}", String::from_utf8_lossy(&out.stderr));
        if said.contains("Can't find service") || said.contains("App function not found") {
            return Err(Error::Unsupported {
                op: Op::Setting,
                reason: "this device has no settings to read or write by key (Android 17 has them); use `shell`, or the Settings app",
            });
        }
        let first = said.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        Err(Error::Refused {
            detail: format!("the Settings app did not run {function}: {}", first.trim()),
        })
    }

    async fn pm_permissions(&self, verb: &str, app: &AppId, permissions: &[String]) -> Result<()> {
        package_name(app)?;
        let mut full = Vec::new();
        for name in permissions {
            let permission = permission_name(name)?;
            let out = self
                .adb
                .shell(&self.serial, &format!("pm {verb} {} {permission}", app.0))
                .await?;
            let text = format!(
                "{}{}",
                out.stdout_text(),
                String::from_utf8_lossy(&out.stderr)
            );
            if out.exit_code != 0 || text.contains("Exception") || text.contains("Error") {
                let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
                return Err(Error::Transport {
                    detail: format!("`pm {verb} {} {permission}` said: {}", app.0, first.trim()),
                });
            }
            full.push(permission);
        }
        let dump = self
            .adb
            .shell(
                &self.serial,
                &format!("dumpsys package {} | grep granted=", app.0),
            )
            .await?
            .stdout_text();
        for permission in &full {
            if let Some(why) = unapplied(&dump, verb, permission) {
                return Err(Error::Refused {
                    detail: format!("`pm {verb} {} {permission}` changed nothing: {why}", app.0),
                });
            }
        }
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
        let barrier = self.video_caught_up(Op::Screenshot);
        let (width, height, bytes) = newest_png(store, barrier).await?;
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
        let length = g
            .fingers
            .iter()
            .filter_map(|f| f.last())
            .map(|p| p.t_ms)
            .max()
            .unwrap_or(0);
        self.control
            .request(
                Op::Gesture,
                map::gesture_request(&g, self.density),
                std::time::Duration::from_millis(length.into()),
            )
            .await?;
        Ok(())
    }

    async fn key(&self, k: Key) -> Result<()> {
        self.press(Op::Key, map::keycode(k)).await
    }

    async fn button(&self, b: Button) -> Result<()> {
        match b {
            Button::Fingerprint => return self.touch_sensor(ENROLLED_FINGER).await,
            Button::WrongFingerprint => return self.touch_sensor(OTHER_FINGER).await,
            Button::Landscape | Button::Portrait => {
                // With auto-rotate on, Android ignores `user_rotation`, so it is turned off first.
                let quarter = u8::from(b == Button::Landscape);
                self.adb
                    .shell(
                        &self.serial,
                        &format!(
                            "settings put system accelerometer_rotation 0; settings put system user_rotation {quarter}"
                        ),
                    )
                    .await?;
                return Ok(());
            }
            _ => {}
        }
        match map::button_keycode(b) {
            Some(code) => self.press(Op::Button, code).await,
            None => {
                self.simple(Op::Button, json!({"op": "notifications"}))
                    .await?;
                Ok(())
            }
        }
    }

    async fn open_url(&self, url: &Url, app: Option<&AppId>) -> Result<Option<AppId>> {
        let reply = self
            .simple(
                Op::OpenUrl,
                json!({"op": "open_url", "url": url.0, "package": app.map(|a| a.0.as_str())}),
            )
            .await?;
        Ok(reply["package"].as_str().map(|p| AppId(p.to_string())))
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
        self.pm_permissions("grant", app, permissions).await
    }

    async fn push(&self, _app: &AppId, _payload: &str) -> Result<()> {
        Err(Error::Unsupported {
            op: Op::App,
            reason: "an Android app's push messages reach it through Google Play services, which only that service may send, and the shell cannot stand in for it; `cmd notification post` can show a notification, but from the shell's package and not from the app, so it does not test the app. Send the push from the Firebase console or its HTTP API, or call a receiver your debug build exposes with `shell`",
        })
    }

    async fn revoke(&self, app: &AppId, permissions: &[String]) -> Result<()> {
        self.pm_permissions("revoke", app, permissions).await
    }

    /// Observed with a debug build on a loaded host: the reader was 1.0 s behind the stream, so a recording held
    /// 1.0 s from before its start and its steps file ran that much ahead of the picture. Both ends wait for the
    /// stream to catch up first.
    async fn record_start(&self, path: &std::path::Path) -> Result<()> {
        if self.video.recording() {
            return Err(Error::Refused {
                detail: "a recording is already running".into(),
            });
        }
        self.video_caught_up(Op::Record).await?;
        self.video
            .record_start(path)
            .map_err(|e| Error::Transport {
                detail: format!("cannot record to {}: {e}", path.display()),
            })?;
        if let Err(e) = self.simple(Op::Record, json!({"op": "sync_frame"})).await {
            let _ = self.video.record_stop();
            return Err(e);
        }
        Ok(())
    }

    async fn record_stop(&self) -> Result<Option<Recorded>> {
        let behind = self.video.recording() && self.video_caught_up(Op::Record).await.is_err();
        let stopped = self.video.record_stop().map_err(|e| Error::Transport {
            detail: format!("cannot finish the recording: {e}"),
        })?;
        Ok(stopped.map(|s| Recorded {
            path: s.recorded.path,
            width: s.recorded.width,
            height: s.recorded.height,
            frames: s.recorded.frames,
            ms: s.recorded.ms,
            early: s.early.map(str::to_string).or_else(|| {
                behind.then(|| "the device stopped answering before its last frames arrived".into())
            }),
        }))
    }

    async fn page(&self, app: Option<&AppId>) -> Result<WebPage> {
        let front = match app {
            Some(app) => app.clone(),
            None => self.foreground().await?.ok_or(Error::Refused {
                detail: "no app is in front".into(),
            })?,
        };
        devtools::page(&self.adb, &self.serial, &front).await
    }

    async fn logs(&self, req: &LogRequest) -> Result<LogReply> {
        package_name(&req.app)?;
        let seconds = req.since.as_secs() + 1;
        let run = |command: String| async move {
            let request = ShellRequest {
                command,
                timeout: LOGS_LIMIT,
            };
            Ok::<_, Error>(self.shell(&request).await?.stdout)
        };
        let crash = logs::crash_of(&run(logs::crash_command(seconds)).await?, &req.app.0);
        let text = if req.crashes {
            String::new()
        } else {
            run(logs::app_command(&req.app.0, seconds)).await?
        };
        Ok(LogReply { text, crash })
    }

    async fn log_wait(&self, req: &LogWait) -> Result<LogWaited> {
        let package = match &req.app {
            Some(app) => {
                package_name(app)?;
                Some(app.0.as_str())
            }
            None => None,
        };
        let mut stream = self
            .adb
            .shell_stream(&self.serial, &logs::watch_command(package))
            .await?;
        let deadline = tokio::time::Instant::now() + req.timeout;
        let (mut pending, mut seen) = (Vec::<u8>::new(), 0usize);
        let mut said = String::new();
        loop {
            let packet = match tokio::time::timeout_at(deadline, stream.next()).await {
                Err(_) => return Ok(LogWaited { line: None, seen }),
                Ok(packet) => packet?,
            };
            match packet {
                Some(ShellPacket::Stdout(data)) => {
                    pending.extend(data);
                    while let Some(end) = pending.iter().position(|b| *b == b'\n') {
                        let line: Vec<u8> = pending.drain(..=end).collect();
                        let line = String::from_utf8_lossy(&line).trim_end().to_string();
                        if line.is_empty() || logs::is_marker(&line) {
                            continue;
                        }
                        seen += 1;
                        if req.matches(&line) {
                            return Ok(LogWaited {
                                line: Some(line),
                                seen,
                            });
                        }
                    }
                }
                Some(ShellPacket::Stderr(data)) => said.push_str(&String::from_utf8_lossy(&data)),
                Some(ShellPacket::Exit(_)) | None => {
                    return Err(Error::Transport {
                        detail: match req.app.as_ref() {
                            Some(app) if said.trim().is_empty() => {
                                format!("the log of {} ended: is it installed?", app.0)
                            }
                            _ => format!("the log ended: {}", said.trim()),
                        },
                    });
                }
            }
        }
    }

    async fn network(&self) -> Result<NetworkView> {
        let request = ShellRequest {
            command: network::COMMAND.to_string(),
            timeout: NETWORK_LIMIT,
        };
        let reply = self.shell(&request).await?;
        if reply.timed_out || reply.stdout.trim().is_empty() {
            return Err(Error::Transport {
                detail: format!(
                    "the device did not print its connectivity state within {} s",
                    NETWORK_LIMIT.as_secs()
                ),
            });
        }
        Ok(network::parse(&reply.stdout))
    }

    async fn setting(&self, op: SettingOp) -> Result<SettingReply> {
        match op {
            SettingOp::Index => {
                let index = self
                    .settings
                    .get_or_try_init(|| async {
                        let reply = self
                            .settings_function(settings::INDEX_FUNCTION, &json!({}))
                            .await?;
                        settings::index(&reply)
                            .map(std::sync::Arc::new)
                            .map_err(|detail| Error::Refused {
                                detail: format!(
                                    "the Settings app's index could not be read: {detail}"
                                ),
                            })
                    })
                    .await?;
                Ok(SettingReply::Index(index.clone()))
            }
            SettingOp::Get(key) => {
                for function in settings::STATE_FUNCTIONS {
                    let reply = match self.settings_function(function, &json!({})).await {
                        Ok(reply) => reply,
                        Err(Error::Refused { .. } | Error::Unsupported { .. }) => continue,
                        Err(e) => return Err(e),
                    };
                    if let Some((value, hint)) = settings::value(&reply, &key) {
                        return Ok(SettingReply::Value { value, hint });
                    }
                }
                Err(Error::Refused {
                    detail: format!("the device reported no value for {key}"),
                })
            }
            SettingOp::Set { key, value } => {
                let reply = self
                    .settings_function(
                        settings::WRITE_FUNCTION,
                        &settings::write_parameters(&key, &value),
                    )
                    .await?;
                match settings::written(&reply) {
                    Ok(value) => Ok(SettingReply::Value { value, hint: None }),
                    Err(why) => Err(Error::Refused {
                        detail: format!("{key} was not set: {why}"),
                    }),
                }
            }
        }
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

    async fn install(&self, path: &std::path::Path) -> Result<Vec<AppId>> {
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
        Ok(after
            .into_iter()
            .filter(|p| !before.contains(p))
            .map(AppId)
            .collect())
    }

    async fn installed_at(&self, app: &AppId) -> Result<Option<String>> {
        package_name(app)?;
        let out = self
            .adb
            .shell(
                &self.serial,
                &format!("dumpsys package {} | grep firstInstallTime", app.0),
            )
            .await?;
        first_install_time(&out.stdout_text())
            .map(Some)
            .ok_or_else(|| Error::Transport {
                detail: format!("`dumpsys package {}` shows no firstInstallTime", app.0),
            })
    }

    async fn hardware_serial(&self) -> Result<Option<String>> {
        Ok(Some(self.adb.hardware_serial(&self.serial).await?))
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
        if let Some(command) = req.command.trim_start().strip_prefix("emu ") {
            console::check(command.trim(), phonedriver_core::device::host_shell())
                .map_err(|detail| Error::Refused { detail })?;
            let (stream, token) = self
                .console(
                    Op::Shell,
                    "`emu` commands go to an Android emulator's console, and a phone has none",
                )
                .await?;
            let said = console::command_on(stream, token.as_deref(), command.trim()).await;
            return Ok(ShellReply {
                exit: i32::from(said.is_err()),
                stdout: said.clone().unwrap_or_default(),
                stderr: said.err().unwrap_or_default(),
                timed_out: false,
            });
        }
        let reply = self
            .control
            .request(
                Op::Shell,
                json!({"op": "shell", "command": req.command, "timeout_ms": req.timeout.as_millis() as u64}),
                req.timeout,
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

/// Why `pm grant` or `pm revoke` did not do what it was asked, read from the `granted=` lines of `dumpsys package`.
/// On API 37 `pm grant` exits 0 and prints nothing for a permission the app's manifest does not request.
fn unapplied(dump: &str, verb: &str, permission: &str) -> Option<&'static str> {
    let state = dump
        .lines()
        .filter_map(|l| {
            l.trim()
                .strip_prefix(permission)?
                .strip_prefix(": granted=")
        })
        .map(|rest| rest.starts_with("true"))
        .reduce(|a, b| a || b);
    match (verb, state) {
        ("grant", None) => Some("the app's manifest does not request that permission"),
        ("grant", Some(false)) => Some("the permission is still not granted"),
        ("revoke", Some(true)) => Some("the permission is still granted"),
        _ => None,
    }
}

fn listed_packages(out: &ShellOutput) -> Result<Vec<String>> {
    let packages: Vec<String> = out
        .stdout_text()
        .lines()
        .filter_map(|l| l.strip_prefix("package:").map(|p| p.trim().to_string()))
        .collect();
    if out.exit_code != 0 {
        return Err(Error::Transport {
            detail: format!("`pm list packages` exited with {}", out.exit_code),
        });
    }
    if packages.is_empty() {
        return Err(Error::Transport {
            detail: "`pm list packages` listed no packages".into(),
        });
    }
    Ok(packages)
}

fn first_install_time(dump: &str) -> Option<String> {
    dump.lines()
        .find_map(|l| l.trim().strip_prefix("firstInstallTime="))
        .map(|t| t.trim().to_string())
}

const LOGS_LIMIT: std::time::Duration = std::time::Duration::from_secs(10);
/// `dumpsys connectivity` took 6 s to 10 s for a minute after a VPN app was force-stopped (API 37 emulator).
const NETWORK_LIMIT: std::time::Duration = std::time::Duration::from_secs(20);

/// An app id goes into shell commands, so it must look like a package name.
pub(crate) fn package_name(app: &AppId) -> Result<()> {
    let ok = !app.0.is_empty()
        && app
            .0
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(Error::Transport {
            detail: format!("{:?} is not a package name", app.0),
        })
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

/// The number goes under its own name: `Control::request` stamps every request with an `id`, which once overwrote
/// it (decision D74).
pub(crate) fn barrier_request(number: u64) -> Value {
    json!({"op": "video_barrier", "barrier": number})
}

/// The newest frame as PNG once `barrier` has passed. The frame on hand is encoded while the barrier travels, and
/// encoded again only when a newer one arrived meanwhile, so a screenshot costs the slower of the two, not both.
async fn newest_png(
    store: &FrameStore,
    barrier: impl std::future::Future<Output = Result<()>>,
) -> Result<(u32, u32, Vec<u8>)> {
    let latest = || {
        store.latest().ok_or_else(|| Error::Transport {
            detail: "no video frame".into(),
        })
    };
    let encode = |frame: std::sync::Arc<phonedriver_video::frame_store::Frame>| {
        let scale = 1.0 / store.pixels_per_point(&frame);
        let (width, height) = scaled_size(frame.width, frame.height, scale);
        async move {
            let bytes = tokio::task::spawn_blocking(move || encode_png(&frame, scale))
                .await
                .map_err(|e| Error::Transport {
                    detail: format!("the screenshot task failed: {e}"),
                })?
                .map_err(|e| Error::Transport {
                    detail: e.to_string(),
                })?;
            Ok::<_, Error>((width, height, bytes))
        }
    };
    let early = latest()?;
    let seq = early.seq;
    let (passed, encoded) = tokio::join!(barrier, encode(early));
    passed?;
    let now = latest()?;
    if now.seq == seq {
        encoded
    } else {
        encode(now).await
    }
}

/// Asks the agent to write barrier `id` into the video stream and waits for it to arrive, so the frame a screenshot
/// serves is the newest the agent has produced. Observed on API 26 through an adb tunnel: after the agent was
/// killed (which is what a second daemon does), neither socket reported it for 5 to 21 s, the stream looked open,
/// a screenshot answered with the last frame and tree reads hung. A barrier that does not come back means the
/// device is gone as far as this connection is concerned, so the caller reconnects. The agent holds the barrier
/// until its encoder has given out what the screen showed when it was asked, and says in `wait_ms` how far behind
/// the encoder is, which is added to `limit`.
async fn caught_up<F>(
    video: &VideoStream,
    device: &str,
    limit: std::time::Duration,
    ask: impl FnOnce(u64) -> F,
) -> Result<()>
where
    F: std::future::Future<Output = Result<Value>>,
{
    let gone = || Error::DeviceGone { id: device.into() };
    let id = video.next_barrier();
    let asked = std::time::Instant::now();
    let reply = tokio::time::timeout(limit, ask(id))
        .await
        .map_err(|_| gone())??;
    let behind = std::time::Duration::from_millis(reply["wait_ms"].as_u64().unwrap_or(0));
    let left = limit.saturating_sub(asked.elapsed()) + behind;
    if video.wait_barrier(id, left).await {
        Ok(())
    } else {
        Err(gone())
    }
}

#[cfg(test)]
mod barrier_tests {
    use std::time::Duration;

    use phonedriver_core::error::Error;
    use phonedriver_video::frame_store::FrameStore;
    use phonedriver_video::stream::VideoStream;
    use serde_json::json;
    use tokio::io::{AsyncWriteExt, DuplexStream};

    use super::{caught_up, newest_png};

    const LIMIT: Duration = Duration::from_millis(300);

    fn open_stream() -> (DuplexStream, VideoStream) {
        let (agent, host) = tokio::io::duplex(1 << 16);
        (agent, VideoStream::spawn(host, FrameStore::new(2.0)))
    }

    fn barrier_packet(id: u64) -> Vec<u8> {
        let mut b = 0u64.to_be_bytes().to_vec();
        b.push(4);
        b.extend_from_slice(&8u32.to_be_bytes());
        b.extend_from_slice(&id.to_be_bytes());
        b
    }

    fn header() -> Vec<u8> {
        let mut h = b"h264".to_vec();
        h.extend_from_slice(&64u32.to_be_bytes());
        h.extend_from_slice(&64u32.to_be_bytes());
        h
    }

    #[tokio::test]
    async fn an_agent_that_never_answers_is_gone_not_a_source_of_the_last_frame() {
        let (_agent, video) = open_stream();
        let e = caught_up(&video, "android:x", LIMIT, |_| std::future::pending())
            .await
            .unwrap_err();
        assert_eq!(
            e,
            Error::DeviceGone {
                id: "android:x".into()
            }
        );
    }

    #[tokio::test]
    async fn an_agent_that_answers_but_whose_stream_delivers_nothing_is_gone() {
        let (_agent, video) = open_stream();
        let e = caught_up(&video, "android:x", LIMIT, |_| async { Ok(json!({})) })
            .await
            .unwrap_err();
        assert_eq!(
            e,
            Error::DeviceGone {
                id: "android:x".into()
            }
        );
    }

    #[tokio::test]
    async fn a_barrier_the_agent_says_is_behind_is_waited_for_that_much_longer() {
        let (mut agent, video) = open_stream();
        agent.write_all(&header()).await.unwrap();
        let late = tokio::spawn(async move {
            tokio::time::sleep(LIMIT + Duration::from_millis(200)).await;
            agent.write_all(&barrier_packet(1)).await.unwrap();
            agent
        });
        caught_up(&video, "android:x", LIMIT, |_| async {
            Ok(json!({"wait_ms": 400}))
        })
        .await
        .unwrap();
        late.await.unwrap();
    }

    #[tokio::test]
    async fn a_barrier_that_comes_back_through_the_stream_lets_the_screenshot_through() {
        let (mut agent, video) = open_stream();
        agent.write_all(&header()).await.unwrap();
        agent.write_all(&barrier_packet(1)).await.unwrap();
        caught_up(&video, "android:x", LIMIT, |_| async { Ok(json!({})) })
            .await
            .unwrap();
    }

    fn frame(shade: u8) -> phonedriver_video::frame_store::Frame {
        phonedriver_video::frame_store::Frame::from_i420(
            8,
            8,
            vec![shade; 64],
            vec![128; 16],
            vec![128; 16],
        )
    }

    #[tokio::test]
    async fn a_frame_that_arrives_while_the_barrier_travels_is_the_one_served() {
        let store = std::sync::Arc::new(FrameStore::new(1.0));
        store.publish(frame(0));
        let later = store.clone();
        let (_, _, served) = newest_png(&store, async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            later.publish(frame(255));
            Ok(())
        })
        .await
        .unwrap();
        let (_, _, newest) = newest_png(&store, async { Ok(()) }).await.unwrap();
        assert_eq!(served, newest);
    }

    #[tokio::test]
    async fn a_barrier_that_fails_fails_the_screenshot() {
        let store = FrameStore::new(1.0);
        store.publish(frame(0));
        let e = newest_png(&store, async {
            Err(Error::DeviceGone {
                id: "android:x".into(),
            })
        })
        .await
        .unwrap_err();
        assert_eq!(
            e,
            Error::DeviceGone {
                id: "android:x".into()
            }
        );
    }

    #[tokio::test]
    async fn an_error_from_the_agent_is_the_error() {
        let (_agent, video) = open_stream();
        let e = caught_up(&video, "android:x", LIMIT, |_| async {
            Err(Error::Transport {
                detail: "boom".into(),
            })
        })
        .await
        .unwrap_err();
        assert_eq!(
            e,
            Error::Transport {
                detail: "boom".into()
            }
        );
    }
}

#[cfg(test)]
mod permission_tests {
    use super::{permission_name, unapplied};

    const DUMP: &str = "      android.permission.INTERNET: granted=true
        android.permission.POST_NOTIFICATIONS: granted=true, flags=[ USER_SENSITIVE_WHEN_GRANTED|USER_SENSITIVE_WHEN_DENIED]
        android.permission.READ_CONTACTS: granted=false, flags=[ USER_SENSITIVE_WHEN_GRANTED|USER_SENSITIVE_WHEN_DENIED]";

    #[test]
    fn a_grant_the_package_manager_accepted_but_did_not_apply_is_a_failure() {
        assert_eq!(
            unapplied(DUMP, "grant", "android.permission.POST_NOTIFICATIONS"),
            None
        );
        let why = unapplied(DUMP, "grant", "android.permission.CAMERA").unwrap();
        assert!(why.contains("does not request"), "{why}");
        let why = unapplied(DUMP, "grant", "android.permission.READ_CONTACTS").unwrap();
        assert!(why.contains("still not granted"), "{why}");
    }

    #[test]
    fn a_revoke_is_applied_when_the_permission_is_no_longer_granted() {
        assert_eq!(
            unapplied(DUMP, "revoke", "android.permission.READ_CONTACTS"),
            None
        );
        assert_eq!(unapplied(DUMP, "revoke", "android.permission.CAMERA"), None);
        let why = unapplied(DUMP, "revoke", "android.permission.POST_NOTIFICATIONS").unwrap();
        assert!(why.contains("still granted"), "{why}");
    }

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

#[cfg(test)]
mod package_tests {
    use crate::adb::ShellOutput;

    use super::{first_install_time, listed_packages};

    fn output(stdout: &str, exit_code: i32) -> ShellOutput {
        ShellOutput {
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
            exit_code,
        }
    }

    #[test]
    fn a_package_listing_that_failed_is_an_error_even_with_some_lines() {
        let e = listed_packages(&output("package:com.a\n", 1)).unwrap_err();
        assert!(format!("{e:?}").contains("pm list packages"), "{e:?}");
    }

    #[test]
    fn an_empty_package_listing_is_an_error() {
        let e = listed_packages(&output("", 0)).unwrap_err();
        assert!(format!("{e:?}").contains("no packages"), "{e:?}");
    }

    #[test]
    fn a_package_listing_names_each_package() {
        let listed = listed_packages(&output("package:com.a\npackage:com.b\n", 0)).unwrap();
        assert_eq!(listed, ["com.a", "com.b"]);
    }

    #[test]
    fn the_first_install_time_is_read_from_the_package_dump() {
        let dump = "    versionName=1.0\n    firstInstallTime=2026-11-19 10:04:31\n    lastUpdateTime=2026-11-19 10:09:02\n";
        assert_eq!(
            first_install_time(dump).as_deref(),
            Some("2026-11-19 10:04:31")
        );
        assert_eq!(first_install_time("Unable to find package: com.a\n"), None);
    }
}
