use crate::element::{Actions, ElementState, ItemPos, Role};
use crate::geometry::Rect;
use crate::raw::{NodeFlags, RawTree};

/// A node that survives pruning (architecture 4.4 rules 1 to 4), before it has a ref.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Index of the source node in `RawTree::nodes`.
    pub raw: usize,
    /// Index of the nearest kept ancestor in the pruned list.
    pub parent: Option<usize>,
    pub depth: usize,
    pub role: Role,
    pub label: Option<String>,
    pub value: Option<String>,
    pub hint: Option<String>,
    pub id: Option<String>,
    pub bounds: Rect,
    pub state: ElementState,
    pub actions: Actions,
    pub position: Option<ItemPos>,
    pub collection: Option<crate::raw::Collection>,
    /// Raw indices of inert text descendants merged into this node's label and hint.
    pub absorbed: Vec<usize>,
}

/// The system's "Content copied" confirmation: our paste sets the clipboard, so every typed text raises it.
fn says_copied(tree: &RawTree, children: &[Vec<usize>], root: usize) -> bool {
    let mut stack = children[root].clone();
    while let Some(d) = stack.pop() {
        if tree.nodes[d]
            .label
            .as_deref()
            .is_some_and(|l| l.ends_with(" copied"))
        {
            return true;
        }
        stack.extend(&children[d]);
    }
    false
}

