//! `{{name}}` in the text of a saved run, and the environment variable a saved secret is read from (decision D98).

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use serde_json::Value;

/// A replay reads a secret only from a variable with this prefix, so an edited steps file cannot make it type any
/// other variable of the process (an API key) into a field.
pub const SECRET_PREFIX: &str = "PHONEDRIVER_SECRET_";

/// Set to `1` where a replay runs, a secret it reads goes into any field, not only a password or one-time code field
/// (decision D105).
pub const SECRET_ANY_FIELD: &str = "PHONEDRIVER_REPLAY_SECRET_ANY_FIELD";

pub const RUN: &str = "run";

/// A short id for one replay: the same in every step of it, different in the next one.
pub fn run_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (nanos, std::process::id()).hash(&mut hasher);
    format!("{:08x}", hasher.finish() as u32)
}

/// A `{{name}}` with no value, and the example it was written with (`{{day1:Tuesday}}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blank {
    pub name: String,
    pub example: Option<String>,
}

/// The steps with every `{{name}}` in the strings of the parameters of those from `from` on (1 is the first)
/// replaced by its value, except the name of the variable a secret is read from, and the names that were used. A
/// blank may hold an example after a colon (`{{day1:Tuesday}}`), the wording the screen uses. When some name has no
/// value, every such name is returned instead, once each, in order, with the first example it was given.
pub fn fill(
    steps: &[Value],
    from: usize,
    vars: &BTreeMap<String, String>,
) -> Result<(Vec<Value>, BTreeSet<String>), Vec<Blank>> {
    let mut used = BTreeSet::new();
    let mut missing: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut filled = steps.to_vec();
    let mut lookup = |name: &str, example: Option<&str>| {
        let found = vars.get(name).cloned();
        match found {
            Some(_) => {
                used.insert(name.to_string());
            }
            None => {
                let kept = missing.entry(name.to_string()).or_default();
                if kept.is_none() {
                    *kept = example.map(str::to_string);
                }
            }
        }
        found
    };
    for step in filled.iter_mut().skip(from.saturating_sub(1)) {
        if let Some(params) = step.get_mut("params") {
            walk(params, &mut lookup);
        }
    }
    if missing.is_empty() {
        Ok((filled, used))
    } else {
        Err(missing
            .into_iter()
            .map(|(name, example)| Blank { name, example })
            .collect())
    }
}

type Lookup<'a> = dyn FnMut(&str, Option<&str>) -> Option<String> + 'a;

fn walk(value: &mut Value, lookup: &mut Lookup) {
    match value {
        Value::String(text) => *text = replace(text, lookup),
        Value::Array(items) => items.iter_mut().for_each(|v| walk(v, lookup)),
        Value::Object(fields) => fields
            .iter_mut()
            .filter(|(key, _)| *key != "secret_env")
            .for_each(|(_, v)| walk(v, lookup)),
        _ => {}
    }
}

fn replace(text: &str, lookup: &mut Lookup) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find("{{") {
        let after = &rest[open + 2..];
        match after.find("}}") {
            Some(end) if is_name(blank(&after[..end]).0) => {
                out.push_str(&rest[..open]);
                let (name, example) = blank(&after[..end]);
                match lookup(name, example) {
                    Some(value) => out.push_str(&value),
                    None => out.push_str(&rest[open..open + end + 4]),
                }
                rest = &after[end + 2..];
            }
            _ => {
                out.push_str(&rest[..open + 2]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The name of a blank, and its example when it has one.
fn blank(inside: &str) -> (&str, Option<&str>) {
    match inside.split_once(':') {
        Some((name, example)) if !example.is_empty() => (name, Some(example)),
        Some((name, _)) => (name, None),
        None => (inside, None),
    }
}

pub fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn names_are_filled_everywhere_in_the_parameters_and_a_value_is_not_read_again() {
        let steps = vec![json!({
            "method": "act",
            "params": {"action": {"kind": "type", "text": "a-{{x}}-{{y}}-{{x}}", "into": {"text": "{{x}}"}}},
            "summary": "{{x}}"
        })];
        let (out, used) = fill(&steps, 1, &vars(&[("x", "{{y}}"), ("y", "2")])).unwrap();
        assert_eq!(used.into_iter().collect::<Vec<_>>(), ["x", "y"]);
        assert_eq!(out[0]["params"]["action"]["text"], "a-{{y}}-2-{{y}}");
        assert_eq!(out[0]["params"]["action"]["into"]["text"], "{{y}}");
        assert_eq!(out[0]["summary"], "{{x}}");
    }

    #[test]
    fn the_variable_a_secret_is_read_from_is_not_filled() {
        let steps = vec![json!({
            "method": "act",
            "params": {"action": {"kind": "type", "secret": true, "secret_env": "PHONEDRIVER_SECRET_{{who}}", "into": {"text": "{{who}}"}}}
        })];
        let (out, _) = fill(&steps, 1, &vars(&[("who", "ADMIN")])).unwrap();
        assert_eq!(
            out[0]["params"]["action"]["secret_env"],
            "PHONEDRIVER_SECRET_{{who}}"
        );
        assert_eq!(out[0]["params"]["action"]["into"]["text"], "ADMIN");
    }

    /// Issue #41: two alarm replays were given `day1: "Tue"` where the buttons say `Tuesday`, and stopped there. A
    /// blank may carry an example after a colon, which the "needs" message shows, and is filled like any other.
    #[test]
    fn a_blank_carries_an_example_that_a_missing_value_names_and_a_given_one_replaces() {
        let steps = vec![
            json!({
                "method": "act",
                "params": {"action": {"kind": "tap", "target": {"text": "{{day1:Tuesday}}"}}}
            }),
            json!({"params": {"command": "echo {{hour:6}} {{day1}} {{note}}"}}),
        ];
        let missing = fill(&steps, 1, &vars(&[])).unwrap_err();
        assert_eq!(
            missing,
            [
                Blank {
                    name: "day1".into(),
                    example: Some("Tuesday".into())
                },
                Blank {
                    name: "hour".into(),
                    example: Some("6".into())
                },
                Blank {
                    name: "note".into(),
                    example: None
                }
            ]
        );
        let (out, _) = fill(
            &steps,
            1,
            &vars(&[("day1", "Thursday"), ("hour", "7"), ("note", "x")]),
        )
        .unwrap();
        assert_eq!(out[0]["params"]["action"]["target"]["text"], "Thursday");
        assert_eq!(out[1]["params"]["command"], "echo 7 Thursday x");
    }

    #[test]
    fn text_that_is_not_a_name_between_braces_stays_as_it_is() {
        let steps = vec![json!({"params": {"command": "echo {{ a }} {{}} {{a b}} {{ {{a}}"}})];
        let (out, _) = fill(&steps, 1, &vars(&[("a", "1")])).unwrap();
        assert_eq!(
            out[0]["params"]["command"],
            "echo {{ a }} {{}} {{a b}} {{ 1"
        );
    }

    #[test]
    fn every_missing_name_is_returned_once_and_steps_before_from_are_not_read() {
        let steps = vec![
            json!({"params": {"command": "{{skipped}}"}}),
            json!({"params": {"command": "{{b}} {{a}} {{b}}"}}),
        ];
        let missing = fill(&steps, 2, &vars(&[])).unwrap_err();
        assert_eq!(
            missing.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert!(fill(&steps, 2, &vars(&[("a", ""), ("b", "")])).is_ok());
    }
}
