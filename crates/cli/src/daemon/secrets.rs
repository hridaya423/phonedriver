//! Text a caller marked secret is masked in every reply, error and trace line from then on (decision D70).

use std::sync::Mutex;

use serde_json::Value;

use super::rpc::RpcError;

pub const MASK: &str = "•••";

/// A shorter secret is masked only in the reply to the call that carried it. Remembered, `12` would blank every
/// label and count that holds those digits.
const REMEMBERED_FROM: usize = 4;

/// The secret texts a call carries: a `type` with `secret`, alone or as a batch step, and a `shell` command with
/// `secret`.
pub fn carried(method: &str, params: &Value) -> Vec<String> {
    let typed = |action: &Value| {
        (action["kind"] == "type" && action["secret"] == true)
            .then(|| action["text"].as_str().map(str::to_string))
            .flatten()
    };
    let found: Vec<String> = match method {
        "act" => typed(&params["action"]).into_iter().collect(),
        "batch" => params["steps"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(typed)
            .collect(),
        "shell" if params["secret"] == true => params["command"]
            .as_str()
            .map(str::to_string)
            .into_iter()
            .collect(),
        _ => Vec::new(),
    };
    found.into_iter().filter(|s| !s.is_empty()).collect()
}

/// Every secret the daemon was given since it started. They stay in memory and are never written anywhere.
#[derive(Default)]
pub struct Secrets(Mutex<Vec<String>>);

impl Secrets {
    /// Remembers what a call carried and returns every text to mask in its reply, longest first, so a secret that
    /// holds another is masked whole.
    pub fn with(&self, carried: Vec<String>) -> Vec<String> {
        let mut known = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let mut all = known.clone();
        for secret in carried {
            if !all.contains(&secret) {
                if secret.chars().count() >= REMEMBERED_FROM {
                    known.push(secret.clone());
                }
                all.push(secret);
            }
        }
        all.sort_by_key(|s| std::cmp::Reverse(s.len()));
        all
    }

    /// `text` with every remembered secret masked: for what the daemon writes to disk itself.
    pub fn masked(&self, text: &str) -> String {
        let mut text = text.to_string();
        hide_text(&mut text, &self.with(Vec::new()));
        text
    }

    /// Remembers what a field shows after `typed`, a secret, went into it, when the field shows the secret changed:
    /// its characters in order, in any case, with others between them (a card number in groups of four, a code in
    /// capitals, a phone number behind `+1`). Text that holds the secret as typed is masked already.
    pub fn shown_as(&self, typed: &str, shown: &str) {
        if !shown.contains(typed) && in_order(typed, shown) {
            self.with(vec![shown.to_string()]);
        }
    }
}

fn in_order(typed: &str, shown: &str) -> bool {
    let mut shown = shown.chars().flat_map(char::to_lowercase);
    typed
        .chars()
        .flat_map(char::to_lowercase)
        .all(|c| shown.any(|s| s == c))
}

/// Characters a field or a screen puts between the parts of a number or a code it shows.
fn is_separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, '-' | '(' | ')')
}

/// A secret is matched with separators between its characters only from this many characters on: a card number,
/// a phone number or a six-digit code, not a four-digit PIN that would also match `12-34`.
const SPACED_FROM: usize = 6;

/// The characters before a cut (`…`) that begin a secret are masked when there are at least this many: fewer are
/// as likely to be the end of a word ("Loading…").
const CUT_FROM: usize = 3;

/// How a secret looks inside a JSON string (a raw tree, a fixture) and inside a quoted string of a snapshot or a
/// comparison, besides as it is.
fn forms(secret: &str) -> Vec<String> {
    let json = serde_json::to_string(secret).unwrap_or_default();
    let quoted = format!("{secret:?}");
    let rendered = secret
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    let mut forms = vec![
        json[1..json.len() - 1].to_string(),
        quoted[1..quoted.len() - 1].to_string(),
        rendered,
    ];
    forms.retain(|f| f != secret);
    forms.sort();
    forms.dedup();
    forms.push(secret.to_string());
    forms
}

