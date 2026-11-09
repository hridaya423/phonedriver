//! `IosDevice`: the `Device` port over the PhonedriverRunner, Simulator tier (architecture 7, 7.3).

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use phonedriver_core::device::{
    App, AppId, AppOp, Button, Capabilities, ClipboardOp, Device, DeviceEvent, DeviceInfo,
    FrameRef, Gesture, Image, ImageFormat, Key, Quality, Result, ShellReply, ShellRequest,
    TextInput, Url,
};
use phonedriver_core::element::Role;
use phonedriver_core::error::{Error, Op};
use phonedriver_core::geometry::Size;
use phonedriver_core::raw::RawTree;
use phonedriver_video::frame_store::{Frame, FrameStore};
use serde_json::{Value, json};
use tokio::sync::{Notify, broadcast, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use idevice::IdeviceService;
use idevice::provider::UsbmuxdProvider;
use idevice::services::installation_proxy::InstallationProxyClient;

use crate::map;
use crate::runner::{Client, Runner};
use crate::simulator::{
    Simulator, privacy_service, shell_args, simctl, simctl_shell, simctl_with_input,
};

/// Luma pixels per point for settle polls: one sample per 4x4 points, as architecture 4.6 measures change.
const POLL_SCALE: f32 = 0.25;
/// After an input or a visible change, settle polls back to back for this long; after that, at `IDLE_POLL`.
const ACTIVE_WINDOW: Duration = Duration::from_secs(3);
const IDLE_POLL: Duration = Duration::from_millis(500);
const ACTIVE_CHANGE_PT2: f32 = 128.0;
const EVENT_BACKLOG: usize = 16;
/// While an app starts, the runner's snapshot fails with `kAXErrorServerNotFound` until the app's accessibility
/// server answers, or still reports the app that was in front: for about 1.3 s after `simctl launch` returns for
/// Settings (iOS 26.5 Simulator, every launch).
const APP_START_WAIT: Duration = Duration::from_secs(5);
const APP_START_RETRY: Duration = Duration::from_millis(100);
/// Characters per second for paste mode. A synthesized Cmd+V does not paste text another app put on the
/// pasteboard (observed on the iOS 26.5 Simulator), so paste types through the runner's text input at a speed
/// that still entered 102 mixed characters exactly in 267 ms; keys mode types at the runner's default 60.
const PASTE_SPEED: u32 = 5000;

enum Host {
    Simulator,
    Physical(UsbmuxdProvider),
}

pub struct IosDevice {
    host: Host,
    info: DeviceInfo,
    screen: Mutex<Size>,
    udid: String,
    runner: Arc<Runner>,
    store: Arc<FrameStore>,
    activity: Arc<Activity>,
    events: broadcast::Sender<DeviceEvent>,
    poller: JoinHandle<()>,
}

struct Activity {
    last: Mutex<Instant>,
    kick: Notify,
    inputs: AtomicUsize,
}

/// While an input runs, settle polls wait: a poll between the requests of one input (the tree read and the tap of
/// `back`) would push the first frame after the input past the settle detector's first-change window.
struct Busy<'a>(&'a Activity);

impl<'a> Busy<'a> {
    fn new(a: &'a Activity) -> Busy<'a> {
        a.inputs.fetch_add(1, Ordering::SeqCst);
        a.touch();
        Busy(a)
    }
}

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.inputs.fetch_sub(1, Ordering::SeqCst);
        self.0.touch();
    }
}

impl Activity {
    fn busy(&self) -> bool {
        self.inputs.load(Ordering::SeqCst) > 0
    }

    fn touch(&self) {
        *self.last.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
        self.kick.notify_one();
    }

    fn active(&self) -> bool {
        self.last
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .elapsed()
            < ACTIVE_WINDOW
    }
}