/// Prunes a raw tree into the nodes a snapshot shows, in preorder.
pub fn prune(tree: &RawTree) -> Vec<Node> {
    let n = tree.nodes.len();
    let screen = tree
        .windows
        .iter()
        .map(|w| w.bounds)
        .reduce(|a, b| a.union(b))
        .or_else(|| tree.nodes.first().map(|r| r.bounds));
    let mut children = vec![Vec::new(); n];
    for (i, node) in tree.nodes.iter().enumerate() {
        if let Some(p) = node.parent {
            children[p].push(i);
        }
    }
    let keyboard = tree
        .windows
        .iter()
        .find(|w| w.kind == "input_method")
        .map(|w| w.bounds);
    let mut clipped: Vec<Option<Rect>> = tree
        .nodes
        .iter()
        .map(|node| {
            // The platform calls a view covered by the keyboard invisible to the user; the agent still needs to
            // know it exists (and that the keyboard hides it).
            let covered = node.flags.contains(NodeFlags::VISIBLE)
                && keyboard.is_some_and(|k| k.contains(node.bounds));
            let visible = (node.flags.contains(NodeFlags::VISIBLE_TO_USER) || covered)
                && node.bounds.area() > 0.0;
            let bounds = match screen {
                Some(s) => node.bounds.clip(s),
                None => Some(node.bounds),
            };
            bounds.filter(|_| visible)
        })
        .collect();

    for i in 0..n {
        if tree.nodes[i].label.as_deref() == Some("Clipboard") && says_copied(tree, &children, i) {
            let mut stack = vec![i];
            while let Some(d) = stack.pop() {
                clipped[d] = None;
                stack.extend(&children[d]);
            }
        }
    }

    let mut absorbed_by: Vec<Option<usize>> = vec![None; n];
    let mut merged: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        let node = &tree.nodes[i];
        let pressable = node.actions.intersects(Actions::TAP | Actions::LONG_PRESS);
        if clipped[i].is_none() || !pressable || absorbed_by[i].is_some() {
            continue;
        }
        if let Some(label) = &node.label {
            hide_repeats(tree, &children, i, label, &mut absorbed_by);
            continue;
        }
        let mut stack: Vec<usize> = children[i].iter().rev().copied().collect();
        while let Some(d) = stack.pop() {
            let child = &tree.nodes[d];
            if !child.actions.is_empty() {
                continue;
            }
            if clipped[d].is_some() && child.label.is_some() && !child.role.is_text_entry() {
                absorbed_by[d] = Some(i);
                merged[i].push(d);
            }
            stack.extend(children[d].iter().rev());
        }
    }

    let mut dissolved = vec![false; n];
    let tappable = |i: usize| {
        tree.nodes[i]
            .actions
            .intersects(Actions::TAP | Actions::LONG_PRESS)
    };
    for i in 0..n {
        let node = &tree.nodes[i];
        if clipped[i].is_none() || !tappable(i) || absorbed_by[i].is_some() {
            continue;
        }
        let wrapper = matches!(node.role, Role::Cell | Role::Other);
        let mut stack: Vec<usize> = children[i].clone();
        let mut twins = Vec::new();
        while let Some(d) = stack.pop() {
            let child = &tree.nodes[d];
            let same = match &node.label {
                Some(label) => child.label.as_ref() == Some(label),
                None => child.label.is_some() && child.flags.contains(NodeFlags::ENABLED),
            };
            if child.role == Role::Button
                && tappable(d)
                && same
                && clipped[d].is_some()
                && absorbed_by[d].is_none()
            {
                twins.push(d);
            }
            stack.extend(&children[d]);
        }
        let plain_wrapper = node.label.is_some() || merged[i].is_empty();
        match twins.as_slice() {
            [_] if wrapper && plain_wrapper => dissolved[i] = true,
            [d] if node.label.is_some() => absorbed_by[*d] = Some(i),
            _ => {}
        }
    }

    let in_keyboard = |i: usize| {
        let w = tree.nodes[i].window;
        tree.windows
            .iter()
            .any(|win| win.id == w && win.kind == "input_method")
    };
    let mut out: Vec<Node> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut kept_as: Vec<Option<usize>> = vec![None; n];
    for i in reading_order(tree, &children) {
        let node = &tree.nodes[i];
        let Some(bounds) = clipped[i] else { continue };
        if absorbed_by[i].is_some() || dissolved[i] {
            continue;
        }
        if in_keyboard(i) {
            // Typing is `type`, not tapping keys, and sixty key nodes turn every typed character into a new screen.
            if node.parent.is_none() {
                kept_as[i] = Some(out.len());
                out.push(Node {
                    raw: i,
                    parent: None,
                    depth: 0,
                    role: Role::Keyboard,
                    label: None,
                    value: None,
                    hint: None,
                    id: None,
                    bounds,
                    state: state_of(node.flags),
                    actions: Actions::empty(),
                    position: None,
                    collection: None,
                    absorbed: Vec::new(),
                });
            }
            continue;
        }
        let texts: Vec<&str> = merged[i]
            .iter()
            .filter_map(|&d| tree.nodes[d].label.as_deref())
            .collect();
        let state_word = |l: &Option<String>| {
            node.flags.contains(NodeFlags::CHECKABLE)
                && l.as_deref()
                    .is_some_and(|l| matches!(l.trim().to_lowercase().as_str(), "on" | "off"))
        };
        let (label, hint) = match texts.split_first() {
            Some((first, rest)) => {
                let mut hints: Vec<&str> = Vec::new();
                for h in node.hint.as_deref().into_iter().chain(rest.iter().copied()) {
                    if h != *first && !hints.contains(&h) {
                        hints.push(h);
                    }
                }
                (
                    Some(first.to_string()),
                    (!hints.is_empty()).then(|| hints.join(" · ")),
                )
            }
            None => (node.label.clone(), node.hint.clone()),
        };
        let label = if state_word(&label) { None } else { label };
        let scrollable =
            node.actions.intersects(Actions::SCROLL) || node.flags.contains(NodeFlags::SCROLLABLE);
        let separator = node.actions.is_empty()
            && node.value.is_none()
            && hint.is_none()
            && label.as_deref().is_some_and(is_separator);
        let keep = !node.actions.is_empty()
            || (label.is_some() && !separator)
            || node.value.is_some()
            || hint.is_some()
            || node.role.is_text_entry()
            || node.role == Role::ProgressIndicator
            || node.role == Role::Keyboard
            || node.role == Role::WebView
            || scrollable;
        if !keep {
            continue;
        }
        // iOS lists every disabled row chevron as a button named "chevron" after its row.
        if node.role == Role::Button
            && !node.flags.contains(NodeFlags::ENABLED)
            && label.as_deref() == Some("chevron")
        {
            continue;
        }
        let mut role = node.role;
        if role == Role::Other && node.actions.intersects(Actions::TAP | Actions::LONG_PRESS) {
            role = Role::Button;
        }
        let parent =
            std::iter::successors(node.parent, |&p| tree.nodes[p].parent).find_map(|p| kept_as[p]);
        let key = (
            parent,
            role,
            label.clone(),
            [bounds.x, bounds.y, bounds.w, bounds.h].map(f32::to_bits),
        );
        if label.is_some() && !seen.insert(key) {
            continue;
        }
        kept_as[i] = Some(out.len());
        out.push(Node {
            raw: i,
            parent,
            depth: parent.map_or(0, |p| out[p].depth + 1),
            role,
            label,
            value: node.value.clone(),
            hint,
            id: node.resource_id.clone(),
            bounds,
            state: state_of(node.flags),
            actions: node.actions,
            position: node.item,
            collection: node.collection,
            absorbed: merged[i].clone(),
        });
    }
    drop_echoes(out)
}

