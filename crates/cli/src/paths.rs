//! Named paths of an app (decision D118): a saved run kept as `paths/<app id>/<name>.steps` in the project folder,
//! which ends by waiting for the screen it arrived on.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Value, json};

use crate::daemon::secrets::MASK;
use crate::variables;

pub const DIR: &str = "paths";

/// How long a path waits for its landmark. The step before it has already settled, so this is for a page whose
/// title comes late; a person who knows the page is slower edits the number in the file.
const ARRIVAL_MS: u64 = 5000;

/// A landmark longer than this is a sentence of content, not a title.
const LANDMARK_CHARS: usize = 40;

/// `launch` names this many paths, then counts the rest.
const NAMED: usize = 12;

/// The server's instructions name this many paths of an app and this many apps, then count the rest: they are read
/// once a session, and `launch` lists an app's own.
const KNOWN: usize = 8;
const KNOWN_APPS: usize = 6;

/// The app and the name of a steps file kept as `paths/<app id>/<name>.steps`.
pub fn named(file: &Path) -> Option<(String, String)> {
    let name = file.file_stem()?.to_str()?;
    let app = file.parent()?;
    let inside = app.parent()?.file_name()?;
    (inside == DIR && file.extension()? == "steps")
        .then(|| Some((app.file_name()?.to_str()?.to_string(), name.to_string())))?
}

/// The step that waits for the screen a run ended on: its first heading, else the first label that is no control
/// (on the API 37 emulator a Settings page's title is a group, "Internet", above the texts of its list), else its
/// first labelled element, and of those one without digits (a count, a clock) before one with. What a person typed
/// into a field is content and never a landmark, and neither is text the daemon masked.
pub fn landmark(elements: &[Value]) -> Option<Value> {
    let label = |e: &Value| {
        e["label"]
            .as_str()
            .map(str::trim)
            .filter(|l| !l.is_empty() && l.chars().count() <= LANDMARK_CHARS && !l.contains(MASK))
            .map(str::to_string)
    };
    let entry = ["text_field", "secure_field", "search_field", "keyboard"];
    let kinds: [&dyn Fn(&str) -> bool; 3] = [
        &|role| role == "heading",
        &|role| ["text", "other", "cell"].contains(&role),
        &|role| !entry.contains(&role),
    ];
    let found = |steady: bool| {
        kinds.iter().find_map(|kind| {
            elements.iter().find_map(|e| {
                let role = e["role"].as_str().filter(|r| kind(r))?;
                let label =
                    label(e).filter(|l| !steady || !l.chars().any(|c| c.is_ascii_digit()))?;
                Some((role.to_string(), label))
            })
        })
    };
    let (role, text) = found(true).or_else(|| found(false))?;
    Some(json!({
        "method": "wait",
        "params": {"condition": {"appears": {"text": text, "role": role}}, "timeout_ms": ARRIVAL_MS},
        "landmark": true,
    }))
}

/// Whether `elements` hold the landmark of `step` as a whole label, of its role when it names one; when not, why.
/// A `wait` also finds its text inside a longer label, which is how the page before the one a path ends on read as
/// an arrival on the API 37 emulator ("Network & internet" for "Internet", where "Internet" is also a row's
/// button). `A|B` is either, as `wait` reads it.
pub fn arrived(step: &Value, elements: &[Value]) -> Result<(), String> {
    let wanted = &step["params"]["condition"]["appears"];
    let texts = || {
        wanted["text"]
            .as_str()
            .unwrap_or_default()
            .split('|')
            .map(str::trim)
    };
    let of_role = |e: &&Value| wanted["role"].is_null() || e["role"] == wanted["role"];
    let label = |e: &Value| e["label"].as_str().unwrap_or_default().trim().to_string();
    let whole = |e: &&Value| texts().any(|t| label(e) == t);
    let kind = |role: &Value| match role.as_str() {
        Some("other") | None => "group".to_string(),
        Some(role) => role.replace('_', " "),
    };
    if elements.iter().filter(of_role).any(|e| whole(&e)) {
        return Ok(());
    }
    let inside = elements
        .iter()
        .filter(of_role)
        .find(|e| texts().any(|t| label(e).to_lowercase().contains(&t.to_lowercase())));
    Err(match (inside, elements.iter().find(whole)) {
        (Some(e), _) => format!("the screen holds it only inside {:?}", label(e)),
        (None, Some(e)) => format!(
            "it is on the screen as a {}, not as the {} the path ended on",
            kind(&e["role"]),
            kind(&wanted["role"])
        ),
        (None, None) => "no label on the screen is that text".into(),
    })
}

/// What a landmark step waits for, as a reply names it.
pub fn arrival(step: &Value) -> Option<&str> {
    (step["landmark"] == true).then(|| step["params"]["condition"]["appears"]["text"].as_str())?
}

