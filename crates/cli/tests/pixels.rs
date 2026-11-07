//! Task 7.2: when pixels are used, checked against the testbed's log on a WebView whose accessibility is off.

#![cfg(target_os = "macos")]
#![allow(dead_code)]

#[path = "support/rig.rs"]
mod rig;

use std::time::Duration;

use rig::*;

async fn opaque_web(rig: &Rig) {
    rig.adb
        .shell(&rig.serial, "am force-stop dev.phonedriver.testbed")
        .await
        .unwrap();
    rig.ok(&["snapshot", "--no-settle"]);
    testbed::open_tab(&rig.adb, &rig.serial, "webopaque").await;
    tokio::time::sleep(Duration::from_millis(3000)).await;
    testbed::clear_log(&rig.adb, &rig.serial).await;
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn a_webview_with_no_tree_gets_ocr_lines_in_a_plain_snapshot_and_they_can_be_tapped() {
    let rig = Rig::new().await;
    opaque_web(&rig).await;
    let mut shown = rig.ok(&["snapshot"]);
    for _ in 0..10 {
        if shown.contains("\"Send\" (ocr)") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(1000)).await;
        shown = rig.ok(&["snapshot"]);
    }
    assert!(shown.contains("webview"), "{shown}");
    assert!(shown.contains("\"Web form\" (ocr)"), "{shown}");
    assert!(shown.contains("\"Send\" (ocr)"), "{shown}");
    rig.ok(&["tap", "Send"]);
    let log = testbed::wait_for_log(&rig.adb, &rig.serial, 3000, |l| {
        l.iter().any(|e| e["ev"] == "web" && e["field"] == "send")
    })
    .await;
    assert!(
        log.iter().any(|e| e["ev"] == "web" && e["field"] == "send"),
        "{log:?}"
    );
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn pixels_forces_ocr_on_a_screen_whose_tree_is_fine() {
    let rig = Rig::new().await;
    rig.tab("web").await;
    rig.ok(&["--timeout", "8000", "wait", "Send"]);
    let plain = rig.ok(&["snapshot"]);
    assert!(!plain.contains("(ocr)"), "{plain}");
    let forced = rig.ok(&["snapshot", "--pixels"]);
    assert!(forced.contains("(ocr)"), "{forced}");
}
