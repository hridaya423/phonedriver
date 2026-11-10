//! Deploys, launches and connects to the on-device agent (architecture 6.2).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use phonedriver_core::device::DisplayMode;
use phonedriver_core::error::{Error, Op};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

use crate::adb::{Adb, Result, ShellPacket, ShellStream};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const JAR: &[u8] = include_bytes!(env!("PHONEDRIVER_AGENT_JAR"));
const JAR_MISSING: i32 = 99;
const NO_HIDDEN_DISPLAY: i32 = 5;
const SIGTERM: i32 = 143;
const OWNED: i32 = 98;
const OWNER_FILE: &str = "/data/local/tmp/phonedriver-owner";
const OWNER_GRACE_TRIES: u32 = 4;
const OWNER_GRACE_STEP: Duration = Duration::from_millis(500);
/// Observed on two API 37 emulators under load (2026-11-10): the first call of a session failed five times with "did not
/// print `ready` within 3 s", and the next call found the agent up. An agent that starts in time is not delayed by this.
const READY_TIMEOUT: Duration = Duration::from_secs(10);
/// Creating the encoder and the virtual display at 2560x1600 took 5 to 8 s to `ready` on the API 37 tablet emulator.
const HIDDEN_READY_TIMEOUT: Duration = Duration::from_secs(20);
const LOG_LINES: usize = 200;
const MAX_FRAME: usize = 64 << 20;
const CONFLICT_HOLDER: &str = "another UiAutomation client, such as `uiautomator`, an Appium server or an instrumentation test";

