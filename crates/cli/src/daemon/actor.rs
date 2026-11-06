//! One task per device. It owns the device connection, the lease and the sessions, so no lock guards device state.

use std::collections::HashMap;
use std::future::pending;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use phone_use_core::device::{
    App, AppOp, Button, Capabilities, ClipboardOp, Device, DeviceEvent, FrameRef, Gesture, Key,
    Quality, ScreenState, ShellRequest, TextInput, TimedPoint, Url,
};
use phone_use_core::diff::{DiffOrFull, diff};
use phone_use_core::element::{Actions, Element, ElementState, Ref, Role, Source};
use phone_use_core::error::{Direction, Error, Op, ScrollEnd, UserAction};
use phone_use_core::geometry::{Point, Rect, Size};
use phone_use_core::gesture;
use phone_use_core::ocr::{merge, tree_is_hollow};
use phone_use_core::prune::prune;
use phone_use_core::raw::RawTree;
use phone_use_core::render::{
    AppLine, DeviceLine, Filters, Header, Orientation, Rendered, SettleLine,
};
use phone_use_core::settle::{Config, Detector, Input, State};
use phone_use_core::similar::{SUGGEST_WITHIN, normalized_distance};
use serde_json::{Value, json};
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time::Instant;

use super::DeviceSource;
use super::session::{Resolved, Session};
use super::shell;
use super::verify::{Expect, Verdict, Verify};
use crate::wire::{
    ActParams, Action, ActionResult, AppParams, BatchParams, BatchResult, ClipboardParams,
    FindParams, Kind, Observation, PinchDirection, ScreenshotParams, ScreenshotResult, ShellParams,
    ShellResult, SnapshotParams, Step, StepFailure, StepResult, Target, TypeMode, WaitCondition,
    WaitParams,
};

pub const LEASE_TTL: Duration = Duration::from_secs(600);
const TICK: Duration = Duration::from_millis(5);
const SCREEN_WAIT: Duration = Duration::from_secs(3);
const KEYGUARD_WAIT: Duration = Duration::from_millis(1000);
const WAIT_DEBOUNCE: Duration = Duration::from_millis(100);
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

pub struct Job {
    pub session: String,
    pub share: bool,
    pub call: Call,
    pub reply: oneshot::Sender<Result<Value, Error>>,
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
        awake: None,
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
    frames: watch::Receiver<FrameRef>,
    events: broadcast::Receiver<DeviceEvent>,
    screen: Size,
}

impl Live {
    fn screen(&self) -> Size {
        self.device.current_screen().unwrap_or(self.screen)
    }
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
    /// When `prepare` last found the screen on and unlocked; cleared by any window or display event.
    awake: Option<Duration>,
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
                    let result = self.handle(job.session, job.share, job.call).await;
                    let _ = job.reply.send(result);
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

    fn now(&self) -> Duration {
        self.base.elapsed()
    }

