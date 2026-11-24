//! One task per device. It owns the device connection, the lease and the sessions, so no lock guards device state.

use std::collections::HashMap;
use std::future::pending;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use phonedriver_core::device::{
    App, AppId, AppOp, Button, Capabilities, ClipboardOp, Device, DeviceEvent, DisplayMode,
    FrameRef, Gesture, Key, LogRequest, Quality, ScreenState, ShellRequest, TextInput, TimedPoint,
    Url,
};
use phonedriver_core::diff::{Change, DiffOrFull, Field, diff, drop_echo};
use phonedriver_core::element::{Actions, Element, ElementState, Ref, Role, Source};
use phonedriver_core::error::{Category, Direction, Error, Op, ScrollEnd, UserAction};
use phonedriver_core::geometry::{Point, Rect, Size};
use phonedriver_core::gesture;
use phonedriver_core::network::LogWait;
use phonedriver_core::ocr::{merge, tree_is_hollow};
use phonedriver_core::prune::prune;
use phonedriver_core::raw::{NodeFlags, RawTree};
use phonedriver_core::render::{
    AppLine, DeviceLine, Filters, Header, Orientation, Rendered, SettleLine, plural, quote,
};
use phonedriver_core::scope::AppScope;
use phonedriver_core::settings::{SettingOp, SettingReply};
use phonedriver_core::settle::{Config, Detector, Input, State};
use phonedriver_core::similar::{SUGGEST_WITHIN, normalized_distance};
use serde_json::{Value, json};
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time::Instant;

use super::DeviceSource;
use super::checkbox;
use super::journal::{self, Kept};
use super::ledger::{Entry, Ledger};
use super::scope::ScopedDevice;
use super::secrets::Secrets;
use super::session::{Resolved, Session};
use super::shell;
use super::verify::{Expect, Verdict, Verify};
use super::{guard, secret_field};
use crate::wire::{
    ActParams, Action, ActionResult, AppParams, Approval, Approved, BatchParams, BatchResult,
    BuildParams, CheckParams, ClipboardParams, Crashed, FindParams, JournalParams, Kind,
    LogsParams, Observation, OnScreen, PageParams, PinchDirection, RecordAction, RecordParams,
    ScreenshotParams, ScreenshotResult, SettingParams, ShellParams, ShellResult, SnapshotParams,
    Step, StepFailure, StepResult, Target, TypeMode, VpnState, WaitCondition, WaitParams,
};

pub const LEASE_TTL: Duration = Duration::from_secs(600);
/// A session of command lines gives the device to another session once it has been idle this long. A person tries
/// `phonedriver snapshot` in a terminal and then starts an agent: with the 10 minute lease the agent was refused
/// ("leased by session default") until the person ran `phonedriver release` (seen in an agent run, 2026-11-12).
/// The device stays connected, so the terminal's next command is not a cold start.
pub const TERMINAL_HOLD: Duration = Duration::from_secs(30);
const TICK: Duration = Duration::from_millis(5);
const SETTING_LINES: usize = 12;
const AUDIT_LINES: usize = 20;
const LOGS_SINCE_S: u64 = 60;
const VPN_POLL: Duration = Duration::from_secs(1);
const LOG_LINE_CHARS: usize = 400;
/// Added to an action's own time when its crash log is read, for the action's preparation before the clock started.
const CRASH_MARGIN: Duration = Duration::from_secs(2);
const SETTING_LINES_OF_MANY: usize = 4;
const SCREEN_WAIT: Duration = Duration::from_secs(3);
/// About 2,000 tokens: a mobile Wikipedia article with its sections closed is 6,086 characters.
const PAGE_CHARS: usize = 8000;
/// A page puts a label and its value on separate lines: Wikipedia's "Born" is followed by the birth name, then the date.
const GREP_LINES_AFTER: usize = 2;
const KEYGUARD_WAIT: Duration = Duration::from_millis(1000);
const WAIT_DEBOUNCE: Duration = Duration::from_millis(100);
/// Typed text is already in the field when the call returns (the paste waits for it), so unlike after a tap there is
/// no effect still to come: the first-change wait and the quiet period after it are short.
const TYPED_QUIET: Duration = Duration::from_millis(150);
/// A tree read that starts this long after the last significant frame can already show where the screen ended up.
const READ_LAG: Duration = Duration::from_millis(120);
/// With no frame yet, the tree is read this long after the action to see whether the screen changed without one.
/// A shorter first-change window (typed text) is already over before a read says more than the text just typed.
const FIRST_READ_MIN_WINDOW: Duration = Duration::from_millis(400);
const FIRST_READ: Duration = Duration::from_millis(100);
const TOGGLE_WAIT: Duration = Duration::from_secs(3);
const MAX_CHECKBOXES: usize = 16;
const TREE_STEP: Duration = Duration::from_millis(100);
const FOCUS_FOLLOW_WAIT: Duration = Duration::from_secs(5);
const FOCUS_KEYBOARD_WAIT: Duration = Duration::from_millis(1200);

/// A web field this many times wider after its focus tap means the page zoomed in on it. Safari on iOS does that for
/// an input whose font is under 16 px, and WebKit then reports frames that taps miss.
const PAGE_ZOOMED: f32 = 1.3;

/// How many text fields a `not_focused` error names.
const NOT_FOCUSED_FIELDS: usize = 5;

/// A `no_app` error lists every installed app when there are at most this many (a Simulator has about 25), so the
/// agent need not call `apps` next.
const NO_APP_LIST: usize = 40;

/// How many drags `type` makes to bring back a field the keyboard pushed out of its list.
const BRING_BACK_DRAGS: u32 = 4;

