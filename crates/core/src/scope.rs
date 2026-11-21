//! The app scope: the apps whose windows the agent may read and act in (decision D102).

use std::collections::{BTreeSet, HashMap};

use crate::alert;
use crate::geometry::{Rect, Size};
use crate::raw::{RawNode, RawTree, RawWindow};

/// Resource ids of the system's "open with" and share sheets. They list other apps and, on a share sheet, the people
/// a person shares with, so they are not the system dialogs the scope lets through.
const CHOOSER_IDS: &[&str] = &["android:id/resolver_list", "android:id/profile_tabhost"];
const CHOOSER_PACKAGES: &[&str] = &["com.android.intentresolver"];

/// What a header says in place of an app's name when the scope withheld its window.
pub const OUTSIDE_NAME: &str = "outside the app scope";

/// A window narrower or shorter than this share of the screen is an edge strip or a divider, not content.
const THIN: f32 = 0.05;
/// The most of the screen's height a status or navigation bar takes.
const BAR_HEIGHT: f32 = 0.1;
/// The package of Android's status and navigation bars. Another app's overlay window is reported to accessibility as
/// a system window too, so a bar's shape alone does not make it a bar.
pub const SYSTEM_UI: &str = "com.android.systemui";

/// A window the scope withheld: the package that owns it, when the platform said, and where it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Outside {
    pub package: Option<String>,
    pub bounds: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppScope {
    packages: BTreeSet<String>,
}

enum Standing {
    Shown,
    Bar,
    Outside,
}

impl AppScope {
    /// `Ok(None)` for no list at all. A list that names no package (`,`) is an error, so a typo does not turn the
    /// scope off.
    pub fn parse(list: &str) -> Result<Option<AppScope>, String> {
        if list.trim().is_empty() {
            return Ok(None);
        }
        let packages: BTreeSet<String> = list
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect();
        if packages.is_empty() {
            return Err(format!("{list:?} names no app"));
        }
        Ok(Some(AppScope { packages }))
    }

    pub fn allows(&self, package: &str) -> bool {
        self.packages.contains(package)
    }

    pub fn packages(&self) -> impl Iterator<Item = &str> {
        self.packages.iter().map(String::as_str)
    }

    /// The only package, when the scope names one.
    pub fn only(&self) -> Option<&str> {
        let mut all = self.packages();
        let first = all.next()?;
        all.next().is_none().then_some(first)
    }

    /// The list as it was set, in a fixed order, for comparing two scopes.
    pub fn list(&self) -> String {
        self.packages().collect::<Vec<_>>().join(",")
    }