/// The paths of `app` in `folder` as a reply names them, in name order: each with the blanks it needs and the
/// screen it arrives on, `find(query) → "Results"`. The second value says whether any has a blank.
fn entries(folder: &Path, app: &str) -> (Vec<String>, bool) {
    let mut blank = false;
    let mut names: Vec<String> = std::fs::read_dir(folder.join(DIR).join(app))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let file = entry.path();
            let (_, name) = named(&file)?;
            let text = std::fs::read_to_string(&file).ok()?;
            let blanks = blanks(&text);
            blank |= !blanks.is_empty();
            let needs = if blanks.is_empty() {
                String::new()
            } else {
                format!("({})", blanks.join(", "))
            };
            let arrives = text
                .lines()
                .rev()
                .find_map(|l| serde_json::from_str::<Value>(l).ok())
                .and_then(|last| arrival(&last).map(|t| format!(" → {t:?}")))
                .unwrap_or_default();
            Some(format!("{name}{needs}{arrives}"))
        })
        .collect();
    names.sort();
    (names, blank)
}

fn capped(mut names: Vec<String>, most: usize) -> String {
    let more = names.len().saturating_sub(most);
    names.truncate(most);
    let more = if more > 0 {
        format!(" and {more} more")
    } else {
        String::new()
    };
    format!("{}{more}", names.join(", "))
}

/// The line `launch` adds for an app that has paths in `folder`: each path with its blanks and where it arrives,
/// and the call that goes down one (decision D131). Nothing for an app with none.
pub fn listed(folder: &Path, app: &str) -> Option<String> {
    let (names, blank) = entries(folder, app);
    if names.is_empty() {
        return None;
    }
    let vars = if blank { " (vars fill its blanks)" } else { "" };
    Some(format!(
        "paths: {} · launch with path <name>{vars} arrives on exactly that screen in one call",
        capped(names, NAMED)
    ))
}

/// What the MCP server's instructions add for a folder with paths (decision D131): an agent chooses its first call
/// from them, before any reply has named a path. Nothing for a folder with none. The list comes before the rule and
/// is called the only one while it is whole: told "saved paths come first" ahead of the names, an agent asked
/// `launch` for a path `sims` that nobody had saved, then took the path to the page next to the one it wanted.
pub fn known(folder: &Path) -> Option<String> {
    let mut apps: Vec<String> = std::fs::read_dir(folder.join(DIR))
        .ok()?
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    apps.sort();
    let (mut blank, mut cut) = (false, false);
    let mut lists: Vec<String> = apps
        .iter()
        .filter_map(|app| {
            let (names, has) = entries(folder, app);
            blank |= has;
            cut |= names.len() > KNOWN;
            (!names.is_empty()).then(|| format!("{app}: {}", capped(names, KNOWN)))
        })
        .collect();
    if lists.is_empty() {
        return None;
    }
    cut |= lists.len() > KNOWN_APPS;
    let more = lists.len().saturating_sub(KNOWN_APPS);
    lists.truncate(KNOWN_APPS);
    let apps_more = if more > 0 {
        format!(
            "; and {} more",
            phonedriver_core::render::plural(more as u64, "app")
        )
    } else {
        String::new()
    };
    let lead = if cut {
        "Saved paths (`launch` lists an app's others):"
    } else {
        "The only saved paths are"
    };
    let vars = if blank {
        " (`vars` fill its blanks)"
    } else {
        ""
    };
    Some(format!(
        " {lead} {}{apps_more}. When one ends on the screen you want, `launch` with `app` and its `path`{vars} arrives in one call; otherwise ignore them.",
        lists.join("; ")
    ))
}

/// The steps file of the path `name` of `app`: the file of that name, else the one path that arrives on a screen
/// of that name (an agent told `languages → "Language & region"` asked for `Language & region`). An error names
/// the paths the app has.
pub fn file(
    folder: &Path,
    app: &str,
    shown: &str,
    name: &str,
) -> Result<std::path::PathBuf, String> {
    let plain =
        |part: &str| !part.is_empty() && !part.starts_with('.') && !part.contains(['/', '\\', ':']);
    if !plain(app) {
        return Err(format!("{app:?} is not an app id a path is kept under"));
    }
    let dir = folder.join(DIR).join(app);
    if plain(name) && dir.join(format!("{name}.steps")).is_file() {
        return Ok(dir.join(format!("{name}.steps")));
    }
    let arrives_there = |file: &std::path::PathBuf| {
        named(file).is_some()
            && std::fs::read_to_string(file).is_ok_and(|text| {
                text.lines()
                    .rev()
                    .find_map(|l| serde_json::from_str::<Value>(l).ok())
                    .and_then(|last| arrival(&last).map(|t| t.eq_ignore_ascii_case(name.trim())))
                    .unwrap_or(false)
            })
    };
    let mut there = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(arrives_there);
    if let (Some(only), None) = (there.next(), there.next()) {
        return Ok(only);
    }
    if !plain(name) {
        return Err(format!(
            "a path is a name, as `launch` lists them, not a file: got {name:?}"
        ));
    }
    Err(match listed(folder, app) {
        Some(line) => format!("{shown} has no path {name}; {line}"),
        None => format!(
            "{shown} has no path {name}, and no other: a run saved as {DIR}/{app}/<name>.steps is one"
        ),
    })
}

