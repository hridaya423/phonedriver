use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::element::Ref;
use crate::geometry::Rect;

pub type DeviceId = String;
pub type SessionName = String;
/// A rendered element line, such as `@e9 button "Network & internet"`.
pub type ElementLine = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceSummary {
    pub id: DeviceId,
    pub model: String,
    pub os: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub fn name(self) -> &'static str {
        match self {
            Direction::Up => "up",
            Direction::Down => "down",
            Direction::Left => "left",
            Direction::Right => "right",
        }
    }

    pub fn opposite(self) -> Direction {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }
}

/// What a guarded tap does (decision D30). `Payment` and `Account` are always asked about; the rest only when the
/// human turned them on with `PHONEDRIVER_ASK`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Payment,
    Account,
    Send,
    Delete,
    Install,
}

impl Category {
    pub const OPT_IN: [Category; 3] = [Category::Send, Category::Delete, Category::Install];

    pub fn name(self) -> &'static str {
        match self {
            Category::Payment => "payment",
            Category::Account => "account",
            Category::Send => "send",
            Category::Delete => "delete",
            Category::Install => "install",
        }
    }

    pub fn what(self) -> &'static str {
        match self {
            Category::Payment => "spends money or cannot be undone",
            Category::Account => "deletes an account",
            Category::Send => "sends a message or post",
            Category::Delete => "deletes something",
            Category::Install => "installs or removes an app",
        }
    }

    pub fn always(self) -> bool {
        !Category::OPT_IN.contains(&self)
    }

    pub fn from_opt_in(word: &str) -> Option<Category> {
        Category::OPT_IN
            .into_iter()
            .find(|c| c.name().eq_ignore_ascii_case(word))
    }
}

/// Why a field is filled only with secret text (decision D30).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretKind {
    Password,
    Code,
}

impl SecretKind {
    pub fn what(self) -> &'static str {
        match self {
            SecretKind::Password => "a password field",
            SecretKind::Code => "a one-time code field",
        }
    }
}

/// Why a search by scrolling stopped without finding its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollEnd {
    /// The list moved, then stopped moving: its end.
    Ended,
    /// The list never moved.
    Stuck,
    /// The scroll limit came first.
    Limit,
}

/// `45s`, or minutes rounded to the nearest from one minute on.
pub fn span(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m", (secs + 30) / 60)
    }
}

fn stale_message(r: Ref, unseen: bool) -> String {
    if unseen {
        format!("@{r} belongs to an earlier screen or another session")
    } else {
        format!("@{r} is no longer on screen")
    }
}

fn covered(behind_keyboard: bool, by: Option<&str>) -> String {
    match (behind_keyboard, by) {
        (true, _) => "under the keyboard".into(),
        (false, Some(name)) => format!("under the {name}"),
        (false, None) => "off screen".into(),
    }
}

fn times(n: u32) -> String {
    if n == 1 {
        "1 time".into()
    } else {
        format!("{n} times")
    }
}

fn not_reached(query: &str, scrolls: u32, end: ScrollEnd, last: Option<&str>) -> String {
    match end {
        ScrollEnd::Ended => {
            let at = last.map_or_else(String::new, |l| format!(" at \"{l}\""));
            format!(
                "scrolled {} and the list ended{at}; \"{query}\" is not in it",
                times(scrolls)
            )
        }
        ScrollEnd::Stuck => {
            format!("the list did not move after {scrolls} scrolls; \"{query}\" not found")
        }
        ScrollEnd::Limit => format!(
            "scrolled {} and \"{query}\" is still not on screen",
            times(scrolls)
        ),
    }
}

/// The operations a session can ask a device for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Tree,
    Screenshot,
    Gesture,
    Key,
    Button,
    OpenUrl,
    Text,
    Clipboard,
    Apps,
    Launch,
    Foreground,
    Ocr,
    App,
    Shell,
    Display,
}

