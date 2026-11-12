//! PhonedriverRunner on simulators: building it, starting it, and talking to it (protocol section 3).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use phonedriver_core::error::{Error, Op};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::time::{Instant, timeout};

pub type Result<T> = std::result::Result<T, Error>;

/// The wire version the runner reports in `hello`.
pub const VERSION: &str = "0.1.0";
/// The first port a simulator runner listens on; each further simulator takes the next free one.
pub const PORT_BASE: u16 = 27753;
const PORT_COUNT: u16 = 32;
const READY_WAIT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(100);
const CONNECT_WAIT: Duration = Duration::from_millis(300);
const HELLO_WAIT: Duration = Duration::from_secs(2);
const MAX_FRAME: usize = 64 << 20;

/// The runner's sources, built into the binary so the daemon can build the runner wherever it runs.
pub const SOURCES: &[(&str, &str)] = &[
    (
        "project.yml",
        include_str!("../../../ios-runner/project.yml"),
    ),
    (
        "PrivateHeaders/XCTestPrivate.h",
        include_str!("../../../ios-runner/PrivateHeaders/XCTestPrivate.h"),
    ),
    (
        "Runner/Buttons.h",
        include_str!("../../../ios-runner/Runner/Buttons.h"),
    ),
    (
        "Runner/Buttons.m",
        include_str!("../../../ios-runner/Runner/Buttons.m"),
    ),
    (
        "Runner/Gesture.h",
        include_str!("../../../ios-runner/Runner/Gesture.h"),
    ),
    (
        "Runner/Gesture.m",
        include_str!("../../../ios-runner/Runner/Gesture.m"),
    ),
    (
        "Runner/Json.h",
        include_str!("../../../ios-runner/Runner/Json.h"),
    ),
    (
        "Runner/Json.m",
        include_str!("../../../ios-runner/Runner/Json.m"),
    ),
    (
        "Runner/PhonedriverRunnerTests.m",
        include_str!("../../../ios-runner/Runner/PhonedriverRunnerTests.m"),
    ),
    (
        "Runner/Screenshot.h",
        include_str!("../../../ios-runner/Runner/Screenshot.h"),
    ),
    (
        "Runner/Screenshot.m",
        include_str!("../../../ios-runner/Runner/Screenshot.m"),
    ),
    (
        "Runner/Server.h",
        include_str!("../../../ios-runner/Runner/Server.h"),
    ),
    (
        "Runner/Server.m",
        include_str!("../../../ios-runner/Runner/Server.m"),
    ),
    (
        "Runner/Snapshot.h",
        include_str!("../../../ios-runner/Runner/Snapshot.h"),
    ),
    (
        "Runner/Snapshot.m",
        include_str!("../../../ios-runner/Runner/Snapshot.m"),
    ),
    (
        "Runner/Typing.h",
        include_str!("../../../ios-runner/Runner/Typing.h"),
    ),
    (
        "Runner/Typing.m",
        include_str!("../../../ios-runner/Runner/Typing.m"),
    ),
];

