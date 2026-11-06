//! Task 3.8: one test per row of architecture 6.3 that a phone can show, checked against the testbed's log.
//! The secure-window row is `e2e::a_secure_screen_says_so_and_its_tree_is_still_readable`.

#![allow(dead_code)]

#[path = "support/rig.rs"]
mod rig;

use std::time::Duration;

use rig::*;
use serde_json::Value;

/// Runs shell commands when dropped, so a failing test still leaves the device as it found it.
struct Restore {
    serial: String,
    cmds: Vec<String>,
}

impl Restore {
    fn new<S: AsRef<str>>(rig: &Rig, cmds: &[S]) -> Restore {
        Restore {
            serial: rig.serial.clone(),
            cmds: cmds.iter().map(|c| c.as_ref().to_string()).collect(),
        }
    }
}

impl Drop for Restore {
    fn drop(&mut self) {
        for c in &self.cmds {
            let _ = std::process::Command::new("adb")
                .args(["-s", &self.serial, "shell", c])
                .output();
        }
    }
}

async fn sh(rig: &Rig, cmd: &str) -> String {
    rig.adb.shell(&rig.serial, cmd).await.unwrap().stdout_text()
}

async fn until<F: std::future::Future<Output = bool>>(what: &str, mut f: impl FnMut() -> F) {
    for _ in 0..80 {
        if f().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("timed out waiting for {what}");
}

async fn asleep(rig: &Rig) -> bool {
    sh(rig, "dumpsys power")
        .await
        .contains("mWakefulness=Asleep")
}

async fn sleep_screen(rig: &Rig) {
    sh(rig, "input keyevent KEYCODE_SLEEP").await;
    until("the screen to go off", || asleep(rig)).await;
}

const WAKE_AND_DISMISS: [&str; 3] = [
    "input keyevent KEYCODE_WAKEUP",
    "wm dismiss-keyguard",
    "input keyevent KEYCODE_HOME",
];

/// API 26 has no `get-disabled`; the emulator image starts with the lock screen off, like API 37's.
async fn lock_disabled(rig: &Rig) -> String {
    let out = sh(rig, "locksettings get-disabled").await;
    match out.trim() {
        "false" => "false".into(),
        _ => "true".into(),
    }
}

/// The keyguard engages 5 s after the screen goes off unless this is 0, so a lock test would race it.
const LOCK_AT_ONCE: &str = "settings put secure lock_screen_lock_after_timeout 0";

async fn toggled(rig: &Rig, on: bool) -> bool {
    testbed::wait_for_log(&rig.adb, &rig.serial, 3000, |l| {
        l.iter().any(|e| e["ev"] == "toggle" && e["on"] == on)
    })
    .await
    .iter()
    .any(|e| e["ev"] == "toggle" && e["on"] == on)
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn a_dark_screen_is_woken_before_the_tap_and_the_reply_says_so() {
    let rig = Rig::new().await;
    rig.tab("taps").await;
    let _restore = Restore::new(&rig, &WAKE_AND_DISMISS);
    sleep_screen(&rig).await;
    testbed::clear_log(&rig.adb, &rig.serial).await;
    let r = rig.json(&["tap", "Dark theme"]);
    assert_eq!(r["woke"], true, "{r}");
    assert!(
        toggled(&rig, true).await,
        "the tap did not reach the testbed"
    );
    assert!(!asleep(&rig).await);
    rig.ok(&["tap", "Dark theme"]);
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn a_swipe_only_lock_screen_is_dismissed_and_the_tap_goes_through() {
    let rig = Rig::new().await;
    rig.tab("taps").await;
    let was = lock_disabled(&rig).await;
    let mut cleanup = vec![
        format!("locksettings set-disabled {was}"),
        "settings delete secure lock_screen_lock_after_timeout".to_string(),
    ];
    cleanup.extend(WAKE_AND_DISMISS.map(String::from));
    let _restore = Restore::new(&rig, &cleanup);
    sh(&rig, "locksettings set-disabled false").await;
    sh(&rig, LOCK_AT_ONCE).await;
    sleep_screen(&rig).await;
    sh(&rig, "input keyevent KEYCODE_WAKEUP").await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    testbed::clear_log(&rig.adb, &rig.serial).await;
    let r = rig.json(&["tap", "Dark theme"]);
    assert_eq!(r["unlocked"], true, "{r}");
    assert_ne!(r["woke"], true, "{r}");
    assert!(
        toggled(&rig, true).await,
        "the tap did not reach the testbed: {r}"
    );
    rig.ok(&["tap", "Dark theme"]);
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn a_pin_lock_screen_fails_needs_user_and_nothing_is_tapped() {
    let rig = Rig::new().await;
    rig.tab("taps").await;
    let was = lock_disabled(&rig).await;
    let mut cleanup = vec![
        "locksettings clear --old 1234".to_string(),
        "settings delete secure lock_screen_lock_after_timeout".to_string(),
        format!("locksettings set-disabled {was}"),
    ];
    cleanup.extend(WAKE_AND_DISMISS.map(String::from));
    let _restore = Restore::new(&rig, &cleanup);
    sh(&rig, "locksettings set-pin 1234").await;
    sh(&rig, LOCK_AT_ONCE).await;
    sleep_screen(&rig).await;
    testbed::clear_log(&rig.adb, &rig.serial).await;
    let out = rig.cli(&["tap", "Dark theme"]);
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(err.contains("locked"), "{err}");
    assert!(err.contains("hint: Unlock the device"), "{err}");
    let json = rig
        .command()
        .args(["--json", "tap", "Dark theme"])
        .output()
        .unwrap();
    let body =
        String::from_utf8_lossy(&json.stdout).to_string() + &String::from_utf8_lossy(&json.stderr);
    assert!(
        body.contains("unlock_device") || body.contains("locked"),
        "{body}"
    );
    tokio::time::sleep(Duration::from_millis(500)).await;
    let log = testbed::read_log(&rig.adb, &rig.serial).await;
    assert!(log.iter().all(|e| e["ev"] != "toggle"), "{log:?}");
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn another_ui_automation_client_fails_with_its_name_and_how_to_stop_it() {
    let rig = Rig::new().await;
    rig.cli(&["release"]);
    tokio::time::sleep(Duration::from_millis(2500)).await;
    let mut holder = rig
        .adb
        .shell_stream(&rig.serial, "uiautomator events")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let out = rig.cli(&["snapshot", "--no-settle"]);
    drop(holder.close_stdin().await);
    drop(holder);
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("UiAutomation"), "{err}");
    assert!(err.contains("hint: Stop"), "{err}");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    rig.ok(&["snapshot", "--no-settle"]);
}

fn header(r: &Value) -> &Value {
    &r["header"]["device"]
}

async fn screen_width(rig: &Rig) -> f64 {
    header(&rig.json(&["snapshot", "--no-settle"]))["screen"]["w"]
        .as_f64()
        .unwrap()
}

async fn tap_lands_on_b10(rig: &Rig) {
    testbed::clear_log(&rig.adb, &rig.serial).await;
    let els = elements(rig);
    let b = find(&els, "button", "b10").clone();
    let (cx, cy) = center(&b);
    rig.ok(&["tap", &format!("@{}", b["ref"].as_str().unwrap())]);
    let log = events(rig, "tap", 1).await;
    let tap = log.last().expect("the testbed saw no tap");
    assert_eq!(tap["id"], "b10", "{log:?}");
    assert!(
        (tap["x_dp"].as_f64().unwrap() - cx).abs() <= 1.5,
        "{tap} vs {cx},{cy}"
    );
    assert!(
        (tap["y_dp"].as_f64().unwrap() - cy).abs() <= 1.5,
        "{tap} vs {cx},{cy}"
    );
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn rotating_the_display_changes_the_header_and_taps_still_land() {
    let rig = Rig::new().await;
    rig.tab("taps").await;
    let was = sh(&rig, "settings get system accelerometer_rotation").await;
    let _restore = Restore::new(
        &rig,
        &[
            "settings put system user_rotation 0",
            &format!("settings put system accelerometer_rotation {}", was.trim()),
        ],
    );
    sh(&rig, "settings put system accelerometer_rotation 0").await;
    sh(&rig, "settings put system user_rotation 1").await;
    let mut landscape = false;
    for _ in 0..60 {
        let r = rig.json(&["snapshot", "--no-settle"]);
        if header(&r)["orientation"] == "landscape" {
            assert!(header(&r)["screen"]["w"].as_f64() > header(&r)["screen"]["h"].as_f64());
            landscape = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(landscape, "the header never said landscape");
    let text = rig.ok(&["snapshot"]);
    assert!(text.contains("landscape"), "{text}");
    tap_lands_on_b10(&rig).await;
    sh(&rig, "settings put system user_rotation 0").await;
    until("portrait again", || async {
        screen_width(&rig).await < 500.0
    })
    .await;
    tap_lands_on_b10(&rig).await;
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn a_changed_wm_size_updates_the_header_and_taps_still_land() {
    let rig = Rig::new().await;
    rig.tab("taps").await;
    let before = screen_width(&rig).await;
    let _restore = Restore::new(&rig, &["wm size reset"]);
    sh(&rig, "wm size 720x1280").await;
    until("the new size in the header", || async {
        (screen_width(&rig).await - before).abs() > 20.0
    })
    .await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    tap_lands_on_b10(&rig).await;
    sh(&rig, "wm size reset").await;
    until("the old size", || async {
        (screen_width(&rig).await - before).abs() < 2.0
    })
    .await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    tap_lands_on_b10(&rig).await;
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn a_changed_wm_density_updates_the_header_and_taps_still_land() {
    let rig = Rig::new().await;
    if sh(&rig, "getprop ro.build.version.sdk").await.trim() == "26" {
        eprintln!(
            "skipped: `wm density` restarts the system server of the API 26 emulator and drops adb"
        );
        return;
    }
    rig.tab("taps").await;
    let before = screen_width(&rig).await;
    let _restore = Restore::new(&rig, &["wm density reset"]);
    sh(&rig, "wm density 560").await;
    until("the new density in the header", || async {
        (screen_width(&rig).await - before).abs() > 20.0
    })
    .await;
    tokio::time::sleep(Duration::from_millis(2500)).await;
    tap_lands_on_b10(&rig).await;
    sh(&rig, "wm density reset").await;
    until("the old density", || async {
        (screen_width(&rig).await - before).abs() < 2.0
    })
    .await;
    tokio::time::sleep(Duration::from_millis(2500)).await;
    tap_lands_on_b10(&rig).await;
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn a_permission_dialog_shows_its_buttons_above_the_app() {
    let rig = Rig::new().await;
    // Two denials make Android answer the request without a dialog, so clearing the app resets the count.
    let reset = [
        "pm clear dev.phoneuse.testbed",
        "pm grant dev.phoneuse.testbed android.permission.POST_NOTIFICATIONS",
    ];
    let _restore = Restore::new(&rig, &reset);
    for c in reset {
        sh(&rig, c).await;
    }
    rig.tab("dialogs").await;
    tokio::time::sleep(Duration::from_millis(2500)).await;
    rig.ok(&["tap", "Ask permission"]);
    let mut text = String::new();
    for _ in 0..50 {
        text = rig.ok(&["snapshot"]);
        if text.contains("Allow") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let allow = text
        .lines()
        .position(|l| l.to_lowercase().contains("allow"));
    let app_row = text.lines().position(|l| l.contains("Ask permission"));
    assert!(allow.is_some(), "{text}");
    assert!(
        app_row.is_none_or(|a| allow.unwrap() < a),
        "the dialog's window must print before the app under it: {text}"
    );
    assert!(text.contains("contacts"), "{text}");
    let deny = text
        .lines()
        .find(|l| {
            let l = l.to_lowercase();
            (l.contains("don") || l.contains("deny")) && l.contains("button")
        })
        .unwrap_or_else(|| panic!("no deny button: {text}"))
        .to_string();
    let e = deny.split_whitespace().next().unwrap().to_string();
    rig.ok(&["tap", &e]);
}
