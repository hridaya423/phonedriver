//! Text as a person reads it, for comparing labels: the same words written with other code points compare equal.

use unicode_normalization::UnicodeNormalization;

/// `text` in NFKC, without the characters that draw nothing (zero-width spaces and joiners, soft hyphens, direction
/// marks, variation selectors, tags), with typographic quotes and dashes as their ASCII forms, and lowercase:
/// `Ｐａｙ`, `P\u{200B}ay` and `PAY` all fold to `pay`, and `Don’t` to `don't`. A letter of another script that only
/// looks the same (Cyrillic `а` for Latin `a`) is not folded.
pub fn fold(text: &str) -> String {
    text.nfkc()
        .filter(|c| !draws_nothing(*c))
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' | '\u{2032}' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' | '\u{2033}' => '"',
            '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
            other => other,
        })
        .flat_map(char::to_lowercase)
        .collect()
}

fn draws_nothing(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{061C}'
            | '\u{115F}'
            | '\u{1160}'
            | '\u{17B4}'
            | '\u{17B5}'
            | '\u{180B}'..='\u{180F}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{3164}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{FFA0}'
            | '\u{E0000}'..='\u{E0FFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_words_in_other_code_points_fold_alike() {
        assert_eq!(fold("\u{FF30}\u{FF41}\u{FF59}"), "pay");
        assert_eq!(fold("P\u{200B}a\u{00AD}y"), "pay");
        assert_eq!(fold("Place\u{00A0}Order"), "place order");
        assert_eq!(
            fold("Don\u{2019}t \u{2014} \u{201C}x\u{201D}"),
            "don't - \"x\""
        );
        assert_eq!(fold("\u{FB01}nish"), "finish");
        assert_eq!(fold("\u{FFE5}\u{FF11}\u{FF0C}\u{FF12}"), "\u{A5}1,2");
        assert_ne!(fold("Pl\u{0430}ce"), "place");
    }
}
