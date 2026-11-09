//! One task per device. It owns the device connection, the lease and the sessions, so no lock guards device state.

use std::collections::HashMap;
use std::future::pending;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use phonedriver_core::device::{
    App, AppId, AppOp, Button, Capabilities, ClipboardOp, Device, DeviceEvent, DisplayMode,
    FrameRef, Gesture, Key, Quality, ScreenState, ShellRequest, TextInput, TimedPoint, Url,
};
use phonedriver_core::diff::{Change, DiffOrFull, Field, diff};
use phonedriver_core::element::{Actions, Element, ElementState, Ref, Role, Source};
use phonedriver_core::error::{Category, Direction, Error, Op, ScrollEnd, UserAction};
use phonedriver_core::geometry::{Point, Rect, Size};
use phonedriver_core::gesture;
use phonedriver_core::ocr::{merge, tree_is_hollow};
use phonedriver_core::prune::prune;
use phonedriver_core::raw::RawTree;
use phonedriver_core::render::{
    AppLine, DeviceLine, Filters, Header, Orientation, Rendered, SettleLine, plural, quote,
};
use phonedriver_core::settle::{Config, Detector, Input, State};
use phonedriver_core::similar::{SUGGEST_WITHIN, normalized_distance};
use serde_json::{Value, json};
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time::Instant;

use super::DeviceSource;
use super::checkbox;
use super::session::{Resolved, Session};
use super::shell;
use super::verify::{Expect, Verdict, Verify};
use super::{guard, secret_field};
use crate::wire::{
    ActParams, Action, ActionResult, AppParams, Approval, BatchParams, BatchResult,
    ClipboardParams, FindParams, Kind, Observation, PinchDirection, ScreenshotParams,
    ScreenshotResult, ShellParams, ShellResult, SnapshotParams, Step, StepFailure, StepResult,
    Target, TypeMode, WaitCondition, WaitParams,
};

pub const LEASE_TTL: Duration = Duration::from_secs(600);
const TICK: Duration = Duration::from_millis(5);
const SCREEN_WAIT: Duration = Duration::from_secs(3);
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
const WEB_VIEW_WAIT: Duration = Duration::from_secs(10);
const LEAVE_WAIT: Duration = Duration::from_millis(2500);
/// Observed on the API 37 tablet emulator: the first "Add alarm" tap after a cold Clock launch opened its time picker
/// after more than 500 ms, so the reply said "no change" while the dialog was on its way, and the agent tapped again.
const COLD_APP_WINDOW: Duration = Duration::from_secs(30);
const COLD_CHANGE_WAIT: Duration = Duration::from_millis(2000);
/// How long a tap whose frames changed but whose elements did not waits for a page that starts late.
const LATE_PAGE_WAIT: Duration = Duration::from_millis(600);
const MASK: &str = "•••";

pub enum Call {
    Snapshot(SnapshotParams),
    Screenshot(ScreenshotParams),
    Act(ActParams),
    Wait(WaitParams),
    Batch(BatchParams),
    Find(FindParams),
    Apps,
    App(AppParams),
    Clipboard(ClipboardParams),
    Shell(ShellParams),
    /// What `doctor` needs: the device answers a tree read, and a frame arrives.
    Health,
    Release,
    /// Who holds the lease, without taking it.
    Leased,
}