impl Op {
    pub fn name(self) -> &'static str {
        match self {
            Op::Tree => "tree",
            Op::Screenshot => "screenshot",
            Op::Gesture => "gesture",
            Op::Key => "key",
            Op::Button => "button",
            Op::OpenUrl => "open_url",
            Op::Text => "text",
            Op::Clipboard => "clipboard",
            Op::Apps => "apps",
            Op::Launch => "launch",
            Op::Foreground => "foreground",
            Op::Ocr => "ocr",
            Op::App => "app",
            Op::Shell => "shell",
            Op::Display => "display",
        }
    }
}

/// Something only a person can do (architecture 4.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UserAction {
    UnlockDevice,
    AcceptUsbDebugging,
    EnableUsbDebugging,
    AllowXiaomiInjection,
    TrustComputer,
    EnableDeveloperMode,
    MountDeveloperImage,
    ResignRunner,
    SetSigningTeam,
    ApproveSystemPrompt { text: String },
}

impl UserAction {
    pub fn message(&self) -> String {
        match self {
            UserAction::UnlockDevice => {
                "The device is locked. Unlock it by hand; phonedriver never enters passcodes.".into()
            }
            UserAction::AcceptUsbDebugging => {
                "The device is asking whether to allow USB debugging from this computer.".into()
            }
            UserAction::EnableUsbDebugging => "USB debugging is off on the device.".into(),
            UserAction::AllowXiaomiInjection => {
                "This Xiaomi device blocks input from USB debugging until a second setting is on."
                    .into()
            }
            UserAction::TrustComputer => "The device does not trust this computer yet.".into(),
            UserAction::EnableDeveloperMode => "Developer Mode is off on the device.".into(),
            UserAction::MountDeveloperImage => {
                "The Developer Disk Image is not mounted on the device.".into()
            }
            UserAction::ResignRunner => {
                "The PhonedriverRunner on the device has an expired or missing signature.".into()
            }
            UserAction::SetSigningTeam => {
                "Running on an iPhone or iPad needs an Apple developer team to sign the runner, and PHONEDRIVER_TEAM is not set."
                    .into()
            }
            UserAction::ApproveSystemPrompt { text } => {
                format!("The device shows a prompt only a person may answer: \"{text}\".")
            }
        }
    }

    pub fn hint(&self) -> String {
        match self {
            UserAction::UnlockDevice => "Unlock the device, then retry the command.",
            UserAction::AcceptUsbDebugging => "Tap Allow on the device's USB debugging prompt.",
            UserAction::EnableUsbDebugging => {
                "Turn on Settings > Developer options > USB debugging on the device."
            }
            UserAction::AllowXiaomiInjection => {
                "Turn on Settings > Developer options > USB debugging (Security settings)."
            }
            UserAction::TrustComputer => "Unlock the device and tap Trust on its prompt.",
            UserAction::EnableDeveloperMode => {
                "Turn on Settings > Privacy & Security > Developer Mode, then restart the device."
            }
            UserAction::MountDeveloperImage => "Run `phonedriver ios setup` to mount the image.",
            UserAction::ResignRunner => {
                "Run `phonedriver doctor` to re-sign and reinstall the runner."
            }
            UserAction::SetSigningTeam => {
                "Set PHONEDRIVER_TEAM to your team ID: ten letters and digits, shown in Xcode > Settings > Accounts, or in parentheses after your name in `security find-identity -v -p codesigning`. Export it in the shell that starts the daemon and run `phonedriver daemon stop` if one is already running. `phonedriver ios setup` checks pairing and Developer Mode first."
            }
            UserAction::ApproveSystemPrompt { .. } => {
                "Answer the prompt on the device, then retry."
            }
        }
        .into()
    }
}

