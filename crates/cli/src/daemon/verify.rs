//! Did typing land? What a text field shows after `type`, `clear` or a replace, and why not when it differs.

use phone_use_core::element::{Element, ElementState, Ref, Role};
use phone_use_core::render::Rendered;
use serde::{Deserialize, Serialize};

/// What the field should hold afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expect {
    /// Exactly this text: a replace, or a clear (empty).
    Exactly(String),
    /// The text typed after what was there: an append, or typing into the focused field.
    Appended { before: String, text: String },
}

#[derive(Debug, Clone)]
pub struct Verify {
    pub field: Option<Ref>,
    pub expect: Expect,
    pub secure: bool,
    /// The `--mode` that was used, so a hint never suggests the mode that just failed.
    pub mode: Option<&'static str>,
    /// Enter followed the text.
    pub submitted: bool,
}

/// `Unknown`: Enter was sent and the field is gone (a form that submits, a search overlay that closes), so the text
/// can be neither confirmed nor refuted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Landed,
    Failed(Unverified),
    Unknown,
}

impl Verdict {
    pub fn verified(&self) -> Option<bool> {
        match self {
            Verdict::Landed => Some(true),
            Verdict::Failed(_) => Some(false),
            Verdict::Unknown => None,
        }
    }
}

/// Why typing is not confirmed: what the field shows and what to do about that cause.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unverified {
    pub shows: String,
    pub hint: String,
}

/// The field as the screen shows it after the action.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FieldView {
    /// The target field (or the focused one, when there was no target) is on screen.
    pub found: bool,
    pub focused: bool,
    pub value: Option<String>,
    /// The ref of another focused text field, when the target is not the focused one.
    pub focus_elsewhere: Option<Ref>,
    pub target: Option<Ref>,
}

