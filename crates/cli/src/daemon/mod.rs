//! The daemon: owns every device connection and the sessions that use them (architecture 5).
//!
//! Adding a verb: add its params and result to `wire.rs`, a `Call` variant and an actor method in
//! `actor.rs`, and a route in `Daemon::route`. The CLI side is a clap variant in `commands.rs`.

pub mod actor;
#[cfg(feature = "android")]
pub mod android;
pub mod check;
pub mod checkbox;
pub mod collect;
pub mod files;
pub mod guard;
pub mod ios;
pub mod journal;
pub mod ledger;
pub mod originals;
pub mod policy;
pub mod progress;
pub mod rpc;
pub mod scope;
pub mod secret_field;
pub(crate) mod secrets;
pub mod session;
pub mod shell;
pub mod socket;
pub mod sources;
pub mod trace;
pub mod verify;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use phonedriver_core::device::{Capabilities, Device, DisplayMode, lane_of};
use phonedriver_core::error::{DeviceSummary, Error, Op, can_be_started};
use phonedriver_core::scope::AppScope;
use serde_json::{Value, json};
use tokio::io::split;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Notify, mpsc, oneshot};
use tokio::time::Instant;

use self::actor::{Call, Job, Phases};
use self::rpc::RpcError;

tokio::task_local! {
    static PHASES: std::cell::RefCell<Phases>;
}
use self::trace::{TraceOptions, Traces};
use crate::wire::{
    ActParams, Action, BatchParams, Common, DeviceEntry, MAX_TIME_LIMIT_S, RecordParams,
    SettingParams, ShellParams, Step, VERSION,
};

const IDLE_EXIT: Duration = Duration::from_secs(15 * 60);
const SHUTDOWN_GRACE: Duration = Duration::from_millis(500);
const TMP_MAX_AGE: Duration = Duration::from_secs(24 * 3600);
const TRACE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);

pub struct Config {
    pub home: PathBuf,
}

/// A device that can be connected, as discovery sees it before any connection is made.
#[derive(Debug, Clone, PartialEq)]
pub struct Listed {
    pub id: String,
    pub model: String,
    pub os: Option<String>,
    pub tier: String,
    pub capabilities: Capabilities,
}

/// Where devices come from: adb for Android, a fake in tests.
#[async_trait]
pub trait DeviceSource: Send + Sync {
    async fn list(&self) -> Result<Vec<Listed>, Error>;
    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error>;

    /// Connects working on `display`. Only sources that can host a virtual display say yes to `Hidden`.
    async fn connect_on(&self, id: &str, display: DisplayMode) -> Result<Arc<dyn Device>, Error> {
        match display {
            DisplayMode::Main => self.connect(id).await,
            DisplayMode::Hidden => Err(Error::Unsupported {
                op: Op::Display,
                reason: "a hidden display exists only on Android 13 and later",
            }),
        }
    }

    /// `list` without the devices that an entry of `denied` (the device list's deny entries) names by id, which are
    /// not asked anything (no `getprop`), so a phone the list names by id is never read.
    async fn list_except(&self, denied: &[String]) -> Result<Vec<Listed>, Error> {
        let mut list = self.list().await?;
        list.retain(|l| !policy::denies_id(denied, &l.id));
        Ok(list)
    }

    /// The devices to look through when a request names `requested` (`None`: no device named). A source that
    /// owns several backends asks only the ones that can hold that device.
    async fn list_for(&self, requested: Option<&str>) -> Result<Vec<Listed>, Error> {
        let _ = requested;
        self.list().await
    }

    /// The devices that are off and that this source can start (simulators that are shut down), the ones to
    /// prefer first. A source that cannot say has none.
    async fn off(&self) -> Vec<DeviceSummary> {
        Vec::new()
    }

    /// Starts the device called `name` (see [`named`]) and returns it once it is listed, under the id it is listed
    /// with. `None` when this source has no device by that name that it can start.
    async fn boot(&self, name: &str) -> Result<Option<Booted>, Error> {
        let _ = name;
        Ok(None)
    }