/// Every failure an agent can see, each written to be acted on.
#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize)]
#[serde(tag = "code_name", rename_all = "snake_case")]
pub enum Error {
    #[error("{}", no_device(requested.as_deref(), connected))]
    NoDevice {
        /// The id passed with `--device` or `PHONEDRIVER_DEVICE` that matches no connected device.
        requested: Option<String>,
        connected: Vec<DeviceSummary>,
    },
    #[error("{} devices are connected; pick one", connected.len())]
    AmbiguousDevice { connected: Vec<DeviceSummary> },
    #[error("device {id} disconnected")]
    DeviceGone { id: DeviceId },
    #[error("device {id} is leased by session {held_by}")]
    DeviceBusy {
        id: DeviceId,
        held_by: SessionName,
        /// Seconds since the holder last called.
        idle_s: u64,
        /// Seconds until the lease lapses without another call.
        expires_s: u64,
    },
    #[error("device {id} is driven by another phonedriver daemon (pid {pid} on {host})")]
    DeviceDriven {
        id: DeviceId,
        pid: u32,
        host: String,
    },
    #[error("{}", action.message())]
    NeedsUser { action: UserAction },
    #[error("{}", stale_message(*r#ref, *unseen))]
    StaleRef {
        #[serde(rename = "ref")]
        r#ref: Ref,
        closest: Option<ElementLine>,
        /// This session has not been shown the ref: it comes from another session or an older daemon.
        unseen: bool,
    },
    #[error("no element matches \"{query}\"")]
    NotFound {
        query: String,
        similar: Vec<ElementLine>,
    },
    #[error("unknown key \"{key}\"")]
    UnknownKey { key: String, valid: Vec<String> },
    #[error("{}", needs_approval(label, app.as_deref(), *category, details))]
    NeedsApproval {
        label: String,
        category: Category,
        app: Option<String>,
        /// Visible text near the button that says what is being bought or done.
        details: Vec<String>,
    },
    #[error(
        "@{} is {}: plain text is not typed into it. Ask the user for the text, then use `--secret` (MCP: the `secret` input) so it is masked and never logged",
        r#ref,
        kind.what()
    )]
    SecretField {
        #[serde(rename = "ref")]
        r#ref: Ref,
        kind: SecretKind,
    },
    #[error("no app named \"{query}\"")]
    NoApp {
        query: String,
        /// Installed apps whose name shares a word with the query or is close to it, as `Name (id)`.
        similar: Vec<String>,
    },
    #[error("{} elements match \"{query}\"", matches.len())]
    Ambiguous {
        query: String,
        matches: Vec<ElementLine>,
    },
    #[error("@{} is {}", r#ref, covered(*behind_keyboard, covered_by.as_deref()))]
    OffScreen {
        #[serde(rename = "ref")]
        r#ref: Ref,
        scroll_hint: Direction,
        /// The element is on screen but the keyboard covers it.
        behind_keyboard: bool,
        /// The element is on screen but this one (a floating `Toolbar`) covers its center.
        covered_by: Option<String>,
    },
    #[error("{} is not supported on this device: {reason}", op.name())]
    Unsupported { op: Op, reason: &'static str },
    #[error("the screen was still changing after {waited_ms} ms")]
    NotSettled {
        waited_ms: u32,
        still_changing: Rect,
    },
    #[error("timed out after {waited_ms} ms waiting for {condition}")]
    Timeout { waited_ms: u32, condition: String },
    #[error("{}", not_reached(query, *scrolls, *end, last.as_deref()))]
    NotReached {
        query: String,
        scrolls: u32,
        end: ScrollEnd,
        /// The label of the last row in the scroll direction, when the list ended.
        last: Option<String>,
    },
    #[error("another UiAutomation client ({holder}) owns the accessibility bridge")]
    AutomationConflict { holder: String },
    #[error("connection error: {detail}")]
    Transport { detail: String },
}

fn no_device(requested: Option<&str>, connected: &[DeviceSummary]) -> String {
    let Some(requested) = requested else {
        return "no device is connected".into();
    };
    if connected.is_empty() {
        return format!("no device \"{requested}\"; none is connected");
    }
    let list = connected
        .iter()
        .map(|d| format!("{} ({})", d.id, d.model))
        .collect::<Vec<_>>()
        .join(", ");
    format!("no device \"{requested}\"; connected: {list}")
}

