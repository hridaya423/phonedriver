//! Request and reply shapes of the client to daemon protocol (`docs/protocol.md` section 1).

use phone_use_core::device::Capabilities;
use phone_use_core::element::{Element, Ref, Role};
use phone_use_core::error::Direction;
use phone_use_core::geometry::Rect;
use phone_use_core::render::Header;
use serde::{Deserialize, Serialize};

/// What a serde error says about the action, without any value it echoes: a field name or a type, never the text typed.
pub fn malformed_reason(e: &serde_json::Error) -> String {
    let m = e.to_string();
    let m = m.split(" at line ").next().unwrap_or_default();
    if m.starts_with("missing field") || m.starts_with("unknown field") {
        return m.to_string();
    }
    match m.split_once(", expected ") {
        Some((_, want)) => format!("a field has the wrong type or value, expected {want}"),
        None => "a field is missing or has the wrong type".to_string(),
    }
}

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

/// One sample of a finger path: points on the screen and milliseconds since the gesture began.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PathPoint {
    pub x: f32,
    pub y: f32,
    pub t_ms: u32,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    Tap {
        target: Target,
        count: Option<u32>,
        long: Option<u32>,
    },
    Gesture {
        fingers: Vec<Vec<PathPoint>>,
    },
    Launch {
        app: String,
        #[serde(default)]
        grant: Vec<String>,
    },
    Open {
        url: String,
        #[serde(default)]
        app: Option<String>,
    },
    Type {
        text: String,
        into: Option<Target>,
        mode: Option<TypeMode>,
        #[serde(default)]
        secret: bool,
        #[serde(default)]
        submit: bool,
        /// Add to what the field holds instead of replacing it (only with `into`).
        #[serde(default)]
        append: bool,
    },
    Press {
        key: String,
    },
    Clear {
        target: Target,
    },
    Swipe {
        direction: Direction,
        from: Option<Target>,
        distance: Option<f32>,
        #[serde(default)]
        fast: bool,
    },
    Drag {
        from: Target,
        to: Target,
    },
    Pinch {
        direction: PinchDirection,
        at: Option<Target>,
        distance: Option<f32>,
    },
    Scroll {
        direction: Direction,
        #[serde(rename = "in")]
        within: Option<Target>,
        until: Option<Target>,
        max: Option<u32>,
    },
}

impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Tap { target, .. } => write!(f, "Tap({target:?})"),
            Action::Launch { app, .. } => write!(f, "Launch({app})"),
            Action::Open { url, .. } => write!(f, "Open({url})"),
            Action::Gesture { fingers } => write!(f, "Gesture({} fingers)", fingers.len()),
            Action::Type {
                text, into, mode, ..
            } => write!(
                f,
                "Type({} chars, into {into:?}, mode {mode:?})",
                text.chars().count()
            ),
            Action::Press { key } => write!(f, "Press({key})"),
            Action::Clear { target } => write!(f, "Clear({target:?})"),
            Action::Swipe { direction, .. } => write!(f, "Swipe({direction:?})"),
            Action::Drag { from, to } => write!(f, "Drag({from:?} to {to:?})"),
            Action::Pinch { direction, .. } => write!(f, "Pinch({direction:?})"),
            Action::Scroll {
                direction, until, ..
            } => write!(f, "Scroll({direction:?}, until {until:?})"),
        }
    }
}

