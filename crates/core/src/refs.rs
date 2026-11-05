use std::collections::{HashMap, HashSet};

use crate::element::{Ref, Role};
use crate::geometry::Point;
use crate::prune::Node;
use crate::raw::RawTree;

/// Bounds may move this far, in points, while a relabeled platform identity stays the same element.
const SAME_PLACE_PT: f32 = 8.0;

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
}

impl RefTable {
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

        let refs: Vec<Ref> = out
            .into_iter()
            .map(|r| {
                r.unwrap_or_else(|| {
                    self.next += 1;
                    Ref(self.next)
                })
            })
            .collect();

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
}
