//! The words a device uses for months and for the halves of the day, in the language its pickers are drawn in
//! (decision D125). They come from the device, so a calendar is read in whatever language it shows.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CalendarNames {
    /// The language tag the names are in, `de-DE`.
    pub locale: String,
    /// Twelve lists, January first: the forms the device writes that month in (`December`, `Dec`, `12月`), the same
    /// form at the same place in each list.
    pub months: Vec<Vec<String>>,
    /// Every form of the first half of the day (`AM`, `午前`), and of the second.
    pub am: Vec<String>,
    pub pm: Vec<String>,
    /// The ten digits the language writes, `٠١٢٣٤٥٦٧٨٩`; empty when they are `0` to `9`.
    #[serde(default)]
    pub digits: String,
}

impl CalendarNames {
    /// A number as the language writes it: `45` is `٤٥` in Egypt, where a time field takes no other digits
    /// (API 37 emulator).
    pub fn written(&self, number: &str) -> String {
        let own: Vec<char> = self.digits.chars().collect();
        number
            .chars()
            .map(|c| match (c.to_digit(10), own.len()) {
                (Some(d), 10) => own[d as usize],
                _ => c,
            })
            .collect()
    }

    /// The month a text names in full, 1 to 12, when it is one of the device's forms and nothing else.
    pub fn month_named(&self, text: &str) -> Option<u32> {
        let text = fold(text);
        self.months
            .iter()
            .position(|forms| forms.iter().any(|form| fold(form) == text))
            .map(|i| i as u32 + 1)
    }

    /// Whether a text is the second half of the day (`Some(true)`), the first, or neither.
    pub fn half_named(&self, text: &str) -> Option<bool> {
        let text = fold(text);
        let any = |forms: &[String]| forms.iter().any(|form| fold(form) == text);
        match (any(&self.am), any(&self.pm)) {
            (true, false) => Some(false),
            (false, true) => Some(true),
            _ => None,
        }
    }
}

/// A text as it is compared: lower case, digits of any script as `0` to `9`, and no dot at the end of a short form
/// (`Dez.`).
pub fn fold(text: &str) -> String {
    text.trim()
        .trim_end_matches('.')
        .chars()
        .flat_map(|c| match digit(c) {
            Some(d) => vec![char::from_digit(d, 10).unwrap_or('0')],
            None => c.to_lowercase().collect(),
        })
        .collect()
}

/// The value of a decimal digit of any script: Arabic-Indic `٣` is 3, as a calendar in Egypt writes it.
pub fn digit(c: char) -> Option<u32> {
    if let Some(d) = c.to_digit(10) {
        return Some(d);
    }
    if !c.is_numeric() {
        return None;
    }
    const ZEROS: [u32; 18] = [
        0x0660, 0x06F0, 0x07C0, 0x0966, 0x09E6, 0x0A66, 0x0AE6, 0x0B66, 0x0BE6, 0x0C66, 0x0CE6,
        0x0D66, 0x0E50, 0x0ED0, 0x0F20, 0x1040, 0x17E0, 0xFF10,
    ];
    let code = c as u32;
    ZEROS
        .iter()
        .find(|zero| (**zero..**zero + 10).contains(&code))
        .map(|zero| code - zero)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn german() -> CalendarNames {
        CalendarNames {
            locale: "de-DE".into(),
            months: (1..=12)
                .map(|m| match m {
                    3 => vec!["März".to_string(), "Mär".to_string()],
                    12 => vec!["Dezember".to_string(), "Dez.".to_string()],
                    _ => vec![format!("m{m}")],
                })
                .collect(),
            am: vec!["AM".into(), "vorm.".into()],
            pm: vec!["PM".into(), "nachm.".into()],
            digits: String::new(),
        }
    }

    #[test]
    fn a_month_is_named_in_any_of_the_devices_forms_whatever_the_case_or_the_dot() {
        let names = german();
        assert_eq!(names.month_named("Dezember"), Some(12));
        assert_eq!(names.month_named("dez"), Some(12));
        assert_eq!(names.month_named("DEZ."), Some(12));
        assert_eq!(names.month_named("MÄRZ"), Some(3));
        assert_eq!(names.month_named("December"), None);
        assert_eq!(names.half_named("nachm."), Some(true));
        assert_eq!(names.half_named("am"), Some(false));
        assert_eq!(names.half_named("noon"), None);
    }

    #[test]
    fn digits_of_other_scripts_read_as_numbers() {
        assert_eq!(fold("١٨ ديسمبر ٢٠٢٦"), "18 ديسمبر 2026");
        assert_eq!(fold("１２月"), "12月");
        assert_eq!(digit('۷'), Some(7));
        assert_eq!(digit('x'), None);
        assert_eq!(digit('½'), None);
    }

    #[test]
    fn a_number_is_written_in_the_languages_digits_when_it_has_its_own() {
        let egypt = CalendarNames {
            digits: "٠١٢٣٤٥٦٧٨٩".into(),
            ..german()
        };
        assert_eq!(egypt.written("05"), "٠٥");
        assert_eq!(german().written("05"), "05");
    }
}
