use std::fmt;

use async_trait::async_trait;
use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, watch};

use crate::error::{DeviceId, Error};
use crate::geometry::{Point, Rect, Size};
use crate::raw::RawTree;

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
    Keystrokes(String),
}

impl fmt::Debug for TextInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (mode, text) = match self {
            TextInput::Paste(t) => ("Paste", t),
            TextInput::PasteSecret(t) => ("PasteSecret", t),
            TextInput::SetValue(t) => ("SetValue", t),
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

/// The seam between the domain and each platform (architecture 4.8).
#[async_trait]
pub trait Device: Send + Sync {
    fn info(&self) -> &DeviceInfo;
    async fn tree(&self) -> Result<RawTree>;
    /// `None` when the device has no frame stream and settle polls screenshots.
    fn frames(&self) -> Option<watch::Receiver<FrameRef>>;
    async fn screenshot(&self, q: Quality) -> Result<Image>;
    fn events(&self) -> broadcast::Receiver<DeviceEvent>;
    async fn gesture(&self, g: Gesture) -> Result<()>;
    async fn key(&self, k: Key) -> Result<()>;
    async fn button(&self, b: Button) -> Result<()>;
    async fn open_url(&self, url: &Url) -> Result<()>;
    async fn text(&self, t: TextInput) -> Result<()>;
    async fn clipboard(&self, op: ClipboardOp) -> Result<Option<String>>;
    async fn apps(&self) -> Result<Vec<App>>;
    async fn launch(&self, app: &AppId) -> Result<()>;
    async fn foreground(&self) -> Result<Option<AppId>>;
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
