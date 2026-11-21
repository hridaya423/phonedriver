//! `IosDevice`: the `Device` port over the PhonedriverRunner, Simulator tier (architecture 7, 7.3).

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use phonedriver_core::device::{
    App, AppId, AppOp, Button, Capabilities, ClipboardOp, Device, DeviceEvent, DeviceInfo,
    FrameRef, Gesture, Image, ImageFormat, Key, Quality, Recorded, Result, ScreenState, ShellReply,
    ShellRequest, TextInput, Url, host_shell,
};
use phonedriver_core::element::Role;
use phonedriver_core::error::{Error, Op};
use phonedriver_core::geometry::Size;
use phonedriver_core::network::{LogWait, LogWaited};
use phonedriver_core::raw::RawTree;
use phonedriver_video::frame_store::{Frame, FrameStore};
use serde_json::{Value, json};
use tokio::sync::{Notify, broadcast, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use idevice::provider::IdeviceProvider;
use idevice::services::installation_proxy::InstallationProxyClient;

use crate::map;
use crate::runner::{Client, Runner};
use crate::simio::{Frames, Seen, SimIo};
use crate::simulator::{
    PHYSICAL_PUSH, Simulator, check_push_payload, log_stream_command, privacy_service, shell_args,
    simctl, simctl_follow, simctl_shell, simctl_with_input, spawn_args,
};
use crate::transport::{Link, Services, Tunnel};

/// Luma pixels per point for settle polls: one sample per 4x4 points, as architecture 4.6 measures change.
const POLL_SCALE: f32 = 0.25;
/// After an input or a visible change, settle polls back to back for this long; after that, at `IDLE_POLL`.
const ACTIVE_WINDOW: Duration = Duration::from_secs(3);
const IDLE_POLL: Duration = Duration::from_millis(500);
const RECORDER_START: Duration = Duration::from_secs(10);
const RECORDER_FINISH: Duration = Duration::from_secs(30);
/// For this long after the host last used the device, the poller keeps its screen awake (decision D56). It is the
/// length of a session's hold on a device, so a phone does not lock under an agent that still holds it.
const KEEP_AWAKE: Duration = Duration::from_secs(600);
/// Auto-Lock's shortest setting is 30 s; a nudge every 10 s held an iPhone 13 Pro Max awake at that setting.
const NUDGE_EVERY: Duration = Duration::from_secs(10);

fn nudge_is_due(since_use: Duration, since_nudge: Duration, locked: bool) -> bool {
    !locked && since_use < KEEP_AWAKE && since_nudge >= NUDGE_EVERY
}
const ACTIVE_CHANGE_PT2: f32 = 128.0;
const EVENT_BACKLOG: usize = 16;
/// While an app starts, the runner's snapshot fails with `kAXErrorServerNotFound` until the app's accessibility
/// server answers, or still reports the app that was in front: for about 1.3 s after `simctl launch` returns for
/// Settings (iOS 26.5 Simulator, every launch).
const APP_START_WAIT: Duration = Duration::from_secs(5);
const APP_START_RETRY: Duration = Duration::from_millis(100);
/// Characters per second for paste mode. A synthesized Cmd+V does not paste text another app put on the
/// pasteboard (observed on the iOS 26.5 Simulator), so paste types through the runner's text input at a speed
/// that still entered 102 mixed characters exactly in 267 ms; keys mode types at the runner's default 60. Not through
/// the Simulator's HID keys: those lost shift and took smart punctuation where the runner's text was exact (D93).
const PASTE_SPEED: u32 = 5000;

enum Host {
    Simulator,
    Cable(Arc<dyn IdeviceProvider>),
    /// Over the network the device closes every lockdown service after the TLS handshake (observed 2026-11-12,
    /// iOS 27.0.1: amfi, the image mounter, the installation proxy), so only the runner's own port is used.
    Network,
    /// Off a Mac over Wi-Fi: the tunnel of the remote pairing reaches the services lockdown closes (decision D103).
    Tunnel(Tunnel),
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
    recording: tokio::sync::Mutex<Option<SimRecording>>,
    simio: Option<tokio::sync::Mutex<SimIo>>,
    system: tokio::sync::OnceCell<std::path::PathBuf>,
}

/// `xcrun simctl io <udid> recordVideo` while it runs. It finishes its file when it is interrupted, not when it
/// is killed.
struct SimRecording {
    recorder: tokio::process::Child,
    path: std::path::PathBuf,
}

impl SimRecording {
    fn interrupt(&self) {
        if let Some(pid) = self.recorder.id() {
            let _ = std::process::Command::new("kill")
                .args(["-INT", &pid.to_string()])
                .status();
        }
    }
}

struct Activity {
    last: Mutex<Instant>,
    /// When the host last asked the device for something; screen changes do not count.
    used: Mutex<Instant>,
    kick: Notify,
    inputs: AtomicUsize,
    /// The lock state in the runner's last hello or screenshot reply.
    locked: AtomicBool,
    /// While simio follows the Simulator's screen buffer and lock state, the poller leaves the runner alone.
    followed: AtomicBool,
}

/// While an input runs, settle polls wait: a poll between the requests of one input (the tree read and the tap of
/// `back`) would push the first frame after the input past the settle detector's first-change window.
struct Busy<'a>(&'a Activity);

