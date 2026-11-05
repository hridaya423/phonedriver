//! Deploys, launches and connects to the on-device agent (architecture 6.2).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use phone_use_core::error::Error;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

use crate::adb::{Adb, Result, ShellPacket, ShellStream};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const JAR: &[u8] = include_bytes!(env!("PHONE_USE_AGENT_JAR"));
const READY_TIMEOUT: Duration = Duration::from_secs(3);
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

/// Puts the agent jar on the device unless it is already there. The name carries the jar's hash, so a path that exists is the right jar.
pub async fn deploy(adb: &Adb, serial: &str) -> Result<Deployed> {
    let digest = Sha256::digest(JAR);
    let sha8: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
    let path = format!("/data/local/tmp/phone-use-agent-{sha8}.jar");
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

async fn wait_ready(shell: &mut ShellStream, log: &AgentLog) -> Result<()> {
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
    let timed_out = tokio::time::timeout(READY_TIMEOUT, wait).await.is_err();
    if ready {
        return Ok(());
    }
    let lines = log.recent();
    let tail = lines.join("\n");
    if exit == Some(4) || tail.contains("already registered") {
        return Err(Error::AutomationConflict {
            holder: CONFLICT_HOLDER.to_string(),
        });
    }
    if timed_out {
        return Err(transport(format!(
            "the agent did not print `ready` within {} s: {tail}",
            READY_TIMEOUT.as_secs()
        )));
    }
    Err(transport(format!(
        "the agent exited with code {} before it was ready: {tail}",
        exit.unwrap_or(-1)
    )))
}

/// Deploys the jar, kills any running agent, starts a new one and waits for it to listen.
pub async fn launch(adb: &Adb, serial: &str) -> Result<Launched> {
    let jar = deploy(adb, serial).await?.path;
    adb.shell(serial, "pkill -f 'dev.phoneuse.agent.[M]ain'")
        .await?;
    let name = format!("phone-use-{VERSION}-{}", random_hex(4)?);
    let token = random_hex(32)?;
    let mut shell = adb
        .shell_stream(
            serial,
            &format!("CLASSPATH={jar} app_process / dev.phoneuse.agent.Main {VERSION} {name}"),
        )
        .await?;
    shell.write_stdin(format!("{token}\n").as_bytes()).await?;
    let log = AgentLog::default();
    wait_ready(&mut shell, &log).await?;
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
}

/// A connected agent. Dropping it closes the shell stream, which ends the agent on the device within about a second.
pub struct Agent {
    pub control: TcpStream,
    pub video: TcpStream,
    pub hello: Hello,
    pub log: AgentLog,
    pub(crate) launched: Launched,
}

impl Agent {
    pub async fn start(adb: &Adb, serial: &str) -> Result<Agent> {
        let launched = launch(adb, serial).await?;
        let mut control = adb.open_abstract(serial, &launched.name).await?;
        control
            .write_all(b"C")
            .await
            .map_err(|e| transport(format!("the agent connection broke: {e}")))?;
        write_frame(
            &mut control,
            &json!({"id": 0, "op": "hello", "version": VERSION, "token": launched.token}),
        )
        .await?;
        let reply = read_frame(&mut control).await.map_err(|e| {
            let lines = launched.log.recent().join("\n");
            transport(format!("no hello from the agent ({e}); agent log: {lines}"))
        })?;
        if reply["ok"] != true {
            return Err(transport(format!(
                "the agent refused hello: {}",
                reply["error"]
            )));
        }
        let hello = serde_json::from_value(reply)
            .map_err(|e| transport(format!("the agent's hello was malformed: {e}")))?;

        let mut video = adb.open_abstract(serial, &launched.name).await?;
        video
            .write_all(b"V")
            .await
            .map_err(|e| transport(format!("the agent connection broke: {e}")))?;
        write_frame(&mut video, &json!({"token": launched.token})).await?;
        Ok(Agent {
            control,
            video,
            hello,
            log: launched.log.clone(),
            launched,
        })
    }
}