/// The refusal's message: what the tap is, and the screen text that says what it commits.
pub fn needs_approval(
    label: &str,
    app: Option<&str>,
    category: Category,
    details: &[String],
) -> String {
    let what = category.what();
    let mut text = match app {
        Some(app) => format!("\"{label}\" in {app} {what}: it needs the human's approval"),
        None => format!("\"{label}\" {what}: it needs the human's approval"),
    };
    if !details.is_empty() {
        text.push_str("\nOn screen:");
        for d in details {
            text.push_str("\n  ");
            text.push_str(d);
        }
    }
    text
}

fn ids(devices: &[DeviceSummary]) -> String {
    devices
        .iter()
        .map(|d| d.id.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

impl Error {
    /// JSON-RPC error code (`docs/protocol.md` section 1).
    pub fn rpc_code(&self) -> i32 {
        match self {
            Error::NoDevice { .. } => -32001,
            Error::AmbiguousDevice { .. } => -32002,
            Error::DeviceGone { .. } => -32003,
            Error::DeviceBusy { .. } => -32004,
            Error::DeviceDriven { .. } => -32021,
            Error::NeedsUser { .. } => -32005,
            Error::StaleRef { .. } => -32006,
            Error::NotFound { .. } => -32007,
            Error::Ambiguous { .. } => -32008,
            Error::OffScreen { .. } => -32009,
            Error::Unsupported { .. } => -32011,
            Error::NotSettled { .. } => -32012,
            Error::Timeout { .. } => -32013,
            Error::AutomationConflict { .. } => -32014,
            Error::Transport { .. } => -32015,
            Error::NotReached { .. } => -32016,
            Error::NoApp { .. } => -32017,
            Error::UnknownKey { .. } => -32018,
            Error::NeedsApproval { .. } => -32019,
            Error::SecretField { .. } => -32020,
        }
    }

    /// The next thing to do, starting with a verb.
    pub fn hint(&self) -> String {
        match self {
            Error::NoDevice {
                requested: Some(_),
                connected,
            } if !connected.is_empty() => format!(
                "Pass `--device <id>` (or set PHONEDRIVER_DEVICE) to one of: {}.",
                ids(connected)
            ),
            Error::NoDevice { .. } => {
                "Connect a device or start an emulator; `phonedriver doctor` says what is missing (adb, USB debugging, a dropped tunnel).".into()
            }
            Error::AmbiguousDevice { connected } => {
                format!("Pass `--device <id>` with one of: {}.", ids(connected))
            }
            Error::DeviceGone { .. } => {
                "Reconnect the device, then run `phonedriver devices`.".into()
            }
            Error::DeviceBusy {
                held_by,
                idle_s,
                expires_s,
                ..
            } => format!(
                "Wait for session {held_by} to release it (idle {}, lease expires in {}), or use another device.",
                span(*idle_s),
                span(*expires_s)
            ),
            Error::DeviceDriven { pid, host, .. } => format!(
                "Stop that daemon (`kill {pid}` on {host}) or use another device, then retry."
            ),
            Error::NeedsUser { action } => action.hint(),
            Error::StaleRef { unseen: true, .. } => {
                "Run `phonedriver snapshot` to read the screen, then use its refs; refs are per session.".into()
            }
            Error::StaleRef { closest, .. } => match closest {
                Some(line) => {
                    format!("Run `phonedriver snapshot` for fresh refs; closest now: {line}")
                }
                None => "Run `phonedriver snapshot` for fresh refs.".into(),
            },
            Error::NotFound { similar, .. } if !similar.is_empty() => {
                format!("Use one of these instead: {}", similar.join("; "))
            }
            Error::NotFound { .. } => "Run `phonedriver snapshot` to see what is on screen.".into(),
            Error::UnknownKey { valid, .. } => {
                format!("Use one of these keys: {}.", valid.join("; "))
            }
            Error::SecretField { .. } => "Ask the user for the text unless they already gave it for this task, then type it on its own with `--secret` (the text on stdin or in PHONEDRIVER_SECRET); a batch cannot carry secret text.".into(),
            Error::NeedsApproval { .. } => "Ask the user in chat whether to tap it; tap again with `--autonomous` only when they say yes or have granted full autonomy.".into(),
            Error::NoApp { similar, .. } if !similar.is_empty() => {
                format!("Use one of these installed apps: {}.", similar.join("; "))
            }
            Error::NoApp { .. } => "Use `phonedriver apps` to list the installed apps.".into(),
            Error::Ambiguous { matches, .. } if matches.iter().all(|m| m.starts_with('@')) => {
                format!("Use a ref instead: {}", matches.join("; "))
            }
            Error::Ambiguous { matches, .. } => {
                format!(
                    "Use text that only one of these has: {}.",
                    matches.join("; ")
                )
            }
            Error::OffScreen {
                behind_keyboard: true,
                scroll_hint,
                ..
            } => format!(
                "Hide the keyboard with `phonedriver press hide-keyboard`, or `phonedriver scroll {}` to bring the element above it.",
                scroll_hint.name()
            ),
            Error::OffScreen {
                scroll_hint,
                covered_by: Some(name),
                ..
            } => format!(
                "Scroll first: `phonedriver scroll {}` moves the element out from under the {name}.",
                scroll_hint.name()
            ),
            Error::OffScreen { scroll_hint, .. } => {
                format!("Scroll first: `phonedriver scroll {}`.", scroll_hint.name())
            }
            Error::Unsupported { op: Op::Text, .. } => {
                "Use `--mode paste` or `--mode keys`; `set` works only where the device can set field values.".into()
            }
            Error::Unsupported { op: Op::Key, .. } => {
                "Use `phonedriver press enter` or tap the field's Done button to end editing.".into()
            }
            Error::Unsupported { op: Op::Display, .. } => {
                "Leave out `--display hidden` (and unset `PHONEDRIVER_DISPLAY`) to work on the phone's own screen.".into()
            }
            Error::Unsupported {
                op: Op::OpenUrl, ..
            } => "Use a link an installed app opens, such as `https:`, or `phonedriver launch` an app by name.".into(),
            Error::Unsupported { op: Op::Ocr, .. } => {
                "Use `phonedriver screenshot` to look at the screen instead.".into()
            }
            Error::Unsupported { .. } => {
                "Run `phonedriver doctor` to see what this device supports.".into()
            }
            Error::NotSettled { .. } => {
                "Run `phonedriver snapshot --no-settle` to see the screen as it is.".into()
            }
            Error::Timeout { .. } => {
                "Increase `--timeout`, or run `phonedriver snapshot` to check the screen.".into()
            }
            Error::NotReached {
                end: ScrollEnd::Ended,
                ..
            } => "Scroll the other way if it may be earlier in the list, or run `phonedriver snapshot` to see where the list ended.".into(),
            Error::NotReached {
                end: ScrollEnd::Stuck,
                ..
            } => "Use `--within <list ref>` to name the list that should scroll, or run `phonedriver snapshot`.".into(),
            Error::NotReached {
                end: ScrollEnd::Limit,
                ..
            } => "Increase `--max` to keep scrolling, or run `phonedriver snapshot` to see where the list is.".into(),
            Error::AutomationConflict { holder } => {
                format!("Stop {holder}, then retry the command.")
            }
            Error::Transport { .. } => "Run `phonedriver doctor` to check the connection.".into(),
        }
    }

    /// The JSON-RPC `error` object: `{code, message, data: {code_name, hint, ...fields}}`.
    pub fn to_rpc(&self) -> Value {
        let mut data = serde_json::to_value(self).unwrap_or_else(|_| json!({}));
        if let Value::Object(map) = &mut data {
            map.insert("hint".into(), Value::String(self.hint()));
        }
        json!({ "code": self.rpc_code(), "message": self.to_string(), "data": data })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::element::Ref;
    use crate::geometry::Rect;

    fn summary() -> DeviceSummary {
        DeviceSummary {
            id: "android:emulator-5554".into(),
            model: "Pixel 9".into(),
            os: "Android 17".into(),
        }
    }

    fn all_user_actions() -> Vec<UserAction> {
        vec![
            UserAction::UnlockDevice,
            UserAction::AcceptUsbDebugging,
            UserAction::EnableUsbDebugging,
            UserAction::AllowXiaomiInjection,
            UserAction::TrustComputer,
            UserAction::EnableDeveloperMode,
            UserAction::MountDeveloperImage,
            UserAction::ResignRunner,
            UserAction::SetSigningTeam,
            UserAction::ApproveSystemPrompt {
                text: "Use Face ID?".into(),
            },
        ]
    }

    fn all_errors() -> Vec<Error> {
        let mut v = vec![
            Error::NoDevice {
                requested: None,
                connected: vec![],
            },
            Error::NoDevice {
                requested: Some("sim:BAD".into()),
                connected: vec![summary()],
            },
            Error::AmbiguousDevice {
                connected: vec![
                    summary(),
                    DeviceSummary {
                        id: "ios:00008110".into(),
                        model: "iPhone 14 Plus".into(),
                        os: "iOS 26.5".into(),
                    },
                ],
            },
            Error::DeviceGone {
                id: "android:emulator-5554".into(),
            },
            Error::DeviceBusy {
                id: "android:emulator-5554".into(),
                held_by: "agent-2".into(),
                idle_s: 180,
                expires_s: 420,
            },
            Error::DeviceDriven {
                id: "android:emulator-5554".into(),
                pid: 4242,
                host: "mac.local".into(),
            },
            Error::StaleRef {
                r#ref: Ref(4),
                closest: Some("@e9 button \"Network & internet\"".into()),
                unseen: false,
            },
            Error::NotFound {
                query: "Wifi".into(),
                similar: vec!["@e4 button \"Wi‑Fi\"".into()],
            },
            Error::Ambiguous {
                query: "Dark theme".into(),
                matches: vec![
                    "@e7 button \"Dark theme\"".into(),
                    "@e8 switch \"Dark theme\" on".into(),
                ],
            },
            Error::OffScreen {
                r#ref: Ref(30),
                scroll_hint: Direction::Down,
                behind_keyboard: false,
                covered_by: None,
            },
            Error::OffScreen {
                r#ref: Ref(32),
                scroll_hint: Direction::Down,
                behind_keyboard: false,
                covered_by: Some("Toolbar".into()),
            },
            Error::OffScreen {
                r#ref: Ref(31),
                scroll_hint: Direction::Down,
                behind_keyboard: true,
                covered_by: None,
            },
            Error::Unsupported {
                op: Op::Clipboard,
                reason: "the Simulator tier has no pasteboard service",
            },
            Error::NotSettled {
                waited_ms: 5000,
                still_changing: Rect {
                    x: 0.0,
                    y: 100.0,
                    w: 360.0,
                    h: 200.0,
                },
            },
            Error::Timeout {
                waited_ms: 10_000,
                condition: "appears \"Done\"".into(),
            },
            Error::NoApp {
                query: "phonedriver testbed".into(),
                similar: vec!["Testbed (dev.phonedriver.testbed)".into()],
            },
            Error::NoApp {
                query: "Zork".into(),
                similar: vec![],
            },
            Error::UnknownKey {
                key: "bogus".into(),
                valid: vec!["home".into(), "back".into()],
            },
            Error::NeedsApproval {
                label: "Place order · $12.00".into(),
                category: Category::Payment,
                app: Some("Testbed".into()),
                details: vec!["Total $12.00".into()],
            },
            Error::SecretField {
                r#ref: Ref(3),
                kind: SecretKind::Password,
            },
            Error::SecretField {
                r#ref: Ref(4),
                kind: SecretKind::Code,
            },
            Error::NotReached {
                query: "Safari".into(),
                scrolls: 3,
                end: ScrollEnd::Ended,
                last: Some("Developer".into()),
            },
            Error::NotReached {
                query: "Row 640".into(),
                scrolls: 2,
                end: ScrollEnd::Stuck,
                last: None,
            },
            Error::NotReached {
                query: "Row 640".into(),
                scrolls: 30,
                end: ScrollEnd::Limit,
                last: None,
            },
            Error::AutomationConflict {
                holder: "io.appium.uiautomator2.server".into(),
            },
            Error::Transport {
                detail: "adb server closed the stream".into(),
            },
        ];
        v.extend(
            all_user_actions()
                .into_iter()
                .map(|action| Error::NeedsUser { action }),
        );
        v
    }

    fn code_of(e: &Error) -> (i32, &'static str) {
        match e {
            Error::NoDevice { .. } => (-32001, "no_device"),
            Error::AmbiguousDevice { .. } => (-32002, "ambiguous_device"),
            Error::DeviceGone { .. } => (-32003, "device_gone"),
            Error::DeviceBusy { .. } => (-32004, "device_busy"),
            Error::DeviceDriven { .. } => (-32021, "device_driven"),
            Error::NeedsUser { .. } => (-32005, "needs_user"),
            Error::StaleRef { .. } => (-32006, "stale_ref"),
            Error::NotFound { .. } => (-32007, "not_found"),
            Error::Ambiguous { .. } => (-32008, "ambiguous"),
            Error::OffScreen { .. } => (-32009, "off_screen"),
            Error::Unsupported { .. } => (-32011, "unsupported"),
            Error::NotSettled { .. } => (-32012, "not_settled"),
            Error::Timeout { .. } => (-32013, "timeout"),
            Error::AutomationConflict { .. } => (-32014, "automation_conflict"),
            Error::Transport { .. } => (-32015, "transport"),
            Error::NotReached { .. } => (-32016, "not_reached"),
            Error::NoApp { .. } => (-32017, "no_app"),
            Error::UnknownKey { .. } => (-32018, "unknown_key"),
            Error::NeedsApproval { .. } => (-32019, "needs_approval"),
            Error::SecretField { .. } => (-32020, "secret_field"),
        }
    }

    const VERBS: &[&str] = &[
        "Run",
        "Connect",
        "Pass",
        "Reconnect",
        "Wait",
        "Use",
        "Scroll",
        "Increase",
        "Stop",
        "Unlock",
        "Tap",
        "Ask",
        "Turn",
        "Open",
        "Answer",
        "Check",
        "Hide",
        "Set",
    ];

    #[test]
    fn every_variant_serializes_with_code_name_message_and_hint() {
        for e in all_errors() {
            let (code, name) = code_of(&e);
            let v = e.to_rpc();
            assert_eq!(v["code"], json!(code), "{e:?}");
            assert_eq!(e.rpc_code(), code);
            assert_eq!(v["data"]["code_name"], json!(name), "{e:?}");
            let message = v["message"].as_str().unwrap();
            let hint = v["data"]["hint"].as_str().unwrap();
            assert!(!message.is_empty(), "{e:?}");
            let first = hint.split_whitespace().next().unwrap_or("");
            assert!(
                VERBS.contains(&first),
                "hint of {name} starts with {first:?}: {hint}"
            );
        }
    }

    #[test]
    fn variant_fields_travel_in_data() {
        let v = Error::StaleRef {
            r#ref: Ref(4),
            closest: None,
            unseen: false,
        }
        .to_rpc();
        assert_eq!(v["data"]["ref"], json!("e4"));
        let v = Error::NeedsUser {
            action: UserAction::ApproveSystemPrompt {
                text: "Use Face ID?".into(),
            },
        }
        .to_rpc();
        assert_eq!(v["data"]["action"]["kind"], json!("approve_system_prompt"));
        assert_eq!(v["data"]["action"]["text"], json!("Use Face ID?"));
        assert!(v["message"].as_str().unwrap().contains("Use Face ID?"));
    }

    #[test]
    fn stale_ref_hint_names_snapshot_and_closest() {
        let e = Error::StaleRef {
            r#ref: Ref(4),
            closest: Some("@e9 button \"Network & internet\"".into()),
            unseen: false,
        };
        let hint = e.hint();
        assert!(hint.contains("phonedriver snapshot"), "{hint}");
        assert!(hint.contains("@e9 button \"Network & internet\""), "{hint}");
        assert!(e.to_string().contains("@e4"));
    }

    #[test]
    fn a_ref_this_session_never_saw_says_so_and_how_to_get_refs() {
        let e = Error::StaleRef {
            r#ref: Ref(5),
            closest: None,
            unseen: true,
        };
        let message = e.to_string();
        assert!(
            message.contains("@e5") && message.contains("earlier screen or another session"),
            "{message}"
        );
        assert!(!message.contains("no longer on screen"), "{message}");
        let hint = e.hint();
        assert!(hint.contains("phonedriver snapshot"), "{hint}");
        assert!(hint.contains("per session"), "{hint}");
    }

    #[test]
    fn unlock_message_says_we_never_enter_passcodes() {
        let m = Error::NeedsUser {
            action: UserAction::UnlockDevice,
        }
        .to_string();
        assert!(m.contains("Unlock"), "{m}");
        assert!(m.contains("never enters passcodes"), "{m}");
    }

    #[test]
    fn user_actions_each_have_message_and_verb_hint() {
        for a in all_user_actions() {
            assert!(!a.message().is_empty());
            let first = a.hint().split_whitespace().next().unwrap_or("").to_string();
            assert!(VERBS.contains(&first.as_str()), "{a:?}: {}", a.hint());
        }
    }

    #[test]
    fn rpc_value_has_exactly_the_protocol_keys() {
        let v = Error::Transport { detail: "x".into() }.to_rpc();
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["code", "data", "message"]);
        assert!(matches!(v["data"], Value::Object(_)));
    }

    #[test]
    fn a_device_id_that_matches_nothing_is_named_and_the_connected_ones_listed() {
        let e = Error::NoDevice {
            requested: Some("sim:BAD".into()),
            connected: vec![summary()],
        };
        assert_eq!(
            e.to_string(),
            "no device \"sim:BAD\"; connected: android:emulator-5554 (Pixel 9)"
        );
        assert_eq!(
            e.hint(),
            "Pass `--device <id>` (or set PHONEDRIVER_DEVICE) to one of: android:emulator-5554."
        );
        let none = Error::NoDevice {
            requested: Some("sim:BAD".into()),
            connected: vec![],
        };
        assert_eq!(none.to_string(), "no device \"sim:BAD\"; none is connected");
        assert!(none.hint().starts_with("Connect a device"));
        assert!(
            none.hint().contains("phonedriver doctor"),
            "{}",
            none.hint()
        );
        assert_eq!(
            Error::NoDevice {
                requested: None,
                connected: vec![]
            }
            .to_string(),
            "no device is connected"
        );
    }

    #[test]
    fn the_missing_signing_team_says_where_to_find_it_and_shows_no_example_id() {
        let e = Error::NeedsUser {
            action: UserAction::SetSigningTeam,
        };
        let hint = e.hint();
        for needle in [
            "PHONEDRIVER_TEAM",
            "Xcode > Settings > Accounts",
            "security find-identity -v -p codesigning",
            "phonedriver ios setup",
            "phonedriver daemon stop",
        ] {
            assert!(hint.contains(needle), "{needle} not in {hint}");
        }
        assert!(!hint.contains("doctor"), "{hint}");
        let message = e.to_string();
        assert!(!message.contains("connection"), "{message}");
        assert!(
            !hint.chars().any(|c| c.is_ascii_digit())
                && !message.chars().any(|c| c.is_ascii_digit()),
            "no example id: {hint} / {message}"
        );
        assert_eq!(e.rpc_code(), -32005);
    }

    #[test]
    fn an_unknown_key_is_called_a_key_and_lists_the_valid_ones() {
        let e = Error::UnknownKey {
            key: "bogus".into(),
            valid: vec!["home".into(), "back".into()],
        };
        assert_eq!(e.to_string(), "unknown key \"bogus\"");
        assert_eq!(e.hint(), "Use one of these keys: home; back.");
    }
}