impl<'a> Busy<'a> {
    fn new(a: &'a Activity) -> Busy<'a> {
        a.inputs.fetch_add(1, Ordering::SeqCst);
        a.used();
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

    fn used(&self) {
        *self.used.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
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
        .union(Capabilities::SHELL)
        .union(Capabilities::POWER_EVENTS);

    /// Starts or reuses the runner on `sim` (task 5.4) and starts settle polling. `home` holds the runner build.
    pub async fn connect(sim: &Simulator, home: &Path) -> Result<IosDevice> {
        let runner = Runner::start(&sim.udid, home).await?;
        let simio = match SimIo::start(home, &sim.udid).await {
            Ok(io) => Some(io),
            Err(e) => {
                eprintln!("touches on {} go through XCTest: {e}", sim.udid);
                None
            }
        };
        let frames = match Frames::start(home, &sim.udid, POLL_SCALE).await {
            Ok(f) => Some(f),
            Err(e) => {
                eprintln!("the screen of {} is polled through XCTest: {e}", sim.udid);
                None
            }
        };
        IosDevice::with_runner(
            Host::Simulator,
            format!("sim:{}", sim.udid),
            &sim.udid,
            &sim.name,
            runner,
            simio,
            frames,
        )
    }

    /// The Runner tier on a attached physical device: builds the runner signed with `team`, starts it and reaches its
    /// port over the cable or the network.
    pub async fn connect_physical(
        device: &crate::transport::Attached,
        model: &str,
        team: &str,
        home: &Path,
    ) -> Result<IosDevice> {
        let id = format!("ios:{}", device.udid);
        let provider = device
            .provider()
            .await
            .map_err(|e| crate::transport::classify(&id, &e))?;
        let runner = Runner::start_device(provider.clone(), &device.udid, team, home).await?;
        let host = match device.link {
            Link::Usb { .. } => Host::Cable(provider),
            Link::Network { .. } => Host::Network,
        };
        IosDevice::with_runner(host, id, &device.udid, model, runner, None, None)
    }

    /// A device whose runner the person installed, started without Xcode (architecture 7.3): on the cable, or over
    /// Wi-Fi through the remote pairing `phonedriver ios wifi on` made (decision D103).
    pub async fn connect_installed(
        device: &crate::transport::Attached,
        model: &str,
        home: &Path,
    ) -> Result<IosDevice> {
        let id = format!("ios:{}", device.udid);
        let mut services = Services::open(device, home).await?;
        let runner = Runner::start_installed(device, &mut services, home).await?;
        let host = match services {
            Services::Lockdown(provider) => Host::Cable(provider),
            Services::Tunnel(tunnel) => Host::Tunnel(tunnel),
        };
        IosDevice::with_runner(host, id, &device.udid, model, runner, None, None)
    }

    fn with_runner(
        host: Host,
        id: String,
        udid: &str,
        model: &str,
        runner: Runner,
        simio: Option<SimIo>,
        frames: Option<Frames>,
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
            used: Mutex::new(Instant::now()),
            kick: Notify::new(),
            inputs: AtomicUsize::new(0),
            locked: AtomicBool::new(hello.locked),
            followed: AtomicBool::new(frames.is_some()),
        });
        if let Some(frames) = frames {
            tokio::spawn(follow(
                frames,
                store.clone(),
                activity.clone(),
                events.clone(),
            ));
        }
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
            recording: tokio::sync::Mutex::new(None),
            simio: simio.map(tokio::sync::Mutex::new),
            system: tokio::sync::OnceCell::new(),
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
            Host::Cable(_) | Host::Network | Host::Tunnel(_) => Err(Error::Unsupported {
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

    /// Whether a notification banner covers where the path starts, or the runner could not say. The runner waits for
    /// one to leave before its own gestures; a touch through the HID port would land on it.
    async fn banner_covers(&self, path: &[phonedriver_core::device::TimedPoint]) -> bool {
        let Some(p) = path.first().filter(|p| p.at.y <= map::BANNER_ZONE) else {
            return false;
        };
        let reply = self
            .client()
            .request(
                Op::Gesture,
                json!({"op": "banner", "x": p.at.x, "y": p.at.y}),
            )
            .await;
        !matches!(reply, Ok(r) if r["covered"] == false)
    }

    async fn snapshot(&self, max_depth: u32) -> Result<Value> {
        self.activity.used();
        let deadline = Instant::now() + APP_START_WAIT;
        loop {
            let reply = self
                .client()
                .request(Op::Tree, json!({"op": "snapshot", "max_depth": max_depth}))
                .await;
            match reply {
                Err(e) if map::app_not_readable_yet(&e) && Instant::now() < deadline => {
                    tokio::time::sleep(APP_START_RETRY).await
                }
                other => return other,
            }
        }
    }
}

impl IosDevice {
    /// Shows the Simulator's Face ID or Touch ID a face or finger that matches, or one that does not (decision
    /// D69), through the notifications the Simulator's own Features menu posts. A Simulator with nothing enrolled
    /// is enrolled first, since an app asks only when something is.
    async fn show_face(&self, matches: bool) -> Result<()> {
        const ENROLLED: &str = "com.apple.BiometricKit.enrollmentChanged";
        if !matches!(self.host, Host::Simulator) {
            return Err(Error::Unsupported {
                op: Op::Button,
                reason: "an iPhone's Face ID and Touch ID take their owner; only a Simulator's can be answered from here",
            });
        }
        let notify = async |args: &[&str]| {
            let mut all = vec!["spawn", self.udid.as_str(), "notifyutil"];
            all.extend_from_slice(args);
            simctl(&all).await
        };
        if !notify(&["-g", ENROLLED]).await?.trim_end().ends_with(" 1") {
            notify(&["-s", ENROLLED, "1"]).await?;
            notify(&["-p", ENROLLED]).await?;
        }
        let outcome = if matches { "match" } else { "nomatch" };
        // `pearl` is Face ID and `fingerTouch` is Touch ID; a Simulator listens to the one it has.
        for sensor in ["pearl", "fingerTouch"] {
            notify(&[
                "-p",
                &format!("com.apple.BiometricKit_Sim.{sensor}.{outcome}"),
            ])
            .await?;
        }
        Ok(())
    }
}

impl Drop for IosDevice {
    fn drop(&mut self) {
        self.poller.abort();
        if let Some(recording) = self.recording.get_mut().as_ref() {
            recording.interrupt();
        }
    }
}

/// Feeds the frame store with simio's pictures of the Simulator's screen buffer, and announces its lock state,
/// until the helper stops.
async fn follow(
    mut frames: Frames,
    store: Arc<FrameStore>,
    activity: Arc<Activity>,
    events: broadcast::Sender<DeviceEvent>,
) {
    while let Some(seen) = frames.next().await {
        match seen {
            Seen::Picture(l) => {
                let chroma = (l.width.div_ceil(2) * l.height.div_ceil(2)) as usize;
                let frame =
                    Frame::from_i420(l.width, l.height, l.y, vec![128; chroma], vec![128; chroma]);
                if store.publish(frame).changed_area_pt2 > ACTIVE_CHANGE_PT2 {
                    activity.touch();
                }
            }
            Seen::Locked(locked) => {
                if activity.locked.swap(locked, Ordering::Relaxed) != locked {
                    let _ = events.send(DeviceEvent::Locked(locked));
                }
            }
        }
    }
    activity.followed.store(false, Ordering::SeqCst);
}

/// Feeds the frame store with small luma screenshots: back to back while the screen is busy, slowly when idle.
/// While simio follows the screen, it only keeps the screen awake.
async fn poll(
    runner: Arc<Runner>,
    store: Arc<FrameStore>,
    activity: Arc<Activity>,
    events: broadcast::Sender<DeviceEvent>,
) {
    let mut nudged = Instant::now();
    loop {
        if activity.busy() {
            activity.kick.notified().await;
            continue;
        }
        let since_use = activity
            .used
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .elapsed();
        if nudge_is_due(
            since_use,
            nudged.elapsed(),
            activity.locked.load(Ordering::Relaxed),
        ) {
            nudged = Instant::now();
            let _ = runner.client.request(Op::Key, json!({"op": "awake"})).await;
        }
        if activity.followed.load(Ordering::SeqCst) {
            tokio::time::sleep(IDLE_POLL).await;
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
        if let Ok(locked) = reply.as_ref().map(|r| r["locked"] == true)
            && activity.locked.swap(locked, Ordering::Relaxed) != locked
        {
            let _ = events.send(DeviceEvent::Locked(locked));
        }
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

async fn installed_apps(services: &mut Services, id: &str) -> Result<Vec<App>> {
    let gone = |e| crate::transport::classify(&id.to_string(), &e);
    let mut client = services
        .client::<InstallationProxyClient>()
        .await
        .map_err(gone)?;
    let found = client.get_apps(None, None).await.map_err(gone)?;
    let mut apps: Vec<App> = found
        .iter()
        .filter(|(_, info)| {
            let tags: Vec<&str> = info
                .as_dictionary()
                .and_then(|d| d.get("SBAppTags"))
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|t| t.as_string())
                .collect();
            !crate::map::hidden(&tags)
        })
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

    /// While simio follows the Simulator's screen; the runner's polled screenshots fall behind it.
    fn frames_follow(&self) -> bool {
        self.activity.followed.load(Ordering::Relaxed)
    }

    fn frames(&self) -> Option<watch::Receiver<FrameRef>> {
        Some(self.store.subscribe())
    }

    /// The Simulator records itself: `simctl io recordVideo` writes H.264 into an MP4. It says `Recording started`
    /// about 300 ms after it is run, and this waits for that line so that the next action is in the video.
    async fn record_start(&self, path: &Path) -> Result<()> {
        use tokio::io::{AsyncBufReadExt, BufReader};
        if !matches!(self.host, Host::Simulator) {
            return Err(Error::Unsupported {
                op: Op::Record,
                reason: "recording an iPhone's screen is not built; the iOS Simulator and Android devices record",
            });
        }
        let mut slot = self.recording.lock().await;
        if let Some(running) = slot.as_mut()
            && matches!(running.recorder.try_wait(), Ok(None))
        {
            return Err(Error::Refused {
                detail: "a recording is already running".into(),
            });
        }
        let mut recorder = tokio::process::Command::new("xcrun")
            .args([
                "simctl",
                "io",
                &self.udid,
                "recordVideo",
                "--codec=h264",
                "--force",
            ])
            .arg(path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| Error::Transport {
                detail: format!("cannot run xcrun simctl (is Xcode installed?): {e}"),
            })?;
        let mut said = BufReader::new(recorder.stderr.take().ok_or_else(|| Error::Transport {
            detail: "the Simulator's recorder has no output to read".into(),
        })?)
        .lines();
        let mut last = String::new();
        let started = tokio::time::timeout(RECORDER_START, async {
            while let Ok(Some(line)) = said.next_line().await {
                if line.contains("Recording started") {
                    return true;
                }
                last = line;
            }
            false
        })
        .await
        .unwrap_or(false);
        if !started {
            let _ = recorder.kill().await;
            return Err(Error::Transport {
                detail: format!("the Simulator's recorder did not start: {last}"),
            });
        }
        // The recorder writes two more lines when it is interrupted; with nobody reading them it would be
        // stopped by the closed pipe before it finishes the file.
        tokio::spawn(async move { while let Ok(Some(_)) = said.next_line().await {} });
        *slot = Some(SimRecording {
            recorder,
            path: path.to_path_buf(),
        });
        Ok(())
    }

    async fn record_stop(&self) -> Result<Option<Recorded>> {
        let Some(mut recording) = self.recording.lock().await.take() else {
            return Ok(None);
        };
        let early = matches!(recording.recorder.try_wait(), Ok(Some(_)))
            .then(|| "the Simulator's recorder had stopped by itself".to_string());
        if early.is_none() {
            recording.interrupt();
        }
        if tokio::time::timeout(RECORDER_FINISH, recording.recorder.wait())
            .await
            .is_err()
        {
            let _ = recording.recorder.kill().await;
            return Err(Error::Transport {
                detail: "the Simulator's recorder did not finish its file".into(),
            });
        }
        let read =
            phonedriver_video::record::probe(&recording.path).map_err(|e| Error::Transport {
                detail: format!("cannot read {}: {e}", recording.path.display()),
            })?;
        Ok(Some(Recorded {
            path: read.path,
            width: read.width,
            height: read.height,
            frames: read.frames,
            ms: read.ms,
            early,
        }))
    }

    async fn screenshot(&self, _q: Quality) -> Result<Image> {
        self.activity.used();
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
        if let (Some(simio), [path]) = (&self.simio, g.fingers.as_slice())
            && map::hid_reaches(path, self.screen())
            && !self.banner_covers(path).await
        {
            let _busy = Busy::new(&self.activity);
            return simio.lock().await.touch(path).await;
        }
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
            Button::Landscape => "landscape",
            Button::Portrait => "portrait",
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
            Button::Fingerprint | Button::WrongFingerprint => {
                return self.show_face(b == Button::Fingerprint).await;
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
        if matches!(self.host, Host::Simulator)
            && names_a_mac_file(&url.0)
            && !phonedriver_core::device::host_shell()
        {
            return Err(Error::Refused {
                detail: format!(
                    "a file: link on a Simulator opens a file of this Mac, not of the device; start the daemon with {}=1 to allow it",
                    phonedriver_core::device::HOST_SHELL
                ),
            });
        }
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
            TextInput::SetValue(_) | TextInput::SetSecret(_) => {
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
        if !matches!(self.host, Host::Simulator) {
            return Err(Error::Unsupported {
                op: Op::Clipboard,
                reason: "the runner tier has no clipboard access on a physical device",
            });
        }
        match op {
            ClipboardOp::Get => Ok(Some(simctl(&["pbpaste", &self.udid]).await?)),
            ClipboardOp::Set(text) => {
                simctl_with_input(&["pbcopy", &self.udid], Some(&text)).await?;
                // Seen on a Mac mini with Xcode 27, 2026-11-16: `simctl pbcopy` exited 0 on every booted simulator
                // and `pbpaste` stayed empty, also after the simulator was shut down and booted again.
                if simctl(&["pbpaste", &self.udid]).await? != text {
                    return Err(Error::Refused {
                        detail: "the Simulator's clipboard did not take the text: `xcrun simctl pbcopy` reported success and `pbpaste` gives something else".into(),
                    });
                }
                Ok(None)
            }
        }
    }

    async fn apps(&self) -> Result<Vec<App>> {
        match &self.host {
            Host::Cable(provider) => {
                let mut services = Services::Lockdown(provider.clone());
                return Box::pin(installed_apps(&mut services, &self.info.id)).await;
            }
            Host::Tunnel(tunnel) => {
                let mut services = Services::Tunnel(tunnel.clone());
                return Box::pin(installed_apps(&mut services, &self.info.id)).await;
            }
            Host::Network => {
                let json =
                    crate::setup::devicectl(&["info", "apps", "--include-all-apps"], &self.udid)
                        .await?;
                return Ok(map::devicectl_apps(&json));
            }
            Host::Simulator => {}
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
                Host::Cable(_) | Host::Network | Host::Tunnel(_) => {
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

    async fn install(&self, path: &Path) -> Result<Vec<AppId>> {
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
        Ok(vec![AppId(id)])
    }

    async fn shell(&self, req: &ShellRequest) -> Result<ShellReply> {
        if !matches!(self.host, Host::Simulator) {
            return Err(Error::Unsupported {
                op: Op::Shell,
                reason: "an iPhone or iPad has no shell for anyone to run commands in; use launch, open, type and the other verbs, or a Simulator",
            });
        }
        let system = self
            .system
            .get_or_try_init(|| crate::simulator::runtime_root(&self.udid))
            .await?;
        let args = shell_args(&self.udid, &req.command, system, host_shell())
            .map_err(|detail| Error::Refused { detail })?;
        let (exit, stdout, stderr, timed_out) = simctl_shell(&args, req.timeout).await?;
        Ok(ShellReply {
            exit,
            stdout,
            stderr,
            timed_out,
        })
    }

    async fn log_wait(&self, req: &LogWait) -> Result<LogWaited> {
        if !matches!(self.host, Host::Simulator) {
            return Err(Error::Unsupported {
                op: Op::Logs,
                reason: "an iPhone or iPad's log cannot be followed from here; the iOS Simulator's and Android devices' can",
            });
        }
        let folder = match &req.app {
            Some(app) => {
                let path = simctl(&["get_app_container", &self.udid, &app.0, "app"]).await?;
                Some(
                    Path::new(path.trim())
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                )
            }
            None => None,
        };
        let command =
            log_stream_command(folder.as_deref()).map_err(|detail| Error::Refused { detail })?;
        let system = self
            .system
            .get_or_try_init(|| crate::simulator::runtime_root(&self.udid))
            .await?;
        let args =
            spawn_args(&self.udid, &command, system).map_err(|detail| Error::Refused { detail })?;
        simctl_follow(&args, req).await
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
                clear_folder(Path::new(dir.trim()))?;
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

    async fn push(&self, app: &AppId, payload: &str) -> Result<()> {
        if !matches!(self.host, Host::Simulator) {
            return Err(Error::Unsupported {
                op: Op::App,
                reason: PHYSICAL_PUSH,
            });
        }
        check_push_payload(payload)?;
        simctl_with_input(&["push", &self.udid, &app.0, "-"], Some(payload)).await?;
        Ok(())
    }

    async fn revoke(&self, app: &AppId, permissions: &[String]) -> Result<()> {
        self.simulator_only()?;
        for name in permissions {
            let service = privacy_service(name)?;
            simctl(&["privacy", &self.udid, "revoke", service, &app.0]).await?;
        }
        Ok(())
    }

    /// The lock state comes with every screenshot the poller takes (twice a second when idle), so this costs no
    /// request. A locked iPhone is the user's to open: past the lock screen is the passcode, which phonedriver never
    /// enters, so a locked screen is reported as secure whether or not a passcode is set.
    async fn screen_state(&self) -> Result<ScreenState> {
        let locked = self.activity.locked.load(Ordering::Relaxed);
        Ok(ScreenState {
            on: true,
            locked,
            secure: locked,
            focused: true,
        })
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

fn clear_folder(dir: &Path) -> Result<()> {
    let transport = |detail| Error::Transport { detail };
    let entries = std::fs::read_dir(dir).map_err(|e| {
        transport(format!(
            "cannot read the app's data folder {}: {e}",
            dir.display()
        ))
    })?;
    let mut failed = Vec::new();
    for entry in entries {
        let removed = entry
            .map_err(|e| format!("an entry ({e})"))
            .and_then(|entry| {
                let path = entry.path();
                let gone = if path.is_dir() {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                };
                gone.map_err(|e| format!("{} ({e})", entry.file_name().to_string_lossy()))
            });
        if let Err(what) = removed {
            failed.push(what);
        }
    }
    if failed.is_empty() {
        return Ok(());
    }
    Err(transport(format!(
        "the app's data was not fully cleared; could not remove {}",
        failed.join(", ")
    )))
}

/// A Simulator shares the Mac's file system, so a `file:` link shows a Mac file inside the device (D108).
fn names_a_mac_file(url: &str) -> bool {
    url.trim_start()
        .get(..5)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("file:"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_link_names_a_mac_file_in_any_case_and_a_web_link_does_not() {
        for url in [
            "file:///Users/me/.ssh/id_ed25519",
            "FILE:///etc/hosts",
            " file:/tmp/x",
        ] {
            assert!(names_a_mac_file(url), "{url}");
        }
        for url in ["https://example.com/file:", "myapp://file", "fil", ""] {
            assert!(!names_a_mac_file(url), "{url}");
        }
    }

    #[test]
    fn a_nudge_is_due_every_10_seconds_while_the_device_was_used_in_the_last_10_minutes_and_is_unlocked()
     {
        let s = Duration::from_secs;
        assert!(nudge_is_due(s(0), s(10), false));
        assert!(nudge_is_due(s(599), s(11), false));
        assert!(!nudge_is_due(s(5), s(9), false));
        assert!(!nudge_is_due(s(600), s(60), false));
        assert!(!nudge_is_due(s(5), s(10), true));
    }

    #[test]
    #[cfg(unix)]
    fn clearing_an_app_folder_empties_it_and_names_what_it_could_not_remove() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let data = home.path().join("data");
        std::fs::create_dir_all(data.join("Library/Caches")).unwrap();
        std::fs::write(data.join("Library/Caches/a"), "x").unwrap();
        std::fs::write(data.join("note"), "x").unwrap();
        clear_folder(&data).unwrap();
        assert_eq!(std::fs::read_dir(&data).unwrap().count(), 0);

        std::fs::create_dir_all(data.join("kept")).unwrap();
        std::fs::write(data.join("kept/a"), "x").unwrap();
        std::fs::write(data.join("note"), "x").unwrap();
        std::fs::set_permissions(data.join("kept"), std::fs::Permissions::from_mode(0o500))
            .unwrap();
        let error = clear_folder(&data).unwrap_err().to_string();
        std::fs::set_permissions(data.join("kept"), std::fs::Permissions::from_mode(0o700))
            .unwrap();
        assert!(error.contains("not fully cleared"), "{error}");
        assert!(error.contains("kept"), "{error}");
        assert!(!data.join("note").exists());

        let error = clear_folder(&home.path().join("gone"))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("cannot read the app's data folder"),
            "{error}"
        );
    }
}
