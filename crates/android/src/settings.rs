//! The Settings app's device-state app functions, which Android 17 answers for the shell user through
//! `cmd app_function` (decision D57): an index of settings by key, their values, and a write by key. Every field of
//! a reply is an array, usually of one.

use std::collections::HashMap;

use phonedriver_core::settings::{SettingItem, SettingScreen, SettingsIndex};
use serde_json::Value;

const PACKAGE: &str = "com.android.settings";
pub const INDEX_FUNCTION: &str = "getDeviceStateMetadata";
pub const WRITE_FUNCTION: &str = "setDeviceStateItem";
/// The functions that report values, asked in this order for a key. `getDeviceStateItem`, which should read one
/// key, fails on API 37 with "No valid executor found for GET_DEVICE_STATE".
pub const STATE_FUNCTIONS: [&str; 6] = [
    "getUncategorizedDeviceState",
    "getBatteryDeviceState",
    "getNotificationsDeviceState",
    "getMobileDataUsageDeviceState",
    "getAppsDeviceState",
    "getStorageDeviceState",
];
const LISTED_VALUES: usize = 8;
const VALUES_CHARS: usize = 120;

pub fn command(function: &str, parameters: &Value) -> String {
    let json = parameters.to_string().replace('\'', "'\\''");
    format!(
        "cmd app_function execute-app-function --package {PACKAGE} --function {function} --parameters '{json}'"
    )
}

pub fn write_parameters(key: &str, value: &str) -> Value {
    serde_json::json!({"setDeviceStateItemParams": {"key": key, "itemizationKeys": [], "value": value}})
}

fn first<'a>(v: &'a Value, field: &str) -> &'a Value {
    &v[field][0]
}

fn each<'a>(v: &'a Value, field: &str) -> impl Iterator<Item = &'a Value> {
    v[field].as_array().into_iter().flatten()
}