/// A short hash of `SOURCES`. Builds are cached under it, and a running runner is reused only when it reports it.
pub fn build_hash() -> String {
    let mut h = Sha256::new();
    for (path, text) in SOURCES {
        h.update(path.as_bytes());
        h.update([0]);
        h.update(text.as_bytes());
        h.update([0]);
    }
    h.finalize()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Screen {
    pub w: f32,
    pub h: f32,
    pub scale: f32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Hello {
    pub version: String,
    pub ios: String,
    pub model: String,
    pub screen: Screen,
    #[serde(default)]
    pub udid: Option<String>,
    #[serde(default)]
    pub build: Option<String>,
}

/// One connection to a runner. Requests run one at a time, as the runner serves them.
/// What the runner's socket is made of: TCP to a simulator's loopback, or a usbmuxd connection to a device.
pub trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

pub struct Client {
    device: String,
    stream: tokio::sync::Mutex<Box<dyn Io>>,
    next_id: AtomicU64,
}

impl Client {
    pub async fn connect(port: u16, device: &str) -> std::io::Result<Client> {
        let stream = timeout(CONNECT_WAIT, TcpStream::connect(("127.0.0.1", port)))
            .await
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))??;
        stream.set_nodelay(true)?;
        Ok(Client::over(stream, device))
    }

    pub fn over(stream: impl Io + 'static, device: &str) -> Client {
        Client {
            device: device.to_string(),
            stream: tokio::sync::Mutex::new(Box::new(stream)),
            next_id: AtomicU64::new(1),
        }
    }

    /// Sends `req` (an object with an `op`) and returns the reply's fields. `op` names the operation in errors.
    pub async fn request(&self, op: Op, mut req: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        req["id"] = id.into();
        let gone = |_| Error::DeviceGone {
            id: self.device.clone(),
        };
        let mut stream = self.stream.lock().await;
        write_frame(&mut *stream, &req).await.map_err(gone)?;
        let reply = loop {
            let reply = read_frame(&mut *stream).await.map_err(gone)?;
            if reply["id"] == id {
                break reply;
            }
        };
        if reply["ok"] == true {
            Ok(reply)
        } else {
            Err(reply_error(op, &reply["error"]))
        }
    }

    pub async fn hello(&self) -> Result<Hello> {
        let reply = self
            .request(Op::Foreground, json!({"op": "hello", "version": VERSION}))
            .await?;
        serde_json::from_value(reply).map_err(|e| Error::Transport {
            detail: format!("the runner's hello does not parse: {e}"),
        })
    }
}

fn reply_error(op: Op, error: &Value) -> Error {
    let message = error["message"].as_str().unwrap_or("no message");
    match error["code"].as_str() {
        Some("unsupported") => Error::Unsupported {
            op,
            reason: match op {
                Op::Button => "the Simulator has no volume buttons",
                _ => "the runner cannot do this on this device",
            },
        },
        _ => Error::Transport {
            detail: format!("runner {}: {message}", op.name()),
        },
    }
}

async fn write_frame(stream: &mut (impl AsyncWrite + Unpin), v: &Value) -> std::io::Result<()> {
    let body = v.to_string();
    let mut buf = Vec::with_capacity(4 + body.len());
    buf.extend_from_slice(&(body.len() as u32).to_be_bytes());
    buf.extend_from_slice(body.as_bytes());
    stream.write_all(&buf).await
}

async fn read_frame(stream: &mut (impl AsyncRead + Unpin)) -> std::io::Result<Value> {
    let mut len = [0u8; 4];
    stream.read_exact(&mut len).await?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(std::io::Error::other(format!("frame of {len} bytes")));
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await?;
    serde_json::from_slice(&body).map_err(std::io::Error::other)
}

pub struct Build {
    pub xctestrun: PathBuf,
    pub hash: String,
    /// False when an earlier build of the same sources was reused.
    pub built: bool,
}

static BUILDING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Builds the runner for the Simulator once per source hash, under `<home>/runner/<hash>`, and removes older builds.
pub async fn build(home: &Path) -> Result<Build> {
    build_for(home, None).await
}

