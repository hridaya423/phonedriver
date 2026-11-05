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
    fresh_list(&rig).await;
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
    fresh_list(&rig).await;
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
    fresh_list(&rig).await;
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

async fn fresh_list(rig: &Rig) {
    rig.adb
        .shell(&rig.serial, "am force-stop dev.phoneuse.testbed")
        .await
        .unwrap();
    rig.tab("list").await;
}

async fn open_list(rig: &Rig) -> Vec<serde_json::Value> {
    fresh_list(rig).await;
    elements(rig)
}

fn rows(els: &[serde_json::Value]) -> Vec<(u32, f64)> {
    els.iter()
        .filter_map(|e| {
            let n = e["label"].as_str()?.strip_prefix("Row ")?.parse().ok()?;
            Some((n, e["bounds"]["y"].as_f64()?))
        })
        .collect()
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn scroll_down_moves_the_list_about_60_percent_of_the_viewport_without_a_fling() {
    let rig = Rig::new().await;
    let els = open_list(&rig).await;
    let list = els
        .iter()
        .filter(|e| e["actions"].as_str().unwrap().contains("SCROLL_DOWN"))
        .max_by(|a, b| {
            a["bounds"]["h"]
                .as_f64()
                .partial_cmp(&b["bounds"]["h"].as_f64())
                .unwrap()
        })
        .unwrap_or_else(|| panic!("no scrollable list in {els:#?}"));
    let viewport = list["bounds"]["h"].as_f64().unwrap();
    let before = rows(&els);
    let r = rig.json(&["scroll", "down"]);
    assert_eq!(r["changed"], true, "{r}");
    let after = rows(&elements(&rig));
    let (n0, y0) = before[1];
    let y1 = after
        .iter()
        .find(|(n, _)| *n == n0)
        .map(|(_, y)| *y)
        .unwrap_or_else(|| {
            let (first, fy) = after[0];
            let h = before[2].1 - before[1].1;
            fy - f64::from(first - n0) * h
        });
    let moved = y0 - y1;
    assert!(
        (0.45 * viewport..=0.75 * viewport).contains(&moved),
        "moved {moved} of {viewport}"
    );
    let log = events(&rig, "scroll", 1).await;
    assert!(!log.is_empty());
    let all = testbed::read_log(&rig.adb, &rig.serial).await;
    assert!(
        all.iter()
            .filter(|e| e["ev"] == "scroll_state")
            .all(|e| e["state"] != 2),
        "the list flung: {all:?}"
    );
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn scroll_until_a_far_row_stops_with_it_visible_in_few_gestures() {
    let rig = Rig::new().await;
    open_list(&rig).await;
    let r = rig.json(&["scroll", "down", "--until", "Row 500", "--max", "30"]);
    let text = r["observation"]["text"].as_str().unwrap();
    assert!(text.contains("Row 500"), "{r}");
    assert!(r["summary"].as_str().unwrap().contains("Row 500"), "{r}");
    let gestures = testbed::read_log(&rig.adb, &rig.serial)
        .await
        .iter()
        .filter(|e| e["ev"] == "scroll_state" && e["state"] == 1)
        .count();
    assert!(gestures < 20, "{gestures} gestures");
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn scroll_until_a_row_that_does_not_exist_fails_not_found_at_the_end() {
    let rig = Rig::new().await;
    open_list(&rig).await;
    let out = rig.cli(&["scroll", "down", "--until", "Row 5000", "--max", "60"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("Row 5000"), "{err}");
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn a_ref_to_a_row_that_scrolled_away_is_stale_or_still_that_row() {
    let rig = Rig::new().await;
    let els = open_list(&rig).await;
    let row3 = els.iter().find(|e| e["label"] == "Row 3").unwrap();
    let r3 = row3["ref"].as_str().unwrap().to_string();
    rig.ok(&["scroll", "down"]);
    testbed::clear_log(&rig.adb, &rig.serial).await;
    let out = rig.cli(&["tap", &format!("@{r3}")]);
    let log = testbed::read_log(&rig.adb, &rig.serial).await;
    let taps: Vec<_> = log.iter().filter(|e| e["ev"] == "row").collect();
    if out.status.success() {
        assert!(taps.iter().all(|e| e["n"] == 3), "{taps:?}");
    } else {
        assert!(taps.is_empty(), "{taps:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("stale") || err.contains("no longer"), "{err}");
    }
}

async fn surface(rig: &Rig) -> (String, f64, f64) {
    rig.tab("gestures").await;
    let els = elements(rig);
    let e = find(&els, "text", "Gesture surface");
    let (x, y) = center(e);
    (e["ref"].as_str().unwrap().to_string(), x, y)
}

async fn touches(rig: &Rig, last: &str) -> Vec<serde_json::Value> {
    let log = testbed::wait_for_log(&rig.adb, &rig.serial, 4000, |l| {
        l.iter().any(|e| e["ev"] == "touch" && e["action"] == last)
    })
    .await;
    log.into_iter().filter(|e| e["ev"] == "touch").collect()
}

fn at(e: &serde_json::Value, action: &str) -> Vec<f64> {
    e.as_array()
        .unwrap()
        .iter()
        .filter(|t| t["action"] == action)
        .map(|t| t["t"].as_f64().unwrap())
        .collect()
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn tap_count_2_logs_two_taps_80_ms_apart() {
    let rig = Rig::new().await;
    let (r, _, _) = surface(&rig).await;
    rig.ok(&["tap", &format!("@{r}"), "--count", "2"]);
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let log = serde_json::Value::Array(touches(&rig, "ACTION_UP").await);
    let downs = at(&log, "ACTION_DOWN");
    let ups = at(&log, "ACTION_UP");
    assert!(downs.len() >= 2 && !ups.is_empty(), "{log}");
    let gap = downs[1] - downs[0];
    assert!((60.0..=100.0).contains(&gap), "gap {gap} ms: {log}");
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn swipe_up_from_a_ref_starts_at_the_center_of_that_element() {
    let rig = Rig::new().await;
    let (r, cx, cy) = surface(&rig).await;
    rig.ok(&["swipe", "up", "--from", &format!("@{r}")]);
    let log = touches(&rig, "ACTION_UP").await;
    let down = log.iter().find(|e| e["action"] == "ACTION_DOWN").unwrap();
    let p = &down["ptrs"][0];
    assert!((p["x_dp"].as_f64().unwrap() - cx).abs() <= 2.0, "{down}");
    assert!((p["y_dp"].as_f64().unwrap() - cy).abs() <= 2.0, "{down}");
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn swipe_left_is_one_finger_moving_left_and_fast_lifts_while_moving() {
    let rig = Rig::new().await;
    surface(&rig).await;
    rig.ok(&["swipe", "left"]);
    let log = touches(&rig, "ACTION_UP").await;
    assert!(
        log.iter().all(|e| e["ptrs"].as_array().unwrap().len() == 1),
        "one pointer only"
    );
    let xs: Vec<f64> = log
        .iter()
        .map(|e| e["ptrs"][0]["x_dp"].as_f64().unwrap())
        .collect();
    assert!(xs.len() >= 6, "{xs:?}");
    assert!(xs.windows(2).all(|w| w[1] <= w[0] + 0.5), "{xs:?}");
    assert!(xs[0] - xs[xs.len() - 1] > 100.0, "{xs:?}");

    testbed::clear_log(&rig.adb, &rig.serial).await;
    rig.ok(&["swipe", "left", "--fast"]);
    let log = touches(&rig, "ACTION_UP").await;
    let moves: Vec<&serde_json::Value> = log
        .iter()
        .filter(|e| e["action"] == "ACTION_MOVE")
        .collect();
    let n = moves.len();
    assert!(n >= 2, "{log:?}");
    let dx = moves[n - 2]["ptrs"][0]["x_dp"].as_f64().unwrap()
        - moves[n - 1]["ptrs"][0]["x_dp"].as_f64().unwrap();
    let dt = moves[n - 1]["t"].as_f64().unwrap() - moves[n - 2]["t"].as_f64().unwrap();
    let up = log.iter().find(|e| e["action"] == "ACTION_UP").unwrap();
    let gap = up["t"].as_f64().unwrap() - moves[n - 1]["t"].as_f64().unwrap();
    assert!(dx / dt * 1000.0 > 1000.0, "{dx} dp in {dt} ms");
    assert!(gap < 40.0, "lifted {gap} ms after the last move");
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn a_long_press_of_800_ms_logs_down_and_up_800_ms_apart() {
    let rig = Rig::new().await;
    let (r, _, _) = surface(&rig).await;
    rig.ok(&["tap", &format!("@{r}"), "--long", "800"]);
    let log = touches(&rig, "ACTION_UP").await;
    let d = at(&serde_json::Value::Array(log.clone()), "ACTION_DOWN")[0];
    let u = at(&serde_json::Value::Array(log.clone()), "ACTION_UP")[0];
    assert!((770.0..=830.0).contains(&(u - d)), "{} ms", u - d);
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn a_two_finger_pinch_logs_two_pointer_ids_and_pointer_down_and_up_on_schedule() {
    let rig = Rig::new().await;
    let (_, cx, cy) = surface(&rig).await;
    let g = serde_json::json!({"fingers": [
        [{"x": cx - 20.0, "y": cy, "t_ms": 0}, {"x": cx - 5.0, "y": cy, "t_ms": 150}, {"x": cx - 60.0, "y": cy, "t_ms": 400}],
        [{"x": cx + 20.0, "y": cy, "t_ms": 0}, {"x": cx + 5.0, "y": cy, "t_ms": 150}, {"x": cx + 60.0, "y": cy, "t_ms": 400}]
    ]});
    rig.ok(&["gesture", &g.to_string()]);
    let log = touches(&rig, "ACTION_UP").await;
    let all = serde_json::Value::Array(log.clone());
    let ids: std::collections::BTreeSet<i64> = log
        .iter()
        .flat_map(|e| e["ptrs"].as_array().unwrap().iter())
        .map(|p| p["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids.len(), 2, "{all}");
    assert!(
        log.iter().any(|e| e["action"]
            .as_str()
            .unwrap()
            .starts_with("ACTION_POINTER_DOWN")),
        "{all}"
    );
    assert!(
        log.iter().any(|e| e["action"]
            .as_str()
            .unwrap()
            .starts_with("ACTION_POINTER_UP")),
        "{all}"
    );
    let first = log[0]["t"].as_f64().unwrap();
    let last = log.last().unwrap()["t"].as_f64().unwrap();
    assert!(
        ((last - first) - 400.0).abs() <= 20.0,
        "{} ms",
        last - first
    );
    let mid = log
        .iter()
        .filter(|e| e["action"] == "ACTION_MOVE")
        .map(|e| e["t"].as_f64().unwrap() - first)
        .find(|t| (100.0..=200.0).contains(t))
        .expect("a move near 150 ms");
    assert!((mid - 150.0).abs() <= 20.0, "{mid}");
}
