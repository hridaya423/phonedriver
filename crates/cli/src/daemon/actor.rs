//! One task per device. It owns the device connection, the lease and the sessions, so no lock guards device state.

use std::collections::HashMap;
use std::future::pending;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use phone_use_core::device::{
    Button, Capabilities, Device, DeviceEvent, FrameRef, Gesture, Key, Quality, TextInput,
    TimedPoint,
};
use phone_use_core::diff::{DiffOrFull, diff};
use phone_use_core::element::{Element, ElementState, Ref, Role};
use phone_use_core::error::{Error, Op};
use phone_use_core::geometry::{Point, Rect, Size};
use phone_use_core::prune::prune;
use phone_use_core::raw::RawTree;
use phone_use_core::render::{
    AppLine, DeviceLine, Filters, Header, Orientation, Rendered, SettleLine,
};
use phone_use_core::settle::{Config, Detector, Input, State};
use serde_json::{Value, json};
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time::Instant;

use super::DeviceSource;
use super::session::{Resolved, Session};
use crate::wire::{
    ActParams, Action, ActionResult, FindParams, Kind, Observation, ScreenshotResult,
    SnapshotParams, Target, TypeMode, WaitParams,
};

pub const LEASE_TTL: Duration = Duration::from_secs(600);
const TICK: Duration = Duration::from_millis(5);
const MASK: &str = "•••";

