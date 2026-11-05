//! The daemon: owns every device connection and the sessions that use them (architecture 5).
//!
//! Adding a verb: add its params and result to `wire.rs`, a `Call` variant and an actor method in
//! `actor.rs`, and a route in `Daemon::route`. The CLI side is a clap variant in `commands.rs`.

pub mod actor;
pub mod android;
pub mod gestures;
pub mod ios;
pub mod rpc;
pub mod session;
pub mod socket;
pub mod sources;
pub mod trace;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use phone_use_core::device::{Capabilities, Device};
use phone_use_core::error::{DeviceSummary, Error};
use serde_json::{Value, json};
use tokio::io::split;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Notify, mpsc, oneshot};
use tokio::time::Instant;

use self::actor::{Call, Job};
use self::rpc::RpcError;
use self::trace::Traces;
use crate::wire::{Common, DeviceEntry, VERSION};

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
}

pub struct Daemon {
    source: Arc<dyn DeviceSource>,
    home: PathBuf,
    actors: Mutex<HashMap<String, mpsc::Sender<Job>>>,
    traces: Traces,
    connections: AtomicUsize,
    last_activity: Mutex<Instant>,
    stop: Notify,
}

impl Daemon {
    pub fn new(source: Arc<dyn DeviceSource>, config: Config) -> Arc<Daemon> {
        trace::clean_older_than(&config.home.join("tmp"), TMP_MAX_AGE);
        trace::clean_older_than(&config.home.join("traces"), TRACE_MAX_AGE);
        Arc::new(Daemon {
            source,
            traces: Traces::new(config.home.join("traces")),
            home: config.home,
            actors: Mutex::default(),
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
        let request = trace::describe(method, &params);
        let result = self.route(session, method, &params).await;
        self.traces
            .record(session, method, &request, &result, started.elapsed());
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
                Ok(json!({"version": VERSION, "pid": std::process::id()}))
            }
            "shutdown" => {
                self.shutdown();
                Ok(json!({}))
            }
            "devices" => Ok(self.devices().await?),
            "release" | "snapshot" | "screenshot" | "act" | "wait" | "open" => {
                let common: Common = rpc::params(params)?;
                let call = match method {
                    "release" => Call::Release,
                    "open" => Call::Open(rpc::params(params)?),
                    "snapshot" => Call::Snapshot(rpc::params(params)?),
                    "screenshot" => Call::Screenshot,
                    "act" => Call::Act(
                        rpc::params(params)
                            .map_err(|_| RpcError::invalid_params("the action is malformed"))?,
                    ),
                    _ => Call::Wait(rpc::params(params)?),
                };
                let id = self.resolve(common.device.as_deref()).await?;
                Ok(self.send(&id, session, common.share, call).await?)
            }
            _ => Err(RpcError::new(
                rpc::METHOD_NOT_FOUND,
                format!("unknown method {method}"),
                Value::Null,
            )),
        }
    }

    async fn resolve(&self, requested: Option<&str>) -> Result<String, Error> {
        let list = self.source.list().await?;
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
            Some(r) => list
                .iter()
                .find(|l| l.id == r || l.id.ends_with(&format!(":{r}")))
                .map(|l| l.id.clone())
                .ok_or_else(|| Error::NoDevice {
                    connected: summaries(),
                }),
            None => match list.as_slice() {
                [] => Err(Error::NoDevice { connected: vec![] }),
                [one] => Ok(one.id.clone()),
                _ => Err(Error::AmbiguousDevice {
                    connected: summaries(),
                }),
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

    async fn send(&self, id: &str, session: &str, share: bool, call: Call) -> Result<Value, Error> {
        let (reply, rx) = oneshot::channel();
        let gone = || Error::DeviceGone { id: id.to_string() };
        self.actor(id)
            .send(Job {
                session: session.to_string(),
                share,
                call,
                reply,
            })
            .await
            .map_err(|_| gone())?;
        rx.await.map_err(|_| gone())?
    }

    async fn devices(&self) -> Result<Value, Error> {
        let mut out = Vec::new();
        for l in self.source.list().await? {
            let existing = self
                .actors
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains_key(&l.id);
            let leased_by = if existing {
                self.send(&l.id, "", false, Call::Leased)
                    .await?
                    .as_str()
                    .map(str::to_string)
            } else {
                None
            };
            out.push(DeviceEntry {
                id: l.id,
                model: l.model,
                os: l.os.unwrap_or_default(),
                tier: l.tier,
                capabilities: l.capabilities,
                leased_by,
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
        while let Ok(Some(line)) = lines.next_line().await {
            let (id, result) = match rpc::parse(&line) {
                Ok(req) => {
                    let session = req.params["session"]
                        .as_str()
                        .unwrap_or("default")
                        .to_string();
                    (req.id, self.call(&session, &req.method, req.params).await)
                }
                Err(e) => (Value::Null, Err(e)),
            };
            let out = format!("{}\n", rpc::reply(&id, result));
            if write.write_all(out.as_bytes()).await.is_err() {
                break;
            }
        }
    }
}
