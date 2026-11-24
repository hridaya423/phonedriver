//! The CLI's side of the daemon socket, including starting the daemon on demand (architecture 5).

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Duration;

use phonedriver_core::scope::AppScope;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf, split};
use tokio::time::timeout;

use crate::daemon::rpc::{RpcError, VERSION_MISMATCH};
use crate::daemon::socket::{self, Stream};
use crate::wire::VERSION;

const READY_WAIT: Duration = Duration::from_secs(3);

tokio::task_local! {
    /// What the call running in this task is doing now. The MCP server sets it around a tool call and reads it for
    /// the progress notifications it sends (decision D124); nothing else sets it.
    pub static DOING: tokio::sync::watch::Sender<String>;
}

/// Says what a long call has reached, for whoever set `DOING`. `what` must hold no text a caller gave.
pub fn doing(what: String) {
    let _ = DOING.try_with(|d| d.send_replace(what));
}

/// A progress line from the daemon goes to whoever set `DOING`, and to stderr when no one did.
fn said(message: &str) {
    if DOING
        .try_with(|d| d.send_replace(message.to_string()))
        .is_err()
    {
        eprintln!("{message}")
    }
}

/// The `hello` params. The CLI asks for progress lines only when its stderr is a terminal: an agent that reads the
/// CLI's output through pipes gets none. An MCP server asks for them and passes them on as progress notifications
/// to a client that gave a progress token, so its streams stay with the protocol.
/// The session goes in too: the daemon registers the session named in `hello` as an agent's, and the calls that
/// follow carry this client's session, so the two must match.
fn hello(kind: &str, session: &str) -> Value {
    let progress = kind == "mcp" || kind == "cli" && std::io::stderr().is_terminal();
    json!({"version": VERSION, "client": kind, "progress": progress, "session": session, "app_scope": app_scope()})
}