fn hide_text(text: &mut String, secrets: &[String]) {
    for secret in secrets {
        for form in forms(secret) {
            if text.contains(form.as_str()) {
                *text = text.replace(form.as_str(), MASK);
            }
        }
        hide_spaced(text, secret);
    }
    hide_cut(text, secrets);
}

/// Masks `secret` where it shows with separators between its characters: `4000 1234 1234 1234` for
/// `4000123412341234`.
fn hide_spaced(text: &mut String, secret: &str) {
    let core: Vec<char> = secret.chars().filter(|c| !is_separator(*c)).collect();
    if core.len() < SPACED_FROM || !text.contains(core[0]) {
        return;
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].1 == core[0] {
            let (mut j, mut k) = (i, 0);
            while j < chars.len() && k < core.len() {
                if chars[j].1 == core[k] {
                    k += 1;
                } else if !(k > 0 && is_separator(chars[j].1)) {
                    break;
                }
                j += 1;
            }
            if k == core.len() {
                spans.push((chars[i].0, chars.get(j).map_or(text.len(), |c| c.0)));
                i = j;
                continue;
            }
        }
        i += 1;
    }
    for (start, end) in spans.into_iter().rev() {
        text.replace_range(start..end, MASK);
    }
}

/// Masks the start of a secret just before a `…` where phonedriver or the device cut a long text: the rest of the
/// secret was cut off, so no whole secret is there to match.
fn hide_cut(text: &mut String, secrets: &[String]) {
    if !text.contains('…') {
        return;
    }
    let cuts: Vec<usize> = text.match_indices('…').map(|(at, _)| at).collect();
    for at in cuts.into_iter().rev() {
        let before = &text[..at];
        let longest = secrets
            .iter()
            .flat_map(|s| forms(s.as_str()))
            .filter_map(|form| {
                let chars: Vec<(usize, char)> = form.char_indices().collect();
                (CUT_FROM..chars.len())
                    .rev()
                    .map(|n| &form[..chars[n].0])
                    .find(|head| before.ends_with(head))
                    .map(str::len)
            })
            .max();
        if let Some(len) = longest {
            text.replace_range(at - len..at, MASK);
        }
    }
}

/// `text` with each of `secrets` masked as the daemon masks them: for text a client makes from what the screen
/// shows, with the secrets it typed itself.
pub fn masked_with(text: &str, secrets: &[String]) -> String {
    let mut secrets = secrets.to_vec();
    secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
    let mut text = text.to_string();
    hide_text(&mut text, &secrets);
    text
}

/// Masks `secrets` in every string of `value`. A screenshot's pixels (`data_b64`) are left as they are.
pub fn hide_value(value: &mut Value, secrets: &[String]) {
    match value {
        Value::String(s) => hide_text(s, secrets),
        Value::Array(items) => items.iter_mut().for_each(|v| hide_value(v, secrets)),
        Value::Object(fields) => {
            for (key, v) in fields {
                if key != "data_b64" {
                    hide_value(v, secrets);
                }
            }
        }
        _ => {}
    }
}

