use std::fmt;

use async_trait::async_trait;
use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, watch};

use crate::error::{DeviceId, Error, Op};
use crate::files::Shelf;
use crate::geometry::{Point, Rect, Size};
use crate::network::{LogWait, LogWaited, NetworkView};
use crate::notice::Notices;
use crate::raw::RawTree;
use crate::settings::{SettingOp, SettingReply};

pub type Result<T> = std::result::Result<T, Error>;

bitflags! {
    /// What a device can do. The session checks these and returns `Unsupported` instead of calling.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
    pub struct Capabilities: u32 {
        const TREE = 1;
        const FRAMES = 1 << 1;
        const SCREENSHOT = 1 << 2;
        const GESTURE = 1 << 3;
        const MULTI_TOUCH = 1 << 4;
        const KEY = 1 << 5;
        const BUTTON = 1 << 6;
        const OPEN_URL = 1 << 7;
        const TEXT_PASTE = 1 << 8;
        const TEXT_SET = 1 << 9;
        const TEXT_KEYS = 1 << 10;
        const CLIPBOARD = 1 << 11;
        const APPS = 1 << 12;
        const LAUNCH = 1 << 13;
        const FOREGROUND = 1 << 14;
        const SHELL = 1 << 15;
        /// The device sends `DeviceEvent::Locked` whenever its screen turns on or off or its lock state changes.
        const POWER_EVENTS = 1 << 16;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: DeviceId,
    pub model: String,
    pub os: String,
    /// Screen size in points, current orientation.
    pub screen: Size,
    /// Device pixels per point.
    pub density: f32,
    pub capabilities: Capabilities,
}

/// The latest decoded frame's sequence number and its change against the previous frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FrameRef {
    pub seq: u64,
    pub changed_area_pt2: f32,
    pub changed_box: Option<Rect>,
}

/// Screenshot quality, 0 to 100.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quality(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    Png,
    Jpeg,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    pub bytes: Vec<u8>,
}

impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Image({}x{} {:?}, {} bytes)",
            self.width,
            self.height,
            self.format,
            self.bytes.len()
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceEvent {
    WindowChanged {
        package: Option<String>,
    },
    DisplayChanged {
        screen: Size,
        rotation: u8,
    },
    /// Pixels per point changed, so every coordinate the device was built with is stale: reconnect.
    DensityChanged,
    Locked(bool),
    Gone,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimedPoint {
    pub at: Point,
    pub t_ms: u32,
}

/// Fingers with timed paths in points. A tap is one finger with two points at the same place.
#[derive(Debug, Clone, PartialEq)]
pub struct Gesture {
    pub fingers: Vec<Vec<TimedPoint>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Key {
    Enter,
    Backspace,
    Tab,
    Escape,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    Home,
    Back,
    Recents,
    Notifications,
    Lock,
    VolumeUp,
    VolumeDown,
    /// A touch of the fingerprint sensor by the finger a test enrolled. Only an emulator has a sensor that can
    /// be touched from outside.
    Fingerprint,
    /// A touch by a finger that is not enrolled.
    WrongFingerprint,
    /// Turn the screen on its side, as when the device is turned a quarter to the left.
    Landscape,
    /// Turn the screen upright.
    Portrait,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url(pub String);

/// Text to enter. `Debug` never prints the text, because it may be a secret.
#[derive(Clone, PartialEq, Eq)]
pub enum TextInput {
    Paste(String),
    /// Paste with the clipboard marked sensitive, so the system hides the text in its clipboard preview.
    PasteSecret(String),
    SetValue(String),
    /// Set a secret as the field's value. Where the device falls back to a paste, the clipboard is marked sensitive.
    SetSecret(String),
    Keystrokes(String),
}

impl fmt::Debug for TextInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (mode, text) = match self {
            TextInput::Paste(t) => ("Paste", t),
            TextInput::PasteSecret(t) => ("PasteSecret", t),
            TextInput::SetValue(t) => ("SetValue", t),
            TextInput::SetSecret(t) => ("SetSecret", t),
            TextInput::Keystrokes(t) => ("Keystrokes", t),
        };
        write!(f, "{mode}({} chars)", text.chars().count())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ClipboardOp {
    Get,
    Set(String),
}

impl fmt::Debug for ClipboardOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClipboardOp::Get => write!(f, "Get"),
            ClipboardOp::Set(t) => write!(f, "Set({} chars)", t.chars().count()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AppId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct App {
    pub id: AppId,
    pub name: String,
    pub system: bool,
}

/// What `Device::app_op` does to an installed app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppOp {
    Stop,
    ClearData,
    Uninstall,
}

/// What `Device::logs` reads: the log lines of `app` from the last `since`, or (`crashes`) only whether it crashed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRequest {
    pub app: AppId,
    pub since: std::time::Duration,
    pub crashes: bool,
}

/// `text` is the app's lines, oldest first (empty for a crash-only request). `crash` is the app's last crash in
/// that time: the exception and the frame that threw it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogReply {
    pub text: String,
    pub crash: Option<String>,
}

/// The web page in front as its browser has it: the text a person can read on it, whole, in reading order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WebPage {
    pub title: String,
    pub url: String,
    pub text: String,
}

