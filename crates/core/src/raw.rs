use bitflags::bitflags;
use serde::{Deserialize, Serialize};

use crate::element::{Actions, ItemPos, Role};
use crate::geometry::{Rect, Scale};

/// A platform-neutral UI tree: nodes in preorder, roles mapped, bounds in points.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RawTree {
    pub windows: Vec<RawWindow>,
    pub nodes: Vec<RawNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawWindow {
    pub id: i32,
    pub kind: String,
    pub package: Option<String>,
    pub layer: i32,
    pub bounds: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawNode {
    pub parent: Option<usize>,
    pub window: i32,
    pub resource_id: Option<String>,
    pub class: String,
    pub role: Role,
    pub label: Option<String>,
    pub value: Option<String>,
    pub hint: Option<String>,
    pub bounds: Rect,
    pub flags: NodeFlags,
    pub actions: Actions,
    pub range: Option<Range>,
    pub collection: Option<Collection>,
    pub item: Option<ItemPos>,
    /// Android `getSourceNodeId`; `None` on iOS and when reflection failed.
    pub source_id: Option<String>,
}

bitflags! {
    /// The `f` letters of the device wire (`docs/protocol.md` section 2).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct NodeFlags: u16 {
        const VISIBLE = 1;
        const VISIBLE_TO_USER = 1 << 1;
        const CHECKABLE = 1 << 2;
        const CHECKED = 1 << 3;
        const ENABLED = 1 << 4;
        const EDITABLE = 1 << 5;
        const FOCUSED = 1 << 6;
        const SELECTED = 1 << 7;
        const PASSWORD = 1 << 8;
        const SCROLLABLE = 1 << 9;
        const HEADING = 1 << 10;
        const EXPANDED = 1 << 11;
        const COLLAPSED = 1 << 12;
        const SHOWING_HINT = 1 << 13;
    }
}

