use crate::fakes;

use std::sync::Arc;

use async_trait::async_trait;
use fakes::tree;
use phonedriver::daemon::rpc::RpcError;
use phonedriver::daemon::{Config, Daemon, DeviceSource, Listed};
use phonedriver_core::device::{Capabilities, Device, Url};
use phonedriver_core::error::Error;
use phonedriver_core::settings::{SettingItem, SettingOp, SettingScreen, SettingsIndex};
use phonedriver_core::testing::{Call, FakeDevice};
use serde_json::{Value, json};

struct One {
    id: &'static str,
    fake: Arc<FakeDevice>,
}

#[async_trait]
impl DeviceSource for One {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(vec![Listed {
            id: self.id.into(),
            model: "Fake".into(),
            os: None,
            tier: "fake".into(),
            capabilities: self.fake.info().capabilities,
        }])
    }
    async fn connect(&self, _id: &str) -> Result<Arc<dyn Device>, Error> {
        Ok(self.fake.clone())
    }
}

const DARK: &str = "dark_ui_mode/dark_ui_activated";
const NFC: &str = "nfc_screen/use_nfc";
const CYCLES: &str = "battery_info/cycle_count";
const DARK_LINK: &str = "intent:#Intent;action=com.android.settings.action.LAUNCH_SETTINGS_PAGES;package=com.android.settings;S.screen_key=dark_ui_mode;end";

fn index() -> SettingsIndex {
    let item =
        |key: &str, purpose: &str, writable: bool, confirm: bool, screen: usize| SettingItem {
            key: key.into(),
            purpose: purpose.into(),
            values: "true | false".into(),
            writable,
            confirm,
            screen,
        };
    SettingsIndex {
        screens: vec![
            SettingScreen {
                key: "dark_ui_mode".into(),
                about: "Dark theme. The system-wide dark theme.".into(),
                link: Some(DARK_LINK.into()),
            },
            SettingScreen {
                key: "nfc_screen".into(),
                about: "NFC.".into(),
                link: None,
            },
        ],
        items: vec![
            item(
                DARK,
                "Whether the system dark theme is manually enabled",
                true,
                false,
                0,
            ),
            item(NFC, "Whether NFC is on", true, true, 1),
            item(CYCLES, "The battery charge cycle count", false, false, 1),
        ],
    }
}