/// Whether a saved run opens an app itself, as one saved from a `launch` on does.
pub fn opens(text: &str) -> bool {
    text.lines()
        .find(|l| !l.trim().is_empty())
        .and_then(|l| serde_json::from_str::<Value>(l).ok())
        .is_some_and(|first| first["params"]["action"]["kind"] == "launch")
}

/// The `{{names}}` a steps file leaves blank, in the order `vars` is asked for them.
fn blanks(text: &str) -> Vec<String> {
    let steps: Vec<Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let given = [(variables::RUN.to_string(), String::new())].into();
    match variables::fill(&steps, 1, &given) {
        Ok(_) => Vec::new(),
        Err(missing) => missing.into_iter().map(|b| b.name).collect(),
    }
}

/// The apps a saved run was in: the one each step was saved on, and the one it ended in.
pub fn apps<'a>(steps: &'a [Value], arrived: Option<&'a str>) -> BTreeSet<&'a str> {
    steps
        .iter()
        .filter_map(|s| s["on"]["app"].as_str())
        .chain(arrived)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waits_for(elements: Value) -> Option<Value> {
        landmark(elements.as_array().unwrap()).map(|s| s["params"]["condition"]["appears"].clone())
    }

    #[test]
    fn the_landmark_is_the_title_before_any_other_text() {
        let screen = json!([
            {"role": "button", "label": "Back"},
            {"role": "text", "label": "Saved networks"},
            {"role": "heading", "label": "Internet"},
        ]);
        assert_eq!(
            waits_for(screen),
            Some(json!({"text": "Internet", "role": "heading"}))
        );
    }

    #[test]
    fn a_title_that_is_a_group_comes_before_the_texts_under_it() {
        let screen = json!([
            {"role": "other", "label": null},
            {"role": "other", "label": "Internet"},
            {"role": "button", "label": "Navigate up"},
            {"role": "text", "label": "Mobile data"},
        ]);
        assert_eq!(
            waits_for(screen),
            Some(json!({"text": "Internet", "role": "other"}))
        );
    }

    #[test]
    fn an_arrival_is_a_whole_label_of_the_landmarks_role() {
        let step = landmark(
            json!([{"role": "other", "label": "Internet"}])
                .as_array()
                .unwrap(),
        )
        .unwrap();
        let on = |screen: Value| arrived(&step, screen.as_array().unwrap());
        assert_eq!(on(json!([{"role": "other", "label": "Internet"}])), Ok(()));
        let before = json!([
            {"role": "other", "label": "Network & internet"},
            {"role": "button", "label": "Internet"},
        ]);
        assert_eq!(
            on(before),
            Err("the screen holds it only inside \"Network & internet\"".into())
        );
        assert_eq!(
            on(json!([{"role": "button", "label": "Internet"}])),
            Err("it is on the screen as a button, not as the group the path ended on".into())
        );
        assert_eq!(
            on(json!([{"role": "other", "label": "Display"}])),
            Err("no label on the screen is that text".into())
        );
        let mut either = step.clone();
        either["params"]["condition"]["appears"] = json!({"text": "Wi-Fi|Internet"});
        assert_eq!(
            arrived(
                &either,
                json!([{"role": "button", "label": "Internet"}])
                    .as_array()
                    .unwrap()
            ),
            Ok(())
        );
    }

    #[test]
    fn a_count_or_a_clock_is_passed_over_while_steadier_text_is_there() {
        let screen = json!([
            {"role": "heading", "label": "3 new messages"},
            {"role": "text", "label": "9:41"},
            {"role": "text", "label": "Inbox"},
        ]);
        assert_eq!(
            waits_for(screen),
            Some(json!({"text": "Inbox", "role": "text"}))
        );
        let only = json!([{"role": "heading", "label": "3 new messages"}]);
        assert_eq!(
            waits_for(only),
            Some(json!({"text": "3 new messages", "role": "heading"}))
        );
    }

    #[test]
    fn typed_text_masked_text_and_a_long_sentence_are_never_the_landmark() {
        let screen = json!([
            {"role": "text_field", "label": "ada@example.com"},
            {"role": "secure_field", "label": "Password"},
            {"role": "text", "label": format!("Signed in as {MASK}")},
            {"role": "text", "label": "This paragraph of the article runs on well past what a title would."},
        ]);
        assert_eq!(waits_for(screen), None);
    }

    #[test]
    fn a_path_is_a_steps_file_two_folders_under_paths() {
        let named = |f: &str| named(Path::new(f));
        assert_eq!(
            named("paths/com.example/wifi.steps"),
            Some(("com.example".into(), "wifi".into()))
        );
        assert_eq!(
            named("/work/app/paths/com.example/wifi.steps"),
            Some(("com.example".into(), "wifi".into()))
        );
        assert_eq!(named("paths/wifi.steps"), None);
        assert_eq!(named("e2e/com.example/wifi.steps"), None);
        assert_eq!(named("paths/com.example/notes.txt"), None);
    }
}