    /// Why this device cannot be driven yet, when that is known without connecting (an iPhone with no signing
    /// team). With several devices and none named, only devices without a blocker are candidates for the default.
    fn blocker(&self, _id: &str) -> Option<&'static str> {
        None
    }

    /// The phone's own serial number (Android's `ro.serialno`), the same over USB and Wi-Fi, which the device list
    /// matches. `None` for a device that has no such serial.
    async fn hardware_serial(&self, id: &str) -> Result<Option<String>, Error> {
        let _ = id;
        Ok(None)
    }
}

/// What `boot` started, or found on.
pub struct Booted {
    pub device: DeviceSummary,
    /// It was on before the call.
    pub already: bool,
}

/// Whether `name` is this device's id (with or without its prefix) or its model in any case.
pub fn named(name: &str, id: &str, model: &str) -> bool {
    names(id, name) || model.eq_ignore_ascii_case(name)
}

pub(crate) fn names(id: &str, requested: &str) -> bool {
    id == requested || id.strip_suffix(requested).is_some_and(|p| p.ends_with(':'))
}

pub struct Daemon {
    source: Arc<policy::Gated>,
    home: PathBuf,
    actors: Mutex<HashMap<String, actor::Handle>>,
    traces: Traces,
    told: Mutex<HashMap<String, String>>,
    secrets: Arc<secrets::Secrets>,
    ledger: Arc<ledger::Ledger>,
    scope: Option<Arc<AppScope>>,
    agents: Mutex<std::collections::HashSet<String>>,
    connections: AtomicUsize,
    greeted: AtomicUsize,
    last_activity: Mutex<Instant>,
    stop: Notify,
}

impl Daemon {
    pub fn new(source: Arc<dyn DeviceSource>, config: Config) -> Arc<Daemon> {
        Self::with_trace(source, config, TraceOptions::default())
    }

    pub fn with_trace(
        source: Arc<dyn DeviceSource>,
        config: Config,
        trace: TraceOptions,
    ) -> Arc<Daemon> {
        Self::start(
            source,
            config,
            trace,
            Vec::new(),
            policy::Policy::default(),
            None,
        )
    }

    /// `apps_under_test` are packages the person named outside the tools; on a physical device they count as theirs
    /// for `apps clear`, `uninstall`, `grant`, `revoke` and `launch --grant`, like an app phonedriver installed.
    /// `app_scope` limits every device to those apps (decision D102).
    pub fn start(
        source: Arc<dyn DeviceSource>,
        config: Config,
        trace: TraceOptions,
        apps_under_test: Vec<String>,
        policy: policy::Policy,
        app_scope: Option<AppScope>,
    ) -> Arc<Daemon> {
        trace::clean_older_than(&config.home.join("tmp"), TMP_MAX_AGE);
        trace::clean_older_than(&config.home.join("traces"), TRACE_MAX_AGE);
        Arc::new(Daemon {
            source: Arc::new(policy::Gated::new(source, policy)),
            traces: Traces::new(config.home.join("traces"), trace),
            ledger: Arc::new(
                ledger::Ledger::new(&config.home).with_apps_under_test(apps_under_test),
            ),
            home: config.home,
            scope: app_scope.map(Arc::new),
            actors: Mutex::default(),
            told: Mutex::default(),
            secrets: Arc::default(),
            agents: Mutex::default(),
            connections: AtomicUsize::new(0),
            greeted: AtomicUsize::new(0),
            last_activity: Mutex::new(Instant::now()),
            stop: Notify::new(),
        })
    }

    pub fn shutdown(&self) {
        self.stop.notify_one();
    }

    fn touch(&self) {
        *self.last_activity.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
    }

    /// One request, in process. The socket server is a transport around this.
    pub async fn call(
        &self,
        session: &str,
        method: &str,
        params: Value,
    ) -> Result<Value, RpcError> {
        self.touch();
        let started = Instant::now();
        let carried = secrets::carried(method, &params);
        self.secrets.with(carried.clone());
        let (mut result, phases) = PHASES
            .scope(Default::default(), async {
                let result = self.route(session, method, &params).await;
                (result, PHASES.with(|p| p.take()))
            })
            .await;
        let secrets = self.secrets.with(carried);
        secrets::hide(&mut result, &secrets);
        self.traces.record(
            session,
            method,
            &params,
            &result,
            started.elapsed(),
            &phases,
            &secrets,
        );
        self.touch();
        result
    }