impl IosDevice {
    /// What the Simulator tier can do. No `TEXT_SET` (the runner has no set-value), no `FRAMES` (no video
    /// stream; `frames()` carries polled luma thumbnails instead).
    pub const CAPABILITIES: Capabilities = Capabilities::TREE
        .union(Capabilities::SCREENSHOT)
        .union(Capabilities::GESTURE)
        .union(Capabilities::MULTI_TOUCH)
        .union(Capabilities::KEY)
        .union(Capabilities::BUTTON)
        .union(Capabilities::OPEN_URL)
        .union(Capabilities::TEXT_PASTE)
        .union(Capabilities::TEXT_KEYS)
        .union(Capabilities::CLIPBOARD)
        .union(Capabilities::APPS)
        .union(Capabilities::LAUNCH)
        .union(Capabilities::FOREGROUND)
        .union(Capabilities::SHELL);

    /// Starts or reuses the runner on `sim` (task 5.4) and starts settle polling. `home` holds the runner build.
    pub async fn connect(sim: &Simulator, home: &Path) -> Result<IosDevice> {
        let runner = Runner::start(&sim.udid, home).await?;
        IosDevice::with_runner(
            Host::Simulator,
            format!("sim:{}", sim.udid),
            &sim.udid,
            &sim.name,
            runner,
        )
    }

    /// The Runner tier on a wired physical device: builds the runner signed with `team`, starts it and reaches it
    /// through usbmuxd.
    pub async fn connect_physical(
        device: &crate::transport::Wired,
        model: &str,
        team: &str,
        home: &Path,
    ) -> Result<IosDevice> {
        let provider = device.provider();
        let runner = Runner::start_device(&provider, &device.udid, team, home).await?;
        IosDevice::with_runner(
            Host::Physical(provider),
            format!("ios:{}", device.udid),
            &device.udid,
            model,
            runner,
        )
    }

    fn with_runner(
        host: Host,
        id: String,
        udid: &str,
        model: &str,
        runner: Runner,
    ) -> Result<IosDevice> {
        let runner = Arc::new(runner);
        let hello = runner.hello.clone();
        let screen = Size {
            w: hello.screen.w,
            h: hello.screen.h,
        };
        let (events, _) = broadcast::channel(EVENT_BACKLOG);
        let store = Arc::new(FrameStore::new(POLL_SCALE));
        let activity = Arc::new(Activity {
            last: Mutex::new(Instant::now()),
            kick: Notify::new(),
            inputs: AtomicUsize::new(0),
        });
        let poller = tokio::spawn(poll(
            runner.clone(),
            store.clone(),
            activity.clone(),
            events.clone(),
        ));
        Ok(IosDevice {
            host,
            info: DeviceInfo {
                id,
                model: model.to_string(),
                os: format!("iOS {}", hello.ios),
                screen,
                density: hello.screen.scale,
                capabilities: Self::CAPABILITIES,
            },
            screen: Mutex::new(screen),
            udid: udid.to_string(),
            runner,
            store,
            activity,
            events,
            poller,
        })
    }