/// A finished screen recording: an MP4 file of `frames` pictures over `ms`. `early` says why it ended before it was
/// stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    pub path: std::path::PathBuf,
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    pub ms: u64,
    pub early: Option<String>,
}

/// `PHONEDRIVER_HOST_SHELL=1` in the daemon's environment lets `shell` act on the computer as well as the device: on
/// an iOS Simulator run programs and name the Mac's files, on an Android emulator send any console command (decision
/// D108).
pub const HOST_SHELL: &str = "PHONEDRIVER_HOST_SHELL";

/// Whether the daemon was started with `PHONEDRIVER_HOST_SHELL=1`.
pub fn host_shell() -> bool {
    std::env::var_os(HOST_SHELL).is_some_and(|v| v == "1")
}

/// A command for the device's own shell. `Debug` never prints the command, because it may hold a secret.
#[derive(Clone, PartialEq, Eq)]
pub struct ShellRequest {
    pub command: String,
    pub timeout: std::time::Duration,
}

impl fmt::Debug for ShellRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Shell({} chars)", self.command.chars().count())
    }
}

#[derive(Clone, PartialEq, Eq, Default)]
pub struct ShellReply {
    pub exit: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl fmt::Debug for ShellReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ShellReply(exit {}, {} + {} bytes)",
            self.exit,
            self.stdout.len(),
            self.stderr.len()
        )
    }
}

/// Whether the screen is on and whether the keyguard is up (architecture 6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenState {
    pub on: bool,
    pub locked: bool,
    /// The keyguard needs a PIN, pattern or password, not just a swipe.
    pub secure: bool,
    /// A window has input focus; input sent before that is rejected right after a wake or an unlock.
    pub focused: bool,
}

/// Which screen an Android agent works on (decision D32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayMode {
    /// The phone's own screen.
    #[default]
    Main,
    /// A virtual display the human cannot see (Android 13 and later).
    Hidden,
}

/// Splits a device id into the device and its lane. `android:emulator-5554+a` is lane `a` of that phone: a hidden
/// display of its own, so several sessions can work on one phone at once (decision D73). A lane name is one to eight
/// lower-case letters and digits; any other suffix is part of the id.
pub fn lane_of(id: &str) -> (&str, Option<&str>) {
    match id.rsplit_once('+') {
        Some((device, lane))
            if (1..=8).contains(&lane.len())
                && lane
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) =>
        {
            (device, Some(lane))
        }
        _ => (id, None),
    }
}

