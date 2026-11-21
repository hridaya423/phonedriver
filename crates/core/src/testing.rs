//! A scripted `Device` for daemon tests.

use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{broadcast, watch};

use crate::device::{
    App, AppId, AppOp, Button, Capabilities, ClipboardOp, Device, DeviceEvent, DeviceInfo,
    FrameRef, Gesture, Image, Key, LogReply, LogRequest, Quality, Recorded, Result, ScreenState,
    ShellReply, ShellRequest, TextInput, Url, WebPage,
};
use crate::error::{Error, Op};
use crate::geometry::{Rect, Size};
use crate::network::{LogWait, LogWaited, NetworkView};
use crate::raw::RawTree;
use crate::settings::{SettingOp, SettingReply, SettingsIndex};

/// One input the fake received, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    Gesture(Gesture),
    Key(Key),
    Button(Button),
    OpenUrl(Url, Option<AppId>),
    Text(TextInput),
    Launch(AppId),
    Grant(AppId, Vec<String>),
    Revoke(AppId, Vec<String>),
    Push(AppId, String),
    Setting(SettingOp),
    Logs(AppId, bool),
    LogWait(Option<AppId>, Vec<String>),
    RecordStart(std::path::PathBuf),
    RecordStop,
    Install(std::path::PathBuf),
    AppOp(AppOp, AppId),
    Wake,
    Shell(ShellRequest),
}

pub struct FakeDevice {
    info: DeviceInfo,
    trees: Mutex<Vec<RawTree>>,
    calls: Mutex<Vec<Call>>,
    clipboard: Mutex<Option<String>>,
    foreground: Mutex<Option<AppId>>,
    url_handler: Mutex<Option<AppId>>,
    quits_at_launch: std::sync::atomic::AtomicBool,
    slow_launch: Mutex<Option<(usize, AppId)>>,
    launch_reads: std::sync::atomic::AtomicUsize,
    screen: Mutex<ScreenState>,
    screen_reads: std::sync::atomic::AtomicUsize,
    tree_reads: std::sync::atomic::AtomicUsize,
    apps: Mutex<Vec<App>>,
    display: Mutex<Option<Size>>,
    events: broadcast::Sender<DeviceEvent>,
    frames: watch::Sender<FrameRef>,
    shell: Mutex<ShellReply>,
    page: Mutex<Option<WebPage>>,
    on_text: Mutex<Option<TextHook>>,
    text_failures: Mutex<Vec<Error>>,
    on_gesture: Mutex<Option<GestureHook>>,
    hidden: Option<u32>,
    shot: Mutex<Option<Image>>,
    shot_failures: Mutex<Vec<Error>>,
    after_shot: Mutex<Option<Vec<RawTree>>>,
    order: Mutex<Vec<&'static str>>,
    settings: Mutex<Option<FakeSettings>>,
    logs: Mutex<Option<LogReply>>,
    log_lines: Mutex<Option<Vec<String>>>,
    log_wait_takes: Mutex<Duration>,
    networks: Mutex<Vec<NetworkView>>,
    recorder: Mutex<Option<Option<std::path::PathBuf>>>,
    text_shows: Mutex<Option<(Duration, Duration)>>,
    follows: bool,
    installs: Mutex<Vec<AppId>>,
    installed_at: Mutex<Option<String>>,
    hardware_serial: Mutex<Option<String>>,
}

type FakeSettings = (std::sync::Arc<SettingsIndex>, Vec<(String, String)>);
type TextHook = Box<dyn Fn(&TextInput) -> Option<Vec<RawTree>> + Send + Sync>;
type GestureHook = Box<dyn Fn(&Gesture) -> Option<Vec<RawTree>> + Send + Sync>;

