//! Translation between the `Device` port's types and the runner's wire format (protocol section 3).

use phonedriver_core::device::{App, AppId, Gesture, Key, TimedPoint};
use phonedriver_core::element::Role;
use phonedriver_core::error::Error;
use phonedriver_core::geometry::{Point, Rect, Size};
use phonedriver_core::raw::{RawNode, RawTree, RawWindow};
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
    add_alerts(&mut tree, reply)?;
    if let Some(k) = tree.nodes.iter().find(|n| n.role == Role::Keyboard) {
        let top = (k.bounds.y - KEYBOARD_GLASS).max(0.0);
        tree.windows.push(RawWindow {
            id: -1,
            kind: "input_method".into(),
            package: None,
            layer: 1,
            bounds: Rect {
                x: k.bounds.x,
                y: top,
                w: k.bounds.w,
                h: (screen.h - top).max(k.bounds.y + k.bounds.h - top),
            },
        });
    }
    Ok(tree)
}

/// iOS 26 draws the keyboard on a glass panel that starts above the keys' frame and runs to the bottom of the screen
/// (iPhone 17 Simulator, iOS 26.5: keys from y 583, panel from y 566). A row in that band shows through the glass
/// but a tap there lands on the panel.
const KEYBOARD_GLASS: f32 = 18.0;

