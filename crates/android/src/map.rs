//! Translation between the `Device` port's types and the agent's wire format.

use phone_use_core::device::{Button, DeviceEvent, Gesture, Key, TextInput};
use phone_use_core::error::{Error, Op};
use phone_use_core::geometry::{Scale, Size};
use phone_use_core::raw::RawTree;
use serde_json::{Value, json};

const KEYCODE_HOME: i32 = 3;
const KEYCODE_BACK: i32 = 4;
const KEYCODE_DPAD_UP: i32 = 19;
const KEYCODE_DPAD_DOWN: i32 = 20;
const KEYCODE_DPAD_LEFT: i32 = 21;
const KEYCODE_DPAD_RIGHT: i32 = 22;
const KEYCODE_VOLUME_UP: i32 = 24;
const KEYCODE_VOLUME_DOWN: i32 = 25;
const KEYCODE_TAB: i32 = 61;
const KEYCODE_ENTER: i32 = 66;
const KEYCODE_DEL: i32 = 67;
const KEYCODE_ESCAPE: i32 = 111;
const KEYCODE_APP_SWITCH: i32 = 187;
const KEYCODE_SLEEP: i32 = 223;

pub fn raw_tree(reply: &Value, density: f32) -> Result<RawTree, Error> {
    RawTree::from_android_json(&reply.to_string(), Scale { density }).map_err(|e| {
        Error::Transport {
            detail: format!("the agent sent a tree that does not parse: {e}"),
        }
    })
}

pub fn keycode(k: Key) -> i32 {
    match k {
        Key::Enter => KEYCODE_ENTER,
        Key::Backspace => KEYCODE_DEL,
        Key::Tab => KEYCODE_TAB,
        Key::Escape => KEYCODE_ESCAPE,
        Key::ArrowUp => KEYCODE_DPAD_UP,
        Key::ArrowDown => KEYCODE_DPAD_DOWN,
        Key::ArrowLeft => KEYCODE_DPAD_LEFT,
        Key::ArrowRight => KEYCODE_DPAD_RIGHT,
    }
}

/// The key a button presses; `Notifications` has no key, it pulls the shade down.
pub fn button_keycode(b: Button) -> Option<i32> {
    match b {
        Button::Home => Some(KEYCODE_HOME),
        Button::Back => Some(KEYCODE_BACK),
        Button::Recents => Some(KEYCODE_APP_SWITCH),
        Button::Notifications => None,
        Button::Lock => Some(KEYCODE_SLEEP),
        Button::VolumeUp => Some(KEYCODE_VOLUME_UP),
        Button::VolumeDown => Some(KEYCODE_VOLUME_DOWN),
    }
}

/// Points become device pixels here and nowhere else.
pub fn gesture_request(g: &Gesture, density: f32) -> Value {
    let pointers: Vec<Value> = g
        .fingers
        .iter()
        .enumerate()
        .map(|(id, path)| {
            let path: Vec<Value> = path
                .iter()
                .map(|p| {
                    json!({
                        "x": (p.at.x * density).round() as i64,
                        "y": (p.at.y * density).round() as i64,
                        "t_ms": p.t_ms,
                    })
                })
                .collect();
            json!({"id": id, "path": path})
        })
        .collect();
    json!({"op": "gesture", "pointers": pointers})
}

pub fn text_request(t: &TextInput) -> Value {
    let (mode, text) = match t {
        TextInput::Paste(s) => ("paste", s),
        TextInput::SetValue(s) => ("set", s),
        TextInput::Keystrokes(s) => ("keys", s),
    };
    json!({"op": "text", "mode": mode, "text": text})
}

/// Maps an event the agent pushed on the control stream; unknown events are ignored.
pub fn event(v: &Value, density: f32) -> Option<DeviceEvent> {
    match v["ev"].as_str()? {
        "window_changed" => Some(DeviceEvent::WindowChanged {
            package: v["pkg"].as_str().map(str::to_string),
        }),
        "display_changed" => Some(DeviceEvent::DisplayChanged {
            screen: Size {
                w: v["w"].as_f64()? as f32 / density,
                h: v["h"].as_f64()? as f32 / density,
            },
            rotation: v["rotation"].as_u64()? as u8,
        }),
        "state" => Some(DeviceEvent::Locked(v["locked"].as_bool()?)),
        _ => None,
    }
}

