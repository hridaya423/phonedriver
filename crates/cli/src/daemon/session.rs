//! What one named session remembers about one device: its refs and the last screen it was shown.

use phonedriver_core::diff::{Change, DiffOrFull, diff_below};
use phonedriver_core::element::{Element, Ref, Role};
use phonedriver_core::error::{Category, Direction, Error};
use phonedriver_core::geometry::{Point, Rect};
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
    /// Text fields the agent was shown that later screens dropped, newest last, with the ref they had.
    gone: Vec<Element>,
}

/// How many dropped text fields a session remembers.
const GONE_FIELDS: usize = 32;

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
    /// Replaces the screen the agent was last shown, remembering the text fields it drops. A keyboard that opens
    /// shrinks a form's list, and the fields below it leave the tree while the agent still holds their refs.
    pub fn set_last(&mut self, next: Option<Rendered>) {
        match (&self.last, &next) {
            (Some(prev), Some(now)) if prev.header.app == now.header.app => {
                let dropped = prev
                    .elements
                    .iter()
                    .filter(|e| e.role.is_text_entry() && e.label.is_some())
                    .filter(|e| now.index_of(e.r#ref).is_none())
                    .cloned();
                self.gone.retain(|g| now.index_of(g.r#ref).is_none());
                self.gone.extend(dropped);
                let over = self.gone.len().saturating_sub(GONE_FIELDS);
                self.gone.drain(..over);
            }
            _ => self.gone.clear(),
        }
        self.last = next;
    }

    /// The text field `r` named before a later screen dropped it.
    pub fn gone_field(&self, r: Ref) -> Option<&Element> {
        self.gone.iter().rev().find(|e| e.r#ref == r)
    }

    /// The one dropped text field whose label contains `text`, in any case.
    pub fn gone_field_named(&self, text: &str) -> Option<&Element> {
        let q = text.to_lowercase();
        let mut named = self.gone.iter().rev().filter(|e| {
            e.label
                .as_deref()
                .is_some_and(|l| l.to_lowercase().contains(&q))
        });
        let first = named.next()?;
        named
            .all(|e| e.role == first.role && e.label == first.label)
            .then_some(first)
    }

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

    /// `resolve` for a search (`scroll --until`, `wait`): a text may sit anywhere in a long sentence, and when it is
    /// on screen more than once the first one ends the search. With `within`, only that element and what is inside it
    /// count: a scroll looks in what it scrolls, not at a toolbar that slides back over it (Chrome's address bar, whose
    /// hint is "Search Google or type URL").
    pub fn seek(
        &self,
        target: &Target,
        now: &Rendered,
        within: Option<Ref>,
    ) -> Result<Resolved, Error> {
        let Target::Text { text, role } = target else {
            return self.resolve(target, now);
        };
        let refs: Vec<_> = std::iter::once(text.as_str())
            .chain(either(text))
            .flat_map(|t| match find_text(now, t, *role, None) {
                TextMatch::None => Vec::new(),
                TextMatch::One { r#ref, .. } => vec![r#ref],
                TextMatch::Many(refs) => refs,
            })
            .collect();
        let holder = within.and_then(|r| now.index_of(r));
        let inside = |i: usize| {
            holder
                .is_none_or(|h| std::iter::successors(Some(i), |&j| now.parents[j]).any(|j| j == h))
        };
        refs.iter()
            .filter_map(|r| now.index_of(*r))
            .filter(|&i| inside(i))
            .min()
            .map(|i| element_at(now, i))
            .ok_or_else(|| not_found(now, text))
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
            Target::Text { text, role } => match find_text(now, text, *role, Some(EXTRA_WORDS)) {
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
        let qs: Vec<String> = std::iter::once(text)
            .chain(either(text))
            .map(str::to_lowercase)
            .collect();
        now.elements
            .iter()
            .filter(|e| role.is_none_or(|r| e.role == r))
            .filter(|e| {
                parts(e).any(|p| {
                    let p = p.to_lowercase();
                    qs.iter().any(|q| p.contains(q))
                })
            })
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
    let mut at = free_point(now, i);
    // Safari on iOS, page zoomed after a field took focus: WebKit reports an element that runs past the left edge with
    // x = 0 and its whole width, so the middle of that frame can be empty page while its visible part hugs the edge.
    if e.bounds.x <= 0.0 && now.in_web_view(e.r#ref) {
        at.x = at.x.min(e.bounds.x + WEB_LEFT_EDGE_TAP);
    }
    // iOS Contacts: the "add phone" row sat half under the keyboard, and taps at its center typed letters into the
    // focused field.
    if let Some(k) = now.header.keyboard.filter(|_| e.role != Role::Keyboard)
        && at.y >= k.y
        && !now.under_keyboard(e)
    {
        at.y = (e.bounds.y + k.y) / 2.0;
    }
    Resolved {
        at,
        element: Some(e.clone()),
        line: Some(now.line(e)),
        under_keyboard: now.under_keyboard(e)
            || (e.role != Role::Keyboard && now.header.keyboard.is_some_and(|k| at.y >= k.y)),
        covered: now
            .covered_by(i)
            .map(|(bar, toward)| (bar.label.clone().unwrap_or_default(), toward)),
    }
}

/// How far in from the screen's left edge a web element that starts there is tapped.
const WEB_LEFT_EDGE_TAP: f32 = 8.0;

/// The texts of a search for `A|B`; agents write alternatives that way. Empty when there is no `|`.
fn either(text: &str) -> Vec<&str> {
    if !text.contains('|') {
        return Vec::new();
    }
    text.split('|')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect()
}

/// Points of free height that hold a line of text.
const MIN_BAND: f32 = 12.0;

/// A control inside an element takes a tap at its own bounds, and so may a panel around it. The Play Store result card
/// spans the whole result: an Install button, a row of ratings that scrolls, and an "Ask Play" panel whose text does
/// not open the app. A card's own title is on top, so an element with controls inside is tapped in the middle of the
/// first free band on its vertical center line. Failing that, the tap goes to the free point nearest the center, and
/// with none free, to the center.
fn free_point(now: &Rendered, i: usize) -> Point {
    let b = now.elements[i].bounds;
    let center = b.center();
    let mut inner: Vec<Rect> = Vec::new();
    let mut stack = now.elements[i].children.clone();
    while let Some(c) = stack.pop() {
        let child = &now.elements[c];
        if child.actions.is_empty() {
            stack.extend(&child.children);
        } else {
            inner.push(child.bounds);
        }
    }
    let free = |p: Point| {
        !inner
            .iter()
            .any(|r| p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h)
    };
    if inner.is_empty() {
        return center;
    }
    const STEP: f32 = 4.0;
    let mut run: Option<(f32, f32)> = None;
    let mut y = b.y + STEP / 2.0;
    while y < b.y + b.h {
        let open = free(Point { x: center.x, y });
        match (open, run) {
            (true, None) => run = Some((y, y)),
            (true, Some((top, _))) => run = Some((top, y)),
            (false, Some((top, bottom))) if bottom - top >= MIN_BAND => break,
            (false, _) => run = None,
        }
        y += STEP;
    }
    if let Some((top, bottom)) = run
        && bottom - top >= MIN_BAND
    {
        return Point {
            x: center.x,
            y: (top + bottom) / 2.0,
        };
    }
    let mut best: Option<(f32, Point)> = None;
    let mut y = b.y + STEP / 2.0;
    while y < b.y + b.h {
        let mut x = b.x + STEP / 2.0;
        while x < b.x + b.w {
            let p = Point { x, y };
            let d = (x - center.x).powi(2) + (y - center.y).powi(2);
            if free(p) && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, p));
            }
            x += STEP;
        }
        y += STEP;
    }
    best.map_or(center, |(_, p)| p)
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
/// Labels print inside quotes, so an agent that copies one often keeps them.
fn unquoted(text: &str) -> Option<&str> {
    let t = text.trim();
    [('"', '"'), ('\'', '\''), ('\u{201c}', '\u{201d}')]
        .iter()
        .find_map(|&(a, b)| t.strip_prefix(a)?.strip_suffix(b))
        .filter(|inner| !inner.trim().is_empty())
}

fn find_text(
    now: &Rendered,
    text: &str,
    role: Option<phonedriver_core::element::Role>,
    extra_words: Option<usize>,
) -> TextMatch {
    let exact = now.resolve_text(text, role);
    if exact != TextMatch::None {
        return exact;
    }
    if let Some(inner) = unquoted(text) {
        return find_text(now, inner, role, extra_words);
    }
    let q = normalize(text);
    for whole in [true, false] {
        let mut labels: Vec<&str> = Vec::new();
        for e in &now.elements {
            for l in parts(e) {
                let l_norm = normalize(l);
                let hit = if whole {
                    l_norm == q
                } else {
                    holds_as_words(&l_norm, &q, extra_words)
                };
                if hit && !labels.contains(&l) {
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

/// `label` has `query` at word boundaries and, with `extra_words`, at most that many other words. Both are normalized.
fn holds_as_words(label: &str, query: &str, extra_words: Option<usize>) -> bool {
    let (Some(first), Some(last)) = (query.chars().next(), query.chars().next_back()) else {
        return false;
    };
    let extra = label
        .split(' ')
        .count()
        .saturating_sub(query.split(' ').count());
    extra_words.is_none_or(|cap| extra <= cap)
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

    fn sought(labels: &[&str], target: &str) -> Result<String, Error> {
        let now = screen(labels);
        Session::default()
            .seek(&text(target), &now, None)
            .map(|r| r.line.unwrap_or_default())
    }

    #[test]
    fn a_search_for_either_of_two_texts_stops_at_the_first_on_screen() {
        let labels = ["Battery", "Wi-Fi", "Airplane Mode"];
        assert_eq!(
            sought(&labels, "Airplane|Wi-Fi").unwrap(),
            "@e2 button \"Wi-Fi\""
        );
        assert_eq!(
            sought(&labels, "Bluetooth | battery").unwrap(),
            "@e1 button \"Battery\""
        );
        assert!(sought(&labels, "Bluetooth|Cellular").is_err());
        let found = Session::default().find(&screen(&labels), "airplane|wi-fi", None);
        assert_eq!(
            found,
            ["@e2 button \"Wi-Fi\"", "@e3 button \"Airplane Mode\""]
        );
    }

    #[test]
    fn a_search_finds_a_word_inside_a_long_sentence() {
        let sentence = "The Snapdragon system on chip was announced in November 2006 and included the Scorpion processor";
        let line = sought(&["History", sentence], "announced").unwrap();
        assert!(line.contains("was announced in"), "{line}");
        assert!(picked(&[sentence], "announced").is_err());
    }

    #[test]
    fn a_search_that_matches_twice_stops_at_the_first() {
        let line = sought(
            &["It was announced in 2006", "Scorpion was announced in 2007"],
            "announced",
        )
        .unwrap();
        assert!(line.contains("2006"), "{line}");
    }

    #[test]
    fn a_target_copied_with_the_quotes_the_screen_prints_around_labels_still_matches() {
        let line = picked(
            &["Display & brightness", "Battery"],
            "\"Display & brightness\"",
        )
        .unwrap();
        assert!(line.ends_with("\"Display & brightness\""), "{line}");
        assert!(picked(&["Battery"], "\u{201c}Battery\u{201d}").is_ok());
        assert!(
            picked(&["\"Hi\" she said", "Hi"], "\"Hi\" she said")
                .unwrap()
                .contains("she said")
        );
    }

    #[test]
    fn a_tap_on_a_card_lands_on_the_card_not_on_a_control_inside_it() {
        let json = r#"{"windows":[{"id":1,"type":"application","bounds":[0,0,400,800]}],"nodes":[
            {"p":-1,"w":1,"cls":"android.view.View","b":[0,100,400,400],"f":"vVe","a":["tap"]},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"Wikipedia","b":[60,110,300,140],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.Button","text":"Install","b":[310,110,390,150],"f":"vVe","a":["tap"]},
            {"p":0,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,160,400,200],"f":"vVeS","a":["scroll_fwd"]},
            {"p":0,"w":1,"cls":"android.widget.TextView","text":"Yes, the app has voice search","b":[0,210,400,290],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.EditText","hint":"Ask a question","b":[0,300,400,400],"f":"vVe","a":["tap","set_text"]}
        ]}"#;
        let tree = RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap();
        let nodes = prune(&tree);
        let refs = RefTable::default().assign(&tree, &nodes);
        let now = Rendered::build(nodes, &refs, Header::default());
        let at = Session::default()
            .resolve(&text("Wikipedia"), &now)
            .unwrap()
            .at;
        let card = now.elements[0].bounds;
        assert!(
            card.x <= at.x && at.x <= card.x + card.w && card.y <= at.y && at.y <= card.y + card.h,
            "{at:?}"
        );
        assert!(
            at.y < 160.0,
            "the card's own title band, above the ratings: {at:?}"
        );
        for e in &now.elements[1..] {
            let b = e.bounds;
            let inside = b.x <= at.x && at.x <= b.x + b.w && b.y <= at.y && at.y <= b.y + b.h;
            assert!(!inside, "{at:?} is on {:?}", e.label);
        }
        let plain = Session::default()
            .resolve(&text("Install"), &now)
            .unwrap()
            .at;
        assert_eq!((plain.x, plain.y), (350.0, 130.0));
    }

    #[test]
    fn a_web_element_cut_off_at_the_left_edge_of_a_zoomed_page_is_tapped_near_that_edge() {
        let json = r#"{"app":{"bundle_id":"com.apple.mobilesafari","pid":1},"nodes":[
            {"p":-1,"cls":"XCUIElementTypeApplication","text":"Safari","b":[0,0,402,874],"f":"vVe"},
            {"p":0,"cls":"XCUIElementTypeWebView","b":[0,0,402,874],"f":"vVe"},
            {"p":1,"cls":"XCUIElementTypeTextField","text":"Customer name:","b":[88,95,315,125],"f":"vVeE","a":["tap"]},
            {"p":1,"cls":"XCUIElementTypeOther","text":"Medium","sd":"0","b":[0,394,153,435],"f":"vVe"}
        ]}"#;
        let tree = RawTree::from_ios_json(json).unwrap();
        let nodes = prune(&tree);
        let refs = RefTable::default().assign(&tree, &nodes);
        let now = Rendered::build(nodes, &refs, Header::default());
        let radio = Session::default()
            .resolve(&text("Medium"), &now)
            .unwrap()
            .at;
        assert!(radio.x <= WEB_LEFT_EDGE_TAP, "{radio:?}");
        assert_eq!(radio.y, 414.5);
        let field = Session::default()
            .resolve(&text("Customer name:"), &now)
            .unwrap()
            .at;
        assert_eq!((field.x, field.y), (201.5, 110.0));
    }

    #[test]
    fn a_search_keeps_word_boundaries() {
        assert!(sought(&["Row 364"], "Row 3").is_err());
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