    fn screen(&self) -> Size {
        *self.screen.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The screen size the snapshot shows, announcing a change (rotation) as `DisplayChanged`.
    fn follow_orientation(&self, reply: &Value) -> Size {
        let Some(now) = map::screen_of(reply) else {
            return self.screen();
        };
        let mut known = self.screen.lock().unwrap_or_else(|e| e.into_inner());
        if *known != now {
            *known = now;
            let _ = self.events.send(DeviceEvent::DisplayChanged {
                screen: now,
                rotation: u8::from(now.w > now.h),
            });
        }
        now
    }

    fn simulator_only(&self) -> Result<()> {
        match self.host {
            Host::Simulator => Ok(()),
            Host::Physical(_) => Err(Error::Unsupported {
                op: Op::App,
                reason: "installing, stopping or removing apps on a physical iPhone is not implemented",
            }),
        }
    }

    fn client(&self) -> &Client {
        &self.runner.client
    }

    async fn input(&self, op: Op, req: Value) -> Result<Value> {
        let _busy = Busy::new(&self.activity);
        self.client().request(op, req).await
    }

    async fn snapshot(&self, max_depth: u32) -> Result<Value> {
        let deadline = Instant::now() + APP_START_WAIT;
        loop {
            let reply = self
                .client()
                .request(Op::Tree, json!({"op": "snapshot", "max_depth": max_depth}))
                .await;
            match reply {
                Err(Error::Transport { detail })
                    if detail.contains("kAXErrorServerNotFound") && Instant::now() < deadline =>
                {
                    tokio::time::sleep(APP_START_RETRY).await
                }
                other => return other,
            }
        }
    }
}

impl Drop for IosDevice {
    fn drop(&mut self) {
        self.poller.abort();
    }
}

/// Feeds the frame store with small luma screenshots: back to back while the screen is busy, slowly when idle.
async fn poll(
    runner: Arc<Runner>,
    store: Arc<FrameStore>,
    activity: Arc<Activity>,
    events: broadcast::Sender<DeviceEvent>,
) {
    loop {
        if activity.busy() {
            activity.kick.notified().await;
            continue;
        }
        if !activity.active() {
            tokio::select! {
                () = tokio::time::sleep(IDLE_POLL) => {}
                () = activity.kick.notified() => {}
            }
        }
        let reply = runner
            .client
            .request(
                Op::Screenshot,
                json!({"op": "screenshot", "format": "luma", "scale": POLL_SCALE}),
            )
            .await;
        match reply.as_ref().map(luma_frame) {
            Ok(Some(frame)) => {
                if store.publish(frame).changed_area_pt2 > ACTIVE_CHANGE_PT2 {
                    activity.touch();
                }
            }
            Ok(None) | Err(Error::Transport { .. }) => tokio::time::sleep(IDLE_POLL).await,
            Err(_) => {
                let _ = events.send(DeviceEvent::Gone);
                return;
            }
        }
    }
}

fn luma_frame(reply: &Value) -> Option<Frame> {
    let (w, h) = (reply["w"].as_u64()? as u32, reply["h"].as_u64()? as u32);
    let y = base64::engine::general_purpose::STANDARD
        .decode(reply["data_b64"].as_str()?)
        .ok()?;
    if y.len() != (w * h) as usize {
        return None;
    }
    let chroma = (w.div_ceil(2) * h.div_ceil(2)) as usize;
    Some(Frame::from_i420(
        w,
        h,
        y,
        vec![128; chroma],
        vec![128; chroma],
    ))
}

async fn installed_apps(provider: &UsbmuxdProvider, id: &str) -> Result<Vec<App>> {
    let gone = |e| crate::transport::classify(&id.to_string(), &e);
    let socket =
        crate::transport::lockdown_service(provider, InstallationProxyClient::service_name())
            .await
            .map_err(gone)?;
    let mut client = InstallationProxyClient::new(socket);
    let found = client.get_apps(None, None).await.map_err(gone)?;
    let mut apps: Vec<App> = found
        .iter()
        .map(|(id, info)| {
            let field = |k: &str| {
                info.as_dictionary()
                    .and_then(|d| d.get(k))
                    .and_then(|v| v.as_string())
            };
            App {
                id: AppId(id.clone()),
                name: field("CFBundleDisplayName")
                    .or(field("CFBundleName"))
                    .unwrap_or(id)
                    .to_string(),
                system: field("ApplicationType") == Some("System"),
            }
        })
        .collect();
    apps.sort_by(|a, b| a.id.0.cmp(&b.id.0));
    Ok(apps)
}

#[async_trait]
impl Device for IosDevice {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    async fn tree(&self) -> Result<RawTree> {
        let reply = self.snapshot(30).await?;
        let screen = self.follow_orientation(&reply);
        map::raw_tree(&reply, screen)
    }

    fn current_screen(&self) -> Option<Size> {
        Some(self.screen())
    }

    fn frames(&self) -> Option<watch::Receiver<FrameRef>> {
        Some(self.store.subscribe())
    }

    async fn screenshot(&self, _q: Quality) -> Result<Image> {
        let reply = self
            .client()
            .request(
                Op::Screenshot,
                json!({"op": "screenshot", "format": "png", "scale": 1}),
            )
            .await?;
        let bytes = reply["data_b64"]
            .as_str()
            .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok())
            .ok_or_else(|| Error::Transport {
                detail: "the runner's screenshot is not base64".into(),
            })?;
        Ok(Image {
            width: reply["w"].as_u64().unwrap_or_default() as u32,
            height: reply["h"].as_u64().unwrap_or_default() as u32,
            format: ImageFormat::Png,
            bytes,
        })
    }

