//! Translation between the `Device` port's types and the runner's wire format (protocol section 3).

use phone_use_core::device::{App, AppId, Gesture, Key, TimedPoint};
use phone_use_core::element::Role;
use phone_use_core::error::Error;
use phone_use_core::geometry::{Point, Rect, Size};
use phone_use_core::raw::{RawTree, RawWindow};
use serde_json::{Value, json};

/// The screen in points as the app sees it now: the application element's frame follows the orientation, while the
/// size the runner reported at connect does not.
pub fn screen_of(reply: &Value) -> Option<Size> {
    let b = reply["nodes"].get(0)?["b"].as_array()?;
    let (w, h) = (b.get(2)?.as_f64()? as f32, b.get(3)?.as_f64()? as f32);
    (w > 0.0 && h > 0.0).then_some(Size { w, h })
}

/// The runner reports one foreground app and no windows, so the app becomes the application window and the
/// on-screen keyboard, when present, the input-method window the header and reading order look for.
pub fn raw_tree(reply: &Value, screen: Size) -> Result<RawTree, Error> {
    let mut wire = reply.clone();
    tidy(&mut wire);
    let mut tree = RawTree::from_ios_json(&wire.to_string()).map_err(|e| Error::Transport {
        detail: format!("the runner sent a snapshot that does not parse: {e}"),
    })?;
    let full = Rect {
        x: 0.0,
        y: 0.0,
        w: screen.w,
        h: screen.h,
    };
    tree.windows.push(RawWindow {
        id: 0,
        kind: "application".into(),
        package: reply["app"]["bundle_id"].as_str().map(str::to_string),
        layer: 0,
        bounds: full,
    });
    if let Some(k) = tree.nodes.iter().find(|n| n.role == Role::Keyboard) {
        tree.windows.push(RawWindow {
            id: -1,
            kind: "input_method".into(),
            package: None,
            layer: 1,
            bounds: k.bounds,
        });
    }
    Ok(tree)
}

/// XCUI exposes each scroll view's indicators as an "Other" element labeled "Vertical scroll bar, 2 pages" with a
/// percentage value. They are never a target, and as the only labeled child of a multi-line field they became its
/// label. The label is localized; this matches the English one the Simulator uses.
fn scroll_indicator(n: &Value) -> bool {
    n["cls"] == "XCUIElementTypeOther"
        && n["text"]
            .as_str()
            .is_some_and(|t| t.contains("scroll bar, "))
        && n["sd"].as_str().is_some_and(|v| v.ends_with('%'))
}

/// Removes scroll indicators and inner switches with their subtrees from the runner's preorder node list,
/// renumbering parents. A toggle row is a labeled switch the width of the row holding an unlabeled switch, the
/// control itself (Settings and SwiftUI `Toggle`); a tap at the row's center does not toggle it, so the row takes
/// the inner switch's bounds and the inner switch goes.
fn tidy(reply: &mut Value) {
    let Some(nodes) = reply["nodes"].as_array_mut() else {
        return;
    };
    let parent = |n: &Value| usize::try_from(n["p"].as_i64().unwrap_or(-1)).ok();
    let is_switch = |n: &Value| n["cls"] == "XCUIElementTypeSwitch";
    let mut drop: Vec<bool> = nodes.iter().map(scroll_indicator).collect();
    for i in 0..nodes.len() {
        if let Some(p) = parent(&nodes[i])
            && is_switch(&nodes[i])
            && is_switch(&nodes[p])
        {
            nodes[p]["b"] = nodes[i]["b"].clone();
            drop[i] = true;
        }
    }
    let mut index: Vec<Option<i64>> = Vec::with_capacity(nodes.len());
    let mut kept = Vec::with_capacity(nodes.len());
    for (i, mut n) in std::mem::take(nodes).into_iter().enumerate() {
        let new_parent = match parent(&n) {
            Some(p) => index.get(p).copied().flatten(),
            None => Some(-1),
        };
        match new_parent {
            Some(p) if !drop[i] => {
                n["p"] = p.into();
                index.push(Some(kept.len() as i64));
                kept.push(n);
            }
            _ => index.push(None),
        }
    }
    *nodes = kept;
}

