use std::fmt::Write;

use serde::{Deserialize, Serialize};

use crate::element::{Actions, Element, ElementState, Ref, Role, Source};
use crate::geometry::{Point, Rect, Size};
use crate::prune::Node;

const MAX_TEXT: usize = 200;
const MASK: &str = "•••";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceLine {
    pub id: String,
    pub model: String,
    pub os: String,
    pub screen: Size,
    pub orientation: Orientation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppLine {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SettleLine {
    pub settled: bool,
    pub ms: u32,
}

/// What the daemon knows about the screen besides its elements.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Header {
    pub device: Option<DeviceLine>,
    pub app: Option<AppLine>,
    /// Keyboard frame when it is shown.
    pub keyboard: Option<Rect>,
    pub settle: Option<SettleLine>,
    /// A secure window is on screen: its pixels are black but its tree is readable.
    #[serde(default)]
    pub secure_hidden: bool,
}

/// A screen ready to print: header plus elements with refs, in preorder.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub header: Header,
    pub elements: Vec<Element>,
    /// Index of each element's parent in `elements`.
    pub parents: Vec<Option<usize>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filters {
    pub interactive: bool,
    pub within: Option<Ref>,
    pub max_lines: Option<usize>,
}

/// The result of resolving a text target.
#[derive(Debug, Clone, PartialEq)]
pub enum TextMatch {
    None,
    One { r#ref: Ref, at: Point },
    Many(Vec<Ref>),
}

impl Rendered {
    pub fn build(nodes: Vec<Node>, refs: &[Ref], header: Header) -> Rendered {
        let parents: Vec<Option<usize>> = nodes.iter().map(|n| n.parent).collect();
        let mut elements: Vec<Element> = nodes
            .into_iter()
            .zip(refs)
            .map(|(n, &r)| Element {
                r#ref: r,
                role: n.role,
                label: n.label,
                value: n.value,
                hint: n.hint,
                id: n.id,
                bounds: n.bounds,
                state: n.state,
                actions: n.actions,
                position: n.position,
                collection: n.collection,
                source: Source::Tree,
                children: Vec::new(),
            })
            .collect();
        for (i, p) in parents.iter().enumerate() {
            if let Some(p) = *p {
                elements[p].children.push(i);
            }
        }
        Rendered {
            header,
            elements,
            parents,
        }
    }

    pub fn index_of(&self, r: Ref) -> Option<usize> {
        self.elements.iter().position(|e| e.r#ref == r)
    }

    /// The element is a keyboard or one of its keys (an iOS keyboard is part of the app's tree).
    fn in_keyboard(&self, r: Ref) -> bool {
        let mut at = self.index_of(r);
        while let Some(i) = at {
            if self.elements[i].role == Role::Keyboard {
                return true;
            }
            at = self.parents[i];
        }
        false
    }

    pub fn loading(&self) -> bool {
        self.elements
            .iter()
            .any(|e| e.role == Role::ProgressIndicator)
    }

    fn ancestors(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(self.parents[i], |&p| self.parents[p])
    }

    /// Prints the header and element lines. Fails with the ref when `within` names no element.
    pub fn text(&self, f: &Filters) -> Result<String, Ref> {
        let root = match f.within {
            Some(r) => Some(self.index_of(r).ok_or(r)?),
            None => None,
        };
        let shown: Vec<bool> = (0..self.elements.len())
            .map(|i| {
                let inside = root.is_none_or(|r| i == r || self.ancestors(i).any(|a| a == r));
                inside && (!f.interactive || !self.elements[i].actions.is_empty())
            })
            .collect();
        let mut out = self.header_text();
        let total = shown.iter().filter(|s| **s).count();
        let limit = f.max_lines.unwrap_or(usize::MAX);
        for (i, e) in self
            .elements
            .iter()
            .enumerate()
            .filter(|(i, _)| shown[*i])
            .take(limit)
        {
            let depth = self.ancestors(i).filter(|&a| shown[a]).count();
            out.push('\n');
            out.push_str(&"  ".repeat(depth));
            out.push_str(&self.line(e));
        }
        if total > limit {
            let _ = write!(out, "\n… {} more", total - limit);
        }
        Ok(out)
    }

    fn header_text(&self) -> String {
        let h = &self.header;
        let mut lines = Vec::new();
        if let Some(d) = &h.device {
            let orientation = match d.orientation {
                Orientation::Portrait => "portrait",
                Orientation::Landscape => "landscape",
            };
            lines.push(format!(
                "device {} · {} · {} · {:.0}x{:.0}pt · {orientation}",
                d.id, d.model, d.os, d.screen.w, d.screen.h
            ));
        }
        let mut parts = Vec::new();
        if let Some(a) = &h.app {
            parts.push(match &a.name {
                Some(n) => format!("app {} ({n})", a.id),
                None => format!("app {}", a.id),
            });
        }
        parts.push(
            if h.keyboard.is_some() {
                "keyboard shown"
            } else {
                "keyboard hidden"
            }
            .into(),
        );
        if let Some(s) = h.settle {
            parts.push(if s.settled {
                format!("settled in {}ms", s.ms)
            } else {
                format!("not settled after {}ms", s.ms)
            });
        }
        if self.loading() {
            parts.push("loading".into());
        }
        if h.secure_hidden {
            parts.push("secure content hidden".into());
        }
        lines.push(parts.join(" · "));
        lines.join("\n")
    }

    /// One element line: `@eN role "label" ["value"] [state…]`.
    pub fn line(&self, e: &Element) -> String {
        let mut s = format!("@{} {}", e.r#ref, e.role.name());
        if let Some(l) = &e.label {
            let _ = write!(s, " {}", quote(l));
        }
        if e.role == Role::List
            && let Some(c) = e.collection
        {
            let _ = match (c.rows, c.cols) {
                (rows, 0 | 1) => write!(s, " {rows} rows"),
                (1, cols) => write!(s, " {cols} columns"),
                (rows, cols) => write!(s, " {rows}x{cols} cells"),
            };
        }
        if let Some(v) = second_text(e) {
            let _ = write!(s, " {v}");
        }
        for w in state_words(e) {
            s.push(' ');
            s.push_str(w);
        }
        if let Some(k) = self.header.keyboard
            && k.contains(e.bounds)
            && !self.in_keyboard(e.r#ref)
        {
            s.push_str(" under-keyboard");
        }
        s
    }

    /// Resolves a text target to the element an action should hit (architecture 4.4).
    pub fn resolve_text(&self, text: &str, role: Option<Role>) -> TextMatch {
        let mut hits: Vec<usize> = Vec::new();
        for (i, e) in self.elements.iter().enumerate() {
            let in_hint = e
                .hint
                .as_deref()
                .is_some_and(|h| h.split(" · ").any(|p| p == text));
            if e.label.as_deref() != Some(text) && !in_hint {
                continue;
            }
            let target = if pressable(e) {
                i
            } else {
                self.ancestors(i)
                    .find(|&a| pressable(&self.elements[a]))
                    .unwrap_or(i)
            };
            if role.is_none_or(|r| self.elements[target].role == r) && !hits.contains(&target) {
                hits.push(target);
            }
        }
        match hits.as_slice() {
            [] => TextMatch::None,
            [i] => {
                let e = &self.elements[*i];
                TextMatch::One {
                    r#ref: e.r#ref,
                    at: e.bounds.center(),
                }
            }
            many => TextMatch::Many(many.iter().map(|&i| self.elements[i].r#ref).collect()),
        }
    }
}

fn pressable(e: &Element) -> bool {
    e.actions.intersects(Actions::TAP | Actions::LONG_PRESS)
}

/// The value as shown: masked for secure fields, omitted when the state word already says it.
pub fn shown_value(e: &Element) -> Option<String> {
    let v = e.value.as_deref()?;
    if e.role == Role::SecureField || e.state.contains(ElementState::PASSWORD) {
        return Some(MASK.into());
    }
    let redundant = e.state.contains(ElementState::CHECKABLE)
        && matches!(
            v.to_lowercase().as_str(),
            "on" | "off" | "checked" | "not checked" | "0" | "1"
        );
    (!redundant).then(|| v.to_string())
}

fn second_text(e: &Element) -> Option<String> {
    shown_value(e).or_else(|| e.hint.clone()).map(|t| quote(&t))
}

pub fn quote(s: &str) -> String {
    let n = s.chars().count();
    let body = if n > MAX_TEXT {
        let head: String = s.chars().take(MAX_TEXT).collect();
        format!("{head}…(+{} chars)", n - MAX_TEXT)
    } else {
        s.to_string()
    };
    format!(
        "\"{}\"",
        body.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

pub fn state_words(e: &Element) -> Vec<&'static str> {
    let mut w = Vec::new();
    let st = e.state;
    if st.contains(ElementState::CHECKABLE) {
        w.push(if st.contains(ElementState::CHECKED) {
            "on"
        } else {
            "off"
        });
    }
    if !st.contains(ElementState::ENABLED) {
        w.push("disabled");
    }
    if st.contains(ElementState::FOCUSED) {
        w.push("focused");
    }
    if st.contains(ElementState::SELECTED) {
        w.push("selected");
    }
    if st.contains(ElementState::EXPANDED) {
        w.push("expanded");
    }
    if st.contains(ElementState::COLLAPSED) {
        w.push("collapsed");
    }
    if e.role != Role::Slider {
        let a = e.actions;
        let vertical = match (
            a.contains(Actions::SCROLL_UP),
            a.contains(Actions::SCROLL_DOWN),
        ) {
            (true, true) => Some("↑↓"),
            (false, true) => Some("↓"),
            (true, false) => Some("↑"),
            _ => None,
        };
        let horizontal = match (
            a.contains(Actions::SCROLL_LEFT),
            a.contains(Actions::SCROLL_RIGHT),
        ) {
            (true, true) => Some("←→"),
            (false, true) => Some("→"),
            (true, false) => Some("←"),
            _ => None,
        };
        w.push(match (vertical, horizontal) {
            (Some("↑↓"), Some("←→")) => "scrollable↑↓←→",
            (Some("↓"), None) => "scrollable↓",
            (Some("↑"), None) => "scrollable↑",
            (Some("↑↓"), None) => "scrollable↑↓",
            (None, Some("→")) => "scrollable→",
            (None, Some("←")) => "scrollable←",
            (None, Some("←→")) => "scrollable←→",
            (Some(_), Some(_)) => "scrollable",
            _ => "",
        });
        w.retain(|s| !s.is_empty());
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::ItemPos;
    use crate::geometry::{Point, Scale, Size};
    use crate::prune::prune;
    use crate::raw::RawTree;
    use crate::refs::RefTable;

    const FIXTURE: &str = include_str!("../tests/fixtures/android/handmade_settings.json");

    fn header() -> Header {
        Header {
            device: Some(DeviceLine {
                id: "android:emulator-5554".into(),
                model: "Pixel 9".into(),
                os: "Android 17".into(),
                screen: Size { w: 360.0, h: 800.0 },
                orientation: Orientation::Portrait,
            }),
            app: Some(AppLine {
                id: "com.android.settings".into(),
                name: Some("Settings".into()),
            }),
            keyboard: None,
            settle: Some(SettleLine {
                settled: true,
                ms: 212,
            }),
            secure_hidden: false,
        }
    }

    fn build_json(json: &str, density: f32) -> Rendered {
        let tree = RawTree::from_android_json(json, Scale { density }).unwrap();
        let nodes = prune(&tree);
        let refs = RefTable::default().assign(&tree, &nodes);
        Rendered::build(nodes, &refs, header())
    }

    fn settings() -> Rendered {
        build_json(FIXTURE, 3.0)
    }

    fn one(json_nodes: &str) -> Rendered {
        let json = format!(
            r#"{{"windows":[{{"id":1,"type":"application","bounds":[0,0,400,800]}}],"nodes":[{json_nodes}]}}"#
        );
        build_json(&json, 1.0)
    }

    fn real(json: &str) -> String {
        build_json(json, 2.625).text(&Filters::default()).unwrap()
    }

    fn captured(json: &str) -> Rendered {
        let density = serde_json::from_str::<serde_json::Value>(json).unwrap()["density"]
            .as_f64()
            .unwrap() as f32;
        build_json(json, density)
    }

    fn settings_main() -> Rendered {
        captured(include_str!("../tests/fixtures/android/settings_main.json"))
    }

    fn golden(name: &str, fixture: &str) {
        let text = captured(fixture).text(&Filters::default()).unwrap();
        insta::assert_snapshot!(name, text);
    }

    #[test]
    fn keys_of_an_ios_keyboard_are_not_under_the_keyboard() {
        let tree = RawTree::from_ios_json(
            r#"{"app":{"bundle_id":"x","pid":1},"nodes":[
            {"p":-1,"cls":"XCUIElementTypeApplication","text":"App","b":[0,0,400,800],"f":"vVe"},
            {"p":0,"cls":"XCUIElementTypeButton","text":"Hidden","a":["tap"],"b":[0,600,100,640],"f":"vVe"},
            {"p":0,"cls":"XCUIElementTypeKeyboard","b":[0,500,400,800],"f":"vVe"},
            {"p":2,"cls":"XCUIElementTypeKey","text":"Q","a":["tap"],"b":[0,520,40,560],"f":"vVe"}
        ]}"#,
        )
        .unwrap();
        let nodes = prune(&tree);
        let refs = RefTable::default().assign(&tree, &nodes);
        let mut h = header();
        h.keyboard = Some(Rect {
            x: 0.0,
            y: 500.0,
            w: 400.0,
            h: 300.0,
        });
        let text = Rendered::build(nodes, &refs, h)
            .text(&Filters::default())
            .unwrap();
        assert!(text.contains("button \"Hidden\" under-keyboard"), "{text}");
        assert!(text.contains("keyboard"), "{text}");
        assert!(
            text.contains("button \"Q\"\n") || text.ends_with("button \"Q\""),
            "{text}"
        );
    }

    #[test]
    fn captured_settings_apps() {
        golden(
            "captured_settings_apps",
            include_str!("../tests/fixtures/android/settings_apps.json"),
        );
    }

    #[test]
    fn captured_testbed_taps() {
        golden(
            "captured_testbed_taps",
            include_str!("../tests/fixtures/android/testbed_taps.json"),
        );
    }

    #[test]
    fn captured_testbed_text() {
        golden(
            "captured_testbed_text",
            include_str!("../tests/fixtures/android/testbed_text.json"),
        );
    }

    #[test]
    fn captured_testbed_list() {
        golden(
            "captured_testbed_list",
            include_str!("../tests/fixtures/android/testbed_list.json"),
        );
    }

    #[test]
    fn captured_testbed_busy() {
        golden(
            "captured_testbed_busy",
            include_str!("../tests/fixtures/android/testbed_busy.json"),
        );
    }

    #[test]
    fn captured_testbed_secure() {
        golden(
            "captured_testbed_secure",
            include_str!("../tests/fixtures/android/testbed_secure.json"),
        );
    }

    #[test]
    fn real_settings_main() {
        insta::assert_snapshot!(
            "real_settings_main",
            real(include_str!(
                "../tests/fixtures/android/real_settings_main.json"
            ))
        );
    }

    #[test]
    fn real_settings_apps() {
        insta::assert_snapshot!(
            "real_settings_apps",
            real(include_str!(
                "../tests/fixtures/android/real_settings_apps.json"
            ))
        );
    }

    #[test]
    fn real_dialog() {
        insta::assert_snapshot!(
            "real_dialog",
            real(include_str!("../tests/fixtures/android/real_dialog.json"))
        );
    }

    #[test]
    fn a_secure_window_adds_secure_content_hidden_to_the_header() {
        let mut r = settings();
        r.header.secure_hidden = true;
        let text = r.text(&Filters::default()).unwrap();
        assert!(
            text.lines()
                .nth(1)
                .unwrap()
                .ends_with("settled in 212ms · secure content hidden"),
            "{text}"
        );
        r.header.secure_hidden = false;
        assert!(!r.text(&Filters::default()).unwrap().contains("secure"));
    }

    #[test]
    fn the_screen_size_prints_in_whole_points() {
        let mut r = settings();
        r.header.device.as_mut().unwrap().screen = Size {
            w: 411.42856,
            h: 923.4286,
        };
        let text = r.text(&Filters::default()).unwrap();
        assert!(text.contains("· 411x923pt ·"), "{text}");
    }

    #[test]
    fn golden_default() {
        insta::assert_snapshot!(
            "settings_default",
            settings_main().text(&Filters::default()).unwrap()
        );
    }

    #[test]
    fn golden_interactive() {
        let f = Filters {
            interactive: true,
            ..Filters::default()
        };
        insta::assert_snapshot!("settings_interactive", settings_main().text(&f).unwrap());
    }

    #[test]
    fn golden_within() {
        let f = Filters {
            within: Some(Ref(2)),
            ..Filters::default()
        };
        insta::assert_snapshot!("settings_within_e2", settings_main().text(&f).unwrap());
    }

    #[test]
    fn golden_max_lines() {
        let f = Filters {
            max_lines: Some(3),
            ..Filters::default()
        };
        insta::assert_snapshot!("settings_max_lines_3", settings_main().text(&f).unwrap());
    }

    #[test]
    fn within_unknown_ref_is_an_error() {
        let f = Filters {
            within: Some(Ref(99)),
            ..Filters::default()
        };
        assert_eq!(settings().text(&f).unwrap_err(), Ref(99));
    }

    #[test]
    fn rows_render_as_one_merged_line_and_switch_separately() {
        let text = settings().text(&Filters::default()).unwrap();
        assert!(
            text.contains("\n  @e4 button \"Network & internet\" \"Mobile, Wi‑Fi, hotspot\"\n"),
            "{text}"
        );
        assert!(
            text.contains("\n    @e8 switch \"Dark theme\" on\n"),
            "{text}"
        );
        assert_eq!(text.matches("Network & internet").count(), 1);
    }

    #[test]
    fn text_resolves_to_actionable_row() {
        let r = settings();
        let row = r.resolve_text("Network & internet", None);
        assert_eq!(
            row,
            TextMatch::One {
                r#ref: Ref(4),
                at: Point { x: 180.0, y: 184.0 }
            }
        );
        assert_eq!(
            r.resolve_text("Mobile, Wi‑Fi, hotspot", None),
            row,
            "subtitle text resolves to its row"
        );
        assert!(
            matches!(r.resolve_text("Dark theme", None), TextMatch::Many(refs) if refs == [Ref(7), Ref(8)])
        );
        assert!(matches!(
            r.resolve_text("Dark theme", Some(Role::Switch)),
            TextMatch::One { r#ref: Ref(8), .. }
        ));
        assert_eq!(r.resolve_text("Nope", None), TextMatch::None);
    }

    #[test]
    fn inert_text_without_actionable_ancestor_resolves_to_its_own_center() {
        let r = settings();
        assert_eq!(
            r.resolve_text("Settings", None),
            TextMatch::One {
                r#ref: Ref(1),
                at: Point { x: 180.0, y: 60.0 }
            }
        );
    }

    #[test]
    fn inert_text_inside_labeled_button_resolves_to_button() {
        let r = one(
            r#"{"p":-1,"w":1,"cls":"android.widget.Button","desc":"Buy","b":[0,0,400,100],"f":"vVe","a":["tap"]},
               {"p":0,"w":1,"cls":"android.widget.TextView","text":"$4.99","b":[10,10,100,40],"f":"vVe"}"#,
        );
        assert_eq!(
            r.resolve_text("$4.99", None),
            TextMatch::One {
                r#ref: Ref(1),
                at: Point { x: 200.0, y: 50.0 }
            }
        );
    }

    #[test]
    fn long_labels_truncate() {
        let label = "x".repeat(300);
        let r = one(&format!(
            r#"{{"p":-1,"w":1,"cls":"android.widget.TextView","text":"{label}","b":[0,0,400,100],"f":"vVe"}}"#
        ));
        let text = r.text(&Filters::default()).unwrap();
        assert!(
            text.contains(&format!("@e1 text \"{}…(+100 chars)\"", "x".repeat(200))),
            "{text}"
        );
    }

    #[test]
    fn secure_values_are_masked() {
        let r = one(
            r#"{"p":-1,"w":1,"cls":"android.widget.EditText","text":"hunter2","desc":"Password","b":[0,0,400,100],"f":"vVeEP","a":["set_text"]}"#,
        );
        let text = r.text(&Filters::default()).unwrap();
        assert!(text.contains("@e1 password \"Password\" \"•••\""), "{text}");
        assert!(!text.contains("hunter2"));
    }

    #[test]
    fn single_child_unlabeled_container_collapses() {
        let r = one(
            r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,400,800],"f":"vVe"},
               {"p":0,"w":1,"cls":"android.widget.LinearLayout","b":[0,0,400,800],"f":"vVe"},
               {"p":1,"w":1,"cls":"android.widget.Button","text":"OK","b":[0,0,100,50],"f":"vVe","a":["tap"]}"#,
        );
        let text = r.text(&Filters::default()).unwrap();
        assert!(text.ends_with("\n@e1 button \"OK\""), "{text}");
    }

    #[test]
    fn zero_area_nodes_are_dropped() {
        let r = one(
            r#"{"p":-1,"w":1,"cls":"android.widget.Button","text":"Ghost","b":[10,10,10,60],"f":"vVe","a":["tap"]}"#,
        );
        assert!(r.elements.is_empty());
    }

    #[test]
    fn visible_progress_indicator_marks_header_loading() {
        let r = one(
            r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,400,800],"f":"vVe"},
               {"p":0,"w":1,"cls":"android.widget.ProgressBar","b":[180,380,220,420],"f":"vVe"}"#,
        );
        let text = r.text(&Filters::default()).unwrap();
        assert!(
            text.lines().nth(1).unwrap().ends_with(" · loading"),
            "{text}"
        );
        assert!(
            !settings()
                .text(&Filters::default())
                .unwrap()
                .contains("loading")
        );
    }

    #[test]
    fn scroll_words() {
        let words = |a: &str| {
            let r = one(&format!(
                r#"{{"p":-1,"w":1,"cls":"android.widget.ListView","b":[0,0,400,800],"f":"vVeS","a":[{a}]}}"#
            ));
            r.text(&Filters::default())
                .unwrap()
                .lines()
                .last()
                .unwrap()
                .to_string()
        };
        assert_eq!(words(r#""scroll_down""#), "@e1 list scrollable↓");
        assert_eq!(
            words(r#""scroll_up","scroll_down""#),
            "@e1 list scrollable↑↓"
        );
        assert_eq!(words(r#""scroll_right""#), "@e1 list scrollable→");
    }

    #[test]
    fn a_list_line_says_how_many_rows_it_has_and_an_inert_item_is_a_cell() {
        let r = one(
            r#"{"p":-1,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,0,400,800],"f":"vVeS","a":["scroll_down"],"coll":{"rows":1000,"cols":1}},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"Row 17","b":[0,0,400,40],"f":"vVe","item":{"row":17,"col":0}}"#,
        );
        let text = r.text(&Filters::default()).unwrap();
        assert!(text.contains("@e1 list 1000 rows scrollable↓"), "{text}");
        assert!(text.contains("@e2 cell \"Row 17\""), "{text}");
        assert_eq!(r.elements[1].position, Some(ItemPos { row: 17, col: 0 }));
    }

    #[test]
    fn a_text_row_without_item_info_is_still_a_cell_of_its_list() {
        let r = one(
            r#"{"p":-1,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,0,400,800],"f":"vVeS","a":["scroll_down"],"coll":{"rows":1000,"cols":1}},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"Row 17","b":[0,0,400,40],"f":"vVe"}"#,
        );
        assert_eq!(r.elements[1].role, Role::Cell);
        assert_eq!(r.elements[1].position, None);
    }

    #[test]
    fn a_clickable_item_stays_a_button() {
        let r = one(
            r#"{"p":-1,"w":1,"cls":"android.view.ViewGroup","text":"Wi-Fi","b":[0,0,400,40],"f":"vVe","a":["tap"],"item":{"row":1,"col":0}}"#,
        );
        assert_eq!(r.elements[0].role, Role::Button);
    }
}