    /// `tree` without what the scope withholds: the nodes of every window that belongs to an app outside it, and the
    /// status and navigation bars, whose notification icons carry text. The withheld windows that show something are
    /// listed in `outside`, and so is the whole screen when no window of an app in the scope is on it (an empty tree
    /// is also what a dark screen or a tree read too early looks like). `screen` is the display size in points.
    pub fn redact(&self, tree: RawTree, screen: Size) -> RawTree {
        let named = tree
            .windows
            .iter()
            .any(|w| w.package.as_deref().is_some_and(|p| self.allows(p)));
        let alerts: Vec<i32> = alert::find(&tree, screen)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        let mut outside = Vec::new();
        let mut shown: HashMap<i32, bool> = HashMap::new();
        for w in &tree.windows {
            let keep = match self.standing(&tree, w, screen, alerts.contains(&w.id)) {
                Standing::Shown => true,
                Standing::Bar => false,
                Standing::Outside => {
                    outside.push(Outside {
                        package: w.package.clone(),
                        bounds: w.bounds,
                    });
                    false
                }
            };
            shown.insert(w.id, keep);
        }
        let windows = tree.windows.into_iter().filter(|w| shown[&w.id]).collect();
        let mut at: Vec<Option<usize>> = Vec::with_capacity(tree.nodes.len());
        let mut nodes: Vec<RawNode> = Vec::new();
        let mut unlisted = false;
        for mut node in tree.nodes {
            let listed = shown.get(&node.window).copied();
            if listed.is_none() {
                unlisted = true;
            }
            let parent = match node.parent {
                Some(p) => at[p].map(Some),
                None => Some(None),
            };
            match (listed, parent) {
                (Some(true), Some(parent)) => {
                    node.parent = parent;
                    at.push(Some(nodes.len()));
                    nodes.push(node);
                }
                _ => at.push(None),
            }
        }
        if unlisted || (!named && outside.is_empty()) {
            outside.push(Outside {
                package: None,
                bounds: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: screen.w,
                    h: screen.h,
                },
            });
        }
        RawTree {
            windows,
            nodes,
            status_bar: tree.status_bar,
            outside,
        }
    }

    fn standing(&self, tree: &RawTree, w: &RawWindow, screen: Size, is_alert: bool) -> Standing {
        if w.kind == "input_method" {
            return Standing::Shown;
        }
        if w.package.as_deref().is_some_and(|p| self.allows(p)) {
            return Standing::Shown;
        }
        if is_alert && !is_chooser(tree, w) {
            return Standing::Shown;
        }
        if is_bar(w, screen) || is_thin(w, screen) {
            return Standing::Bar;
        }
        Standing::Outside
    }
}

fn is_chooser(tree: &RawTree, w: &RawWindow) -> bool {
    w.package
        .as_deref()
        .is_some_and(|p| CHOOSER_PACKAGES.contains(&p))
        || tree.nodes.iter().any(|n| {
            n.window == w.id
                && n.resource_id
                    .as_deref()
                    .is_some_and(|id| CHOOSER_IDS.contains(&id))
        })
}

fn is_bar(w: &RawWindow, screen: Size) -> bool {
    w.kind == "system"
        && w.package.as_deref() == Some(SYSTEM_UI)
        && w.bounds.w >= 0.98 * screen.w
        && w.bounds.h <= BAR_HEIGHT * screen.h
        && (w.bounds.y <= 0.5 || w.bounds.y + w.bounds.h >= screen.h - 0.5)
}