/// With `team`, builds for physical devices signed with that Apple developer team, under `<home>/runner/<hash>-device`.
pub async fn build_for(home: &Path, team: Option<&str>) -> Result<Build> {
    let _one_at_a_time = BUILDING.lock().await;
    let hash = build_hash();
    let root = home.join("runner");
    let dir = root.join(match team {
        Some(_) => format!("{hash}-device"),
        None => hash.clone(),
    });
    let products = dir.join("dd").join("Build").join("Products");
    if let Some(xctestrun) = find_xctestrun(&products) {
        return Ok(Build {
            xctestrun,
            hash,
            built: false,
        });
    }
    let io = |e: std::io::Error| Error::Transport {
        detail: format!("cannot prepare the runner build in {}: {e}", dir.display()),
    };
    let _ = std::fs::remove_dir_all(&dir);
    let src = dir.join("src");
    for (path, text) in SOURCES {
        let file = src.join(path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        std::fs::write(&file, text).map_err(io)?;
    }
    let log = dir.join("build.log");
    let xcodegen = find_tool("xcodegen").ok_or_else(|| Error::Transport {
        detail: "xcodegen is not installed; install it with `brew install xcodegen`".into(),
    })?;
    run_logged(
        Command::new(xcodegen)
            .args(["generate", "--quiet"])
            .current_dir(&src),
        &log,
        "xcodegen generate",
    )
    .await?;
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    };
    let mut xcodebuild = Command::new("xcodebuild");
    xcodebuild.args([
        "-project",
        "PhonedriverRunner.xcodeproj",
        "-scheme",
        "PhonedriverRunner",
    ]);
    match team {
        None => {
            xcodebuild
                .args(["-destination", "generic/platform=iOS Simulator"])
                .arg(format!("ARCHS={arch}"))
                .arg("ONLY_ACTIVE_ARCH=NO");
        }
        Some(team) => {
            xcodebuild
                .args([
                    "-destination",
                    "generic/platform=iOS",
                    "-allowProvisioningUpdates",
                ])
                .arg(format!("DEVELOPMENT_TEAM={team}"))
                .args([
                    "CODE_SIGN_STYLE=Automatic",
                    "CODE_SIGN_IDENTITY=Apple Development",
                ]);
        }
    }
    run_logged(
        xcodebuild
            .arg("-derivedDataPath")
            .arg(dir.join("dd"))
            .args(["-quiet", "build-for-testing"])
            .current_dir(&src),
        &log,
        "xcodebuild build-for-testing",
    )
    .await?;
    let xctestrun = find_xctestrun(&products).ok_or_else(|| Error::Transport {
        detail: format!(
            "the runner built but no .xctestrun file is in {}",
            products.display()
        ),
    })?;
    for entry in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name != hash && name.len() == 16 && name.chars().all(|c| c.is_ascii_hexdigit()) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
    Ok(Build {
        xctestrun,
        hash,
        built: true,
    })
}

fn find_xctestrun(products: &Path) -> Option<PathBuf> {
    std::fs::read_dir(products)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "xctestrun"))
}

/// Looks in `PATH`, then in Homebrew's two prefixes: a daemon started by a GUI app or a remote shell
/// gets a `PATH` without them (observed on the Mac mini: `/usr/bin:/bin:/usr/sbin:/sbin`).
pub fn find_tool(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain([
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ])
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

async fn run_logged(cmd: &mut Command, log: &Path, what: &str) -> Result<()> {
    let file = std::fs::File::create(log).map_err(|e| Error::Transport {
        detail: format!("cannot write {}: {e}", log.display()),
    })?;
    let err = file.try_clone().map_err(|e| Error::Transport {
        detail: format!("cannot write {}: {e}", log.display()),
    })?;
    let status = cmd
        .stdin(std::process::Stdio::null())
        .stdout(file)
        .stderr(err)
        .kill_on_drop(true)
        .status()
        .await
        .map_err(|e| Error::Transport {
            detail: format!("cannot run {what}: {e}"),
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Transport {
            detail: format!(
                "{what} failed ({status}); the runner did not build, see {}",
                log.display()
            ),
        })
    }
}

static CLAIMED: Mutex<Option<HashSet<u16>>> = Mutex::new(None);

/// A port this process uses for a runner, so two simulators started at once never pick the same one.
struct PortClaim(u16);

impl PortClaim {
    fn take(port: u16) -> Option<PortClaim> {
        let fresh = CLAIMED
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_or_insert_with(HashSet::new)
            .insert(port);
        fresh.then(|| PortClaim(port))
    }

    fn is_claimed(port: u16) -> bool {
        CLAIMED
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|c| c.contains(&port))
    }
}

impl Drop for PortClaim {
    fn drop(&mut self) {
        if let Some(c) = CLAIMED.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            c.remove(&self.0);
        }
    }
}

/// A runner serving one simulator. Dropping it ends the `xcodebuild` child this process started, which ends the
/// runner; a reused runner keeps running.
pub struct Runner {
    pub client: Client,
    pub hello: Hello,
    pub port: u16,
    child: Option<Child>,
    _claim: Option<PortClaim>,
}

impl Runner {
    /// True when the runner was already running (started by an earlier daemon) and this process only connected.
    pub fn reused(&self) -> bool {
        self.child.is_none()
    }

