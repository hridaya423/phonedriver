mod support;

use support::testbed;

#[test]
fn parse_log_keeps_only_json_lines() {
    let log = testbed::parse_log(
        "{\"ev\":\"tap\",\"id\":\"b1\"}\n--------- beginning of main\nnoise\n{\"ev\":\"tab\"}\n",
    );
    assert_eq!(log.len(), 2);
    assert_eq!(log[0]["id"], "b1");
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn every_tab_logs_its_selection() {
    let (adb, serial) = (support::adb(), support::serial());
    testbed::install(&adb, &serial).await;
    for tab in [
        "taps", "text", "list", "gestures", "busy", "secure", "dialogs", "web",
    ] {
        testbed::clear_log(&adb, &serial).await;
        testbed::open_tab(&adb, &serial, tab).await;
        let log = testbed::wait_for_log(&adb, &serial, 3000, |l| !l.is_empty()).await;
        assert!(
            log.iter().any(|e| e["ev"] == "tab" && e["name"] == tab),
            "{tab}: {log:?}"
        );
    }
}