    async fn route(&self, session: &str, method: &str, params: &Value) -> Result<Value, RpcError> {
        match method {
            "hello" => {
                let version = params["version"].as_str().unwrap_or_default();
                if version != VERSION {
                    let others = self.greeted.load(Ordering::SeqCst);
                    let hint = if others == 0 {
                        "The CLI restarts the daemon on a version mismatch.".to_string()
                    } else {
                        format!(
                            "Another client of phonedriver {VERSION} is using this daemon (an MCP server in an editor, or another terminal). Run the same version in both, or run `phonedriver daemon stop` to end that daemon and the sessions it holds."
                        )
                    };
                    return Err(RpcError::new(
                        rpc::VERSION_MISMATCH,
                        format!("daemon is version {VERSION}, client is {version}"),
                        json!({"code_name": "version_mismatch", "daemon_version": VERSION,
                               "others": others, "hint": hint}),
                    ));
                }
                let wanted = params["app_scope"].as_str().unwrap_or_default();
                let held = self.scope.as_ref().map(|s| s.list());
                if !wanted.is_empty() && held.as_deref() != Some(wanted) {
                    return Err(RpcError::new(
                        rpc::SCOPE_MISMATCH,
                        match held {
                            Some(held) => format!(
                                "the daemon running was started with the app scope {held}, and this client asks for {wanted}"
                            ),
                            None => format!(
                                "the daemon running was started without an app scope, and this client asks for {wanted}"
                            ),
                        },
                        json!({"code_name": "scope_mismatch",
                               "hint": "Run `phonedriver daemon stop`, then try again: the next command starts a daemon with the scope this client has."}),
                    ));
                }
                if params["client"] == "mcp" {
                    self.agents
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(session.to_string());
                }
                Ok(json!({"version": VERSION, "pid": std::process::id()}))
            }
            "mask" => Ok(json!({"text": params["text"]})),
            "shutdown" => {
                self.shutdown();
                Ok(json!({"pid": std::process::id()}))
            }
            "devices" => Ok(self.devices().await?),
            "device" => Ok(json!({"device": self.resolve(params["device"].as_str()).await?})),
            "hidden" => {
                let extra: Vec<String> = params["ids"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                let hidden = self.source.hidden(&extra).await?;
                Ok(json!({"count": hidden.count, "ids": hidden.ids}))
            }
            "boot" => Ok(self.boot(params["name"].as_str()).await?),
            "told" => {
                let line = params["line"].as_str().unwrap_or_default().to_string();
                let last = self
                    .told
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(session.to_string(), line.clone());
                Ok(json!({"same": last.as_deref() == Some(line.as_str())}))
            }
            "release" | "snapshot" | "screenshot" | "act" | "wait" | "check" | "batch" | "find"
            | "apps" | "app" | "clipboard" | "health" | "shell" | "setting" | "logs"
            | "network" | "notifications" | "page" | "record" | "journal" | "build" | "files" => {
                let common: Common = serde_json::from_value(if params.is_null() {
                    json!({})
                } else {
                    params.clone()
                })
                .map_err(|e| RpcError::invalid_params(e.to_string()))?;
                if method == "release" && common.device.is_none() {
                    return Ok(self.release_everywhere(session).await);
                }
                let call = match method {
                    "release" => Call::Release,
                    "snapshot" => Call::Snapshot(rpc::params(params)?),
                    "screenshot" => Call::Screenshot(rpc::params(params)?),
                    "find" => Call::Find(rpc::params(params)?),
                    "apps" => Call::Apps,
                    "app" => Call::App(rpc::params(params)?),
                    "health" => Call::Health,
                    "journal" => Call::Journal(rpc::params(params)?),
                    "build" => Call::Build(rpc::params(params)?),
                    "clipboard" => Call::Clipboard(rpc::params(params)?),
                    "shell" => {
                        let p: ShellParams = rpc::params(params)?;
                        if p.time_limit_s.is_some_and(|s| s > MAX_TIME_LIMIT_S) {
                            return Err(RpcError::invalid_params(format!(
                                "a shell command can run for at most {MAX_TIME_LIMIT_S} seconds; lower `time_limit_s`"
                            )));
                        }
                        Call::Shell(p)
                    }
                    "logs" => Call::Logs(rpc::params(params)?),
                    "network" => Call::Network,
                    "notifications" => Call::Notifications(rpc::params(params)?),
                    "files" => {
                        let p: crate::wire::FilesParams = rpc::params(params)?;
                        let named = [&p.push, &p.pull, &p.list];
                        if named.iter().filter(|n| n.is_some()).count() > 1 {
                            return Err(RpcError::invalid_params(
                                "files takes one of `push` (a file in the allowed folder), `pull` (a path under the device's Download, Pictures or DCIM) or `list` (one of those folders); with none it lists all three",
                            ));
                        }
                        Call::Files(p)
                    }
                    "page" => Call::Page(rpc::params(params)?),
                    "record" => {
                        let p: RecordParams = rpc::params(params)?;
                        if p.path.as_ref().is_some_and(|path| !path.is_absolute()) {
                            return Err(RpcError::invalid_params(
                                "record needs an absolute `path`",
                            ));
                        }
                        Call::Record(p)
                    }
                    "setting" => {
                        let p: SettingParams = rpc::params(params)?;
                        let asked = [
                            p.find.is_some(),
                            p.key.is_some(),
                            p.values.as_ref().is_some_and(|v| !v.is_empty()),
                            p.open.is_some(),
                            p.restore.is_some(),
                        ];
                        if asked.iter().filter(|a| **a).count() != 1 {
                            return Err(RpcError::invalid_params(
                                "setting needs one of `find` (words), `key` (with `value` to write), `values` ({key: value}) or `open` (a screen's key)",
                            ));
                        }
                        Call::Setting(p)
                    }
                    "act" => {
                        let act: ActParams = rpc::known(params).map_err(|e| match e {
                            rpc::Unknown::Parse(e) => act_error(&e, params),
                            rpc::Unknown::Keys(why) => RpcError::invalid_params(why),
                        })?;
                        if let Action::Gesture { fingers } = &act.action
                            && (fingers.is_empty() || fingers.iter().any(|f| f.len() < 2))
                        {
                            return Err(RpcError::invalid_params(
                                "each finger needs at least two points: where it lands and where it lifts",
                            ));
                        }
                        if let Some(why) = act.action.refusal() {
                            return Err(RpcError::invalid_params(why));
                        }
                        Call::Act(act)
                    }
                    "batch" => {
                        let batch: BatchParams = rpc::params(params)?;
                        if batch.steps.is_empty() {
                            return Err(RpcError::invalid_params(
                                "a batch needs at least one step",
                            ));
                        }
                        for (i, raw) in batch.steps.iter().enumerate() {
                            let step = Step::parse(raw).map_err(|why| {
                                RpcError::invalid_params(format!("step {}: {why}", i + 1))
                            })?;
                            if let Step::Act(action) = &step
                                && let Some(why) = action.refusal()
                            {
                                return Err(RpcError::invalid_params(format!(
                                    "step {}: {why}",
                                    i + 1
                                )));
                            }
                            if let Step::Act(Action::Gesture { fingers }) = &step
                                && (fingers.is_empty() || fingers.iter().any(|f| f.len() < 2))
                            {
                                return Err(RpcError::invalid_params(format!(
                                    "step {}: each finger needs at least two points: where it lands and where it lifts",
                                    i + 1
                                )));
                            }
                        }
                        Call::Batch(batch)
                    }
                    "check" => Call::Check(rpc::params(params)?),
                    _ => Call::Wait(rpc::params(params)?),
                };
                let looked = Instant::now();
                let id = self.resolve(common.device.as_deref()).await;
                let _ = PHASES.try_with(|p| {
                    p.borrow_mut()
                        .push(("device", looked.elapsed().as_micros() as u32))
                });
                Ok(self
                    .send(&id?, session, common.share, common.display, call)
                    .await?)
            }
            _ => Err(RpcError::new(
                rpc::METHOD_NOT_FOUND,
                format!("unknown method {method}"),
                Value::Null,
            )),
        }
    }

    /// A request for a lane (`emulator-5554+a`, or `+a` for the only device) resolves its device and keeps the lane.
    async fn resolve(&self, requested: Option<&str>) -> Result<String, Error> {
        let Some((device, Some(lane))) = requested.map(lane_of) else {
            return self.resolve_device(requested).await;
        };
        let device = self
            .resolve_device((!device.is_empty()).then_some(device))
            .await?;
        Ok(format!("{device}+{lane}"))
    }

    async fn resolve_device(&self, requested: Option<&str>) -> Result<String, Error> {
        let list = self.source.list_for(requested).await?;
        let summaries = || {
            list.iter()
                .map(|l| DeviceSummary {
                    id: l.id.clone(),
                    model: l.model.clone(),
                    os: l.os.clone().unwrap_or_default(),
                })
                .collect::<Vec<_>>()
        };
        match requested {
            Some(r) => match list.iter().find(|l| names(&l.id, r)) {
                Some(l) => Ok(l.id.clone()),
                None => {
                    let list = self.source.list().await?;
                    Err(Error::NoDevice {
                        requested: Some(r.to_string()),
                        connected: list
                            .iter()
                            .map(|l| DeviceSummary {
                                id: l.id.clone(),
                                model: l.model.clone(),
                                os: l.os.clone().unwrap_or_default(),
                            })
                            .collect(),
                        unasked: vec![],
                        off: vec![],
                    })
                }
            },
            None => match list.as_slice() {
                [] => Err(Error::NoDevice {
                    requested: None,
                    connected: vec![],
                    unasked: vec![],
                    off: self.source.off().await,
                }),
                [one] => Ok(one.id.clone()),
                _ => {
                    let ready: Vec<&Listed> = list
                        .iter()
                        .filter(|l| self.source.blocker(&l.id).is_none())
                        .collect();
                    match ready.as_slice() {
                        [one] => Ok(one.id.clone()),
                        _ => Err(Error::AmbiguousDevice {
                            connected: summaries(),
                        }),
                    }
                }
            },
        }
    }

    fn actor(&self, id: &str) -> mpsc::Sender<Job> {
        let spawn = || {
            actor::spawn(
                id.to_string(),
                self.source.clone(),
                self.home.clone(),
                self.secrets.clone(),
                self.ledger.clone(),
                self.scope.clone(),
            )
        };
        let mut actors = self.actors.lock().unwrap_or_else(|e| e.into_inner());
        let handle = actors.entry(id.to_string()).or_insert_with(spawn);
        if handle.jobs.is_closed() {
            *handle = spawn();
        }
        handle.jobs.clone()
    }

    async fn send(
        &self,
        id: &str,
        session: &str,
        share: bool,
        display: DisplayMode,
        call: Call,
    ) -> Result<Value, Error> {
        let (reply, rx) = oneshot::channel();
        let gone = || Error::DeviceGone { id: id.to_string() };
        let display = match lane_of(id) {
            (_, Some(_)) => DisplayMode::Hidden,
            _ => display,
        };
        let terminal = !self
            .agents
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(session);
        self.actor(id)
            .send(Job {
                session: session.to_string(),
                terminal,
                share,
                display,
                call,
                reply,
            })
            .await
            .map_err(|_| gone())?;
        let (result, phases) = rx
            .await
            .map_err(|_| Error::Crashed { id: id.to_string() })?;
        let _ = PHASES.try_with(|p| p.borrow_mut().extend(phases));
        result
    }

    /// Without a name, the devices that are off. With one (a model, any case, or an id), that device started,
    /// or `already` when a connected device has the name.
    async fn boot(&self, name: Option<&str>) -> Result<Value, Error> {
        let off = self.source.off().await;
        let Some(name) = name else {
            return Ok(json!({"off": off}));
        };
        let connected = self.source.list().await?;
        let booted = match connected.iter().find(|l| named(name, &l.id, &l.model)) {
            Some(on) => Some(Booted {
                device: DeviceSummary {
                    id: on.id.clone(),
                    model: on.model.clone(),
                    os: on.os.clone().unwrap_or_default(),
                },
                already: true,
            }),
            None => self.source.boot(name).await?,
        };
        let Some(Booted { device, already }) = booted else {
            let startable = match off.as_slice() {
                [] => "nothing can be started".into(),
                off => format!("can be started: {}", can_be_started(off)),
            };
            return Err(Error::Refused {
                detail: format!("no device named \"{name}\" is off; {startable}"),
            });
        };
        let mut reply = json!(device);
        if already {
            reply["already"] = json!(true);
        }
        Ok(reply)
    }

    async fn devices(&self) -> Result<Value, Error> {
        let mut out = Vec::new();
        for l in self.source.list().await? {
            let mut ids: Vec<String> = self
                .actors
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .keys()
                .filter(|id| lane_of(id).0 == l.id)
                .cloned()
                .collect();
            ids.sort();
            if ids.first() != Some(&l.id) {
                ids.insert(0, l.id.clone());
            }
            for id in ids {
                let held = self
                    .actors
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(&id)
                    .filter(|a| !a.jobs.is_closed())
                    .map(|a| a.held.borrow().clone())
                    .unwrap_or_default();
                let hidden_display = held.hidden_display;
                if id != l.id && held.session.is_none() && hidden_display.is_none() {
                    continue;
                }
                out.push(DeviceEntry {
                    id,
                    model: l.model.clone(),
                    os: l.os.clone().unwrap_or_default(),
                    tier: l.tier.clone(),
                    capabilities: l.capabilities,
                    lease_live: held.session.is_some()
                        && (!held.terminal
                            || held
                                .last_call
                                .is_some_and(|t| t.elapsed() < actor::TERMINAL_HOLD)),
                    leased_by: held.session,
                    lease_idle_s: held.last_call.map(|t| t.elapsed().as_secs()),
                    hidden_display,
                });
            }
        }
        Ok(serde_json::to_value(out).unwrap_or(Value::Null))
    }

    /// Serves until `shutdown`, or until 15 minutes pass with no client connected and no request.
    pub async fn serve(self: Arc<Self>, path: PathBuf) -> std::io::Result<()> {
        let listener = socket::listen(&path).await?;
        self.serve_on(listener).await
    }

    pub async fn serve_on(self: Arc<Self>, listener: socket::Listener) -> std::io::Result<()> {
        use interprocess::local_socket::traits::tokio::Listener as _;
        loop {
            let deadline =
                *self.last_activity.lock().unwrap_or_else(|e| e.into_inner()) + IDLE_EXIT;
            tokio::select! {
                conn = listener.accept() => {
                    let Ok(stream) = conn else { continue };
                    self.touch();
                    let me = self.clone();
                    me.connections.fetch_add(1, Ordering::SeqCst);
                    tokio::spawn(async move {
                        me.clone().connection(stream).await;
                        me.connections.fetch_sub(1, Ordering::SeqCst);
                        me.touch();
                    });
                }
                () = self.stop.notified() => break,
                () = tokio::time::sleep_until(deadline) => {
                    let idle = self.connections.load(Ordering::SeqCst) == 0
                        && Instant::now() >= deadline;
                    if idle { break }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
        let until = Instant::now() + SHUTDOWN_GRACE;
        while self.connections.load(Ordering::SeqCst) > 0 && Instant::now() < until {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok(())
    }

    async fn connection(self: Arc<Self>, stream: socket::Stream) {
        let (read, mut write) = split(stream);
        let mut lines = BufReader::new(read).lines();
        let mut mcp = false;
        let mut wants_progress = false;
        let mut sessions: Vec<String> = Vec::new();
        let mut greeted = false;
        while let Ok(Some(line)) = lines.next_line().await {
            let (id, result) = match rpc::parse(&line) {
                Ok(req) => {
                    let session = req.params["session"]
                        .as_str()
                        .unwrap_or("default")
                        .to_string();
                    if req.method == "hello" && req.params["client"] == "mcp" {
                        mcp = true;
                    }
                    if req.method == "hello" {
                        wants_progress = req.params["progress"] == true;
                    }
                    if let Some(named) = req.params["session"].as_str()
                        && !sessions.iter().any(|s| s == named)
                    {
                        sessions.push(named.to_string());
                    }
                    let mut said = progress::subscribe();
                    let call = self.call(&session, &req.method, req.params);
                    tokio::pin!(call);
                    let result = loop {
                        tokio::select! {
                            result = &mut call => break result,
                            Ok(message) = said.recv(), if wants_progress => {
                                let note = json!({"jsonrpc": "2.0", "method": "progress", "params": {"message": message}});
                                if write.write_all(format!("{note}\n").as_bytes()).await.is_err() {
                                    break call.await;
                                }
                            }
                        }
                    };
                    if req.method == "hello" && result.is_ok() && !greeted {
                        greeted = true;
                        self.greeted.fetch_add(1, Ordering::SeqCst);
                    }
                    (req.id, result)
                }
                Err(e) => (Value::Null, Err(e)),
            };
            let out = format!("{}\n", rpc::reply(&id, result));
            if write.write_all(out.as_bytes()).await.is_err() {
                break;
            }
        }
        if greeted {
            self.greeted.fetch_sub(1, Ordering::SeqCst);
        }
        if mcp {
            for session in sessions {
                self.forget(&session).await;
            }
        }
    }

    /// Ends a session that will not come back: an MCP server's name is its process id. An MCP client that exits or is
    /// killed never sends `release`, and its lease would otherwise block the next agent until it lapses. A command
    /// line session is kept, because each command is its own connection and `save` comes after the run.
    async fn forget(&self, session: &str) {
        self.send_to_all(session, || Call::Forget).await;
        self.traces.close(session);
        self.told
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(session);
        self.agents
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(session);
    }

    /// Ends the leases a session holds on every device.
    /// Releases every device the session holds. The reply lists the settings still changed on them, as a release
    /// of one device does.
    async fn release_everywhere(&self, session: &str) -> Value {
        let changed: Vec<Value> = self
            .send_to_all(session, || Call::Release)
            .await
            .iter()
            .filter_map(|r| r["settings_changed"].as_array())
            .flatten()
            .cloned()
            .collect();
        if changed.is_empty() {
            json!({})
        } else {
            json!({"settings_changed": changed})
        }
    }

    async fn send_to_all(&self, session: &str, call: impl Fn() -> Call) -> Vec<Value> {
        let ids: Vec<String> = self
            .actors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .cloned()
            .collect();
        let mut replies = Vec::new();
        for id in ids {
            if let Ok(reply) = self
                .send(&id, session, false, DisplayMode::Main, call())
                .await
            {
                replies.push(reply);
            }
        }
        replies
    }
}

fn act_error(e: &serde_json::Error, params: &Value) -> RpcError {
    let kind = params["action"]["kind"].as_str().unwrap_or_default();
    let message = e.to_string();
    if message.starts_with(&format!("unknown variant `{kind}`")) && kind.len() <= 32 {
        return RpcError::invalid_params(format!(
            "this daemon does not know the action `{kind}`: it is older than the client, so run `phonedriver daemon stop` and repeat the command to start a new one"
        ));
    }
    if message.starts_with("unknown variant")
        && message.contains("expected one of")
        && kind.len() <= 32
    {
        let expected = message.split("expected one of").nth(1).unwrap_or_default();
        return RpcError::invalid_params(format!(
            "the {kind} action has a value that is not allowed; expected one of{}",
            expected.split(" at line").next().unwrap_or_default()
        ));
    }
    RpcError::invalid_params(format!(
        "the {kind} action is malformed: {}",
        crate::wire::malformed_reason(e)
    ))
}
