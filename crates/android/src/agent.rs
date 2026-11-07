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
const JAR_MISSING: i32 = 99;
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

/// The name carries the jar's hash, so a path that exists is the right jar.
fn jar_path() -> String {
    let digest = Sha256::digest(JAR);
    let sha8: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
    format!("/data/local/tmp/phone-use-agent-{sha8}.jar")
}

async fn start(adb: &Adb, serial: &str, jar: &str, name: &str, token: &str) -> Result<ShellStream> {
    // `$M` keeps the class name out of this shell's own command line, which `pkill -f` would otherwise match.
    let mut shell = adb
        .shell_stream(
            serial,
            &format!(
                "[ -f {jar} ] || exit {JAR_MISSING}; pkill -f 'dev.phoneuse.agent.[M]ain'; M=Main; CLASSPATH={jar} exec app_process / dev.phoneuse.agent.$M {VERSION} {name}"
            ),
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
async fn wait_ready(shell: &mut ShellStream, log: &AgentLog) -> Result<bool> {
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
        return Ok(true);
    }
    if exit == Some(JAR_MISSING) {
        return Ok(false);
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

/// Kills any running agent, starts a new one and waits for it to listen. One adb shell does the existence check of the
/// jar, the kill and the start, so a warm start costs one round trip to the device before the agent boots; the jar is
/// pushed only when the shell reports it missing.
pub async fn launch(adb: &Adb, serial: &str) -> Result<Launched> {
    let jar = jar_path();
    let name = format!("phone-use-{VERSION}-{}", random_hex(4)?);
    let token = random_hex(32)?;
    let log = AgentLog::default();
    let mut shell = start(adb, serial, &jar, &name, &token).await?;
    if !wait_ready(&mut shell, &log).await? {
        deploy(adb, serial).await?;
        shell = start(adb, serial, &jar, &name, &token).await?;
        if !wait_ready(&mut shell, &log).await? {
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
    pub async fn start(adb: &Adb, serial: &str) -> Result<Agent> {
        let launched = launch(adb, serial).await?;
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