/// How long a replayed step waits for the kind of screen it was saved on before it is refused. Android goes on
/// reporting a page that slides in as a window short of one edge after the settle is over, and that reads as a
/// dialog: 16 of 40 replays of plain Settings steps stopped on it, and none of 64 that waited (issue #53).
const SCREEN_KIND_WAIT: Duration = Duration::from_secs(1);
const TREE_STABLE_WAIT: Duration = Duration::from_secs(1);
const LOADING_WAIT: Duration = Duration::from_secs(4);
/// How long a reply waits for a tree that lists elements. A read right after an app starts can list none (observed
/// on the first launch after a daemon restart: a header and no lines), and the reply must carry the screen.
const CONTENT_WAIT: Duration = Duration::from_secs(2);
/// Chrome exposes a web field's new text to accessibility up to a second after the page shows it (Domino's checkout,
/// keys typed in a batch, API 37 emulator).
/// How long an opened link may take to put its page in the tree before the reply goes out with what is there.
const PAGE_WAIT: Duration = Duration::from_secs(5);
const WEB_TEXT_WAIT: Duration = Duration::from_millis(1500);
/// Observed in Photos on the API 37 tablet emulator, 3 of 3 opens of the editor: the reply listed "Cancel edits" and
/// "Save copy" only, and the tabs reached the tree 0.2 to 0.5 s later. On the first open after a boot the screen
/// was one progress indicator for longer than `LOADING_WAIT`, so such a screen gets `SPINNER_WAIT` more. In three benchmark runs the agent then spent two to
/// four calls (screenshot, snapshot, wait) to see the editor.
const BARE_WAIT: Duration = Duration::from_millis(1200);
const SPINNER_WAIT: Duration = Duration::from_secs(4);
const FOCUS_WAIT: Duration = Duration::from_secs(1);
const FOCUS_STEP: Duration = Duration::from_millis(30);
/// Observed in Clock's city search on the API 37 emulators: the suggestions reached the tree 300 to 600 ms after the
/// text did, the reply showed the field and "Clear text" only, and the agent took a snapshot after every city.
const SEARCH_WAIT: Duration = Duration::from_millis(1500);
const WEB_VIEW_WAIT: Duration = Duration::from_secs(10);
const LEAVE_WAIT: Duration = Duration::from_millis(2500);
/// Observed on the API 37 emulator, bench task files-new-folder: Files was launched twice, the launcher was still in
/// front 2.8 s and 3.0 s later, and Files came to the front about 7 s after the second launch. Each reply said "it may
/// have quit at start", and the agent spent four more calls on `am start` and `dumpsys` to find it.
const LATE_START_WAIT: Duration = Duration::from_secs(6);
const LATE_START_STEP: Duration = Duration::from_millis(250);
const KEYBOARD_STAYED: &str = ", and enter left the keyboard up (`press hide-keyboard` closes it)";
/// Observed on the API 37 tablet emulator: the first "Add alarm" tap after a cold Clock launch opened its time picker
/// after more than 500 ms, so the reply said "no change" while the dialog was on its way, and the agent tapped again.
const COLD_APP_WINDOW: Duration = Duration::from_secs(30);
const COLD_CHANGE_WAIT: Duration = Duration::from_millis(2000);
/// How long a tap whose frames changed but whose elements did not waits for a page that starts late.
const LATE_PAGE_WAIT: Duration = Duration::from_millis(600);
/// All the reads a reply makes after its settle, together: each wait below has its own limit, and a spinner that
/// became a web view that never filled ran three of them end to end for 16 s.
const REPORT_WAIT: Duration = WEB_VIEW_WAIT;

pub enum Call {
    Snapshot(SnapshotParams),
    Screenshot(ScreenshotParams),
    Act(ActParams),
    Wait(WaitParams),
    Check(CheckParams),
    Batch(BatchParams),
    Find(FindParams),
    Apps,
    App(AppParams),
    Clipboard(ClipboardParams),
    Shell(ShellParams),
    Setting(SettingParams),
    Logs(LogsParams),
    Network,
    Page(PageParams),
    Record(RecordParams),
    Journal(JournalParams),
    Build(BuildParams),
    /// What `doctor` needs: the device answers a tree read, and a frame arrives.
    Health,
    Release,
    /// `Release`, and the session's state on this device is dropped: its last screen, journal and approvals.
    Forget,
}

