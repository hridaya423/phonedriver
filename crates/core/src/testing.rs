//! A scripted `Device` for daemon tests.

use std::sync::Mutex;

use async_trait::async_trait;
use tokio::sync::{broadcast, watch};

use crate::device::{
    App, AppId, AppOp, Button, Capabilities, ClipboardOp, Device, DeviceEvent, DeviceInfo,
    FrameRef, Gesture, Image, Key, Quality, Result, ScreenState, ShellReply, ShellRequest,
    TextInput, Url,
};
use crate::error::{Error, Op};
use crate::geometry::Size;
use crate::raw::RawTree;

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
    Install(std::path::PathBuf),
    AppOp(AppOp, AppId),
    Wake,
    Shell(String),
}

pub struct FakeDevice {
    info: DeviceInfo,
    trees: Mutex<Vec<RawTree>>,
    calls: Mutex<Vec<Call>>,
    clipboard: Mutex<Option<String>>,
    foreground: Mutex<Option<AppId>>,
    screen: Mutex<ScreenState>,
    screen_reads: std::sync::atomic::AtomicUsize,
    apps: Mutex<Vec<App>>,
    display: Mutex<Option<Size>>,
    events: broadcast::Sender<DeviceEvent>,
    frames: watch::Sender<FrameRef>,
    shell: Mutex<ShellReply>,
    refuse_set_text: std::sync::atomic::AtomicBool,
}

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
            screen_reads: Default::default(),
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
            refuse_set_text: Default::default(),
        }
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

    /// What `shell()` answers from now on.
    pub fn answer_shell(&self, reply: ShellReply) {
        *lock(&self.shell) = reply;
    }

    /// From now on `text(SetValue)` fails the way a field that does not accept `ACTION_SET_TEXT` does.
    pub fn refuse_set_text(&self) {
        self.refuse_set_text
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn set_screen(&self, state: ScreenState) {
        *lock(&self.screen) = state;
    }

    pub fn screen_reads(&self) -> usize {
        self.screen_reads.load(std::sync::atomic::Ordering::SeqCst)
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

    fn current_screen(&self) -> Option<Size> {
        *lock(&self.display)
    }

    async fn tree(&self) -> Result<RawTree> {
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
        Err(Error::Unsupported {
            op: Op::Screenshot,
            reason: "FakeDevice has no pixels",
        })
    }

    fn events(&self) -> broadcast::Receiver<DeviceEvent> {
        self.events.subscribe()
    }

    async fn gesture(&self, g: Gesture) -> Result<()> {
        {
            let mut screen = lock(&self.screen);
            if screen.locked && !screen.secure {
                screen.locked = false;
            }
        }
        self.record(Call::Gesture(g))
    }

    async fn key(&self, k: Key) -> Result<()> {
        self.record(Call::Key(k))
    }

    async fn button(&self, b: Button) -> Result<()> {
        self.record(Call::Button(b))
    }

    async fn open_url(&self, url: &Url, app: Option<&AppId>) -> Result<()> {
        self.record(Call::OpenUrl(url.clone(), app.cloned()))
    }

    async fn text(&self, t: TextInput) -> Result<()> {
        let refused = matches!(t, TextInput::SetValue(_))
            && self
                .refuse_set_text
                .load(std::sync::atomic::Ordering::SeqCst);
        self.record(Call::Text(t))?;
        if refused {
            return Err(crate::error::Error::Unsupported {
                op: crate::error::Op::Text,
                reason: crate::error::REFUSED_SET_TEXT,
            });
        }
        Ok(())
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
        *lock(&self.foreground) = Some(app.clone());
        self.record(Call::Launch(app.clone()))
    }

    async fn grant(&self, app: &AppId, permissions: &[String]) -> Result<()> {
        self.record(Call::Grant(app.clone(), permissions.to_vec()))
    }

    async fn foreground(&self) -> Result<Option<AppId>> {
        Ok(lock(&self.foreground).clone())
    }

    async fn install(&self, path: &std::path::Path) -> Result<Option<AppId>> {
        self.record(Call::Install(path.to_path_buf()))?;
        Ok(Some(AppId("com.example.installed".into())))
    }

    async fn app_op(&self, op: AppOp, app: &AppId) -> Result<()> {
        self.record(Call::AppOp(op, app.clone()))
    }

    async fn shell(&self, req: &ShellRequest) -> Result<ShellReply> {
        self.record(Call::Shell(req.command.clone()))?;
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
