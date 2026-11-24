//! What a `check` step reads for its target: the value a person would say the screen shows for it (decision D113).

use phonedriver_core::element::{Element, ElementState, Role};
use phonedriver_core::render::{Rendered, quote, shown_hint};
use phonedriver_core::text::fold;

/// A target's name and the value read for it. `value` is `None` when the target is on the screen and nothing
/// reads as its value.
#[derive(Debug, PartialEq)]
pub struct Reading {
    pub name: String,
    pub value: Option<String>,
}

impl Reading {
    /// `"Total" = "€24.99"`, or `"Order placed" is shown`. A value that reads as an instruction is marked as a
    /// snapshot line marks it.
    pub fn line(&self) -> String {
        match &self.value {
            Some(v) => format!(
                "{} = {}{}",
                self.name,
                quote(v),
                if phonedriver_core::inject::looks_like_instruction(v) {
                    " (ai-instruction)"
                } else {
                    ""
                }
            ),
            None => format!("{} is shown", self.name),
        }
    }

    /// The value equals `expected` once both are folded (case, width, typographic quotes) and their runs of spaces
    /// are one space.
    pub fn equals(&self, expected: &str) -> bool {
        let plain = |t: &str| fold(t).split_whitespace().collect::<Vec<_>>().join(" ");
        self.value
            .as_deref()
            .is_some_and(|v| plain(v) == plain(expected))
    }
}

/// Reads element `i` of `now`: on or off, its value, or the second text its snapshot line shows (a settings row's
/// summary). Failing those, `asked`, the text of a text target, finds it: the part of a label beside that text, or
/// the element beside a label that is only that text. A password field's text is never read.
pub fn read(now: &Rendered, i: usize, asked: Option<&str>) -> Result<Reading, &'static str> {
    let e = &now.elements[i];
    if e.role == Role::SecureField || e.state.contains(ElementState::PASSWORD) {
        return Err("a password field's text is never read");
    }
    let name = match asked {
        Some(text) => quote(text),
        None => format!("@{}", e.r#ref),
    };
    let own = if e.state.contains(ElementState::CHECKABLE) {
        Some(if e.state.contains(ElementState::CHECKED) {
            "on".to_string()
        } else {
            "off".to_string()
        })
    } else {
        e.value
            .clone()
            .filter(|v| !v.trim().is_empty() && e.label.as_deref() != Some(v.as_str()))
            .or_else(|| shown_hint(e).filter(|_| !e.role.is_text_entry()))
    };
    let label = e.label.as_deref().unwrap_or_default();
    let value = match (own, asked) {
        (Some(v), _) => Some(v),
        (None, None) => Some(label.to_string()).filter(|l| !l.is_empty()),
        (None, Some(text)) => match beside(label, text) {
            Beside::Rest(rest) => Some(rest),
            Beside::Whole => Some(label.to_string()),
            Beside::Nothing => neighbour(now, i).map(|n| n.to_string()),
        },
    };
    Ok(Reading { name, value })
}

/// Of elements that answer to one text and sit inside one another (a settings row and the switch in it), the one a
/// check reads: the one that is on or off, else the outermost. `None` when they are apart.
pub fn nested(now: &Rendered, found: &[usize]) -> Option<usize> {
    let within = |inner: usize, outer: usize| {
        std::iter::successors(Some(inner), |&j| now.parents[j]).any(|j| j == outer)
    };
    let outer = *found
        .iter()
        .find(|&&outer| found.iter().all(|&i| within(i, outer)))?;
    let mut set = found
        .iter()
        .copied()
        .filter(|&i| now.elements[i].state.contains(ElementState::CHECKABLE));
    match (set.next(), set.next()) {
        (Some(one), None) => Some(one),
        _ => Some(outer),
    }
}

enum Beside {
    /// The label is the text and more: this is the more.
    Rest(String),
    /// The label holds the text in its middle, so the whole label is what the screen says.
    Whole,
    /// The label is the text and nothing else.
    Nothing,
}

const SEPARATORS: &[char] = &[':', '·', '-', '\u{2013}', '\u{2014}', '=', '|'];

fn beside(label: &str, text: &str) -> Beside {
    let want = fold(text);
    if fold(label) == want {
        return Beside::Nothing;
    }
    let trimmed = |rest: &str| {
        rest.trim_matches(|c: char| c.is_whitespace() || SEPARATORS.contains(&c))
            .to_string()
    };
    let cuts = label.char_indices().map(|(at, _)| at).chain([label.len()]);
    for at in cuts {
        let rest = if fold(&label[..at]) == want {
            trimmed(&label[at..])
        } else if fold(&label[at..]) == want {
            trimmed(&label[..at])
        } else {
            continue;
        };
        if !rest.is_empty() {
            return Beside::Rest(rest);
        }
    }
    Beside::Whole
}