/// Where an action's time went, in call order: `(phase, ms)`. In microseconds; written into the trace line as milliseconds.
pub type Phases = Vec<(&'static str, u32)>;

pub struct Job {
    pub session: String,
    /// The session is command lines, not an MCP server: its hold on the device yields after `TERMINAL_HOLD`.
    pub terminal: bool,
    pub share: bool,
    pub display: DisplayMode,
    pub call: Call,
    pub reply: oneshot::Sender<(Result<Value, Error>, Phases)>,
}

/// Who holds the device, read without waiting for the call the actor is in. Seen 2026-11-20 on an iPhone on
/// Wi-Fi: a reconnect never returned, and `devices`, which asked the actor itself, waited behind it.
#[derive(Clone, Default)]
pub struct Held {
    pub session: Option<String>,
    pub last_call: Option<Instant>,
    pub terminal: bool,
    pub hidden_display: Option<u32>,
}

#[derive(Clone)]
pub struct Handle {
    pub jobs: mpsc::Sender<Job>,
    pub held: watch::Receiver<Held>,
}

pub fn spawn(
    id: String,
    source: Arc<dyn DeviceSource>,
    home: PathBuf,
    secrets: Arc<Secrets>,
    ledger: Arc<Ledger>,
    scope: Option<Arc<AppScope>>,
) -> Handle {
    let (tx, rx) = mpsc::channel(32);
    let (held, seen) = watch::channel(Held::default());
    let actor = Actor {
        id,
        held,
        source,
        home,
        lease: None,
        live: None,
        sessions: HashMap::new(),
        base: Instant::now(),
        recent: None,
        during: Vec::new(),
        change_seen: false,
        unobserved: None,
        cold_until: None,
        awake: None,
        phases: Vec::new(),
        shown: Mutex::new(None),
        guard_view: None,
        apps: Arc::default(),
        reshaped: None,
        names: Arc::default(),
        recording: None,
        secrets,
        ledger,
        scope,
    };
    tokio::spawn(actor.run(rx));
    Handle {
        jobs: tx,
        held: seen,
    }
}

/// A recording in progress: its file, when it began, and how many actions are in its steps file.
struct Taping {
    path: PathBuf,
    began: Instant,
    steps: u32,
}

impl Taping {
    fn steps_path(&self) -> PathBuf {
        let mut name = self.path.clone().into_os_string();
        name.push(".steps.jsonl");
        PathBuf::from(name)
    }

    /// Adds an action that began at `at` to the steps file.
    fn step(&mut self, at: Instant, summary: &str, landed: Option<Point>) {
        use std::io::Write;
        let mut line =
            json!({"t_ms": at.duration_since(self.began).as_millis() as u64, "summary": summary});
        if let Some(p) = landed {
            line["at"] = json!(p);
        }
        let written = crate::private::options()
            .create(true)
            .append(true)
            .open(self.steps_path())
            .and_then(|mut file| writeln!(file, "{line}"));
        if written.is_ok() {
            self.steps += 1;
        }
    }
}

struct Lease {
    session: String,
    last_call: Instant,
    terminal: bool,
}

struct Live {
    device: Arc<dyn Device>,
    display: DisplayMode,
    frames: watch::Receiver<FrameRef>,
    events: broadcast::Receiver<DeviceEvent>,
    screen: Size,
    ledger_key: tokio::sync::OnceCell<String>,
}

impl Live {
    fn screen(&self) -> Size {
        self.device.current_screen().unwrap_or(self.screen)
    }
}

struct Read {
    at: Duration,
    tree: RawTree,
}

type Inflight = (
    Duration,
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<RawTree, Error>> + Send>>,
);

/// A tree read during the settle: the newest one that started after the last significant frame, and whether the one
/// before it (also after that frame) agrees with it.
struct Probe {
    tree: RawTree,
    at: Duration,
    stable: bool,
}

fn probe_of(mut reads: Vec<Read>, last: Duration) -> Option<Probe> {
    reads.retain(|r| r.at >= last + READ_LAG);
    let newest = reads.pop()?;
    let stable = reads.last().is_some_and(|r| r.tree == newest.tree);
    Some(Probe {
        tree: newest.tree,
        at: newest.at,
        stable,
    })
}

async fn finished(inflight: &mut Option<Inflight>) -> (Duration, Result<RawTree, Error>) {
    let Some((at, read)) = inflight.as_mut() else {
        return pending().await;
    };
    let at = *at;
    let result = read.as_mut().await;
    *inflight = None;
    (at, result)
}

enum Signal {
    Frame(FrameRef),
    Window,
    Display(Size),
    Power,
    Gone,
}

/// On a device that does not push its power state, a screen found on and unlocked stays that way for this long
/// unless a window or display event says otherwise: reading the power state costs a round trip on every action.
const AWAKE_REUSE: Duration = Duration::from_secs(1);
/// How long the screen read for a point tap's guard check may serve the next point tap, while no window change or
/// repaint of a quarter of the screen has arrived since.
const GUARD_VIEW_REUSE: Duration = Duration::from_secs(1);

struct Actor {
    id: String,
    held: watch::Sender<Held>,
    source: Arc<dyn DeviceSource>,
    home: PathBuf,
    lease: Option<Lease>,
    live: Option<Live>,
    sessions: HashMap<String, Session>,
    base: Instant,
    /// The latest input that would have delayed a settle, or the moment the device connected: quiet counts from there, so a snapshot of an already quiet screen does not wait.
    recent: Option<(Duration, Input)>,
    /// What the screen did while the last action's input was being delivered (see `watching`), for the settle that
    /// observes that action.
    during: Vec<(Duration, Input)>,
    change_seen: bool,
    /// When an action was last delivered without waiting for its effects.
    unobserved: Option<Duration>,
    /// Until when the app a launch started may still be too busy to react within the usual first-change window.
    cold_until: Option<Duration>,
    /// When `prepare` last found the screen on and unlocked; cleared by any window or display event.
    awake: Option<Duration>,
    /// The screen the guard judges a touch on: the newest fresh read, and when it was made.
    guard_view: Option<(Duration, Rendered)>,
    /// The apps this connection's device listed, kept for names it holds exactly (`app_named`). The list read at
    /// connect for the header's app names fills it.
    apps: Arc<std::sync::Mutex<Option<Vec<App>>>>,
    /// When a window changed or a quarter of the screen repainted last: a button's ripple (about 100 by 100 pt) is a
    /// repaint that leaves the tree as it was.
    reshaped: Option<Duration>,
    phases: Phases,
    /// The tree the agent was last shown, to tell whether an action changed anything before a frame says so.
    shown: Mutex<Option<RawTree>>,
    names: Arc<Mutex<HashMap<String, String>>>,
    recording: Option<Taping>,
    secrets: Arc<Secrets>,
    ledger: Arc<Ledger>,
    scope: Option<Arc<AppScope>>,
}

macro_rules! own_app_hint {
    () => {
        ". For your own app under test, list its package in PHONEDRIVER_APPS_UNDER_TEST in the environment the daemon starts with (run `phonedriver daemon stop` first if it is running)"
    };
}

mod act;
mod approval;
mod observe;
mod ops;
mod report;
mod wait;

async fn next_signal(live: &mut Option<Live>) -> Signal {
    let Some(l) = live else {
        return pending().await;
    };
    tokio::select! {
        r = l.frames.changed() => match r {
            Ok(()) => Signal::Frame(*l.frames.borrow_and_update()),
            Err(_) => pending().await,
        },
        e = l.events.recv() => match e {
            Ok(DeviceEvent::WindowChanged { .. }) | Err(broadcast::error::RecvError::Lagged(_)) => Signal::Window,
            Ok(DeviceEvent::DisplayChanged { screen, .. }) => Signal::Display(screen),
            Ok(DeviceEvent::Gone | DeviceEvent::DensityChanged) => Signal::Gone,
            Ok(DeviceEvent::Locked(_)) => Signal::Power,
            Err(broadcast::error::RecvError::Closed) => pending().await,
        },
    }
}

/// True when the screen the session now holds gives an agent nothing to act on: a hollow tree, and no text read
/// from its pixels either (a game, a canvas, or a host with no text recognition).
fn nothing_to_act_on(s: &Session) -> bool {
    s.last.as_ref().is_some_and(|last| {
        let outside = last
            .header
            .app
            .as_ref()
            .is_some_and(|a| a.name.as_deref() == Some(phonedriver_core::scope::OUTSIDE_NAME));
        !outside && tree_is_hollow(last) && !last.elements.iter().any(|e| e.source == Source::Ocr)
    })
}

fn zero() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    }
}

impl Actor {
    async fn run(mut self, mut rx: mpsc::Receiver<Job>) {
        loop {
            let expiry = self.lease.as_ref().map(|l| l.last_call + LEASE_TTL);
            tokio::select! {
                job = rx.recv() => {
                    let Some(job) = job else { break };
                    let result = self.handle(job.session, job.terminal, job.share, job.display, job.call)
                        .await;
                    let phases = std::mem::take(&mut self.phases);
                    let _ = job.reply.send((result, phases));
                }
                () = async {
                    match expiry {
                        Some(t) => tokio::time::sleep_until(t).await,
                        None => pending().await,
                    }
                } => self.drop_lease(),
                sig = next_signal(&mut self.live) => {
                    let at = self.now();
                    self.absorb(sig, at);
                }
            }
            self.publish();
        }
    }

    fn publish(&self) {
        self.held.send_replace(Held {
            session: self.lease.as_ref().map(|l| l.session.clone()),
            last_call: self.lease.as_ref().map(|l| l.last_call),
            terminal: self.lease.as_ref().is_some_and(|l| l.terminal),
            hidden_display: self.live.as_ref().and_then(|l| l.device.hidden_display()),
        });
    }

