//! `IosDevice`: the `Device` port over the PhoneUseRunner, Simulator tier (architecture 7, 7.3).

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use phone_use_core::device::{
    App, AppId, Button, Capabilities, ClipboardOp, Device, DeviceEvent, DeviceInfo, FrameRef,
    Gesture, Image, ImageFormat, Key, Quality, Result, TextInput, Url,
};
use phone_use_core::error::{Error, Op};
use phone_use_core::geometry::Size;
use phone_use_core::raw::RawTree;
use phone_use_video::frame_store::{Frame, FrameStore};
use serde_json::{Value, json};
use tokio::sync::{Notify, broadcast, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::map;
use crate::runner::{Client, Runner};
use crate::simulator::{Simulator, simctl, simctl_with_input};

/// Luma pixels per point for settle polls: one sample per 4x4 points, as architecture 4.6 measures change.
const POLL_SCALE: f32 = 0.25;
/// After an input or a visible change, settle polls back to back for this long; after that, at `IDLE_POLL`.
const ACTIVE_WINDOW: Duration = Duration::from_secs(3);
const IDLE_POLL: Duration = Duration::from_millis(500);
const ACTIVE_CHANGE_PT2: f32 = 128.0;
const EVENT_BACKLOG: usize = 16;
/// Characters per second for paste mode. A synthesized Cmd+V does not paste text another app put on the
/// pasteboard (observed on the iOS 26.5 Simulator), so paste types through the runner's text input at a speed
/// that still entered 102 mixed characters exactly in 267 ms; keys mode types at the runner's default 60.
const PASTE_SPEED: u32 = 5000;

pub struct IosDevice {
    info: DeviceInfo,
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
        .union(Capabilities::FOREGROUND);

    /// Starts or reuses the runner on `sim` (task 5.4) and starts settle polling. `home` holds the runner build.
    pub async fn connect(sim: &Simulator, home: &Path) -> Result<IosDevice> {
        let runner = Arc::new(Runner::start(&sim.udid, home).await?);
        let hello = runner.hello.clone();
        let screen = Size {
            w: hello.screen.w,
            h: hello.screen.h,
        };
        let id = format!("sim:{}", sim.udid);
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
            info: DeviceInfo {
                id,
                model: sim.name.clone(),
                os: format!("iOS {}", hello.ios),
                screen,
                density: hello.screen.scale,
                capabilities: Self::CAPABILITIES,
            },
            udid: sim.udid.clone(),
            runner,
            store,
            activity,
            events,
            poller,
        })
    }

    fn client(&self) -> &Client {
        &self.runner.client
    }

    async fn input(&self, op: Op, req: Value) -> Result<Value> {
        let _busy = Busy::new(&self.activity);
        self.client().request(op, req).await
    }

    async fn snapshot(&self, max_depth: u32) -> Result<Value> {
        self.client()
            .request(Op::Tree, json!({"op": "snapshot", "max_depth": max_depth}))
            .await
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

#[async_trait]
impl Device for IosDevice {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    async fn tree(&self) -> Result<RawTree> {
        let reply = self.snapshot(30).await?;
        map::raw_tree(&reply, self.info.screen)
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
                return self
                    .gesture(map::notifications_swipe(self.info.screen))
                    .await;
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

    async fn back(&self) -> Result<Option<&'static str>> {
        let _busy = Busy::new(&self.activity);
        let tree = self.tree().await?;
        match map::back_button(&tree) {
            Some(at) => {
                self.gesture(Gesture {
                    fingers: vec![vec![
                        phone_use_core::device::TimedPoint { at, t_ms: 0 },
                        phone_use_core::device::TimedPoint { at, t_ms: 0 },
                    ]],
                })
                .await?;
                Ok(Some("nav-button"))
            }
            None => {
                self.gesture(map::back_swipe(self.info.screen)).await?;
                Ok(Some("edge-swipe"))
            }
        }
    }

    async fn open_url(&self, url: &Url) -> Result<()> {
        self.input(Op::OpenUrl, json!({"op": "open_url", "url": url.0}))
            .await?;
        Ok(())
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
        match op {
            ClipboardOp::Get => Ok(Some(simctl(&["pbpaste", &self.udid]).await?)),
            ClipboardOp::Set(text) => {
                simctl_with_input(&["pbcopy", &self.udid], Some(&text)).await?;
                Ok(None)
            }
        }
    }

    async fn apps(&self) -> Result<Vec<App>> {
        let plist = simctl(&["listapps", &self.udid]).await?;
        let json = crate::simulator::plist_to_json(&plist).await?;
        Ok(map::apps(&json))
    }

    async fn launch(&self, app: &AppId) -> Result<()> {
        let _busy = Busy::new(&self.activity);
        simctl(&["launch", &self.udid, &app.0]).await?;
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