impl FakeDevice {
    /// `tree()` returns these in order, then keeps returning the last one.
    pub fn new(trees: Vec<RawTree>) -> FakeDevice {
        FakeDevice {
            info: DeviceInfo {
                id: "fake:1".into(),
                model: "Fake".into(),
                os: "FakeOS 1".into(),
                screen: Size { w: 360.0, h: 800.0 },
                density: 3.0,
                capabilities: Capabilities::all() - Capabilities::SCREENSHOT,
            },
            trees: Mutex::new(trees.into_iter().rev().collect()),
            calls: Mutex::new(Vec::new()),
            clipboard: Mutex::new(None),
            foreground: Mutex::new(None),
            url_handler: Mutex::new(None),
            quits_at_launch: Default::default(),
            slow_launch: Mutex::new(None),
            launch_reads: Default::default(),
            screen_reads: Default::default(),
            tree_reads: Default::default(),
            screen: Mutex::new(ScreenState {
                on: true,
                locked: false,
                secure: false,
                focused: true,
            }),
            apps: Mutex::new(Vec::new()),
            display: Mutex::new(None),
            events: broadcast::channel(64).0,
            frames: watch::channel(FrameRef::default()).0,
            shell: Mutex::new(ShellReply::default()),
            page: Mutex::new(None),
            on_text: Mutex::new(None),
            text_failures: Mutex::new(Vec::new()),
            on_gesture: Mutex::new(None),
            hidden: None,
            shot: Mutex::new(None),
            shot_failures: Mutex::new(Vec::new()),
            after_shot: Mutex::new(None),
            order: Mutex::new(Vec::new()),
            settings: Mutex::new(None),
            logs: Mutex::new(None),
            log_lines: Mutex::new(None),
            log_wait_takes: Mutex::new(Duration::ZERO),
            networks: Mutex::new(Vec::new()),
            recorder: Mutex::new(None),
            text_shows: Mutex::new(None),
            follows: false,
            installs: Mutex::new(vec![AppId("com.example.installed".into())]),
            installed_at: Mutex::new(None),
            hardware_serial: Mutex::new(None),
        }
    }

    /// The same device with a screen it can capture: `screenshot()` returns `image`.
    pub fn with_screenshot(mut self, image: Image) -> FakeDevice {
        self.info.capabilities |= Capabilities::SCREENSHOT;
        *lock(&self.shot) = Some(image);
        self
    }

    /// The next `screenshot()` calls fail with these errors, in order, before one succeeds.
    pub fn fail_screenshots(&self, errors: Vec<Error>) {
        *lock(&self.shot_failures) = errors.into_iter().rev().collect();
    }

    /// After the next `screenshot()`, `tree()` returns these in order, then keeps the last: a window that appears
    /// while the picture is taken.
    pub fn after_screenshot(&self, trees: Vec<RawTree>) {
        *lock(&self.after_shot) = Some(trees);
    }

    /// The next `text()` calls fail with these errors, in order, before one succeeds.
    pub fn fail_text(&self, errors: Vec<Error>) {
        *lock(&self.text_failures) = errors.into_iter().rev().collect();
    }