    fn shown(&self) -> Option<RawTree> {
        self.shown.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn now(&self) -> Duration {
        self.base.elapsed()
    }

    fn lap(&mut self, phase: &'static str, since: Instant) {
        self.phases
            .push((phase, since.elapsed().as_micros() as u32));
    }

    fn drop_lease(&mut self) {
        self.lease = None;
        self.live = None;
        self.unobserved = None;
        self.cold_until = None;
        self.awake = None;
        self.guard_view = None;
        self.forget_apps();
    }

    /// The settle parameters for a call that only observes. Right after an action that was sent without settling, the
    /// effect may not have shown yet (a frame takes 100 to 300 ms to arrive), so quiet right now proves nothing:
    /// wait for the first change from the action's own start.
    fn observe(&mut self, mut cfg: Config, expect_change_ms: Option<u64>) -> (Config, Duration) {
        let now = self.now();
        let window = expect_change_ms
            .map(Duration::from_millis)
            .or(Config::action().expect_change);
        match self.unobserved.take() {
            Some(t) if window.is_some_and(|w| now.saturating_sub(t) < w) => {
                cfg.expect_change = window;
                (cfg, t)
            }
            _ => {
                self.during.clear();
                (cfg, now)
            }
        }
    }

    fn absorb(&mut self, sig: Signal, at: Duration) -> Option<Input> {
        let input = match sig {
            Signal::Frame(f) => Input::Frame {
                changed_area_pt2: f.changed_area_pt2,
                changed_box: f.changed_box.unwrap_or_else(zero),
            },
            Signal::Window => {
                if !self.power_is_pushed() {
                    self.awake = None;
                }
                Input::WindowChanged
            }
            Signal::Display(screen) => {
                self.awake = None;
                if let Some(l) = &mut self.live {
                    l.screen = screen;
                }
                Input::WindowChanged
            }
            Signal::Gone => {
                self.live = None;
                self.awake = None;
                return None;
            }
            Signal::Power => {
                self.awake = None;
                return None;
            }
        };
        let significant = match input {
            Input::WindowChanged => true,
            Input::Frame {
                changed_area_pt2, ..
            } => changed_area_pt2 > Config::action().significant_area,
        };
        if significant {
            self.recent = Some((at, input));
        }
        let rect = self.screen_rect();
        let reshaped = match input {
            Input::WindowChanged => true,
            Input::Frame {
                changed_area_pt2, ..
            } => changed_area_pt2 > rect.w * rect.h / 4.0,
        };
        if reshaped {
            self.reshaped = Some(at);
        }
        Some(input)
    }

    fn acquire(&mut self, session: &str, terminal: bool, share: bool) -> Result<(), Error> {
        let now = Instant::now();
        let fresh = Lease {
            session: session.to_string(),
            last_call: now,
            terminal,
        };
        match &mut self.lease {
            Some(l) if l.session == session => l.last_call = now,
            Some(l) if share => l.last_call = now,
            Some(l) => {
                let idle = now.saturating_duration_since(l.last_call);
                let hold = if l.terminal { TERMINAL_HOLD } else { LEASE_TTL };
                if l.terminal && idle >= hold {
                    self.lease = Some(fresh);
                    return Ok(());
                }
                return Err(Error::DeviceBusy {
                    id: self.id.clone(),
                    held_by: l.session.clone(),
                    idle_s: idle.as_secs(),
                    expires_s: hold.saturating_sub(idle).as_secs(),
                });
            }
            None => self.lease = Some(fresh),
        }
        Ok(())
    }

    async fn ensure_live(&mut self, display: DisplayMode) -> Result<(), Error> {
        if self.live.as_ref().is_some_and(|l| l.display == display) {
            return Ok(());
        }
        self.live = None;
        self.awake = None;
        self.guard_view = None;
        self.forget_apps();
        let mut device = self.source.connect_on(&self.id, display).await?;
        if let Some(scope) = &self.scope {
            device = Arc::new(ScopedDevice::new(device, scope.clone()));
        }
        let frames = device
            .frames()
            .unwrap_or_else(|| watch::channel(FrameRef::default()).1);
        let events = device.events();
        let screen = device.info().screen;
        let names = self.names.clone();
        let kept = self.apps.clone();
        let for_names = device.clone();
        tokio::spawn(async move {
            if let Ok(apps) = for_names.apps().await {
                let mut map = names.lock().unwrap_or_else(|e| e.into_inner());
                for a in &apps {
                    map.insert(a.id.0.clone(), a.name.clone());
                }
                kept.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get_or_insert(apps);
            }
        });
        self.recent = Some((self.now(), Input::WindowChanged));
        self.live = Some(Live {
            device,
            display,
            frames,
            events,
            screen,
            ledger_key: tokio::sync::OnceCell::new(),
        });
        Ok(())
    }

    async fn handle(
        &mut self,
        session: String,
        terminal: bool,
        share: bool,
        display: DisplayMode,
        call: Call,
    ) -> Result<Value, Error> {
        if matches!(call, Call::Release | Call::Forget) {
            if self.lease.as_ref().is_some_and(|l| l.session == session) {
                self.drop_lease();
            }
            if matches!(call, Call::Forget) {
                self.sessions.remove(&session);
            }
            return Ok(json!({}));
        }
        self.acquire(&session, terminal, share)?;
        self.publish();
        let t = Instant::now();
        let live = self.ensure_live(display).await;
        self.lap("connect", t);
        if let Err(e) = live {
            if self.lease.as_ref().is_some_and(|l| l.session == session) {
                self.lease = None;
            }
            return Err(e);
        }
        let mut s = self.sessions.remove(&session).unwrap_or_default();
        let result = self.dispatch(&mut s, call).await;
        self.sessions.insert(session, s);
        if let Err(Error::DeviceGone { .. }) = result {
            self.live = None;
        }
        result
    }

    /// Whether the device tells when its screen or lock changes (Android's agent does), so a screen once found
    /// awake stays so until it says otherwise.
    fn power_is_pushed(&self) -> bool {
        self.live.as_ref().is_some_and(|l| {
            l.device
                .info()
                .capabilities
                .contains(Capabilities::POWER_EVENTS)
        })
    }

    fn device(&self) -> Result<Arc<dyn Device>, Error> {
        self.live
            .as_ref()
            .map(|l| l.device.clone())
            .ok_or_else(|| Error::DeviceGone {
                id: self.id.clone(),
            })
    }

    /// The app `query` names. Every launch listed every installed app (`pm` and each app's label on Android) to find
    /// one it had found before, so the list is kept for the connection; a query the kept list does not hold by exact
    /// id or name is listed again, so an app installed since is found and never mistaken for a kept one.
    async fn app_named(&mut self, device: &dyn Device, query: &str) -> Result<App, Error> {
        let known = self
            .apps
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_deref()
            .and_then(|kept| find_app(kept, query).ok().cloned())
            .filter(|found| found.id.0 == query || found.name.eq_ignore_ascii_case(query));
        if let Some(found) = known {
            return Ok(found);
        }
        let listed = device.apps().await?;
        let found = self.find_app(&listed, query)?.clone();
        *self.apps.lock().unwrap_or_else(|e| e.into_inner()) = Some(listed);
        Ok(found)
    }

    fn forget_apps(&self) {
        *self.apps.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    fn find_app<'a>(&self, installed: &'a [App], query: &str) -> Result<&'a App, Error> {
        find_app(installed, query).map_err(|e| match (&self.scope, e) {
            (Some(scope), Error::NoApp { query, .. }) => Error::Refused {
                detail: format!(
                    "no app named \"{query}\" is in the app scope ({}): phonedriver lists, launches and changes only those apps. Name one of them; the scope is the person's setting (PHONEDRIVER_APP_SCOPE)",
                    scope.list()
                ),
            },
            (_, e) => e,
        })
    }

    fn need(&self, cap: Capabilities, op: Op, reason: &'static str) -> Result<(), Error> {
        let device = self.device()?;
        if device.info().capabilities.contains(cap) {
            Ok(())
        } else {
            Err(Error::Unsupported { op, reason })
        }
    }