pub enum Call {
    Snapshot(SnapshotParams),
    Screenshot,
    Act(ActParams),
    Wait(WaitParams),
    Find(FindParams),
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

enum Signal {
    Frame(FrameRef),
    Window,
    Display(Size),
    Gone,
    Ignored,
}

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
            Ok(DeviceEvent::Gone) => Signal::Gone,
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
            Signal::Window => Input::WindowChanged,
            Signal::Display(screen) => {
                if let Some(l) = &mut self.live {
                    l.screen = screen;
                }
                Input::WindowChanged
            }
            Signal::Gone => {
                self.live = None;
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
                return Err(Error::DeviceBusy {
                    id: self.id.clone(),
                    held_by: l.session.clone(),
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
            return Ok(self
                .lease
                .as_ref()
                .map_or(Value::Null, |l| Value::String(l.session.clone())));
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
            Call::Screenshot => to_value(self.screenshot().await?),
            Call::Act(p) => to_value(self.act(s, p).await?),
            Call::Wait(p) => to_value(self.wait(s, p).await?),
            Call::Find(p) => {
                let now = self.fresh(s).await?;
                to_value(s.find(&now, &p.text, p.role))
            }
            Call::Release | Call::Leased => json!({}),
        };
        Ok(value)
    }

    async fn screenshot(&mut self) -> Result<ScreenshotResult, Error> {
        self.need(
            Capabilities::SCREENSHOT,
            Op::Screenshot,
            "this device has no screen capture",
        )?;
        let device = self.device()?;
        let image = device.screenshot(Quality(100)).await?;
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
        })
    }

    fn build(&self, s: &mut Session, tree: &RawTree, settle: Option<SettleLine>) -> Rendered {
        let (info, screen, secure_hidden) = match &self.live {
            Some(l) => (
                l.device.info().clone(),
                l.screen,
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
        let rendered = self.build(s, &tree, line);
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
        Ok(self.build(s, &tree, None))
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
            });
        }
        let now = self.fresh(s).await?;
        s.resolve(target, &now)
    }

    async fn act(&mut self, s: &mut Session, p: ActParams) -> Result<ActionResult, Error> {
        let mut cfg = Config::action();
        if let Some(t) = p.timeout_ms {
            cfg.timeout = Duration::from_millis(t);
        }
        if let Some(t) = p.expect_change_ms {
            cfg.expect_change = Some(Duration::from_millis(t));
        }
        let device = self.device()?;
        match p.action {
            Action::Tap { target } => {
                self.need(
                    Capabilities::GESTURE,
                    Op::Gesture,
                    "this device cannot inject touches",
                )?;
                let Resolved {
                    at, line: element, ..
                } = self.resolve_target(s, &target).await?;
                let started = self.begin_action();
                device.gesture(tap(at)).await?;
                let summary = match &element {
                    Some(line) => format!(
                        "tapped {} at {},{}",
                        short(line),
                        at.x.round(),
                        at.y.round()
                    ),
                    None => format!("tapped at {},{}", at.x.round(), at.y.round()),
                };
                self.finish(s, p.settle, cfg, started, summary, None, None, None)
                    .await
            }
            Action::Press { key } => {
                let started = self.begin_action();
                press(&*device, &key).await?;
                self.finish(
                    s,
                    p.settle,
                    cfg,
                    started,
                    format!("pressed {key}"),
                    None,
                    None,
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
                let started = self.begin_action();
                if let Some(at) = tap_at {
                    device.gesture(tap(at)).await?;
                }
                let input = match (mode, secure) {
                    (TypeMode::Paste, true) => TextInput::PasteSecret(text.clone()),
                    (TypeMode::Paste, false) => TextInput::Paste(text.clone()),
                    (TypeMode::Set, _) => TextInput::SetValue(text.clone()),
                    (TypeMode::Keys, _) => TextInput::Keystrokes(text.clone()),
                };
                device.text(input).await.map_err(|e| withhold(e, secure))?;
                if submit {
                    device.key(Key::Enter).await?;
                }
                let n = text.chars().count();
                let label = field
                    .as_ref()
                    .map(|f| format!(" into @{}", f.r#ref))
                    .unwrap_or_default();
                let summary = format!("typed {n} chars{label}");
                let verify = Verify {
                    field: field.map(|f| f.r#ref),
                    text: text.clone(),
                    secure,
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
        let st = Some(self.settle(cfg, started).await);
        let tree = self.device()?.tree().await?;
        let rendered = self.build(s, &tree, st.map(settle_line));
        let verified = verify.map(|v| v.check(&rendered));
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
            settled,
            settle_ms: ms,
            changed,
            observation,
        })
    }

    async fn wait(&mut self, s: &mut Session, p: WaitParams) -> Result<ActionResult, Error> {
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
            settled: true,
            settle_ms: ms,
            changed: true,
            observation,
        })
    }
}

/// The reply to an input sent with `settle: false`: delivered, nothing observed.
fn delivered(s: &Session, summary: String, via: Option<&'static str>) -> ActionResult {
    ActionResult {
        ok: true,
        summary,
        via: via.map(str::to_string),
        verified: None,
        settled: false,
        settle_ms: 0,
        changed: false,
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

struct Verify {
    field: Option<Ref>,
    text: String,
    secure: bool,
}

impl Verify {
    /// The field now holds the text; a secure field exposes only masked text, so only lengths are compared.
    fn check(&self, now: &Rendered) -> bool {
        let Some(e) = self
            .field
            .and_then(|r| now.index_of(r))
            .map(|i| &now.elements[i])
            .or_else(|| {
                now.elements
                    .iter()
                    .find(|e| e.state.contains(ElementState::FOCUSED) && e.role.is_text_entry())
            })
        else {
            return false;
        };
        let Some(value) = e.value.as_deref() else {
            return false;
        };
        if self.secure || is_secure(e) {
            value.chars().count() >= self.text.chars().count()
        } else {
            value.contains(&self.text)
        }
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

fn tap(at: Point) -> Gesture {
    Gesture {
        fingers: vec![vec![TimedPoint { at, t_ms: 0 }, TimedPoint { at, t_ms: 0 }]],
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

async fn press(device: &dyn Device, key: &str) -> Result<(), Error> {
    let name = Value::String(key.replace('-', "_"));
    if let Ok(b) = serde_json::from_value::<Button>(name.clone()) {
        return device.button(b).await;
    }
    if let Ok(k) = serde_json::from_value::<Key>(name) {
        return device.key(k).await;
    }
    Err(Error::NotFound {
        query: key.to_string(),
        similar: vec![
            "home".into(),
            "back".into(),
            "recents".into(),
            "notifications".into(),
            "lock".into(),
            "volume-up".into(),
            "volume-down".into(),
            "enter".into(),
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