pub fn gesture_request(g: &Gesture) -> Value {
    let pointers: Vec<Value> = g
        .fingers
        .iter()
        .map(|path| {
            let path: Vec<Value> = path
                .iter()
                .map(|p| json!({"x": p.at.x, "y": p.at.y, "t_ms": p.t_ms}))
                .collect();
            json!({"path": path})
        })
        .collect();
    json!({"op": "gesture", "pointers": pointers})
}

/// XCTest's `XCUIKeyboardKey` strings.
pub fn key_string(k: Key) -> &'static str {
    match k {
        Key::Enter => "\r",
        Key::Backspace => "\u{8}",
        Key::Tab => "\t",
        Key::Escape => "\u{1b}",
        Key::ArrowUp => "\u{F700}",
        Key::ArrowDown => "\u{F701}",
        Key::ArrowLeft => "\u{F702}",
        Key::ArrowRight => "\u{F703}",
    }
}

/// The center of the navigation bar's back button. UIKit and SwiftUI both name it `BackButton`.
pub fn back_button(tree: &RawTree) -> Option<Point> {
    tree.nodes
        .iter()
        .find(|n| {
            n.role == Role::Button
                && n.resource_id.as_deref() == Some("BackButton")
                && n.bounds.area() > 0.0
        })
        .map(|n| n.bounds.center())
}

fn swipe(from: Point, to: Point, ms: u32) -> Gesture {
    let steps = 10;
    let path = (0..=steps)
        .map(|s| {
            let f = s as f32 / steps as f32;
            TimedPoint {
                at: Point {
                    x: from.x + (to.x - from.x) * f,
                    y: from.y + (to.y - from.y) * f,
                },
                t_ms: ms * s / steps,
            }
        })
        .collect();
    Gesture {
        fingers: vec![path],
    }
}

/// iOS's interactive pop: a drag from the left edge to past the middle.
pub fn back_swipe(screen: Size) -> Gesture {
    let y = screen.h / 2.0;
    swipe(
        Point { x: 1.0, y },
        Point {
            x: screen.w * 0.7,
            y,
        },
        300,
    )
}

/// Notification Center opens with a drag down from the top edge, left of the notch.
pub fn notifications_swipe(screen: Size) -> Gesture {
    let x = screen.w * 0.2;
    swipe(
        Point { x, y: 1.0 },
        Point {
            x,
            y: screen.h * 0.6,
        },
        300,
    )
}