    /// Steps about to be saved, each launch with the build of the app it launches, read once per app. A replay
    /// refuses another build: the steps were saved on that one's screens.
    async fn with_builds(&self, mut journal: Value) -> Value {
        let Ok(device) = self.device() else {
            return journal;
        };
        let mut builds: HashMap<String, Option<String>> = HashMap::new();
        let Some(steps) = journal["steps"].as_array_mut() else {
            return journal;
        };
        for step in steps {
            let action = &step["params"]["action"];
            let Some(app) = action["app"]
                .as_str()
                .filter(|_| action["kind"] == "launch")
            else {
                continue;
            };
            let app = app.to_string();
            if !builds.contains_key(&app) {
                let build = device.app_build(&AppId(app.clone())).await.ok().flatten();
                builds.insert(app.clone(), build);
            }
            if let Some(build) = &builds[&app] {
                step["build"] = json!(build);
            }
        }
        journal
    }

    async fn dispatch(&mut self, s: &mut Session, call: Call) -> Result<Value, Error> {
        let value = match call {
            Call::Snapshot(p) => {
                let mut seen = self.snapshot(s, p).await?;
                seen.empty = nothing_to_act_on(s);
                to_value(seen)
            }
            Call::Screenshot(p) => to_value(self.screenshot(s, p).await?),
            Call::Act(p) => {
                let mut done = match self.act(s, p).await {
                    Ok(done) => done,
                    Err(e) => return Err(self.with_screen(s, e).await),
                };
                done.observation.empty = nothing_to_act_on(s);
                to_value(done)
            }
            Call::Wait(p) => {
                let mut done = self.wait(s, p).await?;
                done.observation.empty = nothing_to_act_on(s);
                to_value(done)
            }
            Call::Check(p) => {
                let mut done = match self.check(s, p).await {
                    Ok(done) => done,
                    Err(e) => return Err(self.with_screen(s, e).await),
                };
                done.observation.empty = nothing_to_act_on(s);
                to_value(done)
            }
            Call::Batch(p) => {
                let mut done = self.batch(s, p).await?;
                done.observation.empty = nothing_to_act_on(s);
                to_value(done)
            }
            Call::Find(p) => {
                let now = self.fresh(s).await?;
                to_value(s.find(&now, &p.text, p.role))
            }
            Call::Apps => {
                self.need(Capabilities::APPS, Op::Apps, "this device cannot list apps")?;
                to_value(self.device()?.apps().await?)
            }
            Call::App(p) => {
                let step = journal::call(
                    "app",
                    json!({"action": p.action, "app": p.app, "path": p.path, "permissions": p.permissions}),
                );
                let done = self.app(s, p).await?;
                s.journal.push(step);
                done
            }
            Call::Clipboard(p) => {
                self.need(
                    Capabilities::CLIPBOARD,
                    Op::Clipboard,
                    "this device has no clipboard access",
                )?;
                let device = self.device()?;
                match p.set {
                    Some(text) => {
                        device.clipboard(ClipboardOp::Set(text)).await?;
                        json!({})
                    }
                    None => json!({"text": device.clipboard(ClipboardOp::Get).await?}),
                }
            }
            Call::Shell(p) => {
                let (command, secret) = (p.command.clone(), p.secret);
                let done = self.shell(s, p).await?;
                let step = if secret {
                    journal::manual("a shell command was run here with `--secret`; run it again")
                } else {
                    let mut step = journal::call("shell", json!({"command": command}));
                    if done.exit != 0 {
                        step["expect_exit"] = json!(done.exit);
                    }
                    step
                };
                s.journal.push(step);
                to_value(done)
            }
            Call::Setting(p) => {
                let mut params = json!({});
                for (name, value) in [
                    ("key", json!(p.key)),
                    ("value", json!(p.value)),
                    ("values", json!(p.values)),
                ] {
                    if !value.is_null() {
                        params[name] = value;
                    }
                }
                if p.force {
                    params["force"] = json!(true);
                }
                let writes = p.value.is_some() || p.values.is_some();
                let done = self.setting(s, p).await?;
                if writes {
                    s.journal.push(journal::call("setting", params));
                }
                done
            }
            Call::Journal(p) => match p.drop {
                Some(n) => {
                    s.journal.drop_first(n);
                    json!({})
                }
                None if p.keep => self.with_builds(s.journal.peek()).await,
                None => self.with_builds(s.journal.take()).await,
            },
            Call::Build(p) => json!({"build": self.device()?.app_build(&AppId(p.app)).await?}),
            Call::Logs(p) => self.logs(s, p).await?,
            Call::Network => to_value(self.device()?.network().await?),
            Call::Page(p) => self.page(p).await?,
            Call::Record(p) => self.record(p).await?,
            Call::Health => self.health().await?,
            Call::Release | Call::Forget => json!({}),
        };
        Ok(value)
    }
}

/// A reply that only takes lines away is a window caught while it opens: the old screen is gone and the new one is
/// not in the tree yet.
fn only_lost_lines(s: &Session, now: &Rendered) -> bool {
    matches!(
        s.last.as_ref().map(|prev| diff(prev, now)),
        Some(DiffOrFull::Diff(changes))
            if !changes.is_empty() && changes.iter().all(|c| matches!(c, Change::Removed(_)))
    )
}

/// What a reply says about `now`: the changes since the screen the session last saw, or the whole screen when it saw
/// none or this one is another screen.
fn answer(s: &mut Session, now: &Rendered) -> (Kind, String) {
    match s.diff_for_reply(now) {
        Some(DiffOrFull::Diff(changes)) => (Kind::Diff, DiffOrFull::Diff(changes).text(now)),
        _ => (
            Kind::Full,
            now.text(&Filters::default()).unwrap_or_default(),
        ),
    }
}

/// A screen with at most two elements besides the keyboard: it is rarely the whole of a screen an app opens. A web
/// page can be that short (one line of text), so a screen with a web view is never called bare.
fn bare(now: &Rendered) -> bool {
    let web = now.elements.iter().any(|e| e.role == Role::WebView);
    let shown = now
        .elements
        .iter()
        .filter(|e| e.role != Role::Keyboard)
        .count();
    !web && shown <= 2
}

