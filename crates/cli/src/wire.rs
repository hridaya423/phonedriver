//! Request and reply shapes of the client to daemon protocol (`docs/protocol.md` section 1).

use phonedriver_core::device::Capabilities;
use phonedriver_core::element::{Element, Ref, Role};
use phonedriver_core::error::{Category, Direction};
use phonedriver_core::geometry::Rect;
use phonedriver_core::render::Header;
use serde::{Deserialize, Serialize};

/// What a serde error says about the action, without any value it echoes: a field name or a type, never the text typed.
pub fn malformed_reason(e: &serde_json::Error) -> String {
    without_value(&e.to_string())
}

/// A serde message without the value it quotes (`invalid type: string "…", expected a boolean`): the value may be a
/// secret put in the wrong field. Field names stay, since they are the schema's.
pub fn without_value(message: &str) -> String {
    let m = message.split(" at line ").next().unwrap_or_default();
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
    Ref {
        r#ref: Ref,
    },
    Point {
        point: PointJson,
    },
    /// The one element with this id (an Android resource name, an iOS accessibility identifier). When no element
    /// has it, or several do, `text` and `role` find the element as a text target would. Saved steps use it.
    Id {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        role: Option<Role>,
    },
    Text {
        text: String,
        role: Option<Role>,
    },
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
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Tap {
        target: Target,
        count: Option<u32>,
        long: Option<u32>,
        /// The date (`2026-12-18`) or time (`19:45`) to set in the picker this tap opens (decision D122).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        select: Option<String>,
    },
    Gesture {
        fingers: Vec<Vec<PathPoint>>,
    },
    Launch {
        app: String,
        #[serde(default)]
        grant: Vec<String>,
        #[serde(default)]
        restart: bool,
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
        /// Scroll on and return the list's rows, up to this many (decision D116).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        collect: Option<u32>,
    },
}

pub const MAX_TAP_COUNT: u32 = 10;
pub const MAX_LONG_PRESS_MS: u32 = 10_000;

impl Action {
    /// Why the daemon will not run this action, when its parameters alone say so.
    pub fn refusal(&self) -> Option<String> {
        let Action::Tap {
            count,
            long,
            select,
            ..
        } = self
        else {
            return None;
        };
        if let Some(value) = select {
            if count.is_some() || long.is_some() {
                return Some(
                    "`select` goes with one plain tap: leave out `count` and `long`".into(),
                );
            }
            if let Err(why) = crate::daemon::actor::pick::Want::parse(value) {
                return Some(why);
            }
        }
        if count.is_some_and(|c| c > MAX_TAP_COUNT) {
            return Some(format!(
                "a tap can repeat at most {MAX_TAP_COUNT} times; lower `count`"
            ));
        }
        if long.is_some_and(|l| l > MAX_LONG_PRESS_MS) {
            return Some(format!(
                "a tap can hold for at most {} seconds; lower `long`",
                MAX_LONG_PRESS_MS / 1000
            ));
        }
        None
    }
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
                direction,
                until,
                collect,
                ..
            } => write!(
                f,
                "Scroll({direction:?}, until {until:?}, collect {collect:?})"
            ),
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
    Vpn(VpnState),
    Log(LogCondition),
    Notification(NoticeCondition),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VpnState {
    Up,
    Down,
}

/// A log line written after the wait began that holds `pattern` (`A|B` for either), of `app` when given.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogCondition {
    pub pattern: String,
    #[serde(default)]
    pub app: Option<String>,
}

/// A notification posted since the session's last action (or since the wait began) whose title or text holds
/// `pattern` (`A|B` for either; empty for any), from `app` when given.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoticeCondition {
    pub pattern: String,
    #[serde(default)]
    pub app: Option<String>,
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
    /// The screen gives nothing to act on: no elements (or a web view with none inside) and no text read from
    /// its pixels.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub empty: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionResult {
    pub ok: bool,
    pub summary: String,
    /// Where a tap landed, in points.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub at: Option<phonedriver_core::geometry::Point>,
    /// The field a verified `type` went into and whether it replaced the field's text, for a batch's reply to leave
    /// out what that step already said.
    #[serde(skip)]
    pub typed_into: Option<(phonedriver_core::element::Ref, bool)>,
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
    /// How many times in a row this same action has settled with no change, from the third time on.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub unchanged_repeats: Option<u32>,
    /// The app that was in front crashed during the action.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub crashed: Option<Crashed>,
    /// The screen was off and the action woke it first.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub woke: bool,
    /// A swipe-only lock screen was up and the action swiped it away first.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub unlocked: bool,
    /// The rows a `scroll` with `collect` gathered, in the list's order.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub rows: Vec<String>,
    pub observation: Observation,
}