    fn events(&self) -> broadcast::Receiver<DeviceEvent> {
        self.events.subscribe()
    }

    async fn gesture(&self, g: Gesture) -> Result<()> {
        self.input(Op::Gesture, map::gesture_request(&g)).await?;
        Ok(())
    }

    async fn key(&self, k: Key) -> Result<()> {
        let req = match k {
            Key::Enter => json!({"op": "type", "text": "\n"}),
            k => json!({"op": "key", "key": map::key_string(k), "modifiers": []}),
        };
        self.input(Op::Key, req).await?;
        Ok(())
    }

    async fn button(&self, b: Button) -> Result<()> {
        let name = match b {
            Button::Home => "home",
            Button::Lock => "lock",
            Button::VolumeUp => "volume_up",
            Button::VolumeDown => "volume_down",
            Button::Back => return self.back().await.map(|_| ()),
            Button::Notifications => {
                return self.gesture(map::notifications_swipe(self.screen())).await;
            }
            Button::Recents => {
                return Err(Error::Unsupported {
                    op: Op::Button,
                    reason: "iOS has no recents button",
                });
            }
        };
        self.input(Op::Button, json!({"op": "button", "name": name}))
            .await?;
        Ok(())
    }

    async fn hide_keyboard(&self) -> Result<Option<&'static str>> {
        let _busy = Busy::new(&self.activity);
        let reply = self.snapshot(30).await?;
        match map::hide_keyboard_plan(&reply, self.screen()) {
            Some(map::HidePlan::Tap(at)) => {
                self.gesture(phonedriver_core::gesture::tap(at, 1, None))
                    .await?;
                Ok(Some("hide-key"))
            }
            Some(map::HidePlan::Done(at)) => {
                self.gesture(phonedriver_core::gesture::tap(at, 1, None))
                    .await?;
                Ok(Some("done"))
            }
            Some(map::HidePlan::Drag(g)) => {
                self.gesture(g).await?;
                Ok(Some("drag"))
            }
            None => Ok(None),
        }
    }

    async fn back(&self) -> Result<Option<&'static str>> {
        let _busy = Busy::new(&self.activity);
        let tree = self.tree().await?;
        if tree.nodes.iter().any(|n| n.role == Role::Keyboard) {
            return self.hide_keyboard().await;
        }
        match map::back_button(&tree) {
            Some(at) => {
                self.gesture(Gesture {
                    fingers: vec![vec![
                        phonedriver_core::device::TimedPoint { at, t_ms: 0 },
                        phonedriver_core::device::TimedPoint { at, t_ms: 0 },
                    ]],
                })
                .await?;
                Ok(Some("nav-button"))
            }
            None => {
                self.gesture(map::back_swipe(self.screen())).await?;
                Ok(Some("edge-swipe"))
            }
        }
    }

    async fn open_url(&self, url: &Url, _app: Option<&AppId>) -> Result<Option<AppId>> {
        self.input(Op::OpenUrl, json!({"op": "open_url", "url": url.0}))
            .await?;
        Ok(map::url_handler(
            &url.0,
            matches!(self.host, Host::Simulator),
        ))
    }

    async fn text(&self, t: TextInput) -> Result<()> {
        let req = match &t {
            TextInput::Paste(text) | TextInput::PasteSecret(text) => {
                json!({"op": "type", "text": text, "speed": PASTE_SPEED})
            }
            TextInput::Keystrokes(text) => json!({"op": "type", "text": text}),
            TextInput::SetValue(_) => {
                return Err(Error::Unsupported {
                    op: Op::Text,
                    reason: "the iOS runner cannot set a field's value; use paste or keys",
                });
            }
        };
        self.input(Op::Text, req).await?;
        Ok(())
    }

    async fn clipboard(&self, op: ClipboardOp) -> Result<Option<String>> {
        if matches!(self.host, Host::Physical(_)) {
            return Err(Error::Unsupported {
                op: Op::Clipboard,
                reason: "the runner tier has no clipboard access on a physical device",
            });
        }
        match op {
            ClipboardOp::Get => Ok(Some(simctl(&["pbpaste", &self.udid]).await?)),
            ClipboardOp::Set(text) => {
                simctl_with_input(&["pbcopy", &self.udid], Some(&text)).await?;
                Ok(None)
            }
        }
    }

    async fn apps(&self) -> Result<Vec<App>> {
        if let Host::Physical(provider) = &self.host {
            return Box::pin(installed_apps(provider, &self.info.id)).await;
        }
        let plist = simctl(&["listapps", &self.udid]).await?;
        let json = crate::simulator::plist_to_json(&plist).await?;
        Ok(map::apps(&json))
    }

    async fn launch(&self, app: &AppId) -> Result<()> {
        {
            let _busy = Busy::new(&self.activity);
            match self.host {
                Host::Simulator => {
                    simctl(&["launch", &self.udid, &app.0]).await?;
                }
                Host::Physical(_) => {
                    self.client()
                        .request(Op::Launch, json!({"op": "activate", "bundle_id": app.0}))
                        .await?;
                }
            }
        }
        let deadline = Instant::now() + APP_START_WAIT;
        while Instant::now() < deadline && self.foreground().await?.as_ref() != Some(app) {
            tokio::time::sleep(APP_START_RETRY).await;
        }
        Ok(())
    }

    async fn install(&self, path: &Path) -> Result<Option<AppId>> {
        self.simulator_only()?;
        let app = path.to_string_lossy().into_owned();
        if !path.is_dir() || path.extension().is_none_or(|e| e != "app") {
            return Err(Error::Unsupported {
                op: Op::App,
                reason: "the Simulator installs an .app folder",
            });
        }
        simctl(&["install", &self.udid, &app]).await?;
        let plist = format!("{app}/Info.plist");
        let id = crate::simulator::plutil_extract(&plist, "CFBundleIdentifier").await?;
        Ok(Some(AppId(id)))
    }

    async fn shell(&self, req: &ShellRequest) -> Result<ShellReply> {
        if matches!(self.host, Host::Physical(_)) {
            return Err(Error::Unsupported {
                op: Op::Shell,
                reason: "an iPhone or iPad has no shell for anyone to run commands in; use launch, open, type and the other verbs, or a Simulator",
            });
        }
        let args =
            shell_args(&self.udid, &req.command).map_err(|detail| Error::Transport { detail })?;
        let (exit, stdout, stderr, timed_out) = simctl_shell(&args, req.timeout).await?;
        Ok(ShellReply {
            exit,
            stdout,
            stderr,
            timed_out,
        })
    }

    async fn app_op(&self, op: AppOp, app: &AppId) -> Result<()> {
        self.simulator_only()?;
        match op {
            AppOp::Stop => {
                simctl(&["terminate", &self.udid, &app.0]).await?;
            }
            AppOp::ClearData => {
                let _ = simctl(&["terminate", &self.udid, &app.0]).await;
                let dir = simctl(&["get_app_container", &self.udid, &app.0, "data"]).await?;
                let dir = std::path::PathBuf::from(dir.trim());
                for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                    let p = entry.path();
                    let _ = if p.is_dir() {
                        std::fs::remove_dir_all(&p)
                    } else {
                        std::fs::remove_file(&p)
                    };
                }
            }
            AppOp::Uninstall => {
                simctl(&["uninstall", &self.udid, &app.0]).await?;
            }
        }
        Ok(())
    }

    async fn grant(&self, app: &AppId, permissions: &[String]) -> Result<()> {
        self.simulator_only()?;
        for name in permissions {
            let service = privacy_service(name)?;
            simctl(&["privacy", &self.udid, "grant", service, &app.0]).await?;
        }
        Ok(())
    }

    async fn foreground(&self) -> Result<Option<AppId>> {
        let reply = self.snapshot(0).await?;
        Ok(reply["app"]["bundle_id"]
            .as_str()
            .map(|b| AppId(b.to_string())))
    }

    async fn capture_tree(&self) -> Result<Option<String>> {
        let mut reply = self.snapshot(30).await?;
        if let Some(map) = reply.as_object_mut() {
            map.remove("id");
            map.remove("ok");
        }
        Ok(Some(reply.to_string()))
    }
}