/// `field` is a search field and at most one other element has changed since `prev`: its results are not in yet.
fn search_unanswered(prev: Option<&Rendered>, now: &Rendered, field: Ref) -> bool {
    let Some(e) = now.index_of(field).map(|i| &now.elements[i]) else {
        return false;
    };
    let named = |s: &Option<String>| {
        s.as_deref()
            .is_some_and(|s| s.to_ascii_lowercase().contains("search"))
    };
    if e.role != Role::SearchField && !named(&e.hint) && !named(&e.id) && !named(&e.label) {
        return false;
    }
    let Some(DiffOrFull::Diff(changes)) = prev.map(|p| diff(p, now)) else {
        return false;
    };
    let answers = changes
        .iter()
        .filter(|c| {
            let r = match c {
                Change::Added(r) => *r,
                Change::Changed { r#ref, .. } => *r#ref,
                _ => return false,
            };
            r != field
                && now
                    .index_of(r)
                    .is_some_and(|i| now.elements[i].role != Role::Keyboard)
        })
        .count();
    answers < 2
}

/// The screen changed somewhere other than `field` and the keyboard.
fn moved_beyond(s: &Session, now: &Rendered, field: Option<Ref>) -> bool {
    let Some(prev) = s.last.as_ref() else {
        return false;
    };
    let DiffOrFull::Diff(changes) = diff(prev, now) else {
        return true;
    };
    changes.iter().any(|c| {
        let r = match c {
            Change::Uncovered => return true,
            Change::Keyboard(_) => return false,
            Change::Added(r) | Change::Removed(r) => *r,
            Change::Changed {
                field: Field::Covered,
                ..
            } => return false,
            Change::Changed { r#ref, .. } => *r#ref,
        };
        let element = now
            .index_of(r)
            .map(|i| &now.elements[i])
            .or_else(|| prev.index_of(r).map(|i| &prev.elements[i]));
        Some(r) != field && element.is_none_or(|e| e.role != Role::Keyboard)
    })
}

/// For a target like "Row 500" among rows labelled "Row 12", "Row 13": whether the target lies against `direction`,
/// how many rows away it is, and how many rows are visible. Lets the search fling across a long list and come back
/// instead of dragging a screen at a time.
fn numbered_distance(
    until: &Target,
    now: &Rendered,
    direction: Direction,
) -> Option<(bool, i64, i64)> {
    let Target::Text { text, .. } = until else {
        return None;
    };
    let split = |label: &str| -> Option<(String, i64)> {
        let digits = label.len() - label.trim_end_matches(|c: char| c.is_ascii_digit()).len();
        let (head, tail) = label.split_at(label.len() - digits);
        Some((head.to_string(), tail.parse().ok()?))
    };
    let (prefix, want) = split(text)?;
    let seen: Vec<i64> = now
        .elements
        .iter()
        .filter_map(|e| split(e.label.as_deref()?))
        .filter(|(p, _)| *p == prefix)
        .map(|(_, n)| n)
        .collect();
    let (min, max) = (*seen.iter().min()?, *seen.iter().max()?);
    let forward = matches!(direction, Direction::Down | Direction::Right);
    let (behind, rows) = if want > max {
        (!forward, want - max)
    } else if want < min {
        (forward, min - want)
    } else {
        return None;
    };
    Some((behind, rows, max - min + 1))
}

/// The part of a scroll area above the keyboard: a drag that starts on the keys moves nothing (iOS Contacts' new
/// contact form, "no element changed"). The whole area when too little of it is left.
fn above_keyboard(area: Rect, keyboard: Option<Rect>) -> Rect {
    match keyboard {
        Some(k) if k.y > area.y && k.y < area.y + area.h && k.y - area.y >= MIN_SCROLL_SPAN => {
            Rect {
                h: k.y - area.y,
                ..area
            }
        }
        _ => area,
    }
}

const MIN_SCROLL_SPAN: f32 = 120.0;

/// Where a name is against a list whose rows are in alphabetical order (Settings' list of apps): `(behind, far)`,
/// far when its first letter is at least `FAR_LETTERS` past the nearest row's. `None` when the rows are not sorted
/// or the name falls between them.
fn sorted_distance(
    until: &Target,
    now: &Rendered,
    direction: Direction,
    holder: Option<Ref>,
) -> Option<(bool, bool)> {
    const FAR_LETTERS: u32 = 3;
    let Target::Text { text, .. } = until else {
        return None;
    };
    if text.contains('|') {
        return None;
    }
    let list = now.index_of(holder?)?;
    let rows: Vec<String> = now
        .elements
        .iter()
        .enumerate()
        .filter(|(i, _)| now.parents[*i] == Some(list))
        .filter_map(|(_, e)| e.label.as_deref().map(str::to_lowercase))
        .collect();
    if rows.len() < 4 || rows.windows(2).any(|w| w[0] > w[1]) {
        return None;
    }
    let want = text.to_lowercase();
    let (first, last) = (rows.first()?, rows.last()?);
    let letter = |s: &str| {
        s.chars()
            .next()
            .filter(char::is_ascii_lowercase)
            .map(u32::from)
    };
    let forward = matches!(direction, Direction::Down | Direction::Right);
    let (behind, gap) = if want > *last {
        (!forward, letter(&want)?.checked_sub(letter(last)?)?)
    } else if want < *first {
        (forward, letter(first)?.checked_sub(letter(&want)?)?)
    } else {
        return None;
    };
    Some((behind, gap >= FAR_LETTERS))
}

/// Without `max`, 30 scrolls, or enough one-screen drags to cross the rows a numbered target is away.
fn default_scroll_limit(until: &Target, now: &Rendered, direction: Direction) -> u32 {
    let drags = numbered_distance(until, now, direction)
        .map_or(0, |(_, rows, span)| (rows / span.max(1) * 2 + 10).min(300));
    30.max(u32::try_from(drags).unwrap_or(30))
}

/// The label of the last element in `direction` inside `area`.
fn last_label(now: &Rendered, area: Rect, direction: Direction) -> Option<String> {
    let along = |e: &&Element| match direction {
        Direction::Down => e.bounds.center().y,
        Direction::Up => -e.bounds.center().y,
        Direction::Right => e.bounds.center().x,
        Direction::Left => -e.bounds.center().x,
    };
    let in_toolbar = |i: usize| {
        std::iter::successors(now.parents[i], |&p| now.parents[p])
            .any(|p| now.elements[p].label.as_deref() == Some("Toolbar"))
    };
    now.elements
        .iter()
        .enumerate()
        .filter(|(i, e)| {
            !matches!(e.role, Role::List | Role::Other)
                && area.contains(e.bounds)
                && e.label.is_some()
                && !in_toolbar(*i)
        })
        .map(|(_, e)| e)
        .max_by(|a, b| along(a).total_cmp(&along(b)))
        .and_then(|e| e.label.clone())
}

/// The largest element that scrolls along the axis of `direction`.
fn scroll_container(now: &Rendered, direction: Direction) -> Option<&Element> {
    let axis = match direction {
        Direction::Up | Direction::Down => Actions::SCROLL_UP | Actions::SCROLL_DOWN,
        Direction::Left | Direction::Right => Actions::SCROLL_LEFT | Actions::SCROLL_RIGHT,
    };
    now.elements
        .iter()
        .filter(|e| e.actions.intersects(axis))
        .max_by(|a, b| a.bounds.area().total_cmp(&b.bounds.area()))
}

/// The reply to an input sent with `settle: false`: delivered, nothing observed.
fn delivered(s: &Session, summary: String, via: Option<&'static str>) -> ActionResult {
    ActionResult {
        at: None,
        typed_into: None,
        unchanged_repeats: None,
        crashed: None,
        ok: true,
        summary,
        via: via.map(str::to_string),
        verified: None,
        unverified: None,
        settled: false,
        settle_ms: 0,
        changed: false,
        woke: false,
        unlocked: false,
        observation: Observation {
            header: s
                .last
                .as_ref()
                .map(|l| l.header.clone())
                .unwrap_or_default(),
            text: String::new(),
            kind: Kind::Diff,
            changed: false,
            settled: false,
            settle_ms: 0,
            still_changing: None,
            loading: false,
            elements: None,
            capture: None,
            empty: false,
        },
    }
}

/// The first text field inside a group, when the agent named the group (a search bar's container).
fn text_entry_within(now: &Rendered, group: Ref) -> Option<Element> {
    let mut stack = vec![now.index_of(group)?];
    while let Some(i) = stack.pop() {
        for &c in now.elements[i].children.iter().rev() {
            if now.elements[c].role.is_text_entry() {
                return Some(now.elements[c].clone());
            }
            stack.push(c);
        }
    }
    None
}

fn is_secure(e: &Element) -> bool {
    e.role == Role::SecureField || e.state.contains(ElementState::PASSWORD)
}

fn withhold(e: Error, secure: bool) -> Error {
    match e {
        Error::Transport { .. } if secure => Error::Transport {
            detail: "typing failed; the device's message is withheld because the text is secret"
                .into(),
        },
        e => e,
    }
}

fn mode_name(m: TypeMode) -> &'static str {
    match m {
        TypeMode::Paste => "paste",
        TypeMode::Set => "set",
        TypeMode::Keys => "keys",
    }
}

/// `snapshot --audit`'s lines: how many problems the screen has and the first `AUDIT_LINES` of them (decision D90).
fn audited(r: &Rendered) -> String {
    use phonedriver_core::audit::Finding;
    let found = phonedriver_core::audit::audit(r);
    if found.is_empty() {
        return "\naudit: no unnamed controls or small tap targets".into();
    }
    let mut out = format!("\naudit: {}", plural(found.len() as u64, "finding"));
    for f in found.iter().take(AUDIT_LINES) {
        out.push('\n');
        out.push_str(f.line());
    }
    if found.len() > AUDIT_LINES {
        out.push_str(&format!("\nand {} more", found.len() - AUDIT_LINES));
    }
    if found.iter().any(|f| matches!(f, Finding::SmallTarget(_))) {
        out.push('\n');
        out.push_str(phonedriver_core::audit::MEASURED_FROM);
    }
    out
}

fn settle_line(st: State) -> SettleLine {
    match st {
        State::Done {
            settled, elapsed, ..
        } => SettleLine {
            settled,
            ms: elapsed.as_millis() as u32,
        },
        State::Waiting => SettleLine {
            settled: false,
            ms: 0,
        },
    }
}

/// An app by package, then by name in any case, then by a name that contains the text; a close misspelling is `NotFound`
/// with the names it might have meant.
fn find_app<'a>(installed: &'a [App], query: &str) -> Result<&'a App, Error> {
    let q = query.to_lowercase();
    let steps: [fn(&App, &str, &str) -> bool; 3] = [
        |a, query, _| a.id.0 == query,
        |a, _, q| a.name.to_lowercase() == q,
        |a, _, q| a.name.to_lowercase().contains(q),
    ];
    for step in steps {
        let hits: Vec<&App> = installed.iter().filter(|a| step(a, query, &q)).collect();
        match hits.as_slice() {
            [] => {}
            [one] => return Ok(one),
            many => {
                return Err(Error::Ambiguous {
                    query: query.to_string(),
                    matches: many
                        .iter()
                        .map(|a| format!("{} ({})", a.name, a.id.0))
                        .collect(),
                });
            }
        }
    }
    if let Some(app) = named_inside(installed, query) {
        return Ok(app);
    }
    let words = |s: &str| -> Vec<String> {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.chars().count() >= 3)
            .map(str::to_string)
            .collect()
    };
    let wanted = words(query);
    let mut close: Vec<(f32, &App)> = installed
        .iter()
        .map(|a| {
            let shares = words(&a.name).iter().any(|w| wanted.contains(w));
            let d = normalized_distance(query, &a.name);
            (if shares { d.min(SUGGEST_WITHIN) } else { d }, a)
        })
        .filter(|(d, _)| *d <= SUGGEST_WITHIN)
        .collect();
    close.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut installed: Vec<String> = if close.is_empty() && installed.len() <= NO_APP_LIST {
        installed.iter().map(|a| a.name.clone()).collect()
    } else {
        Vec::new()
    };
    installed.sort_by_key(|n| n.to_lowercase());
    Err(Error::NoApp {
        query: query.to_string(),
        similar: close
            .into_iter()
            .take(5)
            .map(|(_, a)| format!("{} ({})", a.name, a.id.0))
            .collect(),
        installed,
    })
}