/// `out` spreads the fingers to zoom in; `in` brings them together to zoom out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PinchDirection {
    In,
    Out,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeMode {
    Paste,
    Set,
    Keys,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitCondition {
    Settled(bool),
    Appears(Target),
    Gone(Target),
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
    /// Set with `verified: false`: what the field shows and why that is.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub unverified: Option<crate::daemon::verify::Unverified>,
    pub settled: bool,
    pub settle_ms: u32,
    pub changed: bool,
    /// The screen was off and the action woke it first.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub woke: bool,
    /// A swipe-only lock screen was up and the action swiped it away first.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub unlocked: bool,
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
    /// Seconds since the leasing session last called.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_idle_s: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenshotResult {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_b64: Option<String>,
    /// How many elements were boxed, when the screenshot asked for marks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marks: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ScreenshotParams {
    #[serde(default)]
    pub inline: bool,
    #[serde(default)]
    pub marks: bool,
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
    #[serde(default)]
    pub pixels: bool,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AppsParams {}

#[derive(Deserialize)]
pub struct ClipboardParams {
    pub set: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AppParams {
    pub action: String,
    pub app: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FindParams {
    pub text: String,
    pub role: Option<Role>,
}

/// What the human decided about a guarded tap (decision D27). `Once` and `Session` come from the MCP server after it
/// asked the human; `Autonomous` is the agent's or the environment's word that the human granted full autonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Approval {
    Once,
    Session,
    Autonomous,
}

#[derive(Debug, Deserialize)]
pub struct ActParams {
    pub action: Action,
    #[serde(default)]
    pub approval: Option<Approval>,
    #[serde(default = "yes")]
    pub settle: bool,
    pub timeout_ms: Option<u64>,
    /// How long to wait for the first change; the default is 500 ms, or 3 s for `launch` and `open`.
    pub expect_change_ms: Option<u64>,
}

fn yes() -> bool {
    true
}

/// Run a command in the device's shell (`docs/protocol.md`, `shell`). The command may be a secret, so `Debug` omits it.
#[derive(Deserialize)]
pub struct ShellParams {
    pub command: String,
    #[serde(default = "yes")]
    pub settle: bool,
    pub timeout_ms: Option<u64>,
    pub expect_change_ms: Option<u64>,
    /// How long the command may run, in seconds; default 30.
    pub time_limit_s: Option<u64>,
    pub grep: Option<String>,
    /// Lines of output to keep; default 40, 0 keeps all.
    pub head: Option<usize>,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub secret: bool,
}

impl std::fmt::Debug for ShellParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ShellParams({} chars)", self.command.chars().count())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShellResult {
    pub exit: i32,
    pub output: String,
    pub omitted: usize,
    pub timed_out: bool,
    pub ms: u32,
    /// The settled screen change; absent when settling was skipped or the command only reads.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub action: Option<ActionResult>,
}

#[derive(Debug, Deserialize)]
pub struct WaitParams {
    pub condition: WaitCondition,
    pub timeout_ms: Option<u64>,
    pub expect_change_ms: Option<u64>,
}

/// One step of a batch: an action, or a wait (`{"kind": "wait", "condition": ...}`).
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Act(Action),
    Wait(WaitCondition),
}

impl Step {
    pub fn parse(v: &serde_json::Value) -> Result<Step, String> {
        if v["kind"] == "wait" {
            let condition = serde_json::from_value(v["condition"].clone())
                .map_err(|_| "the wait condition is malformed".to_string())?;
            return Ok(Step::Wait(condition));
        }
        serde_json::from_value(v.clone())
            .map(Step::Act)
            .map_err(|e| {
                if e.to_string().starts_with("unknown variant") {
                    format!(
                        "the action `{}` is unknown to this daemon, which is older than the client",
                        v["kind"]
                            .as_str()
                            .unwrap_or_default()
                            .chars()
                            .take(32)
                            .collect::<String>()
                    )
                } else {
                    format!("the action is malformed: {}", malformed_reason(&e))
                }
            })
    }
}

#[derive(Debug, Deserialize)]
pub struct BatchParams {
    pub steps: Vec<serde_json::Value>,
    /// Only `Autonomous` counts in a batch: a human approves one tap at a time.
    #[serde(default)]
    pub approval: Option<Approval>,
    pub timeout_ms: Option<u64>,
    pub expect_change_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepResult {
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub unverified: Option<crate::daemon::verify::Unverified>,
}

/// The step that failed: its 1-based position and the error as `Error::to_rpc` builds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepFailure {
    pub step: usize,
    pub error: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchResult {
    pub ok: bool,
    /// The steps that ran and succeeded, in order.
    pub steps: Vec<StepResult>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub failed: Option<StepFailure>,
    pub settled: bool,
    pub settle_ms: u32,
    pub changed: bool,
    /// What changed from where the agent last looked to the end of the last step that ran.
    pub observation: Observation,
}