/// The seam between the domain and each platform (architecture 4.8).
#[async_trait]
pub trait Device: Send + Sync {
    fn info(&self) -> &DeviceInfo;
    /// The id of the virtual display this device works on, when it works on one.
    fn hidden_display(&self) -> Option<u32> {
        None
    }
    async fn tree(&self) -> Result<RawTree>;
    /// `None` when the device has no frame stream and settle polls screenshots.
    fn frames(&self) -> Option<watch::Receiver<FrameRef>>;
    /// Whether `frames()` follows the screen as it is drawn, so a change made during an input shows when it happens.
    fn frames_follow(&self) -> bool {
        self.info().capabilities.contains(Capabilities::FRAMES)
    }
    async fn screenshot(&self, q: Quality) -> Result<Image>;
    fn events(&self) -> broadcast::Receiver<DeviceEvent>;
    async fn gesture(&self, g: Gesture) -> Result<()>;
    async fn key(&self, k: Key) -> Result<()>;
    async fn button(&self, b: Button) -> Result<()>;
    /// Opens `url`, in `app` when given, and returns the app that handles it when the platform says which.
    async fn open_url(&self, url: &Url, app: Option<&AppId>) -> Result<Option<AppId>>;
    async fn text(&self, t: TextInput) -> Result<()>;
    async fn clipboard(&self, op: ClipboardOp) -> Result<Option<String>>;
    async fn apps(&self) -> Result<Vec<App>>;
    /// Brings `app` to the front. With `fresh` it opens on its first screen, whatever screens its task still holds.
    async fn launch(&self, app: &AppId, fresh: bool) -> Result<()>;
    async fn foreground(&self) -> Result<Option<AppId>>;
    /// Grants runtime permissions to an installed app, before it is launched.
    async fn grant(&self, _app: &AppId, _permissions: &[String]) -> Result<()> {
        Err(Error::Unsupported {
            op: Op::Launch,
            reason: "this device cannot grant permissions",
        })
    }
    /// Takes runtime permissions away from an installed app, so its next request asks again.
    async fn revoke(&self, _app: &AppId, _permissions: &[String]) -> Result<()> {
        Err(Error::Unsupported {
            op: Op::App,
            reason: "this device cannot revoke permissions",
        })
    }
    /// Delivers a push notification with this JSON `payload` to an installed app, as the platform's push service
    /// would, so the app's handling of it can be tested.
    async fn push(&self, _app: &AppId, _payload: &str) -> Result<()> {
        Err(Error::Unsupported {
            op: Op::App,
            reason: "this device cannot be sent a push notification to test an app with",
        })
    }
    /// Starts writing the screen's video to an MP4 file at `path`, from the picture now on screen.
    async fn record_start(&self, _path: &std::path::Path) -> Result<()> {
        Err(Error::Unsupported {
            op: Op::Record,
            reason: "this device has no video stream to record; Android devices and the iOS Simulator record",
        })
    }
    /// Ends the recording and finishes its file. `None` when none was running.
    async fn record_stop(&self) -> Result<Option<Recorded>> {
        Err(Error::Unsupported {
            op: Op::Record,
            reason: "this device has no video stream to record; Android devices and the iOS Simulator record",
        })
    }
    async fn logs(&self, _req: &LogRequest) -> Result<LogReply> {
        Err(Error::Unsupported {
            op: Op::Logs,
            reason: "this device's app logs cannot be read; Android devices have them",
        })
    }
    /// Waits for a log line, from now on, that holds one of `req.patterns`; the device streams its own log.
    async fn log_wait(&self, _req: &LogWait) -> Result<LogWaited> {
        Err(Error::Unsupported {
            op: Op::Logs,
            reason: "this device's log cannot be followed; Android devices and the iOS Simulator can",
        })
    }
    /// The networks the device is on, their DNS, and always-on VPN state, as the platform reports them.
    async fn network(&self) -> Result<NetworkView> {
        Err(Error::Unsupported {
            op: Op::Network,
            reason: "this device's networks cannot be read; Android devices can",
        })
    }
    /// Writes `bytes` as the file `name` in a folder of the device's shared storage and tells the device's media
    /// index, so a picture shows in the gallery. Answers the file's path on the device. A file already there is
    /// not replaced.
    async fn file_push(&self, _to: Shelf, _name: &str, _bytes: &[u8]) -> Result<String> {
        Err(Error::Unsupported {
            op: Op::Files,
            reason: "files cannot be copied to or from this device; an Android device's Pictures and Download folders can",
        })
    }
    /// The bytes of the file at `path`, which `files::shared` made. A file over `limit` bytes is refused.
    async fn file_pull(&self, _path: &str, _limit: u64) -> Result<Vec<u8>> {
        Err(Error::Unsupported {
            op: Op::Files,
            reason: "files cannot be copied to or from this device; an Android device's Pictures and Download folders can",
        })
    }
    /// The notifications the device is showing, newest first, with the time on its own clock.
    async fn notifications(&self) -> Result<Notices> {
        Err(Error::Unsupported {
            op: Op::Notifications,
            reason: "this device's notifications cannot be listed; an Android device's can",
        })
    }
    /// The web page `app` shows (the app in front when `None`), read from the browser and not from the screen.
    async fn page(&self, app: Option<&AppId>) -> Result<WebPage> {
        let _ = app;
        Err(Error::Unsupported {
            op: Op::Page,
            reason: "this device's web pages cannot be read whole; on Android, Chrome's and a debug build's can",
        })
    }
    /// System settings as typed reads and writes: the index of what there is, one value, or a write (which answers
    /// with the value afterwards).
    async fn setting(&self, _op: SettingOp) -> Result<SettingReply> {
        Err(Error::Unsupported {
            op: Op::Setting,
            reason: "this device has no settings to read or write by key (Android 17 has them); use `shell`, or the Settings app",
        })
    }
    /// Installs the app file at `path` (on the daemon's machine). Returns the apps that were not on the device
    /// before: none when the file replaced one, and more than one when something else was installed meanwhile.
    async fn install(&self, _path: &std::path::Path) -> Result<Vec<AppId>> {
        Err(Error::Unsupported {
            op: Op::App,
            reason: "this device cannot install apps",
        })
    }
    /// A token that changes when the app is removed and installed again, so a record of an install can tell that
    /// app from a later one under the same id. `None` where the device has no such token.
    async fn installed_at(&self, _app: &AppId) -> Result<Option<String>> {
        Ok(None)
    }
    /// The installed app's version as its store shows it, with its build number: `4.37.1 (437001)`. `None` where
    /// the device cannot tell.
    async fn app_build(&self, _app: &AppId) -> Result<Option<String>> {
        Ok(None)
    }
    /// The phone's own serial number, which stays the same when its connection changes. `None` where the device's id
    /// already is that.
    async fn hardware_serial(&self) -> Result<Option<String>> {
        Ok(None)
    }
    async fn app_op(&self, _op: AppOp, _app: &AppId) -> Result<()> {
        Err(Error::Unsupported {
            op: Op::App,
            reason: "this device cannot stop, clear or remove apps",
        })
    }
    /// Runs a command in the device's shell and waits for it, killing it after `req.timeout`.
    async fn shell(&self, _req: &ShellRequest) -> Result<ShellReply> {
        Err(Error::Unsupported {
            op: Op::Shell,
            reason: "this device has no shell",
        })
    }
    /// A device with no screen power or lock state reports an awake, unlocked screen.
    async fn screen_state(&self) -> Result<ScreenState> {
        Ok(ScreenState {
            on: true,
            locked: false,
            secure: false,
            focused: true,
        })
    }
    async fn wake(&self) -> Result<()> {
        Ok(())
    }
    /// The platform's own tree JSON with a `density` key, for saving fixtures; `None` when the platform has none.
    /// The screen size now, for a device that cannot announce a rotation as `DisplayChanged` in time for the next
    /// header. `None` means the daemon tracks the size from events.
    fn current_screen(&self) -> Option<Size> {
        None
    }
    /// True when a window of `tree` is secure (`FLAG_SECURE`): its pixels are black while its tree reads.
    fn secure_content_hidden(&self, _tree: &RawTree) -> bool {
        false
    }
    async fn capture_tree(&self) -> Result<Option<String>> {
        Ok(None)
    }
    /// Goes back. Returns how, when the platform has more than one way (`nav-button`, `edge-swipe`).
    async fn back(&self) -> Result<Option<&'static str>> {
        self.button(Button::Back).await.map(|()| None)
    }
    /// Closes the on-screen keyboard, which the caller has seen is shown. Returns how. Android's back button closes
    /// it before it goes back.
    async fn hide_keyboard(&self) -> Result<Option<&'static str>> {
        self.back().await?;
        Ok(Some("back"))
    }
}
