//! PhonedriverRunner on simulators: building it, starting it, and talking to it (protocol section 3).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hmac::{KeyInit, Mac};
use phonedriver_core::error::{Error, Op, UserAction};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::time::{Instant, timeout};

pub type Result<T> = std::result::Result<T, Error>;

/// The wire version the runner reports in `hello`. 0.2.0 added the token's proofs (decision D106).
pub const VERSION: &str = "0.2.0";
/// The first port a simulator runner listens on; each further simulator takes the next free one.
pub const PORT_BASE: u16 = 27753;
const PORT_COUNT: u16 = 32;
const READY_WAIT: Duration = Duration::from_secs(60);
/// A Simulator just booted on a CI machine took 26 s to start the runner and 41 s for SpringBoard's first answer
/// (GitHub's macOS runner, 2026-11-17), so the Simulator gets longer; a runner that exits is still seen at once.
const SIMULATOR_READY_WAIT: Duration = Duration::from_secs(180);
const POLL: Duration = Duration::from_millis(100);
const CONNECT_WAIT: Duration = Duration::from_millis(300);
const HELLO_WAIT: Duration = Duration::from_secs(2);
const MAX_FRAME: usize = 64 << 20;

/// The runner's sources, built into the binary so the daemon can build the runner wherever it runs.
pub const SOURCES: &[(&str, &str)] = &[
    (
        "PhonedriverRunner.xcodeproj/project.pbxproj",
        include_str!("../../../ios-runner/PhonedriverRunner.xcodeproj/project.pbxproj"),
    ),
    (
        "PhonedriverRunner.xcodeproj/xcshareddata/xcschemes/PhonedriverRunner.xcscheme",
        include_str!(
            "../../../ios-runner/PhonedriverRunner.xcodeproj/xcshareddata/xcschemes/PhonedriverRunner.xcscheme"
        ),
    ),
    (
        "PrivateHeaders/XCTestPrivate.h",
        include_str!("../../../ios-runner/PrivateHeaders/XCTestPrivate.h"),
    ),
    (
        "Runner/Auth.h",
        include_str!("../../../ios-runner/Runner/Auth.h"),
    ),
    (
        "Runner/Auth.m",
        include_str!("../../../ios-runner/Runner/Auth.m"),
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
    #[serde(default)]
    pub locked: bool,
}

/// The labels of the two proofs in `hello`: the host's over its nonce, and the runner's over the same nonce.
const HOST: &str = "host";
const RUNNER: &str = "runner";

/// The secret a runner is started with: 32 bytes from the OS, as 64 hex characters in the runner's environment
/// (`PHONEDRIVER_TOKEN`). It never crosses the connection: the first `hello` carries the host's proof of it over a
/// fresh nonce and the runner answers with its own (decision D106). Its `Debug` form hides it.
#[derive(Clone, PartialEq)]
pub struct Token(String);

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(..)")
    }
}

fn random_hex(bytes: usize) -> Result<String> {
    let mut b = vec![0u8; bytes];
    getrandom::fill(&mut b).map_err(|e| Error::Transport {
        detail: format!("no randomness for the runner's token: {e}"),
    })?;
    Ok(hex(&b))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
}

impl Token {
    pub fn generate() -> Result<Token> {
        random_hex(32).map(Token)
    }

    fn from_hex(text: &str) -> Option<Token> {
        (text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| Token(text.to_ascii_lowercase()))
    }

    /// The value the runner's environment carries.
    pub fn env(&self) -> &str {
        &self.0
    }

    fn file(home: &Path, udid: &str) -> Result<PathBuf> {
        Ok(home
            .join("runner")
            .join(format!("{}.token", crate::transport::file_udid(udid)?)))
    }

    /// The token of the runner this home last started on `udid`, so a later daemon can reuse that runner.
    pub fn kept(home: &Path, udid: &str) -> Option<Token> {
        Token::from_hex(
            std::fs::read_to_string(Token::file(home, udid).ok()?)
                .ok()?
                .trim(),
        )
    }

    /// Keeps the token in `<home>/runner/<udid>.token`, readable by its owner only on Unix.
    pub fn keep(&self, home: &Path, udid: &str) -> Result<()> {
        crate::transport::write_private(&Token::file(home, udid)?, self.0.as_bytes())
    }

    fn mac(&self, side: &str, nonce: &str) -> hmac::Hmac<Sha256> {
        let mut mac = hmac::Hmac::<Sha256>::new_from_slice(self.0.as_bytes())
            .expect("HMAC takes a key of any length");
        mac.update(side.as_bytes());
        mac.update(b":");
        mac.update(nonce.as_bytes());
        mac
    }

    /// HMAC-SHA256 under the token of `<side>:<nonce>`, as hex.
    fn proof(&self, side: &str, nonce: &str) -> String {
        hex(&self.mac(side, nonce).finalize().into_bytes())
    }

    /// Whether `proof` is `side`'s proof of the token over `nonce`, compared in constant time.
    fn proves(&self, side: &str, nonce: &str, proof: &str) -> bool {
        unhex(proof).is_some_and(|p| self.mac(side, nonce).verify_slice(&p).is_ok())
    }
}