/// A leaf with no action that only repeats its parent's label (MagicOS Settings nests a toolbar named "Settings" in an
/// app bar named "Settings") says nothing new.
fn drop_echoes(nodes: Vec<Node>) -> Vec<Node> {
    let mut has_child = vec![false; nodes.len()];
    for n in &nodes {
        if let Some(p) = n.parent {
            has_child[p] = true;
        }
    }
    let echo: Vec<bool> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            !has_child[i]
                && n.actions.is_empty()
                && n.value.is_none()
                && n.hint.is_none()
                && n.label.is_some()
                && n.parent
                    .is_some_and(|p| nodes[p].label == n.label && nodes[p].actions.is_empty())
        })
        .collect();
    if !echo.contains(&true) {
        return nodes;
    }
    let mut index = vec![None; nodes.len()];
    let mut out = Vec::with_capacity(nodes.len());
    for (i, mut n) in nodes.into_iter().enumerate() {
        if echo[i] {
            continue;
        }
        n.parent = n.parent.and_then(|p| index[p]);
        index[i] = Some(out.len());
        out.push(n);
    }
    out
}

/// A labeled control is one element to a screen reader: its icons and the texts its label already holds add nothing.
/// A text node that is only the punctuation a page puts between inline elements (`,`, ` · `, `• `): it tells a model
/// nothing and costs a line.
fn is_separator(label: &str) -> bool {
    label.chars().all(|c| {
        c.is_whitespace()
            || matches!(
                c,
                '·' | '•' | ',' | '.' | ';' | ':' | '|' | '/' | '-' | '–' | '—'
            )
    })
}

fn hide_repeats(
    tree: &RawTree,
    children: &[Vec<usize>],
    control: usize,
    label: &str,
    absorbed_by: &mut [Option<usize>],
) {
    let mut stack: Vec<usize> = children[control].clone();
    while let Some(d) = stack.pop() {
        let child = &tree.nodes[d];
        let repeats = child.label.as_deref().is_some_and(|l| {
            let l = l.trim().to_lowercase();
            !l.is_empty() && label.to_lowercase().contains(&l)
        });
        // Android rows put a tappable TextView over the row's own label (the Clock alarm's time); it is the row.
        if !child.actions.is_empty() && !(child.role == Role::Text && repeats) {
            continue;
        }
        if child.role == Role::Image || repeats {
            absorbed_by[d] = Some(control);
        }
        stack.extend(&children[d]);
    }
}

/// Siblings whose vertical centers fall in the same band this tall, in points, read as one row.
const ROW_BAND_PT: f32 = 8.0;

/// Orders `(id, center y, x)` items top to bottom, then left to right inside a row. A row starts at the first item
/// whose center is more than `ROW_BAND_PT` below the row's first center; fixed bands would split two controls
/// whose centers straddle a band edge.
fn rows_then_columns(mut items: Vec<(usize, f32, f32)>) -> Vec<usize> {
    items.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut ordered = Vec::with_capacity(items.len());
    let mut row: Vec<(usize, f32, f32)> = Vec::new();
    for item in items {
        if row.first().is_some_and(|f| item.1 - f.1 > ROW_BAND_PT) {
            row.sort_by(|a, b| a.2.total_cmp(&b.2));
            ordered.extend(row.drain(..).map(|r| r.0));
        }
        row.push(item);
    }
    row.sort_by(|a, b| a.2.total_cmp(&b.2));
    ordered.extend(row.into_iter().map(|r| r.0));
    ordered
}

/// Preorder with siblings top to bottom by center, then left to right (the agent lists a window's views in z
/// order, which puts an app bar after the list it overlaps), and the keyboard window last.
fn reading_order(tree: &RawTree, children: &[Vec<usize>]) -> Vec<usize> {
    let is_keyboard = |i: &usize| {
        let w = tree.nodes[*i].window;
        tree.windows
            .iter()
            .any(|win| win.id == w && win.kind == "input_method")
    };
    let mut roots: Vec<usize> = (0..tree.nodes.len())
        .filter(|&i| tree.nodes[i].parent.is_none())
        .collect();
    roots.sort_by_key(is_keyboard);
    let mut order = Vec::with_capacity(tree.nodes.len());
    let mut stack: Vec<usize> = roots.into_iter().rev().collect();
    while let Some(i) = stack.pop() {
        order.push(i);
        let kids = rows_then_columns(
            children[i]
                .iter()
                .map(|&k| {
                    let b = tree.nodes[k].bounds;
                    (k, b.y + b.h / 2.0, b.x)
                })
                .collect(),
        );
        stack.extend(kids.into_iter().rev());
    }
    order
}

