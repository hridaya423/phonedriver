//! The CLI's side of the daemon socket, including starting the daemon on demand (architecture 5).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf, split};
use tokio::time::timeout;

use crate::daemon::rpc::{RpcError, VERSION_MISMATCH};
use crate::daemon::socket::{self, Stream};
use crate::wire::VERSION;

const READY_WAIT: Duration = Duration::from_secs(3);

/// Why a command failed: the daemon said so, or the CLI could not reach it.
#[derive(Debug)]
pub enum Failure {
    Rpc(RpcError),
    Local(String),
}

impl From<RpcError> for Failure {
    fn from(e: RpcError) -> Failure {
        Failure::Rpc(e)
    }
}

impl Failure {
    pub fn message(&self) -> String {
        match self {
            Failure::Rpc(e) => e.message.clone(),
            Failure::Local(m) => m.clone(),
        }
    }

    pub fn hint(&self) -> Option<String> {
        match self {
            Failure::Rpc(e) => e.data["hint"].as_str().map(str::to_string),
            Failure::Local(_) => None,
        }
    }

    /// `needs_user` is 3, `device_gone` and `no_device` are 4, everything else is 1.
    pub fn exit_code(&self) -> u8 {
        match self {
            Failure::Rpc(e) => match e.code {
                -32005 => 3,
                -32003 | -32001 => 4,
                _ => 1,
            },
            Failure::Local(_) => 1,
        }
    }
}

pub struct Client {
    read: BufReader<ReadHalf<Stream>>,
    write: WriteHalf<Stream>,
    next: u64,
    session: String,
}

impl Client {
    /// Connects to the daemon, starting it if it is not running, and checks that its version matches.
    pub async fn connect(session: String) -> Result<Client, Failure> {
        let path = socket::socket_path();
        let mut client = match Self::open(&path, &session).await {
            Ok(c) => c,
            Err(_) => {
                start_daemon().await?;
                Self::open_with_retry(&path, &session).await?
            }
        };
        match client
            .call_raw("hello", json!({"version": VERSION, "client": "cli"}))
            .await
        {
            Ok(_) => Ok(client),
            Err(e) if e.code == VERSION_MISMATCH => {
                let _ = client.call_raw("shutdown", json!({})).await;
                drop(client);
                wait_until_gone(&path).await;
                start_daemon().await?;
                let mut fresh = Self::open_with_retry(&path, &session).await?;
                fresh
                    .call_raw("hello", json!({"version": VERSION, "client": "cli"}))
                    .await?;
                Ok(fresh)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Connects to a daemon that is already listening at `path`, without starting one.
    pub async fn connect_to(path: &Path, session: &str) -> Result<Client, Failure> {
        let mut client = Self::open(path, session)
            .await
            .map_err(|e| Failure::Local(format!("no daemon at {}: {e}", path.display())))?;
        client
            .call_raw("hello", json!({"version": VERSION, "client": "bench"}))
            .await?;
        Ok(client)
    }

    async fn open(path: &Path, session: &str) -> std::io::Result<Client> {
        let (read, write) = split(socket::connect(path).await?);
        Ok(Client {
            read: BufReader::new(read),
            write,
            next: 1,
            session: session.to_string(),
        })
    }

    async fn open_with_retry(path: &Path, session: &str) -> Result<Client, Failure> {
        let deadline = tokio::time::Instant::now() + READY_WAIT;
        loop {
            match Self::open(path, session).await {
                Ok(c) => return Ok(c),
                Err(e) if tokio::time::Instant::now() >= deadline => {
                    return Err(Failure::Local(format!(
                        "the daemon did not start listening on {}: {e}",
                        path.display()
                    )));
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        }
    }

    pub async fn call(&mut self, method: &str, mut params: Value) -> Result<Value, Failure> {
        params["session"] = Value::String(self.session.clone());
        Ok(self.call_raw(method, params).await?)
    }

    async fn call_raw(&mut self, method: &str, params: Value) -> Result<Value, RpcError> {
        let id = self.next;
        self.next += 1;
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let lost =
            |e: std::io::Error| RpcError::new(-32603, format!("lost the daemon: {e}"), Value::Null);
        self.write
            .write_all(format!("{request}\n").as_bytes())
            .await
            .map_err(lost)?;
        let mut line = String::new();
        let n = self.read.read_line(&mut line).await.map_err(lost)?;
        if n == 0 {
            return Err(RpcError::new(
                -32603,
                "the daemon closed the connection",
                Value::Null,
            ));
        }
        let reply: Value = serde_json::from_str(&line).map_err(|e| {
            RpcError::new(
                -32603,
                format!("the daemon sent bad JSON: {e}"),
                Value::Null,
            )
        })?;
        match reply.get("error") {
            Some(e) => Err(RpcError::new(
                e["code"].as_i64().unwrap_or(-32603) as i32,
                e["message"].as_str().unwrap_or("error").to_string(),
                e["data"].clone(),
            )),
            None => Ok(reply["result"].clone()),
        }
    }
}

async fn wait_until_gone(path: &Path) {
    for _ in 0..150 {
        if socket::connect(path).await.is_err() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn log_path() -> PathBuf {
    socket::home().join("daemon.log")
}

/// Spawns `phone-use daemon --detach` and waits for its readiness line on a pipe, not for a fixed time.
async fn start_daemon() -> Result<(), Failure> {
    let exe = std::env::current_exe()
        .map_err(|e| Failure::Local(format!("cannot find the phone-use binary: {e}")))?;
    let log = std::fs::create_dir_all(socket::home())
        .and_then(|()| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path())
        })
        .map(Stdio::from)
        .unwrap_or_else(|_| Stdio::null());
    let mut cmd = tokio::process::Command::new(exe);
    cmd.arg("daemon")
        .arg("--detach")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(log);
    #[cfg(unix)]
    cmd.process_group(0);
    #[cfg(windows)]
    {
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| Failure::Local(format!("cannot start the daemon: {e}")))?;
    if let Some(out) = child.stdout.take() {
        let mut lines = BufReader::new(out).lines();
        let _ = timeout(READY_WAIT, lines.next_line()).await;
    }
    Ok(())
}