/// Where an action's time went, in call order: `(phase, ms)`. In microseconds; written into the trace line as milliseconds.
pub type Phases = Vec<(&'static str, u32)>;

pub struct Job {
    pub session: String,
    pub share: bool,
    pub display: DisplayMode,
    pub call: Call,
    pub reply: oneshot::Sender<(Result<Value, Error>, Phases)>,
}

pub fn spawn(id: String, source: Arc<dyn DeviceSource>, home: PathBuf) -> mpsc::Sender<Job> {
    let (tx, rx) = mpsc::channel(32);
    let actor = Actor {
        id,
        source,
        home,
        lease: None,
        live: None,
        sessions: HashMap::new(),
        base: Instant::now(),
        recent: None,
        unobserved: None,
        cold_until: None,
        awake: None,
        phases: Vec::new(),
        shown: Mutex::new(None),
        guard_view: None,
        reshaped: None,
        names: Arc::default(),
    };
    tokio::spawn(actor.run(rx));
    tx
}

struct Lease {
    session: String,
    last_call: Instant,
}

struct Live {
    device: Arc<dyn Device>,
    display: DisplayMode,
    frames: watch::Receiver<FrameRef>,
    events: broadcast::Receiver<DeviceEvent>,
    screen: Size,
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
    Gone,
    Ignored,
}

/// A screen found on and unlocked stays that way for this long unless a window or display event says otherwise:
/// reading the power state costs an agent round trip on every action, and on the bare tap it is most of what is
/// not the app handling the event.
const AWAKE_REUSE: Duration = Duration::from_secs(1);
/// How long the screen read for a point tap's guard check may serve the next point tap, while no window change or
/// repaint of a quarter of the screen has arrived since.
const GUARD_VIEW_REUSE: Duration = Duration::from_secs(1);

struct Actor {
    id: String,
    source: Arc<dyn DeviceSource>,
    home: PathBuf,
    lease: Option<Lease>,
    live: Option<Live>,
    sessions: HashMap<String, Session>,
    base: Instant,
    /// The latest input that would have delayed a settle, or the moment the device connected: quiet counts from there, so a snapshot of an already quiet screen does not wait.
    recent: Option<(Duration, Input)>,
    /// When an action was last delivered without waiting for its effects.
    unobserved: Option<Duration>,
    /// Until when the app a launch started may still be too busy to react within the usual first-change window.
    cold_until: Option<Duration>,
    /// When `prepare` last found the screen on and unlocked; cleared by any window or display event.
    awake: Option<Duration>,
    /// The screen the last point tap's guard check read, and when.
    guard_view: Option<(Duration, Rendered)>,
    /// When a window changed or a quarter of the screen repainted last: a button's ripple (about 100 by 100 pt) is a
    /// repaint that leaves the tree as it was.
    reshaped: Option<Duration>,
    phases: Phases,
    /// The tree the agent was last shown, to tell whether an action changed anything before a frame says so.
    shown: Mutex<Option<RawTree>>,
    names: Arc<Mutex<HashMap<String, String>>>,
}

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
            Ok(DeviceEvent::Locked(_)) => Signal::Ignored,
            Err(broadcast::error::RecvError::Closed) => pending().await,
        },
    }
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
                    let result = self.handle(job.session, job.share, job.display, job.call)
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
        }
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
            _ => (cfg, now),
        }
    }

    fn absorb(&mut self, sig: Signal, at: Duration) -> Option<Input> {
        let input = match sig {
            Signal::Frame(f) => Input::Frame {
                changed_area_pt2: f.changed_area_pt2,
                changed_box: f.changed_box.unwrap_or_else(zero),
            },
            Signal::Window => {
                self.awake = None;
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
            Signal::Ignored => return None,
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

    fn acquire(&mut self, session: &str, share: bool) -> Result<(), Error> {
        let now = Instant::now();
        match &mut self.lease {
            Some(l) if l.session == session => l.last_call = now,
            Some(l) if share => l.last_call = now,
            Some(l) => {
                let idle = now.saturating_duration_since(l.last_call);
                return Err(Error::DeviceBusy {
                    id: self.id.clone(),
                    held_by: l.session.clone(),
                    idle_s: idle.as_secs(),
                    expires_s: LEASE_TTL.saturating_sub(idle).as_secs(),
                });
            }
            None => {
                self.lease = Some(Lease {
                    session: session.to_string(),
                    last_call: now,
                });
            }
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
        let device = self.source.connect_on(&self.id, display).await?;
        let frames = device
            .frames()
            .unwrap_or_else(|| watch::channel(FrameRef::default()).1);
        let events = device.events();
        let screen = device.info().screen;
        let names = self.names.clone();
        let for_names = device.clone();
        tokio::spawn(async move {
            if let Ok(apps) = for_names.apps().await {
                let mut map = names.lock().unwrap_or_else(|e| e.into_inner());
                for a in apps {
                    map.insert(a.id.0, a.name);
                }
            }
        });
        self.recent = Some((self.now(), Input::WindowChanged));
        self.live = Some(Live {
            device,
            display,
            frames,
            events,
            screen,
        });
        Ok(())
    }

    async fn handle(
        &mut self,
        session: String,
        share: bool,
        display: DisplayMode,
        call: Call,
    ) -> Result<Value, Error> {
        if matches!(call, Call::Leased) {
            let hidden_display = self.live.as_ref().and_then(|l| l.device.hidden_display());
            return Ok(match &self.lease {
                Some(l) => {
                    json!({"session": l.session, "idle_s": l.last_call.elapsed().as_secs(), "hidden_display": hidden_display})
                }
                None => json!({"hidden_display": hidden_display}),
            });
        }
        if matches!(call, Call::Release) {
            if self.lease.as_ref().is_some_and(|l| l.session == session) {
                self.drop_lease();
            }
            return Ok(json!({}));
        }
        self.acquire(&session, share)?;
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

    fn device(&self) -> Result<Arc<dyn Device>, Error> {
        self.live
            .as_ref()
            .map(|l| l.device.clone())
            .ok_or_else(|| Error::DeviceGone {
                id: self.id.clone(),
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

    async fn dispatch(&mut self, s: &mut Session, call: Call) -> Result<Value, Error> {
        let value = match call {
            Call::Snapshot(p) => to_value(self.snapshot(s, p).await?),
            Call::Screenshot(p) => to_value(self.screenshot(s, p).await?),
            Call::Act(p) => to_value(self.act(s, p).await?),
            Call::Wait(p) => to_value(self.wait(s, p).await?),
            Call::Batch(p) => to_value(self.batch(s, p).await?),
            Call::Find(p) => {
                let now = self.fresh(s).await?;
                to_value(s.find(&now, &p.text, p.role))
            }
            Call::Apps => {
                self.need(Capabilities::APPS, Op::Apps, "this device cannot list apps")?;
                to_value(self.device()?.apps().await?)
            }
            Call::App(p) => self.app(s, p).await?,
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
            Call::Shell(p) => to_value(self.shell(s, p).await?),
            Call::Health => self.health().await?,
            Call::Release | Call::Leased => json!({}),
        };
        Ok(value)
    }

    /// Install, stop, clear data or uninstall. On a physical device only an app phonedriver installed (recorded in
    /// `installed.json` under the daemon's home) can be cleared or removed.
    async fn app(&mut self, s: &mut Session, p: AppParams) -> Result<Value, Error> {
        let device = self.device()?;
        if p.action == "install" {
            let path = p.path.ok_or(Error::Unsupported {
                op: Op::App,
                reason: "install needs the path of an app file",
            })?;
            let id = device.install(std::path::Path::new(&path)).await?;
            if let Some(id) = &id {
                self.ledger(|l| {
                    if !l.contains(&id.0) {
                        l.push(id.0.clone());
                    }
                });
            }
            let summary = match id {
                Some(id) => format!("installed {}", id.0),
                None => "installed (it replaced an app already there)".to_string(),
            };
            return Ok(json!({"ok": true, "summary": summary}));
        }
        let op = match p.action.as_str() {
            "stop" => AppOp::Stop,
            "clear" => AppOp::ClearData,
            "uninstall" => AppOp::Uninstall,
            _ => {
                return Err(Error::Unsupported {
                    op: Op::App,
                    reason: "the app action is install, stop, clear or uninstall",
                });
            }
        };
        let query = p.app.ok_or(Error::Unsupported {
            op: Op::App,
            reason: "this action needs the app's name or id",
        })?;
        let installed = device.apps().await?;
        let found = find_app(&installed, &query)?.clone();
        if op != AppOp::Stop && self.physical() {
            let mut mine = false;
            self.ledger(|l| mine = l.contains(&found.id.0));
            if !mine {
                return Err(Error::Unsupported {
                    op: Op::App,
                    reason: "phonedriver did not install this app on this physical device, so it will not clear or remove it",
                });
            }
        }
        device.app_op(op, &found.id).await?;
        if op == AppOp::Uninstall {
            self.ledger(|l| l.retain(|a| *a != found.id.0));
        }
        s.set_last(None);
        let word = match op {
            AppOp::Stop => "stopped",
            AppOp::ClearData => "cleared the data of",
            AppOp::Uninstall => "uninstalled",
        };
        Ok(json!({"ok": true, "summary": format!("{word} {}", found.name)}))
    }

    /// Runs a command in the device's shell, then (unless it only reads) settles and reports what changed on screen.
    async fn shell(&mut self, s: &mut Session, p: ShellParams) -> Result<ShellResult, Error> {
        self.need(
            Capabilities::SHELL,
            Op::Shell,
            "an iPhone or iPad has no shell for anyone to run commands in; use launch, open, type and the other verbs, or a Simulator",
        )?;
        if self.physical() && !p.force {
            let mut mine = Vec::new();
            self.ledger(|l| mine = l.clone());
            if let Some(reason) = shell::refuse_on_physical(&p.command, &mine) {
                return Err(Error::Unsupported {
                    op: Op::Shell,
                    reason,
                });
            }
        }
        let settle = p.settle && !shell::only_reads(&p.command);
        let mut cfg = Config::action();
        cfg.expect_change = Some(Duration::from_millis(p.expect_change_ms.unwrap_or(250)));
        if let Some(t) = p.timeout_ms {
            cfg.timeout = Duration::from_millis(t);
        }
        if settle && s.last.is_none() {
            let now = self.fresh(s).await?;
            s.set_last(Some(now));
        }
        let limit = Duration::from_secs(p.time_limit_s.unwrap_or(30));
        let device = self.device()?;
        let req = ShellRequest {
            command: p.command.clone(),
            timeout: limit,
        };
        let started = self.begin_action();
        let ran = Instant::now();
        let reply = tokio::time::timeout(limit + Duration::from_secs(5), device.shell(&req))
            .await
            .map_err(|_| Error::Timeout {
                waited_ms: (limit.as_millis() + 5000) as u32,
                condition: "the shell command to return".into(),
            })??;
        let ms = ran.elapsed().as_millis() as u32;
        let tidy = shell::tidy(
            &reply.stdout,
            &reply.stderr,
            p.grep.as_deref(),
            p.head.unwrap_or(shell::DEFAULT_HEAD),
        );
        let output = if p.secret {
            tidy.text.replace(p.command.as_str(), MASK)
        } else {
            tidy.text
        };
        let action = if settle {
            let summary = if p.secret {
                format!("ran a command ({} chars)", p.command.chars().count())
            } else {
                "ran".to_string()
            };
            Some(
                self.finish(s, true, cfg, started, summary, None, None, None)
                    .await?,
            )
        } else {
            if !p.settle {
                self.unobserved = Some(started);
            }
            None
        };
        Ok(ShellResult {
            exit: reply.exit,
            output,
            omitted: tidy.omitted,
            timed_out: reply.timed_out,
            ms,
            action,
        })
    }

    /// Everything but an emulator or a Simulator belongs to a person.
    fn physical(&self) -> bool {
        match self.id.split_once(':') {
            Some(("android", serial)) => !serial.starts_with("emulator-"),
            Some(("sim" | "fake", _)) => false,
            _ => true,
        }
    }

    /// Reads, lets `f` change, and writes back the apps phonedriver installed on this device.
    fn ledger(&self, f: impl FnOnce(&mut Vec<String>)) {
        let path = self.home.join("installed.json");
        let mut all: serde_json::Map<String, Value> = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let mut mine: Vec<String> = all
            .get(&self.id)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let before = mine.clone();
        f(&mut mine);
        if mine != before {
            all.insert(self.id.clone(), json!(mine));
            let _ = std::fs::create_dir_all(&self.home)
                .and_then(|()| std::fs::write(&path, Value::Object(all).to_string()));
        }
    }

    async fn health(&mut self) -> Result<Value, Error> {
        let device = self.device()?;
        device.tree().await?;
        let frames_ms = match device.frames() {
            None => None,
            Some(mut rx) => {
                rx.mark_unchanged();
                let started = Instant::now();
                match tokio::time::timeout(Duration::from_secs(1), rx.changed()).await {
                    Ok(Ok(())) => Some(started.elapsed().as_millis() as u64),
                    _ => None,
                }
            }
        };
        let info = device.info();
        Ok(
            json!({"id": info.id, "model": info.model, "os": info.os, "frames_ms": frames_ms, "hidden_display": device.hidden_display()}),
        )
    }

    /// With `marks`, boxes every element that takes a tap, a long press or text, labelled with the ref a tap on it
    /// would use. Screenshots are one pixel per point, so element bounds are pixel coordinates.
    async fn screenshot(
        &mut self,
        s: &mut Session,
        p: ScreenshotParams,
    ) -> Result<ScreenshotResult, Error> {
        let inline = p.inline;
        self.need(
            Capabilities::SCREENSHOT,
            Op::Screenshot,
            "this device has no screen capture",
        )?;
        let device = self.device()?;
        let now = if p.marks {
            Some(self.fresh(s).await?)
        } else {
            None
        };
        let mut image = match device.screenshot(Quality(100)).await {
            Err(Error::DeviceGone { .. }) => {
                let display = self.live.as_ref().map(|l| l.display);
                self.live = None;
                self.ensure_live(display.unwrap_or(DisplayMode::Main))
                    .await?;
                self.device()?.screenshot(Quality(100)).await?
            }
            shot => shot?,
        };
        let mut marked = None;
        if let Some(now) = now {
            let marks: Vec<phonedriver_video::marks::Mark> = now
                .elements
                .iter()
                .filter(|e| {
                    e.actions
                        .intersects(Actions::TAP | Actions::LONG_PRESS | Actions::EDIT)
                        && e.bounds.w > 0.0
                        && e.bounds.h > 0.0
                })
                .map(|e| phonedriver_video::marks::Mark {
                    label: e.r#ref.to_string(),
                    x: e.bounds.x,
                    y: e.bounds.y,
                    w: e.bounds.w,
                    h: e.bounds.h,
                })
                .collect();
            let png = image.bytes.clone();
            let drawn = tokio::task::spawn_blocking(move || {
                phonedriver_video::marks::draw_marks(&png, &marks).map(|b| (b, marks.len()))
            })
            .await
            .map_err(|e| Error::Transport {
                detail: format!("the marks task failed: {e}"),
            })?
            .map_err(|detail| Error::Transport {
                detail: format!("cannot draw marks: {detail}"),
            })?;
            image.bytes = drawn.0;
            marked = Some(drawn.1 as u32);
        }
        let dir = self
            .home
            .join("tmp")
            .join(self.id.replace([':', '/', '\\'], "_"));
        let path = dir.join(format!("{}.png", ulid::Ulid::generate()));
        std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::write(&path, &image.bytes))
            .map_err(|e| Error::Transport {
                detail: format!("cannot write {}: {e}", path.display()),
            })?;
        Ok(ScreenshotResult {
            path: path.display().to_string(),
            width: image.width,
            height: image.height,
            scale: 1.0,
            data_b64: inline
                .then(|| base64::engine::general_purpose::STANDARD.encode(&image.bytes)),
            marks: marked,
        })
    }

    fn build(&self, s: &mut Session, tree: &RawTree, settle: Option<SettleLine>) -> Rendered {
        *self.shown.lock().unwrap_or_else(|e| e.into_inner()) = Some(tree.clone());
        let (info, screen, secure_hidden) = match &self.live {
            Some(l) => (
                l.device.info().clone(),
                l.screen(),
                l.device.secure_content_hidden(tree),
            ),
            None => return Rendered::build(Vec::new(), &[], Header::default()),
        };
        let nodes = prune(tree);
        let refs = s.refs.assign(tree, &nodes);
        let alerts = phonedriver_core::alert::find(tree, screen);
        let app = tree
            .windows
            .iter()
            .filter(|w| !alerts.iter().any(|(id, _)| *id == w.id))
            .find(|w| w.kind == "application" && w.package.is_some())
            .and_then(|w| w.package.clone())
            .map(|id| {
                let name = self
                    .names
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(&id)
                    .cloned();
                AppLine { id, name }
            });
        let keyboard = tree
            .windows
            .iter()
            .find(|w| w.kind == "input_method")
            .map(|w| w.bounds);
        let header = Header {
            device: Some(DeviceLine {
                id: info.id,
                model: info.model,
                os: info.os,
                screen,
                orientation: if screen.w > screen.h {
                    Orientation::Landscape
                } else {
                    Orientation::Portrait
                },
            }),
            app,
            keyboard,
            settle,
            secure_hidden,
            alerts: alerts.into_iter().map(|(_, a)| a).collect(),
        };
        Rendered::build(nodes, &refs, header)
    }

    fn begin_action(&mut self) -> Duration {
        if let Some(l) = &mut self.live {
            l.frames.borrow_and_update();
            while l.events.try_recv().is_ok() {}
        }
        self.now()
    }

    async fn settle(&mut self, cfg: Config, started: Duration) -> State {
        self.settle_probing(cfg, started, false).await.0
    }

    /// With `probe`, reads the tree while the screen is quiet but the quiet period is not over, so a reply that needs
    /// the tree after the settle finds it already read. Two equal reads that both started after the last significant
    /// frame are what `stabilize` would have gathered after the settle.
    async fn settle_probing(
        &mut self,
        cfg: Config,
        started: Duration,
        probe: bool,
    ) -> (State, Option<Probe>) {
        let mut det = Detector::new(cfg, started);
        if let Some((t, input)) = self.recent
            && (cfg.expect_change.is_none() || t >= started)
        {
            det.feed(input, t);
        }
        let device = if probe { self.device().ok() } else { None };
        let mut reads: Vec<Read> = Vec::new();
        let mut inflight: Option<Inflight> = None;
        let mut probed_at = Duration::ZERO;
        loop {
            let st = det.poll(self.now());
            if matches!(st, State::Done { .. }) {
                let last = det.last_significant();
                if let (Some((at, read)), Some(last)) = (inflight.take(), last)
                    && at >= last + READ_LAG
                    && let Ok(tree) = read.await
                {
                    reads.push(Read { at, tree });
                }
                let settled = matches!(
                    st,
                    State::Done {
                        settled: true,
                        changed: true,
                        ..
                    }
                );
                let probed = last.filter(|_| settled).and_then(|l| probe_of(reads, l));
                return (st, probed);
            }
            if let (Some(device), None) = (&device, &inflight) {
                let now = self.now();
                let wanted = match det.last_significant() {
                    Some(last) => {
                        let due = last + READ_LAG;
                        now >= due && reads.iter().filter(|r| r.at >= due).count() < 2
                    }
                    None => {
                        cfg.expect_change
                            .is_some_and(|w| w >= FIRST_READ_MIN_WINDOW)
                            && now >= started + FIRST_READ
                            && now >= probed_at + FIRST_READ
                    }
                };
                if wanted {
                    let device = device.clone();
                    inflight = Some((now, Box::pin(async move { device.tree().await })));
                }
            }
            tokio::select! {
                sig = next_signal(&mut self.live) => {
                    let at = self.now();
                    if let Some(input) = self.absorb(sig, at) {
                        det.feed(input, at);
                    }
                }
                (at, read) = finished(&mut inflight) => {
                    if let Ok(tree) = read {
                        if det.last_significant().is_none() {
                            probed_at = self.now();
                            // A first call (a launch right after connecting) has no screen to compare against, and
                            // a polled device has no frames, so the read itself is the change.
                            if self.shown().is_none_or(|shown| shown != tree) {
                                det.feed(Input::WindowChanged, probed_at);
                            }
                        } else {
                            reads.push(Read { at, tree });
                        }
                    }
                }
                () = tokio::time::sleep(TICK) => {}
            }
        }
    }

    async fn snapshot(&mut self, s: &mut Session, p: SnapshotParams) -> Result<Observation, Error> {
        let device = self.device()?;
        let settle = if p.no_settle {
            None
        } else {
            let mut cfg = Config::snapshot();
            if let Some(t) = p.timeout_ms {
                cfg.timeout = Duration::from_millis(t);
            }
            let (cfg, started) = self.observe(cfg, None);
            let t = Instant::now();
            let st = self.settle(cfg, started).await;
            self.lap("settle", t);
            Some(st)
        };
        let t = Instant::now();
        let mut tree = device.tree().await?;
        // Run a7: a snapshot right after `shell am start` read the screen between two activities and printed a
        // header with no lines.
        let empty_since = Instant::now();
        while settle.is_some()
            && !tree
                .nodes
                .iter()
                .any(|n| n.label.is_some() || !n.actions.is_empty())
            && empty_since.elapsed() < CONTENT_WAIT
        {
            tokio::time::sleep(TREE_STEP).await;
            tree = device.tree().await?;
        }
        self.lap("tree", t);
        let t = Instant::now();
        let line = settle.map(settle_line);
        let mut rendered = self.build(s, &tree, line);
        self.lap("render", t);
        s.ocr = false;
        if p.pixels || tree_is_hollow(&rendered) {
            match self.merge_pixels(s, &mut rendered).await {
                Ok(()) => s.ocr = true,
                Err(e) if p.pixels => return Err(e),
                Err(_) => {}
            }
        }
        self.read_checkboxes(&mut rendered, s.last.as_ref()).await?;
        let filters = Filters {
            interactive: p.interactive,
            within: p.within,
            max_lines: p.max_lines,
        };
        let text = if p.diff && s.last.is_some() {
            match s.diff_for_reply(&rendered).ok_or_else(unreachable_state)? {
                DiffOrFull::Diff(changes) => {
                    (Kind::Diff, DiffOrFull::Diff(changes).text(&rendered))
                }
                DiffOrFull::Full => (
                    Kind::Full,
                    rendered.text(&filters).map_err(|r| stale(r, s))?,
                ),
            }
        } else {
            (
                Kind::Full,
                rendered.text(&filters).map_err(|r| stale(r, s))?,
            )
        };
        let capture = if p.capture {
            device.capture_tree().await?
        } else {
            None
        };
        let (settled, ms, still) = match settle {
            Some(State::Done {
                settled,
                elapsed,
                still_changing,
                ..
            }) => (settled, elapsed.as_millis() as u32, still_changing),
            _ => (false, 0, None),
        };
        let obs = Observation {
            header: rendered.header.clone(),
            text: text.1,
            kind: text.0,
            changed: true,
            settled,
            settle_ms: ms,
            still_changing: still,
            loading: rendered.loading(),
            elements: p.elements.then(|| rendered.elements.clone()),
            capture,
        };
        s.set_last(Some(rendered));
        Ok(obs)
    }

    /// Fetches the screen as it is now for resolving a target. It does not replace what the agent was last shown.
    async fn fresh(&mut self, s: &mut Session) -> Result<Rendered, Error> {
        let tree = self.device()?.tree().await?;
        let mut rendered = self.build(s, &tree, None);
        if s.ocr {
            self.merge_pixels(s, &mut rendered).await?;
        }
        self.read_checkboxes(&mut rendered, s.last.as_ref()).await?;
        Ok(rendered)
    }

    /// A web checkbox or radio repaints after the screen has looked settled (up to 3 s on a page that has just loaded in the
    /// emulator), so a read right away still shows the old state and the reply says nothing changed.
    async fn await_toggle(&mut self, s: &mut Session, r: Ref, was: bool) -> Result<(), Error> {
        let started = Instant::now();
        while started.elapsed() < TOGGLE_WAIT {
            tokio::time::sleep(TREE_STEP).await;
            let now = self.fresh(s).await?;
            let state = now.index_of(r).map(|i| now.elements[i].state);
            if state.is_none_or(|st| st.contains(ElementState::CHECKED) != was) {
                break;
            }
        }
        Ok(())
    }

    /// A web checkbox or radio says neither checkable nor checked in the tree, so its state is read from the screenshot.
    async fn read_checkboxes(
        &self,
        rendered: &mut Rendered,
        prev: Option<&Rendered>,
    ) -> Result<(), Error> {
        let screen = self.screen_rect();
        let keyboard = rendered.header.keyboard;
        let unread: Vec<usize> = rendered
            .elements
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                matches!(e.role, Role::Checkbox | Role::Radio)
                    && !e.state.contains(ElementState::CHECKABLE)
                    && !checkbox::readable(e.bounds, screen, keyboard)
                    && rendered.in_web_view(e.r#ref)
            })
            .map(|(i, _)| i)
            .collect();
        for i in unread {
            let known = prev
                .and_then(|p| {
                    p.index_of(rendered.elements[i].r#ref)
                        .map(|j| p.elements[j].state)
                })
                .filter(|st| st.contains(ElementState::CHECKABLE));
            if let Some(st) = known {
                let state = &mut rendered.elements[i].state;
                state.insert(ElementState::CHECKABLE);
                state.set(ElementState::CHECKED, st.contains(ElementState::CHECKED));
            }
        }
        let boxes: Vec<usize> = rendered
            .elements
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                matches!(e.role, Role::Checkbox | Role::Radio)
                    && !e.state.contains(ElementState::CHECKABLE)
                    && e.bounds.w > 0.0
                    && e.bounds.h > 0.0
                    && checkbox::readable(e.bounds, screen, keyboard)
                    && rendered.in_web_view(e.r#ref)
            })
            .map(|(i, _)| i)
            .take(MAX_CHECKBOXES)
            .collect();
        let device = self.device()?;
        if boxes.is_empty()
            || !device
                .info()
                .capabilities
                .contains(Capabilities::SCREENSHOT)
        {
            return Ok(());
        }
        let image = device.screenshot(Quality(100)).await?;
        let rects: Vec<Rect> = boxes.iter().map(|&i| rendered.elements[i].bounds).collect();
        let ticked = tokio::task::spawn_blocking(move || checkbox::ticked(&image.bytes, &rects))
            .await
            .map_err(|e| Error::Transport {
                detail: format!("checkbox task failed: {e}"),
            })??;
        for (&i, on) in boxes.iter().zip(ticked) {
            let state = &mut rendered.elements[i].state;
            state.insert(ElementState::CHECKABLE);
            state.set(ElementState::CHECKED, on);
        }
        Ok(())
    }

    /// Adds the text a screenshot shows and the tree does not. A box keeps the ref of the same text in the last
    /// snapshot, so a ref the agent was shown still names it.
    async fn merge_pixels(
        &mut self,
        s: &mut Session,
        rendered: &mut Rendered,
    ) -> Result<(), Error> {
        let Some(screen) = rendered.header.device.as_ref().map(|d| d.screen) else {
            return Ok(());
        };
        let image = self.device()?.screenshot(Quality(100)).await?;
        let boxes =
            tokio::task::spawn_blocking(move || crate::ocr::recognize(&image.bytes, screen))
                .await
                .map_err(|e| Error::Transport {
                    detail: format!("OCR task failed: {e}"),
                })??;
        let mut taken = Vec::new();
        let last: Vec<(Ref, String)> = s
            .last
            .iter()
            .flat_map(|l| l.elements.iter())
            .filter(|e| e.source == Source::Ocr)
            .map(|e| (e.r#ref, e.label.clone().unwrap_or_default()))
            .collect();
        merge(rendered, boxes, |b| {
            match last
                .iter()
                .find(|(r, text)| *text == b.text && !taken.contains(r))
            {
                Some((r, _)) => {
                    taken.push(*r);
                    *r
                }
                None => s.refs.fresh(),
            }
        });
        Ok(())
    }

    /// Where a target is now. Only a ref or text target needs the screen, so a point costs no tree read.
    async fn resolve_target(
        &mut self,
        s: &mut Session,
        target: &Target,
    ) -> Result<Resolved, Error> {
        if let Target::Point { point } = target {
            let screen = self.screen_rect();
            let inside = (screen.x..=screen.x + screen.w).contains(&point.x)
                && (screen.y..=screen.y + screen.h).contains(&point.y);
            if !inside {
                return Err(Error::PointOffScreen {
                    x: point.x,
                    y: point.y,
                    w: screen.w.round(),
                    h: screen.h.round(),
                });
            }
            return Ok(Resolved {
                at: Point {
                    x: point.x,
                    y: point.y,
                },
                element: None,
                line: None,
                under_keyboard: false,
                covered: None,
            });
        }
        let now = self.fresh(s).await?;
        s.resolve(target, &now)
    }

    /// Wakes a dark screen, swipes a swipe-only lock screen away, and refuses a PIN lock (architecture 6.3).
    async fn prepare(&mut self, action: &Action) -> Result<(bool, bool), Error> {
        if matches!(action, Action::Press { key } if key == "lock") {
            self.awake = None;
            return Ok((false, false));
        }
        if self
            .awake
            .is_some_and(|at| self.now().saturating_sub(at) < AWAKE_REUSE)
        {
            return Ok((false, false));
        }
        let device = self.device()?;
        let mut state = device.screen_state().await?;
        let (mut woke, mut unlocked) = (false, false);
        if !state.on {
            device.wake().await?;
            woke = true;
            state = self.until_screen(true, "the screen to turn on").await?;
            // Observed on API 26: the keyguard reports locked a few hundred ms after the screen reports on.
            let started = Instant::now();
            while !state.locked && started.elapsed() < KEYGUARD_WAIT {
                tokio::time::sleep(Duration::from_millis(100)).await;
                state = device.screen_state().await?;
            }
        }
        if state.locked {
            if state.secure {
                return Err(Error::NeedsUser {
                    action: UserAction::UnlockDevice,
                });
            }
            let screen = self.screen_rect();
            // A drag that starts on a notification card (the middle of the screen on API 26) does not unlock.
            let start = Point {
                x: screen.w / 2.0,
                y: screen.h * 0.9,
            };
            device
                .gesture(gesture::swipe_from(start, screen, Direction::Up, 0.6, true))
                .await?;
            self.until_screen(false, "the lock screen to go away")
                .await?;
            unlocked = true;
        }
        if woke || unlocked {
            self.until_app_content(None).await?;
        }
        self.awake = Some(self.now());
        Ok((woke, unlocked))
    }

    async fn until_app_content(&self, leaving: Option<&AppId>) -> Result<(), Error> {
        let device = self.device()?;
        // Observed on API 26 and 37: input sent as soon as the screen is on and unlocked is rejected
        // by the dispatcher until a window has input focus again, and on API 26 the tree lists only
        // system windows, then part of the app, until the app's window is back. A cold-started Settings
        // on API 37 likewise reports an empty tree for a second after its first frame.
        let started = Instant::now();
        let mut last_len = 0;
        let mut equal_reads = 0;
        // Observed on the iOS Simulator: Settings adds a Toolbar group 300 ms to 1 s after its first frame, which
        // made the first refs stale at once, so a polled device must read the same count three times in a row. A
        // device with frames settles on them after this wait and reads again until two reads agree, so one read that
        // shows the app is enough here; a second matching read cost about 200 ms per launch on the emulator.
        let needed = if device.info().capabilities.contains(Capabilities::FRAMES) {
            0
        } else {
            3
        };
        while started.elapsed() < SCREEN_WAIT {
            let tree = device.tree().await?;
            equal_reads = if tree.nodes.len() == last_len {
                equal_reads + 1
            } else {
                0
            };
            let stable = equal_reads >= needed;
            let page_pending = tree
                .nodes
                .iter()
                .enumerate()
                .any(|(i, n)| n.role == Role::WebView && tree.children(i).next().is_none());
            last_len = tree.nodes.len();
            let app_is_back = tree.nodes.iter().any(|n| {
                tree.windows.iter().any(|w| {
                    w.id == n.window
                        && w.kind == "application"
                        && (started.elapsed() > LEAVE_WAIT
                            || leaving.is_none_or(|l| w.package.as_deref() != Some(l.0.as_str())))
                })
            });
            if app_is_back && stable && !page_pending && device.screen_state().await?.focused {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }

    async fn until_screen(&self, wait_for_on: bool, what: &str) -> Result<ScreenState, Error> {
        let device = self.device()?;
        let started = Instant::now();
        loop {
            let state = device.screen_state().await?;
            let done = if wait_for_on { state.on } else { !state.locked };
            if done {
                return Ok(state);
            }
            if started.elapsed() > SCREEN_WAIT {
                return Err(Error::Timeout {
                    waited_ms: started.elapsed().as_millis() as u32,
                    condition: what.to_string(),
                });
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// A tap on a button that spends money or cannot be undone needs the human's word (decision D27). A point is
    /// matched to the element under it on a fresh read.
    async fn guard_tap(
        &mut self,
        s: &mut Session,
        approval: Option<Approval>,
        ask: &[Category],
        target: &Target,
        found: Option<&Element>,
        at: Point,
    ) -> Result<Option<&'static str>, Error> {
        if approval == Some(Approval::Autonomous) {
            return Ok(None);
        }
        let button = match (found, target) {
            (Some(e), _) => Some(e.clone()),
            (None, Target::Point { .. }) => {
                let now = self.screen_for_guard(s).await?;
                guard::hit(&now, at).cloned()
            }
            _ => None,
        };
        self.guard_button(s, approval, ask, button).await
    }

    async fn screen_for_guard(&mut self, s: &mut Session) -> Result<Rendered, Error> {
        if let Some((read, view)) = &self.guard_view
            && self.now().saturating_sub(*read) < GUARD_VIEW_REUSE
            && self.reshaped.is_none_or(|t| t <= *read)
        {
            return Ok(view.clone());
        }
        let view = self.fresh(s).await?;
        self.guard_view = Some((self.now(), view.clone()));
        Ok(view)
    }

    /// `type --submit` presses enter, which sends the form: guarded when the field's own form holds such a button.
    async fn guard_submit(
        &mut self,
        s: &mut Session,
        approval: Option<Approval>,
        ask: &[Category],
        field: Option<&Element>,
    ) -> Result<Option<&'static str>, Error> {
        let button = field
            .zip(s.last.as_ref())
            .and_then(|(f, now)| guard::submit_button(now, f, ask))
            .cloned();
        self.guard_button(s, approval, ask, button).await
    }

    async fn guard_button(
        &mut self,
        s: &mut Session,
        approval: Option<Approval>,
        ask: &[Category],
        button: Option<Element>,
    ) -> Result<Option<&'static str>, Error> {
        let Some((button, category)) = button.and_then(|b| guard::guarded(&b, ask).map(|c| (b, c)))
        else {
            return Ok(None);
        };
        match guard::stance(s, approval, category) {
            guard::Stance::Run(note) => Ok(note),
            guard::Stance::Ask => {
                let now = self.fresh(s).await?;
                let details = guard::context(&now, &button);
                Err(Error::NeedsApproval {
                    label: button.label.or(button.value).unwrap_or_default(),
                    category,
                    app: guard::app_name(&now),
                    details,
                })
            }
        }
    }

    async fn act(&mut self, s: &mut Session, p: ActParams) -> Result<ActionResult, Error> {
        let t = Instant::now();
        let prepared = self.prepare(&p.action).await;
        self.lap("prepare", t);
        let (woke, unlocked) = prepared?;
        if !matches!(
            &p.action,
            Action::Tap {
                target: Target::Point { .. },
                ..
            }
        ) {
            self.guard_view = None;
        }
        let acted = self.act_now(s, p).await;
        if acted.is_err() {
            self.awake = None;
        }
        let mut result = acted?;
        result.woke = woke;
        result.unlocked = unlocked;
        Ok(result)
    }

    async fn act_now(&mut self, s: &mut Session, p: ActParams) -> Result<ActionResult, Error> {
        let mut cfg = Config::action();
        if let Some(t) = p.timeout_ms {
            cfg.timeout = Duration::from_millis(t);
        }
        if let Some(t) = p.expect_change_ms {
            cfg.expect_change = Some(Duration::from_millis(t));
        } else if self.cold_until.is_some_and(|t| self.now() < t) {
            cfg.expect_change = Some(COLD_CHANGE_WAIT);
        }
        let device = self.device()?;
        match p.action {
            Action::Tap {
                target,
                count,
                long,
            } => {
                self.need(
                    Capabilities::GESTURE,
                    Op::Gesture,
                    "this device cannot inject touches",
                )?;
                let Resolved {
                    at,
                    line: element,
                    element: found,
                    under_keyboard,
                    covered,
                } = {
                    let t = Instant::now();
                    let r = self.resolve_target(s, &target).await;
                    self.lap("resolve", t);
                    r?
                };
                // Calendar's "Add" on a Simulator without calendar access: the tap waited 2 s for nothing.
                if let Some(e) = found.as_ref().filter(|e| {
                    !e.state.contains(ElementState::ENABLED)
                        && !matches!(target, Target::Point { .. })
                }) {
                    return Err(Error::Disabled { r#ref: e.r#ref });
                }
                let mut hid = false;
                let (at, element, found, covered) = match (under_keyboard, &found) {
                    (true, Some(e)) => {
                        let behind = Error::OffScreen {
                            r#ref: e.r#ref,
                            scroll_hint: Direction::Down,
                            behind_keyboard: true,
                            covered_by: None,
                        };
                        if !p.settle {
                            return Err(behind);
                        }
                        // Agents answered "under the keyboard" with a hide-keyboard call and the same tap, two calls
                        // for one intent (iOS Contacts, run i2c). The tap does both and finds the element again,
                        // since the form moves when the keyboard goes.
                        let wanted = e.r#ref;
                        let started = self.begin_action();
                        device.hide_keyboard().await?;
                        self.settle_probing(cfg, started, false).await;
                        let now = self.fresh(s).await?;
                        let r = s.resolve(&Target::Ref { r#ref: wanted }, &now)?;
                        if r.under_keyboard
                            || now.header.keyboard.is_some()
                                && r.at.y >= now.header.keyboard.map_or(f32::MAX, |k| k.y)
                        {
                            return Err(behind);
                        }
                        hid = true;
                        (r.at, r.line, r.element, r.covered)
                    }
                    _ => (at, element, found, covered),
                };
                if let (Some((bar, scroll_hint)), Some(e)) = (covered, &found) {
                    return Err(Error::OffScreen {
                        r#ref: e.r#ref,
                        scroll_hint,
                        behind_keyboard: false,
                        covered_by: Some(bar),
                    });
                }
                let approved = self
                    .guard_tap(s, p.approval, &p.ask, &target, found.as_ref(), at)
                    .await?;
                let started = self.begin_action();
                let t = Instant::now();
                let sent = device
                    .gesture(gesture::tap(at, count.unwrap_or(1), long))
                    .await;
                self.lap("input", t);
                sent?;
                let verb = match (count, long) {
                    (_, Some(ms)) => format!("long-pressed {ms} ms"),
                    (Some(n), None) if n > 1 => format!("tapped {n} times"),
                    _ => "tapped".to_string(),
                };
                let summary = match &element {
                    Some(line) => format!(
                        "{verb} {} at {},{}",
                        short(line),
                        at.x.round(),
                        at.y.round()
                    ),
                    None => format!("{verb} at {},{}", at.x.round(), at.y.round()),
                };
                let summary = if hid {
                    format!("hid the keyboard, then {summary}")
                } else {
                    summary
                };
                let summary = approved_summary(summary, approved);
                if let Some(e) = found.as_ref().filter(|e| {
                    let can_change = match e.role {
                        Role::Checkbox => true,
                        Role::Radio => !e.state.contains(ElementState::CHECKED),
                        _ => false,
                    };
                    can_change && s.last.as_ref().is_some_and(|l| l.in_web_view(e.r#ref))
                }) {
                    self.await_toggle(s, e.r#ref, e.state.contains(ElementState::CHECKED))
                        .await?;
                }
                let r = self
                    .finish(s, p.settle, cfg, started, summary.clone(), None, None, None)
                    .await?;
                let ripple_only = r.settled
                    && r.changed
                    && r.observation.kind == Kind::Diff
                    && r.observation.text.is_empty();
                if !p.settle || !ripple_only {
                    return Ok(r);
                }
                // MagicOS Settings draws the button's ripple, goes quiet for longer than the quiet window, and only
                // then starts the next page (Honor X8: "settled in 331ms · no element changed", then the page).
                let again = self.now();
                let late = Config {
                    expect_change: Some(LATE_PAGE_WAIT),
                    ..cfg
                };
                let (st, probe) = self.settle_probing(late, again, true).await;
                match st {
                    State::Done {
                        settled,
                        changed: true,
                        still_changing,
                        elapsed,
                    } => {
                        let st = State::Done {
                            settled,
                            changed: true,
                            still_changing,
                            elapsed: elapsed + Duration::from_millis(r.settle_ms.into()),
                        };
                        self.report(s, st, probe, summary, None, None, None).await
                    }
                    _ => Ok(r),
                }
            }
            Action::Swipe {
                direction,
                from,
                distance,
                fast,
            } => {
                self.need(
                    Capabilities::GESTURE,
                    Op::Gesture,
                    "this device cannot inject touches",
                )?;
                let screen = self.screen_rect();
                let (start, line) = match &from {
                    Some(t) => {
                        let r = self.resolve_target(s, t).await?;
                        (Some(r.at), r.line)
                    }
                    None => (None, None),
                };
                let distance = distance.unwrap_or(0.5);
                let g = match start {
                    Some(at) => gesture::swipe_from(at, screen, direction, distance, fast),
                    None => gesture::swipe(screen, direction, distance, fast),
                };
                let started = self.begin_action();
                device.gesture(g).await?;
                let from_text = line
                    .map(|l| format!(" from {}", short(&l)))
                    .unwrap_or_default();
                let summary = format!("swiped {}{from_text}", direction.name());
                self.finish(s, p.settle, cfg, started, summary, None, None, None)
                    .await
            }
            Action::Drag { from, to } => {
                self.need(
                    Capabilities::GESTURE,
                    Op::Gesture,
                    "this device cannot inject touches",
                )?;
                let a = self.resolve_target(s, &from).await?;
                let b = self.resolve_target(s, &to).await?;
                let started = self.begin_action();
                device.gesture(gesture::press_drag(a.at, b.at)).await?;
                let label = |r: &Resolved| {
                    r.line.as_deref().map_or_else(
                        || format!("{},{}", r.at.x.round(), r.at.y.round()),
                        |l| short(l).to_string(),
                    )
                };
                let summary = format!("dragged {} to {}", label(&a), label(&b));
                self.finish(s, p.settle, cfg, started, summary, None, None, None)
                    .await
            }
            Action::Pinch {
                direction,
                at,
                distance,
            } => {
                self.need(
                    Capabilities::MULTI_TOUCH,
                    Op::Gesture,
                    "this device cannot inject two-finger touches",
                )?;
                let screen = self.screen_rect();
                let (center, line) = match &at {
                    Some(t) => {
                        let r = self.resolve_target(s, t).await?;
                        (r.at, r.line)
                    }
                    None => (screen.center(), None),
                };
                let out = direction == PinchDirection::Out;
                let started = self.begin_action();
                device
                    .gesture(gesture::pinch(center, screen, out, distance.unwrap_or(0.5)))
                    .await?;
                let on = line
                    .map(|l| format!(" on {}", short(&l)))
                    .unwrap_or_default();
                let summary = format!("pinched {}{on}", if out { "out" } else { "in" });
                self.finish(s, p.settle, cfg, started, summary, None, None, None)
                    .await
            }
            Action::Gesture { fingers } => {
                self.need(
                    Capabilities::GESTURE,
                    Op::Gesture,
                    "this device cannot inject touches",
                )?;
                let g = Gesture {
                    fingers: fingers
                        .iter()
                        .map(|f| {
                            f.iter()
                                .map(|p| TimedPoint {
                                    at: Point { x: p.x, y: p.y },
                                    t_ms: p.t_ms,
                                })
                                .collect()
                        })
                        .collect(),
                };
                let started = self.begin_action();
                device.gesture(g).await?;
                let summary = format!("gestured with {} fingers", fingers.len());
                self.finish(s, p.settle, cfg, started, summary, None, None, None)
                    .await
            }
            Action::Launch {
                app,
                grant,
                restart,
            } => {
                self.need(
                    Capabilities::LAUNCH,
                    Op::Launch,
                    "this device cannot launch apps",
                )?;
                let t = Instant::now();
                let installed = device.apps().await?;
                self.lap("apps", t);
                let found = find_app(&installed, &app)?;
                if !grant.is_empty() {
                    if self.physical() {
                        let mut mine = false;
                        self.ledger(|l| mine = l.contains(&found.id.0));
                        if !mine {
                            return Err(Error::Unsupported {
                                op: Op::Launch,
                                reason: "phonedriver did not install this app on this physical device, so it will not grant it permissions",
                            });
                        }
                    }
                    device.grant(&found.id, &grant).await?;
                }
                cfg.expect_change = p
                    .expect_change_ms
                    .map(Duration::from_millis)
                    .or(Config::launch().expect_change);
                let t = Instant::now();
                let leaving = device.foreground().await?.filter(|f| *f != found.id);
                self.lap("foreground", t);
                let started = self.begin_action();
                if restart {
                    let t = Instant::now();
                    // `simctl terminate` exits 3 ("found nothing to terminate") when the app is not running, and
                    // such an app opens on its first screen anyway.
                    let _ = device.app_op(AppOp::Stop, &found.id).await;
                    self.lap("stop", t);
                }
                let t = Instant::now();
                device.launch(&found.id).await?;
                self.lap("launch", t);
                let t = Instant::now();
                self.until_app_content(leaving.as_ref()).await?;
                self.lap("app_content", t);
                self.cold_until = Some(self.now() + COLD_APP_WINDOW);
                s.set_last(None);
                let verb = if restart { "restarted" } else { "launched" };
                let mut summary = if grant.is_empty() {
                    format!("{verb} {}", found.name)
                } else {
                    format!("{verb} {} (granted {})", found.name, grant.join(", "))
                };
                // Observed on the iOS 26.5 Simulator: Photos crashed at every launch, the home screen stayed, and the
                // agent read "launched Photos" as a launch that worked and tapped the icon again and again.
                if let Some(front) = device.foreground().await?.filter(|f| *f != found.id) {
                    summary = format!(
                        "{summary}, but {} is not in front ({} is): it may have quit at start",
                        found.name, front.0
                    );
                }
                self.finish(s, p.settle, cfg, started, summary, None, None, None)
                    .await
            }
            Action::Open { url, app } => {
                self.need(
                    Capabilities::OPEN_URL,
                    Op::OpenUrl,
                    "this device cannot open links",
                )?;
                cfg.expect_change = p
                    .expect_change_ms
                    .map(Duration::from_millis)
                    .or(Config::launch().expect_change);
                let target = match &app {
                    Some(name) => Some(find_app(&device.apps().await?, name)?.id.clone()),
                    None => None,
                };
                let leaving = device.foreground().await?;
                let started = self.begin_action();
                let t = Instant::now();
                let handler = device.open_url(&Url(url.clone()), target.as_ref()).await?;
                self.lap("open_url", t);
                let leaving = leaving.filter(|l| handler.as_ref() != Some(l));
                let t = Instant::now();
                self.until_app_content(leaving.as_ref()).await?;
                self.lap("app_content", t);
                let waited = Instant::now();
                while waited.elapsed() < PAGE_WAIT && device.tree().await?.web_view_pending() {
                    tokio::time::sleep(TREE_STEP).await;
                }
                self.lap("page", waited);
                self.cold_until = Some(self.now() + COLD_APP_WINDOW);
                let summary = format!("opened {url}");
                self.finish(s, p.settle, cfg, started, summary, None, None, None)
                    .await
            }
            Action::Scroll {
                direction,
                within,
                until,
                max,
            } => {
                self.need(
                    Capabilities::GESTURE,
                    Op::Gesture,
                    "this device cannot inject touches",
                )?;
                self.scroll(s, p.settle, cfg, direction, within, until, max)
                    .await
            }
            Action::Press { key } if key == "hide-keyboard" => {
                let shown = device
                    .tree()
                    .await?
                    .windows
                    .iter()
                    .any(|w| w.kind == "input_method");
                if !shown {
                    let st = State::Done {
                        settled: true,
                        changed: false,
                        still_changing: None,
                        elapsed: Duration::ZERO,
                    };
                    return self
                        .report(
                            s,
                            st,
                            None,
                            "keyboard already hidden".into(),
                            None,
                            None,
                            None,
                        )
                        .await;
                }
                let started = self.begin_action();
                let via = device.hide_keyboard().await?;
                let summary = format!("hid the keyboard via {}", via.unwrap_or("back"));
                let result = self
                    .finish(s, p.settle, cfg, started, summary, via, None, None)
                    .await?;
                if p.settle && result.observation.header.keyboard.is_some() {
                    return Err(Error::Unsupported {
                        op: Op::Key,
                        reason: "this screen kept the keyboard when it was told to hide",
                    });
                }
                Ok(result)
            }
            Action::Press { key } => {
                let started = self.begin_action();
                let via = press(&*device, &key).await?;
                let summary = match via {
                    Some(how) => format!("pressed {key} via {how}"),
                    None => format!("pressed {key}"),
                };
                self.finish(s, p.settle, cfg, started, summary, via, None, None)
                    .await
            }
            Action::Clear { target } => {
                self.need(
                    Capabilities::TEXT_SET,
                    Op::Text,
                    "this device cannot set field values",
                )?;
                let r = self.resolve_target(s, &target).await?;
                let secure = r.element.as_ref().is_some_and(is_secure);
                let started = self.begin_action();
                device.gesture(gesture::tap(r.at, 1, None)).await?;
                device
                    .text(TextInput::SetValue(String::new()))
                    .await
                    .map_err(|e| withhold(e, secure))?;
                let label = r
                    .line
                    .as_deref()
                    .map(|l| format!(" {}", short(l)))
                    .unwrap_or_default();
                let verify = Verify {
                    field: r.element.map(|e| e.r#ref),
                    expect: Expect::Exactly(String::new()),
                    secure,
                    mode: Some("set"),
                    submitted: false,
                };
                self.finish(
                    s,
                    p.settle,
                    cfg,
                    started,
                    format!("cleared{label}"),
                    Some("set"),
                    Some(verify),
                    None,
                )
                .await
            }
            Action::Type {
                text,
                into,
                mode,
                secret,
                submit,
                append,
            } => {
                let chosen = mode.is_some();
                let mut brought = String::new();
                let (field, tap_at): (Option<Element>, Option<Point>) = match &into {
                    Some(t) => {
                        let r = match self.resolve_target(s, t).await {
                            Err(missing @ (Error::StaleRef { .. } | Error::NotFound { .. })) => {
                                let gone = match (&missing, t) {
                                    (Error::StaleRef { r#ref, .. }, _) => s.gone_field(*r#ref),
                                    (_, Target::Text { text, .. }) => s.gone_field_named(text),
                                    _ => None,
                                };
                                let Some(gone) = gone.cloned() else {
                                    return Err(missing);
                                };
                                let (r, said) = self.bring_back(s, cfg, &gone, missing).await?;
                                brought = said;
                                r
                            }
                            r => r?,
                        };
                        let inner = r
                            .element
                            .as_ref()
                            .filter(|e| !e.role.is_text_entry())
                            .and_then(|e| {
                                s.last
                                    .as_ref()
                                    .and_then(|now| text_entry_within(now, e.r#ref))
                            });
                        match inner {
                            Some(f) => {
                                let at = f.bounds.center();
                                (Some(f), Some(at))
                            }
                            None => (r.element, Some(r.at)),
                        }
                    }
                    None => {
                        let focused = |r: &Rendered| {
                            r.elements
                                .iter()
                                .find(|e| {
                                    e.state.contains(ElementState::FOCUSED)
                                        && e.role.is_text_entry()
                                })
                                .cloned()
                        };
                        let known = s.last.as_ref().and_then(focused);
                        if known.is_some() {
                            (known, None)
                        } else {
                            // Chrome, a tap on Wikipedia's search icon while the page loaded: no field took focus,
                            // and the text was pasted into nothing and reported as typed.
                            let now = self.fresh(s).await?;
                            if now.header.keyboard.is_none() && focused(&now).is_none() {
                                return Err(Error::NotFocused {
                                    fields: now
                                        .elements
                                        .iter()
                                        .filter(|e| e.role.is_text_entry())
                                        .take(NOT_FOCUSED_FIELDS)
                                        .map(|e| now.line(e).trim_start().to_string())
                                        .collect(),
                                });
                            }
                            (focused(&now), None)
                        }
                    }
                };
                // A paste in hidden mode still raises the system's clipboard toast on the phone's own screen ("Settings
                // is accessing the clipboard", HONOR X8, Android 13), so a native field there is set instead. A web
                // field keeps paste: set on a WebView falls back to select-all and paste over what it holds.
                let hidden = self
                    .live
                    .as_ref()
                    .is_some_and(|l| l.display == DisplayMode::Hidden);
                let native = field.as_ref().is_some_and(|f| {
                    f.actions.contains(Actions::EDIT)
                        && !s.last.as_ref().is_some_and(|l| l.in_web_view(f.r#ref))
                });
                let can_set = device.info().capabilities.contains(Capabilities::TEXT_SET);
                let masked = secret || field.as_ref().is_some_and(is_secure);
                let whole = into.is_some() && !append;
                let mode = mode.unwrap_or(if hidden && native && can_set && (whole || !masked) {
                    TypeMode::Set
                } else {
                    TypeMode::Paste
                });
                let (cap, reason) = match mode {
                    TypeMode::Paste => (Capabilities::TEXT_PASTE, "this device cannot paste"),
                    TypeMode::Set => (
                        Capabilities::TEXT_SET,
                        "this device cannot set field values",
                    ),
                    TypeMode::Keys => (Capabilities::TEXT_KEYS, "this device cannot type keys"),
                };
                self.need(cap, Op::Text, reason)?;
                if !secret
                    && !text.is_empty()
                    && let Some((f, kind)) = field
                        .as_ref()
                        .and_then(|f| secret_field::kind(f).map(|k| (f, k)))
                {
                    return Err(Error::SecretField {
                        r#ref: f.r#ref,
                        kind,
                    });
                }
                let approved = if submit {
                    self.guard_submit(s, p.approval, &p.ask, field.as_ref())
                        .await?
                } else {
                    None
                };
                let secure = secret || field.as_ref().is_some_and(is_secure);
                let replace = into.is_some() && !append;
                let mut held = field
                    .as_ref()
                    .and_then(|f| f.value.clone())
                    .filter(|v| !v.is_empty());
                let mut wipe = replace && held.is_some() && mode != TypeMode::Set;
                let started = self.begin_action();
                let mut field = field;
                let mut zoomed = false;
                if let Some(at) = tap_at {
                    let at = match (&field, wipe && !can_set) {
                        (Some(f), true) => Point {
                            x: f.bounds.x + f.bounds.w - 4.0,
                            y: at.y,
                        },
                        _ => at,
                    };
                    device.gesture(gesture::tap(at, 1, None)).await?;
                    let in_page = field
                        .as_ref()
                        .is_some_and(|f| s.last.as_ref().is_some_and(|l| l.in_web_view(f.r#ref)));
                    if in_page && let Some(f) = self.follow_focus(s, field.as_ref()).await? {
                        // Chrome on Android zooms too, but reports where elements are and zooms back in on the
                        // field while the keyboard is up, so a pinch there cost about half a second and did not hold.
                        let android = device.info().id.starts_with("android:");
                        zoomed = !android
                            && field.as_ref().is_some_and(|before| {
                                before.r#ref == f.r#ref
                                    && f.bounds.w > before.bounds.w * PAGE_ZOOMED
                            });
                        held = f.value.clone().filter(|v| !v.is_empty());
                        wipe = replace && held.is_some() && mode != TypeMode::Set;
                        field = Some(f);
                    }
                }
                let before = held.clone().unwrap_or_default();
                if let Some(held) = held.filter(|_| wipe) {
                    let emptied = if can_set {
                        TextInput::SetValue(String::new())
                    } else {
                        TextInput::Keystrokes("\u{8}".repeat(held.chars().count()))
                    };
                    device
                        .text(emptied)
                        .await
                        .map_err(|e| withhold(e, secure))?;
                }
                let input = match (mode, secure) {
                    (TypeMode::Paste, true) => TextInput::PasteSecret(text.clone()),
                    (TypeMode::Paste, false) => TextInput::Paste(text.clone()),
                    (TypeMode::Set, _) if replace => TextInput::SetValue(text.clone()),
                    (TypeMode::Set, _) => TextInput::SetValue(format!("{before}{text}")),
                    (TypeMode::Keys, _) => TextInput::Keystrokes(text.clone()),
                };
                if !(replace && text.is_empty()) {
                    let t = Instant::now();
                    let typed = device.text(input).await;
                    self.lap("text", t);
                    typed.map_err(|e| withhold(e, secure))?;
                }
                let unzoom = zoomed
                    && device
                        .info()
                        .capabilities
                        .contains(Capabilities::MULTI_TOUCH);
                if unzoom {
                    let screen = self.screen_rect();
                    let above = s
                        .last
                        .as_ref()
                        .and_then(|l| l.header.keyboard)
                        .map_or(screen.center().y, |k| (screen.y + k.y) / 2.0);
                    let center = Point {
                        x: screen.center().x,
                        y: above,
                    };
                    device
                        .gesture(gesture::pinch(center, screen, false, 0.5))
                        .await?;
                }
                if submit {
                    device.key(Key::Enter).await?;
                }
                let n = text.chars().count();
                let label = field
                    .as_ref()
                    .map(|f| format!(" into @{}", f.r#ref))
                    .unwrap_or_default();
                let unzoomed = if unzoom {
                    ", then zoomed the page back out"
                } else {
                    ""
                };
                let summary = approved_summary(
                    format!(
                        "{brought}typed {}{label}{unzoomed}",
                        plural(n as u64, "char")
                    ),
                    approved,
                );
                let expect = if replace {
                    Expect::Exactly(text.clone())
                } else {
                    Expect::Appended {
                        before: field
                            .as_ref()
                            .and_then(|f| f.value.clone())
                            .unwrap_or_default(),
                        text: text.clone(),
                    }
                };
                let field_ref = field.map(|f| f.r#ref);
                let verify = Verify {
                    field: field_ref,
                    expect,
                    secure,
                    mode: Some(mode_name(mode)),
                    submitted: submit,
                };
                let rekey = !chosen
                    && !secure
                    && !submit
                    && replace
                    && !text.is_empty()
                    && field_ref.is_some_and(|r| s.last.as_ref().is_some_and(|l| l.in_web_view(r)));
                let keys = rekey.then(|| text.clone());
                let mask = secret.then_some(text);
                let cfg = if submit || p.expect_change_ms.is_some() {
                    cfg
                } else {
                    Config {
                        expect_change: Some(TYPED_QUIET),
                        quiet: TYPED_QUIET,
                        ..cfg
                    }
                };
                let first = self
                    .finish(
                        s,
                        p.settle,
                        cfg,
                        started,
                        summary.clone(),
                        Some(mode_name(mode)),
                        Some(verify.clone()),
                        mask.clone(),
                    )
                    .await?;
                let Some(keys) = keys.filter(|_| {
                    first.verified == Some(false) && field_empty(s.last.as_ref(), field_ref)
                }) else {
                    return Ok(first);
                };
                let started = self.begin_action();
                device.text(TextInput::Keystrokes(keys)).await?;
                let verify = Verify {
                    mode: Some("keys"),
                    ..verify
                };
                self.finish(
                    s,
                    p.settle,
                    cfg,
                    started,
                    summary,
                    Some("keys"),
                    Some(verify),
                    mask,
                )
                .await
            }
        }
    }

    /// A page can answer a tap on its search box by building another field and focusing that one, a few hundred ms
    /// to a few seconds later (Wikipedia, where the old field is gone and the keyboard comes up with the new one).
    /// Text pasted before that goes to the old field. Reads the screen until the same field has been focused twice
    /// in a row with the keyboard up, and returns that field as it now reads.
    async fn follow_focus(
        &mut self,
        s: &mut Session,
        field: Option<&Element>,
    ) -> Result<Option<Element>, Error> {
        let started = Instant::now();
        let mut at = field.map(|f| f.r#ref);
        let mut held = None;
        let mut steady = 0;
        loop {
            tokio::time::sleep(TREE_STEP).await;
            let now = self.fresh(s).await?;
            let focused = now
                .elements
                .iter()
                .find(|e| e.state.contains(ElementState::FOCUSED) && e.role.is_text_entry())
                .filter(|e| now.in_web_view(e.r#ref));
            match focused {
                Some(e) if Some(e.r#ref) == at => {
                    held = Some((*e).clone());
                    steady += 1;
                }
                Some(e) => {
                    at = Some(e.r#ref);
                    held = Some((*e).clone());
                    steady = 1;
                }
                None => steady = 0,
            }
            let waited = started.elapsed();
            let keyboard = now.header.keyboard.is_some() || waited >= FOCUS_KEYBOARD_WAIT;
            if (steady >= 2 && keyboard) || waited >= FOCUS_FOLLOW_WAIT {
                return Ok(held);
            }
        }
    }

    fn screen_rect(&self) -> Rect {
        let size = self
            .live
            .as_ref()
            .map_or(Size { w: 0.0, h: 0.0 }, Live::screen);
        Rect {
            x: 0.0,
            y: 0.0,
            w: size.w,
            h: size.h,
        }
    }

    /// Brings back a text field the agent was shown that a later screen dropped: a keyboard that opens shrinks a
    /// form's list, and the fields below it leave the tree (Contacts, run a4). Drags the list toward where the field
    /// was until exactly one field with its role and label shows, and says so for the summary.
    async fn bring_back(
        &mut self,
        s: &mut Session,
        cfg: Config,
        gone: &Element,
        stale: Error,
    ) -> Result<(Resolved, String), Error> {
        let device = self.device()?;
        let mut now = self.fresh(s).await?;
        let mut drags = 0u32;
        let mut direction = Direction::Down;
        loop {
            let same: Vec<Ref> = now
                .elements
                .iter()
                .filter(|e| e.role == gone.role && e.label == gone.label)
                .map(|e| e.r#ref)
                .collect();
            if let [r] = same[..] {
                let said = match drags {
                    0 => String::new(),
                    n => format!(
                        "scrolled {} {} to {}, then ",
                        direction.name(),
                        plural(n.into(), "time"),
                        quote(gone.label.as_deref().unwrap_or_default())
                    ),
                };
                return Ok((s.resolve(&Target::Ref { r#ref: r }, &now)?, said));
            }
            let view = above_keyboard(self.screen_rect(), now.header.keyboard);
            direction = if gone.bounds.center().y > view.y + view.h / 2.0 {
                Direction::Down
            } else {
                Direction::Up
            };
            let Some(list) = scroll_container(&now, direction).filter(|_| drags < BRING_BACK_DRAGS)
            else {
                return Err(stale);
            };
            let area = above_keyboard(list.bounds, now.header.keyboard);
            let started = self.begin_action();
            device.gesture(gesture::scroll(area, direction)).await?;
            drags += 1;
            self.settle(cfg, started).await;
            now = self.fresh(s).await?;
        }
    }

    /// One drag that stops under the finger, or with `until` drags until the text shows (architecture 10.1). The
    /// end of the content is two drags in a row that change nothing.
    #[allow(clippy::too_many_arguments)]
    async fn scroll(
        &mut self,
        s: &mut Session,
        settle: bool,
        cfg: Config,
        direction: Direction,
        within: Option<Target>,
        until: Option<Target>,
        max: Option<u32>,
    ) -> Result<ActionResult, Error> {
        let device = self.device()?;
        let t = Instant::now();
        let mut now = self.fresh(s).await?;
        self.lap("fresh", t);
        let holder = match &within {
            Some(t) => s.resolve(t, &now)?.element,
            None => scroll_container(&now, direction).cloned(),
        };
        let area = holder
            .as_ref()
            .map_or_else(|| self.screen_rect(), |e| e.bounds);
        let area = above_keyboard(area, now.header.keyboard);
        let holder = holder.map(|e| e.r#ref);
        let Some(until) = until else {
            let started = self.begin_action();
            let t = Instant::now();
            device.gesture(gesture::scroll(area, direction)).await?;
            self.lap("gesture", t);
            let summary = format!("scrolled {}", direction.name());
            return self
                .finish(s, settle, cfg, started, summary, None, None, None)
                .await;
        };
        let mut gestures = 0u32;
        let mut idle = 0u32;
        let mut moved_any = false;
        let mut last = None;
        let may_fling = device.info().capabilities.contains(Capabilities::FRAMES);
        let max = max.unwrap_or_else(|| default_scroll_limit(&until, &now, direction));
        let mut sorted = true;
        loop {
            match s.seek(&until, &now, holder, gestures > 0) {
                Ok(found) if found.covered.is_none() || idle >= 2 => {
                    let line = found.line.unwrap_or_default();
                    let summary = if gestures == 0 {
                        format!("already on screen, no scroll: {}", short(&line))
                    } else {
                        format!(
                            "scrolled {} {} to {}",
                            direction.name(),
                            plural(gestures.into(), "time"),
                            short(&line)
                        )
                    };
                    let st = match last {
                        Some(State::Done {
                            settled,
                            still_changing,
                            elapsed,
                            ..
                        }) => State::Done {
                            settled,
                            changed: true,
                            still_changing,
                            elapsed,
                        },
                        _ => State::Done {
                            settled: false,
                            changed: false,
                            still_changing: None,
                            elapsed: Duration::ZERO,
                        },
                    };
                    return self.report(s, st, None, summary, None, None, None).await;
                }
                Ok(_) | Err(Error::NotFound { .. } | Error::StaleRef { .. }) => {}
                Err(Error::Ambiguous { query, matches }) => {
                    return Err(Error::Ambiguous {
                        query,
                        matches: matches
                            .into_iter()
                            .map(|m| match m.split_once(' ') {
                                Some((r, rest)) if r.starts_with('@') => rest.to_string(),
                                _ => m,
                            })
                            .collect(),
                    });
                }
                Err(e) => return Err(e),
            }
            let query = || match &until {
                Target::Text { text, .. } => text.clone(),
                Target::Ref { r#ref } => format!("@{}", r#ref),
                Target::Point { .. } => "the target".into(),
            };
            if idle >= 2 {
                return Err(Error::NotReached {
                    query: query(),
                    scrolls: gestures,
                    end: if moved_any {
                        ScrollEnd::Ended
                    } else {
                        ScrollEnd::Stuck
                    },
                    last: moved_any
                        .then(|| last_label(&now, area, direction))
                        .flatten(),
                });
            }
            if gestures >= max {
                return Err(Error::NotReached {
                    query: query(),
                    scrolls: gestures,
                    end: ScrollEnd::Limit,
                    last: None,
                });
            }
            let started = self.begin_action();
            let (dir, far) = match numbered_distance(&until, &now, direction) {
                Some((behind, rows, span)) => {
                    let dir = if behind {
                        direction.opposite()
                    } else {
                        direction
                    };
                    (dir, rows > 3 * span)
                }
                None => match sorted_distance(&until, &now, direction, holder).filter(|_| sorted) {
                    Some((behind, far)) if behind => (direction.opposite(), far),
                    Some((_, far)) => (direction, far),
                    None => {
                        // The name falls between two rows on screen (or the rows are not in order): from here
                        // the search goes one way, so a name the list lacks cannot send it back and forth.
                        sorted = false;
                        (direction, false)
                    }
                },
            };
            // A fling moves nothing on the iOS runner, so a device without a frame stream scrolls one drag at a time.
            let g = if far && may_fling {
                gesture::fling(area, dir)
            } else {
                gesture::scroll(area, dir)
            };
            device.gesture(g).await?;
            gestures += 1;
            last = Some(self.settle(cfg, started).await);
            let next = self.fresh(s).await?;
            let moved = !matches!(diff(&now, &next), DiffOrFull::Diff(c) if c.is_empty());
            idle = if moved { 0 } else { idle + 1 };
            moved_any |= moved;
            now = next;
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn finish(
        &mut self,
        s: &mut Session,
        settle: bool,
        cfg: Config,
        started: Duration,
        summary: String,
        via: Option<&'static str>,
        verify: Option<Verify>,
        mask: Option<String>,
    ) -> Result<ActionResult, Error> {
        if !settle {
            self.unobserved = Some(started);
            return Ok(delivered(s, summary, via));
        }
        self.unobserved = None;
        let polled = !self
            .device()?
            .info()
            .capabilities
            .contains(Capabilities::FRAMES);
        let started = if polled { self.now() } else { started };
        let t = Instant::now();
        let (st, probe) = self.settle_probing(cfg, started, true).await;
        self.lap("settle", t);
        self.report(s, st, probe, summary, via, verify, mask).await
    }

    /// The accessibility tree can lag the pixels: a dialog or an expanding card appears a moment after the screen went
    /// quiet, and a page keeps loading behind a progress bar. Reads again until two reads agree and nothing is loading.
    async fn stabilize(
        &self,
        mut tree: RawTree,
        read_at: Option<Duration>,
    ) -> Result<RawTree, Error> {
        let started = Instant::now();
        let mut wait = read_at.map_or(TREE_STEP, |at| {
            TREE_STEP.saturating_sub(self.now().saturating_sub(at))
        });
        loop {
            tokio::time::sleep(wait).await;
            wait = TREE_STEP;
            let next = self.device()?.tree().await?;
            let same = next == tree;
            tree = next;
            let limit = if tree.web_view_pending() {
                WEB_VIEW_WAIT
            } else if tree.loading() {
                LOADING_WAIT
            } else {
                TREE_STABLE_WAIT
            };
            if (same && !tree.loading()) || started.elapsed() >= limit {
                return Ok(tree);
            }
        }
    }

    /// Reads the screen after a settle and answers with what changed since the agent last saw it.
    #[allow(clippy::too_many_arguments)]
    async fn report(
        &mut self,
        s: &mut Session,
        st: State,
        probe: Option<Probe>,
        mut summary: String,
        via: Option<&'static str>,
        verify: Option<Verify>,
        mask: Option<String>,
    ) -> Result<ActionResult, Error> {
        let t = Instant::now();
        let (mut tree, agreed, read_at) = match probe {
            Some(p) => (p.tree, p.stable, Some(p.at)),
            None => (self.device()?.tree().await?, false, None),
        };
        self.lap("tree", t);
        let agreed = agreed && !tree.loading() && !tree.web_view_pending();
        if !agreed
            && (matches!(st, State::Done { changed: true, .. })
                || verify.is_some()
                || tree.loading())
        {
            let t = Instant::now();
            tree = self.stabilize(tree, read_at).await?;
            self.lap("stabilize", t);
        }
        let render = Instant::now();
        let st = Some(st);
        let mut rendered = self.build(s, &tree, None);
        if only_lost_lines(s, &rendered) {
            let started = Instant::now();
            while started.elapsed() < CONTENT_WAIT {
                tokio::time::sleep(TREE_STEP).await;
                let tree = self.device()?.tree().await?;
                rendered = self.build(s, &tree, None);
                if !only_lost_lines(s, &rendered) {
                    break;
                }
            }
        }
        let empty_since = Instant::now();
        while rendered.elements.is_empty() && empty_since.elapsed() < CONTENT_WAIT {
            tokio::time::sleep(TREE_STEP).await;
            let tree = self.device()?.tree().await?;
            rendered = self.build(s, &tree, None);
        }
        self.read_checkboxes(&mut rendered, s.last.as_ref()).await?;
        let judge = |s: &Session, rendered: &Rendered| {
            verify
                .as_ref()
                .map(|v| v.check(rendered, v.submitted && moved_beyond(s, rendered, v.field)))
        };
        let mut verdict = judge(s, &rendered);
        let in_page = verify
            .as_ref()
            .and_then(|v| v.field)
            .is_some_and(|r| rendered.in_web_view(r));
        let waited = Instant::now();
        while in_page
            && matches!(verdict, Some(Verdict::Failed(_)))
            && waited.elapsed() < WEB_TEXT_WAIT
        {
            tokio::time::sleep(TREE_STEP).await;
            let tree = self.device()?.tree().await?;
            rendered = self.build(s, &tree, None);
            self.read_checkboxes(&mut rendered, s.last.as_ref()).await?;
            verdict = judge(s, &rendered);
        }
        // Contacts names a new phone field "Phone (Mobile)" and, once it holds text, "Phone": a new ref.
        if let (Some(v), Some(Verdict::Landed)) = (&verify, &verdict)
            && let Some(old) = v.field.filter(|r| rendered.index_of(*r).is_none())
            && let Some(new) = v.view(&rendered).target
        {
            summary = summary.replace(&format!(" into @{old}"), &format!(" into @{new}"));
        }
        let verified = verdict.as_ref().and_then(Verdict::verified);
        let unverified = match verdict {
            Some(Verdict::Failed(u)) => Some(u),
            _ => None,
        };
        let (kind, mut text) = match s.diff_for_reply(&rendered) {
            Some(DiffOrFull::Diff(changes)) => {
                (Kind::Diff, DiffOrFull::Diff(changes).text(&rendered))
            }
            _ => (
                Kind::Full,
                rendered.text(&Filters::default()).unwrap_or_default(),
            ),
        };
        let (settled, changed_signal, ms, still) = match st {
            Some(State::Done {
                settled,
                changed,
                elapsed,
                still_changing,
            }) => (settled, changed, elapsed.as_millis() as u32, still_changing),
            _ => (false, false, 0, None),
        };
        let changed = changed_signal || kind == Kind::Full || !text.is_empty();
        let mut elements = None;
        if let Some(secret) = &mask {
            text = text.replace(secret.as_str(), MASK);
            elements = None::<Vec<Element>>;
        }
        let observation = Observation {
            header: rendered.header.clone(),
            text,
            kind,
            changed,
            settled,
            settle_ms: ms,
            still_changing: still,
            loading: rendered.loading(),
            elements,
            capture: None,
        };
        s.set_last(Some(rendered));
        self.lap("render", render);
        Ok(ActionResult {
            ok: true,
            summary,
            via: via.map(str::to_string),
            verified,
            unverified,
            settled,
            settle_ms: ms,
            changed,
            woke: false,
            unlocked: false,
            observation,
        })
    }

    /// Runs the steps in order, each settled before the next, and stops at the first error. The observation is the
    /// diff from what the agent last saw, so a batch costs one reply however many steps it has.
    async fn batch(&mut self, s: &mut Session, p: BatchParams) -> Result<BatchResult, Error> {
        let baseline = s.last.clone();
        let mut done = Vec::new();
        let mut failed = None;
        let mut unseen_ref = false;
        let mut last: Option<ActionResult> = None;
        for (i, raw) in p.steps.iter().enumerate() {
            let step = Step::parse(raw).map_err(|_| unreachable_state())?;
            let result = match step {
                Step::Act(action) => {
                    self.act(
                        s,
                        ActParams {
                            action,
                            approval: p.approval.filter(|a| *a == Approval::Autonomous || i == 0),
                            ask: p.ask.clone(),
                            settle: true,
                            timeout_ms: p.timeout_ms,
                            expect_change_ms: p.expect_change_ms,
                        },
                    )
                    .await
                }
                Step::Wait(condition) => {
                    self.wait(
                        s,
                        WaitParams {
                            condition,
                            timeout_ms: p.timeout_ms,
                            expect_change_ms: p.expect_change_ms,
                        },
                    )
                    .await
                }
            };
            match result {
                Ok(r) => {
                    done.push(StepResult {
                        summary: r.summary.clone(),
                        verified: r.verified,
                        unverified: r.unverified.clone(),
                    });
                    last = Some(r);
                }
                Err(e @ Error::DeviceGone { .. }) => return Err(e),
                Err(e) => {
                    unseen_ref = matches!(e, Error::StaleRef { unseen: true, .. });
                    failed = Some(StepFailure {
                        step: i + 1,
                        error: e.to_rpc(),
                    });
                    break;
                }
            }
        }
        let mut result = match (last, failed.is_some()) {
            (Some(r), false) => r,
            _ => {
                let tree = self.device()?.tree().await?;
                let rendered = self.build(s, &tree, None);
                let observation = Observation {
                    header: rendered.header.clone(),
                    text: String::new(),
                    kind: Kind::Full,
                    changed: true,
                    settled: false,
                    settle_ms: 0,
                    still_changing: None,
                    loading: rendered.loading(),
                    elements: None,
                    capture: None,
                };
                s.set_last(Some(rendered));
                ActionResult {
                    ok: true,
                    summary: String::new(),
                    via: None,
                    verified: None,
                    unverified: None,
                    settled: false,
                    settle_ms: 0,
                    changed: true,
                    woke: false,
                    unlocked: false,
                    observation,
                }
            }
        };
        if unseen_ref {
            result.observation.kind = Kind::Full;
            result.observation.text = String::new();
        } else if let (Some(base), Some(now)) = (&baseline, &s.last) {
            match diff(base, now) {
                DiffOrFull::Diff(changes) => {
                    result.observation.kind = Kind::Diff;
                    result.observation.text = DiffOrFull::Diff(changes).text(now);
                }
                DiffOrFull::Full => {
                    result.observation.kind = Kind::Full;
                    result.observation.text = now.text(&Filters::default()).unwrap_or_default();
                }
            }
        } else if let Some(now) = &s.last {
            result.observation.kind = Kind::Full;
            result.observation.text = now.text(&Filters::default()).unwrap_or_default();
        }
        result.observation.changed = result.observation.kind == Kind::Full
            || !result.observation.text.is_empty()
            || result.changed;
        Ok(BatchResult {
            ok: failed.is_none(),
            steps: done,
            failed,
            settled: result.settled,
            settle_ms: result.settle_ms,
            changed: result.observation.changed,
            observation: result.observation,
        })
    }

    async fn wait(&mut self, s: &mut Session, p: WaitParams) -> Result<ActionResult, Error> {
        match p.condition {
            WaitCondition::Settled(_) => self.wait_settled(s, p).await,
            WaitCondition::Appears(ref t) => {
                let t = t.clone();
                self.wait_for(s, &t, true, p.timeout_ms).await
            }
            WaitCondition::Gone(ref t) => {
                let t = t.clone();
                self.wait_for(s, &t, false, p.timeout_ms).await
            }
        }
    }

    /// Waits until the target is on screen (or gone). The tree is read once at the start and then once after each
    /// window change or frame that moved enough pixels, never on a timer: apps' own content-change events are not
    /// subscribed (architecture 4.6), pixels are the signal.
    async fn wait_for(
        &mut self,
        s: &mut Session,
        target: &Target,
        present: bool,
        timeout_ms: Option<u64>,
    ) -> Result<ActionResult, Error> {
        let device = self.device()?;
        self.unobserved = None;
        let timeout = timeout_ms.map_or(Config::action().timeout, Duration::from_millis);
        let started = Instant::now();
        let deadline = started + timeout;
        let mut reads = 0u32;
        let mut last_read = false;
        loop {
            let tree = device.tree().await?;
            reads += 1;
            let rendered = self.build(s, &tree, None);
            let found = matches!(
                s.seek(target, &rendered, None, true),
                Ok(_) | Err(Error::Ambiguous { .. })
            );
            if found == present {
                let text = rendered.text(&Filters::default()).unwrap_or_default();
                let what = describe(target);
                let summary = format!(
                    "{what} {} after {reads} tree read{}",
                    if present { "appeared" } else { "is gone" },
                    if reads == 1 { "" } else { "s" }
                );
                let observation = Observation {
                    header: rendered.header.clone(),
                    text,
                    kind: Kind::Full,
                    changed: true,
                    settled: false,
                    settle_ms: 0,
                    still_changing: None,
                    loading: rendered.loading(),
                    elements: None,
                    capture: None,
                };
                s.set_last(Some(rendered));
                return Ok(ActionResult {
                    ok: true,
                    summary,
                    via: None,
                    verified: None,
                    unverified: None,
                    settled: false,
                    settle_ms: started.elapsed().as_millis() as u32,
                    changed: true,
                    woke: false,
                    unlocked: false,
                    observation,
                });
            }
            if last_read {
                return Err(Error::Timeout {
                    waited_ms: started.elapsed().as_millis() as u32,
                    condition: format!(
                        "{} to {}",
                        describe(target),
                        if present { "appear" } else { "be gone" }
                    ),
                });
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if tokio::time::timeout(left, self.next_change())
                .await
                .is_err()
            {
                last_read = true;
            }
        }
    }

    /// Returns after a window change or a frame with a real change, once the burst that carried it has paused.
    async fn next_change(&mut self) {
        loop {
            let sig = next_signal(&mut self.live).await;
            let at = self.now();
            let significant = match self.absorb(sig, at) {
                Some(Input::WindowChanged) => true,
                Some(Input::Frame {
                    changed_area_pt2, ..
                }) => changed_area_pt2 > Config::action().first_change_area,
                None => false,
            };
            if significant {
                tokio::time::sleep(WAIT_DEBOUNCE).await;
                return;
            }
        }
    }

    async fn wait_settled(
        &mut self,
        s: &mut Session,
        p: WaitParams,
    ) -> Result<ActionResult, Error> {
        let mut cfg = Config::snapshot();
        let timeout = p
            .timeout_ms
            .map_or(Config::action().timeout, Duration::from_millis);
        cfg.timeout = timeout;
        let (cfg, started) = self.observe(cfg, p.expect_change_ms);
        let st = self.settle(cfg, started).await;
        if let State::Done {
            settled: false,
            elapsed,
            still_changing,
            ..
        } = st
        {
            return Err(Error::NotSettled {
                waited_ms: elapsed.as_millis() as u32,
                still_changing: still_changing.unwrap_or_else(zero),
            });
        }
        let tree = self.device()?.tree().await?;
        let mut rendered = self.build(s, &tree, Some(settle_line(st)));
        self.read_checkboxes(&mut rendered, s.last.as_ref()).await?;
        let text = rendered.text(&Filters::default()).unwrap_or_default();
        let ms = match st {
            State::Done { elapsed, .. } => elapsed.as_millis() as u32,
            State::Waiting => 0,
        };
        let observation = Observation {
            header: rendered.header.clone(),
            text,
            kind: Kind::Full,
            changed: true,
            settled: true,
            settle_ms: ms,
            still_changing: None,
            loading: rendered.loading(),
            elements: None,
            capture: None,
        };
        s.set_last(Some(rendered));
        Ok(ActionResult {
            ok: true,
            summary: "screen settled".into(),
            via: None,
            verified: None,
            unverified: None,
            settled: true,
            settle_ms: ms,
            changed: true,
            woke: false,
            unlocked: false,
            observation,
        })
    }
}

/// For a target like "Row 500" among rows labelled "Row 12", "Row 13": whether the target lies against `direction`,
/// how many rows away it is, and how many rows are visible. Lets the search fling across a long list and come back
/// instead of dragging a screen at a time.
/// A reply that only takes lines away is a window caught while it opens: the old screen is gone and the new one is
/// not in the tree yet.
fn only_lost_lines(s: &Session, now: &Rendered) -> bool {
    matches!(
        s.last.as_ref().map(|prev| diff(prev, now)),
        Some(DiffOrFull::Diff(changes))
            if !changes.is_empty() && changes.iter().all(|c| matches!(c, Change::Removed(_)))
    )
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
        Target::Text { text, .. } => format!("\"{text}\""),
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
        ],
    })
}

fn stale(r: Ref, s: &Session) -> Error {
    Error::StaleRef {
        r#ref: r,
        closest: None,
        unseen: s.unseen(r),
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
