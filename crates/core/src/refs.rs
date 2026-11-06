use std::collections::{HashMap, HashSet};

use crate::element::{Ref, Role};
use crate::geometry::Point;
use crate::prune::Node;
use crate::raw::RawTree;

/// Bounds may move this far, in points, while a relabeled platform identity stays the same element.
const SAME_PLACE_PT: f32 = 8.0;

/// How many elements that left the screen stay claimable by the same element coming back.
const RETIRED_KEPT: usize = 400;

type Identity = (i32, String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StructuralKey {
    role: Role,
    id: Option<String>,
    label: Option<String>,
    ancestor: Option<(Role, Option<String>, Option<String>)>,
}

#[derive(Debug, Clone)]
struct Seen {
    r: Ref,
    role: Role,
    label: Option<String>,
    key: StructuralKey,
    center: Point,
}

fn in_list(nodes: &[Node], i: usize) -> bool {
    std::iter::successors(nodes[i].parent, |&p| nodes[p].parent)
        .any(|p| nodes[p].role == Role::List)
}

/// Assigns session-stable refs to pruned nodes (architecture 4.3).
#[derive(Debug, Default)]
pub struct RefTable {
    next: u32,
    by_identity: HashMap<Identity, usize>,
    last: Vec<Seen>,
    retired: Vec<Seen>,
}

impl RefTable {
    /// A ref no tree element has, for an element that comes from pixels.
    pub fn fresh(&mut self) -> Ref {
        self.next += 1;
        Ref(self.next)
    }

    /// One ref per node, matched against the previous call's nodes.
    pub fn assign(&mut self, tree: &RawTree, nodes: &[Node]) -> Vec<Ref> {
        let keys: Vec<StructuralKey> = nodes
            .iter()
            .map(|n| StructuralKey {
                role: n.role,
                id: n.id.clone(),
                label: n.label.clone(),
                ancestor: n.parent.map(|p| {
                    let a = &nodes[p];
                    (a.role, a.id.clone(), a.label.clone())
                }),
            })
            .collect();
        let identities: Vec<Option<Identity>> = nodes
            .iter()
            .map(|n| {
                let raw = &tree.nodes[n.raw];
                raw.source_id.clone().map(|s| (raw.window, s))
            })
            .collect();

        let mut out: Vec<Option<Ref>> = vec![None; nodes.len()];
        let mut claimed: HashSet<usize> = HashSet::new();

        for (i, n) in nodes.iter().enumerate() {
            let Some(&j) = identities[i]
                .as_ref()
                .and_then(|id| self.by_identity.get(id))
            else {
                continue;
            };
            let prev = &self.last[j];
            let same_place = prev.center.distance(n.bounds.center()) < SAME_PLACE_PT;
            // A list recycles one view for many items, so an item at the old place with another label is another
            // item; elsewhere a new label at the old place is the same element changing (a counter, a clock).
            let continuous = prev.label == n.label || (same_place && !in_list(nodes, i));
            if prev.role == n.role && continuous && claimed.insert(j) {
                out[i] = Some(prev.r);
            }
        }

        for (i, n) in nodes.iter().enumerate() {
            if out[i].is_some() {
                continue;
            }
            let center = n.bounds.center();
            let best = self
                .last
                .iter()
                .enumerate()
                .filter(|(j, s)| !claimed.contains(j) && s.key == keys[i])
                .min_by(|(_, a), (_, b)| {
                    a.center
                        .distance(center)
                        .total_cmp(&b.center.distance(center))
                })
                .map(|(j, _)| j);
            if let Some(j) = best {
                claimed.insert(j);
                out[i] = Some(self.last[j].r);
            }
        }

        for (i, n) in nodes.iter().enumerate() {
            if out[i].is_some() {
                continue;
            }
            let center = n.bounds.center();
            let best = self
                .last
                .iter()
                .enumerate()
                .filter(|(j, s)| {
                    !claimed.contains(j)
                        && (s.key.role, &s.key.id, &s.key.label)
                            == (keys[i].role, &keys[i].id, &keys[i].label)
                })
                .min_by(|(_, a), (_, b)| {
                    a.center
                        .distance(center)
                        .total_cmp(&b.center.distance(center))
                })
                .map(|(j, _)| j);
            if let Some(j) = best {
                claimed.insert(j);
                out[i] = Some(self.last[j].r);
            }
        }

        let unique = |ids: &mut dyn Iterator<Item = Option<&String>>| {
            let mut count: HashMap<String, usize> = HashMap::new();
            for id in ids.flatten() {
                *count.entry(id.clone()).or_default() += 1;
            }
            count
                .into_iter()
                .filter(|(_, c)| *c == 1)
                .map(|(id, _)| id)
                .collect::<HashSet<String>>()
        };
        let now_unique = unique(&mut nodes.iter().map(|n| n.id.as_ref()));
        let before_unique = unique(&mut self.last.iter().map(|s| s.key.id.as_ref()));
        for (i, n) in nodes.iter().enumerate() {
            let Some(id) = n.id.as_ref().filter(|id| now_unique.contains(*id)) else {
                continue;
            };
            if out[i].is_some() || !before_unique.contains(id) {
                continue;
            }
            let found = self
                .last
                .iter()
                .position(|s| s.key.id.as_ref() == Some(id) && s.role == n.role);
            if let Some(j) = found
                && claimed.insert(j)
            {
                out[i] = Some(self.last[j].r);
            }
        }

        let mut taken: HashSet<Ref> = out.iter().flatten().copied().collect();
        for place_only in [false, true] {
            for (i, n) in nodes.iter().enumerate() {
                if out[i].is_some() {
                    continue;
                }
                let center = n.bounds.center();
                let best = self
                    .retired
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| {
                        let same = if place_only {
                            (s.key.role, &s.key.id, &s.key.label)
                                == (keys[i].role, &keys[i].id, &keys[i].label)
                        } else {
                            s.key == keys[i]
                        };
                        same && s.center.distance(center) < SAME_PLACE_PT && !taken.contains(&s.r)
                    })
                    .map(|(j, _)| j)
                    .next();
                if let Some(j) = best {
                    taken.insert(self.retired[j].r);
                    out[i] = Some(self.retired[j].r);
                }
            }
        }

        let refs: Vec<Ref> = out
            .into_iter()
            .map(|r| {
                r.unwrap_or_else(|| {
                    self.next += 1;
                    Ref(self.next)
                })
            })
            .collect();

        let kept: HashSet<Ref> = refs.iter().copied().collect();
        self.retired.retain(|s| !kept.contains(&s.r));
        let gone: Vec<Seen> = self
            .last
            .drain(..)
            .filter(|s| !kept.contains(&s.r))
            .collect();
        self.retired.extend(gone);
        let over = self.retired.len().saturating_sub(RETIRED_KEPT);
        self.retired.drain(..over);

        self.by_identity.clear();
        for (i, id) in identities.into_iter().enumerate() {
            if let Some(id) = id {
                self.by_identity.insert(id, i);
            }
        }
        self.last = nodes
            .iter()
            .zip(&refs)
            .zip(keys)
            .map(|((n, &r), key)| Seen {
                r,
                role: n.role,
                label: n.label.clone(),
                key,
                center: n.bounds.center(),
            })
            .collect();
        refs
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use proptest::prelude::*;

    use super::*;
    use crate::geometry::Scale;
    use crate::prune::prune;
    use crate::raw::RawTree;

    fn android(rows: &[(&str, &str, f32)]) -> RawTree {
        android_in("androidx.recyclerview.widget.RecyclerView", rows)
    }

    fn android_in(container: &str, rows: &[(&str, &str, f32)]) -> RawTree {
        let mut nodes = vec![
            r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,400,2000],"f":"vVe","n":"root"}"#.to_string(),
            format!(r#"{{"p":0,"w":1,"cls":"{container}","b":[0,0,400,2000],"f":"vVeS","a":["scroll_down"],"n":"list"}}"#),
        ];
        for (n, label, y) in rows {
            nodes.push(format!(
                r#"{{"p":1,"w":1,"cls":"android.widget.TextView","text":"{label}","b":[0,{y},400,{}],"f":"vVe","a":["tap"],"n":"{n}"}}"#,
                y + 50.0
            ));
        }
        let json = format!(r#"{{"windows":[],"nodes":[{}]}}"#, nodes.join(","));
        RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
    }

    fn refs(table: &mut RefTable, tree: &RawTree) -> Vec<String> {
        table
            .assign(tree, &prune(tree))
            .iter()
            .map(|r| r.to_string())
            .collect()
    }

    fn ios(nodes: &[(&str, &str, f32)]) -> RawTree {
        let mut json = vec![
            r#"{"p":-1,"cls":"XCUIElementTypeOther","b":[0,0,400,800],"f":"vVe"}"#.to_string(),
        ];
        for (id, label, y) in nodes {
            json.push(format!(
                r#"{{"p":0,"cls":"XCUIElementTypeStaticText","id":"{id}","text":"{label}","b":[0,{y},400,{}],"f":"vVe"}}"#,
                y + 40.0
            ));
        }
        RawTree::from_ios_json(&format!(
            r#"{{"app":{{"bundle_id":"x","pid":1}},"nodes":[{}]}}"#,
            json.join(",")
        ))
        .unwrap()
    }

    fn assign_labels(table: &mut RefTable, tree: &RawTree) -> Vec<(String, Ref)> {
        let nodes = prune(tree);
        let refs = table.assign(tree, &nodes);
        nodes
            .into_iter()
            .zip(refs)
            .map(|(n, r)| (n.label.unwrap_or_default(), r))
            .collect()
    }

    #[test]
    fn an_element_with_a_unique_app_id_keeps_its_ref_when_relabeled() {
        let mut t = RefTable::default();
        let before = assign_labels(&mut t, &ios(&[("status", "Last tap: none", 100.0)]));
        let after = assign_labels(&mut t, &ios(&[("status", "Last tap: b17", 100.0)]));
        assert_eq!(after[0].1, before[0].1);
    }

    #[test]
    fn rows_sharing_one_app_id_do_not_keep_refs_by_place() {
        let mut t = RefTable::default();
        let before = assign_labels(
            &mut t,
            &ios(&[("cell", "Row 1", 100.0), ("cell", "Row 2", 140.0)]),
        );
        let after = assign_labels(
            &mut t,
            &ios(&[("cell", "Row 2", 100.0), ("cell", "Row 3", 140.0)]),
        );
        assert_eq!(after[0].1, before[1].1, "Row 2 follows its item");
        assert!(before.iter().all(|(_, r)| *r != after[1].1), "Row 3 is new");
    }

    #[test]
    fn first_tree_numbers_kept_nodes_in_preorder() {
        let mut t = RefTable::default();
        let tree = android(&[("a", "Alpha", 0.0), ("b", "Beta", 50.0)]);
        assert_eq!(refs(&mut t, &tree), ["e1", "e2", "e3"]);
        assert_eq!(refs(&mut t, &tree), ["e1", "e2", "e3"]);
    }

    #[test]
    fn insert_at_top_keeps_old_refs() {
        let mut t = RefTable::default();
        refs(
            &mut t,
            &android(&[("a", "Alpha", 0.0), ("b", "Beta", 50.0)]),
        );
        let after = android(&[
            ("z", "Zeta", 0.0),
            ("a", "Alpha", 50.0),
            ("b", "Beta", 100.0),
        ]);
        assert_eq!(refs(&mut t, &after), ["e1", "e4", "e2", "e3"]);
    }

    #[test]
    fn label_change_in_place_keeps_ref() {
        let mut t = RefTable::default();
        let in_a_group =
            |label: &str| android_in("android.widget.LinearLayout", &[("c", label, 0.0)]);
        refs(&mut t, &in_a_group("3 items"));
        assert_eq!(refs(&mut t, &in_a_group("4 items")), ["e1", "e2"]);
    }

    #[test]
    fn a_recycled_list_slot_showing_another_item_in_the_same_place_is_a_new_element() {
        let mut t = RefTable::default();
        assert_eq!(
            refs(&mut t, &android(&[("7", "Row 3", 300.0)])),
            ["e1", "e2"]
        );
        assert_eq!(
            refs(&mut t, &android(&[("7", "Row 14", 300.0)])),
            ["e1", "e3"],
            "a tap on @e2 must not land on Row 14"
        );
    }

    #[test]
    fn recycled_row_gets_a_new_ref() {
        let mut t = RefTable::default();
        let before = android(&[("7", "Row 12", 300.0), ("8", "Row 13", 400.0)]);
        assert_eq!(refs(&mut t, &before), ["e1", "e2", "e3"]);
        let after = android(&[("3", "Row 12", 100.0), ("7", "Row 20", 900.0)]);
        assert_eq!(
            refs(&mut t, &after),
            ["e1", "e2", "e4"],
            "Row 12 follows its label; Row 20 is new"
        );
    }

    #[test]
    fn ios_rows_reordered_follow_labels() {
        let ios = |labels: &[&str]| {
            let mut nodes = vec![r#"{"p":-1,"cls":"XCUIElementTypeTable","b":[0,0,400,800],"f":"vVeS","a":["scroll_down"]}"#.to_string()];
            for (i, l) in labels.iter().enumerate() {
                let y = i * 50;
                nodes.push(format!(r#"{{"p":0,"cls":"XCUIElementTypeCell","text":"{l}","b":[0,{y},400,{}],"f":"vVe","a":["tap"]}}"#, y + 50));
            }
            RawTree::from_ios_json(&format!(r#"{{"nodes":[{}]}}"#, nodes.join(","))).unwrap()
        };
        let mut t = RefTable::default();
        assert_eq!(
            refs(&mut t, &ios(&["A", "B", "C"])),
            ["e1", "e2", "e3", "e4"]
        );
        assert_eq!(
            refs(&mut t, &ios(&["C", "A", "B"])),
            ["e1", "e4", "e2", "e3"]
        );
    }

    #[derive(Debug, Clone)]
    enum Edit {
        Insert(usize),
        Delete(usize),
        Move(usize, usize),
        Relabel(usize),
    }

    fn edit() -> impl Strategy<Value = Edit> {
        prop_oneof![
            any::<usize>().prop_map(Edit::Insert),
            any::<usize>().prop_map(Edit::Delete),
            (any::<usize>(), any::<usize>()).prop_map(|(a, b)| Edit::Move(a, b)),
            any::<usize>().prop_map(Edit::Relabel),
        ]
    }

    fn tree_of(rows: &[(u32, String)]) -> RawTree {
        let owned: Vec<(String, String, f32)> = rows
            .iter()
            .enumerate()
            .map(|(i, (id, l))| (id.to_string(), l.clone(), i as f32 * 50.0))
            .collect();
        let borrowed: Vec<(&str, &str, f32)> = owned
            .iter()
            .map(|(a, b, y)| (a.as_str(), b.as_str(), *y))
            .collect();
        android_in("android.widget.LinearLayout", &borrowed)
    }

    proptest! {
        #[test]
        fn refs_are_stable_unique_and_never_reused(initial in 1usize..15, edits in prop::collection::vec(edit(), 0..12)) {
            let mut fresh = 0u32;
            let mut next_id = || { fresh += 1; fresh };
            let mut rows: Vec<(u32, String)> = (0..initial).map(|_| { let id = next_id(); (id, format!("Row {id}")) }).collect();
            let mut table = RefTable::default();
            let mut other = RefTable::default();
            let mut issued: HashSet<String> = HashSet::new();

            let tree = tree_of(&rows);
            let mut prev: HashMap<u32, String> = HashMap::new();
            let first = refs(&mut table, &tree);
            prop_assert_eq!(&first, &refs(&mut other, &tree), "deterministic");
            for (row, r) in rows.iter().zip(&first[1..]) { prev.insert(row.0, r.clone()); }
            issued.extend(first);

            for e in edits {
                let n = rows.len();
                match e {
                    Edit::Insert(at) => { let id = next_id(); rows.insert(at % (n + 1), (id, format!("Row {id}"))); }
                    Edit::Delete(at) if n > 0 => { rows.remove(at % n); }
                    Edit::Move(a, b) if n > 0 => { let r = rows.remove(a % n); rows.insert(b % n, r); }
                    Edit::Relabel(at) if n > 0 => { let id = next_id(); rows[at % n].1 = format!("Renamed {id}"); }
                    _ => {}
                }
                let tree = tree_of(&rows);
                let got = refs(&mut table, &tree);
                prop_assert_eq!(&got, &refs(&mut other, &tree), "deterministic");
                let unique: HashSet<&String> = got.iter().collect();
                prop_assert_eq!(unique.len(), got.len(), "no ref on two live elements: {:?}", got);
                prop_assert_eq!(&got[0], "e1", "the list keeps its ref");
                let mut next_prev = HashMap::new();
                for (row, r) in rows.iter().zip(&got[1..]) {
                    match prev.get(&row.0) {
                        Some(old) => prop_assert_eq!(old, r, "row {} kept its ref", row.0),
                        None => prop_assert!(!issued.contains(r), "new row got reused ref {}", r),
                    }
                    next_prev.insert(row.0, r.clone());
                }
                issued.extend(got);
                prev = next_prev;
            }
        }
    }

    #[test]
    fn an_element_keeps_its_ref_when_a_wrapper_group_appears_around_it() {
        let screen = |wrapped: bool| {
            let (field_parent, group) = if wrapped {
                (
                    1,
                    r#",{"p":0,"cls":"XCUIElementTypeOther","text":"Toolbar","b":[0,700,400,800],"f":"vVe"}"#,
                )
            } else {
                (0, "")
            };
            RawTree::from_ios_json(&format!(
                r#"{{"app":{{"bundle_id":"x","pid":1}},"nodes":[
                {{"p":-1,"cls":"XCUIElementTypeOther","b":[0,0,400,800],"f":"vVe"}}{group},
                {{"p":{field_parent},"cls":"XCUIElementTypeSearchField","text":"Search","a":["tap"],"b":[10,730,390,780],"f":"vVe"}},
                {{"p":0,"cls":"XCUIElementTypeButton","text":"Settings","a":["tap"],"b":[0,0,100,40],"f":"vVe"}}]}}"#
            ))
            .unwrap()
        };
        let mut t = RefTable::default();
        let before = assign_labels(&mut t, &screen(false));
        let after = assign_labels(&mut t, &screen(true));
        let of = |v: &[(String, Ref)], label: &str| v.iter().find(|(l, _)| l == label).unwrap().1;
        assert_eq!(of(&after, "Search"), of(&before, "Search"));
        assert_eq!(of(&after, "Settings"), of(&before, "Settings"));
        assert!(before.iter().all(|(_, r)| *r != of(&after, "Toolbar")));
    }

    #[test]
    fn two_same_label_elements_do_not_trade_refs_when_a_wrapper_appears() {
        let screen = |wrapped: bool| {
            let (parent, group) = if wrapped {
                (
                    1,
                    r#",{"p":0,"cls":"XCUIElementTypeOther","text":"Box","b":[0,0,400,800],"f":"vVe"}"#,
                )
            } else {
                (0, "")
            };
            RawTree::from_ios_json(&format!(
                r#"{{"app":{{"bundle_id":"x","pid":1}},"nodes":[
                {{"p":-1,"cls":"XCUIElementTypeOther","b":[0,0,400,800],"f":"vVe"}}{group},
                {{"p":{parent},"cls":"XCUIElementTypeButton","text":"Edit","a":["tap"],"b":[0,100,100,140],"f":"vVe"}},
                {{"p":{parent},"cls":"XCUIElementTypeButton","text":"Edit","a":["tap"],"b":[0,300,100,340],"f":"vVe"}}]}}"#
            ))
            .unwrap()
        };
        let mut t = RefTable::default();
        let before = assign_labels(&mut t, &screen(false));
        let after = assign_labels(&mut t, &screen(true));
        let edits = |v: &[(String, Ref)]| -> Vec<Ref> {
            v.iter()
                .filter(|(l, _)| l == "Edit")
                .map(|(_, r)| *r)
                .collect()
        };
        assert_eq!(edits(&after), edits(&before));
    }

    fn android_windows(windows: &[&[(&str, &str, [f32; 4])]]) -> RawTree {
        let mut nodes = Vec::new();
        for (w, children) in windows.iter().enumerate() {
            let w = w + 1;
            nodes.push(format!(
                r#"{{"p":-1,"w":{w},"cls":"android.widget.FrameLayout","b":[0,0,400,800],"f":"vVe","n":"root{w}"}}"#
            ));
            let root = nodes.len() - 1;
            for (n, label, b) in *children {
                nodes.push(format!(
                    r#"{{"p":{root},"w":{w},"cls":"android.widget.Button","text":"{label}","b":[{},{},{},{}],"f":"vVe","a":["tap"],"n":"{n}{w}"}}"#,
                    b[0], b[1], b[2], b[3]
                ));
            }
        }
        RawTree::from_android_json(
            &format!(r#"{{"windows":[],"nodes":[{}]}}"#, nodes.join(",")),
            Scale { density: 1.0 },
        )
        .unwrap()
    }

    #[test]
    fn a_dialog_comes_back_from_a_popup_with_the_refs_it_had() {
        let dialog = || {
            android_windows(&[&[
                ("h", "Hour", [10.0, 100.0, 100.0, 150.0]),
                ("m", "Minute", [120.0, 100.0, 220.0, 150.0]),
                ("ap", "AM", [240.0, 100.0, 300.0, 150.0]),
            ]])
        };
        let popup = || android_windows(&[&[("pm", "PM", [240.0, 160.0, 300.0, 200.0])]]);
        let mut t = RefTable::default();
        let before = refs(&mut t, &dialog());
        refs(&mut t, &popup());
        assert_eq!(refs(&mut t, &dialog()), before);
    }

    #[test]
    fn a_tab_bar_keeps_its_refs_across_a_dialog_that_hid_it() {
        let screen = |dialog: bool| {
            if dialog {
                android_windows(&[&[("ok", "OK", [200.0, 600.0, 300.0, 650.0])]])
            } else {
                android_windows(&[&[
                    ("t1", "ALARM", [0.0, 700.0, 100.0, 760.0]),
                    ("t2", "CLOCK", [100.0, 700.0, 200.0, 760.0]),
                ]])
            }
        };
        let mut t = RefTable::default();
        let tabs = refs(&mut t, &screen(false));
        refs(&mut t, &screen(true));
        refs(&mut t, &screen(true));
        assert_eq!(refs(&mut t, &screen(false)), tabs);
    }

    #[test]
    fn an_element_that_returns_elsewhere_is_a_new_element() {
        let screen = |x: f32| android_windows(&[&[("b", "Edit", [x, 100.0, x + 50.0, 150.0])]]);
        let mut t = RefTable::default();
        refs(&mut t, &screen(0.0));
        refs(
            &mut t,
            &android_windows(&[&[("z", "Other", [0.0, 300.0, 50.0, 350.0])]]),
        );
        assert_eq!(refs(&mut t, &screen(200.0)), ["e3"]);
    }
}
