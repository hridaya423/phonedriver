mod support;

use std::time::Duration;

use phone_use_android::device::AndroidDevice;
use phone_use_core::device::{
    AppId, Button, ClipboardOp, Device, DeviceEvent, Gesture, ImageFormat, Key, Quality, TextInput,
    TimedPoint, Url,
};
use phone_use_core::element::Role;
use phone_use_core::geometry::Point;
use phone_use_core::prune::{Node, prune};
use support::agent as raw;
use support::testbed;

const UNICODE: &str = "héllo 世界 مرحبا 👋🏽";

async fn connect() -> (AndroidDevice, String, tokio::sync::MutexGuard<'static, ()>) {
    let guard = raw::exclusive().await;
    let (adb, serial) = (support::adb(), support::serial());
    testbed::ensure_installed(&adb, &serial).await;
    let device = AndroidDevice::connect(&adb, &serial).await.unwrap();
    (device, serial, guard)
}

async fn nodes(device: &AndroidDevice) -> Vec<Node> {
    prune(&device.tree().await.unwrap())
}

async fn wait_for_node(
    device: &AndroidDevice,
    within: Duration,
    pred: impl Fn(&Node) -> bool,
) -> Option<Node> {
    let start = std::time::Instant::now();
    loop {
        if let Some(n) = nodes(device).await.into_iter().find(&pred) {
            return Some(n);
        }
        if start.elapsed() > within {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn tap_at(p: Point) -> Gesture {
    Gesture {
        fingers: vec![vec![
            TimedPoint { at: p, t_ms: 0 },
            TimedPoint { at: p, t_ms: 50 },
        ]],
    }
}

fn label_starts(n: &Node, text: &str) -> bool {
    n.label
        .as_deref()
        .is_some_and(|l| l.to_lowercase().starts_with(&text.to_lowercase()))
}

async fn open_settings(serial: &str) {
    let adb = support::adb();
    adb.shell(serial, "am force-stop com.android.settings")
        .await
        .unwrap();
    adb.shell(serial, "am start -a android.settings.SETTINGS")
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn settings_tree_has_the_network_and_internet_button_and_a_tap_opens_it() {
    let (device, serial, _g) = connect().await;
    open_settings(&serial).await;
    let info = device.info().clone();
    assert!(info.id.starts_with("android:"));
    assert!(
        info.screen.w > 200.0 && info.screen.w < 1000.0,
        "points: {info:?}"
    );

    let button = wait_for_node(&device, Duration::from_secs(5), |n| {
        n.role == Role::Button && label_starts(n, "Network & internet")
    })
    .await
    .expect("a Button labeled Network & internet");
    device
        .gesture(tap_at(button.bounds.center()))
        .await
        .unwrap();

    let airplane = wait_for_node(&device, Duration::from_secs(2), |n| {
        label_starts(n, "Airplane mode")
    })
    .await;
    assert!(
        airplane.is_some(),
        "the Network & internet screen did not open"
    );
    let screen = nodes(&device).await;
    assert!(
        !screen
            .iter()
            .any(|n| n.role == Role::Button && label_starts(n, "Connected devices")),
        "the main Settings list should be gone"
    );
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn screenshot_is_a_png_of_the_display_in_points_and_frames_move_after_input() {
    let (device, serial, _g) = connect().await;
    testbed::open_tab(&support::adb(), &serial, "taps").await;
    tokio::time::sleep(Duration::from_millis(800)).await;
    let image = device.screenshot(Quality(90)).await.unwrap();
    assert_eq!(image.format, ImageFormat::Png);
    let info = device.info();
    assert!(
        (image.width as f32 - info.screen.w).abs() <= 1.0,
        "{image:?} {info:?}"
    );
    assert!(
        (image.height as f32 - info.screen.h).abs() <= 1.0,
        "{image:?} {info:?}"
    );
    assert_eq!(&image.bytes[..8], b"\x89PNG\r\n\x1a\n");

    let mut frames = device.frames().expect("android has a frame stream");
    let before = frames.borrow_and_update().seq;
    let toggle = wait_for_node(&device, Duration::from_secs(3), |n| n.role == Role::Switch)
        .await
        .expect("the Dark theme switch");
    device
        .gesture(tap_at(toggle.bounds.center()))
        .await
        .unwrap();
    let moved = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            frames.changed().await.unwrap();
            let f = *frames.borrow();
            if f.seq > before && f.changed_area_pt2 > 0.0 {
                return f;
            }
        }
    })
    .await
    .expect("a changed frame after the tap");
    assert!(moved.changed_box.is_some());
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn text_clipboard_keys_buttons_apps_and_events() {
    let (device, serial, _g) = connect().await;
    let adb = support::adb();
    let mut events = device.events();
    testbed::open_tab(&adb, &serial, "text").await;
    let field = wait_for_node(&device, Duration::from_secs(3), |n| {
        n.id.as_deref().is_some_and(|i| i.ends_with(":id/plain"))
    })
    .await
    .expect("the plain field");
    device.gesture(tap_at(field.bounds.center())).await.unwrap();
    tokio::time::sleep(Duration::from_millis(800)).await;

    device
        .clipboard(ClipboardOp::Set("OLD-CLIP".into()))
        .await
        .unwrap();
    testbed::clear_log(&adb, &serial).await;
    device.text(TextInput::Paste(UNICODE.into())).await.unwrap();
    let log = testbed::wait_for_log(&adb, &serial, 2000, |l| {
        l.iter().any(|e| e["ev"] == "text" && e["value"] == UNICODE)
    })
    .await;
    assert!(
        log.iter()
            .any(|e| e["field"] == "plain" && e["value"] == UNICODE),
        "{log:?}"
    );
    assert_eq!(
        device.clipboard(ClipboardOp::Get).await.unwrap().as_deref(),
        Some("OLD-CLIP")
    );

    testbed::clear_log(&adb, &serial).await;
    device.key(Key::Backspace).await.unwrap();
    let log = testbed::wait_for_log(&adb, &serial, 2000, |l| !l.is_empty()).await;
    assert!(log.iter().any(|e| e["ev"] == "text"), "{log:?}");

    let apps = device.apps().await.unwrap();
    assert!(
        apps.iter()
            .any(|a| a.id.0 == testbed::PACKAGE && a.name == "PhoneUse Testbed")
    );

    device.button(Button::Home).await.unwrap();
    let mut left = false;
    for _ in 0..30 {
        if device
            .foreground()
            .await
            .unwrap()
            .is_some_and(|a| a.0 != testbed::PACKAGE)
        {
            left = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(left, "Home did not leave the testbed");
    device
        .launch(&AppId(testbed::PACKAGE.into()))
        .await
        .unwrap();
    let mut back = false;
    for _ in 0..30 {
        if device.foreground().await.unwrap() == Some(AppId(testbed::PACKAGE.into())) {
            back = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(back, "launch did not foreground the testbed");

    let mut saw_window = false;
    while let Ok(e) = events.try_recv() {
        saw_window |= matches!(e, DeviceEvent::WindowChanged { .. });
    }
    assert!(saw_window, "no WindowChanged event during Home and launch");

    let handled = device.open_url(&Url("https://example.com".into())).await;
    assert!(handled.is_ok(), "{handled:?}");
    device.button(Button::Home).await.unwrap();
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn screenshots_keep_working_while_the_screen_is_idle_and_no_frames_arrive() {
    let (device, serial, _g) = connect().await;
    testbed::open_tab(&support::adb(), &serial, "taps").await;
    let mut frames = device.frames().expect("android has a frame stream");
    let start = std::time::Instant::now();
    let mut idle_for = Duration::ZERO;
    let mut last = frames.borrow_and_update().seq;
    let mut last_at = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(25) && idle_for < Duration::from_millis(1500) {
        if tokio::time::timeout(Duration::from_millis(100), frames.changed())
            .await
            .is_ok()
            && frames.borrow().seq != last
        {
            last = frames.borrow().seq;
            last_at = std::time::Instant::now();
        }
        idle_for = last_at.elapsed();
    }
    assert!(
        idle_for >= Duration::from_millis(1500),
        "this emulator kept sending frames for 25 s, so the idle case was not reached"
    );
    let image = device.screenshot(Quality(90)).await;
    assert!(image.is_ok(), "{image:?}");
}