    /// `"tree"` and `"screenshot"` in the order the device was asked, to tell what a screenshot was taken after.
    pub fn order(&self) -> Vec<&'static str> {
        lock(&self.order).clone()
    }

    /// After a `text` call for which `f` returns trees, `tree()` returns those in order, then keeps the last: a field
    /// that ignores one kind of input, or shows the text a few reads late.
    pub fn on_text(&self, f: impl Fn(&TextInput) -> Option<Vec<RawTree>> + Send + Sync + 'static) {
        *lock(&self.on_text) = Some(Box::new(f));
    }

    /// After a `gesture` call for which `f` returns trees, `tree()` returns those in order, then keeps the last.
    /// Frames that follow the screen without the `FRAMES` capability, as simio's on the iOS Simulator.
    pub fn following_frames(mut self) -> FakeDevice {
        self.info.capabilities -= Capabilities::FRAMES;
        self.follows = true;
        self
    }

    /// `text()` draws a frame `shown` after it starts and returns at `confirmed`, as the iOS runner confirms text
    /// after the app has drawn it.
    pub fn text_shows_before_it_returns(&self, shown: Duration, confirmed: Duration) {
        *lock(&self.text_shows) = Some((shown, confirmed));
    }

    pub fn on_gesture(&self, f: impl Fn(&Gesture) -> Option<Vec<RawTree>> + Send + Sync + 'static) {
        *lock(&self.on_gesture) = Some(Box::new(f));
    }

    /// `tree()` returns this from now on, as a screen that changes while the daemon waits.
    pub fn set_tree(&self, tree: RawTree) {
        *lock(&self.trees) = vec![tree];
    }

    /// The same device working on virtual display `id`.
    pub fn on_hidden_display(mut self, id: u32) -> FakeDevice {
        self.hidden = Some(id);
        self
    }

    /// The same device under another id, as an Android device is `android:<serial>`.
    /// Launched apps quit at once, as Photos does on a broken Simulator: the app in front stays in front.
    pub fn set_quits_at_launch(&self, quits: bool) {
        self.quits_at_launch
            .store(quits, std::sync::atomic::Ordering::Relaxed);
    }

    /// A launched app comes to the front only at the `reads`th `foreground()` call after its launch, as a slow cold
    /// start does.
    pub fn set_launch_takes(&self, reads: usize) {
        self.launch_reads
            .store(reads, std::sync::atomic::Ordering::Relaxed);
    }

    /// The app `open_url` reports as handling a link that names no app.
    pub fn set_url_handler(&self, app: Option<AppId>) {
        *lock(&self.url_handler) = app;
    }

    pub fn with_id(mut self, id: &str) -> FakeDevice {
        self.info.id = id.into();
        self
    }

    /// The same device without some capabilities, as the polled iOS tier lacks `FRAMES`.
    pub fn lacking(mut self, caps: Capabilities) -> FakeDevice {
        self.info.capabilities -= caps;
        self
    }

    /// The screen size `current_screen()` reports from now on, as a device that follows its own rotation does.
    pub fn rotate_to(&self, screen: Size) {
        *lock(&self.display) = Some(screen);
    }

    /// The apps an `install` adds to the device, none for a file that replaces an app.
    pub fn set_installs(&self, ids: &[&str]) {
        *lock(&self.installs) = ids.iter().map(|id| AppId((*id).into())).collect();
    }

    /// What `installed_at` answers for every app from now on.
    pub fn set_installed_at(&self, stamp: &str) {
        *lock(&self.installed_at) = Some(stamp.into());
    }

    /// What `hardware_serial` answers from now on.
    pub fn set_hardware_serial(&self, serial: &str) {
        *lock(&self.hardware_serial) = Some(serial.into());
    }

    /// The launchable apps `apps()` reports, as `(id, name)`.
    pub fn install(&self, apps: &[(&str, &str)]) {
        *lock(&self.apps) = apps
            .iter()
            .map(|(id, name)| App {
                id: AppId((*id).into()),
                name: (*name).into(),
                system: false,
            })
            .collect();
    }

    /// The device has these settings, with these values by key. A write is kept; a write of `"refuse"` fails.
    pub fn with_settings(&self, index: SettingsIndex, values: &[(&str, &str)]) {
        let values = values
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        *lock(&self.settings) = Some((std::sync::Arc::new(index), values));
    }

    /// The device can record its screen: a start writes a small file, a stop answers 2 frames over 1500 ms.
    pub fn with_recorder(&self) {
        *lock(&self.recorder) = Some(None);
    }

    /// Ends a running recording the way a lost connection does: the next stop finds none.
    pub fn lose_recording(&self) {
        if let Some(running) = lock(&self.recorder).as_mut() {
            *running = None;
        }
    }

    /// What `logs()` answers from now on; without it the device has no logs.
    pub fn answer_logs(&self, reply: LogReply) {
        *lock(&self.logs) = Some(reply);
    }

    /// The lines a `log_wait` finds arriving; without them the device cannot follow its log.
    pub fn answer_log_lines(&self, lines: &[&str]) {
        *lock(&self.log_lines) = Some(lines.iter().map(|l| l.to_string()).collect());
    }

    /// How long a `log_wait` takes before it answers.
    pub fn log_wait_takes(&self, took: Duration) {
        *lock(&self.log_wait_takes) = took;
    }

    /// What `network()` answers: each call takes the next view, and the last one repeats. Without any the device
    /// cannot read its networks.
    pub fn answer_networks(&self, views: Vec<NetworkView>) {
        *lock(&self.networks) = views;
    }

    /// What `shell()` answers from now on.
    pub fn answer_shell(&self, reply: ShellReply) {
        *lock(&self.shell) = reply;
    }

    /// The web page `page()` reads from now on; without one it refuses, as an app with no web page does.
    pub fn show_page(&self, page: WebPage) {
        *lock(&self.page) = Some(page);
    }

    pub fn set_screen(&self, state: ScreenState) {
        *lock(&self.screen) = state;
    }

    pub fn screen_reads(&self) -> usize {
        self.screen_reads.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn tree_reads(&self) -> usize {
        self.tree_reads.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn calls(&self) -> Vec<Call> {
        lock(&self.calls).clone()
    }

    pub fn emit(&self, event: DeviceEvent) {
        let _ = self.events.send(event);
    }

    pub fn push_frame(&self, frame: FrameRef) {
        self.frames.send_replace(frame);
    }

    fn record(&self, call: Call) -> Result<()> {
        lock(&self.calls).push(call);
        Ok(())
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[async_trait]
impl Device for FakeDevice {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn hidden_display(&self) -> Option<u32> {
        self.hidden
    }

    fn current_screen(&self) -> Option<Size> {
        *lock(&self.display)
    }

    async fn tree(&self) -> Result<RawTree> {
        lock(&self.order).push("tree");
        self.tree_reads
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let mut trees = lock(&self.trees);
        match trees.len() {
            0 => Err(Error::Transport {
                detail: "FakeDevice has no tree scripted".into(),
            }),
            1 => Ok(trees[0].clone()),
            _ => Ok(trees.pop().unwrap_or_default()),
        }
    }

    fn frames(&self) -> Option<watch::Receiver<FrameRef>> {
        Some(self.frames.subscribe())
    }

    async fn screenshot(&self, _q: Quality) -> Result<Image> {
        lock(&self.order).push("screenshot");
        if let Some(e) = lock(&self.shot_failures).pop() {
            return Err(e);
        }
        let image = lock(&self.shot).clone().ok_or(Error::Unsupported {
            op: Op::Screenshot,
            reason: "FakeDevice has no pixels",
        })?;
        if let Some(trees) = lock(&self.after_shot).take() {
            *lock(&self.trees) = trees.into_iter().rev().collect();
        }
        Ok(image)
    }

    fn events(&self) -> broadcast::Receiver<DeviceEvent> {
        self.events.subscribe()
    }

    fn frames_follow(&self) -> bool {
        self.follows || self.info.capabilities.contains(Capabilities::FRAMES)
    }

    async fn gesture(&self, g: Gesture) -> Result<()> {
        {
            let mut screen = lock(&self.screen);
            if screen.locked && !screen.secure {
                screen.locked = false;
            }
        }
        if let Some(trees) = lock(&self.on_gesture).as_ref().and_then(|f| f(&g)) {
            *lock(&self.trees) = trees.into_iter().rev().collect();
        }
        self.record(Call::Gesture(g))
    }

    async fn key(&self, k: Key) -> Result<()> {
        self.record(Call::Key(k))
    }

    async fn button(&self, b: Button) -> Result<()> {
        self.record(Call::Button(b))
    }

    async fn open_url(&self, url: &Url, app: Option<&AppId>) -> Result<Option<AppId>> {
        self.record(Call::OpenUrl(url.clone(), app.cloned()))?;
        Ok(app.cloned().or_else(|| lock(&self.url_handler).clone()))
    }

    async fn text(&self, t: TextInput) -> Result<()> {
        if let Some(e) = lock(&self.text_failures).pop() {
            return Err(e);
        }
        if let Some(trees) = lock(&self.on_text).as_ref().and_then(|f| f(&t)) {
            *lock(&self.trees) = trees.into_iter().rev().collect();
        }
        let shows = *lock(&self.text_shows);
        if let Some((shown, confirmed)) = shows {
            tokio::time::sleep(shown).await;
            self.push_frame(FrameRef {
                seq: u64::MAX,
                changed_area_pt2: 10_000.0,
                changed_box: Some(Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 100.0,
                    h: 100.0,
                }),
            });
            tokio::time::sleep(confirmed.saturating_sub(shown)).await;
        }
        self.record(Call::Text(t))
    }

    async fn clipboard(&self, op: ClipboardOp) -> Result<Option<String>> {
        let mut clip = lock(&self.clipboard);
        match op {
            ClipboardOp::Get => Ok(clip.clone()),
            ClipboardOp::Set(t) => {
                *clip = Some(t);
                Ok(None)
            }
        }
    }

    async fn apps(&self) -> Result<Vec<App>> {
        Ok(lock(&self.apps).clone())
    }

    async fn launch(&self, app: &AppId) -> Result<()> {
        let reads = self.launch_reads.load(std::sync::atomic::Ordering::Relaxed);
        if reads > 0 {
            *lock(&self.slow_launch) = Some((reads, app.clone()));
        } else if !self
            .quits_at_launch
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            *lock(&self.foreground) = Some(app.clone());
        }
        self.record(Call::Launch(app.clone()))
    }

    async fn grant(&self, app: &AppId, permissions: &[String]) -> Result<()> {
        self.record(Call::Grant(app.clone(), permissions.to_vec()))
    }

    async fn revoke(&self, app: &AppId, permissions: &[String]) -> Result<()> {
        self.record(Call::Revoke(app.clone(), permissions.to_vec()))
    }

    async fn push(&self, app: &AppId, payload: &str) -> Result<()> {
        self.record(Call::Push(app.clone(), payload.to_string()))
    }

    async fn record_start(&self, path: &std::path::Path) -> Result<()> {
        let mut recorder = lock(&self.recorder);
        let Some(running) = recorder.as_mut() else {
            return Err(Error::Unsupported {
                op: Op::Record,
                reason: "this fake has no video stream",
            });
        };
        if running.is_some() {
            return Err(Error::Refused {
                detail: "a recording is already running".into(),
            });
        }
        std::fs::write(path, b"video").map_err(|e| Error::Transport {
            detail: e.to_string(),
        })?;
        *running = Some(path.to_path_buf());
        self.record(Call::RecordStart(path.to_path_buf()))
    }

    async fn record_stop(&self) -> Result<Option<Recorded>> {
        let mut recorder = lock(&self.recorder);
        let Some(running) = recorder.as_mut() else {
            return Err(Error::Unsupported {
                op: Op::Record,
                reason: "this fake has no video stream",
            });
        };
        self.record(Call::RecordStop)?;
        Ok(running.take().map(|path| Recorded {
            path,
            width: 360,
            height: 800,
            frames: 2,
            ms: 1500,
            early: None,
        }))
    }

    async fn logs(&self, req: &LogRequest) -> Result<LogReply> {
        let Some(reply) = lock(&self.logs).clone() else {
            return Err(Error::Unsupported {
                op: Op::Logs,
                reason: "this fake has no logs",
            });
        };
        self.record(Call::Logs(req.app.clone(), req.crashes))?;
        Ok(reply)
    }

    async fn log_wait(&self, req: &LogWait) -> Result<LogWaited> {
        let Some(lines) = lock(&self.log_lines).clone() else {
            return Err(Error::Unsupported {
                op: Op::Logs,
                reason: "this fake has no log to follow",
            });
        };
        self.record(Call::LogWait(req.app.clone(), req.patterns.clone()))?;
        let took = *lock(&self.log_wait_takes);
        tokio::time::sleep(took).await;
        let at = lines.iter().position(|l| req.matches(l));
        Ok(LogWaited {
            line: at.map(|i| lines[i].clone()),
            seen: at.map_or(lines.len(), |i| i + 1),
        })
    }

    async fn network(&self) -> Result<NetworkView> {
        let mut views = lock(&self.networks);
        match views.len() {
            0 => Err(Error::Unsupported {
                op: Op::Network,
                reason: "this fake has no networks",
            }),
            1 => Ok(views[0].clone()),
            _ => Ok(views.remove(0)),
        }
    }

    async fn setting(&self, op: SettingOp) -> Result<SettingReply> {
        let mut settings = lock(&self.settings);
        let Some((index, values)) = settings.as_mut() else {
            return Err(Error::Unsupported {
                op: Op::Setting,
                reason: "this fake has no settings",
            });
        };
        lock(&self.calls).push(Call::Setting(op.clone()));
        let read = |values: &[(String, String)], key: &str| {
            values
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| SettingReply::Value {
                    value: v.clone(),
                    hint: None,
                })
                .ok_or(Error::Refused {
                    detail: format!("the device reported no value for {key}"),
                })
        };
        match op {
            SettingOp::Index => Ok(SettingReply::Index(index.clone())),
            SettingOp::Get(key) => read(values, &key),
            SettingOp::Set { key, value } if value == "refuse" => Err(Error::Refused {
                detail: format!("{key} was not set: Preference not enabled"),
            }),
            SettingOp::Set { key, value } => {
                values.retain(|(k, _)| *k != key);
                values.push((key.clone(), value));
                read(values, &key)
            }
        }
    }

    async fn foreground(&self) -> Result<Option<AppId>> {
        let mut slow = lock(&self.slow_launch);
        if let Some((left, app)) = slow.as_mut() {
            *left -= 1;
            if *left == 0 {
                *lock(&self.foreground) = Some(app.clone());
                *slow = None;
            }
        }
        Ok(lock(&self.foreground).clone())
    }

    async fn install(&self, path: &std::path::Path) -> Result<Vec<AppId>> {
        self.record(Call::Install(path.to_path_buf()))?;
        Ok(lock(&self.installs).clone())
    }

    async fn installed_at(&self, _app: &AppId) -> Result<Option<String>> {
        Ok(lock(&self.installed_at).clone())
    }

    async fn hardware_serial(&self) -> Result<Option<String>> {
        Ok(lock(&self.hardware_serial).clone())
    }

    async fn app_op(&self, op: AppOp, app: &AppId) -> Result<()> {
        self.record(Call::AppOp(op, app.clone()))
    }

    async fn page(&self) -> Result<WebPage> {
        lock(&self.page).clone().ok_or(Error::Refused {
            detail: "com.example has no web page that can be read".into(),
        })
    }

    async fn shell(&self, req: &ShellRequest) -> Result<ShellReply> {
        self.record(Call::Shell(req.clone()))?;
        Ok(lock(&self.shell).clone())
    }

    async fn screen_state(&self) -> Result<ScreenState> {
        self.screen_reads
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(*lock(&self.screen))
    }

    async fn wake(&self) -> Result<()> {
        lock(&self.screen).on = true;
        self.record(Call::Wake)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{
        Button, Capabilities, ClipboardOp, Device, DeviceEvent, FrameRef, Gesture, TextInput,
        TimedPoint,
    };
    use crate::geometry::{Point, Scale};
    use crate::raw::RawTree;

    fn tree(label: &str) -> RawTree {
        let json = format!(
            r#"{{"windows":[],"nodes":[{{"p":-1,"w":1,"cls":"android.widget.TextView","text":"{label}","b":[0,0,10,10],"f":"vVe"}}]}}"#
        );
        RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
    }

    #[tokio::test]
    async fn trees_come_back_in_order_then_the_last_repeats() {
        let fake = FakeDevice::new(vec![tree("one"), tree("two")]);
        assert_eq!(
            fake.tree().await.unwrap().nodes[0].label.as_deref(),
            Some("one")
        );
        assert_eq!(
            fake.tree().await.unwrap().nodes[0].label.as_deref(),
            Some("two")
        );
        assert_eq!(
            fake.tree().await.unwrap().nodes[0].label.as_deref(),
            Some("two")
        );
    }

    #[tokio::test]
    async fn inputs_are_recorded() {
        let fake = FakeDevice::new(vec![]);
        let tap = Gesture {
            fingers: vec![vec![
                TimedPoint {
                    at: Point { x: 5.0, y: 6.0 },
                    t_ms: 0,
                },
                TimedPoint {
                    at: Point { x: 5.0, y: 6.0 },
                    t_ms: 50,
                },
            ]],
        };
        fake.gesture(tap.clone()).await.unwrap();
        fake.button(Button::Back).await.unwrap();
        fake.text(TextInput::Paste("hello".into())).await.unwrap();
        assert_eq!(
            fake.calls(),
            [
                Call::Gesture(tap),
                Call::Button(Button::Back),
                Call::Text(TextInput::Paste("hello".into()))
            ]
        );
    }

    #[tokio::test]
    async fn scripted_events_and_frames_are_delivered() {
        let fake = FakeDevice::new(vec![]);
        let mut events = fake.events();
        fake.emit(DeviceEvent::WindowChanged {
            package: Some("com.android.settings".into()),
        });
        assert_eq!(
            events.recv().await.unwrap(),
            DeviceEvent::WindowChanged {
                package: Some("com.android.settings".into())
            }
        );

        let mut frames = fake.frames().unwrap();
        fake.push_frame(FrameRef {
            seq: 7,
            changed_area_pt2: 9000.0,
            changed_box: None,
        });
        frames.changed().await.unwrap();
        assert_eq!(frames.borrow().seq, 7);
    }

    #[tokio::test]
    async fn clipboard_round_trips_and_capabilities_are_declared() {
        let fake = FakeDevice::new(vec![]);
        assert!(
            fake.info()
                .capabilities
                .contains(Capabilities::TREE | Capabilities::GESTURE | Capabilities::CLIPBOARD)
        );
        assert!(!fake.info().capabilities.contains(Capabilities::SCREENSHOT));
        fake.clipboard(ClipboardOp::Set("x".into())).await.unwrap();
        assert_eq!(
            fake.clipboard(ClipboardOp::Get).await.unwrap().as_deref(),
            Some("x")
        );
    }

    #[test]
    fn text_debug_output_never_contains_the_text() {
        let shown = format!("{:?}", TextInput::Keystrokes("correct horse".into()));
        assert!(!shown.contains("correct horse"), "{shown}");
    }
}
