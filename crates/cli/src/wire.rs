//! Request and reply shapes of the client to daemon protocol (`docs/protocol.md` section 1).

use phone_use_core::device::Capabilities;
use phone_use_core::element::{Element, Ref, Role};
use phone_use_core::geometry::Rect;
use phone_use_core::render::Header;
use serde::{Deserialize, Serialize};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Target {
    Ref { r#ref: Ref },
    Point { point: PointJson },
    Text { text: String, role: Option<Role> },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PointJson {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    Tap {
        target: Target,
    },
    Type {
        text: String,
        into: Option<Target>,
        mode: Option<TypeMode>,
        #[serde(default)]
        secret: bool,
        #[serde(default)]
        submit: bool,
    },
    Press {
        key: String,
    },
    Swipe {
        direction: Dir,
        from: Option<Target>,
        distance: Option<f32>,
        #[serde(default)]
        fast: bool,
    },
    Launch {
        app: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Tap { target } => write!(f, "Tap({target:?})"),
            Action::Type {
                text, into, mode, ..
            } => write!(
                f,
                "Type({} chars, into {into:?}, mode {mode:?})",
                text.chars().count()
            ),
            Action::Press { key } => write!(f, "Press({key})"),
            Action::Swipe {
                direction, from, ..
            } => write!(f, "Swipe({direction:?}, from {from:?})"),
            Action::Launch { app } => write!(f, "Launch({app})"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeMode {
    Paste,
    Set,
    Keys,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitCondition {
    Settled(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Full,
    Diff,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub header: Header,
    /// The rendered snapshot or diff, exactly what the CLI prints.
    pub text: String,
    pub kind: Kind,
    pub changed: bool,
    pub settled: bool,
    pub settle_ms: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub still_changing: Option<Rect>,
    pub loading: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub elements: Option<Vec<Element>>,
    /// The device's own tree JSON, only when the request asked to capture a fixture.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub capture: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionResult {
    pub ok: bool,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub via: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub verified: Option<bool>,
    pub settled: bool,
    pub settle_ms: u32,
    pub changed: bool,
    pub observation: Observation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceEntry {
    pub id: String,
    pub model: String,
    pub os: String,
    pub tier: String,
    pub capabilities: Capabilities,
    pub leased_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenshotResult {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
}

#[derive(Debug, Default, Deserialize)]
pub struct Common {
    pub device: Option<String>,
    #[serde(default)]
    pub share: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct SnapshotParams {
    #[serde(default)]
    pub diff: bool,
    #[serde(default)]
    pub interactive: bool,
    pub within: Option<Ref>,
    pub max_lines: Option<usize>,
    #[serde(default)]
    pub no_settle: bool,
    #[serde(default)]
    pub elements: bool,
    #[serde(default)]
    pub capture: bool,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ActParams {
    pub action: Action,
    #[serde(default = "yes")]
    pub settle: bool,
    pub timeout_ms: Option<u64>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct WaitParams {
    pub condition: WaitCondition,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct OpenParams {
    pub url: String,
    pub timeout_ms: Option<u64>,
}