fn transport(detail: impl Into<String>) -> Error {
    Error::Transport {
        detail: detail.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deployed {
    pub path: String,
    pub pushed: bool,
}

/// The name carries the jar's hash, so a path that exists is the right jar.
fn jar_path() -> String {
    let digest = Sha256::digest(JAR);
    let sha8: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
    format!("/data/local/tmp/phonedriver-agent-{sha8}.jar")
}

fn host_name() -> String {
    static HOST: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HOST.get_or_init(|| {
        let name = std::process::Command::new("hostname")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        let name: String = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            .collect();
        if name.is_empty() {
            "unknown-host".into()
        } else {
            name
        }
    })
    .clone()
}

/// Names this daemon in the owner file: another daemon that finds a live agent under a different name must not kill it.
fn owner() -> String {
    format!("pid={} host={}", std::process::id(), host_name())
}

/// One adb shell does the existence check of the jar, the owner check, the kill and the start. The owner file is read
/// only while an agent is running: an agent that adbd has already ended leaves a stale file, and an agent with no file
/// is from a phonedriver that did not write one.
fn start_command(
    jar: &str,
    owner_file: &str,
    name: &str,
    display: DisplayMode,
    me: &str,
) -> String {
    let hidden = match display {
        DisplayMode::Main => "",
        DisplayMode::Hidden => " hidden",
    };
    // `$M` keeps the class name out of this shell's own command line, which `pkill -f` would otherwise match.
    format!(
        "[ -f {jar} ] || exit {JAR_MISSING}; o=$(cat {owner_file} 2>/dev/null); if [ -n \"$o\" ] && [ \"$o\" != '{me}' ] && pgrep -f 'dev.phonedriver.agent.[M]ain' >/dev/null; then echo \"owned $o\"; exit {OWNED}; fi; pkill -f 'dev.phonedriver.agent.[M]ain'; echo '{me}' > {owner_file}; M=Main; CLASSPATH={jar} exec app_process / dev.phonedriver.agent.$M {VERSION} {name}{hidden}"
    )
}

async fn start(
    adb: &Adb,
    serial: &str,
    jar: &str,
    name: &str,
    token: &str,
    display: DisplayMode,
) -> Result<ShellStream> {
    let mut shell = adb
        .shell_stream(
            serial,
            &start_command(jar, OWNER_FILE, name, display, &owner()),
        )
        .await?;
    shell.write_stdin(format!("{token}\n").as_bytes()).await?;
    Ok(shell)
}

/// Puts the agent jar on the device unless it is already there.
pub async fn deploy(adb: &Adb, serial: &str) -> Result<Deployed> {
    let path = jar_path();
    if adb.shell(serial, &format!("ls {path}")).await?.exit_code == 0 {
        return Ok(Deployed {
            path,
            pushed: false,
        });
    }
    let part = format!("{path}.part");
    adb.shell(serial, &format!("rm -f {part}")).await?;
    adb.push(serial, JAR, &part, 0o444).await?;
    let moved = adb.shell(serial, &format!("mv {part} {path}")).await?;
    if moved.exit_code != 0 {
        return Err(transport(format!(
            "could not move the agent jar into place: {}",
            String::from_utf8_lossy(&moved.stderr)
        )));
    }
    // Every version left its jar behind (an emulator here held 52), and a running agent keeps the jar it opened.
    adb.shell(
        serial,
        &format!(
            "for f in /data/local/tmp/phonedriver-agent-*.jar; do [ \"$f\" = {path} ] || rm -f \"$f\"; done"
        ),
    )
    .await?;
    Ok(Deployed { path, pushed: true })
}

/// The last lines the agent wrote to stdout and stderr, for the daemon's log.
#[derive(Debug, Clone, Default)]
pub struct AgentLog(Arc<Mutex<VecDeque<String>>>);

impl AgentLog {
    fn push(&self, line: String) {
        let mut lines = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if lines.len() == LOG_LINES {
            lines.pop_front();
        }
        lines.push_back(line);
    }

    pub fn recent(&self) -> Vec<String> {
        let lines = self.0.lock().unwrap_or_else(|e| e.into_inner());
        lines.iter().cloned().collect()
    }
}

/// A running agent process that nobody has connected to yet. Dropping it closes the shell stream, which makes adbd kill the agent.
pub struct Launched {
    pub name: String,
    pub token: String,
    pub log: AgentLog,
    drain: JoinHandle<()>,
}

impl Drop for Launched {
    fn drop(&mut self) {
        self.drain.abort();
    }
}

fn random_hex(bytes: usize) -> Result<String> {
    let mut b = vec![0u8; bytes];
    getrandom::fill(&mut b)
        .map_err(|e| transport(format!("no randomness for the agent token: {e}")))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

struct LineSplitter {
    pending: String,
}

impl LineSplitter {
    fn feed(&mut self, bytes: &[u8], mut line: impl FnMut(String)) {
        self.pending.push_str(&String::from_utf8_lossy(bytes));
        while let Some(i) = self.pending.find('\n') {
            let rest = self.pending.split_off(i + 1);
            let text = std::mem::replace(&mut self.pending, rest);
            line(text.trim_end().to_string());
        }
    }
}

/// `Ok(false)` when the shell found no jar at the path (exit `JAR_MISSING`).
async fn wait_ready(
    shell: &mut ShellStream,
    log: &AgentLog,
    limit: Duration,
    serial: &str,
) -> Result<bool> {
    let mut out = LineSplitter {
        pending: String::new(),
    };
    let mut err = LineSplitter {
        pending: String::new(),
    };
    let mut ready = false;
    let mut exit = None;
    let wait = async {
        while !ready && exit.is_none() {
            match shell.next().await? {
                Some(ShellPacket::Stdout(d)) => {
                    out.feed(&d, |l| {
                        ready |= l == "ready";
                        log.push(l);
                    });
                }
                Some(ShellPacket::Stderr(d)) => err.feed(&d, |l| log.push(l)),
                Some(ShellPacket::Exit(code)) => exit = Some(code),
                None => exit = Some(-1),
            }
        }
        Ok::<(), Error>(())
    };
    let timed_out = tokio::time::timeout(limit, wait).await.is_err();
    if ready {
        return Ok(true);
    }
    if exit == Some(JAR_MISSING) {
        return Ok(false);
    }
    Err(not_ready(exit, timed_out, limit, &log.recent(), serial))
}

fn not_ready(
    exit: Option<i32>,
    timed_out: bool,
    limit: Duration,
    lines: &[String],
    serial: &str,
) -> Error {
    let tail = lines.join("\n");
    if exit == Some(OWNED)
        && let Some(held) = lines.iter().rev().find_map(|l| l.strip_prefix("owned "))
        && let Some((pid, host)) = held
            .strip_prefix("pid=")
            .and_then(|r| r.split_once(" host="))
        && let Ok(pid) = pid.parse()
    {
        return Error::DeviceDriven {
            id: format!("android:{serial}"),
            pid,
            host: host.to_string(),
        };
    }
    if exit == Some(4) || tail.contains("already registered") {
        return Error::AutomationConflict {
            holder: CONFLICT_HOLDER.to_string(),
        };
    }
    if exit == Some(NO_HIDDEN_DISPLAY) {
        return Error::Unsupported {
            op: Op::Display,
            reason: "a hidden display needs Android 13 (API 33) or newer, and the phone must allow it",
        };
    }
    if timed_out {
        return transport(format!(
            "the agent did not print `ready` within {} s: {tail}",
            limit.as_secs()
        ));
    }
    if exit == Some(SIGTERM) && tail.is_empty() {
        return transport(
            "the agent was stopped before it was ready; starting an agent kills the running one, so another phonedriver daemon using this device does this",
        );
    }
    transport(format!(
        "the agent exited with code {} before it was ready: {tail}",
        exit.unwrap_or(-1)
    ))
}

/// adbd ends an agent about a second after its shell stream closes, so an owner that has just exited still shows for
/// that long: an owned device is asked again a few times before it is reported.
#[allow(clippy::too_many_arguments)]
async fn start_ready(
    adb: &Adb,
    serial: &str,
    jar: &str,
    name: &str,
    token: &str,
    display: DisplayMode,
    log: &AgentLog,
    limit: Duration,
) -> Result<(ShellStream, bool)> {
    let mut tries = 0;
    loop {
        let mut shell = start(adb, serial, jar, name, token, display).await?;
        match wait_ready(&mut shell, log, limit, serial).await {
            Err(Error::DeviceDriven { .. }) if tries < OWNER_GRACE_TRIES => {
                tries += 1;
                tokio::time::sleep(OWNER_GRACE_STEP).await;
            }
            found => return found.map(|found| (shell, found)),
        }
    }
}

/// Kills this daemon's own running agent, starts a new one and waits for it to listen. One adb shell does the existence check of the
/// jar, the kill and the start, so a warm start costs one round trip to the device before the agent boots; the jar is
/// pushed only when the shell reports it missing.
pub async fn launch(adb: &Adb, serial: &str, display: DisplayMode) -> Result<Launched> {
    let jar = jar_path();
    let name = format!("phonedriver-{VERSION}-{}", random_hex(4)?);
    let token = random_hex(32)?;
    let log = AgentLog::default();
    let ready_limit = match display {
        DisplayMode::Main => READY_TIMEOUT,
        DisplayMode::Hidden => HIDDEN_READY_TIMEOUT,
    };
    let (mut shell, found) =
        start_ready(adb, serial, &jar, &name, &token, display, &log, ready_limit).await?;
    if !found {
        deploy(adb, serial).await?;
        let (again, found) =
            start_ready(adb, serial, &jar, &name, &token, display, &log, ready_limit).await?;
        shell = again;
        if !found {
            return Err(transport(format!(
                "the agent jar is not at {jar} after the push"
            )));
        }
    }
    let sink = log.clone();
    let drain = tokio::spawn(async move {
        let mut out = LineSplitter {
            pending: String::new(),
        };
        let mut err = LineSplitter {
            pending: String::new(),
        };
        while let Ok(Some(packet)) = shell.next().await {
            match packet {
                ShellPacket::Stdout(d) => out.feed(&d, |l| sink.push(l)),
                ShellPacket::Stderr(d) => err.feed(&d, |l| sink.push(l)),
                ShellPacket::Exit(code) => {
                    sink.push(format!("[agent exited with code {code}]"));
                    break;
                }
            }
        }
    });
    Ok(Launched {
        name,
        token,
        log,
        drain,
    })
}

pub async fn write_frame<W: AsyncWrite + Unpin>(s: &mut W, v: &Value) -> Result<()> {
    let body =
        serde_json::to_vec(v).map_err(|e| transport(format!("cannot encode a frame: {e}")))?;
    let mut msg = (body.len() as u32).to_be_bytes().to_vec();
    msg.extend(body);
    s.write_all(&msg)
        .await
        .map_err(|e| transport(format!("the agent connection broke: {e}")))
}

/// Reads frames until the reply to request `id`. The agent pushes events at any time, including before a
/// reply (a window change right after connecting), and they are not replies.
pub async fn read_reply<R: AsyncRead + Unpin>(s: &mut R, id: u64) -> Result<Value> {
    loop {
        let frame = read_frame(s).await?;
        if frame["id"].as_u64() == Some(id) {
            return Ok(frame);
        }
    }
}

pub async fn read_frame<R: AsyncRead + Unpin>(s: &mut R) -> Result<Value> {
    let broke = |e: std::io::Error| transport(format!("the agent connection broke: {e}"));
    let mut len = [0u8; 4];
    s.read_exact(&mut len).await.map_err(broke)?;
    let n = u32::from_be_bytes(len) as usize;
    if n > MAX_FRAME {
        return Err(transport(format!("the agent sent a {n} byte frame")));
    }
    let mut body = vec![0u8; n];
    s.read_exact(&mut body).await.map_err(broke)?;
    serde_json::from_slice(&body).map_err(|e| transport(format!("the agent sent bad JSON: {e}")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct DisplaySize {
    pub w: u32,
    pub h: u32,
    pub rotation: u8,
}

/// What the agent reports in its `hello` reply. Sizes are device pixels.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Hello {
    pub api: u32,
    pub model: String,
    pub density: f32,
    pub display: DisplaySize,
    /// Set when the agent works on a virtual display: its id.
    #[serde(default)]
    pub hidden_display: Option<u32>,
}

/// A connected agent. Dropping it closes the shell stream, which ends the agent on the device within about a second.
pub struct Agent {
    pub control: TcpStream,
    pub video: TcpStream,
    pub hello: Hello,
    pub log: AgentLog,
    pub(crate) launched: Launched,
}

async fn open_stream(
    adb: &Adb,
    serial: &str,
    launched: &Launched,
    kind: u8,
    first: Value,
) -> Result<TcpStream> {
    let mut stream = adb.open_abstract(serial, &launched.name).await?;
    stream
        .write_all(&[kind])
        .await
        .map_err(|e| transport(format!("the agent connection broke: {e}")))?;
    write_frame(&mut stream, &first).await?;
    Ok(stream)
}

impl Agent {
    pub async fn start(adb: &Adb, serial: &str, display: DisplayMode) -> Result<Agent> {
        let launched = launch(adb, serial, display).await?;
        let (control, video) = tokio::try_join!(
            open_stream(
                adb,
                serial,
                &launched,
                b'C',
                json!({"id": 0, "op": "hello", "version": VERSION, "token": launched.token})
            ),
            open_stream(
                adb,
                serial,
                &launched,
                b'V',
                json!({"token": launched.token})
            ),
        )?;
        let mut control = control;
        let reply = read_reply(&mut control, 0).await.map_err(|e| {
            let lines = launched.log.recent().join("\n");
            transport(format!("no hello from the agent ({e}); agent log: {lines}"))
        })?;
        if reply["ok"] != true {
            return Err(transport(format!("the agent refused hello: {reply}")));
        }
        let hello = serde_json::from_value(reply)
            .map_err(|e| transport(format!("the agent's hello was malformed: {e}")))?;
        Ok(Agent {
            control,
            video,
            hello,
            log: launched.log.clone(),
            launched,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_carries_the_hidden_display_id_when_there_is_one() {
        let hello: Hello = serde_json::from_value(json!({
            "api": 37, "model": "m", "density": 2.0,
            "display": {"w": 2560, "h": 1600, "rotation": 0},
            "hidden_display": 7
        }))
        .unwrap();
        assert_eq!(hello.hidden_display, Some(7));
        let plain: Hello = serde_json::from_value(json!({
            "api": 37, "model": "m", "density": 2.0,
            "display": {"w": 2560, "h": 1600, "rotation": 0}
        }))
        .unwrap();
        assert_eq!(plain.hidden_display, None);
    }

    #[cfg(unix)]
    fn run_start_command(
        owner_in_file: Option<&str>,
        agent_running: bool,
        me: &str,
    ) -> (i32, String, Vec<String>) {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let log = dir.path().join("calls");
        let stub = |name: &str, body: &str| {
            let path = bin.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::process::Command::new("chmod")
                .arg("+x")
                .arg(&path)
                .status()
                .unwrap();
        };
        stub("pgrep", if agent_running { "exit 0" } else { "exit 1" });
        stub("pkill", &format!("echo pkill >> {}", log.display()));
        stub(
            "app_process",
            &format!("echo app_process >> {}", log.display()),
        );
        let jar = dir.path().join("agent.jar");
        std::fs::write(&jar, b"").unwrap();
        let owner_file = dir.path().join("owner");
        if let Some(o) = owner_in_file {
            std::fs::write(&owner_file, format!("{o}\n")).unwrap();
        }
        let command = start_command(
            jar.to_str().unwrap(),
            owner_file.to_str().unwrap(),
            "phonedriver-0.1.0-ab",
            DisplayMode::Main,
            me,
        );
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .output()
            .unwrap();
        let calls = std::fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect();
        (
            out.status.code().unwrap(),
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
            calls,
        )
    }

    #[cfg(unix)]
    #[test]
    fn a_live_agent_another_daemon_owns_is_not_killed() {
        let (code, out, calls) =
            run_start_command(Some("pid=4242 host=other"), true, "pid=7 host=mine");
        assert_eq!(code, OWNED, "{out}");
        assert_eq!(out, "owned pid=4242 host=other");
        assert!(calls.is_empty(), "{calls:?}");
    }

    #[cfg(unix)]
    #[test]
    fn this_daemons_own_agent_is_replaced_and_the_owner_file_names_it() {
        let (code, _, calls) = run_start_command(Some("pid=7 host=mine"), true, "pid=7 host=mine");
        assert_eq!(code, 0);
        assert_eq!(calls, ["pkill", "app_process"]);
    }

    #[cfg(unix)]
    #[test]
    fn an_owner_file_with_no_agent_running_is_stale_and_is_taken_over() {
        let (code, _, calls) =
            run_start_command(Some("pid=4242 host=other"), false, "pid=7 host=mine");
        assert_eq!(code, 0);
        assert_eq!(calls, ["pkill", "app_process"]);
    }

    #[cfg(unix)]
    #[test]
    fn an_agent_with_no_owner_file_is_from_an_older_phonedriver_and_is_replaced() {
        let (code, _, calls) = run_start_command(None, true, "pid=7 host=mine");
        assert_eq!(code, 0);
        assert_eq!(calls, ["pkill", "app_process"]);
    }

    #[test]
    fn an_agent_owned_by_another_daemon_is_an_error_naming_it() {
        let lines = vec!["owned pid=4242 host=mac.local".to_string()];
        let e = not_ready(Some(OWNED), false, READY_TIMEOUT, &lines, "emulator-5556");
        assert_eq!(
            e,
            Error::DeviceDriven {
                id: "android:emulator-5556".into(),
                pid: 4242,
                host: "mac.local".into(),
            }
        );
        let text = e.to_string();
        assert!(
            text.contains("4242") && text.contains("mac.local"),
            "{text}"
        );
        assert!(e.hint().contains("kill 4242"), "{}", e.hint());
    }

    #[test]
    fn an_agent_stopped_before_it_was_ready_names_what_stops_agents() {
        let e = not_ready(Some(143), false, READY_TIMEOUT, &[], "emulator-5556").to_string();
        assert!(e.contains("another phonedriver"), "{e}");
        assert!(!e.trim_end().ends_with(':'), "{e}");
        let other = not_ready(
            Some(1),
            false,
            READY_TIMEOUT,
            &["boom".into()],
            "emulator-5556",
        )
        .to_string();
        assert!(other.ends_with("before it was ready: boom"), "{other}");
    }

    #[test]
    fn an_agent_that_refuses_a_hidden_display_is_unsupported_not_a_transport_failure() {
        let lines = vec!["hidden display: needs Android 13 (API 33), this is API 26".to_string()];
        assert!(matches!(
            not_ready(
                Some(NO_HIDDEN_DISPLAY),
                false,
                READY_TIMEOUT,
                &lines,
                "emulator-5556"
            ),
            Error::Unsupported {
                op: Op::Display,
                ..
            }
        ));
        assert!(matches!(
            not_ready(Some(1), false, READY_TIMEOUT, &lines, "emulator-5556"),
            Error::Transport { .. }
        ));
    }
}