fn state_of(f: NodeFlags) -> ElementState {
    let pairs = [
        (NodeFlags::ENABLED, ElementState::ENABLED),
        (NodeFlags::FOCUSED, ElementState::FOCUSED),
        (NodeFlags::SELECTED, ElementState::SELECTED),
        (NodeFlags::CHECKED, ElementState::CHECKED),
        (NodeFlags::CHECKABLE, ElementState::CHECKABLE),
        (NodeFlags::EXPANDED, ElementState::EXPANDED),
        (NodeFlags::COLLAPSED, ElementState::COLLAPSED),
        (NodeFlags::PASSWORD, ElementState::PASSWORD),
    ];
    pairs
        .iter()
        .filter(|(nf, _)| f.contains(*nf))
        .fold(ElementState::empty(), |s, (_, es)| s | *es)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{Actions, Role};
    use crate::geometry::Scale;
    use crate::raw::RawTree;

    const FIXTURE: &str = include_str!("../tests/fixtures/android/handmade_settings.json");

    fn settings() -> Vec<Node> {
        prune(&RawTree::from_android_json(FIXTURE, Scale { density: 3.0 }).unwrap())
    }

    #[test]
    fn a_web_view_without_a_title_is_kept_so_what_is_in_it_is_known_to_be_web() {
        let json = r#"{"density":1.0,"windows":[],"nodes":[
            {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,400,800],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.webkit.WebView","b":[0,100,400,800],"f":"vVe"},
            {"p":1,"w":1,"cls":"android.widget.CheckBox","text":"Bacon","b":[10,200,40,230],"f":"vVe","a":["tap"]}
        ]}"#;
        let nodes = prune(&RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap());
        let roles: Vec<Role> = nodes.iter().map(|n| n.role).collect();
        assert_eq!(roles, [Role::WebView, Role::Checkbox]);
        assert_eq!(nodes[1].parent, Some(0));
    }

    #[test]
    fn text_made_only_of_separators_is_dropped_and_words_around_it_stay() {
        let json = r#"{"density":1.0,"windows":[],"nodes":[
            {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,400,800],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"the ","b":[0,10,40,30],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":" · ","b":[40,10,50,30],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":",","b":[50,10,60,30],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.view.View","text":"• ","b":[0,40,20,60],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"★","b":[0,70,20,90],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"-","b":[0,100,20,120],"f":"vVe","a":["tap"]},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"7","b":[0,130,20,150],"f":"vVe"}
        ]}"#;
        let nodes = prune(&RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap());
        let labels: Vec<&str> = nodes.iter().filter_map(|n| n.label.as_deref()).collect();
        assert_eq!(labels, ["the ", "★", "-", "7"]);
    }

    #[test]
    fn controls_whose_centers_straddle_a_band_edge_still_read_left_to_right() {
        let row = vec![(7, 186.7, 104.4), (5, 190.5, 0.0), (6, 186.7, 52.2)];
        assert_eq!(rows_then_columns(row), [5, 6, 7]);
        let two_rows = vec![(1, 300.0, 0.0), (2, 100.0, 200.0), (3, 100.0, 10.0)];
        assert_eq!(rows_then_columns(two_rows), [3, 2, 1]);
    }

    #[test]
    fn keeps_only_meaningful_nodes_in_preorder() {
        let nodes = settings();
        let raws: Vec<usize> = nodes.iter().map(|n| n.raw).collect();
        assert_eq!(raws, [1, 2, 3, 4, 7, 10, 13, 16, 17]);
        let parents: Vec<Option<usize>> = nodes.iter().map(|n| n.parent).collect();
        assert_eq!(
            parents,
            [
                None,
                None,
                None,
                Some(2),
                Some(2),
                Some(2),
                Some(2),
                Some(6),
                None
            ]
        );
        let depths: Vec<usize> = nodes.iter().map(|n| n.depth).collect();
        assert_eq!(depths, [0, 0, 0, 1, 1, 1, 1, 2, 0]);
    }

    #[test]
    fn rows_take_their_label_from_inert_text_children() {
        let nodes = settings();
        let row = &nodes[3];
        assert_eq!(row.role, Role::Button);
        assert_eq!(row.label.as_deref(), Some("Network & internet"));
        assert_eq!(row.hint.as_deref(), Some("Mobile, Wi‑Fi, hotspot"));
        assert_eq!(row.absorbed, [5, 6]);
        let dark = &nodes[6];
        assert_eq!(dark.label.as_deref(), Some("Dark theme"));
        assert_eq!(
            dark.hint.as_deref(),
            Some("Will turn on automatically at sunset")
        );
        assert_eq!(nodes[7].role, Role::Switch);
        assert!(nodes[7].state.contains(ElementState::CHECKED));
    }

    #[test]
    fn scroll_only_containers_do_not_absorb_text() {
        let json = r#"{"windows":[],"nodes":[
            {"p":-1,"w":1,"cls":"android.widget.ScrollView","b":[0,0,100,100],"f":"vVeS","a":["scroll_down"]},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"Terms","b":[0,0,100,20],"f":"vVe"}
        ]}"#;
        let nodes = prune(&RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap());
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].label, None);
        assert_eq!(nodes[0].actions, Actions::SCROLL_DOWN);
        assert_eq!(nodes[1].label.as_deref(), Some("Terms"));
    }

    #[test]
    fn an_app_bar_listed_after_its_list_prints_first_and_a_row_reads_left_to_right() {
        let json = r#"{"windows":[{"id":1,"type":"application","bounds":[0,0,100,300]}],"nodes":[
            {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,100,300],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.ListView","b":[0,100,100,300],"f":"vVeS","a":["scroll_down"]},
            {"p":1,"w":1,"cls":"android.widget.TextView","text":"Row","b":[0,100,100,150],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"Right","b":[60,10,100,40],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"Left","b":[0,11,40,41],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"Title","b":[0,0,100,10],"f":"vVe"}
        ]}"#;
        let nodes = prune(&RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap());
        let labels: Vec<_> = nodes
            .iter()
            .map(|n| n.label.as_deref().unwrap_or("-"))
            .collect();
        assert_eq!(labels, ["Title", "Left", "Right", "-", "Row"]);
        assert_eq!(nodes[4].parent, Some(3));
    }

    #[test]
    fn the_keyboard_window_prints_after_the_windows_it_covers() {
        let json = r#"{"windows":[
            {"id":2,"type":"input_method","bounds":[0,200,100,300]},
            {"id":1,"type":"application","bounds":[0,0,100,300]}],"nodes":[
            {"p":-1,"w":2,"cls":"android.widget.FrameLayout","text":"keys","b":[0,200,100,300],"f":"vVe"},
            {"p":-1,"w":1,"cls":"android.widget.TextView","text":"app","b":[0,0,100,20],"f":"vVe"}
        ]}"#;
        let nodes = prune(&RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap());
        let labels: Vec<_> = nodes
            .iter()
            .map(|n| n.label.as_deref().unwrap_or("-"))
            .collect();
        assert_eq!(labels, ["app", "-"]);
        assert_eq!(nodes[1].role, Role::Keyboard);
    }

    #[test]
    fn the_keyboard_window_is_one_node_without_its_keys() {
        let json = r#"{"windows":[
            {"id":2,"type":"input_method","bounds":[0,200,100,300]},
            {"id":1,"type":"application","bounds":[0,0,100,300]}],"nodes":[
            {"p":-1,"w":2,"cls":"android.widget.FrameLayout","b":[0,200,100,300],"f":"vVe"},
            {"p":0,"w":2,"cls":"android.widget.Button","text":"q","b":[0,200,10,220],"f":"vVe","a":["tap"]},
            {"p":0,"w":2,"cls":"android.widget.Button","text":"w","b":[10,200,20,220],"f":"vVe","a":["tap"]},
            {"p":-1,"w":1,"cls":"android.widget.TextView","text":"app","b":[0,0,100,20],"f":"vVe"}
        ]}"#;
        let nodes = prune(&RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap());
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[1].role, Role::Keyboard);
        assert_eq!(nodes[1].bounds.y, 200.0);
    }

    #[test]
    fn a_node_the_platform_calls_invisible_only_because_the_keyboard_covers_it_is_kept() {
        let json = r#"{"windows":[
            {"id":2,"type":"input_method","bounds":[0,200,100,300]},
            {"id":1,"type":"application","bounds":[0,0,100,300]}],"nodes":[
            {"p":-1,"w":2,"cls":"android.widget.FrameLayout","b":[0,200,100,300],"f":"vVe"},
            {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,100,300],"f":"vVe"},
            {"p":1,"w":1,"cls":"android.widget.Button","text":"Send","b":[0,230,100,260],"f":"ve","a":["tap"]},
            {"p":1,"w":1,"cls":"android.widget.Button","text":"Hidden","b":[0,100,100,130],"f":"ve","a":["tap"]}
        ]}"#;
        let nodes = prune(&RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap());
        let labels: Vec<_> = nodes.iter().filter_map(|n| n.label.as_deref()).collect();
        assert_eq!(labels, ["Send"]);
    }

    #[test]
    fn texts_that_repeat_in_a_merged_label_are_said_once() {
        let json = r#"{"windows":[{"id":1,"type":"application","bounds":[0,0,400,200]}],"nodes":[
            {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,400,200],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.Button","hint":"Go forward","b":[0,0,100,50],"f":"vVe","a":["tap"]},
            {"p":1,"w":1,"cls":"android.widget.TextView","text":"Go forward","b":[0,0,50,50],"f":"vVe"},
            {"p":1,"w":1,"cls":"android.widget.TextView","text":"Go forward","b":[50,0,100,50],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.Button","b":[0,100,100,150],"f":"vVe","a":["tap"]},
            {"p":4,"w":1,"cls":"android.widget.TextView","text":"Home","b":[0,100,50,150],"f":"vVe"},
            {"p":4,"w":1,"cls":"android.widget.TextView","text":"News","b":[50,100,100,150],"f":"vVe"},
            {"p":4,"w":1,"cls":"android.widget.TextView","text":"News","b":[50,120,100,150],"f":"vVe"}
        ]}"#;
        let nodes = prune(&RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap());
        let said: Vec<_> = nodes
            .iter()
            .map(|n| (n.label.as_deref(), n.hint.as_deref()))
            .collect();
        assert_eq!(
            said,
            [(Some("Go forward"), None), (Some("Home"), Some("News"))]
        );
    }

    #[test]
    fn a_labeled_button_hides_its_icon_and_the_text_that_repeats_its_label() {
        let json = r#"{"app":{"bundle_id":"x","pid":1},"nodes":[
            {"p":-1,"cls":"XCUIElementTypeOther","b":[0,0,100,100],"f":"vVe"},
            {"p":0,"cls":"XCUIElementTypeButton","text":"Taps","a":["tap"],"b":[0,50,40,90],"f":"vVes"},
            {"p":1,"cls":"XCUIElementTypeImage","text":"hand.tap","b":[10,55,30,70],"f":"vVe"},
            {"p":1,"cls":"XCUIElementTypeStaticText","text":"Taps","b":[10,75,30,85],"f":"vVe"},
            {"p":0,"cls":"XCUIElementTypeButton","text":"Wi-Fi","a":["tap"],"b":[40,50,100,90],"f":"vVe"},
            {"p":4,"cls":"XCUIElementTypeStaticText","text":"Connected","b":[50,75,90,85],"f":"vVe"}
        ]}"#;
        let nodes = prune(&RawTree::from_ios_json(json).unwrap());
        let labels: Vec<_> = nodes
            .iter()
            .map(|n| n.label.as_deref().unwrap_or("-"))
            .collect();
        assert_eq!(labels, ["Taps", "Wi-Fi", "Connected"]);
    }

    #[test]
    fn a_labeled_button_hides_texts_its_label_already_holds() {
        let json = r#"{"app":{"bundle_id":"x","pid":1},"nodes":[
            {"p":-1,"cls":"XCUIElementTypeOther","b":[0,0,400,200],"f":"vVe"},
            {"p":0,"cls":"XCUIElementTypeButton","text":"Apple Account, Sign in to iCloud.","a":["tap"],"b":[0,0,400,80],"f":"vVe"},
            {"p":1,"cls":"XCUIElementTypeStaticText","text":"Apple Account","b":[10,10,200,30],"f":"vVe"},
            {"p":1,"cls":"XCUIElementTypeStaticText","text":"Sign in to iCloud.","b":[10,40,300,60],"f":"vVe"}
        ]}"#;
        let nodes = prune(&RawTree::from_ios_json(json).unwrap());
        assert_eq!(nodes.len(), 1, "{nodes:#?}");
    }

    #[test]
    fn drops_invisible_and_offscreen_and_zero_area() {
        let json = r#"{"windows":[{"id":1,"type":"application","bounds":[0,0,100,200]}],"nodes":[
            {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,100,200],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"hidden","b":[0,0,100,20],"f":"ve"},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"below","b":[0,300,100,320],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"flat","b":[0,50,100,50],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"half","b":[0,190,100,210],"f":"vVe"}
        ]}"#;
        let nodes = prune(&RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap());
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].label.as_deref(), Some("half"));
        assert_eq!(nodes[0].bounds.h, 10.0, "clipped to the screen");
    }

    #[test]
    fn the_clipboard_confirmation_our_own_paste_raises_is_not_part_of_the_screen() {
        let json = r#"{"windows":[{"id":1,"type":"application","bounds":[0,0,400,800]}],"nodes":[
            {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,400,800],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.Button","text":"Save","a":["tap"],"b":[0,100,100,160],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.view.ViewGroup","desc":"Clipboard","b":[0,700,400,800],"f":"vVe"},
            {"p":2,"w":1,"cls":"android.widget.Button","desc":"Clipboard","a":["tap"],"b":[0,700,100,800],"f":"vVe"},
            {"p":2,"w":1,"cls":"android.view.ViewGroup","desc":"Content copied","b":[100,700,400,800],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.view.ViewGroup","desc":"Clipboard","b":[0,300,400,400],"f":"vVe"},
            {"p":5,"w":1,"cls":"android.widget.Button","text":"Clipboard","a":["tap"],"b":[0,300,100,400],"f":"vVe"}
        ]}"#;
        let nodes = prune(&RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap());
        let labels: Vec<_> = nodes.iter().filter_map(|n| n.label.as_deref()).collect();
        assert_eq!(
            labels.iter().filter(|l| **l == "Clipboard").count(),
            2,
            "{labels:?}"
        );
        assert!(!labels.contains(&"Content copied"), "{labels:?}");
    }

    fn labels_and_roles(nodes: &[Node]) -> Vec<(Role, String)> {
        nodes
            .iter()
            .map(|n| (n.role, n.label.clone().unwrap_or_default()))
            .collect()
    }

    #[test]
    fn an_ios_cell_and_its_same_label_button_are_one_button() {
        let json = r#"{"app":{"bundle_id":"x","pid":1},"nodes":[
            {"p":-1,"cls":"XCUIElementTypeOther","b":[0,0,400,800],"f":"vVe"},
            {"p":0,"cls":"XCUIElementTypeTable","b":[0,0,400,800],"f":"vVeS","a":["scroll_down"]},
            {"p":1,"cls":"XCUIElementTypeCell","text":"Display & Text Size","a":["tap"],"b":[0,100,400,150],"f":"vVe"},
            {"p":2,"cls":"XCUIElementTypeButton","text":"Display & Text Size","a":["tap"],"b":[16,100,350,150],"f":"vVe"}
        ]}"#;
        let nodes = prune(&RawTree::from_ios_json(json).unwrap());
        assert_eq!(
            labels_and_roles(&nodes),
            [
                (Role::List, String::new()),
                (Role::Button, "Display & Text Size".to_string())
            ]
        );
        assert_eq!(nodes[1].parent, Some(0));
        assert_eq!(
            nodes[1].bounds.w, 334.0,
            "the inner button is what is tapped"
        );
    }

    #[test]
    fn an_android_clickable_row_and_its_same_label_button_are_one_button() {
        let json = r#"{"windows":[{"id":1,"type":"application","bounds":[0,0,400,800]}],"nodes":[
            {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,400,800],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.view.ViewGroup","desc":"Wi-Fi","b":[0,100,400,150],"f":"vVe","a":["tap"]},
            {"p":1,"w":1,"cls":"android.widget.Button","text":"Wi-Fi","b":[16,100,350,150],"f":"vVe","a":["tap"]}
        ]}"#;
        let nodes = prune(&RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap());
        assert_eq!(
            labels_and_roles(&nodes),
            [(Role::Button, "Wi-Fi".to_string())]
        );
    }

    #[test]
    fn a_switch_keeps_its_state_and_absorbs_the_same_label_button_inside_it() {
        let json = r#"{"app":{"bundle_id":"x","pid":1},"nodes":[
            {"p":-1,"cls":"XCUIElementTypeOther","b":[0,0,400,800],"f":"vVe"},
            {"p":0,"cls":"XCUIElementTypeSwitch","text":"Dark Appearance","a":["tap"],"b":[0,100,400,150],"f":"vVeC"},
            {"p":1,"cls":"XCUIElementTypeButton","text":"Dark Appearance","a":["tap"],"b":[0,100,400,150],"f":"vVe"}
        ]}"#;
        let nodes = prune(&RawTree::from_ios_json(json).unwrap());
        assert_eq!(
            labels_and_roles(&nodes),
            [(Role::Switch, "Dark Appearance".to_string())]
        );
    }

    #[test]
    fn two_different_buttons_that_share_a_label_stay_two() {
        let json = r#"{"windows":[{"id":1,"type":"application","bounds":[0,0,400,800]}],"nodes":[
            {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,400,800],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.Button","text":"Edit","b":[0,100,100,150],"f":"vVe","a":["tap"]},
            {"p":0,"w":1,"cls":"android.widget.Button","text":"Edit","b":[0,200,100,250],"f":"vVe","a":["tap"]}
        ]}"#;
        let nodes = prune(&RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap());
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn an_unlabeled_ios_cell_around_one_enabled_button_is_that_button() {
        let json = r#"{"app":{"bundle_id":"x","pid":1},"nodes":[
            {"p":-1,"cls":"XCUIElementTypeOther","b":[0,0,400,800],"f":"vVe"},
            {"p":0,"cls":"XCUIElementTypeTable","b":[0,0,400,800],"f":"vVeS","a":["scroll_down"]},
            {"p":1,"cls":"XCUIElementTypeCell","a":["tap"],"b":[0,100,400,150],"f":"vVe"},
            {"p":2,"cls":"XCUIElementTypeButton","text":"General","a":["tap"],"b":[0,100,350,150],"f":"vVe"},
            {"p":3,"cls":"XCUIElementTypeStaticText","text":"General","b":[16,110,100,140],"f":"vVe"},
            {"p":2,"cls":"XCUIElementTypeButton","text":"chevron","a":["tap"],"b":[360,110,380,140],"f":"vV"}
        ]}"#;
        let nodes = prune(&RawTree::from_ios_json(json).unwrap());
        let shown = labels_and_roles(&nodes);
        assert!(!shown.iter().any(|(r, _)| *r == Role::Cell), "{shown:?}");
        assert_eq!(
            shown.iter().filter(|(_, l)| l == "General").count(),
            1,
            "{shown:?}"
        );
    }

    #[test]
    fn a_cell_with_two_enabled_buttons_or_with_its_own_text_stays() {
        let two = r#"{"app":{"bundle_id":"x","pid":1},"nodes":[
            {"p":-1,"cls":"XCUIElementTypeTable","b":[0,0,400,800],"f":"vVeS","a":["scroll_down"]},
            {"p":0,"cls":"XCUIElementTypeCell","a":["tap"],"b":[0,100,400,150],"f":"vVe"},
            {"p":1,"cls":"XCUIElementTypeButton","text":"Reply","a":["tap"],"b":[0,100,100,150],"f":"vVe"},
            {"p":1,"cls":"XCUIElementTypeButton","text":"Delete","a":["tap"],"b":[200,100,300,150],"f":"vVe"}
        ]}"#;
        let nodes = prune(&RawTree::from_ios_json(two).unwrap());
        assert!(nodes.iter().any(|n| n.role == Role::Cell));
        let text = r#"{"windows":[{"id":1,"type":"application","bounds":[0,0,400,800]}],"nodes":[
            {"p":-1,"w":1,"cls":"android.view.ViewGroup","b":[0,100,400,150],"f":"vVe","a":["tap"]},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"Backup is on","b":[0,100,200,150],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.Button","text":"Turn off","b":[200,100,300,150],"f":"vVe","a":["tap"]}
        ]}"#;
        let nodes = prune(&RawTree::from_android_json(text, Scale { density: 1.0 }).unwrap());
        assert_eq!(nodes.len(), 2, "{:?}", labels_and_roles(&nodes));
        assert_eq!(nodes[0].label.as_deref(), Some("Backup is on"));
    }

    #[test]
    fn an_unlabeled_android_wrapper_around_one_button_is_that_button() {
        let json = r#"{"windows":[{"id":1,"type":"application","bounds":[0,0,400,800]}],"nodes":[
            {"p":-1,"w":1,"cls":"android.view.ViewGroup","b":[0,100,400,150],"f":"vVe","a":["tap"]},
            {"p":0,"w":1,"cls":"android.widget.Button","text":"Buy","b":[0,100,400,150],"f":"vVe","a":["tap"]}
        ]}"#;
        let nodes = prune(&RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap());
        assert_eq!(
            labels_and_roles(&nodes),
            [(Role::Button, "Buy".to_string())]
        );
    }

    #[test]
    fn a_disabled_chevron_is_dropped_and_an_exact_duplicate_sibling_is_dropped() {
        let json = r#"{"app":{"bundle_id":"x","pid":1},"nodes":[
            {"p":-1,"cls":"XCUIElementTypeTable","b":[0,0,400,800],"f":"vVeS","a":["scroll_down"]},
            {"p":0,"cls":"XCUIElementTypeButton","text":"General","a":["tap"],"b":[0,100,350,150],"f":"vVe"},
            {"p":0,"cls":"XCUIElementTypeButton","text":"chevron","a":["tap"],"b":[360,110,380,140],"f":"vV"},
            {"p":0,"cls":"XCUIElementTypeButton","text":"Dictate","a":["tap"],"b":[300,200,350,250],"f":"vVe"},
            {"p":0,"cls":"XCUIElementTypeButton","text":"Dictate","a":["tap"],"b":[300,200,350,250],"f":"vVe"},
            {"p":0,"cls":"XCUIElementTypeButton","text":"Dictate","a":["tap"],"b":[300,300,350,350],"f":"vVe"}
        ]}"#;
        let nodes = prune(&RawTree::from_ios_json(json).unwrap());
        let shown: Vec<String> = nodes.iter().filter_map(|n| n.label.clone()).collect();
        assert_eq!(shown, ["General", "Dictate", "Dictate"]);
        assert_eq!(nodes[3].bounds.y, 300.0, "the same label elsewhere stays");
    }
}