/// The app scope this client's environment sets, normalized, so the daemon can refuse to serve a client that asked
/// for a scope it was not started with (decision D102). Text that names no app is sent as it is.
fn app_scope() -> String {
    let set = std::env::var("PHONEDRIVER_APP_SCOPE").unwrap_or_default();
    match AppScope::parse(&set) {
        Ok(Some(scope)) => scope.list(),
        Ok(None) => String::new(),
        Err(_) => set,
    }
}

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

    /// The screen a failed action came back with, under the heading a failed batch uses.
    pub fn screen(&self) -> Option<String> {
        let Failure::Rpc(e) = self else { return None };
        let text = e.data["screen"].as_str().filter(|t| !t.is_empty())?;
        Some(format!("screen now, against what you last saw:\n{text}"))
    }

    /// `needs_user` is 3, `device_gone` and `no_device` are 4, `needs_approval` is 5, everything else is 1.
    pub fn exit_code(&self) -> u8 {
        match self {
            Failure::Rpc(e) => match e.code {
                -32005 => 3,
                -32019 => 5,
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
        match client.call_raw("hello", hello(kind, &session)).await {
            Ok(_) => Ok(client),
            Err(e) if e.code == VERSION_MISMATCH && e.data["others"].as_u64().unwrap_or(0) == 0 => {
                let _ = client.call_raw("shutdown", json!({})).await;
                drop(client);
                wait_until_gone(&path).await;
                start_daemon().await?;
                let mut fresh = Self::open_with_retry(&path, &session).await?;
                fresh.call_raw("hello", hello(kind, &session)).await?;
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
        client.call_raw("hello", hello(kind, session)).await?;
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
                    let remote = std::env::var_os("PHONEDRIVER_SOCKET").map_or_else(String::new, |v| {
                        format!(
                            " (PHONEDRIVER_SOCKET={}; if it is an ssh forward, check that the ssh process is still running)",
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

    /// Records `line` as the device line this session was last shown and says whether it already was.
    pub async fn told(&mut self, line: &str) -> Result<bool, Failure> {
        let reply = self.call("told", json!({"line": line})).await?;
        Ok(reply["same"].as_bool().unwrap_or(false))
    }

    pub async fn call(&mut self, method: &str, mut params: Value) -> Result<Value, Failure> {
        params["session"] = Value::String(self.session.clone());
        Ok(self.call_raw(method, params).await?)
    }

    async fn call_raw(&mut self, method: &str, params: Value) -> Result<Value, RpcError> {
        let id = self.next;
        self.next += 1;
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.write
            .write_all(format!("{request}\n").as_bytes())
            .await
            .map_err(|e| lost(format!("lost the daemon: {e}")))?;
        read_reply(&mut self.read, id, said).await
    }
}

/// Reads lines until the reply to request `id`, handing each progress line to `progress`. A reply with an earlier id
/// belongs to a call whose caller stopped waiting (an MCP client cancelled the tool call after the request went out),
/// and is skipped, so it is never taken for this call's.
async fn read_reply<R: tokio::io::AsyncBufRead + Unpin>(
    read: &mut R,
    id: u64,
    mut progress: impl FnMut(&str),
) -> Result<Value, RpcError> {
    let reply = loop {
        let mut line = String::new();
        let n = read
            .read_line(&mut line)
            .await
            .map_err(|e| lost(format!("lost the daemon: {e}")))?;
        if n == 0 {
            return Err(lost("the daemon closed the connection".to_string()));
        }
        let reply: Value = serde_json::from_str(&line).map_err(|e| {
            RpcError::new(
                -32603,
                format!("the daemon sent bad JSON: {e}"),
                Value::Null,
            )
        })?;
        if reply["method"] == "progress" {
            if let Some(message) = reply["params"]["message"].as_str() {
                progress(message);
            }
            continue;
        }
        if reply["id"].as_u64().is_some_and(|got| got < id) {
            continue;
        }
        break reply;
    };
    match reply.get("error") {
        Some(e) => Err(RpcError::new(
            e["code"].as_i64().unwrap_or(-32603) as i32,
            e["message"].as_str().unwrap_or("error").to_string(),
            e["data"].clone(),
        )),
        None => Ok(reply["result"].clone()),
    }
}

/// The daemon went away mid-call. Its log says why; the next command starts a new one.
fn lost(message: String) -> RpcError {
    RpcError::new(
        -32603,
        message,
        json!({"hint": format!(
            "{} says why the daemon stopped; the next command starts a new one",
            log_path().display()
        )}),
    )
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

/// Spawns `phonedriver daemon --detach` and waits for its readiness line on a pipe, not for a fixed time.
async fn start_daemon() -> Result<(), Failure> {
    let exe = std::env::current_exe()
        .map_err(|e| Failure::Local(format!("cannot find the phonedriver binary: {e}")))?;
    let log = create_home(&socket::home()).and_then(|()| {
        rotate(&log_path(), LOG_LIMIT);
        crate::private::options()
            .create(true)
            .append(true)
            .open(log_path())
    });
    let ready = spawn_daemon(&exe, log, daemon_env(std::env::vars_os()))
        .map_err(|e| Failure::Local(format!("cannot start the daemon: {e}")))?;
    let mut lines = BufReader::new(ready).lines();
    let _ = timeout(READY_WAIT, lines.next_line()).await;
    Ok(())
}

/// Past this a daemon's log is moved to `daemon.log.1` before the next daemon writes: every daemon appended to one file
/// for as long as phonedriver was installed.
const LOG_LIMIT: u64 = 4 * 1024 * 1024;

/// Moves `log` to `<log>.1`, over an older one, when it is longer than `limit`.
fn rotate(log: &Path, limit: u64) {
    if std::fs::metadata(log).is_ok_and(|m| m.len() > limit) {
        let _ = std::fs::rename(log, log.with_extension("log.1"));
    }
}

/// Creates phonedriver's home owner-only when it is missing. One that is there keeps its mode: `PHONEDRIVER_HOME` may
/// name a folder the person made.
fn create_home(home: &Path) -> std::io::Result<()> {
    if home.is_dir() {
        return Ok(());
    }
    crate::private::dir(home)
}

/// The environment the daemon starts with: this process's, without `PHONEDRIVER_SECRET` and every
/// `PHONEDRIVER_SECRET_*` (decision D105). The CLI and a replay read those; the daemon is given secrets by its callers
/// and outlives them.
pub(crate) fn daemon_env(
    vars: impl IntoIterator<Item = (std::ffi::OsString, std::ffi::OsString)>,
) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
    vars.into_iter()
        .filter(|(name, _)| {
            let name = name.to_string_lossy().to_ascii_uppercase();
            name != "PHONEDRIVER_SECRET" && !name.starts_with(crate::variables::SECRET_PREFIX)
        })
        .collect()
}

#[cfg(unix)]
fn spawn_daemon(
    exe: &Path,
    log: std::io::Result<std::fs::File>,
    env: Vec<(std::ffi::OsString, std::ffi::OsString)>,
) -> std::io::Result<tokio::process::ChildStdout> {
    use std::process::Stdio;
    let mut child = tokio::process::Command::new(exe)
        .env_clear()
        .envs(env)
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
    env: Vec<(std::ffi::OsString, std::ffi::OsString)>,
) -> std::io::Result<tokio::fs::File> {
    let log = match log {
        Ok(file) => file,
        Err(_) => std::fs::OpenOptions::new().write(true).open("NUL")?,
    };
    crate::detach::spawn_daemon(exe, &log, &env).map(tokio::fs::File::from_std)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_daemon_log_past_its_limit_is_moved_aside_before_a_new_daemon_writes() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("daemon.log");
        std::fs::write(&log, vec![b'x'; 100]).unwrap();
        rotate(&log, 200);
        assert!(log.exists() && !dir.path().join("daemon.log.1").exists());
        std::fs::write(&log, vec![b'y'; 300]).unwrap();
        rotate(&log, 200);
        assert!(!log.exists());
        assert_eq!(
            std::fs::read(dir.path().join("daemon.log.1"))
                .unwrap()
                .len(),
            300
        );
        rotate(&dir.path().join("missing.log"), 200);
    }

    #[tokio::test]
    async fn a_reply_left_by_a_cancelled_call_is_not_taken_for_the_next_calls() {
        let wire = concat!(
            r#"{"jsonrpc":"2.0","id":3,"result":{"for":"the cancelled call"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"progress","params":{"message":"settling"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":4,"result":{"for":"this call"}}"#,
            "\n",
        );
        let mut read = wire.as_bytes();
        let reply = read_reply(&mut read, 4, |_| {}).await.unwrap();
        assert_eq!(reply, json!({"for": "this call"}));
    }

    #[tokio::test]
    async fn a_daemon_that_goes_away_mid_call_says_where_its_log_is() {
        let mut read: &[u8] = b"";
        let e = read_reply(&mut read, 1, |_| {}).await.unwrap_err();
        assert!(e.message.contains("closed the connection"), "{}", e.message);
        let hint = e.data["hint"].as_str().unwrap();
        assert!(hint.contains("daemon.log"), "{hint}");
    }

    #[tokio::test]
    async fn a_progress_line_from_the_daemon_reaches_the_call_that_set_doing() {
        let (set, done) = tokio::sync::watch::channel(String::new());
        DOING
            .scope(set, async {
                said("starting the Android agent");
                doing("step 2 of 3".into());
            })
            .await;
        assert_eq!(*done.borrow(), "step 2 of 3");
        let (set, done) = tokio::sync::watch::channel(String::new());
        DOING
            .scope(set, async { said("starting the Android agent") })
            .await;
        assert_eq!(*done.borrow(), "starting the Android agent");
    }

    #[test]
    fn an_mcp_server_and_a_cli_on_a_terminal_ask_for_progress() {
        assert_eq!(hello("mcp", "s")["progress"], json!(true));
        assert_eq!(hello("bench", "s")["progress"], json!(false));
        assert_eq!(
            hello("cli", "s")["progress"],
            json!(std::io::stderr().is_terminal())
        );
    }

    #[test]
    fn the_daemon_is_given_no_phonedriver_secret_variable() {
        use std::ffi::OsString;
        let given = [
            ("PATH", "/bin"),
            ("PHONEDRIVER_SECRET", "a"),
            ("PHONEDRIVER_SECRET_1", "b"),
            ("phonedriver_secret_app", "c"),
            ("PHONEDRIVER_SECRETS", "d"),
            ("PHONEDRIVER_HOME", "/h"),
        ]
        .map(|(k, v)| (OsString::from(k), OsString::from(v)));
        let kept: Vec<OsString> = daemon_env(given).into_iter().map(|(k, _)| k).collect();
        assert_eq!(kept, ["PATH", "PHONEDRIVER_SECRETS", "PHONEDRIVER_HOME"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_spawned_daemon_sees_only_the_environment_it_is_given() {
        use std::ffi::OsString;
        use std::os::unix::fs::PermissionsExt;
        use tokio::io::AsyncReadExt;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("fake-daemon");
        std::fs::write(&exe, "#!/bin/sh\nenv\n").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let env = daemon_env([
            (OsString::from("KEPT_HERE"), OsString::from("yes")),
            (OsString::from("PHONEDRIVER_SECRET_1"), OsString::from("no")),
        ]);
        let mut out = spawn_daemon(&exe, Err(std::io::Error::other("no log")), env).unwrap();
        let mut text = String::new();
        out.read_to_string(&mut text).await.unwrap();
        assert!(text.lines().any(|l| l == "KEPT_HERE=yes"), "{text}");
        assert!(!text.contains("PHONEDRIVER_SECRET"), "{text}");
    }

    #[test]
    fn hello_names_the_session_the_calls_carry() {
        assert_eq!(hello("mcp", "mcp-4242")["session"], json!("mcp-4242"));
    }
}