    fn drop_lease(&mut self) {
        self.lease = None;
        self.live = None;
        self.unobserved = None;
        self.awake = None;
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

    async fn ensure_live(&mut self) -> Result<(), Error> {
        if self.live.is_some() {
            return Ok(());
        }
        let device = self.source.connect(&self.id).await?;
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
            frames,
            events,
            screen,
        });
        Ok(())
    }

    async fn handle(&mut self, session: String, share: bool, call: Call) -> Result<Value, Error> {
        if matches!(call, Call::Leased) {
            return Ok(self.lease.as_ref().map_or(
                Value::Null,
                |l| json!({"session": l.session, "idle_s": l.last_call.elapsed().as_secs()}),
            ));
        }
        if matches!(call, Call::Release) {
            if self.lease.as_ref().is_some_and(|l| l.session == session) {
                self.drop_lease();
            }
            return Ok(json!({}));
        }
        self.acquire(&session, share)?;
        if let Err(e) = self.ensure_live().await {
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

    /// Install, stop, clear data or uninstall. On a physical device only an app phone-use installed (recorded in
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
                    reason: "phone-use did not install this app on this physical device, so it will not clear or remove it",
                });
            }
        }
        device.app_op(op, &found.id).await?;
        if op == AppOp::Uninstall {
            self.ledger(|l| l.retain(|a| *a != found.id.0));
        }
        s.last = None;
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
            s.last = Some(self.fresh(s).await?);
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

    /// Reads, lets `f` change, and writes back the apps phone-use installed on this device.
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
        Ok(json!({"id": info.id, "model": info.model, "os": info.os, "frames_ms": frames_ms}))
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
        let mut image = device.screenshot(Quality(100)).await?;
        let mut marked = None;
        if p.marks {
            let now = self.fresh(s).await?;
            let marks: Vec<phone_use_video::marks::Mark> = now
                .elements
                .iter()
                .filter(|e| {
                    e.actions
                        .intersects(Actions::TAP | Actions::LONG_PRESS | Actions::EDIT)
                        && e.bounds.w > 0.0
                        && e.bounds.h > 0.0
                })
                .map(|e| phone_use_video::marks::Mark {
                    label: e.r#ref.to_string(),
                    x: e.bounds.x,
                    y: e.bounds.y,
                    w: e.bounds.w,
                    h: e.bounds.h,
                })
                .collect();
            let png = image.bytes.clone();
            let drawn = tokio::task::spawn_blocking(move || {
                phone_use_video::marks::draw_marks(&png, &marks).map(|b| (b, marks.len()))
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
        let app = tree
            .windows
            .iter()
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
        let mut det = Detector::new(cfg, started);
        if let Some((t, input)) = self.recent
            && (cfg.expect_change.is_none() || t >= started)
        {
            det.feed(input, t);
        }
        loop {
            let st = det.poll(self.now());
            if matches!(st, State::Done { .. }) {
                return st;
            }
            tokio::select! {
                sig = next_signal(&mut self.live) => {
                    let at = self.now();
                    if let Some(input) = self.absorb(sig, at) {
                        det.feed(input, at);
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
            Some(self.settle(cfg, started).await)
        };
        let tree = device.tree().await?;
        let line = settle.map(settle_line);
        let mut rendered = self.build(s, &tree, line);
        s.ocr = false;
        if p.pixels || tree_is_hollow(&rendered) {
            match self.merge_pixels(s, &mut rendered).await {
                Ok(()) => s.ocr = true,
                Err(e) if p.pixels => return Err(e),
                Err(_) => {}
            }
        }
        let filters = Filters {
            interactive: p.interactive,
            within: p.within,
            max_lines: p.max_lines,
        };
        let text = if p.diff && s.last.is_some() {
            let prev = s.last.as_ref().ok_or_else(unreachable_state)?;
            match diff(prev, &rendered) {
                DiffOrFull::Diff(changes) => {
                    (Kind::Diff, DiffOrFull::Diff(changes).text(&rendered))
                }
                DiffOrFull::Full => (
                    Kind::Full,
                    rendered.text(&filters).map_err(|r| stale(r, &rendered))?,
                ),
            }
        } else {
            (
                Kind::Full,
                rendered.text(&filters).map_err(|r| stale(r, &rendered))?,
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
        s.last = Some(rendered);
        Ok(obs)
    }

    /// Fetches the screen as it is now for resolving a target. It does not replace what the agent was last shown.
    async fn fresh(&mut self, s: &mut Session) -> Result<Rendered, Error> {
        let tree = self.device()?.tree().await?;
        let mut rendered = self.build(s, &tree, None);
        if s.ocr {
            self.merge_pixels(s, &mut rendered).await?;
        }
        Ok(rendered)
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
            self.until_app_content().await?;
        }
        self.awake = Some(self.now());
        Ok((woke, unlocked))
    }

    async fn until_app_content(&self) -> Result<(), Error> {
        let device = self.device()?;
        // Observed on API 26 and 37: input sent as soon as the screen is on and unlocked is rejected
        // by the dispatcher until a window has input focus again, and on API 26 the tree lists only
        // system windows, then part of the app, until the app's window is back. A cold-started Settings
        // on API 37 likewise reports an empty tree for a second after its first frame.
        let started = Instant::now();
        let mut last_len = 0;
        let mut equal_reads = 0;
        // Observed on the iOS Simulator: Settings adds a Toolbar group 300 ms to 1 s after its first frame, which
        // made the first refs stale at once, so a polled device must read the same count three times in a row.
        let needed = if device.info().capabilities.contains(Capabilities::FRAMES) {
            1
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
                tree.windows
                    .iter()
                    .any(|w| w.id == n.window && w.kind == "application")
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

    async fn act(&mut self, s: &mut Session, p: ActParams) -> Result<ActionResult, Error> {
        let (woke, unlocked) = self.prepare(&p.action).await?;
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
                } = self.resolve_target(s, &target).await?;
                if let (true, Some(e)) = (under_keyboard, &found) {
                    return Err(Error::OffScreen {
                        r#ref: e.r#ref,
                        scroll_hint: Direction::Down,
                        behind_keyboard: true,
                        covered_by: None,
                    });
                }
                if let (Some((bar, scroll_hint)), Some(e)) = (covered, &found) {
                    return Err(Error::OffScreen {
                        r#ref: e.r#ref,
                        scroll_hint,
                        behind_keyboard: false,
                        covered_by: Some(bar),
                    });
                }
                let started = self.begin_action();
                device
                    .gesture(gesture::tap(at, count.unwrap_or(1), long))
                    .await?;
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
                self.finish(s, p.settle, cfg, started, summary, None, None, None)
                    .await
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
            Action::Launch { app } => {
                self.need(
                    Capabilities::LAUNCH,
                    Op::Launch,
                    "this device cannot launch apps",
                )?;
                let installed = device.apps().await?;
                let found = find_app(&installed, &app)?;
                cfg.expect_change = p
                    .expect_change_ms
                    .map(Duration::from_millis)
                    .or(Config::launch().expect_change);
                let started = self.begin_action();
                device.launch(&found.id).await?;
                self.until_app_content().await?;
                s.last = None;
                let summary = format!("launched {}", found.name);
                self.finish(s, p.settle, cfg, started, summary, None, None, None)
                    .await
            }
            Action::Open { url } => {
                self.need(
                    Capabilities::OPEN_URL,
                    Op::OpenUrl,
                    "this device cannot open links",
                )?;
                cfg.expect_change = p
                    .expect_change_ms
                    .map(Duration::from_millis)
                    .or(Config::launch().expect_change);
                let started = self.begin_action();
                device.open_url(&Url(url.clone())).await?;
                self.until_app_content().await?;
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
                        .report(s, st, "keyboard already hidden".into(), None, None, None)
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
                let mode = mode.unwrap_or(TypeMode::Paste);
                let (cap, reason) = match mode {
                    TypeMode::Paste => (Capabilities::TEXT_PASTE, "this device cannot paste"),
                    TypeMode::Set => (
                        Capabilities::TEXT_SET,
                        "this device cannot set field values",
                    ),
                    TypeMode::Keys => (Capabilities::TEXT_KEYS, "this device cannot type keys"),
                };
                self.need(cap, Op::Text, reason)?;
                let (field, tap_at): (Option<Element>, Option<Point>) = match &into {
                    Some(t) => {
                        let r = self.resolve_target(s, t).await?;
                        (r.element, Some(r.at))
                    }
                    None => (
                        s.last.as_ref().and_then(|last| {
                            last.elements
                                .iter()
                                .find(|e| {
                                    e.state.contains(ElementState::FOCUSED)
                                        && e.role.is_text_entry()
                                })
                                .cloned()
                        }),
                        None,
                    ),
                };
                let secure = secret || field.as_ref().is_some_and(is_secure);
                let replace = into.is_some() && !append;
                let held = field
                    .as_ref()
                    .and_then(|f| f.value.clone())
                    .filter(|v| !v.is_empty());
                let wipe = replace && held.is_some() && mode != TypeMode::Set;
                let can_set = device.info().capabilities.contains(Capabilities::TEXT_SET);
                let started = self.begin_action();
                if let Some(at) = tap_at {
                    let at = match (&field, wipe && !can_set) {
                        (Some(f), true) => Point {
                            x: f.bounds.x + f.bounds.w - 4.0,
                            y: at.y,
                        },
                        _ => at,
                    };
                    device.gesture(gesture::tap(at, 1, None)).await?;
                }
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
                    (TypeMode::Set, _) => TextInput::SetValue(text.clone()),
                    (TypeMode::Keys, _) => TextInput::Keystrokes(text.clone()),
                };
                if !(replace && text.is_empty()) {
                    device.text(input).await.map_err(|e| withhold(e, secure))?;
                }
                if submit {
                    device.key(Key::Enter).await?;
                }
                let n = text.chars().count();
                let label = field
                    .as_ref()
                    .map(|f| format!(" into @{}", f.r#ref))
                    .unwrap_or_default();
                let summary = format!("typed {n} chars{label}");
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
                let verify = Verify {
                    field: field.map(|f| f.r#ref),
                    expect,
                    secure,
                    mode: Some(mode_name(mode)),
                    submitted: submit,
                };
                let mask = secret.then_some(text);
                self.finish(
                    s,
                    p.settle,
                    cfg,
                    started,
                    summary,
                    Some(mode_name(mode)),
                    Some(verify),
                    mask,
                )
                .await
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
        let mut now = self.fresh(s).await?;
        let area = match &within {
            Some(t) => s
                .resolve(t, &now)?
                .element
                .map_or_else(|| self.screen_rect(), |e| e.bounds),
            None => scroll_container(&now, direction).unwrap_or_else(|| self.screen_rect()),
        };
        let Some(until) = until else {
            let started = self.begin_action();
            device.gesture(gesture::scroll(area, direction)).await?;
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
        loop {
            match s.resolve(&until, &now) {
                Ok(found) if found.covered.is_none() || idle >= 2 => {
                    let line = found.line.unwrap_or_default();
                    let summary = format!(
                        "scrolled {} {gestures} times to {}",
                        direction.name(),
                        short(&line)
                    );
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
                            settled: true,
                            changed: false,
                            still_changing: None,
                            elapsed: Duration::ZERO,
                        },
                    };
                    return self.report(s, st, summary, None, None, None).await;
                }
                Ok(_) | Err(Error::NotFound { .. } | Error::StaleRef { .. }) => {}
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
                None => (direction, false),
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
        let st = self.settle(cfg, started).await;
        self.report(s, st, summary, via, verify, mask).await
    }

    /// Reads the screen after a settle and answers with what changed since the agent last saw it.
    async fn report(
        &mut self,
        s: &mut Session,
        st: State,
        summary: String,
        via: Option<&'static str>,
        verify: Option<Verify>,
        mask: Option<String>,
    ) -> Result<ActionResult, Error> {
        let st = Some(st);
        let tree = self.device()?.tree().await?;
        let rendered = self.build(s, &tree, st.map(settle_line));
        let verdict = verify.as_ref().map(|v| v.check(&rendered));
        let verified = verdict.as_ref().and_then(Verdict::verified);
        let unverified = match verdict {
            Some(Verdict::Failed(u)) => Some(u),
            _ => None,
        };
        let (kind, mut text) = match s.last.as_ref().map(|prev| diff(prev, &rendered)) {
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
        s.last = Some(rendered);
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
        let mut last: Option<ActionResult> = None;
        for (i, raw) in p.steps.iter().enumerate() {
            let step = Step::parse(raw).map_err(|_| unreachable_state())?;
            let result = match step {
                Step::Act(action) => {
                    self.act(
                        s,
                        ActParams {
                            action,
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
                s.last = Some(rendered);
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
        if let (Some(base), Some(now)) = (&baseline, &s.last) {
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
                s.resolve(target, &rendered),
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
                s.last = Some(rendered);
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
        let rendered = self.build(s, &tree, Some(settle_line(st)));
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
        s.last = Some(rendered);
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
fn scroll_container(now: &Rendered, direction: Direction) -> Option<Rect> {
    let axis = match direction {
        Direction::Up | Direction::Down => Actions::SCROLL_UP | Actions::SCROLL_DOWN,
        Direction::Left | Direction::Right => Actions::SCROLL_LEFT | Actions::SCROLL_RIGHT,
    };
    now.elements
        .iter()
        .filter(|e| e.actions.intersects(axis))
        .max_by(|a, b| a.bounds.area().total_cmp(&b.bounds.area()))
        .map(|e| e.bounds)
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
    Err(Error::NoApp {
        query: query.to_string(),
        similar: close
            .into_iter()
            .take(5)
            .map(|(_, a)| format!("{} ({})", a.name, a.id.0))
            .collect(),
    })
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

fn stale(r: Ref, _now: &Rendered) -> Error {
    Error::StaleRef {
        r#ref: r,
        closest: None,
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