/// SpringBoard's alerts (permission prompts, "Allow Paste") come in `alerts`, one node list each, because the
/// foreground app's snapshot does not hold them. Each becomes a window of kind `alert` in front of the app.
fn add_alerts(tree: &mut RawTree, reply: &Value) -> Result<(), Error> {
    let Some(alerts) = reply["alerts"].as_array() else {
        return Ok(());
    };
    let mut windows = Vec::new();
    let mut front: Vec<RawNode> = Vec::new();
    for (k, nodes) in alerts.iter().enumerate() {
        let part = RawTree::from_ios_json(&json!({"nodes": nodes}).to_string()).map_err(|e| {
            Error::Transport {
                detail: format!("the runner sent an alert that does not parse: {e}"),
            }
        })?;
        let Some(first) = part.nodes.first() else {
            continue;
        };
        let id = -2 - k as i32;
        windows.push(RawWindow {
            id,
            kind: "alert".into(),
            package: Some("com.apple.springboard".into()),
            layer: 2,
            bounds: first.bounds,
        });
        let base = front.len();
        for mut n in part.nodes {
            n.window = id;
            n.parent = n.parent.map(|p| p + base);
            front.push(n);
        }
    }
    let shift = front.len();
    for n in &mut tree.nodes {
        n.parent = n.parent.map(|p| p + shift);
    }
    front.append(&mut tree.nodes);
    tree.nodes = front;
    windows.append(&mut tree.windows);
    tree.windows = windows;
    Ok(())
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

/// Which way an indicator runs, how many pages its scroll view has, and where it is, as `(vertical, pages, percent)`.
fn scroll_position(n: &Value) -> Option<(bool, u32, f32)> {
    let text = n["text"].as_str()?;
    let (axis, rest) = text.split_once(" scroll bar, ")?;
    let pages = rest.split_whitespace().next()?.parse().ok()?;
    let percent = n["sd"].as_str()?.trim_end_matches('%').parse().ok()?;
    Some((axis == "Vertical", pages, percent))
}

/// Removes scroll indicators and inner switches with their subtrees from the runner's preorder node list,
/// renumbering parents. The window that holds the keyboard keeps only the `Keyboard` node (now a child of the
/// application): its keys and its Emoji and Dictate buttons are the keyboard, and typing is `type`. A scroll view
/// has no scroll actions in XCUI, but its indicator says where it is, so the indicator becomes `scroll_up` and
/// `scroll_down` (or left and right) on its owner and `N pages` as its value. A toggle row is a labeled switch the
/// width of the row holding an unlabeled switch, the control itself (Settings and SwiftUI `Toggle`); a tap at the row's center does not toggle it, so the row takes
/// the inner switch's bounds and the inner switch goes.
fn tidy(reply: &mut Value) {
    let Some(nodes) = reply["nodes"].as_array_mut() else {
        return;
    };
    let parent = |n: &Value| usize::try_from(n["p"].as_i64().unwrap_or(-1)).ok();
    let is_switch = |n: &Value| n["cls"] == "XCUIElementTypeSwitch";
    let mut drop: Vec<bool> = nodes.iter().map(scroll_indicator).collect();
    for i in 0..nodes.len() {
        if drop[i]
            && let Some(p) = parent(&nodes[i])
            && let Some((vertical, pages, percent)) = scroll_position(&nodes[i])
            && pages > 1
        {
            let (back, forward) = if vertical {
                ("scroll_up", "scroll_down")
            } else {
                ("scroll_left", "scroll_right")
            };
            let owner = &mut nodes[p];
            if !owner["a"].is_array() {
                owner["a"] = json!([]);
            }
            let actions = owner["a"].as_array_mut().expect("set above");
            for (wanted, name) in [(percent > 0.0, back), (percent < 100.0, forward)] {
                if wanted && !actions.iter().any(|a| a == name) {
                    actions.push(name.into());
                }
            }
            if vertical && owner["sd"].is_null() {
                owner["sd"] = format!("{pages} pages").into();
            }
        }
        if let Some(p) = parent(&nodes[i])
            && is_switch(&nodes[i])
            && is_switch(&nodes[p])
        {
            nodes[p]["b"] = nodes[i]["b"].clone();
            drop[i] = true;
        }
    }
    if let Some(k) = nodes
        .iter()
        .position(|n| n["cls"] == "XCUIElementTypeKeyboard")
    {
        let mut window = k;
        while let Some(p) = parent(&nodes[window]).filter(|&p| p != 0) {
            window = p;
        }
        for (i, dropped) in drop.iter_mut().enumerate() {
            let mut at = Some(i);
            while let Some(a) = at {
                if a == window {
                    *dropped = i != k;
                    break;
                }
                at = parent(&nodes[a]);
            }
        }
        nodes[k]["p"] = 0.into();
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

/// XCTest sends no touch-moved event for a point that does not move, so UIKit keeps the velocity of the last real
/// move and a scroll that held still before lifting still flung Safari 143 to 194 pt further. Half a point back and
/// forth on each still sample reads as stopped (0 pt of drift in the same test).
const STILL_WIGGLE: f32 = 0.5;
/// XCTest moves the finger in a straight line between the points it is given, and each point costs time: a 400 ms
/// path of 17 points took 1,240 ms to play and one of 5 points 870 to 950 ms, scrolling as far.
const STILL_STEP_MS: u32 = 50;

fn for_xctest(path: &[TimedPoint]) -> Vec<Value> {
    let still = |i: usize| i > 0 && path[i - 1].at == path[i].at && path[i - 1].t_ms < path[i].t_ms;
    let straight = |i: usize| {
        let (a, b, c) = (&path[i - 1], &path[i], &path[i + 1]);
        let (t0, t1, t2) = (a.t_ms as f32, b.t_ms as f32, c.t_ms as f32);
        if t2 <= t0 || b.at == c.at || a.at == b.at {
            return false;
        }
        let f = (t1 - t0) / (t2 - t0);
        let x = a.at.x + (c.at.x - a.at.x) * f;
        let y = a.at.y + (c.at.y - a.at.y) * f;
        (x - b.at.x).abs() < 0.01 && (y - b.at.y).abs() < 0.01
    };
    let round = |v: f32| (v * 100.0).round() / 100.0;
    let mut out = Vec::new();
    let mut last_kept = 0u32;
    let mut wiggles = 0u32;
    for (i, p) in path.iter().enumerate() {
        let end = i + 1 == path.len();
        let y = if still(i) {
            if !end && p.t_ms < last_kept + STILL_STEP_MS {
                continue;
            }
            wiggles += 1;
            if wiggles % 2 == 1 {
                STILL_WIGGLE
            } else {
                -STILL_WIGGLE
            }
        } else {
            if i > 0 && !end && straight(i) {
                continue;
            }
            0.0
        };
        last_kept = p.t_ms;
        out.push(json!({"x": round(p.at.x), "y": round(p.at.y + y), "t_ms": p.t_ms}));
    }
    out
}

pub fn gesture_request(g: &Gesture) -> Value {
    let pointers: Vec<Value> = g
        .fingers
        .iter()
        .map(|path| json!({"path": for_xctest(path)}))
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

/// How to close the on-screen keyboard.
#[derive(Debug, Clone, PartialEq)]
pub enum HidePlan {
    /// A "Hide keyboard" key (iPad keyboards have one).
    Tap(Point),
    /// Drag the content above the keyboard up: forms that dismiss on drag (Contacts, Settings) hide it. A drag down
    /// would also dismiss a sheet such as Contacts' New Contact, and lose its draft.
    Drag(Gesture),
}

/// Reads the runner's snapshot, keys included, and plans how to hide the keyboard; `None` when none is shown.
pub fn hide_keyboard_plan(reply: &Value, screen: Size) -> Option<HidePlan> {
    let nodes = reply["nodes"].as_array()?;
    let keyboard = nodes
        .iter()
        .find(|n| n["cls"] == "XCUIElementTypeKeyboard")?;
    let bounds = |n: &Value| -> Option<[f32; 4]> {
        let a = n["b"].as_array()?;
        Some([
            a.first()?.as_f64()? as f32,
            a.get(1)?.as_f64()? as f32,
            a.get(2)?.as_f64()? as f32,
            a.get(3)?.as_f64()? as f32,
        ])
    };
    let hide_key = nodes.iter().find(|n| {
        n["cls"] == "XCUIElementTypeButton"
            && n["text"]
                .as_str()
                .is_some_and(|t| t.eq_ignore_ascii_case("hide keyboard"))
    });
    if let Some([left, top, right, bottom]) = hide_key.and_then(bounds) {
        return Some(HidePlan::Tap(Point {
            x: (left + right) / 2.0,
            y: (top + bottom) / 2.0,
        }));
    }
    let [_, top, _, _] = bounds(keyboard)?;
    let from = Point {
        x: screen.w / 2.0,
        y: top - 40.0,
    };
    let to = Point {
        x: screen.w / 2.0,
        y: (top - 160.0).max(screen.h * 0.1),
    };
    Some(HidePlan::Drag(swipe(from, to, 300)))
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
    fn a_springboard_alert_becomes_an_alert_window_in_front_of_the_app() {
        let mut reply: Value = serde_json::from_str(TESTBED_MAIN).unwrap();
        reply["alerts"] = json!([[
            {"p": -1, "cls": "XCUIElementTypeAlert", "text": "Allow “Testbed” to use your location?", "b": [51, 300, 351, 500], "f": "vVe"},
            {"p": 0, "cls": "XCUIElementTypeStaticText", "text": "Allow “Testbed” to use your location?", "b": [60, 310, 340, 350], "f": "vVe"},
            {"p": 0, "cls": "XCUIElementTypeButton", "text": "Allow While Using App", "b": [60, 420, 340, 460], "f": "vVe", "a": ["tap"]}
        ]]);
        let tree = raw_tree(&reply, SCREEN).unwrap();
        assert_eq!(tree.windows[0].kind, "alert");
        assert_eq!(tree.windows[1].kind, "application");
        let alerts = phonedriver_core::alert::find(&tree, SCREEN);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].1.package, "com.apple.springboard");
        assert_eq!(
            alerts[0].1.title.as_deref(),
            Some("Allow “Testbed” to use your location?")
        );
        let nodes = phonedriver_core::prune::prune(&tree);
        let first = &tree.nodes[nodes[0].raw];
        assert_eq!(first.role, Role::Alert);
        assert!(
            nodes
                .iter()
                .any(|n| n.label.as_deref() == Some("Allow While Using App") && n.parent.is_some())
        );
    }

    #[test]
    fn the_foreground_app_becomes_the_application_window() {
        let reply: Value = serde_json::from_str(TESTBED_MAIN).unwrap();
        let tree = raw_tree(&reply, SCREEN).unwrap();
        assert_eq!(tree.windows.len(), 1);
        assert_eq!(tree.windows[0].kind, "application");
        assert_eq!(
            tree.windows[0].package.as_deref(),
            Some("dev.phonedriver.testbed")
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
        assert_eq!(kb.bounds.y, 574.0 - KEYBOARD_GLASS);
        assert_eq!(kb.bounds.h, 300.0 + KEYBOARD_GLASS);
    }

    #[test]
    fn the_keyboard_window_covers_the_glass_panel_around_the_keys_down_to_the_screen_edge() {
        let reply = json!({"app": {"bundle_id": "x", "pid": 1}, "nodes": [
            {"p": -1, "cls": "XCUIElementTypeApplication", "b": [0, 0, 402, 874], "f": "vVe"},
            {"p": 0, "cls": "XCUIElementTypeKeyboard", "b": [0, 583, 402, 816], "f": "vVe"}
        ]});
        let tree = raw_tree(&reply, SCREEN).unwrap();
        let kb = tree
            .windows
            .iter()
            .find(|w| w.kind == "input_method")
            .unwrap();
        assert!(
            kb.bounds.y <= 566.0,
            "the panel is drawn from y 566: {:?}",
            kb.bounds
        );
        assert_eq!(kb.bounds.y + kb.bounds.h, 874.0);
    }

    fn keyboard_reply() -> Value {
        json!({"app": {"bundle_id": "x", "pid": 1}, "nodes": [
            {"p": -1, "cls": "XCUIElementTypeApplication", "b": [0, 0, 402, 874], "f": "vVe"},
            {"p": 0, "cls": "XCUIElementTypeWindow", "b": [0, 0, 402, 874], "f": "vVe"},
            {"p": 1, "cls": "XCUIElementTypeTextField", "text": "Plain", "a": ["tap"], "b": [32, 112, 370, 134], "f": "vVeFE"},
            {"p": 0, "cls": "XCUIElementTypeWindow", "b": [0, 0, 402, 874], "f": "vVe"},
            {"p": 3, "cls": "XCUIElementTypeOther", "b": [0, 566, 402, 874], "f": "vVe"},
            {"p": 4, "cls": "XCUIElementTypeKeyboard", "b": [0, 583, 402, 816], "f": "vVe"},
            {"p": 5, "cls": "XCUIElementTypeButton", "text": "Q", "a": ["tap"], "b": [4, 700, 40, 750], "f": "vVe"},
            {"p": 5, "cls": "XCUIElementTypeButton", "text": "return", "a": ["tap"], "b": [300, 752, 399, 806], "f": "vVe"},
            {"p": 4, "cls": "XCUIElementTypeOther", "b": [0, 583, 402, 874], "f": "vVe"},
            {"p": 8, "cls": "XCUIElementTypeButton", "text": "Emoji", "a": ["tap"], "b": [8, 805, 76, 874], "f": "vVe"},
            {"p": 8, "cls": "XCUIElementTypeButton", "text": "Dictate", "a": ["tap"], "b": [325, 805, 393, 874], "f": "vVe"}
        ]})
    }

    #[test]
    fn the_keyboard_window_is_one_keyboard_node_without_keys_or_accessory_buttons() {
        let tree = raw_tree(&keyboard_reply(), SCREEN).unwrap();
        let roles: Vec<Role> = tree.nodes.iter().map(|n| n.role).collect();
        assert_eq!(
            roles.iter().filter(|r| **r == Role::Keyboard).count(),
            1,
            "{roles:?}"
        );
        assert!(
            tree.nodes
                .iter()
                .all(|n| n.label.as_deref() == Some("Plain") || n.label.is_none()),
            "{:?}",
            tree.nodes.iter().map(|n| &n.label).collect::<Vec<_>>()
        );
        let kept = phonedriver_core::prune::prune(&tree);
        let shown: Vec<Role> = kept.iter().map(|n| n.role).collect();
        assert_eq!(shown, [Role::TextField, Role::Keyboard]);
        let kb = tree
            .windows
            .iter()
            .find(|w| w.kind == "input_method")
            .unwrap();
        assert_eq!(
            (kb.bounds.y, kb.bounds.h),
            (583.0 - KEYBOARD_GLASS, 874.0 - 583.0 + KEYBOARD_GLASS)
        );
    }

    #[test]
    fn hiding_the_keyboard_taps_a_hide_key_when_there_is_one_and_otherwise_drags_the_form_up() {
        assert_eq!(hide_keyboard_plan(&json!({"nodes": []}), SCREEN), None);
        let drag = hide_keyboard_plan(&keyboard_reply(), SCREEN).unwrap();
        let HidePlan::Drag(g) = drag else {
            panic!("{drag:?}")
        };
        let path = &g.fingers[0];
        assert!(path[0].at.y < 583.0, "starts above the keyboard: {path:?}");
        assert!(
            path.last().unwrap().at.y < path[0].at.y,
            "moves up, never down: a drag down dismisses a sheet and loses its draft"
        );
        let mut with_key = keyboard_reply();
        with_key["nodes"].as_array_mut().unwrap().push(json!({
            "p": 5, "cls": "XCUIElementTypeButton", "text": "Hide keyboard", "a": ["tap"],
            "b": [340, 760, 400, 810], "f": "vVe"}));
        assert_eq!(
            hide_keyboard_plan(&with_key, SCREEN),
            Some(HidePlan::Tap(Point { x: 370.0, y: 785.0 }))
        );
    }

    fn scrolled(bar: &str, percent: &str) -> Vec<phonedriver_core::prune::Node> {
        let reply = json!({"app": {"bundle_id": "x", "pid": 1}, "nodes": [
            {"p": -1, "cls": "XCUIElementTypeApplication", "b": [0, 0, 402, 874], "f": "vVe"},
            {"p": 0, "cls": "XCUIElementTypeTable", "b": [0, 0, 402, 874], "f": "vVeS"},
            {"p": 1, "cls": "XCUIElementTypeCell", "text": "Row", "a": ["tap"], "b": [0, 100, 402, 150], "f": "vVe"},
            {"p": 1, "cls": "XCUIElementTypeOther", "text": bar, "sd": percent, "b": [369, 62, 399, 791], "f": "vVe"}
        ]});
        phonedriver_core::prune::prune(&raw_tree(&reply, SCREEN).unwrap())
    }

    #[test]
    fn a_list_scroll_bar_says_which_way_the_list_scrolls_and_how_many_pages_it_has() {
        use phonedriver_core::element::Actions;
        let list = |bar, pct| scrolled(bar, pct).remove(0);
        let top = list("Vertical scroll bar, 2 pages", "0%");
        assert_eq!(top.actions, Actions::SCROLL_DOWN);
        assert_eq!(top.value.as_deref(), Some("2 pages"));
        let middle = list("Vertical scroll bar, 3 pages", "48%");
        assert_eq!(middle.actions, Actions::SCROLL_UP | Actions::SCROLL_DOWN);
        let bottom = list("Vertical scroll bar, 2 pages", "100%");
        assert_eq!(bottom.actions, Actions::SCROLL_UP);
        let one_page = list("Vertical scroll bar, 1 page", "0%");
        assert_eq!(one_page.actions, Actions::empty());
        assert_eq!(one_page.value, None);
        let sideways = list("Horizontal scroll bar, 2 pages", "0%");
        assert_eq!(sideways.actions, Actions::SCROLL_RIGHT);
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
    fn a_scroll_goes_to_xctest_as_one_straight_move_and_a_still_hold_that_wiggles() {
        let area = phonedriver_core::geometry::Rect {
            x: 0.0,
            y: 100.0,
            w: 400.0,
            h: 700.0,
        };
        let g = phonedriver_core::gesture::scroll(area, phonedriver_core::error::Direction::Down);
        let sent: Vec<(f64, u64)> = gesture_request(&g)["pointers"][0]["path"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (p["y"].as_f64().unwrap(), p["t_ms"].as_u64().unwrap()))
            .collect();
        assert_eq!(
            sent,
            [
                (660.0, 0),
                (240.0, 250),
                (240.5, 300),
                (239.5, 350),
                (240.5, 400)
            ]
        );
        let at = Point { x: 200.0, y: 180.0 };
        let tap = Gesture {
            fingers: vec![vec![TimedPoint { at, t_ms: 0 }, TimedPoint { at, t_ms: 0 }]],
        };
        assert_eq!(
            gesture_request(&tap)["pointers"][0]["path"],
            json!([{"x": 200.0, "y": 180.0, "t_ms": 0}, {"x": 200.0, "y": 180.0, "t_ms": 0}])
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
            "dev.phonedriver.testbed": {"CFBundleName": "Testbed", "ApplicationType": "User"}
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
                    id: AppId("dev.phonedriver.testbed".into()),
                    name: "Testbed".into(),
                    system: false
                },
            ]
        );
    }
}
