//! Spelling suggestions for text targets that match nothing.

/// Optimal string alignment distance: insertions, deletions, substitutions and swaps of two neighbors, so `Rwo` is one step from `Row`.
fn distance(a: &[char], b: &[char]) -> usize {
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
}

/// The distance between two texts, ignoring case, as a share of the longer one: 0 is equal, 1 is nothing in common.
pub fn normalized_distance(a: &str, b: &str) -> f32 {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 0.0;
    }
    distance(&a, &b) as f32 / longest as f32
}

/// Suggestions are texts at most this far from the query.
pub const SUGGEST_WITHIN: f32 = 0.3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_swap_and_a_missing_letter_are_close_and_unrelated_words_are_not() {
        assert!(normalized_distance("Rwo 5", "Row 5") <= SUGGEST_WITHIN);
        assert!(normalized_distance("Calculatr", "Calculator") <= SUGGEST_WITHIN);
        assert!(normalized_distance("Calculatr", "calculator") <= SUGGEST_WITHIN);
        assert!(normalized_distance("Display", "Wallpaper") > SUGGEST_WITHIN);
        assert_eq!(normalized_distance("", ""), 0.0);
    }
}
