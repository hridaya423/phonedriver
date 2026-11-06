//! What one named session remembers about one device: its refs and the last screen it was shown.

use phone_use_core::element::{Element, Role};
use phone_use_core::error::Error;
use phone_use_core::geometry::Point;
use phone_use_core::refs::RefTable;
use phone_use_core::render::{Rendered, TextMatch};
use phone_use_core::similar::{SUGGEST_WITHIN, normalized_distance};

use crate::wire::{PointJson, Target};

#[derive(Default)]
pub struct Session {
    pub refs: RefTable,
    pub last: Option<Rendered>,
    /// The last snapshot merged text from pixels, so target resolution does too.
    pub ocr: bool,
}

pub struct Resolved {
    pub at: Point,
    pub element: Option<Element>,
    /// The element's rendered line, such as `@e6 button "Display & touch"`.
    pub line: Option<String>,
    /// The keyboard covers the element's bounds.
    pub under_keyboard: bool,
}

impl Session {
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
            }),
            Target::Ref { r#ref } => match now.index_of(*r#ref) {
                Some(i) => Ok(element_at(now, i)),
                None => Err(Error::StaleRef {
                    r#ref: *r#ref,
                    closest: self.closest(*r#ref, now),
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

    fn closest(&self, r: phone_use_core::element::Ref, now: &Rendered) -> Option<String> {
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
    }
}

fn lines(now: &Rendered, refs: &[phone_use_core::element::Ref]) -> Vec<String> {
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

/// Exact label first (core), then case-insensitive exact, then substring (architecture 10.1).
fn find_text(now: &Rendered, text: &str, role: Option<phone_use_core::element::Role>) -> TextMatch {
    let exact = now.resolve_text(text, role);
    if exact != TextMatch::None {
        return exact;
    }
    let q = text.to_lowercase();
    for matches in [
        (|l: &str, q: &str| l == q) as fn(&str, &str) -> bool,
        |l, q| l.contains(q),
    ] {
        let mut labels: Vec<&str> = Vec::new();
        for e in &now.elements {
            for l in e
                .label
                .iter()
                .map(String::as_str)
                .chain(e.hint.as_deref().into_iter().flat_map(|h| h.split(" · ")))
            {
                if matches(&l.to_lowercase(), &q) && !labels.contains(&l) {
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