/// `what` is the exception and the frame that threw it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Crashed {
    pub app: String,
    pub what: String,
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
    /// The lease will not yield to another session by lapsing: an MCP server's, or a command line's within `TERMINAL_HOLD`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lease_live: bool,
    /// The virtual display the live device works on, when it works on one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden_display: Option<u32>,
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
    /// How many rows at the top of the picture the system's status bar covers, when the last screen read showed one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_bar: Option<u32>,
    /// The screen point of the picture's top left pixel, when the picture is the part `within` asked for. A pixel
    /// `px,py` of it is the screen point `origin.x + px / scale`, `origin.y + py / scale`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<PointJson>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ScreenshotParams {
    #[serde(default)]
    pub inline: bool,
    #[serde(default)]
    pub marks: bool,
    /// The picture is this element with [`CROP_MARGIN`] points around it, not the whole screen.
    #[serde(default)]
    pub within: Option<Target>,
}

/// Points of screen kept around the element a screenshot is cut to, so its edge and what it touches can be seen.
pub const CROP_MARGIN: f32 = 8.0;

#[derive(Debug, Default, Deserialize)]
pub struct Common {
    pub device: Option<String>,
    #[serde(default)]
    pub share: bool,
    #[serde(default)]
    pub display: phonedriver_core::device::DisplayMode,
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
    #[serde(default)]
    pub audit: bool,
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
    #[serde(default)]
    pub permissions: Vec<String>,
    pub payload: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordAction {
    Start,
    Stop,
}

/// A recording of the screen to an MP4 file (`docs/protocol.md`, `record`). `path` is absolute, and only for a
/// start; without it the file goes under the daemon's `tmp`.
#[derive(Debug, Deserialize)]
pub struct RecordParams {
    pub action: RecordAction,
    pub path: Option<std::path::PathBuf>,
}

/// An installed app's build (`docs/protocol.md`, `build`), which a replay compares with the one its file was saved on.
#[derive(Debug, Deserialize)]
pub struct BuildParams {
    pub app: String,
}

/// The session's steps (`docs/protocol.md`, `journal`): taken out, read in place with `keep`, or the first `drop` of them
/// removed once the client has stored them.
#[derive(Debug, Default, Deserialize)]
pub struct JournalParams {
    #[serde(default)]
    pub keep: bool,
    pub drop: Option<usize>,
}

/// An app's recent log lines (`docs/protocol.md`, `logs`). `app` defaults to the one in front.
#[derive(Debug, Deserialize)]
pub struct LogsParams {
    pub app: Option<String>,
    pub grep: Option<String>,
    /// The last lines to keep; default 40, 0 keeps all.
    pub head: Option<usize>,
    /// How far back to read, in seconds; default 60.
    pub since_s: Option<u64>,
}

/// The notifications the device shows (`docs/protocol.md`, `notifications`).
#[derive(Debug, Deserialize)]
pub struct NotificationsParams {
    pub app: Option<String>,
    /// The newest ones to list; default 20.
    pub limit: Option<u32>,
}

/// The web page in front, whole (`docs/protocol.md`, `page`).
#[derive(Debug, Deserialize)]
pub struct PageParams {
    pub grep: Option<String>,
    /// Print everything, however long.
    #[serde(default)]
    pub all: bool,
}

/// A system setting by key (`docs/protocol.md`, `setting`): `find` searches (`a|b` for several searches), `key`
/// reads, `key` with `value` writes, `values` writes several, `open` shows the settings screen with that key,
/// `restore` puts back what the writes replaced.
#[derive(Debug, Deserialize)]
pub struct SettingParams {
    pub find: Option<String>,
    pub key: Option<String>,
    pub value: Option<String>,
    pub values: Option<std::collections::BTreeMap<String, String>>,
    pub open: Option<String>,
    /// Write back what these keys were before phonedriver first wrote them on this device; none means every key.
    pub restore: Option<Vec<String>>,
    #[serde(default)]
    pub force: bool,
    #[serde(default = "yes")]
    pub settle: bool,
    pub timeout_ms: Option<u64>,
    pub expect_change_ms: Option<u64>,
}

/// One file to or from the device (`docs/protocol.md`, `files`): `push` names a file in `folder`, `pull` a path
/// under the device's Download, Pictures or DCIM. `folder` is the one folder the person allowed, from the client's
/// `PHONEDRIVER_FILES`.
#[derive(Debug, Deserialize)]
pub struct FilesParams {
    pub folder: Option<std::path::PathBuf>,
    pub push: Option<String>,
    pub pull: Option<String>,
    pub to: Option<phonedriver_core::files::Shelf>,
    #[serde(default)]
    pub approval: Option<Approval>,
    #[serde(default)]
    pub approved: Option<Approved>,
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

/// What the human was shown when they approved a held action: the label and the screen's details. An approval
/// that carries it covers only that commitment; if the button or the details changed before the action ran, the human
/// is asked again (decision D104).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approved {
    pub label: String,
    #[serde(default)]
    pub details: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ActParams {
    pub action: Action,
    #[serde(default)]
    pub approval: Option<Approval>,
    #[serde(default)]
    pub approved: Option<Approved>,
    /// The opt-in categories of tap to ask the human about, besides payments and account deletion (decision D30).
    #[serde(default)]
    pub ask: Vec<Category>,
    #[serde(default = "yes")]
    pub settle: bool,
    pub timeout_ms: Option<u64>,
    /// How long to wait for the first change; the default is 500 ms, or 3 s for `launch` and `open`.
    pub expect_change_ms: Option<u64>,
    /// A secret `type` goes only into a password or one-time code field (decision D105): a replay sets it for the
    /// secrets it reads from the environment.
    #[serde(default)]
    pub secret_field_only: bool,
    /// A replayed step: the screen it was saved on. The action is refused before any touch on another app, or with
    /// a dialog in front where there was none, or none where there was one (decision D110).
    #[serde(default)]
    pub on: Option<OnScreen>,
    /// A replayed step: what the step after it will touch, and the screen that one was saved on. When this target
    /// was not on screen before the action and is there after it, in one place and with nothing drawn over it for
    /// the quiet period, the action's settle ends there instead of waiting for the whole screen (decision D127).
    #[serde(default)]
    pub next: Option<Next>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Next {
    pub target: Target,
    #[serde(default)]
    pub on: Option<OnScreen>,
}

/// The screen a saved step was done on: the app in front, and whether a dialog, sheet or alert was over it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnScreen {
    pub app: Option<String>,
    pub over: bool,
}

fn yes() -> bool {
    true
}

pub const MAX_TIME_LIMIT_S: u64 = 600;

/// Run a command in the device's shell (`docs/protocol.md`, `shell`). The command may be a secret, so `Debug` omits it.
#[derive(Deserialize)]
pub struct ShellParams {
    pub command: String,
    #[serde(default = "yes")]
    pub settle: bool,
    pub timeout_ms: Option<u64>,
    pub expect_change_ms: Option<u64>,
    /// How long the command may run, in seconds; default 30, at most `MAX_TIME_LIMIT_S`.
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

/// What `check` reads: the value the screen shows for `target`. With `equals`, another value is an error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckParams {
    pub target: Target,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<String>,
    /// A name for the value in the reply, for telling two checks of the same label apart.
    #[serde(default, rename = "as", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// One step of a batch: an action, a check (`{"kind": "check", "target": ..., "equals"?: ..., "as"?: ...}`), or a wait (`{"kind": "wait", "condition": ..., "timeout_ms"?: ...}`), whose own
/// timeout stands in for the batch's.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Act(Action),
    Check(CheckParams),
    Wait {
        condition: WaitCondition,
        timeout_ms: Option<u64>,
    },
}

impl Step {
    pub fn parse(v: &serde_json::Value) -> Result<Step, String> {
        if v["kind"] == "wait" {
            let condition = serde_json::from_value(v["condition"].clone())
                .map_err(|_| "the wait condition is malformed".to_string())?;
            let timeout_ms = match &v["timeout_ms"] {
                serde_json::Value::Null => None,
                t => Some(
                    t.as_u64()
                        .ok_or("a wait's `timeout_ms` is a whole number of milliseconds")?,
                ),
            };
            return Ok(Step::Wait {
                condition,
                timeout_ms,
            });
        }
        if v["kind"] == "check" {
            let mut own = v.clone();
            if let Some(fields) = own.as_object_mut() {
                fields.remove("kind");
                if let Some(other) = fields
                    .keys()
                    .find(|k| !["target", "equals", "as"].contains(&k.as_str()))
                {
                    return Err(format!("a check has no `{other}`"));
                }
            }
            return serde_json::from_value(own)
                .map(Step::Check)
                .map_err(|e| format!("the check is malformed: {}", malformed_reason(&e)));
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
    #[serde(default)]
    pub approved: Option<Approved>,
    #[serde(default)]
    pub ask: Vec<Category>,
    pub timeout_ms: Option<u64>,
    pub expect_change_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepResult {
    pub summary: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub rows: Vec<String>,
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

/// Where the `device <id> · <model> · <os> · <size> · <orientation>` line is in a reply, if it has one.
pub fn device_line_at(text: &str) -> Option<usize> {
    text.lines()
        .take(3)
        .position(|l| l.starts_with("device ") && l.matches(" · ").count() >= 3)
}