/// Masks `secrets` in every string of a reply or an error. A screenshot's pixels (`data_b64`) are left as they are.
pub fn hide(result: &mut Result<Value, RpcError>, secrets: &[String]) {
    if secrets.is_empty() {
        return;
    }
    match result {
        Ok(v) => hide_value(v, secrets),
        Err(e) => {
            hide_text(&mut e.message, secrets);
            hide_value(&mut e.data, secrets);
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_secret_is_masked_in_later_replies_and_errors_but_not_in_a_screenshots_pixels() {
        let secrets = Secrets::default();
        let carried = carried(
            "act",
            &json!({"action": {"kind": "type", "text": "hunter2!", "secret": true}}),
        );
        assert_eq!(carried, ["hunter2!"]);
        secrets.with(carried);
        let later = secrets.with(Vec::new());
        let mut reply = Ok(
            json!({"text": "@e1 textbox \"Note\" \"hunter2!\"", "elements": [{"value": "say hunter2! twice"}], "data_b64": "hunter2!"}),
        );
        hide(&mut reply, &later);
        assert_eq!(
            reply.unwrap(),
            json!({"text": "@e1 textbox \"Note\" \"•••\"", "elements": [{"value": "say ••• twice"}], "data_b64": "hunter2!"})
        );
        let mut error = Err(RpcError::new(
            1,
            "no field named hunter2!",
            json!({"hint": "hunter2!"}),
        ));
        hide(&mut error, &later);
        let error = error.unwrap_err();
        assert_eq!(error.message, "no field named •••");
        assert_eq!(error.data, json!({"hint": "•••"}));
    }

    #[test]
    fn a_secret_cut_short_before_an_ellipsis_is_masked_up_to_the_cut() {
        let secrets = Secrets::default();
        secrets.with(vec!["ABCDEFGHIJ-tail".into()]);
        assert_eq!(secrets.masked("out: xxABCDEFGH…"), "out: xx•••…");
        assert_eq!(
            secrets.masked("\"xxABC…(+9 chars)\""),
            "\"xx•••…(+9 chars)\""
        );
        assert_eq!(secrets.masked("Loading…"), "Loading…");
        assert_eq!(secrets.masked("xxAB…"), "xxAB…");
    }

    #[test]
    fn a_secret_is_masked_escaped_as_json_or_a_quoted_string_and_spaced_out() {
        let secrets = Secrets::default();
        secrets.with(vec![r#"pa"ss\word"#.into(), "4000123412341234".into()]);
        assert_eq!(
            secrets.masked(r#"{"text":"pa\"ss\\word"}"#),
            r#"{"text":"•••"}"#
        );
        assert_eq!(
            secrets.masked("card 4000 1234 1234 1234, or (4000) 1234-1234-1234."),
            "card •••, or (•••."
        );
        assert_eq!(secrets.masked("4000 1234"), "4000 1234");
    }

    #[test]
    fn a_field_that_shows_a_secret_changed_is_remembered_and_one_that_shows_other_text_is_not() {
        let secrets = Secrets::default();
        secrets.shown_as("kqxwz7pl4", "+1 (KQX) WZ7-PL4");
        secrets.shown_as("kqxwz7pl4", "Card number");
        secrets.shown_as("kqxwz7pl4", "•••••••••");
        assert_eq!(
            secrets.masked("+1 (KQX) WZ7-PL4 · Card number"),
            "••• · Card number"
        );
        assert_eq!(secrets.with(Vec::new()).len(), 1);
    }

    #[test]
    fn a_secret_inside_a_longer_one_does_not_uncover_the_rest_of_it() {
        let secrets = Secrets::default();
        secrets.with(vec!["4821".into()]);
        let all = secrets.with(vec!["pin-4821-end".into()]);
        let mut reply = Ok(json!("typed pin-4821-end, then 4821"));
        hide(&mut reply, &all);
        assert_eq!(reply.unwrap(), json!("typed •••, then •••"));
    }

    #[test]
    fn a_short_secret_is_masked_in_its_own_reply_and_forgotten_after() {
        let secrets = Secrets::default();
        let own = secrets.with(carried("shell", &json!({"command": "id", "secret": true})));
        assert_eq!(own, ["id"]);
        assert!(secrets.with(Vec::new()).is_empty());
        assert!(
            carried(
                "act",
                &json!({"action": {"kind": "type", "text": "", "secret": true}})
            )
            .is_empty()
        );
        assert_eq!(
            carried(
                "batch",
                &json!({"steps": [{"kind": "type", "text": "one-secret", "secret": true}, {"kind": "type", "text": "plain"}]})
            ),
            ["one-secret"]
        );
    }
}