fn is_thin(w: &RawWindow, screen: Size) -> bool {
    w.kind != "application" && (w.bounds.w <= THIN * screen.w || w.bounds.h <= THIN * screen.h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Scale;

    const PHONE: Size = Size { w: 400.0, h: 800.0 };

    fn scope(list: &str) -> AppScope {
        AppScope::parse(list).unwrap().unwrap()
    }

    fn tree(windows: &str, nodes: &str) -> RawTree {
        RawTree::from_android_json(
            &format!(r#"{{"windows":[{windows}],"nodes":[{nodes}]}}"#),
            Scale { density: 1.0 },
        )
        .unwrap()
    }

    fn app_window(id: i32, package: &str) -> String {
        format!(
            r#"{{"id":{id},"type":"application","package":"{package}","layer":0,"bounds":[0,0,400,800]}}"#
        )
    }

    fn text(window: i32, parent: i32, label: &str) -> String {
        format!(
            r#"{{"p":{parent},"w":{window},"cls":"android.widget.TextView","text":"{label}","b":[0,0,100,50],"f":"vV"}}"#
        )
    }

    fn root(window: i32) -> String {
        format!(
            r#"{{"p":-1,"w":{window},"cls":"android.widget.FrameLayout","b":[0,0,400,800],"f":"vV"}}"#
        )
    }

    fn labels(t: &RawTree) -> Vec<&str> {
        t.nodes.iter().filter_map(|n| n.label.as_deref()).collect()
    }

    #[test]
    fn a_list_is_packages_split_on_commas_and_a_list_of_nothing_is_no_scope() {
        assert_eq!(AppScope::parse(""), Ok(None));
        assert_eq!(AppScope::parse("  "), Ok(None));
        let s = scope(" com.b.app , com.a.app,,");
        assert_eq!(s.list(), "com.a.app,com.b.app");
        assert!(s.allows("com.a.app") && !s.allows("com.c.app"));
        assert_eq!(s.only(), None);
        assert_eq!(scope("com.a.app").only(), Some("com.a.app"));
        assert!(AppScope::parse(",,").is_err());
    }

    #[test]
    fn the_text_of_a_window_outside_the_scope_is_gone_and_the_window_is_listed() {
        let t = tree(
            &format!(
                "{},{}",
                app_window(1, "com.mine"),
                app_window(2, "com.chat")
            ),
            &[
                root(1),
                text(1, 0, "Mine"),
                root(2),
                text(2, 2, "secret-chat"),
            ]
            .join(","),
        );
        let r = scope("com.mine").redact(t, PHONE);
        assert_eq!(labels(&r), ["Mine"]);
        assert_eq!(r.nodes[1].parent, Some(0));
        assert_eq!(r.windows.len(), 1);
        assert_eq!(r.outside.len(), 1);
        assert_eq!(r.outside[0].package.as_deref(), Some("com.chat"));
    }

    #[test]
    fn parents_follow_the_nodes_that_stay() {
        let t = tree(
            &format!(
                "{},{}",
                app_window(2, "com.chat"),
                app_window(1, "com.mine")
            ),
            &[
                root(2),
                text(2, 0, "secret"),
                root(1),
                text(1, 2, "Mine"),
                text(1, 2, "Also mine"),
            ]
            .join(","),
        );
        let r = scope("com.mine").redact(t, PHONE);
        assert_eq!(labels(&r), ["Mine", "Also mine"]);
        assert_eq!(r.nodes[0].parent, None);
        assert_eq!(r.nodes[1].parent, Some(0));
        assert_eq!(r.nodes[2].parent, Some(0));
    }

    #[test]
    fn the_keyboard_and_the_status_bar_are_not_outside() {
        let bar = r#"{"id":3,"type":"system","package":"com.android.systemui","layer":1,"bounds":[0,0,400,48]}"#;
        let ime = r#"{"id":2,"type":"input_method","package":"com.google.ime","layer":2,"bounds":[0,500,400,800]}"#;
        let t = tree(
            &format!("{},{ime},{bar}", app_window(1, "com.mine")),
            &[
                root(1),
                text(1, 0, "Mine"),
                root(2),
                root(3),
                text(3, 3, "WhatsApp: 3 new messages from Sam"),
            ]
            .join(","),
        );
        let r = scope("com.mine").redact(t, PHONE);
        assert!(r.outside.is_empty(), "{:?}", r.outside);
        assert_eq!(labels(&r), ["Mine"]);
        assert!(r.windows.iter().any(|w| w.kind == "input_method"));
        assert!(!r.windows.iter().any(|w| w.id == 3));
    }

    #[test]
    fn a_bar_shaped_window_is_a_bar_only_when_the_system_ui_owns_it() {
        let overlay = r#"{"id":3,"type":"system","package":"com.chat","layer":1,"bounds":[0,0,400,48]}"#;
        let t = tree(
            &format!("{},{overlay}", app_window(1, "com.mine")),
            &[root(1), root(3), text(3, 1, "your PIN is 4711")].join(","),
        );
        let r = scope("com.mine").redact(t, PHONE);
        assert_eq!(r.outside.len(), 1, "{:?}", r.outside);
        assert_eq!(r.outside[0].package.as_deref(), Some("com.chat"));
        assert!(labels(&r).is_empty());
    }

    #[test]
    fn a_tree_with_no_window_of_an_app_in_the_scope_is_outside() {
        let r = scope("com.mine").redact(tree("", ""), PHONE);
        assert_eq!(r.outside.len(), 1, "{:?}", r.outside);
        let ime = r#"{"id":2,"type":"input_method","package":"com.google.ime","layer":2,"bounds":[0,500,400,800]}"#;
        let r = scope("com.mine").redact(tree(ime, &root(2)), PHONE);
        assert_eq!(r.outside.len(), 1, "{:?}", r.outside);
        let r = scope("com.mine").redact(tree(&app_window(1, "com.mine"), &root(1)), PHONE);
        assert!(r.outside.is_empty(), "{:?}", r.outside);
    }

    #[test]
    fn a_heads_up_notification_of_another_app_is_outside_and_its_text_is_gone() {
        let heads_up = r#"{"id":3,"type":"system","package":"com.android.systemui","layer":1,"bounds":[10,40,390,140]}"#;
        let t = tree(
            &format!("{},{heads_up}", app_window(1, "com.mine")),
            &[root(1), root(3), text(3, 1, "secret-text")].join(","),
        );
        let r = scope("com.mine").redact(t, PHONE);
        assert!(labels(&r).is_empty());
        assert_eq!(
            r.outside[0].package.as_deref(),
            Some("com.android.systemui")
        );
    }

    #[test]
    fn a_thin_edge_window_is_not_content() {
        let strip =
            r#"{"id":2,"type":"system","package":"gesture","layer":4,"bounds":[0,40,18,780]}"#;
        let t = tree(
            &format!("{},{strip}", app_window(1, "com.mine")),
            &[root(1), root(2)].join(","),
        );
        assert!(scope("com.mine").redact(t, PHONE).outside.is_empty());
    }

    #[test]
    fn a_permission_dialog_is_let_through_and_a_share_sheet_is_not() {
        let dialog = |package: &str, extra: &str| {
            tree(
                &format!(
                    r#"{},{{"id":2,"type":"application","package":"{package}","layer":1,"bounds":[20,200,380,600]}}"#,
                    app_window(1, "com.mine")
                ),
                &[
                    root(1),
                    text(1, 0, "Mine"),
                    format!(
                        r#"{{"p":-1,"w":2,"id":"{extra}","cls":"android.widget.LinearLayout","b":[20,200,380,600],"f":"vV"}}"#
                    ),
                    r#"{"p":2,"w":2,"id":"com.android.permissioncontroller:id/permission_message","cls":"android.widget.TextView","text":"Allow Mine to use your location?","b":[30,210,370,260],"f":"vV"}"#.to_string(),
                ]
                .join(","),
            )
        };
        let r = scope("com.mine").redact(
            dialog("com.google.android.permissioncontroller", "x"),
            PHONE,
        );
        assert!(r.outside.is_empty(), "{:?}", r.outside);
        assert!(labels(&r).contains(&"Allow Mine to use your location?"));
        let r = scope("com.mine").redact(dialog("android", "android:id/resolver_list"), PHONE);
        assert_eq!(r.outside.len(), 1);
        assert!(!labels(&r).contains(&"Allow Mine to use your location?"));
        let r = scope("com.mine").redact(dialog("com.android.intentresolver", "x"), PHONE);
        assert_eq!(r.outside.len(), 1);
    }

    #[test]
    fn a_window_with_no_package_and_a_node_with_no_window_are_outside() {
        let nameless = r#"{"id":2,"type":"application","layer":1,"bounds":[0,0,400,800]}"#;
        let t = tree(
            &format!("{},{nameless}", app_window(1, "com.mine")),
            &[root(1), root(2), text(2, 1, "who knows")].join(","),
        );
        let r = scope("com.mine").redact(t, PHONE);
        assert_eq!(r.outside.len(), 1);
        assert_eq!(r.outside[0].package, None);
        assert!(labels(&r).is_empty());
        let t = tree(
            &app_window(1, "com.mine"),
            &[root(1), text(9, 0, "stray")].join(","),
        );
        let r = scope("com.mine").redact(t, PHONE);
        assert_eq!(r.outside.len(), 1);
        assert!(labels(&r).is_empty());
    }
}