/// The text beside a label that has no value of its own: the nearest labelled element of the same parent on the
/// label's row and to its right, or else the text right under it. A control (a button under a heading) is not a
/// value.
fn neighbour(now: &Rendered, i: usize) -> Option<&str> {
    let e = &now.elements[i];
    fn text(n: &Element) -> Option<&str> {
        n.value.as_deref().or(n.label.as_deref())
    }
    let siblings = || {
        (0..now.elements.len())
            .filter(move |&j| j != i && now.parents[j] == now.parents[i])
            .map(|j| &now.elements[j])
            .filter(|n| text(n).is_some_and(|t| !t.trim().is_empty()))
            .filter(|n| !n.state.contains(ElementState::PASSWORD) && n.role != Role::SecureField)
    };
    let mid = e.bounds.y + e.bounds.h / 2.0;
    let right = siblings()
        .filter(|n| n.bounds.y <= mid && mid <= n.bounds.y + n.bounds.h)
        .filter(|n| n.bounds.x >= e.bounds.x + e.bounds.w - 1.0)
        .min_by(|a, b| a.bounds.x.total_cmp(&b.bounds.x));
    let below = || {
        siblings()
            .filter(|n| n.actions.is_empty())
            .filter(|n| n.bounds.y >= e.bounds.y + e.bounds.h - 1.0)
            .filter(|n| {
                n.bounds.x < e.bounds.x + e.bounds.w && e.bounds.x < n.bounds.x + n.bounds.w
            })
            .min_by(|a, b| a.bounds.y.total_cmp(&b.bounds.y))
            .filter(|n| n.bounds.y - (e.bounds.y + e.bounds.h) <= e.bounds.h)
    };
    right.or_else(below).and_then(text)
}

#[cfg(test)]
mod tests {
    use phonedriver_core::geometry::Scale;
    use phonedriver_core::raw::RawTree;
    use phonedriver_core::refs::RefTable;
    use phonedriver_core::render::Header;

    use super::*;

