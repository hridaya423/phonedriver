//! Live tests against approved physical devices. `PHONE_USE_TEST_IOS_EXPECT` lists `udid=tier` pairs, for example
//! `00008142-001971E2346B801C=coredevice,00008110-00142036217A201E=runner`.

use phone_use_ios::discovery;

fn expected() -> Vec<(String, String)> {
    std::env::var("PHONE_USE_TEST_IOS_EXPECT")
        .expect("PHONE_USE_TEST_IOS_EXPECT")
        .split(',')
        .map(|pair| {
            let (udid, tier) = pair.split_once('=').expect("udid=tier");
            (udid.to_string(), tier.to_string())
        })
        .collect()
}

#[tokio::test]
#[ignore = "needs approved physical iOS devices"]
async fn discovery_lists_each_wired_device_with_the_tier_its_os_and_services_allow() {
    let found = discovery::list().await.unwrap();
    for (udid, tier) in expected() {
        let device = found
            .iter()
            .find(|d| d.udid == udid)
            .unwrap_or_else(|| panic!("{udid} not listed in {found:?}"));
        assert_eq!(device.tier.name(), tier, "{device:?}");
        assert!(
            !device.name.is_empty() && !device.model.is_empty(),
            "{device:?}"
        );
        eprintln!("{device:?}");
    }
}

fn testbed_log(udid: &str) -> String {
    let dir = std::env::temp_dir().join(format!("putb-{udid}.log"));
    let status = std::process::Command::new("xcrun")
        .args(["devicectl", "device", "copy", "from", "--device", udid])
        .args(["--domain-type", "appDataContainer"])
        .args(["--domain-identifier", "dev.phoneuse.testbed"])
        .args(["--source", "Documents/putb.log", "--destination"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(status.status.success(), "{status:?}");
    std::fs::read_to_string(dir).unwrap()
}

/// The testbed must be in front on the Taps tab of the iPad: the tap lands on B3 at (895, 540) of the stream's frame.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs approved physical iOS devices"]
async fn coredevice_tier_streams_the_screen_and_a_tap_reaches_the_testbed() {
    use phone_use_core::device::{Device, Gesture, TimedPoint};
    use phone_use_core::geometry::Point;
    let udid = std::env::var("PHONE_USE_TEST_IOS_UDID").expect("PHONE_USE_TEST_IOS_UDID");
    let wired = phone_use_ios::transport::wired()
        .await
        .unwrap()
        .into_iter()
        .find(|w| w.udid == udid)
        .unwrap();
    let device = phone_use_ios::coredevice::CoreDevice::connect(&wired, "iPad17,3")
        .await
        .unwrap();
    let shot = device
        .screenshot(phone_use_core::device::Quality(80))
        .await
        .unwrap();
    assert_eq!((shot.width, shot.height), (1032, 1368));
    let before = testbed_log(&udid).matches("\"ev\":\"tap\"").count();
    let at = Point { x: 895.0, y: 540.0 };
    let p = TimedPoint { at, t_ms: 0 };
    device
        .gesture(Gesture {
            fingers: vec![vec![p, p]],
        })
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let log = testbed_log(&udid);
    assert_eq!(log.matches("\"ev\":\"tap\"").count(), before + 1, "{log}");
    assert!(
        log.lines().last().unwrap().contains("\"id\":\"b3\""),
        "{log}"
    );
}

/// Taps the Text tab and the Plain field of the testbed on the iPad, then types into it. Paste is not asserted:
/// it raises the system's "Allow Paste" prompt (spike I1).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs approved physical iOS devices"]
async fn coredevice_tier_types_into_a_focused_field() {
    use phone_use_core::device::{Device, Gesture, TextInput, TimedPoint};
    use phone_use_core::geometry::Point;
    let udid = std::env::var("PHONE_USE_TEST_IOS_UDID").expect("PHONE_USE_TEST_IOS_UDID");
    let wired = phone_use_ios::transport::wired()
        .await
        .unwrap()
        .into_iter()
        .find(|w| w.udid == udid)
        .unwrap();
    let device = phone_use_ios::coredevice::CoreDevice::connect(&wired, "iPad17,3")
        .await
        .unwrap();
    let tap = async |x: f32, y: f32| {
        let p = TimedPoint {
            at: Point { x, y },
            t_ms: 0,
        };
        device
            .gesture(Gesture {
                fingers: vec![vec![p, p]],
            })
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    };
    tap(177.0, 1318.0).await;
    tap(500.0, 93.0).await;
    device
        .text(TextInput::Keystrokes("Ab1!".into()))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    let log = testbed_log(&udid);
    let values: Vec<&str> = log
        .lines()
        .filter(|l| l.contains("\"field\":\"plain\""))
        .collect();
    eprintln!("{}", values.join("\n"));
    assert!(values.last().unwrap().contains("Ab1!"), "{values:?}");
}

/// The Home button sends the testbed to the background.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs approved physical iOS devices"]
async fn coredevice_tier_home_button_backgrounds_the_app() {
    use phone_use_core::device::{Button, Device};
    let udid = std::env::var("PHONE_USE_TEST_IOS_UDID").expect("PHONE_USE_TEST_IOS_UDID");
    let wired = phone_use_ios::transport::wired()
        .await
        .unwrap()
        .into_iter()
        .find(|w| w.udid == udid)
        .unwrap();
    let device = phone_use_ios::coredevice::CoreDevice::connect(&wired, "iPad17,3")
        .await
        .unwrap();
    device.button(Button::Home).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    let log = testbed_log(&udid);
    assert!(
        log.lines()
            .last()
            .unwrap()
            .contains("\"state\":\"background\""),
        "{log}"
    );
}