fn text(v: &Value, field: &str) -> Option<String> {
    let text = match first(v, field) {
        Value::String(s) => s.clone(),
        Value::Null => return None,
        other => other.to_string(),
    };
    let text = text
        .replace("<external_data>", "")
        .replace("</external_data>", "");
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn returned(reply: &str) -> Result<Value, String> {
    let v: Value = serde_json::from_str(reply).map_err(|_| {
        reply
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("the device answered nothing")
            .trim()
            .to_string()
    })?;
    Ok(first(&v, "androidAppfunctionsReturnValue").clone())
}

fn values_text(possible: &str, types: &HashMap<String, &Value>) -> String {
    if possible == "BOOL" {
        return "true | false".into();
    }
    let Some(name) = possible.strip_prefix("itemization:") else {
        let plain = possible
            .split(", ")
            .map(
                |part| match part.strip_suffix(')').and_then(|p| p.split_once(" (")) {
                    Some((value, label)) if value == label => value,
                    _ => part,
                },
            )
            .collect::<Vec<_>>()
            .join(", ");
        return if plain.chars().count() > VALUES_CHARS {
            let head: String = plain.chars().take(VALUES_CHARS).collect();
            format!("{head}…")
        } else {
            plain
        };
    };
    let Some(kind) = types.get(name) else {
        return name.to_string();
    };
    let listed: Vec<String> = each(kind, "values")
        .filter_map(|v| {
            let key = text(v, "key")?;
            Some(match text(v, "value") {
                Some(label) if !label.eq_ignore_ascii_case(&key) => format!("{key} ({label})"),
                _ => key,
            })
        })
        .collect();
    if (1..=LISTED_VALUES).contains(&listed.len()) {
        return listed.join(" | ");
    }
    text(kind, "hintText")
        .map(|h| h.trim_end_matches('.').to_string())
        .unwrap_or_else(|| name.to_string())
}

const INTENT: &str = "intent:#Intent;";
/// `FLAG_ACTIVITY_NEW_TASK | FLAG_ACTIVITY_CLEAR_TASK`.
const CLEARED_TASK: &str = "launchFlags=0x10008000;";

/// Settings' own link, started as it is given, is handed to the page a running Settings already shows, and that
/// page ignores it: `am start` answers "intent has been delivered to currently running top-most instance" and the
/// first screen stays in front (API 37 emulator, 2026-11-26: SIMs, then Battery). In a cleared task the screen asked
/// for is the one shown.
fn fresh(link: &str) -> String {
    match link.strip_prefix(INTENT) {
        Some(rest) if !link.contains("launchFlags=") => format!("{INTENT}{CLEARED_TASK}{rest}"),
        _ => link.to_string(),
    }
}

/// The index from a `getDeviceStateMetadata` reply. A screen's key is the `screen_key` of its link; a link that
/// needs an item filled in (`%24itemization`) is not kept, because nothing here can fill it.
pub fn index(reply: &str) -> Result<SettingsIndex, String> {
    let root = returned(reply)?;
    let types: HashMap<String, &Value> = each(&root, "itemizationTypes")
        .filter_map(|t| Some((text(t, "key")?, t)))
        .collect();
    let mut out = SettingsIndex::default();
    for screen in each(&root, "perScreenMetadata") {
        let link = text(screen, "intentUri");
        let items: Vec<&Value> = each(screen, "deviceStateItemsMetadata").collect();
        let key = link
            .as_deref()
            .and_then(|l| l.split_once("S.screen_key="))
            .and_then(|(_, rest)| rest.split(';').next())
            .map(str::to_string)
            .or_else(|| {
                let key = text(items.first()?, "key")?;
                Some(key.split('/').next()?.to_string())
            });
        let Some(key) = key else { continue };
        let at = out.screens.len();
        out.screens.push(SettingScreen {
            key,
            about: text(screen, "description").unwrap_or_default(),
            link: link
                .filter(|l| !l.contains("%24itemization") && !l.contains("$itemization"))
                .map(|l| fresh(&l)),
        });
        for item in items {
            let Some(key) = text(item, "key") else {
                continue;
            };
            out.items.push(SettingItem {
                key,
                purpose: text(item, "purpose").unwrap_or_default(),
                values: values_text(&text(item, "possibleValues").unwrap_or_default(), &types),
                writable: first(item, "writable").as_bool().unwrap_or(false),
                confirm: text(item, "sensitivity").as_deref() == Some("REQUIRES_CONFIRMATION"),
                screen: at,
            });
        }
    }
    Ok(out)
}

/// The value of `key`, from the reply of one of `STATE_FUNCTIONS`, with the device's note when it says a
/// precondition fails. Every item carries a note; the passing ones ("Passing set preconditions: Always enabled.")
/// tell an agent nothing.
pub fn value(reply: &str, key: &str) -> Option<(String, Option<String>)> {
    let root = returned(reply).ok()?;
    each(&root, "perScreenDeviceStates")
        .flat_map(|screen| each(screen, "deviceStateItems"))
        .find(|item| text(item, "key").as_deref() == Some(key))
        .map(|item| {
            (
                text(item, "jsonValue").unwrap_or_default(),
                text(item, "hintText").filter(|h| h.contains("Failing")),
            )
        })
}

/// The value after a `setDeviceStateItem`, or why the device did not set it.
pub fn written(reply: &str) -> Result<String, String> {
    let root = returned(reply)?;
    let now = text(&root, "currentValue").unwrap_or_default();
    if first(&root, "isSuccessful").as_bool() == Some(true) {
        return Ok(now);
    }
    let why = text(&root, "failureReason").unwrap_or_else(|| "the device gave no reason".into());
    Err(if now.is_empty() || now == "N/A" {
        why
    } else {
        format!("{why} (it is {now})")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const METADATA: &str = include_str!("../tests/fixtures/settings_metadata_api37.json");
    const STATE: &str = include_str!("../tests/fixtures/settings_state_api37.json");

    fn item<'a>(index: &'a SettingsIndex, key: &str) -> &'a SettingItem {
        index.item(key).unwrap_or_else(|| panic!("no {key}"))
    }

    #[test]
    fn a_screens_link_starts_it_in_a_cleared_task_and_a_link_with_flags_of_its_own_keeps_them() {
        assert_eq!(
            fresh("intent:#Intent;action=a.B;S.screen_key=k;end"),
            "intent:#Intent;launchFlags=0x10008000;action=a.B;S.screen_key=k;end"
        );
        let own = "intent:#Intent;action=a.B;launchFlags=0x4000000;end";
        assert_eq!(fresh(own), own);
        assert_eq!(fresh("https://example.com/a"), "https://example.com/a");
    }

    #[test]
    fn the_index_holds_each_setting_with_its_screen_and_what_a_write_accepts() {
        let index = index(METADATA).unwrap();
        let dark = item(&index, "dark_ui_mode/dark_ui_activated");
        assert_eq!(dark.values, "true | false");
        assert!(dark.writable && !dark.confirm);
        let screen = &index.screens[dark.screen];
        assert_eq!(screen.key, "dark_ui_mode");
        assert!(screen.about.starts_with("Dark theme."), "{}", screen.about);
        assert_eq!(
            screen.link.as_deref(),
            Some(
                "intent:#Intent;launchFlags=0x10008000;action=com.android.settings.action.LAUNCH_SETTINGS_PAGES;package=com.android.settings;S.screen_key=dark_ui_mode;end"
            )
        );
        assert_eq!(
            item(&index, "sound_screen/media_volume").values,
            "INTEGER(min=0, max=15)"
        );
        assert_eq!(
            item(&index, "dark_ui_mode/dark_ui_auto_mode").values,
            "None, Turns on at custom time, Turns on from sunset to sunrise"
        );
    }

    #[test]
    fn a_value_type_the_device_defines_is_spelled_out_or_described() {
        let index = index(METADATA).unwrap();
        assert_eq!(
            item(&index, "api_dark_ui_mode/dark_theme_mode_selector").values,
            "0 (Standard) | 1 (Expanded)"
        );
        assert_eq!(
            item(&index, "api_dark_ui_mode/dark_theme_start_time").values,
            "A time of day defined by the 24-hour format HH:mm"
        );
        assert_eq!(
            item(
                &index,
                "key_system_region_picker_page/default_system_language_region_preference"
            )
            .values,
            "Country code of the chosen region"
        );
    }

    #[test]
    fn read_only_and_confirm_marks_are_kept_and_a_screen_that_needs_an_item_has_no_link() {
        let index = index(METADATA).unwrap();
        let nfc = item(&index, "nfc_and_payment_screen/use_nfc");
        assert!(nfc.confirm && !nfc.writable);
        assert!(!item(&index, "battery_info_screen/battery_info_manufacture_date").writable);
        assert!(!item(&index, "wifi_tether/use_wifi_hotspot").confirm);
        let apn = index.screen("telephony_apn_key").unwrap();
        assert_eq!(apn.link, None);
    }

    #[test]
    fn a_value_is_read_from_a_state_reply_without_the_devices_data_tags() {
        assert_eq!(
            value(STATE, "dark_ui_mode/dark_ui_activated"),
            Some(("true".into(), None))
        );
        let failing = r#"{"androidAppfunctionsReturnValue":[{"perScreenDeviceStates":[{"deviceStateItems":[{"hintText":["Failing get preconditions: The battery information feature is not enabled."],"jsonValue":["Error: Not allowed to read value"],"key":["battery_info_screen/battery_info_manufacture_date"]}]}]}]}"#;
        assert_eq!(
            value(failing, "battery_info_screen/battery_info_manufacture_date"),
            Some((
                "Error: Not allowed to read value".into(),
                Some(
                    "Failing get preconditions: The battery information feature is not enabled."
                        .into()
                )
            ))
        );
        assert_eq!(
            value(STATE, "dark_ui_mode/dark_ui_auto_mode"),
            Some(("None".into(), None))
        );
        assert_eq!(value(STATE, "sound_screen/media_volume"), None);
    }

    #[test]
    fn a_write_answers_with_the_value_or_the_devices_reason() {
        let ok = r#"{"androidAppfunctionsReturnValue":[{"isSuccessful":[true],"currentValue":["true"]}]}"#;
        assert_eq!(written(ok), Ok("true".into()));
        let off = r#"{"androidAppfunctionsReturnValue":[{"failureReason":["Preference not enabled"],"isSuccessful":[false],"currentValue":["N\/A"]}]}"#;
        assert_eq!(written(off), Err("Preference not enabled".into()));
        let bad = r#"{"androidAppfunctionsReturnValue":[{"failureReason":["Error with code 8 due to Request is invalid."],"isSuccessful":[false],"currentValue":["83"]}]}"#;
        assert_eq!(
            written(bad),
            Err("Error with code 8 due to Request is invalid. (it is 83)".into())
        );
        let none = "Error executing app function: android.app.appfunctions.AppFunctionException: App function not found. (code 1003). See logcat for more details. \n";
        assert!(
            written(none)
                .unwrap_err()
                .contains("App function not found")
        );
    }

    #[test]
    fn a_value_with_a_quote_cannot_end_the_shell_argument() {
        let c = command(WRITE_FUNCTION, &write_parameters("a/b", "it's"));
        assert!(c.ends_with(r#""value":"it'\''s"}}'"#), "{c}");
        assert!(c.starts_with(
            "cmd app_function execute-app-function --package com.android.settings --function setDeviceStateItem --parameters '{"
        ));
    }
}
