//! End-to-end tests: the real CLI and daemon against the Android testbed, checked against the testbed's own event log.
//! Run with `ADB_SERVER_SOCKET=... PHONEDRIVER_TEST_DEVICE=android:emulator-5554 blaze test -p phonedriver --test e2e -- --ignored`.

#![allow(dead_code)]

#[path = "support/rig.rs"]
mod rig;

use rig::*;
use serde_json::Value;

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn snapshot_json_save_fixture_writes_the_tree_and_a_screenshot() {
    let rig = Rig::new().await;
    rig.tab("taps").await;
    let dir = std::env::temp_dir().join(format!("phonedriver-fixture-{}", std::process::id()));
    let result = rig.json(&[
        "snapshot",
        "--save-fixture",
        "settings_main",
        "--fixture-dir",
        dir.to_str().unwrap(),
    ]);
    assert!(result["text"].as_str().unwrap().contains("@e"));
    let tree: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("settings_main.json")).unwrap())
            .unwrap();
    assert!(tree["nodes"].as_array().unwrap().len() > 10);
    assert!(tree["density"].as_f64().unwrap() > 1.0);
    let png = std::fs::read(dir.join("settings_main.png")).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn tap_on_a_ref_lands_within_one_dp_of_the_buttons_center() {
    let rig = Rig::new().await;
    rig.tab("taps").await;
    let els = elements(&rig);
    let b17 = find(&els, "button", "b17");
    let r = b17["ref"].as_str().unwrap();
    let (cx, cy) = center(b17);
    let out = rig.ok(&["tap", &format!("@{r}")]);
    assert!(out.starts_with(&format!("ok tapped @{r} \"b17\"")), "{out}");
    let taps = events(&rig, "tap", 1).await;
    assert_eq!(taps[0]["id"], "b17", "{taps:?}");
    assert!(near(&taps[0]["x_dp"], cx), "{taps:?} vs {cx},{cy}");
    assert!(near(&taps[0]["y_dp"], cy), "{taps:?} vs {cx},{cy}");
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn tap_at_a_point_lands_within_one_dp_of_it() {
    let rig = Rig::new().await;
    rig.tab("taps").await;
    rig.ok(&["tap", "120,300"]);
    let taps = events(&rig, "tap", 1).await;
    assert!(near(&taps[0]["x_dp"], 120.0), "{taps:?}");
    assert!(near(&taps[0]["y_dp"], 300.0), "{taps:?}");
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn type_enters_unicode_exactly_and_reports_verified() {
    let rig = Rig::new().await;
    rig.tab("text").await;
    let els = elements(&rig);
    let plain = find(&els, "text_field", "plain")["ref"]
        .as_str()
        .unwrap()
        .to_string();
    let out = rig.ok(&["type", UNICODE, "--into", &format!("@{plain}")]);
    assert!(out.contains("verified"), "{out}");
    assert!(!out.contains("NOT verified"), "{out}");
    let log = testbed::wait_for_log(&rig.adb, &rig.serial, 3000, |l| {
        l.iter().any(|e| e["ev"] == "text" && e["value"] == UNICODE)
    })
    .await;
    let last = log
        .iter()
        .rev()
        .find(|e| e["ev"] == "text" && e["field"] == "plain")
        .unwrap_or_else(|| panic!("no text event in {log:?}"));
    assert_eq!(last["value"], UNICODE);
    let result = rig.json(&["type", "", "--into", &format!("@{plain}"), "--mode", "set"]);
    assert_eq!(
        result["verified"], false,
        "an empty string is not on the field after set"
    );
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn a_secret_from_stdin_types_by_length_and_leaves_no_trace() {
    let rig = Rig::new().await;
    rig.tab("text").await;
    let els = elements(&rig);
    let field = find(&els, "secure_field", "password")["ref"]
        .as_str()
        .unwrap()
        .to_string();
    let out = rig
        .command()
        .args(["type", "--secret", "--into", &format!("@{field}")])
        .write_stdin("hunter2")
        .output()
        .unwrap();
    let shown =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{shown}");
    assert!(
        shown.contains("verified") && !shown.contains("NOT verified"),
        "{shown}"
    );
    assert!(!shown.contains("hunter2"), "{shown}");
    let log = testbed::wait_for_log(&rig.adb, &rig.serial, 3000, |l| {
        l.iter()
            .any(|e| e["ev"] == "text" && e["field"] == "password" && e["len"] == 7)
    })
    .await;
    assert!(
        log.iter()
            .any(|e| e["field"] == "password" && e["len"] == 7),
        "{log:?}"
    );
    nothing_under_the_home_mentions(&rig, "hunter2");
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn typing_plain_text_into_a_password_field_is_refused_and_nothing_is_typed() {
    let rig = Rig::new().await;
    rig.tab("text").await;
    let els = elements(&rig);
    let field = find(&els, "secure_field", "password")["ref"]
        .as_str()
        .unwrap()
        .to_string();
    let out = rig.cli(&["type", "hunter2", "--into", &format!("@{field}")]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("password") && stderr.contains("--secret"),
        "{stderr}"
    );
    assert!(!stderr.contains("hunter2"), "{stderr}");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let log = testbed::read_log(&rig.adb, &rig.serial).await;
    assert!(log.iter().all(|e| e["field"] != "password"), "{log:?}");
    nothing_under_the_home_mentions(&rig, "hunter2");
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn a_secret_on_argv_is_a_usage_error_and_nothing_is_typed() {
    let rig = Rig::new().await;
    rig.tab("text").await;
    let out = rig.cli(&["type", "--secret", "hunter2"]);
    assert_eq!(out.status.code(), Some(2));
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let log = testbed::read_log(&rig.adb, &rig.serial).await;
    assert!(log.iter().all(|e| e["ev"] != "text"), "{log:?}");
}

async fn display_px(rig: &Rig) -> (f64, f64, f64) {
    let wm = |what: &'static str| {
        let adb = rig.adb.clone();
        let serial = rig.serial.clone();
        async move {
            adb.shell(&serial, &format!("wm {what}"))
                .await
                .unwrap()
                .stdout_text()
        }
    };
    let (size, density) = (wm("size").await, wm("density").await);
    let last = |text: &str| {
        text.lines()
            .rfind(|l| l.contains(':'))
            .unwrap()
            .split(':')
            .nth(1)
            .unwrap()
            .trim()
            .to_string()
    };
    let size = last(&size);
    let (w, h) = size.split_once('x').unwrap();
    (
        w.parse().unwrap(),
        h.parse().unwrap(),
        last(&density).parse::<f64>().unwrap() / 160.0,
    )
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn screenshot_has_the_display_size_in_points() {
    let rig = Rig::new().await;
    rig.tab("taps").await;
    let shot = rig.json(&["screenshot"]);
    let (w, h, density) = display_px(&rig).await;
    assert!(
        (shot["width"].as_f64().unwrap() - w / density).abs() <= 1.0,
        "{shot} {w}x{h}@{density}"
    );
    assert!(
        (shot["height"].as_f64().unwrap() - h / density).abs() <= 1.0,
        "{shot} {w}x{h}@{density}"
    );
    let path = shot["path"].as_str().unwrap();
    assert_eq!(&std::fs::read(path).unwrap()[..8], b"\x89PNG\r\n\x1a\n");
    let keep = std::env::var("PHONEDRIVER_KEEP_SCREENSHOT").ok();
    if let Some(dest) = keep {
        std::fs::copy(path, dest).unwrap();
    }
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn a_busy_screen_snapshots_without_an_idle_wait_and_says_loading() {
    let rig = Rig::new().await;
    rig.tab("busy").await;
    rig.ok(&["snapshot"]);
    let start = std::time::Instant::now();
    let r = rig.json(&["snapshot"]);
    let took = start.elapsed();
    let text = r["text"].as_str().unwrap();
    assert_eq!(r["settled"], true, "{text}");
    assert!(
        r["settle_ms"].as_u64().unwrap() < 100,
        "no idle wait: {text}"
    );
    assert!(text.lines().nth(1).unwrap().contains("loading"), "{text}");
    assert!(
        took < std::time::Duration::from_millis(600),
        "{took:?}: a tree read through the SSH tunnel is about 120 ms and the debug binary adds more"
    );
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn a_secure_screen_says_so_and_its_tree_is_still_readable() {
    let rig = Rig::new().await;
    rig.tab("text").await;
    let els = elements(&rig);
    let plain = find(&els, "text_field", "plain")["ref"]
        .as_str()
        .unwrap()
        .to_string();
    rig.ok(&["tap", &format!("@{plain}")]);
    assert!(rig.ok(&["snapshot"]).contains("keyboard shown"));
    rig.tab("secure").await;
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    let text = rig.ok(&["snapshot"]);
    let header = |t: &str| {
        t.lines()
            .find(|l| l.starts_with("app "))
            .unwrap_or_default()
            .to_string()
    };
    assert!(header(&text).contains("secure content hidden"), "{text}");
    assert!(text.contains("SECRET code 123456"), "{text}");
    rig.tab("taps").await;
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    let text = rig.ok(&["snapshot"]);
    assert!(!text.contains("secure content hidden"), "{text}");
}