    /// Connects to this simulator's runner if one with the same version and build already answers on a runner port;
    /// otherwise builds the runner (once per source hash) and starts it with `xcodebuild test-without-building` on
    /// the first free port from `PORT_BASE`, as a child in this process's process group.
    pub async fn start(udid: &str, home: &Path) -> Result<Runner> {
        let device = format!("sim:{udid}");
        let hash = build_hash();
        let mut free = Vec::new();
        for port in PORT_BASE..PORT_BASE + PORT_COUNT {
            if PortClaim::is_claimed(port) {
                continue;
            }
            match Client::connect(port, &device).await {
                Ok(client) => {
                    let Ok(Ok(hello)) = timeout(HELLO_WAIT, client.hello()).await else {
                        continue;
                    };
                    let mine = hello.udid.as_deref() == Some(udid)
                        && hello.build.as_deref() == Some(hash.as_str())
                        && hello.version == VERSION;
                    if let (true, Some(claim)) = (mine, PortClaim::take(port)) {
                        return Ok(Runner {
                            client,
                            hello,
                            port,
                            child: None,
                            _claim: Some(claim),
                        });
                    }
                }
                Err(_) => {
                    if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
                        free.push(port);
                    }
                }
            }
        }
        let (port, claim) = free
            .into_iter()
            .find_map(|p| PortClaim::take(p).map(|c| (p, c)))
            .ok_or_else(|| Error::Transport {
                detail: format!(
                    "no free runner port between {PORT_BASE} and {}",
                    PORT_BASE + PORT_COUNT - 1
                ),
            })?;
        let build = build(home).await?;
        let log = home.join("runner").join(format!("{udid}.log"));
        let file = std::fs::File::create(&log).map_err(|e| Error::Transport {
            detail: format!("cannot write {}: {e}", log.display()),
        })?;
        let err = file.try_clone().map_err(|e| Error::Transport {
            detail: format!("cannot write {}: {e}", log.display()),
        })?;
        let mut child = Command::new("xcodebuild")
            .arg("test-without-building")
            .arg("-xctestrun")
            .arg(&build.xctestrun)
            .args(["-destination", &format!("id={udid}")])
            .args(["-test-timeouts-enabled", "NO"])
            .env("TEST_RUNNER_PHONEDRIVER_PORT", port.to_string())
            .env("TEST_RUNNER_PHONEDRIVER_BUILD", &build.hash)
            .stdin(std::process::Stdio::null())
            .stdout(file)
            .stderr(err)
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| Error::Transport {
                detail: format!("cannot run xcodebuild: {e}"),
            })?;
        let deadline = Instant::now() + READY_WAIT;
        loop {
            if let Ok(client) = Client::connect(port, &device).await
                && let Ok(Ok(hello)) = timeout(HELLO_WAIT, client.hello()).await
                && hello.udid.as_deref() == Some(udid)
            {
                return Ok(Runner {
                    client,
                    hello,
                    port,
                    child: Some(child),
                    _claim: Some(claim),
                });
            }
            if let Ok(Some(status)) = child.try_wait() {
                return Err(Error::Transport {
                    detail: format!(
                        "the runner exited ({status}) before answering; see {}",
                        log.display()
                    ),
                });
            }
            if Instant::now() >= deadline {
                return Err(Error::Timeout {
                    waited_ms: READY_WAIT.as_millis() as u32,
                    condition: format!(
                        "the runner to answer on port {port}; see {}",
                        log.display()
                    ),
                });
            }
            tokio::time::sleep(POLL).await;
        }
    }
}

/// The runner's fixed port on a device; the host reaches it through usbmuxd, so nothing is claimed on the Mac.
pub const DEVICE_PORT: u16 = PORT_BASE;