impl Verify {
    pub fn view(&self, now: &Rendered) -> FieldView {
        let focused_entry = |now: &'_ Rendered| -> Option<usize> {
            now.elements
                .iter()
                .position(|e| e.state.contains(ElementState::FOCUSED) && e.role.is_text_entry())
        };
        let target = self
            .field
            .and_then(|r| now.index_of(r))
            .filter(|&i| now.elements[i].role.is_text_entry());
        let chosen = target.or_else(|| focused_entry(now));
        let Some(i) = chosen else {
            return FieldView {
                target: self.field,
                ..FieldView::default()
            };
        };
        let e: &Element = &now.elements[i];
        let focused = e.state.contains(ElementState::FOCUSED);
        let elsewhere = if target.is_some() && !focused {
            focused_entry(now).map(|j| now.elements[j].r#ref)
        } else {
            None
        };
        FieldView {
            found: true,
            focused,
            value: e.value.clone(),
            focus_elsewhere: elsewhere,
            target: Some(e.r#ref),
        }
    }

    pub fn check(&self, now: &Rendered, navigated: bool) -> Verdict {
        let secure = self.secure
            || self.field.and_then(|r| now.index_of(r)).is_some_and(|i| {
                let e = &now.elements[i];
                e.role == Role::SecureField || e.state.contains(ElementState::PASSWORD)
            });
        self.verdict(&self.view(now), secure, navigated)
    }

    /// `navigated`: the screen changed beyond the field. After Enter that means the text was sent somewhere, and
    /// Chrome's address bar then shows the completed URL instead of what was typed.
    fn verdict(&self, view: &FieldView, secure: bool, navigated: bool) -> Verdict {
        match self.judge(view, secure) {
            None => Verdict::Landed,
            Some(_) if self.submitted && (!view.found || navigated) => Verdict::Unknown,
            Some(u) => Verdict::Failed(u),
        }
    }

    pub fn judge(&self, v: &FieldView, secure: bool) -> Option<Unverified> {
        let want = match &self.expect {
            Expect::Exactly(t) => t.clone(),
            Expect::Appended { before, text } => format!("{before}{text}"),
        };
        let name = v
            .target
            .or(self.field)
            .map_or_else(|| "the field".to_string(), |r| format!("@{r}"));
        if !v.found {
            return Some(Unverified {
                shows: "no text field is on screen".into(),
                hint:
                    "the screen moved away from the field; `phone-use snapshot` shows what is there"
                        .into(),
            });
        }
        if let Some(other) = v.focus_elsewhere {
            return Some(Unverified {
                shows: shown(&name, v.value.as_deref(), secure),
                hint: format!(
                    "focus is on @{other}, not {name}, so the text went there; tap {name} and type again"
                ),
            });
        }
        let Some(value) = v.value.as_deref() else {
            let ok = matches!(&self.expect, Expect::Exactly(t) if t.is_empty());
            return (!ok).then(|| Unverified {
                shows: format!("{name} shows no text"),
                hint: "the text may not have landed (type again, or `--mode keys`), or the field does not expose its text; `phone-use screenshot` shows what it displays".into(),
            });
        };
        let (got, wanted) = (value.chars().count(), want.chars().count());
        if secure {
            return (got != wanted).then(|| Unverified {
                shows: format!("{name} holds {got} characters, not {wanted}"),
                hint: "the field kept a different number of characters than typed".into(),
            });
        }
        let ok = match &self.expect {
            Expect::Exactly(t) => value == t,
            Expect::Appended { text, .. } => value.ends_with(text.as_str()),
        };
        if ok || self.phone_formatted(value) {
            return None;
        }
        let shows = shown(&name, Some(value), false);
        let typed = match &self.expect {
            Expect::Exactly(t) => t.as_str(),
            Expect::Appended { text, .. } => text.as_str(),
        };
        let hint = if value.is_empty() && !typed.is_empty() {
            let other = match self.mode {
                Some("keys") => "",
                _ => "; `--mode keys` types key by key",
            };
            format!(
                "the field took none of the text: it may be read-only, filter what it accepts, or not be the view that has focus{other}"
            )
        } else if !typed.is_empty() && is_prefix(value, &want) {
            format!(
                "the field kept the first {got} of {wanted} characters: it limits its length (maxLength)"
            )
        } else if !typed.is_empty() && is_subsequence(value, &want) {
            "the field dropped some characters: it accepts only some kinds (digits, for one)".into()
        } else if typed.is_empty() {
            "the field would not clear; `phone-use snapshot` shows what it holds".into()
        } else {
            "the field holds other text than typed: it may reformat what it receives or have taken input from elsewhere".into()
        };
        Some(Unverified { shows, hint })
    }
}

impl Verify {
    /// A phone field rewrites `3710658` as `+1 371-065-8` while it is typed into: the digits typed are the last digits
    /// shown, behind at most a three-digit country code.
    fn phone_formatted(&self, value: &str) -> bool {
        let typed = match &self.expect {
            Expect::Exactly(t) => t.as_str(),
            Expect::Appended { text, .. } => text.as_str(),
        };
        let phone_like = typed
            .chars()
            .all(|c| c.is_ascii_digit() || "+-(). ".contains(c));
        let digits = |s: &str| s.chars().filter(char::is_ascii_digit).collect::<String>();
        let (typed, shown) = (digits(typed), digits(value));
        phone_like && typed.len() >= 4 && shown.ends_with(&typed) && shown.len() <= typed.len() + 3
    }
}

fn shown(name: &str, value: Option<&str>, secure: bool) -> String {
    match value {
        _ if secure => format!("{name} is a secure field"),
        Some(v) if !v.is_empty() => format!("{name} shows {}", phone_use_core::render::quote(v)),
        _ => format!("{name} shows nothing"),
    }
}

fn is_prefix(value: &str, want: &str) -> bool {
    !value.is_empty() && value.len() < want.len() && want.starts_with(value)
}

fn is_subsequence(value: &str, want: &str) -> bool {
    let mut it = want.chars();
    !value.is_empty() && value.chars().all(|c| it.any(|w| w == c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verify(expect: Expect) -> Verify {
        Verify {
            field: Some(Ref(5)),
            expect,
            secure: false,
            mode: Some("paste"),
            submitted: false,
        }
    }

    fn view(value: Option<&str>) -> FieldView {
        FieldView {
            found: true,
            focused: true,
            value: value.map(str::to_string),
            focus_elsewhere: None,
            target: Some(Ref(5)),
        }
    }

    fn exactly(t: &str) -> Verify {
        verify(Expect::Exactly(t.into()))
    }

    #[test]
    fn a_replace_is_verified_only_when_the_field_holds_exactly_the_text() {
        assert_eq!(exactly("07").judge(&view(Some("07")), false), None);
        let bad = exactly("07").judge(&view(Some("107")), false).unwrap();
        assert_eq!(bad.shows, "@e5 shows \"107\"");
    }

    #[test]
    fn an_empty_replace_is_not_verified_by_a_field_that_still_holds_text() {
        let bad = exactly("").judge(&view(Some("10")), false).unwrap();
        assert_eq!(bad.shows, "@e5 shows \"10\"");
        assert!(bad.hint.contains("would not clear"), "{}", bad.hint);
        assert_eq!(exactly("").judge(&view(Some("")), false), None);
        assert_eq!(exactly("").judge(&view(None), false), None);
    }

    #[test]
    fn a_field_cut_to_a_length_limit_says_so() {
        let bad = exactly("0730").judge(&view(Some("07")), false).unwrap();
        assert_eq!(bad.shows, "@e5 shows \"07\"");
        assert!(
            bad.hint.contains("first 2 of 4") && bad.hint.contains("maxLength"),
            "{}",
            bad.hint
        );
    }

    #[test]
    fn a_field_that_drops_characters_says_what_kind_it_accepts() {
        let bad = exactly("a1b2").judge(&view(Some("12")), false).unwrap();
        assert!(bad.hint.contains("accepts only some kinds"), "{}", bad.hint);
    }

    #[test]
    fn a_field_that_took_nothing_says_so_and_names_the_other_mode_only_when_it_has_not_been_tried()
    {
        let bad = exactly("07").judge(&view(Some("")), false).unwrap();
        assert_eq!(bad.shows, "@e5 shows nothing");
        assert!(
            bad.hint.contains("took none") && bad.hint.contains("--mode keys"),
            "{}",
            bad.hint
        );
        let mut keys = exactly("07");
        keys.mode = Some("keys");
        assert!(
            !keys
                .judge(&view(Some("")), false)
                .unwrap()
                .hint
                .contains("--mode")
        );
    }

    #[test]
    fn focus_on_another_field_is_named() {
        let mut v = view(Some("37"));
        v.focused = false;
        v.focus_elsewhere = Some(Ref(9));
        let bad = exactly("07").judge(&v, false).unwrap();
        assert!(
            bad.hint.contains("focus is on @e9, not @e5"),
            "{}",
            bad.hint
        );
    }

    #[test]
    fn an_append_needs_the_text_at_the_end() {
        let v = verify(Expect::Appended {
            before: "ab".into(),
            text: "cd".into(),
        });
        assert_eq!(v.judge(&view(Some("abcd")), false), None);
        assert!(v.judge(&view(Some("ab")), false).is_some());
    }

    #[test]
    fn a_secure_field_is_compared_by_length_and_never_shown() {
        let v = exactly("hunter2");
        assert_eq!(v.judge(&view(Some("•••••••")), true), None);
        let bad = v.judge(&view(Some("••")), true).unwrap();
        assert!(
            !bad.shows.contains("hunter2") && bad.shows.contains("2 characters"),
            "{}",
            bad.shows
        );
    }

    #[test]
    fn a_submit_that_moved_the_screen_is_unknown_not_failed() {
        let mut v = exactly("en.wikipedia.org");
        v.submitted = true;
        let completed = view(Some("https://en.wikipedia.org/wiki/Main_Page"));
        assert_eq!(v.verdict(&completed, false, true), Verdict::Unknown);
        assert!(matches!(
            v.verdict(&completed, false, false),
            Verdict::Failed(_)
        ));
        assert_eq!(
            v.verdict(&view(Some("en.wikipedia.org")), false, true),
            Verdict::Landed
        );
        assert_eq!(
            v.verdict(&FieldView::default(), false, false),
            Verdict::Unknown
        );
        v.submitted = false;
        assert!(matches!(
            v.verdict(&completed, false, true),
            Verdict::Failed(_)
        ));
    }

    #[test]
    fn a_phone_field_that_formats_the_digits_is_verified_and_a_wrong_digit_is_not() {
        let v = verify(Expect::Exactly("3710658".into()));
        let shows = |value: &str| FieldView {
            found: true,
            focused: true,
            value: Some(value.into()),
            focus_elsewhere: None,
            target: Some(Ref(5)),
        };
        assert!(v.judge(&shows("+1 371-065-8"), false).is_none());
        assert!(v.judge(&shows("+1 371-065-9"), false).is_some());
        assert!(v.judge(&shows("+1 371"), false).is_some());
        let word = verify(Expect::Exactly("hello".into()));
        assert!(word.judge(&shows("h-e-l-l-o"), false).is_some());
    }

    #[test]
    fn a_field_that_left_the_screen_and_one_that_exposes_no_text_are_told_apart() {
        let gone = exactly("07").judge(&FieldView::default(), false).unwrap();
        assert!(gone.shows.contains("no text field"), "{}", gone.shows);
        let blind = exactly("07").judge(&view(None), false).unwrap();
        assert!(blind.hint.contains("does not expose"), "{}", blind.hint);
    }
}
