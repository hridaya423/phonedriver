//! System alerts: windows the system, not the foreground app, put over the screen (decision D23).

use serde::{Deserialize, Serialize};

use crate::element::Role;
use crate::geometry::Size;
use crate::raw::{RawNode, RawTree, RawWindow};

/// Packages whose small windows are dialogs: runtime permissions, the installer, and the system server (ANR and crash
/// dialogs, the "open with" chooser).
const DIALOG_PACKAGES: &[&str] = &[
    "com.android.permissioncontroller",
    "com.google.android.permissioncontroller",
    "com.android.packageinstaller",
    "com.google.android.packageinstaller",
    "android",
];

/// Ids of the framework's `AlertDialog` layout and of the "isn't responding" dialog. System UI dialogs (USB debugging,
/// screen capture) are alert windows when they carry one of these; its volume panel and shade do not.
const DIALOG_IDS: &[&str] = &[
    "android:id/alertTitle",
    "android:id/parentPanel",
    "android:id/aerr_close",
    "android:id/aerr_wait",
];

const TITLE_IDS: &[&str] = &[
    "permission_message",
    "alertTitle",
    "aerr_title",
    "message",
    "title",
];

/// A window larger than this share of the screen is a page, not a dialog.
const DIALOG_AREA: f32 = 0.9;

/// A system alert on screen: the package that owns it and the question it asks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertLine {
    pub package: String,
    pub title: Option<String>,
}

/// The alerts of `tree`, topmost first. `screen` is the display size in points. A window of kind `alert` is one
/// already (the iOS runner marks SpringBoard's alerts so).
pub fn find(tree: &RawTree, screen: Size) -> Vec<(i32, AlertLine)> {
    tree.windows
        .iter()
        .filter(|w| is_alert(tree, w, screen))
        .map(|w| {
            let package = w.package.clone().unwrap_or_default();
            (
                w.id,
                AlertLine {
                    package,
                    title: title(tree, w.id),
                },
            )
        })
        .collect()
}

fn nodes_of(tree: &RawTree, window: i32) -> impl Iterator<Item = &RawNode> {
    tree.nodes.iter().filter(move |n| n.window == window)
}

fn is_alert(tree: &RawTree, w: &RawWindow, screen: Size) -> bool {
    if w.kind == "alert" {
        return true;
    }
    if w.kind == "input_method" {
        return false;
    }
    let Some(package) = w.package.as_deref() else {
        return false;
    };
    let framework_dialog = nodes_of(tree, w.id).any(|n| {
        n.resource_id
            .as_deref()
            .is_some_and(|id| DIALOG_IDS.contains(&id))
    });
    let small = w.bounds.area() < DIALOG_AREA * screen.w * screen.h;
    let system = DIALOG_PACKAGES.contains(&package);
    let reads = nodes_of(tree, w.id).any(|n| n.label.is_some());
    reads
        && ((system && small)
            || (framework_dialog && (system || package == "com.android.systemui")))
}

