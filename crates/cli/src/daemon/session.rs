//! What one named session remembers about one device: its refs and the last screen it was shown.

use phonedriver_core::diff::{Change, DiffOrFull, diff_below};
use phonedriver_core::element::{Element, Ref, Role};
use phonedriver_core::error::{Category, Direction, Error};
use phonedriver_core::geometry::Point;
use phonedriver_core::refs::RefTable;
use phonedriver_core::render::{Rendered, TextMatch};
use phonedriver_core::similar::{SUGGEST_WITHIN, normalized_distance};

use crate::wire::{PointJson, Target};

/// The screen a new one replaced, kept while the agent is still shown that new one (`covering` is its refs).
struct Below {
    covering: Vec<Ref>,
    screen: Rendered,
}

fn refs_of(r: &Rendered) -> Vec<Ref> {
    r.elements.iter().map(|e| e.r#ref).collect()
}

#[derive(Default)]
pub struct Session {
    pub refs: RefTable,
    pub last: Option<Rendered>,
    below: Option<Below>,
    /// The last snapshot merged text from pixels, so target resolution does too.
    pub ocr: bool,
    /// The categories of guarded tap the human allowed for the rest of this session; it ends with the daemon.
    pub guard_allowed: Vec<Category>,
}

pub struct Resolved {
    pub at: Point,
    pub element: Option<Element>,
    /// The element's rendered line, such as `@e6 button "Display & touch"`.
    pub line: Option<String>,
    /// The keyboard covers the element's bounds.
    pub under_keyboard: bool,
    /// A floating toolbar covers the element's center: its label and the way to scroll the element clear.
    pub covered: Option<(String, Direction)>,
}

impl Session {
    /// What `now` changed since the screen the agent was last shown. A dialog or menu is a window of its own whose
    /// tree has none of the screen under it, so when it closes the screen is back as a new one: say so instead.
    pub fn diff_for_reply(&mut self, now: &Rendered) -> Option<DiffOrFull> {
        let prev = self.last.as_ref()?;
        let below = self
            .below
            .as_ref()
            .filter(|b| b.covering == refs_of(prev))
            .map(|b| &b.screen);
        let had_below = below.is_some();
        let reply = diff_below(prev, now, below);
        match &reply {
            DiffOrFull::Full => {
                self.below = Some(Below {
                    covering: refs_of(now),
                    screen: prev.clone(),
                });
            }
            DiffOrFull::Diff(changes) if matches!(changes.first(), Some(Change::Uncovered)) => {
                self.below = None;
            }
            DiffOrFull::Diff(_) if had_below => {
                if let Some(b) = self.below.as_mut() {
                    b.covering = refs_of(now);
                }
            }
            DiffOrFull::Diff(_) => {}
        }
        Some(reply)
    }

    /// Finds where a target is on `now`. A ref the screen no longer has fails with the closest current element.
    pub fn resolve(&self, target: &Target, now: &Rendered) -> Result<Resolved, Error> {
        match target {
            Target::Point {
                point: PointJson { x, y },
            } => Ok(Resolved {
                at: Point { x: *x, y: *y },
                element: None,
                line: None,
                under_keyboard: false,
                covered: None,
            }),
            Target::Ref { r#ref } => match now.index_of(*r#ref) {
                Some(i) => Ok(element_at(now, i)),
                None => Err(Error::StaleRef {
                    r#ref: *r#ref,
                    closest: self.closest(*r#ref, now),
                    unseen: self.unseen(*r#ref),
                }),
            },
            Target::Text { text, role } => match find_text(now, text, *role) {
                TextMatch::One { r#ref, .. } => {
                    let i = now.index_of(r#ref).ok_or_else(|| not_found(now, text))?;
                    Ok(element_at(now, i))
                }
                TextMatch::None => Err(not_found(now, text)),
                TextMatch::Many(refs) => {
                    let actionable: Vec<_> = refs
                        .iter()
                        .copied()
                        .filter(|r| {
                            now.index_of(*r)
                                .is_some_and(|i| !now.elements[i].actions.is_empty())
                        })
                        .collect();
                    match actionable.as_slice() {
                        [one] => Ok(element_at(
                            now,
                            now.index_of(*one).ok_or_else(|| not_found(now, text))?,
                        )),
                        _ => Err(Error::Ambiguous {
                            query: text.clone(),
                            matches: lines(now, &refs),
                        }),
                    }
                }
            },
        }
    }

    /// The rendered lines of the elements whose label or hint contains `text`, in any case.
    pub fn find(&self, now: &Rendered, text: &str, role: Option<Role>) -> Vec<String> {
        let q = text.to_lowercase();
        now.elements
            .iter()
            .filter(|e| role.is_none_or(|r| e.role == r))
            .filter(|e| parts(e).any(|p| p.to_lowercase().contains(&q)))
            .map(|e| now.line(e))
            .collect()
    }

    /// The session was never shown this ref: it has seen no screen yet, or never issued a ref that high.
    pub fn unseen(&self, r: phonedriver_core::element::Ref) -> bool {
        self.last.is_none() || !self.refs.issued(r)
    }

    fn closest(&self, r: phonedriver_core::element::Ref, now: &Rendered) -> Option<String> {
        let old = self.last.as_ref()?.elements.iter().find(|e| e.r#ref == r)?;
        let label = old.label.as_deref()?.to_lowercase();
        now.elements
            .iter()
            .filter(|e| e.label.as_deref().map(str::to_lowercase).as_deref() == Some(&label))
            .min_by(|a, b| {
                let da = a.bounds.center().distance(old.bounds.center());
                let db = b.bounds.center().distance(old.bounds.center());
                da.total_cmp(&db)
            })
            .map(|e| now.line(e))
    }
}

fn element_at(now: &Rendered, i: usize) -> Resolved {
    let e = &now.elements[i];
    Resolved {
        at: e.bounds.center(),
        element: Some(e.clone()),
        line: Some(now.line(e)),
        under_keyboard: e.role != Role::Keyboard
            && now.header.keyboard.is_some_and(|k| k.contains(e.bounds)),
        covered: now
            .covered_by(i)
            .map(|(bar, toward)| (bar.label.clone().unwrap_or_default(), toward)),
    }
}

fn lines(now: &Rendered, refs: &[phonedriver_core::element::Ref]) -> Vec<String> {
    refs.iter()
        .filter_map(|r| now.index_of(*r))
        .map(|i| now.line(&now.elements[i]))
        .collect()
}

fn parts(e: &Element) -> impl Iterator<Item = &str> {
    e.label
        .iter()
        .map(String::as_str)
        .chain(e.hint.as_deref().into_iter().flat_map(|h| h.split(" · ")))
}

fn not_found(now: &Rendered, text: &str) -> Error {
    let q = text.to_lowercase();
    let mut close: Vec<(f32, &Element)> = now
        .elements
        .iter()
        .filter_map(|e| {
            parts(e)
                .map(|p| {
                    let contained = p.to_lowercase().contains(&q) || q.contains(&p.to_lowercase());
                    let d = normalized_distance(text, p);
                    if contained { d.min(SUGGEST_WITHIN) } else { d }
                })
                .filter(|d| *d <= SUGGEST_WITHIN)
                .min_by(f32::total_cmp)
                .map(|d| (d, e))
        })
        .collect();
    close.sort_by(|a, b| a.0.total_cmp(&b.0));
    let similar = close
        .into_iter()
        .take(5)
        .map(|(_, e)| now.line(e))
        .collect();
    Error::NotFound {
        query: text.to_string(),
        similar,
    }
}

/// Exact label first (core), then the same label in any case and spacing, then a label that holds the text as whole
/// words and not much else (decision D33).
fn find_text(
    now: &Rendered,
    text: &str,
    role: Option<phonedriver_core::element::Role>,
) -> TextMatch {
    let exact = now.resolve_text(text, role);
    if exact != TextMatch::None {
        return exact;
    }
    let q = normalize(text);
    for matches in [
        (|l: &str, q: &str| l == q) as fn(&str, &str) -> bool,
        holds_as_words,
    ] {
        let mut labels: Vec<&str> = Vec::new();
        for e in &now.elements {
            for l in parts(e) {
                if matches(&normalize(l), &q) && !labels.contains(&l) {
                    labels.push(l);
                }
            }
        }
        let mut refs = Vec::new();
        for l in labels {
            match now.resolve_text(l, role) {
                TextMatch::One { r#ref, .. } => refs.push(r#ref),
                TextMatch::Many(many) => refs.extend(many),
                TextMatch::None => {}
            }
        }
        refs.sort();
        refs.dedup();
        match refs.as_slice() {
            [] => continue,
            [r] => {
                let at = now.elements[now.index_of(*r).unwrap_or(0)].bounds.center();
                return TextMatch::One { r#ref: *r, at };
            }
            _ => return TextMatch::Many(refs),
        }
    }
    TextMatch::None
}

const EXTRA_WORDS: usize = 4;

fn normalize(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn joins_a_word(c: char) -> bool {
    let unspaced =
        matches!(c, '\u{0e00}'..='\u{0e7f}' | '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{9fff}');
    c.is_alphanumeric() && !unspaced
}

/// `label` has `query` at word boundaries and at most `EXTRA_WORDS` other words. Both are normalized.
fn holds_as_words(label: &str, query: &str) -> bool {
    let (Some(first), Some(last)) = (query.chars().next(), query.chars().next_back()) else {
        return false;
    };
    let extra = label
        .split(' ')
        .count()
        .saturating_sub(query.split(' ').count());
    extra <= EXTRA_WORDS
        && label.match_indices(query).any(|(at, _)| {
            let before = label[..at].chars().next_back();
            let after = label[at + query.len()..].chars().next();
            !(joins_a_word(first) && before.is_some_and(joins_a_word))
                && !(joins_a_word(last) && after.is_some_and(joins_a_word))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use phonedriver_core::geometry::Scale;
    use phonedriver_core::prune::prune;
    use phonedriver_core::raw::RawTree;
    use phonedriver_core::render::Header;

    fn screen(labels: &[&str]) -> Rendered {
        let nodes: Vec<String> = labels
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let y = i * 40;
                format!(
                    r#"{{"p":-1,"w":1,"cls":"android.widget.Button","text":{},"b":[0,{y},400,{}],"f":"vVe","a":["tap"]}}"#,
                    serde_json::to_string(l).unwrap(),
                    y + 30
                )
            })
            .collect();
        let json = format!(
            r#"{{"windows":[{{"id":1,"type":"application","bounds":[0,0,400,800]}}],"nodes":[{}]}}"#,
            nodes.join(",")
        );
        let tree = RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap();
        let nodes = prune(&tree);
        let refs = RefTable::default().assign(&tree, &nodes);
        Rendered::build(nodes, &refs, Header::default())
    }

    fn text(t: &str) -> Target {
        Target::Text {
            text: t.into(),
            role: None,
        }
    }

    fn picked(labels: &[&str], target: &str) -> Result<String, Error> {
        let now = screen(labels);
        Session::default()
            .resolve(&text(target), &now)
            .map(|r| r.line.unwrap_or_default())
    }

    #[test]
    fn a_target_is_not_found_inside_a_longer_number() {
        let err = picked(&["Row 364", "Row 30"], "Row 3").unwrap_err();
        let Error::NotFound { similar, .. } = err else {
            panic!("{err:?}")
        };
        assert!(similar.iter().any(|l| l.contains("Row 364")), "{similar:?}");
    }

    #[test]
    fn an_exact_label_wins_over_longer_labels_that_contain_it() {
        let line = picked(&["Row 364", "Row 3", "Row 30"], "Row 3").unwrap();
        assert!(line.ends_with("\"Row 3\""), "{line}");
        let line = picked(&["List view", "List"], "list").unwrap();
        assert!(line.ends_with("\"List\""), "{line}");
    }

    #[test]
    fn a_short_target_is_not_found_inside_a_long_sentence() {
        let sentence =
            "Allow phonedriver to read the list of apps on this device so that it can open them";
        let err = picked(&[sentence, "Allow"], "list").unwrap_err();
        assert!(matches!(err, Error::NotFound { .. }), "{err:?}");
        let err = picked(&[sentence], "list").unwrap_err();
        assert!(matches!(err, Error::NotFound { .. }), "{err:?}");
    }

    #[test]
    fn a_target_matches_whole_words_of_a_short_label() {
        let line = picked(&["Network & internet", "Display"], "network").unwrap();
        assert!(line.ends_with("\"Network & internet\""), "{line}");
        let line = picked(&["Save and continue", "Cancel"], "Save").unwrap();
        assert!(line.contains("Save and continue"), "{line}");
        assert!(picked(&["Network & internet"], "net").is_err());
    }

    #[test]
    fn whitespace_does_not_tell_two_labels_apart() {
        let line = picked(&["Dark \u{a0} theme"], " dark theme").unwrap();
        assert!(line.contains("theme"), "{line}");
    }

    #[test]
    fn text_without_spaces_still_matches_inside_a_label() {
        let line = picked(&["网络设置", "显示"], "设置").unwrap();
        assert!(line.contains("网络设置"), "{line}");
    }

    #[test]
    fn several_whole_word_matches_are_ambiguous_and_listed() {
        let err = picked(&["Row 1", "Row 2"], "Row").unwrap_err();
        let Error::Ambiguous { matches, .. } = err else {
            panic!("{err:?}")
        };
        assert_eq!(matches.len(), 2, "{matches:?}");
    }
}