fn setup(id: &'static str, settings: bool) -> (Arc<Daemon>, Arc<FakeDevice>, tempfile::TempDir) {
    let fake = Arc::new(
        FakeDevice::new(vec![tree(&["One"]), tree(&["Two"])]).lacking(Capabilities::FRAMES),
    );
    if settings {
        fake.with_settings(index(), &[(DARK, "false"), (NFC, "true"), (CYCLES, "12")]);
    }
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::new(
        Arc::new(One {
            id,
            fake: fake.clone(),
        }),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    (d, fake, home)
}

async fn ok(d: &Daemon, params: Value) -> Value {
    d.call("default", "setting", params)
        .await
        .unwrap_or_else(|e| panic!("setting failed: {e:?}"))
}

async fn err(d: &Daemon, params: Value) -> RpcError {
    d.call("default", "setting", params)
        .await
        .expect_err("expected an error")
}

fn writes(fake: &FakeDevice) -> Vec<Call> {
    fake.calls()
        .into_iter()
        .filter(|c| matches!(c, Call::Setting(SettingOp::Set { .. })))
        .collect()
}

#[tokio::test(start_paused = true)]
async fn find_lists_the_settings_and_screens_that_match() {
    let (d, _fake, _home) = setup("fake:1", true);
    let r = ok(&d, json!({"find": "dark"})).await;
    assert_eq!(
        r["lines"],
        json!([
            "dark_ui_mode/dark_ui_activated  [true | false]  Whether the system dark theme is manually enabled",
            "screen dark_ui_mode  Dark theme",
        ])
    );
    assert_eq!(r["more"], 0);
    let r = ok(&d, json!({"find": "teapot"})).await;
    assert_eq!(r["lines"], json!([]));
}

#[tokio::test(start_paused = true)]
async fn find_takes_several_searches_separated_by_a_bar() {
    let (d, _fake, _home) = setup("fake:1", true);
    let r = ok(&d, json!({"find": "dark theme|nfc|teapot"})).await;
    let lines: Vec<&str> = r["lines"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(lines.iter().any(|l| l.starts_with(DARK)), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with(NFC)), "{lines:?}");
    assert_eq!(
        lines.iter().filter(|l| l.starts_with(DARK)).count(),
        1,
        "{lines:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn values_writes_several_settings_in_one_call() {
    let (d, fake, _home) = setup("fake:1", true);
    let r = ok(&d, json!({"values": {DARK: "true", NFC: "false"}})).await;
    assert_eq!(r["values"], json!({DARK: "true", NFC: "false"}));
    assert_eq!(writes(&fake).len(), 2);
}

#[tokio::test(start_paused = true)]
async fn nothing_is_written_when_one_of_several_keys_is_wrong_and_a_refusal_says_what_was_already_set()
 {
    let (d, fake, _home) = setup("fake:1", true);
    let e = err(&d, json!({"values": {DARK: "true", CYCLES: "3"}})).await;
    assert!(e.message.contains("read-only"), "{}", e.message);
    assert!(writes(&fake).is_empty());
    let e = err(&d, json!({"values": {DARK: "true", NFC: "refuse"}})).await;
    assert_eq!(e.code, -32025);
    assert!(
        e.message.contains("nfc_screen/use_nfc was not set"),
        "{}",
        e.message
    );
    assert!(
        e.message
            .contains("already set: dark_ui_mode/dark_ui_activated = true"),
        "{}",
        e.message
    );
    let hint = e.data["hint"].as_str().unwrap();
    assert!(!hint.contains("nothing was changed"), "{hint}");
    assert!(hint.contains("already set"), "{hint}");
}

#[tokio::test(start_paused = true)]
async fn a_key_reads_its_value_and_a_value_writes_it() {
    let (d, fake, _home) = setup("fake:1", true);
    let r = ok(&d, json!({"key": DARK})).await;
    assert_eq!(r["value"], "false");
    assert!(writes(&fake).is_empty());
    let r = ok(&d, json!({"key": DARK, "value": "true"})).await;
    assert_eq!(r["key"], DARK);
    assert_eq!(r["value"], "true");
    assert_eq!(
        writes(&fake),
        [Call::Setting(SettingOp::Set {
            key: DARK.into(),
            value: "true".into()
        })]
    );
    assert_eq!(ok(&d, json!({"key": DARK})).await["value"], "true");
}

#[tokio::test(start_paused = true)]
async fn a_write_the_device_refuses_is_an_error_with_the_devices_reason() {
    let (d, _fake, _home) = setup("fake:1", true);
    let e = err(&d, json!({"key": DARK, "value": "refuse"})).await;
    assert_eq!(e.code, -32025);
    assert!(
        e.message.contains("Preference not enabled"),
        "{}",
        e.message
    );
}

#[tokio::test(start_paused = true)]
async fn an_unknown_key_is_refused_with_the_keys_that_come_closest() {
    let (d, fake, _home) = setup("fake:1", true);
    for params in [
        json!({"key": "dark"}),
        json!({"key": "dark", "value": "true"}),
    ] {
        let e = err(&d, params).await;
        assert_eq!(e.code, -32025);
        assert!(
            e.message.contains("no setting has the key \"dark\""),
            "{}",
            e.message
        );
        assert!(e.message.contains(DARK), "{}", e.message);
    }
    assert!(writes(&fake).is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_read_only_setting_is_not_written() {
    let (d, fake, _home) = setup("fake:1", true);
    let e = err(&d, json!({"key": CYCLES, "value": "1"})).await;
    assert_eq!(e.code, -32025);
    assert!(e.message.contains("read-only"), "{}", e.message);
    assert!(writes(&fake).is_empty());
}

#[tokio::test(start_paused = true)]
async fn open_opens_the_screens_link_and_returns_the_screen() {
    let (d, fake, _home) = setup("fake:1", true);
    let r = ok(&d, json!({"open": "dark_ui_mode"})).await;
    assert!(
        fake.calls()
            .contains(&Call::OpenUrl(Url(DARK_LINK.into()), None)),
        "{:?}",
        fake.calls()
    );
    assert_eq!(r["summary"], "opened the settings screen dark_ui_mode");
    let journal = d.call("default", "journal", json!({})).await.unwrap();
    assert_eq!(journal["steps"].as_array().unwrap().len(), 1, "{journal}");
    let e = err(&d, json!({"open": "nfc_screen"})).await;
    assert_eq!(e.code, -32025);
    assert!(e.message.contains("cannot be opened"), "{}", e.message);
    let e = err(&d, json!({"open": "nope"})).await;
    assert!(
        e.message.contains("no settings screen has the key"),
        "{}",
        e.message
    );
}

#[tokio::test(start_paused = true)]
async fn open_takes_the_title_a_screen_shows_or_the_key_of_a_setting_on_it() {
    for name in [
        "Dark theme",
        "dark theme",
        DARK,
        "dark_ui_mode/no_such_setting",
    ] {
        let (d, fake, _home) = setup("fake:1", true);
        let r = ok(&d, json!({"open": name})).await;
        assert!(
            fake.calls()
                .contains(&Call::OpenUrl(Url(DARK_LINK.into()), None)),
            "{name}: {:?}",
            fake.calls()
        );
        assert_eq!(
            r["summary"], "opened the settings screen dark_ui_mode",
            "{name}"
        );
    }
    let (d, fake, _home) = setup("fake:1", true);
    let e = err(&d, json!({"open": "NFC"})).await;
    assert!(e.message.contains("cannot be opened"), "{}", e.message);
    assert!(
        !fake.calls().iter().any(|c| matches!(c, Call::OpenUrl(..))),
        "{:?}",
        fake.calls()
    );
}

#[tokio::test(start_paused = true)]
async fn open_with_a_word_no_screen_is_named_by_answers_with_the_screens_that_hold_it() {
    let (d, fake, _home) = setup("fake:1", true);
    let e = err(&d, json!({"open": "dark"})).await;
    assert_eq!(e.code, -32025);
    assert_eq!(
        e.message,
        "no settings screen has the key \"dark\"; open one of: dark_ui_mode (Dark theme)"
    );
    let e = err(&d, json!({"open": "nope"})).await;
    assert_eq!(
        e.message,
        "no settings screen has the key \"nope\"; `find` lists screens as `screen <key>`"
    );
    assert!(
        !fake.calls().iter().any(|c| matches!(c, Call::OpenUrl(..))),
        "{:?}",
        fake.calls()
    );
}

#[tokio::test(start_paused = true)]
async fn on_a_physical_device_a_setting_the_user_must_confirm_needs_force() {
    let (d, fake, _home) = setup("android:R5CT123", true);
    let e = err(&d, json!({"key": NFC, "value": "false"})).await;
    assert_eq!(e.code, -32011);
    assert!(e.message.contains("confirm"), "{}", e.message);
    assert!(writes(&fake).is_empty());
    ok(&d, json!({"key": DARK, "value": "true"})).await;
    let r = ok(&d, json!({"key": NFC, "value": "false", "force": true})).await;
    assert_eq!(r["value"], "false");
    let (d, _fake, _home) = setup("android:emulator-5554", true);
    ok(&d, json!({"key": NFC, "value": "false"})).await;
}

#[tokio::test(start_paused = true)]
async fn a_device_without_settings_says_so_and_a_call_with_nothing_to_do_is_invalid() {
    let (d, _fake, _home) = setup("fake:1", false);
    let e = err(&d, json!({"find": "dark"})).await;
    assert_eq!(e.code, -32011);
    let (d, _fake, _home) = setup("fake:1", true);
    let e = err(&d, json!({})).await;
    assert_eq!(e.code, -32602);
    let e = err(&d, json!({"value": "true"})).await;
    assert_eq!(e.code, -32602);
}

fn restored(r: &Value) -> Vec<String> {
    r["restored"]
        .as_array()
        .unwrap_or_else(|| panic!("no `restored` in {r}"))
        .iter()
        .map(|l| {
            format!(
                "{} {} {}",
                l["key"].as_str().unwrap(),
                l["state"].as_str().unwrap(),
                l["value"].as_str().unwrap()
            )
        })
        .collect()
}

/// Issue #52: an agent that turned animations off or the theme dark for a test left the device that way. The value
/// each key had before its first write is kept, and `restore` writes them back and says what it did to each.
#[tokio::test(start_paused = true)]
async fn restore_writes_back_what_each_key_was_before_its_first_write_and_reports_each() {
    let (d, fake, home) = setup("fake:1", true);
    let r = ok(&d, json!({"key": DARK, "value": "true"})).await;
    assert_eq!(
        r["was"], "false",
        "the first write says what it replaced: {r}"
    );
    let r = ok(&d, json!({"key": DARK, "value": "maybe"})).await;
    assert!(r.get("was").is_none(), "only the first write of a key: {r}");
    ok(&d, json!({"values": {NFC: "false"}})).await;
    let kept: Vec<_> = std::fs::read_dir(home.path().join("settings-before"))
        .unwrap()
        .collect();
    assert_eq!(kept.len(), 1, "one file for the device");

    let r = ok(&d, json!({"restore": []})).await;
    assert_eq!(
        restored(&r),
        [
            format!("{DARK} restored false"),
            format!("{NFC} restored true")
        ]
    );
    assert_eq!(r["restored"][0]["from"], "maybe");
    assert_eq!(ok(&d, json!({"key": DARK})).await["value"], "false");
    assert_eq!(ok(&d, json!({"key": NFC})).await["value"], "true");
    assert_eq!(writes(&fake).len(), 5);

    let r = ok(&d, json!({"restore": []})).await;
    assert_eq!(r["restored"], json!([]), "nothing is left to restore");
    assert_eq!(writes(&fake).len(), 5);
}

#[tokio::test(start_paused = true)]
async fn restore_takes_keys_and_leaves_the_others_for_later() {
    let (d, _fake, _home) = setup("fake:1", true);
    ok(&d, json!({"values": {DARK: "true", NFC: "false"}})).await;
    let r = ok(&d, json!({"restore": [NFC]})).await;
    assert_eq!(restored(&r), [format!("{NFC} restored true")]);
    assert_eq!(ok(&d, json!({"key": DARK})).await["value"], "true");
    let e = err(&d, json!({"restore": [CYCLES]})).await;
    assert_eq!(e.code, -32025, "{e:?}");
    assert!(
        e.message.contains("phonedriver did not change"),
        "{}",
        e.message
    );
    let r = ok(&d, json!({"restore": []})).await;
    assert_eq!(restored(&r), [format!("{DARK} restored false")]);
}

/// The person may have changed the setting themselves since: their value stays, and the reply says so. A value
/// that is already the original needs no write.
#[tokio::test(start_paused = true)]
async fn a_key_someone_else_changed_since_is_left_alone_and_restore_says_so() {
    let (d, fake, _home) = setup("fake:1", true);
    ok(&d, json!({"values": {DARK: "true", NFC: "false"}})).await;
    fake.with_settings(index(), &[(DARK, "auto"), (NFC, "true"), (CYCLES, "12")]);
    let before = writes(&fake).len();
    let r = ok(&d, json!({"restore": []})).await;
    assert_eq!(
        restored(&r),
        [format!("{DARK} kept auto"), format!("{NFC} already true")]
    );
    assert_eq!(r["restored"][0]["was"], "false");
    assert_eq!(writes(&fake).len(), before, "nothing was written");
    assert_eq!(ok(&d, json!({"restore": []})).await["restored"], json!([]));
}

#[tokio::test(start_paused = true)]
async fn a_key_the_agent_wrote_back_itself_has_nothing_to_restore() {
    let (d, _fake, home) = setup("fake:1", true);
    ok(&d, json!({"key": DARK, "value": "true"})).await;
    ok(&d, json!({"key": DARK, "value": "false"})).await;
    assert_eq!(ok(&d, json!({"restore": []})).await["restored"], json!([]));
    let left = std::fs::read_dir(home.path().join("settings-before"))
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(left, 0, "an empty record is no file");
}

/// The record is a file, so what an agent that crashed (or a daemon that was stopped) changed can still be undone.
#[tokio::test(start_paused = true)]
async fn a_new_daemon_restores_what_the_last_one_changed() {
    let (d, fake, home) = setup("fake:1", true);
    ok(&d, json!({"key": DARK, "value": "true"})).await;
    drop(d);
    let d = Daemon::new(
        Arc::new(One {
            id: "fake:1",
            fake: fake.clone(),
        }),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    let r = ok(&d, json!({"restore": []})).await;
    assert_eq!(restored(&r), [format!("{DARK} restored false")]);
}

/// A restore is not done silently at the end: the change may be the task. `release` says what is still changed.
#[tokio::test(start_paused = true)]
async fn release_says_which_settings_are_still_changed_and_restores_nothing() {
    let (d, fake, _home) = setup("fake:1", true);
    let r = d.call("default", "release", json!({})).await.unwrap();
    assert!(r.get("settings_changed").is_none(), "{r}");
    ok(&d, json!({"key": DARK, "value": "true"})).await;
    let before = writes(&fake).len();
    for params in [json!({}), json!({"device": "fake:1"})] {
        let r = d.call("default", "release", params).await.unwrap();
        assert_eq!(
            r["settings_changed"],
            json!([{"device": "fake:1", "key": DARK, "was": "false", "set": "true"}])
        );
    }
    assert_eq!(writes(&fake).len(), before);
}

#[tokio::test(start_paused = true)]
async fn on_a_physical_device_restoring_a_setting_the_user_must_confirm_needs_force() {
    let (d, _fake, _home) = setup("android:R5CT123", true);
    ok(&d, json!({"key": NFC, "value": "false", "force": true})).await;
    let r = ok(&d, json!({"restore": []})).await;
    assert_eq!(restored(&r), [format!("{NFC} needs_force false")]);
    let r = ok(&d, json!({"restore": [], "force": true})).await;
    assert_eq!(restored(&r), [format!("{NFC} restored true")]);
}
