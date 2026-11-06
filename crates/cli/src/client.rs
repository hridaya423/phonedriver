//! The CLI's side of the daemon socket, including starting the daemon on demand (architecture 5).

use std::path::{Path, PathBuf};
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
        Self::connect_as(session, "cli").await
    }

    /// Like `connect`, naming the kind of client in `hello`. A daemon releases the sessions of an `mcp` client when
    /// its connection closes, because an MCP server keeps one connection for its whole life.
    pub async fn connect_as(session: String, kind: &str) -> Result<Client, Failure> {
        let path = socket::socket_path();
        let mut client = match Self::open(&path, &session).await {
            Ok(c) => c,
            Err(_) => {
                start_daemon().await?;
                Self::open_with_retry(&path, &session).await?
            }
        };
        match client
            .call_raw("hello", json!({"version": VERSION, "client": kind}))
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
                    .call_raw("hello", json!({"version": VERSION, "client": kind}))
                    .await?;
                Ok(fresh)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Connects to a daemon that is already listening at `path`, without starting one.
    pub async fn connect_to(path: &Path, session: &str) -> Result<Client, Failure> {
        Self::connect_to_as(path, session, "bench").await
    }

    pub async fn connect_to_as(path: &Path, session: &str, kind: &str) -> Result<Client, Failure> {
        let mut client = Self::open(path, session)
            .await
            .map_err(|e| Failure::Local(format!("no daemon at {}: {e}", path.display())))?;
        client
            .call_raw("hello", json!({"version": VERSION, "client": kind}))
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
                    let remote = std::env::var_os("PHONE_USE_SOCKET").map_or_else(String::new, |v| {
                        format!(
                            " (PHONE_USE_SOCKET={}; if it is an ssh forward, check that the ssh process is still running)",
                            v.to_string_lossy()
                        )
                    });
                    return Err(Failure::Local(format!(
                        "the daemon did not start listening on {}: {e}{remote}",
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

/// Stops the daemon listening at `path` without starting one, and returns its pid; `None` when nothing listens. The
/// daemon ends the iOS runners it started when it exits, so this waits for the process itself, not just its socket.
pub async fn stop_daemon(path: &Path) -> Result<Option<u32>, Failure> {
    let Ok(mut client) = Client::open(path, "stop").await else {
        return Ok(None);
    };
    let reply = client.call_raw("shutdown", json!({})).await?;
    drop(client);
    wait_until_gone(path).await;
    if socket::connect(path).await.is_ok() {
        return Err(Failure::Local(format!(
            "the daemon at {} did not stop within 3 s",
            path.display()
        )));
    }
    let pid = reply["pid"].as_u64().and_then(|p| u32::try_from(p).ok());
    if let Some(pid) = pid {
        wait_until_exited(pid).await;
    }
    Ok(Some(pid.unwrap_or_default()))
}

#[cfg(unix)]
async fn wait_until_exited(pid: u32) {
    for _ in 0..250 {
        let alive = tokio::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .is_ok_and(|s| s.success());
        if !alive {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[cfg(not(unix))]
async fn wait_until_exited(_pid: u32) {}

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
    let log = std::fs::create_dir_all(socket::home()).and_then(|()| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path())
    });
    let ready = spawn_daemon(&exe, log)
        .map_err(|e| Failure::Local(format!("cannot start the daemon: {e}")))?;
    let mut lines = BufReader::new(ready).lines();
    let _ = timeout(READY_WAIT, lines.next_line()).await;
    Ok(())
}

#[cfg(unix)]
fn spawn_daemon(
    exe: &Path,
    log: std::io::Result<std::fs::File>,
) -> std::io::Result<tokio::process::ChildStdout> {
    use std::process::Stdio;
    let mut child = tokio::process::Command::new(exe)
        .arg("daemon")
        .arg("--detach")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(log.map_or_else(|_| Stdio::null(), Stdio::from))
        .process_group(0)
        .spawn()?;
    child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("no stdout"))
}

#[cfg(windows)]
fn spawn_daemon(
    exe: &Path,
    log: std::io::Result<std::fs::File>,
) -> std::io::Result<tokio::fs::File> {
    let log = match log {
        Ok(file) => file,
        Err(_) => std::fs::OpenOptions::new().write(true).open("NUL")?,
    };
    crate::detach::spawn_daemon(exe, &log).map(tokio::fs::File::from_std)
}
