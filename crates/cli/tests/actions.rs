//! Tasks 3.2 to 3.6: what the verbs return and what they do on the device, checked against the testbed's log.

#![allow(dead_code)]

#[path = "support/rig.rs"]
mod rig;

use rig::*;

async fn open_settings(rig: &Rig) {
    rig.adb
        .shell(&rig.serial, "am force-stop com.android.settings")
        .await
        .unwrap();
    rig.adb
        .shell(&rig.serial, "am start -a android.settings.SETTINGS")
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    rig.ok(&["snapshot", "--no-settle"]);
}

fn text(r: &serde_json::Value) -> &str {
    r["observation"]["text"].as_str().unwrap()
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn tapping_a_row_that_opens_another_screen_returns_a_full_snapshot() {
    let rig = Rig::new().await;
    open_settings(&rig).await;
    let r = rig.json(&["--expect-change", "1500", "tap", "Display"]);
    assert_eq!(r["observation"]["kind"], "full", "{r}");
    assert_eq!(r["changed"], true, "{r}");
    assert!(text(&r).contains("@e"), "{r}");
    rig.adb
        .shell(&rig.serial, "input keyevent KEYCODE_HOME")
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn toggling_a_switch_returns_one_changed_line() {
    let rig = Rig::new().await;
    rig.tab("taps").await;
    let r = rig.json(&["tap", "Dark theme"]);
    assert_eq!(r["observation"]["kind"], "diff", "{r}");
    let lines: Vec<&str> = text(&r).lines().collect();
    let element = |l: &str| l.split(' ').nth(1).map(str::to_string);
    assert!(
        lines
            .iter()
            .all(|l| l.starts_with("~ @e") && element(l) == element(lines[0])),
        "every line is about the one switch: {lines:?}"
    );
    assert!(
        lines.len() == 1 || rig.serial == "emulator-5556",
        "one changed field, except on API 26 where the switch also flips the word ON or OFF in its label: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("off") && l.contains("on")),
        "{lines:?}"
    );
    let log = testbed::read_log(&rig.adb, &rig.serial).await;
    assert!(
        log.iter().any(|e| e["ev"] == "toggle" && e["on"] == true),
        "{log:?}"
    );
    rig.ok(&["tap", "Dark theme"]);
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn typing_into_a_field_returns_a_value_line_and_a_password_returns_no_value() {
    let rig = Rig::new().await;
    rig.tab("text").await;
    let r = rig.json(&["type", "hello", "--into", "plain"]);
    let changed: Vec<&str> = text(&r).lines().filter(|l| l.starts_with("~ @e")).collect();
    assert!(
        changed
            .iter()
            .any(|l| l.contains("textbox") && l.contains("→ \"hello\"")),
        "{r}"
    );

    let r = rig.json(&["type", "hunter2", "--into", "password"]);
    let t = text(&r);
    assert!(t.contains("secure field changed"), "{t}");
    assert!(!t.contains("hunter2"), "{t}");
    assert!(!r.to_string().contains("hunter2"), "{r}");
}

fn failure(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn a_text_target_taps_the_row_that_holds_the_text() {
    let rig = Rig::new().await;
    open_settings(&rig).await;
    let r = rig.json(&["--expect-change", "1500", "tap", "Network & internet"]);
    let summary = r["summary"].as_str().unwrap().to_lowercase();
    assert!(summary.contains("network & internet"), "{r}");
    assert_eq!(r["observation"]["kind"], "full", "{r}");
    rig.adb
        .shell(&rig.serial, "input keyevent KEYCODE_HOME")
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn list_rows_render_as_cells_with_their_position_and_the_list_says_its_size() {
    let rig = Rig::new().await;
    rig.tab("taps").await;
    rig.tab("list").await;
    let r = rig.json(&["snapshot", "--no-settle"]);
    assert!(
        r["text"].as_str().unwrap().contains("list 1000 rows"),
        "{r}"
    );
    let els = r["elements"].as_array().unwrap();
    let row = els.iter().find(|e| e["label"] == "Row 3").expect("Row 3");
    assert_eq!(row["role"], "cell");
    assert_eq!(row["position"]["row"], 3);
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn tapping_row_5_by_text_taps_row_5_and_ambiguous_and_misspelled_texts_fail_with_help() {
    let rig = Rig::new().await;
    rig.tab("list").await;
    rig.ok(&["tap", "Row 5"]);
    let log = testbed::wait_for_log(&rig.adb, &rig.serial, 3000, |l| {
        l.iter().any(|e| e["ev"] == "row")
    })
    .await;
    assert!(
        log.iter().any(|e| e["ev"] == "row" && e["n"] == 5),
        "{log:?}"
    );

    let out = rig.cli(&["tap", "Row"]);
    let err = failure(&out);
    assert!(!out.status.success());
    assert!(err.contains("elements match \"Row\""), "{err}");
    assert!(err.contains("hint: Use a ref instead: @e"), "{err}");

    let out = rig.cli(&["tap", "Rwo 5"]);
    let err = failure(&out);
    assert!(err.contains("no element matches \"Rwo 5\""), "{err}");
    assert!(err.contains("\"Row 5\""), "{err}");
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn find_lists_the_visible_rows_whose_text_contains_the_query() {
    let rig = Rig::new().await;
    rig.tab("list").await;
    let out = rig.ok(&["find", "row 1", "--role", "cell"]);
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.iter().any(|l| l.ends_with("cell \"Row 1\"")), "{out}");
    assert!(
        lines.iter().any(|l| l.ends_with("cell \"Row 10\"")),
        "{out}"
    );
    assert!(lines.iter().all(|l| l.contains(" cell \"Row 1")), "{out}");
    assert!(!lines.iter().any(|l| l.contains("\"Row 2\"")), "{out}");
    assert_eq!(rig.ok(&["find", "zzzz"]).trim(), "no match");
}