    fn screen(nodes: &[&str]) -> Rendered {
        let mut all = vec![
            r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}"#
                .to_string(),
        ];
        all.extend(nodes.iter().map(|n| n.to_string()));
        let json = format!(
            r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[{}]}}"#,
            all.join(",")
        );
        let tree = RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap();
        let nodes = phonedriver_core::prune::prune(&tree);
        let refs = RefTable::default().assign(&tree, &nodes);
        Rendered::build(nodes, &refs, Header::default())
    }

    fn text(label: &str, b: [i32; 4]) -> String {
        format!(
            r#"{{"p":0,"w":1,"cls":"android.widget.TextView","text":"{label}","b":[{},{},{},{}],"f":"vVe"}}"#,
            b[0], b[1], b[2], b[3]
        )
    }

    fn reading(now: &Rendered, label: &str, asked: Option<&str>) -> Reading {
        let i = now
            .elements
            .iter()
            .position(|e| e.label.as_deref().is_some_and(|l| l.starts_with(label)))
            .unwrap();
        read(now, i, asked).unwrap()
    }

    #[test]
    fn a_label_that_holds_the_text_and_more_gives_the_rest() {
        for (label, value) in [
            ("Total €24.99", "€24.99"),
            ("Total: €24.99", "€24.99"),
            ("TOTAL · €24.99", "€24.99"),
            ("€24.99 total", "€24.99"),
        ] {
            let now = screen(&[&text(label, [0, 100, 360, 160])]);
            let r = reading(&now, &label[..3], Some("Total"));
            assert_eq!(r.value.as_deref(), Some(value), "{label}");
            assert_eq!(r.line(), format!("\"Total\" = \"{value}\""));
        }
    }

    #[test]
    fn a_label_alone_takes_the_text_on_its_row_and_else_the_text_right_under_it() {
        let row = screen(&[
            &text("Subtotal", [0, 40, 180, 100]),
            &text("€20.00", [180, 40, 360, 100]),
            &text("Total", [0, 100, 180, 160]),
            &text("€24.99", [180, 100, 360, 160]),
        ]);
        assert_eq!(
            reading(&row, "Total", Some("Total")).value.as_deref(),
            Some("€24.99")
        );
        let stacked = screen(&[
            &text("Total", [0, 100, 360, 130]),
            &text("€24.99", [0, 130, 360, 160]),
            &text("Delivery", [0, 300, 360, 330]),
        ]);
        assert_eq!(
            reading(&stacked, "Total", Some("Total")).value.as_deref(),
            Some("€24.99")
        );
        assert_eq!(
            reading(&stacked, "Delivery", Some("Delivery")).line(),
            "\"Delivery\" is shown"
        );
    }

    #[test]
    fn a_button_under_a_heading_is_not_its_value() {
        let now = screen(&[
            &text("Total", [0, 100, 360, 130]),
            r#"{"p":0,"w":1,"cls":"android.widget.Button","text":"Pay now","b":[0,130,360,160],"f":"vVe","a":["tap"]}"#,
        ]);
        assert_eq!(reading(&now, "Total", Some("Total")).value, None);
    }

    #[test]
    fn a_switch_reads_on_or_off_and_a_field_its_text_and_a_password_field_nothing() {
        let now = screen(&[
            r#"{"p":0,"w":1,"cls":"android.widget.Switch","text":"Dark theme","b":[0,40,360,100],"f":"vVecC","a":["tap"]}"#,
            r#"{"p":0,"w":1,"cls":"android.widget.EditText","text":"ada@example.com","desc":"Email","b":[0,100,360,160],"f":"vVeE","a":["tap","set_text"]}"#,
            r#"{"p":0,"w":1,"cls":"android.widget.EditText","text":"hunter2","desc":"Password","b":[0,160,360,220],"f":"vVeEP","a":["tap","set_text"]}"#,
        ]);
        assert_eq!(
            reading(&now, "Dark theme", Some("Dark theme"))
                .value
                .as_deref(),
            Some("on")
        );
        let email = reading(&now, "Email", Some("Email"));
        assert_eq!(email.value.as_deref(), Some("ada@example.com"));
        assert!(email.equals("ADA@example.com") && !email.equals("ada@example.org"));
        let i = now
            .elements
            .iter()
            .position(|e| e.label.as_deref() == Some("Password"))
            .unwrap();
        assert!(read(&now, i, Some("Password")).is_err());
    }

    fn settings_row() -> Rendered {
        screen(&[
            r#"{"p":0,"w":1,"cls":"android.widget.LinearLayout","b":[0,100,360,180],"f":"vVe","a":["tap"]}"#,
            r#"{"p":1,"w":1,"cls":"android.widget.TextView","text":"Dark theme","b":[20,100,280,140],"f":"vVe"}"#,
            r#"{"p":1,"w":1,"cls":"android.widget.TextView","text":"Will turn on at sunset","b":[20,140,280,180],"f":"vVe"}"#,
            r#"{"p":1,"w":1,"cls":"android.widget.Switch","desc":"Dark theme","sd":"Off","b":[300,110,350,170],"f":"vVce","a":["tap"]}"#,
            r#"{"p":0,"w":1,"cls":"android.widget.LinearLayout","b":[0,180,360,260],"f":"vVe","a":["tap"]}"#,
            r#"{"p":5,"w":1,"cls":"android.widget.TextView","text":"Screen timeout","b":[20,180,280,220],"f":"vVe"}"#,
            r#"{"p":5,"w":1,"cls":"android.widget.TextView","text":"30 seconds","b":[20,220,280,260],"f":"vVe"}"#,
        ])
    }

    #[test]
    fn a_row_reads_the_line_under_its_title() {
        let now = settings_row();
        assert_eq!(
            reading(&now, "Screen timeout", Some("Screen timeout")).line(),
            "\"Screen timeout\" = \"30 seconds\""
        );
    }

    #[test]
    fn of_a_row_and_the_switch_in_it_the_switch_is_read_and_of_two_rows_neither() {
        let now = settings_row();
        let named = |text: &str| -> Vec<usize> {
            (0..now.elements.len())
                .filter(|&i| now.elements[i].label.as_deref() == Some(text))
                .collect()
        };
        let both = named("Dark theme");
        assert_eq!(both.len(), 2, "{:?}", now.elements);
        let i = nested(&now, &both).expect("one inside the other");
        assert_eq!(
            read(&now, i, Some("Dark theme")).unwrap().line(),
            "\"Dark theme\" = \"off\""
        );
        let apart = [both[0], named("Screen timeout")[0]];
        assert_eq!(nested(&now, &apart), None);
    }

    #[test]
    fn a_ref_reads_the_element_itself() {
        let now = screen(&[&text("€24.99", [0, 100, 360, 160])]);
        let r = reading(&now, "€24.99", None);
        assert_eq!(r.value.as_deref(), Some("€24.99"));
        assert!(r.line().starts_with("@e") && r.line().ends_with("= \"€24.99\""));
    }
}