impl NodeFlags {
    fn from_letter(c: char) -> Option<NodeFlags> {
        Some(match c {
            'v' => NodeFlags::VISIBLE,
            'V' => NodeFlags::VISIBLE_TO_USER,
            'c' => NodeFlags::CHECKABLE,
            'C' => NodeFlags::CHECKED,
            'e' => NodeFlags::ENABLED,
            'E' => NodeFlags::EDITABLE,
            'F' => NodeFlags::FOCUSED,
            's' => NodeFlags::SELECTED,
            'P' => NodeFlags::PASSWORD,
            'S' => NodeFlags::SCROLLABLE,
            'H' => NodeFlags::HEADING,
            'X' => NodeFlags::EXPANDED,
            'x' => NodeFlags::COLLAPSED,
            'h' => NodeFlags::SHOWING_HINT,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Range {
    pub min: f32,
    pub max: f32,
    pub cur: f32,
    #[serde(rename = "type")]
    pub kind: RangeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RangeKind {
    Int,
    Float,
    Percent,
}

impl Range {
    fn display(&self) -> String {
        match self.kind {
            RangeKind::Int => format!("{}", self.cur.round() as i64),
            RangeKind::Percent => format!("{}%", self.cur.round() as i64),
            RangeKind::Float => format!("{}", self.cur),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    pub rows: u32,
    pub cols: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("tree is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unknown flag letter '{0}'")]
    UnknownFlag(char),
    #[error("unknown action {0:?}")]
    UnknownAction(String),
    #[error("node {index} names parent {parent}, which does not precede it")]
    BadParent { index: usize, parent: i64 },
}

#[derive(Deserialize)]
struct WireTree {
    #[serde(default)]
    windows: Vec<WireWindow>,
    nodes: Vec<WireNode>,
}

#[derive(Deserialize)]
struct WireWindow {
    id: i32,
    #[serde(rename = "type")]
    kind: String,
    package: Option<String>,
    #[serde(default)]
    layer: i32,
    bounds: [f32; 4],
}

#[derive(Deserialize)]
struct WireNode {
    p: i64,
    #[serde(default)]
    w: i32,
    id: Option<String>,
    cls: String,
    text: Option<String>,
    desc: Option<String>,
    hint: Option<String>,
    sd: Option<String>,
    rd: Option<String>,
    b: [f32; 4],
    #[serde(default)]
    f: String,
    #[serde(default)]
    a: Vec<String>,
    range: Option<Range>,
    coll: Option<Collection>,
    item: Option<ItemPos>,
    n: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Platform {
    Android,
    Ios,
}

impl RawTree {
    /// A progress indicator is visible, or a web view has no children yet.
    pub fn loading(&self) -> bool {
        self.web_view_pending() || self.progress_visible()
    }

    /// A web view has no children yet: its page has not reached the accessibility tree.
    pub fn web_view_pending(&self) -> bool {
        self.nodes.iter().enumerate().any(|(i, n)| {
            n.role == Role::WebView && !self.nodes.iter().any(|c| c.parent == Some(i))
        })
    }

    fn progress_visible(&self) -> bool {
        self.nodes.iter().any(|n| {
            n.role == Role::ProgressIndicator && n.flags.contains(NodeFlags::VISIBLE_TO_USER)
        })
    }

    /// Parses an Android agent `tree` reply. Bounds arrive in pixels and leave in points.
    pub fn from_android_json(json: &str, scale: Scale) -> Result<RawTree, ParseError> {
        let wire: WireTree = serde_json::from_str(json)?;
        Self::from_wire(wire, scale, Platform::Android)
    }

    /// Parses a runner `snapshot` reply. Bounds are already points.
    pub fn from_ios_json(json: &str) -> Result<RawTree, ParseError> {
        let wire: WireTree = serde_json::from_str(json)?;
        Self::from_wire(wire, Scale { density: 1.0 }, Platform::Ios)
    }

    fn from_wire(wire: WireTree, scale: Scale, platform: Platform) -> Result<RawTree, ParseError> {
        let windows = wire
            .windows
            .into_iter()
            .map(|w| RawWindow {
                id: w.id,
                kind: w.kind,
                package: w.package,
                layer: w.layer,
                bounds: ltrb(w.bounds, scale),
            })
            .collect();
        let mut wire_nodes = wire.nodes;
        // Compose puts a toggle's role description on a child of the node that is checkable and takes the tap.
        for i in 0..wire_nodes.len() {
            let Some(rd) = wire_nodes[i].rd.clone() else {
                continue;
            };
            if let Ok(p) = usize::try_from(wire_nodes[i].p)
                && p < i
                && wire_nodes[p].rd.is_none()
                && wire_nodes[p].f.contains('c')
            {
                wire_nodes[p].rd = Some(rd);
            }
        }
        let mut nodes: Vec<RawNode> = wire_nodes
            .into_iter()
            .enumerate()
            .map(|(i, n)| node(i, n, scale, platform))
            .collect::<Result<_, _>>()?;
        // RecyclerView attaches collection item info only to rows bound while an accessibility service was
        // connected, so rows created before the agent connected carry none: a text row of a list is a cell either way.
        for i in 0..nodes.len() {
            let in_list = nodes[i].parent.is_some_and(|p| nodes[p].role == Role::List);
            let pressable = nodes[i]
                .actions
                .intersects(Actions::TAP | Actions::LONG_PRESS);
            if in_list && !pressable && nodes[i].role == Role::Text {
                nodes[i].role = Role::Cell;
            }
        }
        Ok(RawTree { windows, nodes })
    }

    pub fn children(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        self.nodes
            .iter()
            .enumerate()
            .skip(index + 1)
            .filter(move |(_, n)| n.parent == Some(index))
            .map(|(i, _)| i)
    }
}

fn ltrb(b: [f32; 4], scale: Scale) -> Rect {
    let d = scale.density;
    Rect {
        x: b[0] / d,
        y: b[1] / d,
        w: (b[2] - b[0]) / d,
        h: (b[3] - b[1]) / d,
    }
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|s| !s.is_empty())
}

fn node(
    index: usize,
    n: WireNode,
    scale: Scale,
    platform: Platform,
) -> Result<RawNode, ParseError> {
    let parent = match n.p {
        -1 => None,
        p if p >= 0 && (p as usize) < index => Some(p as usize),
        p => return Err(ParseError::BadParent { index, parent: p }),
    };
    let mut flags = NodeFlags::empty();
    for c in n.f.chars() {
        flags |= NodeFlags::from_letter(c).ok_or(ParseError::UnknownFlag(c))?;
    }
    let horizontal = n.coll.is_some_and(|c| c.cols > c.rows);
    let mut actions = Actions::empty();
    for a in &n.a {
        actions |= match a.as_str() {
            "tap" => Actions::TAP,
            "long_tap" => Actions::LONG_PRESS,
            "set_text" => Actions::EDIT,
            "scroll_up" => Actions::SCROLL_UP,
            "scroll_down" => Actions::SCROLL_DOWN,
            "scroll_left" => Actions::SCROLL_LEFT,
            "scroll_right" => Actions::SCROLL_RIGHT,
            "scroll_fwd" if horizontal => Actions::SCROLL_RIGHT,
            "scroll_fwd" => Actions::SCROLL_DOWN,
            "scroll_back" if horizontal => Actions::SCROLL_LEFT,
            "scroll_back" => Actions::SCROLL_UP,
            other => return Err(ParseError::UnknownAction(other.to_string())),
        };
    }

    let mut role = match platform {
        Platform::Android => Role::from_android_class(&n.cls),
        Platform::Ios => Role::from_xcui(&n.cls),
    };
    if role == Role::Other && n.coll.is_some_and(|c| c.rows > 0 || c.cols > 0) {
        role = Role::List;
    }
    let pressable = actions.intersects(Actions::TAP | Actions::LONG_PRESS);
    if n.item.is_some() && !pressable && matches!(role, Role::Text | Role::Other) {
        role = Role::Cell;
    }
    if flags.contains(NodeFlags::PASSWORD) && matches!(role, Role::TextField | Role::Other) {
        role = Role::SecureField;
    }
    if flags.contains(NodeFlags::HEADING) && matches!(role, Role::Text | Role::Other) {
        role = Role::Heading;
    }
    if role == Role::Other
        && let Some(r) = n.rd.as_deref().and_then(Role::from_role_description)
    {
        role = r;
    }

    let text = non_empty(n.text);
    let desc = non_empty(n.desc);
    let state = non_empty(n.sd);
    let range_text = n.range.map(|r| r.display());
    let (label, value) = match platform {
        Platform::Android => {
            let editable = flags.contains(NodeFlags::EDITABLE);
            let shows_hint = flags.contains(NodeFlags::SHOWING_HINT);
            let label = desc.or_else(|| text.clone().filter(|_| !editable));
            let field_text = text.filter(|_| editable && !shows_hint);
            (label, state.or(range_text).or(field_text))
        }
        Platform::Ios => (text.or(desc), state.or(range_text)),
    };

    Ok(RawNode {
        parent,
        window: n.w,
        resource_id: non_empty(n.id),
        class: n.cls,
        role,
        label,
        value,
        hint: non_empty(n.hint),
        bounds: ltrb(n.b, scale),
        flags,
        actions,
        range: n.range,
        collection: n.coll,
        item: n.item,
        source_id: non_empty(n.n),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{Actions, Role};
    use crate::geometry::{Rect, Scale};

    const FIXTURE: &str = include_str!("../tests/fixtures/android/handmade_settings.json");

    fn settings() -> RawTree {
        RawTree::from_android_json(FIXTURE, Scale { density: 3.0 }).unwrap()
    }

    #[test]
    fn a_compose_view_with_a_role_description_takes_that_role() {
        let json = r#"{"windows":[],"nodes":[
            {"p":-1,"w":1,"cls":"android.view.View","id":"compose_switch","sd":"Off","b":[0,437,137,563],"f":"vVce","a":["tap"]},
            {"p":0,"w":1,"cls":"android.view.View","rd":"Switch","b":[0,458,137,542],"f":"vVe"},
            {"p":-1,"w":1,"cls":"android.view.View","id":"plain","b":[0,0,10,10],"f":"vVe","a":["tap"]}
        ]}"#;
        let t = RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap();
        assert_eq!(t.nodes[0].role, Role::Switch);
        assert_eq!(t.nodes[0].value.as_deref(), Some("Off"));
        assert_eq!(t.nodes[2].role, Role::Other);
    }

    #[test]
    fn parses_handmade_settings() {
        let t = settings();
        assert_eq!(t.nodes.len(), 18);
        assert_eq!(t.windows.len(), 1);
        let w = &t.windows[0];
        assert_eq!(
            (w.id, w.kind.as_str(), w.package.as_deref(), w.layer),
            (12, "application", Some("com.android.settings"), 0)
        );
        assert_eq!(
            w.bounds,
            Rect {
                x: 0.0,
                y: 0.0,
                w: 360.0,
                h: 800.0
            }
        );

        let parents: Vec<Option<usize>> = t.nodes.iter().map(|n| n.parent).collect();
        assert_eq!(
            parents,
            [
                None,
                Some(0),
                Some(0),
                Some(0),
                Some(3),
                Some(4),
                Some(4),
                Some(3),
                Some(7),
                Some(7),
                Some(3),
                Some(10),
                Some(10),
                Some(3),
                Some(13),
                Some(13),
                Some(13),
                Some(0)
            ]
            .to_vec()
        );
        assert_eq!(t.children(3).collect::<Vec<_>>(), [4, 7, 10, 13]);
    }

    #[test]
    fn decodes_every_key() {
        let t = settings();
        let heading = &t.nodes[1];
        assert_eq!(heading.role, Role::Heading);
        assert_eq!(heading.label.as_deref(), Some("Settings"));
        assert!(
            heading
                .flags
                .contains(NodeFlags::HEADING | NodeFlags::VISIBLE_TO_USER)
        );

        let search = &t.nodes[2];
        assert_eq!(search.role, Role::SearchField);
        assert_eq!(
            search.resource_id.as_deref(),
            Some("com.android.settings:id/search_src_text")
        );
        assert_eq!(search.label, None, "text shown as hint is not a label");
        assert_eq!(search.value, None);
        assert_eq!(search.hint.as_deref(), Some("Search settings"));
        assert_eq!(search.actions, Actions::TAP | Actions::EDIT);
        assert!(
            search
                .flags
                .contains(NodeFlags::EDITABLE | NodeFlags::SHOWING_HINT)
        );

        let list = &t.nodes[3];
        assert_eq!(list.role, Role::List);
        assert_eq!(list.class, "androidx.recyclerview.widget.RecyclerView");
        assert_eq!(list.actions, Actions::SCROLL_DOWN);
        assert_eq!(list.collection, Some(Collection { rows: 12, cols: 1 }));
        assert_eq!(
            list.bounds,
            Rect {
                x: 0.0,
                y: 144.0,
                w: 360.0,
                h: 576.0
            }
        );
        assert_eq!(list.window, 12);
        assert_eq!(list.source_id.as_deref(), Some("103"));

        let row = &t.nodes[13];
        assert_eq!(row.role, Role::Other);
        assert_eq!(row.label, None);
        assert_eq!(row.actions, Actions::TAP | Actions::LONG_PRESS);
        assert_eq!(row.item, Some(ItemPos { row: 3, col: 0 }));

        let switch = &t.nodes[16];
        assert_eq!(switch.role, Role::Switch);
        assert_eq!(switch.label.as_deref(), Some("Dark theme"));
        assert_eq!(switch.value.as_deref(), Some("On"));
        assert!(
            switch
                .flags
                .contains(NodeFlags::CHECKABLE | NodeFlags::CHECKED | NodeFlags::ENABLED)
        );

        let slider = &t.nodes[17];
        assert_eq!(slider.role, Role::Slider);
        assert_eq!(slider.value.as_deref(), Some("64%"));
        assert_eq!(
            slider.range,
            Some(Range {
                min: 0.0,
                max: 100.0,
                cur: 64.0,
                kind: RangeKind::Percent
            })
        );
        assert_eq!(slider.actions, Actions::SCROLL_DOWN | Actions::SCROLL_UP);
        assert!(slider.flags.contains(NodeFlags::FOCUSED));
    }

    #[test]
    fn decodes_every_flag_letter() {
        let json = r#"{"windows":[],"nodes":[{"p":-1,"w":1,"cls":"android.widget.EditText","b":[0,0,10,10],"f":"vVcCeEFsPSHXxh"}]}"#;
        let t = RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap();
        assert_eq!(t.nodes[0].flags, NodeFlags::all());
        assert_eq!(t.nodes[0].role, Role::SecureField);
    }

    #[test]
    fn unknown_flag_letter_is_an_error_naming_it() {
        let json = r#"{"windows":[],"nodes":[{"p":-1,"w":1,"cls":"x","b":[0,0,1,1],"f":"vZ"}]}"#;
        let err = RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap_err();
        assert!(matches!(err, ParseError::UnknownFlag('Z')), "{err:?}");
        assert!(err.to_string().contains("'Z'"));
    }

    #[test]
    fn unknown_action_is_an_error_naming_it() {
        let json = r#"{"windows":[],"nodes":[{"p":-1,"w":1,"cls":"x","b":[0,0,1,1],"a":["fly"]}]}"#;
        let err = RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap_err();
        assert!(err.to_string().contains("fly"));
    }

    #[test]
    fn parent_after_child_is_an_error() {
        let json = r#"{"windows":[],"nodes":[{"p":1,"w":1,"cls":"x","b":[0,0,1,1]},{"p":-1,"w":1,"cls":"x","b":[0,0,1,1]}]}"#;
        let err = RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap_err();
        assert!(
            matches!(
                err,
                ParseError::BadParent {
                    index: 0,
                    parent: 1
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn compose_lazy_list_with_collection_info_is_a_list() {
        let json = r#"{"windows":[],"nodes":[{"p":-1,"w":1,"cls":"android.view.View","b":[0,0,1,1],"f":"vVeS","a":["scroll_down","scroll_fwd"],"coll":{"rows":31,"cols":1}},{"p":0,"w":1,"cls":"android.view.View","b":[0,0,1,1],"f":"vVe","a":["tap"]}]}"#;
        let t = RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap();
        assert_eq!(t.nodes[0].role, Role::List);
        assert_eq!(t.nodes[1].role, Role::Other);
    }

    #[test]
    fn horizontal_collection_maps_forward_to_right() {
        let json = r#"{"windows":[],"nodes":[{"p":-1,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,0,1,1],"a":["scroll_fwd"],"coll":{"rows":1,"cols":8}}]}"#;
        let t = RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap();
        assert_eq!(t.nodes[0].actions, Actions::SCROLL_RIGHT);
    }

    #[test]
    fn ios_snapshot_uses_points_and_xcui_roles() {
        let json = r#"{"app":{"bundle_id":"com.apple.Preferences","pid":7},"nodes":[
            {"p":-1,"cls":"XCUIElementTypeApplication","text":"Settings","b":[0,0,393,852],"f":"vVe"},
            {"p":0,"cls":"XCUIElementTypeSwitch","text":"Airplane Mode","sd":"0","id":"airplane","b":[16,200,377,244],"f":"vVec","a":["tap"]}
        ]}"#;
        let t = RawTree::from_ios_json(json).unwrap();
        assert_eq!(t.nodes[1].role, Role::Switch);
        assert_eq!(t.nodes[1].label.as_deref(), Some("Airplane Mode"));
        assert_eq!(t.nodes[1].value.as_deref(), Some("0"));
        assert_eq!(t.nodes[1].resource_id.as_deref(), Some("airplane"));
        assert_eq!(
            t.nodes[1].bounds,
            Rect {
                x: 16.0,
                y: 200.0,
                w: 361.0,
                h: 44.0
            }
        );
        assert_eq!(t.nodes[1].source_id, None);
    }
}