impl Runner {
    /// Starts the runner on a attached device with `xcodebuild test-without-building` (signed build from
    /// `build_for(home, Some(team))`) or reuses one that answers `hello` with this udid and build.
    pub async fn start_device(
        provider: &dyn idevice::provider::IdeviceProvider,
        udid: &str,
        team: &str,
        home: &Path,
    ) -> Result<Runner> {
        let device = format!("ios:{udid}");
        let hash = build_hash();
        let connect = || async {
            let socket = provider
                .connect(DEVICE_PORT)
                .await
                .ok()
                .and_then(idevice::Idevice::get_socket)?;
            let client = Client::over(socket, &device);
            let hello = timeout(HELLO_WAIT, client.hello()).await.ok()?.ok()?;
            Some((client, hello))
        };
        if let Some((client, hello)) = connect().await
            && hello.udid.as_deref() == Some(udid)
            && hello.build.as_deref() == Some(hash.as_str())
        {
            return Ok(Runner {
                client,
                hello,
                port: DEVICE_PORT,
                child: None,
                _claim: None,
            });
        }
        let build = build_for(home, Some(team)).await?;
        let log = home.join("runner").join(format!("{udid}.log"));
        let io = |e: std::io::Error| Error::Transport {
            detail: format!("cannot write {}: {e}", log.display()),
        };
        let file = std::fs::File::create(&log).map_err(io)?;
        let err = file.try_clone().map_err(io)?;
        let mut child = Command::new("xcodebuild")
            .arg("test-without-building")
            .arg("-xctestrun")
            .arg(&build.xctestrun)
            .args(["-destination", &format!("id={udid}")])
            .args(["-test-timeouts-enabled", "NO"])
            .env("TEST_RUNNER_PHONEDRIVER_PORT", DEVICE_PORT.to_string())
            .env("TEST_RUNNER_PHONEDRIVER_BUILD", &build.hash)
            .env("TEST_RUNNER_PHONEDRIVER_UDID", udid)
            .stdin(std::process::Stdio::null())
            .stdout(file)
            .stderr(err)
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| Error::Transport {
                detail: format!("cannot run xcodebuild: {e}"),
            })?;
        let deadline = Instant::now() + READY_WAIT;
        loop {
            if let Some((client, hello)) = connect().await
                && hello.udid.as_deref() == Some(udid)
            {
                return Ok(Runner {
                    client,
                    hello,
                    port: DEVICE_PORT,
                    child: Some(child),
                    _claim: None,
                });
            }
            if let Ok(Some(status)) = child.try_wait() {
                return Err(Error::Transport {
                    detail: format!(
                        "the runner exited ({status}) before answering; see {}",
                        log.display()
                    ),
                });
            }
            if Instant::now() >= deadline {
                return Err(Error::Timeout {
                    waited_ms: READY_WAIT.as_millis() as u32,
                    condition: format!("the runner to answer on {udid}; see {}", log.display()),
                });
            }
            tokio::time::sleep(POLL).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_runner_source_file_is_embedded() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ios-runner");
        let mut on_disk = vec!["project.yml".to_string()];
        for dir in ["Runner", "PrivateHeaders"] {
            for e in std::fs::read_dir(root.join(dir)).unwrap().flatten() {
                on_disk.push(format!("{dir}/{}", e.file_name().to_string_lossy()));
            }
        }
        on_disk.sort();
        let mut embedded: Vec<String> = SOURCES.iter().map(|(p, _)| p.to_string()).collect();
        embedded.sort();
        assert_eq!(embedded, on_disk);
    }

    #[test]
    fn a_port_is_claimed_once_until_its_claim_drops() {
        let port = PORT_BASE + PORT_COUNT + 7;
        let first = PortClaim::take(port).unwrap();
        assert!(PortClaim::take(port).is_none());
        assert!(PortClaim::is_claimed(port));
        drop(first);
        assert!(!PortClaim::is_claimed(port));
        assert!(PortClaim::take(port).is_some());
    }

    #[test]
    fn build_hash_is_16_hex_chars_and_stable() {
        let h = build_hash();
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(h, build_hash());
    }

    #[test]
    fn unsupported_replies_become_unsupported_errors() {
        let e = reply_error(
            Op::Button,
            &json!({"code": "unsupported", "message": "the Simulator has no volume buttons"}),
        );
        assert!(matches!(e, Error::Unsupported { op: Op::Button, .. }));
        let e = reply_error(Op::Gesture, &json!({"code": "failed", "message": "boom"}));
        assert_eq!(
            e,
            Error::Transport {
                detail: "runner gesture: boom".into()
            }
        );
    }
}
