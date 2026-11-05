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
    /// Raw indices of inert text descendants merged into this node's label and hint.
    pub absorbed: Vec<usize>,
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
    let clipped: Vec<Option<Rect>> = tree
        .nodes
        .iter()
        .map(|node| {
            let visible =
                node.flags.contains(NodeFlags::VISIBLE_TO_USER) && node.bounds.area() > 0.0;
            let bounds = match screen {
                Some(s) => node.bounds.clip(s),
                None => Some(node.bounds),
            };
            bounds.filter(|_| visible)
        })
        .collect();

    let mut absorbed_by: Vec<Option<usize>> = vec![None; n];
    let mut merged: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        let node = &tree.nodes[i];
        let pressable = node.actions.intersects(Actions::TAP | Actions::LONG_PRESS);
        if clipped[i].is_none() || !pressable || node.label.is_some() || absorbed_by[i].is_some() {
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

    let mut out: Vec<Node> = Vec::new();
    let mut kept_as: Vec<Option<usize>> = vec![None; n];
    for i in reading_order(tree, &children) {
        let node = &tree.nodes[i];
        let Some(bounds) = clipped[i] else { continue };
        if absorbed_by[i].is_some() {
            continue;
        }
        let texts: Vec<&str> = merged[i]
            .iter()
            .filter_map(|&d| tree.nodes[d].label.as_deref())
            .collect();
        let (label, hint) = match texts.split_first() {
            Some((first, rest)) => {
                let mut hints: Vec<&str> = node.hint.as_deref().into_iter().collect();
                hints.extend(rest);
                (
                    Some(first.to_string()),
                    (!hints.is_empty()).then(|| hints.join(" · ")),
                )
            }
            None => (node.label.clone(), node.hint.clone()),
        };
        let scrollable =
            node.actions.intersects(Actions::SCROLL) || node.flags.contains(NodeFlags::SCROLLABLE);
        let keep = !node.actions.is_empty()
            || label.is_some()
            || node.value.is_some()
            || hint.is_some()
            || node.role.is_text_entry()
            || node.role == Role::ProgressIndicator
            || scrollable;
        if !keep {
            continue;
        }
        let mut role = node.role;
        if role == Role::Other && node.actions.intersects(Actions::TAP | Actions::LONG_PRESS) {
            role = Role::Button;
        }
        let parent =
            std::iter::successors(node.parent, |&p| tree.nodes[p].parent).find_map(|p| kept_as[p]);
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
            absorbed: merged[i].clone(),
        });
    }
    out
}

/// Siblings whose vertical centers fall in the same band this tall, in points, read as one row.
const ROW_BAND_PT: f32 = 8.0;

/// Preorder with siblings top to bottom by center, then left to right (the agent lists a window's views in z
/// order, which puts an app bar after the list it overlaps), and the keyboard window last.
fn reading_order(tree: &RawTree, children: &[Vec<usize>]) -> Vec<usize> {
    let key = |&i: &usize| {
        let b = tree.nodes[i].bounds;
        (
            ((b.y + b.h / 2.0) / ROW_BAND_PT).round() as i64,
            b.x.round() as i64,
        )
    };
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
        let mut kids = children[i].clone();
        kids.sort_by_key(key);
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
        assert_eq!(labels, ["app", "keys"]);
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
}
