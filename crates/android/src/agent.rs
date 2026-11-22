//! Deploys, launches and connects to the on-device agent (architecture 6.2).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use phonedriver_core::device::DisplayMode;
use phonedriver_core::error::{Error, Op, UserAction};
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
const SIGTERM: i32 = 143;
const OWNED: i32 = 98;
const OWNER_FILE: &str = "/data/local/tmp/phonedriver-owner";
const OWNER_GRACE_TRIES: u32 = 4;
const OWNER_GRACE_STEP: Duration = Duration::from_millis(500);
/// Observed on two API 37 emulators under load (2026-11-10): the first call of a session failed five times with "did not
/// print `ready` within 3 s", and the next call found the agent up. An agent that starts in time is not delayed by this.
const READY_TIMEOUT: Duration = Duration::from_secs(10);
/// Creating the encoder and the virtual display at 2560x1600 took 5 to 8 s on the API 37 tablet emulator.
const HIDDEN_HELLO_TIMEOUT: Duration = Duration::from_secs(20);
/// The agent frees a lane when the thread of the connection that held it sees its socket close, which a thread busy
/// with an action does only when the action ends. Observed on GitHub's Linux runner (2026-11-17): the video stream
/// ended during a `type`, the daemon dropped the connection and asked for the phone's screen again at once, and the
/// agent answered "lane in use". The agent belongs to one daemon (its owner file), so a held lane is this daemon's
/// own dropped connection, and a reconnect waits for it to be freed.
const LANE_FREE_WAIT: Duration = Duration::from_secs(5);
const LANE_FREE_STEP: Duration = Duration::from_millis(100);
const NO_HIDDEN_DISPLAY: &str =
    "a hidden display needs Android 13 (API 33) or newer, and the phone must allow it";
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

