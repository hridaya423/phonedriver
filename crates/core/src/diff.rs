use std::collections::HashMap;

use crate::element::{Element, ElementState, Ref, Role};
use crate::render::{Plan, Rendered, quote, shown_hint, shown_value, state_words};

/// Above this share of changed refs the agent is on a new screen and gets a full snapshot.
const NEW_SCREEN_SHARE: f32 = 0.6;

/// A screen that came back is told as a diff against the one it replaced when at most this share of its lines differ.
const BACK_SHARE: f32 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Label,
    Value,
    Hint,
    State,
    Covered,
}

/// One difference between two observations. `from` and `to` are already rendered and masked.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Added(Ref),
    Removed(Ref),
    /// The screen the agent was shown before the one it was last shown is on screen again.
    Uncovered,
    Changed {
        r#ref: Ref,
        field: Field,
        from: String,
        to: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiffOrFull {
    Diff(Vec<Change>),
    Full,
}

fn is_secure(e: &Element) -> bool {
    e.role == Role::SecureField || e.state.contains(ElementState::PASSWORD)
}

fn words(e: &Element) -> String {
    let w = state_words(e);
    if w.is_empty() {
        "(none)".into()
    } else {
        w.join(" ")
    }
}

fn covered(under: bool) -> String {
    if under { "under-keyboard" } else { "(none)" }.into()
}

fn opt(s: Option<String>) -> String {
    s.map_or_else(|| "(none)".into(), |s| quote(&s))
}

/// `diff`, except that when `next` is a new screen but is almost `below`, the screen `prev` had replaced (a dialog or a
/// menu closed), the answer is a diff against `below` that says so.
pub fn diff_below(prev: &Rendered, next: &Rendered, below: Option<&Rendered>) -> DiffOrFull {
    let plain = diff(prev, next);
    let (DiffOrFull::Full, Some(below)) = (&plain, below) else {
        return plain;
    };
    match diff(below, next) {
        DiffOrFull::Diff(changes)
            if changes.len() as f32 <= next.elements.len() as f32 * BACK_SHARE =>
        {
            let mut all = vec![Change::Uncovered];
            all.extend(changes);
            DiffOrFull::Diff(all)
        }
        _ => plain,
    }
}

pub fn diff(prev: &Rendered, next: &Rendered) -> DiffOrFull {
    let before: HashMap<Ref, &Element> = prev.elements.iter().map(|e| (e.r#ref, e)).collect();
    let after: HashMap<Ref, &Element> = next.elements.iter().map(|e| (e.r#ref, e)).collect();
    let removed: Vec<Ref> = prev
        .elements
        .iter()
        .map(|e| e.r#ref)
        .filter(|r| !after.contains_key(r))
        .collect();
    let added = next
        .elements
        .iter()
        .filter(|e| !before.contains_key(&e.r#ref))
        .count();
    let union = prev.elements.len() + added;
    if union > 0 && (removed.len() + added) as f32 / union as f32 > NEW_SCREEN_SHARE {
        return DiffOrFull::Full;
    }

    let mut changes: Vec<Change> = removed.into_iter().map(Change::Removed).collect();
    for e in &next.elements {
        let r = e.r#ref;
        let Some(old) = before.get(&r) else {
            changes.push(Change::Added(r));
            continue;
        };
        let value = if is_secure(e) {
            let changed = old.value != e.value;
            let mask = |on: bool| if on { "•••" } else { "" }.to_string();
            (mask(false), mask(changed))
        } else {
            (opt(shown_value(old)), opt(shown_value(e)))
        };
        let fields = [
            (Field::Label, opt(old.label.clone()), opt(e.label.clone())),
            (Field::Value, value.0, value.1),
            (Field::Hint, opt(shown_hint(old)), opt(shown_hint(e))),
            (Field::State, words(old), words(e)),
            (
                Field::Covered,
                covered(prev.under_keyboard(old)),
                covered(next.under_keyboard(e)),
            ),
        ];
        for (field, from, to) in fields {
            if from != to {
                changes.push(Change::Changed {
                    r#ref: r,
                    field,
                    from,
                    to,
                });
            }
        }
    }
    DiffOrFull::Diff(changes)
}

impl DiffOrFull {
    /// `+`, `-`, `~` lines; consecutive removals share one line. Empty for `Full` and for no change.
    pub fn text(&self, next: &Rendered) -> String {
        let DiffOrFull::Diff(changes) = self else {
            return String::new();
        };
        let mut lines: Vec<String> = Vec::new();
        let mut removing = false;
        let mut covering: Vec<(&str, &str, Vec<Ref>)> = Vec::new();
        let mut fields: HashMap<Ref, usize> = HashMap::new();
        for c in changes {
            if let Change::Changed { r#ref, field, .. } = c
                && *field != Field::Covered
            {
                *fields.entry(*r#ref).or_default() += 1;
            }
        }
        let mut told: Vec<Ref> = Vec::new();
        let mut added = vec![false; next.elements.len()];
        for c in changes {
            if let Change::Added(r) = c
                && let Some(i) = next.index_of(*r)
            {
                added[i] = true;
            }
        }
        let plan = next.inline_plan(&added);
        for c in changes {
            match c {
                Change::Removed(r) => {
                    match lines.last_mut() {
                        Some(last) if removing => last.push_str(&format!(" @{r}")),
                        _ => lines.push(format!("- @{r}")),
                    }
                    removing = true;
                    continue;
                }
                Change::Uncovered => lines.push(
                    if changes.len() == 1 {
                        "the screen before the last one is back, unchanged"
                    } else {
                        "the screen before the last one is back, except:"
                    }
                    .into(),
                ),
                Change::Added(r) => {
                    if let Some(i) = next.index_of(*r) {
                        match &plan[i] {
                            Plan::Own => lines.push(format!("+ {}", next.line(&next.elements[i]))),
                            Plan::Line(l) => lines.push(format!("+ {l}")),
                            Plan::Skip => {}
                        }
                    }
                }
                Change::Changed {
                    r#ref,
                    field,
                    from,
                    to,
                } => {
                    let Some(e) = next.index_of(*r#ref).map(|i| &next.elements[i]) else {
                        continue;
                    };
                    if *field == Field::Covered {
                        match covering.iter_mut().find(|(f, t, _)| f == from && t == to) {
                            Some((_, _, refs)) => refs.push(*r#ref),
                            None => covering.push((from, to, vec![*r#ref])),
                        }
                        continue;
                    }
                    if fields.get(r#ref).is_some_and(|n| *n > 1) && !is_secure(e) {
                        if !told.contains(r#ref) {
                            told.push(*r#ref);
                            lines.push(format!("~ {}", next.line(e).trim_start()));
                        }
                        removing = false;
                        continue;
                    }
                    let head = format!("~ @{} {}", r#ref, e.role.name());
                    lines.push(match field {
                        Field::Value if is_secure(e) => {
                            format!("~ @{} secure field changed", r#ref)
                        }
                        Field::Label => format!("{head} {from} → {to}"),
                        _ => match &e.label {
                            Some(l) => format!("{head} {} {from} → {to}", quote(l)),
                            None => format!("{head} {from} → {to}"),
                        },
                    });
                }
            }
            removing = false;
        }
        for (from, to, refs) in covering {
            if next.header.keyboard.is_none() {
                lines.push("keyboard hidden".into());
                continue;
            }
            let refs: Vec<String> = refs.iter().map(|r| format!("@{r}")).collect();
            lines.push(format!("~ {} {from} → {to}", refs.join(" ")));
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::Ref;
    use crate::geometry::{Rect, Scale};
    use crate::prune::prune;
    use crate::raw::RawTree;
    use crate::refs::RefTable;
    use crate::render::{Header, Rendered};

    fn screen(table: &mut RefTable, nodes: &[String]) -> Rendered {
        let json = format!(
            r#"{{"windows":[{{"id":1,"type":"application","bounds":[0,0,400,2000]}}],"nodes":[{}]}}"#,
            nodes.join(",")
        );
        let tree = RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap();
        let pruned = prune(&tree);
        let refs = table.assign(&tree, &pruned);
        Rendered::build(pruned, &refs, Header::default())
    }

    fn node(slot: usize, id: &str, cls: &str, extra: &str) -> String {
        let y = slot * 60;
        format!(
            r#"{{"p":-1,"w":1,"cls":"{cls}","b":[0,{y},400,{}],"n":"{id}",{extra}}}"#,
            y + 50
        )
    }

    fn switch(checked: bool) -> String {
        let f = if checked { "vVecC" } else { "vVec" };
        node(
            1,
            "switch",
            "android.widget.Switch",
            &format!(r#""desc":"Dark theme","f":"{f}","a":["tap"]"#),
        )
    }

    fn buttons(labels: &[&str]) -> Vec<String> {
        labels
            .iter()
            .enumerate()
            .map(|(i, l)| {
                node(
                    10 + i,
                    l,
                    "android.widget.Button",
                    &format!(r#""text":"{l}","f":"vVe","a":["tap"]"#),
                )
            })
            .collect()
    }

    #[test]
    fn an_element_that_changes_in_several_ways_is_one_line_with_what_it_shows_now() {
        let mut t = RefTable::default();
        let cell = |text: &str, hint: &str| {
            node(
                1,
                "cell",
                "android.widget.Button",
                &format!(r#""text":"{text}","hint":"{hint}","f":"vVe","a":["tap"]"#),
            )
        };
        let long = "Manage your overall setup and preferences";
        let a = screen(&mut t, &[cell("General", long)]);
        let b = screen(&mut t, &[cell(long, "")]);
        assert_eq!(diff(&a, &b).text(&b), format!("~ @e1 button \"{long}\""));
    }

    #[test]
    fn web_text_that_appears_is_one_line_with_its_links_inline() {
        let mut t = RefTable::default();
        let page = |extra: &[String]| {
            let mut nodes = vec![
                r#"{"p":-1,"w":1,"cls":"android.webkit.WebView","b":[0,0,400,2000],"n":"web","f":"vVe"}"#.to_string(),
                r#"{"p":0,"w":1,"cls":"android.view.View","text":"History","b":[0,0,400,40],"n":"h","f":"vVe","a":["tap"]}"#.to_string(),
            ];
            nodes.extend_from_slice(extra);
            nodes
        };
        let text = |t: &str, n: &str, y: usize| {
            format!(
                r#"{{"p":0,"w":1,"cls":"android.widget.TextView","text":"{t}","b":[0,{y},400,{}],"n":"{n}","f":"vVe"}}"#,
                y + 20
            )
        };
        let link = |t: &str, n: &str, y: usize| {
            format!(
                r#"{{"p":0,"w":1,"cls":"android.view.View","text":"{t}","b":[0,{y},400,{}],"n":"{n}","f":"vVe","a":["tap"]}}"#,
                y + 20
            )
        };
        let a = screen(&mut t, &page(&[]));
        let b = screen(
            &mut t,
            &page(&[
                text("Qualcomm announced it was developing the ", "t1", 100),
                link("Scorpion", "l1", 120),
                text(" CPU in November 2007.", "t2", 140),
            ]),
        );
        assert_eq!(
            diff(&a, &b).text(&b),
            "+ @e3 text \"Qualcomm announced it was developing the [Scorpion @e4] CPU in November 2007.\""
        );
    }

    #[test]
    fn toggle_is_one_line() {
        let mut t = RefTable::default();
        let mut before = buttons(&["A", "B", "C"]);
        before.push(switch(false));
        let mut after = buttons(&["A", "B", "C"]);
        after.push(switch(true));
        let (a, b) = (screen(&mut t, &before), screen(&mut t, &after));
        let d = diff(&a, &b);
        assert_eq!(
            d,
            DiffOrFull::Diff(vec![Change::Changed {
                r#ref: Ref(4),
                field: Field::State,
                from: "off".into(),
                to: "on".into()
            }])
        );
        assert_eq!(d.text(&b), "~ @e4 switch \"Dark theme\" off → on");
    }

    #[test]
    fn navigation_returns_full() {
        let mut t = RefTable::default();
        let a = screen(
            &mut t,
            &buttons(&["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"]),
        );
        let b = screen(
            &mut t,
            &buttons(&["A", "B", "C", "P", "Q", "R", "S", "T", "U", "V"]),
        );
        assert_eq!(diff(&a, &b), DiffOrFull::Full);
    }

    #[test]
    fn small_change_stays_a_diff() {
        let mut t = RefTable::default();
        let a = screen(
            &mut t,
            &buttons(&["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"]),
        );
        let b = screen(
            &mut t,
            &buttons(&["A", "B", "C", "D", "E", "F", "G", "H", "I", "K"]),
        );
        let d = diff(&a, &b);
        assert_eq!(d.text(&b), "- @e10\n+ @e11 button \"K\"");
    }

    #[test]
    fn consecutive_removals_collapse() {
        let mut t = RefTable::default();
        let all: Vec<&str> = (0..20)
            .map(|i| {
                [
                    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P",
                    "Q", "R", "S", "T",
                ][i]
            })
            .collect();
        let a = screen(&mut t, &buttons(&all));
        let kept: Vec<String> = buttons(&all).into_iter().skip(5).collect();
        let b = screen(&mut t, &kept);
        assert_eq!(diff(&a, &b).text(&b), "- @e1 @e2 @e3 @e4 @e5");
    }

    #[test]
    fn value_edit_shows_old_and_new() {
        let mut t = RefTable::default();
        let field = |text: &str| {
            node(
                1,
                "field",
                "android.widget.EditText",
                &format!(r#""desc":"Name","text":"{text}","f":"vVeE","a":["set_text"]"#),
            )
        };
        let a = screen(&mut t, &[field("Bo")]);
        let b = screen(&mut t, &[field("Bob")]);
        assert_eq!(
            diff(&a, &b).text(&b),
            "~ @e1 textbox \"Name\" \"Bo\" → \"Bob\""
        );
    }

    #[test]
    fn secure_edit_hides_values() {
        let mut t = RefTable::default();
        let field = |text: &str| {
            node(
                1,
                "field",
                "android.widget.EditText",
                &format!(r#""desc":"Password","text":"{text}","f":"vVeEP","a":["set_text"]"#),
            )
        };
        let a = screen(&mut t, &[field("hunter")]);
        let b = screen(&mut t, &[field("hunter2")]);
        let d = diff(&a, &b);
        assert_eq!(d.text(&b), "~ @e1 secure field changed");
        assert!(!format!("{d:?}").contains("hunter"));
    }

    #[test]
    fn focus_moving_on_a_non_input_group_is_not_a_change_but_on_a_textbox_it_is() {
        let mut t = RefTable::default();
        let group = |focused: bool| {
            node(
                1,
                "g",
                "android.view.ViewGroup",
                &format!(
                    r#""desc":"Section","f":"{}","a":["tap"]"#,
                    if focused { "vVeF" } else { "vVe" }
                ),
            )
        };
        let a = screen(&mut t, &[group(true)]);
        let b = screen(&mut t, &[group(false)]);
        assert_eq!(diff(&a, &b), DiffOrFull::Diff(vec![]));
        let field = |focused: bool| {
            node(
                1,
                "f",
                "android.widget.EditText",
                &format!(
                    r#""desc":"Name","f":"{}","a":["set_text"]"#,
                    if focused { "vVeEF" } else { "vVeE" }
                ),
            )
        };
        let c = screen(&mut t, &[field(true)]);
        let e = screen(&mut t, &[field(false)]);
        assert_eq!(
            diff(&c, &e).text(&e),
            "~ @e2 textbox \"Name\" focused → (none)"
        );
    }

    #[test]
    fn a_hint_that_only_repeats_the_label_is_not_a_change_when_it_goes() {
        let mut t = RefTable::default();
        let button = |hint: &str| {
            node(
                1,
                "b",
                "android.widget.Button",
                &format!(r#""text":"Go forward","hint":"{hint}","f":"vVe","a":["tap"]"#),
            )
        };
        let a = screen(&mut t, &[button("Go forward")]);
        let b = screen(&mut t, &[button("")]);
        assert_eq!(diff(&a, &b), DiffOrFull::Diff(vec![]));
        let c = screen(&mut t, &[button("Opens the next page")]);
        assert_eq!(
            diff(&b, &c).text(&c),
            "~ @e1 button \"Go forward\" (none) → \"Opens the next page\""
        );
    }

    fn with_keyboard(mut r: Rendered, shown: bool) -> Rendered {
        r.header.keyboard = shown.then_some(Rect {
            x: 0.0,
            y: 500.0,
            w: 400.0,
            h: 1500.0,
        });
        r
    }

    #[test]
    fn a_keyboard_that_goes_away_is_one_short_line_and_the_elements_it_starts_to_cover_are_one_ref_line()
     {
        let mut t = RefTable::default();
        let labels = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L"];
        let nodes = buttons(&labels);
        let covered = with_keyboard(screen(&mut t, &nodes), true);
        let clear = with_keyboard(screen(&mut t, &nodes), false);
        let gone = diff(&covered, &clear);
        assert_eq!(gone.text(&clear), "keyboard hidden");
        assert_eq!(
            diff(&clear, &covered).text(&covered),
            "~ @e1 @e2 @e3 @e4 @e5 @e6 @e7 @e8 @e9 @e10 @e11 @e12 (none) → under-keyboard"
        );
    }

    const TEN: [&str; 10] = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"];

    #[test]
    fn closing_a_dialog_that_replaced_the_screen_says_the_screen_is_back_instead_of_printing_it_again()
     {
        let mut t = RefTable::default();
        let a = screen(&mut t, &buttons(&TEN));
        let dialog = screen(&mut t, &buttons(&["OK", "Cancel"]));
        let back = screen(&mut t, &buttons(&TEN));
        assert_eq!(diff(&dialog, &back), DiffOrFull::Full);
        let d = diff_below(&dialog, &back, Some(&a));
        assert_eq!(
            d.text(&back),
            "the screen before the last one is back, unchanged"
        );
    }

    #[test]
    fn a_screen_that_came_back_with_a_few_changes_lists_them() {
        let mut t = RefTable::default();
        let a = screen(&mut t, &buttons(&TEN));
        let dialog = screen(&mut t, &buttons(&["OK", "Cancel"]));
        let mut later = TEN;
        later[9] = "K";
        let back = screen(&mut t, &buttons(&later));
        assert_eq!(
            diff_below(&dialog, &back, Some(&a)).text(&back),
            "the screen before the last one is back, except:\n- @e10\n+ @e13 button \"K\""
        );
    }

    #[test]
    fn a_different_screen_after_a_dialog_is_new_and_so_is_any_screen_with_nothing_below() {
        let mut t = RefTable::default();
        let a = screen(&mut t, &buttons(&TEN));
        let dialog = screen(&mut t, &buttons(&["OK", "Cancel"]));
        let other = screen(
            &mut t,
            &buttons(&["P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y"]),
        );
        assert_eq!(diff_below(&dialog, &other, Some(&a)), DiffOrFull::Full);
        let back = screen(&mut t, &buttons(&TEN));
        assert_eq!(diff_below(&dialog, &back, None), DiffOrFull::Full);
    }

    #[test]
    fn no_change_is_empty() {
        let mut t = RefTable::default();
        let a = screen(&mut t, &buttons(&["A"]));
        let b = screen(&mut t, &buttons(&["A"]));
        assert_eq!(diff(&a, &b), DiffOrFull::Diff(vec![]));
        assert_eq!(diff(&a, &b).text(&b), "");
    }
}
