//! The daemon: owns every device connection and the sessions that use them (architecture 5).
//!
//! Adding a verb: add its params and result to `wire.rs`, a `Call` variant and an actor method in
//! `actor.rs`, and a route in `Daemon::route`. The CLI side is a clap variant in `commands.rs`.

pub mod actor;
#[cfg(feature = "android")]
pub mod android;
pub mod checkbox;
pub mod guard;
pub mod ios;
pub mod progress;
pub mod rpc;
pub mod secret_field;
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
use phonedriver_core::device::{Capabilities, Device, DisplayMode};
use phonedriver_core::error::{DeviceSummary, Error, Op};
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
use crate::wire::{ActParams, Action, BatchParams, Common, DeviceEntry, Step, VERSION};

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

    /// The devices to look through when a request names `requested` (`None`: no device named). A source that
    /// owns several backends asks only the ones that can hold that device.
    async fn list_for(&self, requested: Option<&str>) -> Result<Vec<Listed>, Error> {
        let _ = requested;
        self.list().await
    }

    /// Why this device cannot be driven yet, when that is known without connecting (an iPhone with no signing
    /// team). With several devices and none named, only devices without a blocker are candidates for the default.
    fn blocker(&self, _id: &str) -> Option<&'static str> {
        None
    }
}

pub(crate) fn names(id: &str, requested: &str) -> bool {
    id == requested || id.strip_suffix(requested).is_some_and(|p| p.ends_with(':'))
}

pub struct Daemon {
    source: Arc<dyn DeviceSource>,
    home: PathBuf,
    actors: Mutex<HashMap<String, mpsc::Sender<Job>>>,
    traces: Traces,
    told: Mutex<HashMap<String, String>>,
    agents: Mutex<std::collections::HashSet<String>>,
    connections: AtomicUsize,
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
        trace::clean_older_than(&config.home.join("tmp"), TMP_MAX_AGE);
        trace::clean_older_than(&config.home.join("traces"), TRACE_MAX_AGE);
        Arc::new(Daemon {
            source,
            traces: Traces::new(config.home.join("traces"), trace),
            home: config.home,
            actors: Mutex::default(),
            told: Mutex::default(),
            agents: Mutex::default(),
            connections: AtomicUsize::new(0),
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
        let (result, phases) = PHASES
            .scope(Default::default(), async {
                let result = self.route(session, method, &params).await;
                (result, PHASES.with(|p| p.take()))
            })
            .await;
        self.traces.record(
            session,
            method,
            &params,
            &result,
            started.elapsed(),
            &phases,
        );
        self.touch();
        result
    }

    async fn route(&self, session: &str, method: &str, params: &Value) -> Result<Value, RpcError> {
        match method {
            "hello" => {
                let version = params["version"].as_str().unwrap_or_default();
                if version != VERSION {
                    return Err(RpcError::new(
                        rpc::VERSION_MISMATCH,
                        format!("daemon is version {VERSION}, client is {version}"),
                        json!({"code_name": "version_mismatch", "daemon_version": VERSION,
                               "hint": "The CLI restarts the daemon on a version mismatch."}),
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
            "shutdown" => {
                self.shutdown();
                Ok(json!({"pid": std::process::id()}))
            }
            "devices" => Ok(self.devices().await?),
            "told" => {
                let line = params["line"].as_str().unwrap_or_default().to_string();
                let last = self
                    .told
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(session.to_string(), line.clone());
                Ok(json!({"same": last.as_deref() == Some(line.as_str())}))
            }
            "release" | "snapshot" | "screenshot" | "act" | "wait" | "batch" | "find" | "apps"
            | "app" | "clipboard" | "health" | "shell" => {
                let common: Common = rpc::params(params)?;
                if method == "release" && common.device.is_none() {
                    self.release_everywhere(session).await;
                    return Ok(json!({}));
                }
                let call = match method {
                    "release" => Call::Release,
                    "snapshot" => Call::Snapshot(rpc::params(params)?),
                    "screenshot" => Call::Screenshot(rpc::params(params)?),
                    "find" => Call::Find(rpc::params(params)?),
                    "apps" => Call::Apps,
                    "app" => Call::App(rpc::params(params)?),
                    "health" => Call::Health,
                    "clipboard" => Call::Clipboard(rpc::params(params)?),
                    "shell" => Call::Shell(rpc::params(params)?),
                    "act" => {
                        let act: ActParams = serde_json::from_value(params.clone())
                            .map_err(|e| act_error(&e, params))?;
                        if let Action::Gesture { fingers } = &act.action
                            && (fingers.is_empty() || fingers.iter().any(|f| f.len() < 2))
                        {
                            return Err(RpcError::invalid_params(
                                "each finger needs at least two points: where it lands and where it lifts",
                            ));
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
                    _ => Call::Wait(rpc::params(params)?),
                };
                let id = self.resolve(common.device.as_deref()).await?;
                Ok(self
                    .send(&id, session, common.share, common.display, call)
                    .await?)
            }
            _ => Err(RpcError::new(
                rpc::METHOD_NOT_FOUND,
                format!("unknown method {method}"),
                Value::Null,
            )),
        }
    }

    async fn resolve(&self, requested: Option<&str>) -> Result<String, Error> {
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
                    })
                }
            },
            None => match list.as_slice() {
                [] => Err(Error::NoDevice {
                    requested: None,
                    connected: vec![],
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
        self.actors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(id.to_string())
            .or_insert_with(|| actor::spawn(id.to_string(), self.source.clone(), self.home.clone()))
            .clone()
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
        let (result, phases) = rx.await.map_err(|_| gone())?;
        let _ = PHASES.try_with(|p| p.borrow_mut().extend(phases));
        result
    }

    async fn devices(&self) -> Result<Value, Error> {
        let mut out = Vec::new();
        for l in self.source.list().await? {
            let existing = self
                .actors
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains_key(&l.id);
            let lease = if existing {
                self.send(&l.id, "", false, DisplayMode::Main, Call::Leased)
                    .await?
            } else {
                Value::Null
            };
            let leased_by = lease["session"].as_str().map(str::to_string);
            let hidden_display = lease["hidden_display"].as_u64().map(|d| d as u32);
            let lease_idle_s = lease["idle_s"].as_u64();
            out.push(DeviceEntry {
                id: l.id,
                model: l.model,
                os: l.os.unwrap_or_default(),
                tier: l.tier,
                capabilities: l.capabilities,
                leased_by,
                lease_idle_s,
                hidden_display,
            });
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
                    (req.id, result)
                }
                Err(e) => (Value::Null, Err(e)),
            };
            let out = format!("{}\n", rpc::reply(&id, result));
            if write.write_all(out.as_bytes()).await.is_err() {
                break;
            }
        }
        if mcp {
            for session in sessions {
                self.release_everywhere(&session).await;
            }
        }
    }

    /// Ends the leases a session holds on every device. An MCP client that exits or is killed never sends `release`,
    /// and its lease would otherwise block the next agent until it lapses.
    async fn release_everywhere(&self, session: &str) {
        let ids: Vec<String> = self
            .actors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .cloned()
            .collect();
        for id in ids {
            let _ = self
                .send(&id, session, false, DisplayMode::Main, Call::Release)
                .await;
        }
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