/// Parses `simctl listapps` output converted to JSON by `plutil`.
pub fn apps(json: &Value) -> Vec<App> {
    let mut out: Vec<App> = json
        .as_object()
        .into_iter()
        .flatten()
        .map(|(id, info)| App {
            id: AppId(id.clone()),
            name: info["CFBundleDisplayName"]
                .as_str()
                .or_else(|| info["CFBundleName"].as_str())
                .unwrap_or(id)
                .to_string(),
            system: info["ApplicationType"] == "System",
        })
        .collect();
    out.sort_by(|a, b| a.id.0.cmp(&b.id.0));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TESTBED_MAIN: &str = include_str!("../../core/tests/fixtures/ios/testbed_main.json");
    const SCREEN: Size = Size { w: 402.0, h: 874.0 };

    #[test]
    fn the_screen_is_the_application_frame() {
        let reply: Value = serde_json::from_str(TESTBED_MAIN).unwrap();
        assert_eq!(screen_of(&reply), Some(SCREEN));
        let landscape = json!({"nodes": [{"cls": "XCUIElementTypeApplication", "p": -1, "b": [0, 0, 1376, 1032]}]});
        assert_eq!(
            screen_of(&landscape),
            Some(Size {
                w: 1376.0,
                h: 1032.0
            })
        );
        assert_eq!(screen_of(&json!({"nodes": []})), None);
    }

    #[test]
    fn the_foreground_app_becomes_the_application_window() {
        let reply: Value = serde_json::from_str(TESTBED_MAIN).unwrap();
        let tree = raw_tree(&reply, SCREEN).unwrap();
        assert_eq!(tree.windows.len(), 1);
        assert_eq!(tree.windows[0].kind, "application");
        assert_eq!(
            tree.windows[0].package.as_deref(),
            Some("dev.phoneuse.testbed")
        );
        assert_eq!(tree.windows[0].bounds.w, 402.0);
    }

    #[test]
    fn an_on_screen_keyboard_becomes_the_input_method_window() {
        let reply = json!({"app": {"bundle_id": "x", "pid": 1}, "nodes": [
            {"p": -1, "cls": "XCUIElementTypeApplication", "b": [0, 0, 402, 874], "f": "vVe"},
            {"p": 0, "cls": "XCUIElementTypeKeyboard", "b": [0, 574, 402, 874], "f": "vVe"}
        ]});
        let tree = raw_tree(&reply, SCREEN).unwrap();
        let kb = tree
            .windows
            .iter()
            .find(|w| w.kind == "input_method")
            .unwrap();
        assert_eq!(kb.bounds.y, 574.0);
        assert_eq!(kb.bounds.h, 300.0);
    }

    #[test]
    fn scroll_indicators_are_dropped_and_do_not_name_their_field() {
        let reply = json!({"app": {"bundle_id": "x", "pid": 1}, "nodes": [
            {"p": -1, "cls": "XCUIElementTypeApplication", "b": [0, 0, 402, 874], "f": "vVe"},
            {"p": 0, "cls": "XCUIElementTypeTextField", "id": "text.multi", "hint": "Multiline", "a": ["tap"], "b": [32, 164, 370, 228], "f": "vVeE"},
            {"p": 1, "cls": "XCUIElementTypeOther", "text": "Horizontal scroll bar, 1 page", "sd": "0%", "b": [32, 195, 370, 225], "f": "vVe"},
            {"p": 2, "cls": "XCUIElementTypeOther", "b": [32, 195, 370, 225], "f": "vVe"},
            {"p": 0, "cls": "XCUIElementTypeOther", "text": "Vertical scroll bar, 2 pages", "sd": "0%", "b": [369, 62, 399, 791], "f": "vVe"},
            {"p": 4, "cls": "XCUIElementTypeOther", "b": [369, 62, 399, 791], "f": "vVe"},
            {"p": 0, "cls": "XCUIElementTypeButton", "text": "Done", "a": ["tap"], "b": [0, 300, 60, 340], "f": "vVe"}
        ]});
        let tree = raw_tree(&reply, SCREEN).unwrap();
        let labels: Vec<Option<&str>> = tree.nodes.iter().map(|n| n.label.as_deref()).collect();
        assert_eq!(labels, vec![None, None, Some("Done")]);
        assert_eq!(tree.nodes[1].parent, Some(0));
        assert_eq!(tree.nodes[2].parent, Some(0));
    }

    #[test]
    fn a_switch_row_takes_the_bounds_of_the_switch_inside_it() {
        let reply = json!({"app": {"bundle_id": "x", "pid": 1}, "nodes": [
            {"p": -1, "cls": "XCUIElementTypeApplication", "b": [0, 0, 402, 874], "f": "vVe"},
            {"p": 0, "cls": "XCUIElementTypeSwitch", "id": "UIAppearanceSettings", "text": "Dark Appearance", "sd": "0", "a": ["tap"], "f": "vVec", "b": [36, 184, 366, 212]},
            {"p": 1, "cls": "XCUIElementTypeButton", "id": "UIAppearanceSettings", "text": "Dark Appearance", "a": ["tap"], "f": "vVe", "b": [36, 187.67, 168.33, 208]},
            {"p": 1, "cls": "XCUIElementTypeSwitch", "sd": "0", "a": ["tap"], "f": "vVec", "b": [305, 184, 368, 212]},
            {"p": 0, "cls": "XCUIElementTypeButton", "text": "Done", "a": ["tap"], "b": [0, 300, 60, 340], "f": "vVe"}
        ]});
        let tree = raw_tree(&reply, SCREEN).unwrap();
        let switches: Vec<_> = tree
            .nodes
            .iter()
            .filter(|n| n.role == Role::Switch)
            .collect();
        assert_eq!(switches.len(), 1);
        assert_eq!(switches[0].label.as_deref(), Some("Dark Appearance"));
        assert_eq!(switches[0].bounds.center(), Point { x: 336.5, y: 198.0 });
        assert_eq!(tree.nodes.len(), 4);
        assert_eq!(tree.nodes[3].parent, Some(0));
    }

    #[test]
    fn gestures_stay_in_points() {
        let g = Gesture {
            fingers: vec![vec![
                TimedPoint {
                    at: Point { x: 10.5, y: 20.0 },
                    t_ms: 0,
                },
                TimedPoint {
                    at: Point { x: 30.0, y: 40.0 },
                    t_ms: 120,
                },
            ]],
        };
        assert_eq!(
            gesture_request(&g),
            json!({"op": "gesture", "pointers": [{"path": [
                {"x": 10.5, "y": 20.0, "t_ms": 0}, {"x": 30.0, "y": 40.0, "t_ms": 120}]}]})
        );
    }

    #[test]
    fn the_back_button_is_the_one_named_back_button() {
        let reply = json!({"app": {"bundle_id": "x", "pid": 1}, "nodes": [
            {"p": -1, "cls": "XCUIElementTypeApplication", "b": [0, 0, 402, 874], "f": "vVe"},
            {"p": 0, "cls": "XCUIElementTypeNavigationBar", "id": "Level 2", "b": [0, 62, 402, 168], "f": "vVe"},
            {"p": 1, "cls": "XCUIElementTypeButton", "text": "Menu", "a": ["tap"], "b": [0, 62, 10, 106], "f": "vVe"},
            {"p": 1, "cls": "XCUIElementTypeButton", "id": "BackButton", "text": "Level 1", "a": ["tap"], "b": [16, 62, 60, 106], "f": "vVe"},
            {"p": 0, "cls": "XCUIElementTypeButton", "text": "Elsewhere", "a": ["tap"], "b": [0, 300, 60, 340], "f": "vVe"}
        ]});
        let tree = raw_tree(&reply, SCREEN).unwrap();
        assert_eq!(back_button(&tree), Some(Point { x: 38.0, y: 84.0 }));
        let none = json!({"app": {"bundle_id": "x", "pid": 1}, "nodes": [
            {"p": -1, "cls": "XCUIElementTypeApplication", "b": [0, 0, 402, 874], "f": "vVe"},
            {"p": 0, "cls": "XCUIElementTypeButton", "text": "Elsewhere", "a": ["tap"], "b": [0, 300, 60, 340], "f": "vVe"}
        ]});
        assert_eq!(back_button(&raw_tree(&none, SCREEN).unwrap()), None);
    }

    #[test]
    fn the_back_swipe_starts_at_the_left_edge_and_crosses_the_middle() {
        let g = back_swipe(SCREEN);
        let path = &g.fingers[0];
        assert!(path[0].at.x <= 2.0);
        assert!(path.last().unwrap().at.x > SCREEN.w / 2.0);
        assert!(path.windows(2).all(|w| w[1].t_ms >= w[0].t_ms));
    }

    #[test]
    fn listapps_names_and_system_flag() {
        let json = json!({
            "com.apple.Preferences": {"CFBundleDisplayName": "Settings", "ApplicationType": "System"},
            "dev.phoneuse.testbed": {"CFBundleName": "Testbed", "ApplicationType": "User"}
        });
        assert_eq!(
            apps(&json),
            vec![
                App {
                    id: AppId("com.apple.Preferences".into()),
                    name: "Settings".into(),
                    system: true
                },
                App {
                    id: AppId("dev.phoneuse.testbed".into()),
                    name: "Testbed".into(),
                    system: false
                },
            ]
        );
    }
}