/// Agents name an app with its maker in front ("Google Play Store" for Play Store, "Google Chrome" for Chrome). An
/// installed name that ends the query in whole words wins, the longest first; "Google Maps" does not open Google.
fn named_inside<'a>(installed: &'a [App], query: &str) -> Option<&'a App> {
    let words = |s: &str| -> Vec<String> {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect()
    };
    let q = words(query);
    let mut best: Vec<(usize, &App)> = Vec::new();
    for a in installed {
        let name = words(&a.name);
        if name.is_empty() || name.len() >= q.len() || !q.ends_with(&name) {
            continue;
        }
        match best.first() {
            Some((n, _)) if *n > name.len() => {}
            Some((n, _)) if *n == name.len() => best.push((name.len(), a)),
            _ => best = vec![(name.len(), a)],
        }
    }
    match best.as_slice() {
        [(_, one)] => Some(one),
        _ => None,
    }
}

fn describe(target: &Target) -> String {
    match target {
        Target::Text { text, .. }
        | Target::Id {
            text: Some(text), ..
        } => format!("\"{text}\""),
        Target::Id { id, .. } => id.clone(),
        Target::Ref { r#ref } => format!("@{}", r#ref),
        Target::Point { point } => format!("{},{}", point.x, point.y),
    }
}

/// `@e6 button "Display & touch" "Dark theme, font size"` becomes `@e6 "Display & touch"`.
fn short(line: &str) -> String {
    let mut parts = line.splitn(3, ' ');
    let r = parts.next().unwrap_or_default();
    parts.next();
    match parts.next() {
        Some(rest) => match first_quoted(rest) {
            Some(q) => format!("{r} {q}"),
            None => r.to_string(),
        },
        None => r.to_string(),
    }
}

fn first_quoted(s: &str) -> Option<String> {
    let rest = s.strip_prefix('"')?;
    let mut out = String::from("\"");
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        out.push(c);
        match c {
            '\\' => out.extend(chars.next()),
            '"' => return Some(out),
            _ => {}
        }
    }
    None
}

