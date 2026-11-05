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
    ApproveSystemPrompt { text: String },
}

impl UserAction {
    pub fn message(&self) -> String {
        match self {
            UserAction::UnlockDevice => {
                "The device is locked. Unlock it by hand; phone-use never enters passcodes.".into()
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
                "The PhoneUseRunner on the device has an expired or missing signature.".into()
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
            UserAction::MountDeveloperImage => "Run `phone-use doctor` to mount the image.",
            UserAction::ResignRunner => {
                "Run `phone-use doctor` to re-sign and reinstall the runner."
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
    #[error("no device is connected")]
    NoDevice { connected: Vec<DeviceSummary> },
    #[error("{} devices are connected; pick one", connected.len())]
    AmbiguousDevice { connected: Vec<DeviceSummary> },
    #[error("device {id} disconnected")]
    DeviceGone { id: DeviceId },
    #[error("device {id} is leased by session {held_by}")]
    DeviceBusy { id: DeviceId, held_by: SessionName },
    #[error("{}", action.message())]
    NeedsUser { action: UserAction },
    #[error("@{} is no longer on screen", r#ref)]
    StaleRef {
        #[serde(rename = "ref")]
        r#ref: Ref,
        closest: Option<ElementLine>,
    },
    #[error("no element matches \"{query}\"")]
    NotFound {
        query: String,
        similar: Vec<ElementLine>,
    },
    #[error("{} elements match \"{query}\"", matches.len())]
    Ambiguous {
        query: String,
        matches: Vec<ElementLine>,
    },
    #[error("@{} is off screen", r#ref)]
    OffScreen {
        #[serde(rename = "ref")]
        r#ref: Ref,
        scroll_hint: Direction,
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
    #[error("another UiAutomation client ({holder}) owns the accessibility bridge")]
    AutomationConflict { holder: String },
    #[error("connection error: {detail}")]
    Transport { detail: String },
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
        }
    }

    /// The next thing to do, starting with a verb.
    pub fn hint(&self) -> String {
        match self {
            Error::NoDevice { .. } => {
                "Connect a device or start an emulator, then run `phone-use devices`.".into()
            }
            Error::AmbiguousDevice { connected } => {
                format!("Pass `--device <id>` with one of: {}.", ids(connected))
            }
            Error::DeviceGone { .. } => {
                "Reconnect the device, then run `phone-use devices`.".into()
            }
            Error::DeviceBusy { held_by, .. } => {
                format!("Wait for session {held_by} to release it, or use another device.")
            }
            Error::NeedsUser { action } => action.hint(),
            Error::StaleRef { closest, .. } => match closest {
                Some(line) => {
                    format!("Run `phone-use snapshot` for fresh refs; closest now: {line}")
                }
                None => "Run `phone-use snapshot` for fresh refs.".into(),
            },
            Error::NotFound { similar, .. } if !similar.is_empty() => {
                format!("Use one of these instead: {}", similar.join("; "))
            }
            Error::NotFound { .. } => "Run `phone-use snapshot` to see what is on screen.".into(),
            Error::Ambiguous { matches, .. } => {
                format!("Use a ref instead: {}", matches.join("; "))
            }
            Error::OffScreen { scroll_hint, .. } => {
                format!("Scroll first: `phone-use scroll {}`.", scroll_hint.name())
            }
            Error::Unsupported { .. } => {
                "Run `phone-use doctor` to see what this device supports.".into()
            }
            Error::NotSettled { .. } => {
                "Run `phone-use snapshot --no-settle` to see the screen as it is.".into()
            }
            Error::Timeout { .. } => {
                "Increase `--timeout`, or run `phone-use snapshot` to check the screen.".into()
            }
            Error::AutomationConflict { holder } => {
                format!("Stop {holder}, then retry the command.")
            }
            Error::Transport { .. } => "Run `phone-use doctor` to check the connection.".into(),
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
            UserAction::ApproveSystemPrompt {
                text: "Use Face ID?".into(),
            },
        ]
    }

    fn all_errors() -> Vec<Error> {
        let mut v = vec![
            Error::NoDevice { connected: vec![] },
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
            },
            Error::StaleRef {
                r#ref: Ref(4),
                closest: Some("@e9 button \"Network & internet\"".into()),
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
        "Turn",
        "Open",
        "Answer",
        "Check",
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
        };
        let hint = e.hint();
        assert!(hint.contains("phone-use snapshot"), "{hint}");
        assert!(hint.contains("@e9 button \"Network & internet\""), "{hint}");
        assert!(e.to_string().contains("@e4"));
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
}