/// What a runner said to the host's first `hello`.
enum Greeting {
    /// It proved the token.
    Ours(Hello),
    /// It speaks another wire version, so it cannot prove anything to this host.
    OtherVersion(String),
    /// It answered without proving the token: a listener that knows only the public `udid` and `build`.
    NotProven,
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

    /// The first request on a connection: the runner serves nothing before it, and the host trusts the runner only
    /// when the reply proves the same token.
    async fn greet(&self, token: &Token) -> Result<Greeting> {
        let nonce = random_hex(16)?;
        let reply = self
            .request(
                Op::Foreground,
                json!({"op": "hello", "version": VERSION, "nonce": nonce, "proof": token.proof(HOST, &nonce)}),
            )
            .await?;
        let proven = reply["proof"]
            .as_str()
            .is_some_and(|p| token.proves(RUNNER, &nonce, p));
        let hello: Hello = serde_json::from_value(reply).map_err(|e| Error::Transport {
            detail: format!("the runner's hello does not parse: {e}"),
        })?;
        Ok(if hello.version != VERSION {
            Greeting::OtherVersion(hello.version)
        } else if proven {
            Greeting::Ours(hello)
        } else {
            Greeting::NotProven
        })
    }

    /// Says `hello` with `token` and returns the runner's facts once it proved the token.
    pub async fn hello(&self, token: &Token) -> Result<Hello> {
        match self.greet(token).await? {
            Greeting::Ours(hello) => Ok(hello),
            Greeting::OtherVersion(version) => Err(Error::Transport {
                detail: format!(
                    "the runner speaks version {version}, and this phonedriver speaks {VERSION}"
                ),
            }),
            Greeting::NotProven => Err(Error::Transport {
                detail: "the runner did not prove the token it was started with".into(),
            }),
        }
    }
}

/// The runner's `hello` on `client` if it proves `kept` and reports `udid` and the build `hash`. Without a kept token
/// nothing is reused: `udid` and `hash` are public, so only the token says this home started that runner.
async fn ours(client: &Client, kept: Option<&Token>, udid: &str, hash: &str) -> Option<Hello> {
    let Ok(Ok(Greeting::Ours(hello))) = timeout(HELLO_WAIT, client.greet(kept?)).await else {
        return None;
    };
    (hello.udid.as_deref() == Some(udid) && hello.build.as_deref() == Some(hash)).then_some(hello)
}

