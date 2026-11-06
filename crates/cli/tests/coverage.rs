//! Task 3.10: Compose and WebView content in the snapshot, checked against the testbed's log.

#![allow(dead_code)]

#[path = "support/rig.rs"]
mod rig;

use std::time::Duration;

use rig::*;

fn by_id<'a>(els: &'a [serde_json::Value], id: &str) -> &'a serde_json::Value {
    els.iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("no element with id {id} in {els:#?}"))
}

async fn compose(rig: &Rig) {
    rig.adb
        .shell(&rig.serial, "am force-stop dev.phoneuse.testbed")
        .await
        .unwrap();
    rig.ok(&["snapshot", "--no-settle"]);
    testbed::open_tab(&rig.adb, &rig.serial, "compose").await;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if rig
            .ok(&["snapshot", "--no-settle"])
            .contains("Compose button")
        {
            break;
        }
    }
    testbed::clear_log(&rig.adb, &rig.serial).await;
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn compose_elements_have_roles_labels_and_their_test_tags_as_ids() {
    let rig = Rig::new().await;
    compose(&rig).await;
    let els = elements(&rig);
    let api = rig
        .adb
        .shell(&rig.serial, "getprop ro.build.version.sdk")
        .await
        .unwrap()
        .stdout_text();
    let title = els
        .iter()
        .find(|e| e["label"] == "Compose title")
        .expect("the title");
    let heading_flag_exists = api.trim().parse::<u32>().unwrap() >= 28;
    assert_eq!(
        title["role"],
        if heading_flag_exists {
            "heading"
        } else {
            "text"
        },
        "isHeading exists from API 28: {title}"
    );
    let button = by_id(&els, "compose_button");
    assert_eq!(button["role"], "button");
    assert_eq!(button["label"], "Compose button");
    let switch = by_id(&els, "compose_switch");
    assert_eq!(switch["role"], "switch", "{switch}");
    assert!(switch["state"].as_str().unwrap().contains("CHECKABLE"));
    if heading_flag_exists {
        assert_eq!(switch["value"], "Off");
    }
    assert_eq!(by_id(&els, "compose_field")["role"], "text_field");
    let list = by_id(&els, "compose_list");
    assert_eq!(list["role"], "list");
    assert_eq!(list["collection"]["rows"], 1000);
    let text = rig.ok(&["snapshot", "--no-settle"]);
    assert!(text.contains("switch off"), "{text}");
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn compose_controls_take_taps_and_text() {
    let rig = Rig::new().await;
    compose(&rig).await;
    rig.ok(&["tap", "Compose button"]);
    let log = events(&rig, "compose_click", 1).await;
    assert_eq!(log[0]["id"], "compose_button", "{log:?}");

    let els = elements(&rig);
    let switch = by_id(&els, "compose_switch")["ref"]
        .as_str()
        .unwrap()
        .to_string();
    let r = rig.json(&["tap", &format!("@{switch}")]);
    assert!(
        r["observation"]["text"]
            .as_str()
            .unwrap()
            .contains("off → on"),
        "{r}"
    );
    let log = events(&rig, "compose_toggle", 1).await;
    assert_eq!(log[0]["on"], true, "{log:?}");

    let field = by_id(&els, "compose_field")["ref"]
        .as_str()
        .unwrap()
        .to_string();
    let r = rig.json(&["type", UNICODE, "--into", &format!("@{field}")]);
    assert_eq!(r["verified"], true, "{r}");
    let log = testbed::wait_for_log(&rig.adb, &rig.serial, 3000, |l| {
        l.iter()
            .any(|e| e["ev"] == "compose_text" && e["value"] == UNICODE)
    })
    .await;
    assert!(
        log.iter()
            .any(|e| e["ev"] == "compose_text" && e["value"] == UNICODE),
        "{log:?}"
    );
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn a_compose_lazy_list_scrolls_to_item_300_and_the_tap_lands_on_it() {
    let rig = Rig::new().await;
    compose(&rig).await;
    let r = rig.json(&["scroll", "down", "--until", "Item 300"]);
    assert!(r["summary"].as_str().unwrap().contains("Item 300"), "{r}");
    rig.ok(&["tap", "Item 300"]);
    let log = events(&rig, "compose_item", 1).await;
    assert_eq!(log[0]["n"], 300, "{log:?}");
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn web_form_inputs_are_text_fields_and_take_text_taps_and_a_checkbox() {
    let rig = Rig::new().await;
    rig.tab("web").await;
    rig.ok(&["--timeout", "8000", "wait", "Send"]);
    let els = elements(&rig);
    let name = find(&els, "text_field", "Name");
    assert_eq!(name["role"], "text_field");
    let r = rig.json(&["type", "Ada Lovelace", "--into", "Name"]);
    assert_eq!(r["verified"], true, "{r}");
    let log = testbed::wait_for_log(&rig.adb, &rig.serial, 3000, |l| {
        l.iter()
            .any(|e| e["field"] == "name" && e["value"] == "Ada Lovelace")
    })
    .await;
    assert!(
        log.iter()
            .any(|e| e["ev"] == "web" && e["field"] == "name" && e["value"] == "Ada Lovelace"),
        "{log:?}"
    );
    rig.ok(&["tap", "Agree"]);
    let log = testbed::wait_for_log(&rig.adb, &rig.serial, 3000, |l| {
        l.iter().any(|e| e["field"] == "agree")
    })
    .await;
    assert!(
        log.iter()
            .any(|e| e["field"] == "agree" && e["value"] == true),
        "{log:?}"
    );
    rig.ok(&["tap", "Send"]);
    let log = testbed::wait_for_log(&rig.adb, &rig.serial, 3000, |l| {
        l.iter().any(|e| e["field"] == "send")
    })
    .await;
    assert!(log.iter().any(|e| e["field"] == "send"), "{log:?}");
}