/// Turns a failed agent reply into the error an agent should see.
pub fn reply_error(op: Op, error: &str) -> Error {
    if error.contains("unmappable character") {
        return Error::Unsupported {
            op,
            reason: "keystroke mode has no key for this character; use paste mode",
        };
    }
    Error::Transport {
        detail: format!("the agent could not run {}: {error}", op.name()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phone_use_core::device::TimedPoint;
    use phone_use_core::element::Role;
    use phone_use_core::geometry::Point;
    use phone_use_core::prune::prune;

    const SETTINGS: &str =
        include_str!("../../core/tests/fixtures/android/real_settings_main.json");

    #[test]
    fn raw_tree_converts_pixels_to_points_and_keeps_roles_and_flags() {
        let reply: Value = serde_json::from_str(SETTINGS).unwrap();
        let tree = raw_tree(&reply, 2.625).unwrap();
        let nodes = prune(&tree);
        let net = nodes
            .iter()
            .find(|n| {
                n.label
                    .as_deref()
                    .is_some_and(|l| l.starts_with("Network & internet"))
            })
            .expect("Network & internet");
        assert_eq!(net.role, Role::Button);
        let px = &reply["nodes"][net.raw]["b"];
        let (l, t, r) = (
            px[0].as_f64().unwrap() as f32,
            px[1].as_f64().unwrap() as f32,
            px[2].as_f64().unwrap() as f32,
        );
        assert!((net.bounds.x - l / 2.625).abs() < 0.01, "{:?}", net.bounds);
        assert!((net.bounds.y - t / 2.625).abs() < 0.01, "{:?}", net.bounds);
        assert!(
            (net.bounds.w - (r - l) / 2.625).abs() < 0.01,
            "{:?}",
            net.bounds
        );
        assert!(net.bounds.w < 500.0, "points, not pixels: {:?}", net.bounds);
    }

    #[test]
    fn raw_tree_rejects_garbage_as_a_transport_error() {
        let err = raw_tree(&json!({"nodes": [{"p": 5}]}), 2.0).unwrap_err();
        assert!(matches!(err, Error::Transport { .. }));
    }

    #[test]
    fn gestures_leave_in_pixels_one_finger_per_pointer_id() {
        let g = Gesture {
            fingers: vec![
                vec![
                    TimedPoint {
                        at: Point { x: 100.0, y: 200.0 },
                        t_ms: 0,
                    },
                    TimedPoint {
                        at: Point { x: 100.4, y: 200.0 },
                        t_ms: 50,
                    },
                ],
                vec![TimedPoint {
                    at: Point { x: 10.5, y: 20.25 },
                    t_ms: 0,
                }],
            ],
        };
        let r = gesture_request(&g, 2.625);
        assert_eq!(r["op"], "gesture");
        assert_eq!(r["pointers"][0]["id"], 0);
        assert_eq!(
            r["pointers"][0]["path"][0],
            json!({"x": 263, "y": 525, "t_ms": 0})
        );
        assert_eq!(r["pointers"][0]["path"][1]["x"], 264);
        assert_eq!(r["pointers"][1]["id"], 1);
        assert_eq!(
            r["pointers"][1]["path"][0],
            json!({"x": 28, "y": 53, "t_ms": 0})
        );
    }

    #[test]
    fn keys_and_buttons_map_to_android_keycodes() {
        assert_eq!(keycode(Key::Enter), 66);
        assert_eq!(keycode(Key::Backspace), 67);
        assert_eq!(button_keycode(Button::Home), Some(3));
        assert_eq!(button_keycode(Button::Back), Some(4));
        assert_eq!(button_keycode(Button::Recents), Some(187));
        assert_eq!(button_keycode(Button::Notifications), None);
    }

    #[test]
    fn pushed_events_map_to_device_events_in_points() {
        assert_eq!(
            event(
                &json!({"ev":"window_changed","w":12,"pkg":"com.android.settings","t":5}),
                2.625
            ),
            Some(DeviceEvent::WindowChanged {
                package: Some("com.android.settings".into())
            })
        );
        assert_eq!(
            event(
                &json!({"ev":"display_changed","w":2625,"h":1050,"rotation":1}),
                2.625
            ),
            Some(DeviceEvent::DisplayChanged {
                screen: Size {
                    w: 1000.0,
                    h: 400.0
                },
                rotation: 1
            })
        );
        assert_eq!(
            event(&json!({"ev":"state","locked":true}), 2.625),
            Some(DeviceEvent::Locked(true))
        );
        assert_eq!(event(&json!({"ev":"nope"}), 2.625), None);
        assert_eq!(event(&json!({"id":3,"ok":true}), 2.625), None);
    }

    #[test]
    fn text_modes_map_to_agent_modes_and_unmappable_characters_are_unsupported() {
        assert_eq!(text_request(&TextInput::Paste("a".into()))["mode"], "paste");
        assert_eq!(
            text_request(&TextInput::SetValue("a".into()))["mode"],
            "set"
        );
        assert_eq!(
            text_request(&TextInput::Keystrokes("a".into()))["mode"],
            "keys"
        );
        assert!(matches!(
            reply_error(
                Op::Text,
                "java.lang.IllegalArgumentException: unmappable character U+E9 at 1"
            ),
            Error::Unsupported { op: Op::Text, .. }
        ));
        assert!(matches!(
            reply_error(Op::Tree, "boom"),
            Error::Transport { .. }
        ));
    }
}