fn title(tree: &RawTree, window: i32) -> Option<String> {
    let by_id = |want: &str| {
        nodes_of(tree, window).find_map(|n| {
            let id = n.resource_id.as_deref()?;
            let tail = id.rsplit('/').next()?;
            (tail == want).then(|| n.label.clone()).flatten()
        })
    };
    TITLE_IDS
        .iter()
        .find_map(|id| by_id(id))
        .or_else(|| {
            nodes_of(tree, window)
                .find(|n| n.role == Role::Heading && n.label.is_some())
                .and_then(|n| n.label.clone())
        })
        .or_else(|| {
            nodes_of(tree, window)
                .find(|n| n.actions.is_empty() && matches!(n.role, Role::Text) && n.label.is_some())
                .and_then(|n| n.label.clone())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Scale;

    const PHONE: Size = Size { w: 411.4, h: 923.4 };

    fn android(json: &str) -> RawTree {
        RawTree::from_android_json(json, Scale { density: 2.625 }).unwrap()
    }

    #[test]
    fn a_real_runtime_permission_dialog_is_an_alert_with_its_question() {
        let tree = android(include_str!(
            "../tests/fixtures/android/real_permission_dialog.json"
        ));
        let alerts = find(&tree, PHONE);
        assert_eq!(alerts.len(), 1);
        assert_eq!(
            alerts[0].1.package,
            "com.google.android.permissioncontroller"
        );
        assert_eq!(
            alerts[0].1.title.as_deref(),
            Some("Allow Camera to take pictures and record video?")
        );
    }

    #[test]
    fn full_screen_pages_of_apps_are_not_alerts() {
        for fixture in [
            include_str!("../tests/fixtures/android/real_settings_main.json"),
            include_str!("../tests/fixtures/android/real_settings_apps.json"),
            include_str!("../tests/fixtures/android/real_dialog.json"),
        ] {
            assert!(find(&android(fixture), PHONE).is_empty());
        }
    }

    #[test]
    fn a_permission_controller_page_that_fills_the_screen_is_not_an_alert() {
        let tree = android(
            r#"{"windows":[{"id":7,"type":"application","package":"com.google.android.permissioncontroller","bounds":[0,0,1080,2424]}],
                "nodes":[{"p":-1,"w":7,"cls":"android.widget.TextView","text":"App permissions","b":[0,0,1080,200],"f":"vVe"}]}"#,
        );
        assert!(find(&tree, PHONE).is_empty());
    }

    #[test]
    fn an_app_not_responding_dialog_over_another_app_is_an_alert() {
        let tree = android(
            r#"{"windows":[
                {"id":9,"type":"system","package":"android","bounds":[100,900,980,1500]},
                {"id":3,"type":"application","package":"com.example.shop","bounds":[0,0,1080,2424]}],
                "nodes":[
                {"p":-1,"w":9,"cls":"android.widget.LinearLayout","id":"android:id/parentPanel","b":[100,900,980,1500],"f":"vVe"},
                {"p":0,"w":9,"cls":"android.widget.TextView","id":"android:id/alertTitle","text":"Shop isn't responding","b":[120,920,960,1000],"f":"vVe"},
                {"p":0,"w":9,"cls":"android.widget.Button","id":"android:id/aerr_close","text":"Close app","b":[120,1100,960,1200],"f":"vVe","a":["tap"]},
                {"p":0,"w":9,"cls":"android.widget.Button","id":"android:id/aerr_wait","text":"Wait","b":[120,1220,960,1320],"f":"vVe","a":["tap"]},
                {"p":-1,"w":3,"cls":"android.widget.TextView","text":"Checkout","b":[0,0,1080,200],"f":"vVe"}]}"#,
        );
        let alerts = find(&tree, PHONE);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].0, 9);
        assert_eq!(alerts[0].1.package, "android");
        assert_eq!(alerts[0].1.title.as_deref(), Some("Shop isn't responding"));
    }

    #[test]
    fn system_ui_windows_without_a_dialog_layout_are_not_alerts() {
        let tree = android(
            r#"{"windows":[{"id":5,"type":"system","package":"com.android.systemui","bounds":[800,600,1080,1400]}],
                "nodes":[{"p":-1,"w":5,"cls":"android.widget.SeekBar","text":"Media volume","b":[800,600,1080,1400],"f":"vVe"}]}"#,
        );
        assert!(find(&tree, PHONE).is_empty());
    }

    #[test]
    fn a_system_ui_dialog_with_the_framework_layout_is_an_alert() {
        let tree = android(
            r#"{"windows":[{"id":5,"type":"system","package":"com.android.systemui","bounds":[100,900,980,1500]}],
                "nodes":[
                {"p":-1,"w":5,"cls":"android.widget.LinearLayout","id":"android:id/parentPanel","b":[100,900,980,1500],"f":"vVe"},
                {"p":0,"w":5,"cls":"android.widget.TextView","id":"android:id/alertTitle","text":"Allow USB debugging?","b":[120,920,960,1000],"f":"vVe"}]}"#,
        );
        let alerts = find(&tree, PHONE);
        assert_eq!(alerts[0].1.title.as_deref(), Some("Allow USB debugging?"));
    }

    #[test]
    fn a_window_marked_alert_by_the_ios_runner_is_one() {
        let mut tree = RawTree::from_ios_json(
            r#"{"nodes":[
            {"p":-1,"cls":"XCUIElementTypeAlert","text":"“Testbed” Would Like to Send You Notifications","b":[50,300,350,500]},
            {"p":0,"cls":"XCUIElementTypeStaticText","text":"“Testbed” Would Like to Send You Notifications","b":[60,310,340,350]},
            {"p":0,"cls":"XCUIElementTypeButton","text":"Allow","b":[60,420,340,460],"a":["tap"]}]}"#,
        )
        .unwrap();
        for n in &mut tree.nodes {
            n.window = -2;
        }
        tree.windows.push(RawWindow {
            id: -2,
            kind: "alert".into(),
            package: Some("com.apple.springboard".into()),
            layer: 2,
            bounds: crate::geometry::Rect {
                x: 50.0,
                y: 300.0,
                w: 300.0,
                h: 200.0,
            },
        });
        let alerts = find(&tree, Size { w: 402.0, h: 874.0 });
        assert_eq!(alerts[0].1.package, "com.apple.springboard");
        assert_eq!(
            alerts[0].1.title.as_deref(),
            Some("“Testbed” Would Like to Send You Notifications")
        );
    }
}