/// Presses a button or key; returns how, for `back` on platforms with more than one way.
async fn press(device: &dyn Device, key: &str) -> Result<Option<&'static str>, Error> {
    let name = Value::String(key.replace('-', "_"));
    if let Ok(b) = serde_json::from_value::<Button>(name.clone()) {
        if b == Button::Back {
            return device.back().await;
        }
        return device.button(b).await.map(|()| None);
    }
    if let Ok(k) = serde_json::from_value::<Key>(name) {
        return device.key(k).await.map(|()| None);
    }
    Err(Error::UnknownKey {
        key: key.to_string(),
        valid: vec![
            "home".into(),
            "back".into(),
            "recents".into(),
            "notifications".into(),
            "lock".into(),
            "volume-up".into(),
            "volume-down".into(),
            "enter".into(),
            "hide-keyboard".into(),
            "fingerprint".into(),
            "wrong-fingerprint".into(),
            "landscape".into(),
            "portrait".into(),
        ],
    })
}

/// What the human has said about held actions, for one action: their approval, the opt-in categories to ask about,
/// and what they were shown when they approved.
struct Consent {
    approval: Option<Approval>,
    ask: Vec<Category>,
    approved: Option<Approved>,
}

fn stale(r: Ref, s: &Session) -> Error {
    Error::StaleRef {
        r#ref: r,
        closest: None,
        unseen: s.unseen(r),
        screen: None,
    }
}

fn unreachable_state() -> Error {
    Error::Transport {
        detail: "the session lost its last screen".into(),
    }
}

fn to_value<T: serde::Serialize>(v: T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

fn approved_summary(summary: String, note: Option<&str>) -> String {
    match note {
        Some(note) => format!("{summary}; {note}"),
        None => summary,
    }
}

fn field_empty(now: Option<&Rendered>, field: Option<Ref>) -> bool {
    now.zip(field)
        .and_then(|(n, r)| n.index_of(r).map(|i| &n.elements[i]))
        .is_none_or(|e| e.value.as_deref().is_none_or(str::is_empty))
}

#[cfg(test)]
mod tests {
    use super::*;
    use phonedriver_core::device::{App, AppId};

    fn apps(names: &[(&str, &str)]) -> Vec<App> {
        names
            .iter()
            .map(|(name, id)| App {
                id: AppId(id.to_string()),
                name: name.to_string(),
                system: false,
            })
            .collect()
    }

    #[test]
    fn a_search_for_either_of_two_names_does_not_fling_through_a_sorted_list() {
        use phonedriver_core::geometry::Scale;
        use phonedriver_core::raw::RawTree;
        use phonedriver_core::refs::RefTable;
        use phonedriver_core::render::Header;
        let rows: Vec<String> = ["Alpha", "Bravo", "Charlie", "Delta", "Echo"]
            .iter()
            .enumerate()
            .map(|(i, l)| {
                format!(
                    r#"{{"p":0,"w":1,"cls":"android.widget.TextView","text":"{l}","b":[0,{},400,{}],"f":"vVe","a":["tap"]}}"#,
                    i * 50,
                    i * 50 + 40
                )
            })
            .collect();
        let json = format!(
            r#"{{"windows":[{{"id":1,"type":"application","bounds":[0,0,400,800]}}],"nodes":[
            {{"p":-1,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,0,400,800],"f":"vVeS","a":["scroll_fwd"]}},{}]}}"#,
            rows.join(",")
        );
        let tree = RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap();
        let nodes = phonedriver_core::prune::prune(&tree);
        let refs = RefTable::default().assign(&tree, &nodes);
        let now = Rendered::build(nodes, &refs, Header::default());
        let list = Some(now.elements[0].r#ref);
        let text = |t: &str| Target::Text {
            text: t.into(),
            role: None,
        };
        assert_eq!(
            sorted_distance(&text("Zulu"), &now, Direction::Down, list),
            Some((false, true))
        );
        assert_eq!(
            sorted_distance(&text("Zulu|Alpha"), &now, Direction::Down, list),
            None
        );
    }

    #[test]
    fn a_search_field_waits_for_its_results_and_a_form_field_does_not() {
        use phonedriver_core::geometry::Scale;
        use phonedriver_core::refs::RefTable;
        use phonedriver_core::render::Header;
        let mut table = RefTable::default();
        let mut screen = |hint: &str, text: &str, rows: &[&str]| {
            let mut nodes = vec![
                r#"{"p":-1,"w":1,"cls":"android.widget.ImageButton","desc":"Back","b":[0,0,50,50],"n":"back","f":"vVe","a":["tap"]}"#.to_string(),
                format!(
                    r#"{{"p":-1,"w":1,"cls":"android.widget.EditText","text":"{text}","hint":"{hint}","b":[50,0,350,50],"n":"field","f":"vVe","a":["tap","set_text"]}}"#
                ),
            ];
            for (i, row) in rows.iter().enumerate() {
                let y = 60 + i * 60;
                nodes.push(format!(
                    r#"{{"p":-1,"w":1,"cls":"android.widget.Button","text":"{row}","b":[0,{y},400,{}],"n":"{row}","f":"vVe","a":["tap"]}}"#,
                    y + 50
                ));
            }
            let json = format!(
                r#"{{"windows":[{{"id":1,"type":"application","bounds":[0,0,400,800]}}],"nodes":[{}]}}"#,
                nodes.join(",")
            );
            let tree = RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap();
            let pruned = phonedriver_core::prune::prune(&tree);
            let refs = table.assign(&tree, &pruned);
            Rendered::build(pruned, &refs, Header::default())
        };
        let field = |r: &Rendered| {
            r.elements
                .iter()
                .find(|e| e.role.is_text_entry())
                .unwrap()
                .r#ref
        };
        let empty = screen("Search for a city", "", &[]);
        let typed = screen("Search for a city", "Tokyo", &["Clear text"]);
        let answered = screen(
            "Search for a city",
            "Tokyo",
            &["Clear text", "Tokyo, Japan", "Yokohama, Japan"],
        );
        assert!(search_unanswered(Some(&empty), &typed, field(&typed)));
        assert!(!search_unanswered(
            Some(&empty),
            &answered,
            field(&answered)
        ));
        let other = screen(
            "Search for a city",
            "Osaka",
            &["Clear text", "Tokyo, Japan", "Yokohama, Japan"],
        );
        assert!(search_unanswered(Some(&answered), &other, field(&other)));
        let form = screen("Email", "", &[]);
        let filled = screen("Email", "a@b.c", &[]);
        assert!(!search_unanswered(Some(&form), &filled, field(&filled)));
    }

    #[test]
    fn an_app_asked_for_with_its_maker_in_front_is_found() {
        let installed = apps(&[
            ("Play Store", "com.android.vending"),
            ("Google", "com.google.android.googlequicksearchbox"),
            ("Chrome", "com.android.chrome"),
            ("Google TV", "com.google.android.videos"),
        ]);
        let id = |q: &str| find_app(&installed, q).map(|a| a.id.0.clone());
        assert_eq!(id("Google Play Store").unwrap(), "com.android.vending");
        assert_eq!(id("Google Chrome").unwrap(), "com.android.chrome");
        assert!(matches!(id("Store"), Ok(i) if i == "com.android.vending"));
        assert!(id("Google Maps").is_err());
    }
}