/// The runner started with `token` on `udid` answering `client`, for the waits after a start.
async fn answers(client: &Client, token: &Token, udid: &str) -> Option<Greeting> {
    match timeout(HELLO_WAIT, client.greet(token)).await {
        Ok(Ok(Greeting::Ours(hello))) if hello.udid.as_deref() != Some(udid) => None,
        Ok(Ok(greeting)) => Some(greeting),
        _ => None,
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

/// Builds the runner for the Simulator once per source hash, under `<home>/runner/<hash>`, and removes the builds
/// of other sources.
pub async fn build(home: &Path) -> Result<Build> {
    build_for(home, None).await
}

/// With `team`, builds for physical devices signed with that Apple developer team, under `<home>/runner/<hash>-device`,
/// and builds again when the signing profile of that build has run out.
pub async fn build_for(home: &Path, team: Option<&str>) -> Result<Build> {
    build_signed(home, team, None).await
}

/// Builds the device runner again for the device `udid`, letting Xcode add that device to the team's Apple
/// developer account, which is the only way its signing profile comes to include the device.
pub async fn register(home: &Path, team: &str, udid: &str) -> Result<Build> {
    build_signed(home, Some(team), Some(udid)).await
}

async fn build_signed(home: &Path, team: Option<&str>, register: Option<&str>) -> Result<Build> {
    let _one_at_a_time = BUILDING.lock().await;
    let hash = build_hash();
    let root = home.join("runner");
    let dir = root.join(match team {
        Some(_) => format!("{hash}-device"),
        None => hash.clone(),
    });
    let products = dir.join("dd").join("Build").join("Products");
    if register.is_none()
        && let Some(xctestrun) = reusable(&products, team.is_some(), SystemTime::now())
    {
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
            match register {
                Some(udid) => xcodebuild
                    .args(["-destination", &format!("id={udid}")])
                    .arg("-allowProvisioningDeviceRegistration"),
                None => xcodebuild.args(["-destination", "generic/platform=iOS"]),
            };
            xcodebuild
                .arg("-allowProvisioningUpdates")
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
    for stale in stale_builds(&root, &hash) {
        let _ = std::fs::remove_dir_all(stale);
    }
    Ok(Build {
        xctestrun,
        hash,
        built: true,
    })
}

/// Where a device build keeps its signing profile, under its build products.
pub const DEVICE_PROFILE: &str =
    "Debug-iphoneos/PhonedriverRunner-Runner.app/embedded.mobileprovision";

/// The `ExpirationDate` of a signing profile as the file writes it, `2027-11-12T20:35:22Z`. The file is a signed
/// envelope with its property list inside as plain text.
pub fn profile_expiry(profile: &Path) -> Option<String> {
    let bytes = std::fs::read(profile).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let after = text.split("<key>ExpirationDate</key>").nth(1)?;
    let date = after.split("<date>").nth(1)?.split("</date>").next()?;
    Some(date.to_string())
}

fn unix_seconds(date: &str) -> Option<u64> {
    let (day, time) = date.strip_suffix('Z')?.split_once('T')?;
    let mut ymd = day.split('-').map(|n| n.parse::<i64>().ok());
    let (y, m, d) = (ymd.next()??, ymd.next()??, ymd.next()??);
    let mut hms = time.split(':').map(|n| n.parse::<i64>().ok());
    let (h, min, sec) = (hms.next()??, hms.next()??, hms.next()??);
    let y = if m <= 2 { y - 1 } else { y };
    let year_of_era = y.rem_euclid(400);
    let day_of_year = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = y.div_euclid(400) * 146_097 + day_of_era - 719_468;
    u64::try_from(days * 86_400 + h * 3600 + min * 60 + sec).ok()
}

/// True when `date`, a profile's `ExpirationDate`, is `now` or earlier.
pub fn expired(date: &str, now: SystemTime) -> bool {
    let now = now.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    unix_seconds(date).is_some_and(|end| now >= end)
}

/// The build under `products`, unless it is signed and its profile has run out. A free Apple ID's profile lasts
/// seven days and a paid team's a year; building again signs the runner with a new one.
fn reusable(products: &Path, signed: bool, now: SystemTime) -> Option<PathBuf> {
    let xctestrun = find_xctestrun(products)?;
    let run_out = signed
        && profile_expiry(&products.join(DEVICE_PROFILE)).is_some_and(|date| expired(&date, now));
    (!run_out).then_some(xctestrun)
}

/// The builds under `root` made from other sources than `hash`, for the Simulator and for devices.
fn stale_builds(root: &Path, hash: &str) -> Vec<PathBuf> {
    std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let sources = name.strip_suffix("-device").unwrap_or(&name);
            sources != hash && sources.len() == 16 && sources.chars().all(|c| c.is_ascii_hexdigit())
        })
        .map(|entry| entry.path())
        .collect()
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
    session: Option<XctestSession>,
    _claim: Option<PortClaim>,
}

/// The XCTest session idevice holds with testmanagerd for a runner the person installed; dropping it ends the runner.
struct XctestSession(tokio::task::JoinHandle<std::result::Result<(), idevice::IdeviceError>>);

impl Drop for XctestSession {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl Runner {
    /// True when the runner was already running (started by an earlier daemon) and this process only connected.
    pub fn reused(&self) -> bool {
        self.child.is_none() && self.session.is_none()
    }

    /// Connects to this simulator's runner if one with the same version and build already answers on a runner port;
    /// otherwise builds the runner (once per source hash) and starts it with `xcodebuild test-without-building` on
    /// the first free port from `PORT_BASE`, as a child in this process's process group.
    pub async fn start(udid: &str, home: &Path) -> Result<Runner> {
        let device = format!("sim:{udid}");
        let hash = build_hash();
        let kept = Token::kept(home, udid);
        let mut free = Vec::new();
        for port in PORT_BASE..PORT_BASE + PORT_COUNT {
            if PortClaim::is_claimed(port) {
                continue;
            }
            match Client::connect(port, &device).await {
                Ok(client) => {
                    let Some(hello) = ours(&client, kept.as_ref(), udid, &hash).await else {
                        continue;
                    };
                    if let Some(claim) = PortClaim::take(port) {
                        return Ok(Runner {
                            client,
                            hello,
                            port,
                            child: None,
                            session: None,
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
        let token = Token::generate()?;
        token.keep(home, udid)?;
        let log = home.join("runner").join(format!("{udid}.log"));
        let file = std::fs::File::create(&log).map_err(|e| Error::Transport {
            detail: format!("cannot write {}: {e}", log.display()),
        })?;
        let err = file.try_clone().map_err(|e| Error::Transport {
            detail: format!("cannot write {}: {e}", log.display()),
        })?;
        let mut child = test_without_building(&build.xctestrun, udid, home)
            .env("TEST_RUNNER_PHONEDRIVER_PORT", port.to_string())
            .env("TEST_RUNNER_PHONEDRIVER_BUILD", &build.hash)
            .env("TEST_RUNNER_PHONEDRIVER_TOKEN", token.env())
            .stdin(std::process::Stdio::null())
            .stdout(file)
            .stderr(err)
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| Error::Transport {
                detail: format!("cannot run xcodebuild: {e}"),
            })?;
        let deadline = Instant::now() + SIMULATOR_READY_WAIT;
        loop {
            if let Ok(client) = Client::connect(port, &device).await
                && let Some(Greeting::Ours(hello)) = answers(&client, &token, udid).await
            {
                return Ok(Runner {
                    client,
                    hello,
                    port,
                    child: Some(child),
                    session: None,
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
                    waited_ms: SIMULATOR_READY_WAIT.as_millis() as u32,
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

/// What xcodebuild's log says a person has to do before the runner can start, if anything. The two texts are from
/// logs of an iPhone 13 Pro Max on iOS 27.0.1 (2026-11-12): the first when its signing profile did not list the
/// device, the second when it was locked.
fn blocked(log: &str) -> Option<UserAction> {
    if log.contains("0xe8008012") {
        Some(UserAction::RegisterDevice)
    } else if log.contains("because the device is locked") {
        Some(UserAction::UnlockDevice)
    } else {
        None
    }
}

/// The runner's fixed port on a device; the host reaches it through usbmuxd, so nothing is claimed on the Mac.
pub const DEVICE_PORT: u16 = PORT_BASE;

/// Where the host opens the runner's port on a device (decision D106).
pub enum DevicePort {
    /// usbmuxd on the cable; on Wi-Fi, TCP straight to the device's address, which nothing encrypts.
    Provider(std::sync::Arc<dyn idevice::provider::IdeviceProvider>),
    /// Off a Mac on Wi-Fi, inside the remote pairing's TLS-PSK tunnel, to the device's address in the tunnel.
    Tunnel(idevice::tcp::handle::AdapterHandle),
}

impl DevicePort {
    /// Through the tunnel when the developer services go through one.
    pub fn of(services: &crate::transport::Services) -> DevicePort {
        match services {
            crate::transport::Services::Lockdown(provider) => {
                DevicePort::Provider(provider.clone())
            }
            crate::transport::Services::Tunnel(tunnel) => {
                DevicePort::Tunnel(tunnel.adapter.clone())
            }
        }
    }

    async fn open(&self) -> Option<Box<dyn Io>> {
        match self {
            DevicePort::Provider(provider) => provider
                .connect(DEVICE_PORT)
                .await
                .ok()
                .and_then(idevice::Idevice::get_socket)
                .map(|s| Box::new(s) as Box<dyn Io>),
            DevicePort::Tunnel(adapter) => adapter
                .clone()
                .connect(DEVICE_PORT)
                .await
                .ok()
                .map(|s| Box::new(s) as Box<dyn Io>),
        }
    }

    async fn client(&self, device: &str) -> Option<Client> {
        Some(Client::over(self.open().await?, device))
    }
}

/// `xcodebuild test-without-building`, which runs the runner, for `xctestrun` on `udid`. Xcode otherwise leaves a new
/// folder in its own DerivedData at every start, and when the runner is stopped it gathers 120 to 250 MB of the
/// Simulator's system logs into it; here the logs go under `home`, replaced at every start, and none are gathered.
fn test_without_building(xctestrun: &Path, udid: &str, home: &Path) -> Command {
    let logs = home.join("runner").join(format!("{udid}.dd"));
    let _ = std::fs::remove_dir_all(&logs);
    let mut xcodebuild = Command::new("xcodebuild");
    xcodebuild
        .arg("test-without-building")
        .arg("-xctestrun")
        .arg(xctestrun)
        .args(["-destination", &format!("id={udid}")])
        .args(["-test-timeouts-enabled", "NO"])
        .args(["-collect-test-diagnostics", "never"])
        .arg("-derivedDataPath")
        .arg(logs);
    xcodebuild
}

impl Runner {
    /// Starts the runner on a attached device with `xcodebuild test-without-building` (signed build from
    /// `build_for(home, Some(team))`) or reuses one that answers `hello` with this udid and build.
    pub async fn start_device(
        provider: std::sync::Arc<dyn idevice::provider::IdeviceProvider>,
        udid: &str,
        team: &str,
        home: &Path,
    ) -> Result<Runner> {
        let device = format!("ios:{udid}");
        let hash = build_hash();
        let port = DevicePort::Provider(provider);
        if let Some(client) = port.client(&device).await
            && let Some(hello) = ours(&client, Token::kept(home, udid).as_ref(), udid, &hash).await
        {
            return Ok(Runner {
                client,
                hello,
                port: DEVICE_PORT,
                child: None,
                session: None,
                _claim: None,
            });
        }
        let build = build_for(home, Some(team)).await?;
        let token = Token::generate()?;
        token.keep(home, udid)?;
        let log = home.join("runner").join(format!("{udid}.log"));
        let io = |e: std::io::Error| Error::Transport {
            detail: format!("cannot write {}: {e}", log.display()),
        };
        let file = std::fs::File::create(&log).map_err(io)?;
        let err = file.try_clone().map_err(io)?;
        let mut child = test_without_building(&build.xctestrun, udid, home)
            .env("TEST_RUNNER_PHONEDRIVER_PORT", DEVICE_PORT.to_string())
            .env("TEST_RUNNER_PHONEDRIVER_BUILD", &build.hash)
            .env("TEST_RUNNER_PHONEDRIVER_UDID", udid)
            .env("TEST_RUNNER_PHONEDRIVER_TOKEN", token.env())
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
            if let Some(client) = port.client(&device).await
                && let Some(Greeting::Ours(hello)) = answers(&client, &token, udid).await
            {
                return Ok(Runner {
                    client,
                    hello,
                    port: DEVICE_PORT,
                    child: Some(child),
                    session: None,
                    _claim: None,
                });
            }
            if let Some(action) = blocked(&std::fs::read_to_string(&log).unwrap_or_default()) {
                return Err(Error::NeedsUser { action });
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

/// The unsigned runner for devices, built from this version's sources by the `runner-ipa` job of the release workflow on a macOS
/// machine, for a person without a Mac to sign and install (architecture 7.3).
pub const RUNNER_IPA: &str = concat!(
    "https://github.com/hridaya423/phonedriver/releases/download/v",
    env!("CARGO_PKG_VERSION"),
    "/PhonedriverRunner.ipa"
);

/// The executable of the runner app, `<scheme>-Runner`. A signing tool may give the app another bundle id (a free
/// Apple ID cannot register an id another team holds), so the installed runner is found by its executable.
pub const RUNNER_EXECUTABLE: &str = "PhonedriverRunner-Runner";

/// The bundle id of the runner among installation_proxy's apps, if one is installed.
pub fn installed_runner<'a>(
    apps: impl IntoIterator<Item = (&'a String, &'a plist::Value)>,
) -> Option<String> {
    apps.into_iter()
        .find(|(_, info)| {
            info.as_dictionary()
                .and_then(|d| d.get("CFBundleExecutable"))
                .and_then(plist::Value::as_string)
                == Some(RUNNER_EXECUTABLE)
        })
        .map(|(id, _)| id.clone())
}

/// The runner's bundle id on the device, if the person installed it.
pub async fn find_installed(
    services: &mut crate::transport::Services,
) -> std::result::Result<Option<String>, idevice::IdeviceError> {
    let apps = services
        .client::<idevice::services::installation_proxy::InstallationProxyClient>()
        .await?
        .get_apps(Some("User"), None)
        .await?;
    Ok(installed_runner(&apps))
}

/// Writes the runner's XCTest log lines, as xcodebuild's output does on a Mac. Typed text never reaches them
/// (section 7.3, `type`).
struct RunnerLog(std::fs::File);

impl idevice::services::dvt::xctest::listener::XCUITestListener for RunnerLog {
    async fn log_message(
        &mut self,
        message: &str,
    ) -> std::result::Result<(), idevice::IdeviceError> {
        use std::io::Write;
        let _ = writeln!(self.0, "{message}");
        Ok(())
    }

    async fn log_debug_message(
        &mut self,
        message: &str,
    ) -> std::result::Result<(), idevice::IdeviceError> {
        self.log_message(message).await
    }
}

impl Runner {
    /// Starts the runner the person installed (signed with their own Apple ID by a tool such as Sideloadly) through
    /// testmanagerd with idevice's XCTest client, so no Xcode is needed; or reuses one that answers `hello` with
    /// this udid and build. The developer disk image must be mounted. Over Wi-Fi, testmanagerd is reached through the
    /// remote pairing's tunnel in `services` and the runner at its port on the device's address.
    pub async fn start_installed(
        device: &crate::transport::Attached,
        services: &mut crate::transport::Services,
        home: &Path,
    ) -> Result<Runner> {
        use idevice::services::dvt::xctest::{TestConfig, XCUITestService};
        use idevice::services::installation_proxy::InstallationProxyClient;
        let udid = device.udid.as_str();
        let id = format!("ios:{udid}");
        let hash = build_hash();
        let gone = |e: idevice::IdeviceError| crate::transport::classify(&id, &e);
        let port = DevicePort::of(services);
        if let Some(client) = port.client(&id).await
            && let Some(hello) = ours(&client, Token::kept(home, udid).as_ref(), udid, &hash).await
        {
            return Ok(Runner {
                client,
                hello,
                port: DEVICE_PORT,
                child: None,
                session: None,
                _claim: None,
            });
        }
        let bundle = find_installed(services)
            .await
            .map_err(gone)?
            .ok_or(Error::NeedsUser {
                action: UserAction::InstallRunner,
            })?;
        let mut proxy = services
            .client::<InstallationProxyClient>()
            .await
            .map_err(gone)?;
        let mut config = TestConfig::from_installation_proxy(&mut proxy, &bundle, None)
            .await
            .map_err(gone)?;
        let token = Token::generate()?;
        token.keep(home, udid)?;
        let mut env = plist::Dictionary::new();
        env.insert("PHONEDRIVER_PORT".into(), DEVICE_PORT.to_string().into());
        env.insert("PHONEDRIVER_BUILD".into(), hash.clone().into());
        env.insert("PHONEDRIVER_UDID".into(), udid.into());
        env.insert("PHONEDRIVER_TOKEN".into(), token.env().into());
        config.runner_env = Some(env);
        let dir = home.join("runner");
        let log = dir.join(format!("{udid}.log"));
        let io = |e: std::io::Error| Error::Transport {
            detail: format!("cannot write {}: {e}", log.display()),
        };
        std::fs::create_dir_all(&dir).map_err(io)?;
        let mut listener = RunnerLog(std::fs::File::create(&log).map_err(io)?);
        let mut session = XctestSession(match services {
            crate::transport::Services::Lockdown(provider) => {
                let service = XCUITestService::new(provider.clone());
                tokio::spawn(async move { service.run(config, &mut listener, None).await })
            }
            crate::transport::Services::Tunnel(tunnel) => {
                let os = crate::transport::identity(device).await.map_err(gone)?.os;
                let major = crate::discovery::major(&os)? as u8;
                let tunnel = tunnel.clone();
                tokio::spawn(async move {
                    XCUITestService::run_over_rsd(
                        tunnel.adapter,
                        &tunnel.rsd,
                        major,
                        config,
                        &mut listener,
                        None,
                    )
                    .await
                })
            }
        });
        let deadline = Instant::now() + READY_WAIT;
        loop {
            if let Some(client) = port.client(&id).await
                && let Some(greeting) = answers(&client, &token, udid).await
            {
                let hello = match greeting {
                    Greeting::Ours(hello) => hello,
                    Greeting::OtherVersion(version) => {
                        return Err(Error::Transport {
                            detail: format!(
                                "the runner installed on {udid} speaks version {version}, and this phonedriver speaks {VERSION}; install the PhonedriverRunner.ipa built from the same commit"
                            ),
                        });
                    }
                    Greeting::NotProven => {
                        return Err(Error::Transport {
                            detail: format!(
                                "what answers on port {DEVICE_PORT} of {udid} did not prove the token the runner was started with; see {}",
                                log.display()
                            ),
                        });
                    }
                };
                return Ok(Runner {
                    client,
                    hello,
                    port: DEVICE_PORT,
                    child: None,
                    session: Some(session),
                    _claim: None,
                });
            }
            if session.0.is_finished() {
                let ended = (&mut session.0).await;
                return Err(match ended {
                    Ok(Err(e)) => match gone(e) {
                        Error::Transport { detail } => Error::Transport {
                            detail: format!(
                                "the runner did not start: {detail}; see {}",
                                log.display()
                            ),
                        },
                        other => other,
                    },
                    _ => Error::Transport {
                        detail: format!(
                            "the runner's XCTest session ended before it answered; see {}",
                            log.display()
                        ),
                    },
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

    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const NONCE: &str = "00112233445566778899aabbccddeeff";

    fn token(hex: &str) -> Token {
        Token::from_hex(hex).unwrap()
    }

    /// A runner as `Server.m` serves one: the first frame must be a `hello` that proves `token`, and the reply proves
    /// it back. `proves` false answers every hello with the right udid and build and no proof, as a listener that
    /// knows only the public values would. Returns the port and the first frame of each connection.
    async fn fake_runner(
        token: &str,
        proves: bool,
    ) -> (u16, tokio::sync::mpsc::UnboundedReceiver<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let token = token.to_string();
        let (first, frames) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let token = Token::from_hex(&token).unwrap();
                let first = first.clone();
                tokio::spawn(async move {
                    let mut authorized = false;
                    while let Ok(req) = read_frame(&mut stream).await {
                        if !authorized {
                            let _ = first.send(req.to_string());
                        }
                        let nonce = req["nonce"].as_str().unwrap_or_default().to_string();
                        let valid = req["op"] == "hello"
                            && req["proof"]
                                .as_str()
                                .is_some_and(|p| token.proves(HOST, &nonce, p));
                        if !authorized && proves && !valid {
                            let _ = write_frame(
                                &mut stream,
                                &json!({"id": req["id"], "ok": false, "error": {"code": "unauthorized", "message": "this connection did not prove the runner's token"}}),
                            )
                            .await;
                            return;
                        }
                        authorized = true;
                        let mut reply = json!({
                            "id": req["id"], "ok": true, "version": VERSION, "ios": "26.5", "model": "iPhone17,5",
                            "screen": {"w": 402, "h": 874, "scale": 3}, "udid": "A", "build": build_hash(),
                        });
                        if proves {
                            reply["proof"] = token.proof(RUNNER, &nonce).into();
                        }
                        let _ = write_frame(&mut stream, &reply).await;
                    }
                });
            }
        });
        (port, frames)
    }

    async fn greet(port: u16, token: &Token) -> Result<Greeting> {
        Client::connect(port, "sim:A")
            .await
            .unwrap()
            .greet(token)
            .await
    }

    #[test]
    fn the_proofs_are_hmac_sha256_of_the_side_and_the_nonce_under_the_token() {
        let t = token(TOKEN);
        assert_eq!(
            t.proof(HOST, NONCE),
            "697be53d43916d03fc8a70e5b6fd692fdee4d872d846bb4c3271d03fbe95120b"
        );
        assert_eq!(
            t.proof(RUNNER, NONCE),
            "7367f1e29a5a277032c0c58465a5988d0c8db3526755fb2409d41e1f3616b8ba"
        );
        assert!(t.proves(
            RUNNER,
            NONCE,
            "7367f1e29a5a277032c0c58465a5988d0c8db3526755fb2409d41e1f3616b8ba"
        ));
        assert!(!t.proves(
            RUNNER,
            NONCE,
            "697be53d43916d03fc8a70e5b6fd692fdee4d872d846bb4c3271d03fbe95120b"
        ));
        assert!(!t.proves(RUNNER, NONCE, "not hex"));
        assert!(!t.proves(RUNNER, NONCE, ""));
    }

    #[test]
    fn each_token_is_32_fresh_bytes_and_its_debug_form_hides_it() {
        let (a, b) = (Token::generate().unwrap(), Token::generate().unwrap());
        assert_ne!(a, b);
        assert_eq!(a.env().len(), 64);
        assert!(a.env().chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!format!("{a:?}").contains(a.env()));
        assert!(Token::from_hex("abc").is_none());
    }

    #[tokio::test]
    async fn a_runner_that_proves_the_token_is_ours_and_the_token_never_crosses_the_wire() {
        let (port, mut first) = fake_runner(TOKEN, true).await;
        let Greeting::Ours(hello) = greet(port, &token(TOKEN)).await.unwrap() else {
            panic!("the runner started with this token was not recognised")
        };
        assert_eq!(hello.udid.as_deref(), Some("A"));
        let frame = first.recv().await.unwrap();
        assert!(!frame.contains(TOKEN), "{frame}");
        assert!(frame.contains("\"proof\""), "{frame}");
    }

    #[tokio::test]
    async fn a_runner_started_with_another_token_refuses_the_connection_and_is_not_ours() {
        let (port, _first) = fake_runner(TOKEN, true).await;
        let other = Token::generate().unwrap();
        assert!(greet(port, &other).await.is_err());
    }

    #[tokio::test]
    async fn a_listener_that_answers_with_the_right_udid_and_build_but_no_proof_is_not_ours() {
        let (port, _first) = fake_runner(TOKEN, false).await;
        assert!(matches!(
            greet(port, &token(TOKEN)).await,
            Ok(Greeting::NotProven)
        ));
    }

    #[tokio::test]
    async fn a_runner_is_reused_only_with_the_token_this_home_kept_for_it() {
        let (port, _first) = fake_runner(TOKEN, true).await;
        let client = || async { Client::connect(port, "sim:A").await.unwrap() };
        let hash = build_hash();
        assert!(ours(&client().await, None, "A", &hash).await.is_none());
        assert!(
            ours(
                &client().await,
                Some(&Token::generate().unwrap()),
                "A",
                &hash
            )
            .await
            .is_none()
        );
        assert!(
            ours(&client().await, Some(&token(TOKEN)), "B", &hash)
                .await
                .is_none()
        );
        assert!(
            ours(
                &client().await,
                Some(&token(TOKEN)),
                "A",
                "0000000000000000"
            )
            .await
            .is_none()
        );
        assert!(
            ours(&client().await, Some(&token(TOKEN)), "A", &hash)
                .await
                .is_some()
        );
    }

    #[test]
    fn a_kept_token_is_read_back_and_written_for_its_owner_only() {
        let home = tempfile::tempdir().unwrap();
        assert!(Token::kept(home.path(), "A").is_none());
        let t = Token::generate().unwrap();
        t.keep(home.path(), "A").unwrap();
        assert_eq!(Token::kept(home.path(), "A"), Some(t.clone()));
        assert!(Token::kept(home.path(), "B").is_none());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let file = home.path().join("runner").join("A.token");
            assert_eq!(
                std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let again = Token::generate().unwrap();
        again.keep(home.path(), "A").unwrap();
        assert_eq!(Token::kept(home.path(), "A"), Some(again));
        assert!(t.keep(home.path(), "../A").is_err());
    }

    #[tokio::test]
    async fn through_the_remote_pairing_tunnel_the_runner_port_is_opened_inside_the_tunnel() {
        let (ours, mut device) = tokio::io::duplex(1 << 16);
        let server: std::net::Ipv6Addr = "fd00::1".parse().unwrap();
        let adapter = idevice::tcp::adapter::Adapter::new(
            Box::new(ours),
            "fd00::2".parse().unwrap(),
            server.into(),
        )
        .to_async_handle();
        let port = DevicePort::Tunnel(adapter);
        let opening = tokio::spawn(async move { port.open().await.is_some() });
        let mut syn = [0u8; 44];
        timeout(Duration::from_secs(5), device.read_exact(&mut syn))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(syn[0] >> 4, 6, "not an IPv6 packet");
        assert_eq!(syn[6], 6, "not TCP");
        assert_eq!(syn[24..40], server.octets());
        assert_eq!(u16::from_be_bytes([syn[42], syn[43]]), DEVICE_PORT);
        opening.abort();
    }

    #[test]
    fn the_installed_runner_is_found_by_its_executable_whatever_its_bundle_id() {
        let app = |executable: &str| {
            let mut info = plist::Dictionary::new();
            info.insert("CFBundleExecutable".into(), executable.into());
            plist::Value::Dictionary(info)
        };
        let mut apps = std::collections::HashMap::new();
        apps.insert("com.example.notes".to_string(), app("Notes"));
        assert_eq!(installed_runner(&apps), None);
        apps.insert(
            "dev.phonedriver.runner.xctrunner.ABCDE12345".to_string(),
            app("PhonedriverRunner-Runner"),
        );
        assert_eq!(
            installed_runner(&apps),
            Some("dev.phonedriver.runner.xctrunner.ABCDE12345".to_string())
        );
    }

    #[test]
    fn the_project_builds_every_source_file_the_binary_carries() {
        let (_, project) = SOURCES
            .iter()
            .find(|(path, _)| path.ends_with("project.pbxproj"))
            .unwrap();
        for (path, _) in SOURCES.iter().filter(|(p, _)| p.starts_with("Runner/")) {
            let name = path.trim_start_matches("Runner/");
            let listed = if name.ends_with(".m") {
                format!("/* {name} in Sources */")
            } else {
                format!("path = {name};")
            };
            assert!(project.contains(&listed), "{path} is not in the project");
        }
    }

    #[test]
    fn an_unregistered_or_locked_device_in_the_log_is_a_user_action_and_a_clean_log_is_not() {
        let unregistered = "Failed to install embedded profile for dev.phonedriver.runner.xctrunner : 0xe8008012 (This provisioning profile cannot be installed on this device.)";
        assert_eq!(blocked(unregistered), Some(UserAction::RegisterDevice));
        let locked = "NSLocalizedRecoverySuggestion=Xcode cannot launch PhonedriverRunner on the iPhone because the device is locked.";
        assert_eq!(blocked(locked), Some(UserAction::UnlockDevice));
        assert_eq!(
            blocked("Test Case '-[PhonedriverRunnerTests testServe]' started."),
            None
        );
    }

    const PROFILE: &str = "Debug-iphoneos/PhonedriverRunner-Runner.app/embedded.mobileprovision";

    fn products_with_profile(expires: Option<&str>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Runner_iphoneos27.0-arm64.xctestrun"), "").unwrap();
        if let Some(expires) = expires {
            let profile = dir.path().join(PROFILE);
            std::fs::create_dir_all(profile.parent().unwrap()).unwrap();
            let mut bytes = vec![0x30, 0x82, 0xff, 0xfe, 0x06, 0x09];
            bytes.extend_from_slice(
                format!("<plist><dict>\n\t<key>ExpirationDate</key>\n\t<date>{expires}</date>\n</dict></plist>")
                    .as_bytes(),
            );
            bytes.extend_from_slice(&[0xa0, 0x82, 0x00, 0xff]);
            std::fs::write(profile, bytes).unwrap();
        }
        dir
    }

    #[test]
    fn a_date_in_a_profile_is_read_as_seconds_since_1970() {
        assert_eq!(unix_seconds("2027-11-12T20:35:22Z"), Some(1_826_051_722));
        assert_eq!(unix_seconds("2024-02-29T23:59:59Z"), Some(1_709_251_199));
        assert_eq!(unix_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix_seconds("next week"), None);
    }

    #[test]
    fn a_signed_build_is_used_again_until_its_profile_runs_out() {
        let at = |secs: u64| std::time::UNIX_EPOCH + Duration::from_secs(secs);
        let week = products_with_profile(Some("2026-11-16T00:00:00Z"));
        assert_eq!(
            profile_expiry(&week.path().join(PROFILE)).as_deref(),
            Some("2026-11-16T00:00:00Z")
        );
        assert!(reusable(week.path(), true, at(1_794_787_199)).is_some());
        assert!(reusable(week.path(), true, at(1_794_787_200)).is_none());
        assert!(
            reusable(week.path(), false, at(1_794_787_200)).is_some(),
            "a Simulator build is not signed"
        );
        let unreadable = products_with_profile(None);
        assert!(reusable(unreadable.path(), true, at(1_794_787_200)).is_some());
        assert!(reusable(&unreadable.path().join("missing"), true, at(0)).is_none());
    }

    #[test]
    fn builds_of_other_sources_are_stale_for_the_simulator_and_for_devices() {
        let root = tempfile::tempdir().unwrap();
        for name in [
            "0f8c198b18fb0011",
            "0f8c198b18fb0011-device",
            "4aa8a282c62f4dee",
            "4aa8a282c62f4dee-device",
            "00008110-001E04900239801E.log",
            "notes",
        ] {
            std::fs::create_dir(root.path().join(name)).unwrap();
        }
        let mut stale: Vec<String> = stale_builds(root.path(), "4aa8a282c62f4dee")
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        stale.sort();
        assert_eq!(stale, ["0f8c198b18fb0011", "0f8c198b18fb0011-device"]);
    }

    #[test]
    fn every_runner_source_file_is_embedded() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ios-runner");
        fn walk(root: &Path, dir: &str, out: &mut Vec<String>) {
            for e in std::fs::read_dir(root.join(dir)).unwrap().flatten() {
                let name = format!("{dir}/{}", e.file_name().to_string_lossy());
                if e.path().is_dir() {
                    if !name.ends_with("/xcuserdata") && !name.ends_with("/project.xcworkspace") {
                        walk(root, &name, out);
                    }
                } else {
                    out.push(name);
                }
            }
        }
        let mut on_disk = Vec::new();
        for dir in ["Runner", "PrivateHeaders", "PhonedriverRunner.xcodeproj"] {
            walk(&root, dir, &mut on_disk);
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