/// This computer in the owner file: a hash of its name, so the file that stays on the phone does not name it.
fn host_id() -> String {
    let digest = Sha256::digest(format!("phonedriver owner {}", host_name()));
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Names this daemon in the owner file: another daemon that finds a live agent under a different name must not kill it.
fn owner() -> String {
    format!("pid={} host={}", std::process::id(), host_id())
}

/// The computer an owner line names, in words: a phonedriver before 0.1.7 wrote the host's name.
fn owner_host(host: &str) -> String {
    if host == host_id() {
        "this computer".into()
    } else if host.len() == 16 && host.chars().all(|c| c.is_ascii_hexdigit()) {
        "another computer".into()
    } else {
        host.to_string()
    }
}

/// One adb shell does the existence check of the jar, the owner check, the kill and the start. The owner file is read
/// only while an agent is running: an agent that adbd has already ended leaves a stale file, and an agent with no file
/// is from a phonedriver that did not write one.
fn start_command(jar: &str, owner_file: &str, name: &str, me: &str) -> String {
    // `$M` keeps the class name out of this shell's own command line, which `pkill -f` would otherwise match.
    format!(
        "[ -f {jar} ] || exit {JAR_MISSING}; o=$(cat {owner_file} 2>/dev/null); if [ -n \"$o\" ] && [ \"$o\" != '{me}' ] && pgrep -f 'dev.phonedriver.agent.[M]ain' >/dev/null; then echo \"owned $o\"; exit {OWNED}; fi; pkill -f 'dev.phonedriver.agent.[M]ain'; echo '{me}' > {owner_file}; M=Main; CLASSPATH={jar} exec app_process / dev.phonedriver.agent.$M {VERSION} {name}"
    )
}

async fn start(adb: &Adb, serial: &str, jar: &str, name: &str, token: &str) -> Result<ShellStream> {
    let mut shell = adb
        .shell_stream(serial, &start_command(jar, OWNER_FILE, name, &owner()))
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

/// A running agent process. Every lane of the device holds it; dropping the last hold closes the shell stream, which
/// makes adbd kill the agent.
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
    let tail = lines
        .iter()
        .filter(|l| !l.starts_with("\tat ") && !l.starts_with("\t... "))
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\n");
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
            host: owner_host(host),
        };
    }
    if exit == Some(4) || tail.contains("already registered") {
        return Error::AutomationConflict {
            holder: CONFLICT_HOLDER.to_string(),
        };
    }
    if timed_out {
        return transport(format!(
            "the agent did not print `ready` within {} s: {tail}",
            limit.as_secs()
        ));
    }
    // Observed on the API 37 emulator after its system server died (bench task alarm-gym): the agent's stack ended in
    // a null `IAccessibilityManager`, 1,200 characters with "Ask the user to run `doctor`" under them, three times.
    if tail.contains("IAccessibilityManager") && tail.contains("null object reference") {
        return Error::Refused {
            detail: format!(
                "Android's accessibility service is not running on {serial}: the system is still starting, or it crashed and is restarting. Wait 30 seconds and call again; an emulator that stays like this needs a restart"
            ),
        };
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
async fn start_ready(
    adb: &Adb,
    serial: &str,
    jar: &str,
    name: &str,
    token: &str,
    log: &AgentLog,
) -> Result<(ShellStream, bool)> {
    let mut tries = 0;
    loop {
        let mut shell = start(adb, serial, jar, name, token).await?;
        match wait_ready(&mut shell, log, READY_TIMEOUT, serial).await {
            Err(Error::DeviceDriven { .. }) if tries < OWNER_GRACE_TRIES => {
                tries += 1;
                tokio::time::sleep(OWNER_GRACE_STEP).await;
            }
            found => return found.map(|found| (shell, found)),
        }
    }
}

type Running = Arc<tokio::sync::Mutex<Weak<Launched>>>;

/// The agent process of each device this daemon drives, by adb server and serial. The system registers one
/// UiAutomation at a time, so every lane of a device connects to the same process (decision D73).
fn running(adb: &Adb, serial: &str) -> Running {
    static RUNNING: std::sync::OnceLock<Mutex<HashMap<(String, String), Running>>> =
        std::sync::OnceLock::new();
    RUNNING
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry((adb.addr().to_string(), serial.to_string()))
        .or_default()
        .clone()
}

/// The device's running agent, or a new one when none of this daemon's is alive.
pub async fn launch(adb: &Adb, serial: &str) -> Result<Arc<Launched>> {
    let slot = running(adb, serial);
    let mut held = slot.lock().await;
    if let Some(alive) = held.upgrade()
        && !alive.drain.is_finished()
    {
        return Ok(alive);
    }
    let launched = Arc::new(launch_new(adb, serial).await?);
    *held = Arc::downgrade(&launched);
    Ok(launched)
}

/// Kills this daemon's own running agent, starts a new one and waits for it to listen. One adb shell does the existence check of the
/// jar, the kill and the start, so a warm start costs one round trip to the device before the agent boots; the jar is
/// pushed only when the shell reports it missing.
async fn launch_new(adb: &Adb, serial: &str) -> Result<Launched> {
    let jar = jar_path();
    let name = format!("phonedriver-{VERSION}-{}", random_hex(4)?);
    let token = random_hex(32)?;
    let log = AgentLog::default();
    let (mut shell, found) = start_ready(adb, serial, &jar, &name, &token, &log).await?;
    if !found {
        deploy(adb, serial).await?;
        let (again, found) = start_ready(adb, serial, &jar, &name, &token, &log).await?;
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

/// The agent deflates a frame of 1 KB or more and sets this bit of its length.
const DEFLATED: u32 = 0x8000_0000;

pub async fn read_frame<R: AsyncRead + Unpin>(s: &mut R) -> Result<Value> {
    let broke = |e: std::io::Error| transport(format!("the agent connection broke: {e}"));
    let mut len = [0u8; 4];
    s.read_exact(&mut len).await.map_err(broke)?;
    let len = u32::from_be_bytes(len);
    let n = (len & !DEFLATED) as usize;
    if n > MAX_FRAME {
        return Err(transport(format!("the agent sent a {n} byte frame")));
    }
    let mut body = vec![0u8; n];
    s.read_exact(&mut body).await.map_err(broke)?;
    if len & DEFLATED != 0 {
        let mut plain = Vec::new();
        std::io::Read::read_to_end(
            &mut std::io::Read::take(
                flate2::read::ZlibDecoder::new(body.as_slice()),
                MAX_FRAME as u64,
            ),
            &mut plain,
        )
        .map_err(|e| transport(format!("the agent sent a frame that does not inflate: {e}")))?;
        body = plain;
    }
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

/// One lane's connection to the agent. Dropping the last one of a device closes the shell stream, which ends the agent
/// on the device within about a second.
pub struct Agent {
    pub control: TcpStream,
    pub video: TcpStream,
    pub hello: Hello,
    pub log: AgentLog,
    pub(crate) launched: Arc<Launched>,
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

/// True when the agent refused hello because a connection of this daemon still holds the lane.
fn lane_held(reply: &Value) -> bool {
    reply["error"]
        .as_str()
        .is_some_and(|e| e.starts_with("lane in use: "))
}

/// Why the agent answered hello with a refusal: a phone that cannot give a hidden display, a locked phone asked
/// for one, a lane another connection holds, or a version that is not ours.
fn refused_hello(reply: &Value) -> Error {
    let error = reply["error"].as_str().unwrap_or_default();
    if error.contains("hidden display:") {
        return Error::Unsupported {
            op: Op::Display,
            reason: NO_HIDDEN_DISPLAY,
        };
    }
    if error.contains("the phone is locked") {
        return Error::NeedsUser {
            action: UserAction::UnlockDevice,
        };
    }
    if let Some(lane) = error.strip_prefix("lane in use: ") {
        return Error::Refused {
            detail: format!("lane {lane} of this device is already open from this daemon"),
        };
    }
    transport(format!("the agent refused hello: {reply}"))
}

const SLOW_LINK: Duration = Duration::from_millis(120);

/// The bit rate and frame rate to ask for on a slow link, told by how long the agent's hello took (two round
/// trips: 9 to 14 ms over USB, 31 to 34 ms through an ssh tunnel, 433 to 481 ms on a link shaped to 3G, 780 kbit/s
/// down and 100 ms each way). The agent's own 8 Mbit/s is ten times what that link carries, and the frames that
/// tell a tap has settled queue behind the stream (2026-11-12).
///
/// Not for a hidden display: it is opened inside hello, so that hello's time says nothing about the link, and its
/// encoder already runs at the agent's own rate. Observed with two lanes: the second lane's stream was configured
/// with the first one's slow-link numbers, and a screenshot after a tap showed the screen from before it.
fn video_for(display: DisplayMode, hello: Duration) -> Option<(u32, u32, u32)> {
    (display == DisplayMode::Main && hello >= SLOW_LINK).then_some((400_000, 10, 1280))
}

impl Agent {
    /// Connects to the device's agent, starting it when it is not running. With `DisplayMode::Hidden` the agent
    /// opens a virtual display for this connection: the one named `lane`, or the unnamed one of D32.
    pub async fn start(
        adb: &Adb,
        serial: &str,
        display: DisplayMode,
        lane: Option<&str>,
    ) -> Result<Agent> {
        let launched = launch(adb, serial).await?;
        let on = match display {
            DisplayMode::Main => json!({}),
            DisplayMode::Hidden => json!({"display": "hidden", "lane": lane.unwrap_or_default()}),
        };
        let with = |mut first: Value| {
            for (k, v) in on.as_object().into_iter().flatten() {
                first[k] = v.clone();
            }
            first
        };
        let no_hello = |e: Error| {
            let lines = launched.log.recent().join("\n");
            transport(format!("no hello from the agent ({e}); agent log: {lines}"))
        };
        let freeing = std::time::Instant::now();
        let (control, reply, asked) = loop {
            let asked = std::time::Instant::now();
            let mut control = open_stream(
                adb,
                serial,
                &launched,
                b'C',
                with(json!({"id": 0, "op": "hello", "version": VERSION, "token": launched.token})),
            )
            .await?;
            let reply = match display {
                DisplayMode::Main => read_reply(&mut control, 0).await.map_err(no_hello)?,
                DisplayMode::Hidden => {
                    tokio::time::timeout(HIDDEN_HELLO_TIMEOUT, read_reply(&mut control, 0))
                        .await
                        .map_err(|_| {
                            transport(format!(
                                "the agent did not open a hidden display within {} s",
                                HIDDEN_HELLO_TIMEOUT.as_secs()
                            ))
                        })?
                        .map_err(no_hello)?
                }
            };
            if reply["ok"] == true {
                break (control, reply, asked);
            }
            if !lane_held(&reply) || freeing.elapsed() >= LANE_FREE_WAIT {
                return Err(refused_hello(&reply));
            }
            tokio::time::sleep(LANE_FREE_STEP).await;
        };
        let asked = asked.elapsed();
        let mut first = with(json!({"token": launched.token}));
        if let Some((bit_rate, fps, max_size)) = video_for(display, asked) {
            first["bit_rate"] = json!(bit_rate);
            first["fps"] = json!(fps);
            first["max_size"] = json!(max_size);
        }
        let video = open_stream(adb, serial, &launched, b'V', first).await?;
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

    #[tokio::test]
    async fn a_deflated_frame_reads_as_the_same_json() {
        use std::io::Write;
        let v = json!({"id": 7, "nodes": vec!["row"; 400]});
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        z.write_all(&serde_json::to_vec(&v).unwrap()).unwrap();
        let small = z.finish().unwrap();
        let mut wire = (small.len() as u32 | DEFLATED).to_be_bytes().to_vec();
        wire.extend(small);
        assert_eq!(read_frame(&mut wire.as_slice()).await.unwrap(), v);
    }

    #[test]
    fn a_lane_this_daemon_still_holds_is_waited_for_and_other_refusals_are_not() {
        let refused = |error: &str| json!({"id": 0, "ok": false, "error": error});
        assert!(lane_held(&refused("lane in use: the phone's own screen")));
        assert!(lane_held(&refused("lane in use: b")));
        assert!(!lane_held(&refused("the phone is locked")));
        assert!(!lane_held(&refused("hidden display: not supported")));
        assert!(!lane_held(&json!({"id": 0, "ok": true})));
        assert!(matches!(
            refused_hello(&refused("lane in use: b")),
            Error::Refused { detail } if detail == "lane b of this device is already open from this daemon"
        ));
    }

    #[test]
    fn only_a_slow_link_gets_the_small_stream() {
        assert_eq!(
            video_for(DisplayMode::Main, Duration::from_millis(34)),
            None
        );
        assert_eq!(
            video_for(DisplayMode::Main, Duration::from_millis(433)),
            Some((400_000, 10, 1280))
        );
    }

    #[test]
    fn a_hidden_display_keeps_the_full_stream_however_long_its_hello_took() {
        assert_eq!(video_for(DisplayMode::Hidden, Duration::from_secs(3)), None);
    }

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
    fn the_owner_file_names_this_computer_by_a_hash_and_not_by_its_name() {
        let me = owner();
        assert!(!me.contains(&host_name()), "{me}");
        let host = me.split_once(" host=").unwrap().1;
        assert!(
            host.len() == 16 && host.chars().all(|c| c.is_ascii_hexdigit()),
            "{me}"
        );
        let lines = vec![format!("owned pid=4242 host={host}")];
        let e = not_ready(Some(OWNED), false, READY_TIMEOUT, &lines, "emulator-5556");
        assert!(
            e.hint().contains("`kill 4242` on this computer"),
            "{}",
            e.hint()
        );
        let lines = vec!["owned pid=4242 host=0123456789abcdef".to_string()];
        let e = not_ready(Some(OWNED), false, READY_TIMEOUT, &lines, "emulator-5556");
        assert!(e.hint().contains("on another computer"), "{}", e.hint());
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
    fn an_agent_that_finds_no_accessibility_service_says_the_system_is_not_up_and_to_wait() {
        let seen = "java.lang.reflect.InvocationTargetException
\tat java.lang.reflect.Method.invoke(Native Method)
\tat dev.phonedriver.agent.Accessibility.connect(Accessibility.java:50)
\tat dev.phonedriver.agent.Main.run(Main.java:59)
Caused by: java.lang.NullPointerException: Attempt to invoke interface method 'void android.view.accessibility.IAccessibilityManager.registerUiTestAutomationService(android.os.IBinder, android.accessibilityservice.IAccessibilityServiceClient, android.accessibilityservice.AccessibilityServiceInfo, int, int)' on a null object reference
\tat android.app.UiAutomationConnection.registerUiTestAutomationServiceLocked(UiAutomationConnection.java:689)
\t... 6 more";
        let lines: Vec<String> = seen.lines().map(str::to_string).collect();
        let e = not_ready(Some(1), false, READY_TIMEOUT, &lines, "emulator-5554");
        assert!(matches!(e, Error::Refused { .. }), "{e:?}");
        let text = e.to_string();
        assert!(
            text.contains("accessibility service is not running on emulator-5554")
                && text.contains("Wait 30 seconds"),
            "{text}"
        );
        assert!(!text.contains("UiAutomationConnection"), "{text}");
    }

    #[test]
    fn an_agent_that_dies_of_an_exception_is_reported_without_the_stack_frames() {
        let lines: Vec<String> = [
            "java.lang.IllegalStateException: boom",
            "\tat a.b.C.d(C.java:1)",
            "Caused by: java.lang.RuntimeException: why",
            "\tat x.Y.z(Y.java:2)",
            "\t... 6 more",
        ]
        .map(str::to_string)
        .to_vec();
        let text = not_ready(Some(1), false, READY_TIMEOUT, &lines, "emulator-5554").to_string();
        assert!(
            text.ends_with("before it was ready: java.lang.IllegalStateException: boom\nCaused by: java.lang.RuntimeException: why"),
            "{text}"
        );
    }

    #[test]
    fn a_hello_refused_for_the_hidden_display_is_unsupported_and_a_lane_in_use_is_refused() {
        let no_display = json!({"id": 0, "ok": false,
            "error": "java.lang.IllegalStateException: hidden display: needs Android 13 (API 33) or newer, this is API 26"});
        assert!(matches!(
            refused_hello(&no_display),
            Error::Unsupported {
                op: Op::Display,
                ..
            }
        ));
        let taken = json!({"id": 0, "ok": false, "error": "lane in use: a"});
        let Error::Refused { detail } = refused_hello(&taken) else {
            panic!("a lane in use is a refusal");
        };
        assert!(detail.contains("lane a"), "{detail}");
        let locked = json!({"id": 0, "ok": false, "error": "the phone is locked"});
        assert!(matches!(
            refused_hello(&locked),
            Error::NeedsUser {
                action: UserAction::UnlockDevice
            }
        ));
        let other = json!({"id": 0, "ok": false, "error": "agent is version 0.1.3, host is 0.1.4"});
        assert!(matches!(refused_hello(&other), Error::Transport { .. }));
    }
}
