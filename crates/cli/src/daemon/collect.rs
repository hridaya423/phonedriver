//! The rows of a list, gathered across scrolls (decision D116).

use phonedriver_core::element::{Actions, ItemPos};
use phonedriver_core::render::Rendered;

/// One row: its snapshot line without the ref, then the lines of what it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub at: Option<ItemPos>,
    pub text: String,
}

impl Row {
    /// The same row seen again: the same text, and the same place in the list where the app says both places.
    fn same(&self, other: &Row) -> bool {
        self.text == other.text
            && match (self.at, other.at) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            }
    }
}

/// The rows the list `holder` shows now, in order. A list whose only child is a plain wrapper is read through it.
pub fn rows(now: &Rendered, holder: usize) -> Vec<Row> {
    let mut list = holder;
    while let [only] = now.elements[list].children.as_slice()
        && !now.elements[*only].children.is_empty()
        && !now.elements[*only].actions.contains(Actions::TAP)
    {
        list = *only;
    }
    now.elements[list]
        .children
        .iter()
        .map(|&i| Row {
            at: now.elements[i].position,
            text: text(now, i),
        })
        .collect()
}

fn text(now: &Rendered, i: usize) -> String {
    let line = now.line(&now.elements[i]);
    let mut out = match line.split_once(' ') {
        Some((r, rest)) if r.starts_with('@') => rest.to_string(),
        _ => line,
    };
    for &child in &now.elements[i].children {
        out.push_str(", ");
        out.push_str(&text(now, child));
    }
    out
}

/// How many leading rows of a new page may be cut-off copies of rows already taken.
const CUT_ROWS: usize = 2;

/// Adds the rows of `page` that `all` does not hold yet and returns how many it took. `forward` is a scroll down or
/// right: the page continues `all` at its end, and its first rows repeat the last ones taken. The longest such
/// overlap is dropped, so rows with the same text and no place in the list, where one page ends and the next
/// begins, count once. The last row taken may have been cut off by the page's edge and read differently then: when
/// only the rows before it line up, the page's own row takes its place.
pub fn merge(all: &mut Vec<Row>, page: Vec<Row>, forward: bool) -> usize {
    let mut page = page;
    if !forward {
        all.reverse();
        page.reverse();
    }
    let overlap = |kept: &[Row], skip: usize| {
        (1..=kept.len().min(page.len() - skip)).rev().find(|&k| {
            kept[kept.len() - k..]
                .iter()
                .zip(&page[skip..skip + k])
                .all(|(a, b)| a.same(b))
        })
    };
    let best = (0..=1usize.min(all.len()))
        .flat_map(|cut| (0..page.len().min(CUT_ROWS + 1)).map(move |skip| (cut, skip)))
        .filter_map(|(cut, skip)| overlap(&all[..all.len() - cut], skip).map(|k| (k, cut, skip)))
        .max_by_key(|&(k, cut, skip)| (k, std::cmp::Reverse(cut), std::cmp::Reverse(skip)));
    let new = match best {
        Some((k, cut, skip)) => {
            all.truncate(all.len() - cut);
            page.split_off(skip + k)
        }
        None => page,
    };
    let added = new.len();
    all.extend(new);
    if !forward {
        all.reverse();
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(texts: &[&str]) -> Vec<Row> {
        texts
            .iter()
            .map(|t| Row {
                at: None,
                text: t.to_string(),
            })
            .collect()
    }

    fn texts(all: &[Row]) -> Vec<&str> {
        all.iter().map(|r| r.text.as_str()).collect()
    }

    #[test]
    fn a_page_adds_only_what_follows_the_rows_it_repeats() {
        let mut all = Vec::new();
        assert_eq!(merge(&mut all, page(&["a", "b", "c"]), true), 3);
        assert_eq!(merge(&mut all, page(&["b", "c", "d", "e"]), true), 2);
        assert_eq!(merge(&mut all, page(&["b", "c", "d", "e"]), true), 0);
        assert_eq!(texts(&all), ["a", "b", "c", "d", "e"]);
    }

    #[test]
    fn a_page_with_nothing_seen_before_is_added_whole_and_a_cut_first_row_is_skipped() {
        let mut all = page(&["a", "b"]);
        assert_eq!(merge(&mut all, page(&["x", "y"]), true), 2);
        assert_eq!(merge(&mut all, page(&["x (cut)", "y", "z"]), true), 1);
        assert_eq!(texts(&all), ["a", "b", "x", "y", "z"]);
    }

    /// Seen on Settings (API 37 emulator): the row at the page's edge is cut off and reads `button "Storage"`, and
    /// on the next page `button "Storage" "51% used - 7.78 GB free"`. The six rows the pages shared were listed twice.
    #[test]
    fn a_row_cut_off_at_the_edge_of_a_page_is_replaced_by_the_whole_row_on_the_next() {
        let mut all = Vec::new();
        merge(&mut all, page(&["Sound", "Display", "Storage"]), true);
        let next = page(&["Display", "Storage 51% used", "Battery", ""]);
        assert_eq!(merge(&mut all, next, true), 3);
        let last = page(&["51% used", "Battery", "Location"]);
        assert_eq!(merge(&mut all, last, true), 1);
        assert_eq!(
            texts(&all),
            [
                "Sound",
                "Display",
                "Storage 51% used",
                "Battery",
                "Location"
            ]
        );

        let mut all = page(&["Storage 51% used", "Battery", "Location"]);
        let above = page(&["", "Display", "Storage 51% used", "Battery"]);
        assert_eq!(merge(&mut all, above, false), 2);
        assert_eq!(texts(&all)[..2], ["", "Display"]);
    }

    #[test]
    fn scrolling_back_puts_the_new_rows_in_front() {
        let mut all = page(&["d", "e", "f"]);
        assert_eq!(merge(&mut all, page(&["b", "c", "d", "e"]), false), 2);
        assert_eq!(texts(&all), ["b", "c", "d", "e", "f"]);
    }

    #[test]
    fn two_rows_with_one_text_are_one_row_only_where_the_app_gives_them_one_place() {
        let at = |row: u32, text: &str| Row {
            at: Some(ItemPos { row, col: 0 }),
            text: text.to_string(),
        };
        let mut all = vec![at(0, "Unknown caller"), at(1, "Unknown caller")];
        let next = vec![at(1, "Unknown caller"), at(2, "Unknown caller")];
        assert_eq!(merge(&mut all, next, true), 1);
        assert_eq!(all.len(), 3);
        let mut all = vec![at(0, "Unknown caller"), at(1, "Unknown caller")];
        let apart = vec![at(2, "Unknown caller"), at(3, "Unknown caller")];
        assert_eq!(merge(&mut all, apart, true), 2);
    }
}
