//! Task 3.1: settle on real signals, against the testbed's Settle and Busy tabs.

#![allow(dead_code)]

#[path = "support/rig.rs"]
mod rig;

use std::time::Duration;

use rig::*;

async fn click_events(rig: &Rig, id: &str) -> usize {
    tokio::time::sleep(Duration::from_millis(300)).await;
    testbed::read_log(&rig.adb, &rig.serial)
        .await
        .iter()
        .filter(|e| e["ev"] == "click" && e["id"] == id)
        .count()
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn a_screen_that_opens_700_ms_after_the_tap_is_in_the_reply_with_a_wider_first_change_window()
{
    let rig = Rig::new().await;
    rig.tab("settle").await;
    let r = rig.json(&["--expect-change", "1500", "tap", "Slow open"]);
    assert_eq!(r["changed"], true, "{r}");
    assert_eq!(r["observation"]["kind"], "full", "{r}");
    assert!(
        r["observation"]["text"]
            .as_str()
            .unwrap()
            .contains("Second screen"),
        "{r}"
    );
    assert_eq!(r["settled"], true, "{r}");
    rig.tab("settle").await;
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn a_tap_that_does_nothing_says_so_within_the_first_change_window() {
    let rig = Rig::new().await;
    rig.tab("settle").await;
    let r = rig.json(&["tap", "Do nothing"]);
    assert_eq!(click_events(&rig, "nothing").await, 1, "{r}");
    assert_eq!(r["changed"], false, "{r}");
    assert_eq!(r["settled"], true, "{r}");
    let ms = r["settle_ms"].as_u64().unwrap();
    assert!((450..=600).contains(&ms), "{r}");
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn a_400_ms_animation_settles_after_it_ends_and_a_quiet_period() {
    let rig = Rig::new().await;
    rig.tab("settle").await;
    let r = rig.json(&["--expect-change", "1500", "tap", "Animate"]);
    assert_eq!(r["settled"], true, "{r}");
    assert_eq!(r["changed"], true, "{r}");
    let ms = r["settle_ms"].as_u64().unwrap();
    assert!((400..=2500).contains(&ms), "{r}");
    let log = testbed::read_log(&rig.adb, &rig.serial).await;
    assert!(log.iter().any(|e| e["ev"] == "animate_end"), "{log:?}");
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn a_ticker_does_not_keep_the_busy_tab_from_settling() {
    let rig = Rig::new().await;
    rig.tab("busy").await;
    rig.ok(&["snapshot"]);
    tokio::time::sleep(Duration::from_millis(400)).await;
    let r = rig.json(&["snapshot"]);
    let text = r["text"].as_str().unwrap();
    assert!(text.contains("Spinning"), "{text}");
    assert_eq!(r["settled"], true, "{text}");
    assert!(r["settle_ms"].as_u64().unwrap() < 300, "{text}");
    assert_eq!(r["loading"], true, "{text}");
    assert!(text.lines().nth(1).unwrap().contains("loading"), "{text}");
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn a_flashing_area_keeps_the_screen_unsettled_until_it_ends() {
    let rig = Rig::new().await;
    rig.tab("settle").await;
    rig.ok(&["--no-settle", "tap", "Flash"]);
    testbed::wait_for_log(&rig.adb, &rig.serial, 3000, |l| {
        l.iter().any(|e| e["ev"] == "flash_start")
    })
    .await;
    tokio::time::sleep(Duration::from_millis(3000)).await;

    let mut refused = false;
    for _ in 0..6 {
        let waited = rig.cli(&["--timeout", "1000", "wait", "settled"]);
        if !waited.status.success() {
            let err = String::from_utf8_lossy(&waited.stderr).to_string();
            assert!(err.contains("the screen was still changing after"), "{err}");
            refused = true;
            break;
        }
    }
    assert!(refused, "wait settled never refused while the area flashed");

    let mut unsettled = None;
    for _ in 0..6 {
        let r = rig.json(&["--timeout", "1000", "snapshot"]);
        if r["settled"] == false {
            unsettled = Some(r);
            break;
        }
    }
    let r = unsettled.expect("snapshot never timed out unsettled while the area flashed");
    assert!(r["settle_ms"].as_u64().unwrap() >= 1000, "{r}");
    let still = &r["still_changing"];
    let covered = still["w"].as_f64().unwrap() * still["h"].as_f64().unwrap();
    assert!(covered > 0.6 * 200.0 * 200.0, "{still}");
    assert!(r["text"].as_str().unwrap().contains("Flash"), "{r}");

    let log = testbed::wait_for_log(&rig.adb, &rig.serial, 20_000, |l| {
        l.iter().any(|e| e["ev"] == "flash_end")
    })
    .await;
    assert!(log.iter().any(|e| e["ev"] == "flash_end"), "{log:?}");
    tokio::time::sleep(Duration::from_millis(3000)).await;
    let r = rig.json(&["--timeout", "5000", "snapshot"]);
    assert_eq!(r["settled"], true, "{r}");
}
