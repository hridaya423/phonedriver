//! `IosDevice` against the iOS testbed on a booted simulator, checked against the testbed's event log.
//! `PHONEDRIVER_TEST_DEVICE=sim:<udid> cargo test -p phonedriver-ios --test device_live -- --ignored --test-threads=1`.

#![cfg(unix)]

use std::path::PathBuf;
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::Duration;

use phonedriver_core::device::{
    AppId, Button, ClipboardOp, Device, Gesture, Key, Quality, TextInput, TimedPoint, Url,
};
use phonedriver_core::error::Error;
use phonedriver_core::geometry::Point;
use phonedriver_core::raw::RawTree;
use phonedriver_ios::device::IosDevice;
use phonedriver_ios::simulator::Simulator;
use phonedriver_ios::testbed;
use serde_json::Value;
use tokio::runtime::Runtime;

static RT: LazyLock<Runtime> = LazyLock::new(|| Runtime::new().unwrap());
static DEVICE: OnceLock<Arc<IosDevice>> = OnceLock::new();

fn udid() -> String {
    let v =
        std::env::var("PHONEDRIVER_TEST_DEVICE").expect("set PHONEDRIVER_TEST_DEVICE=sim:<udid>");
    v.strip_prefix("sim:").unwrap_or(&v).to_string()
}

fn home() -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("phonedriver-home")
}

fn device() -> Arc<IosDevice> {
    DEVICE
        .get_or_init(|| {
            let udid = udid();
            testbed::install(&udid);
            RT.block_on(async {
                let sim = Simulator::list()
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|s| s.udid == udid)
                    .expect("simulator not booted");
                Arc::new(IosDevice::connect(&sim, &home()).await.unwrap())
            })
        })
        .clone()
}

fn tree(d: &IosDevice) -> RawTree {
    RT.block_on(d.tree()).unwrap()
}

fn center_of(tree: &RawTree, id: &str) -> Point {
    tree.nodes
        .iter()
        .find(|n| n.resource_id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("no node {id}"))
        .bounds
        .center()
}

fn tap(d: &IosDevice, at: Point) {
    let g = Gesture {
        fingers: vec![vec![TimedPoint { at, t_ms: 0 }, TimedPoint { at, t_ms: 0 }]],
    };
    RT.block_on(d.gesture(g)).unwrap();
}

fn tap_id(d: &IosDevice, id: &str) {
    let at = center_of(&tree(d), id);
    tap(d, at);
}

fn ev<'a>(e: &'a Value, name: &str) -> Option<&'a Value> {
    (e["ev"] == name).then_some(e)
}

#[test]
#[ignore]
fn the_tree_has_the_testbed_as_its_application_window() {
    let d = device();
    testbed::launch(&udid(), "taps");
    let t = tree(&d);
    let b17 = t
        .nodes
        .iter()
        .find(|n| n.resource_id.as_deref() == Some("taps.b17"))
        .unwrap();
    assert_eq!(b17.label.as_deref(), Some("B17"));
    assert_eq!(t.windows[0].kind, "application");
    assert_eq!(t.windows[0].package.as_deref(), Some(testbed::BUNDLE_ID));
    assert_eq!(
        RT.block_on(d.foreground()).unwrap(),
        Some(AppId(testbed::BUNDLE_ID.into()))
    );
}

#[test]
#[ignore]
fn a_tap_lands_on_the_element_center() {
    let d = device();
    let udid = udid();
    testbed::launch(&udid, "taps");
    let at = center_of(&tree(&d), "taps.b17");
    tap(&d, at);
    let log = testbed::wait_for(&udid, "tap", |e| ev(e, "tap").is_some());
    let taps: Vec<&Value> = log.iter().filter_map(|e| ev(e, "tap")).collect();
    assert_eq!(taps.len(), 1);
    assert_eq!(taps[0]["id"], "b17");
    let touches: Vec<&Value> = log.iter().filter_map(|e| ev(e, "touch")).collect();
    assert_eq!(touches[0]["phase"], "began");
    assert_eq!(touches.last().unwrap()["phase"], "ended");
    for t in touches {
        assert!((t["x"].as_f64().unwrap() - at.x as f64).abs() <= 0.5, "{t}");
        assert!((t["y"].as_f64().unwrap() - at.y as f64).abs() <= 0.5, "{t}");
    }
}

fn enrolled(udid: &str, on: bool) {
    const STATE: &str = "com.apple.BiometricKit.enrollmentChanged";
    let value = if on { "1" } else { "0" };
    for args in [vec!["-s", STATE, value], vec!["-p", STATE]] {
        let done = std::process::Command::new("xcrun")
            .args(["simctl", "spawn", udid, "notifyutil"])
            .args(args)
            .status()
            .unwrap();
        assert!(done.success());
    }
}

#[test]
#[ignore]
fn a_face_id_prompt_is_failed_by_the_wrong_face_and_passed_by_the_right_one() {
    let d = device();
    let udid = udid();
    enrolled(&udid, false);
    testbed::launch(&udid, "dialogs");
    tap_id(&d, "dialogs.biometric");
    let log = testbed::wait_for(&udid, "biometric", |e| ev(e, "biometric").is_some());
    let asked: Vec<&Value> = log.iter().filter_map(|e| ev(e, "biometric")).collect();
    assert_eq!(asked[0]["result"], "unavailable", "nothing is enrolled yet");

    RT.block_on(d.button(Button::Fingerprint)).unwrap();
    testbed::clear_log(&udid);
    tap_id(&d, "dialogs.biometric");
    std::thread::sleep(Duration::from_millis(1500));
    RT.block_on(d.button(Button::WrongFingerprint)).unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    let prompt = tree(&d);
    assert!(
        prompt.nodes.iter().any(|n| n
            .label
            .as_deref()
            .is_some_and(|l| l.contains("Not Recogni"))),
        "the system says the face was not recognised: {:?}",
        prompt
            .nodes
            .iter()
            .filter_map(|n| n.label.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        testbed::read_log(&udid)
            .iter()
            .all(|e| ev(e, "biometric").is_none()),
        "the app has no answer yet"
    );
    RT.block_on(d.button(Button::Fingerprint)).unwrap();
    let log = testbed::wait_for(&udid, "biometric", |e| ev(e, "biometric").is_some());
    let answer: Vec<&Value> = log.iter().filter_map(|e| ev(e, "biometric")).collect();
    assert_eq!(answer[0]["result"], "succeeded");
    enrolled(&udid, false);
}

#[test]
#[ignore]
fn a_recording_of_the_simulator_is_an_mp4_of_the_taps_made_while_it_ran() {
    let d = device();
    let udid = udid();
    testbed::launch(&udid, "taps");
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("sim_record.mp4");
    let _ = std::fs::remove_file(&path);
    assert_eq!(RT.block_on(d.record_stop()).unwrap(), None);
    RT.block_on(d.record_start(&path)).unwrap();
    let began = std::time::Instant::now();
    assert!(matches!(
        RT.block_on(d.record_start(&path)),
        Err(Error::Refused { .. })
    ));
    for id in ["taps.b17", "taps.b3", "taps.b20"] {
        tap_id(&d, id);
        std::thread::sleep(Duration::from_millis(600));
    }
    let wall_ms = began.elapsed().as_millis() as u64;
    let done = RT
        .block_on(d.record_stop())
        .unwrap()
        .expect("a recording was running");
    assert_eq!(done.path, path);
    assert_eq!(done.early, None);
    assert!(
        done.width >= 1000 && done.height > done.width,
        "an upright phone screen in pixels: {done:?}"
    );
    assert!(done.frames >= 3, "{done:?}");
    assert!(
        done.ms >= 1200 && done.ms <= wall_ms + 300,
        "{done:?} in {wall_ms} ms"
    );
    assert_eq!(
        phonedriver_video::record::probe(&path).unwrap().frames,
        done.frames
    );
    assert_eq!(RT.block_on(d.record_stop()).unwrap(), None);
    let taps = testbed::wait_for(&udid, "three taps", |e| ev(e, "tap").is_some());
    assert_eq!(taps.iter().filter_map(|e| ev(e, "tap")).count(), 3);
}

#[test]
#[ignore]
fn a_screenshot_is_a_png_at_one_pixel_per_point() {
    let d = device();
    testbed::launch(&udid(), "taps");
    let shot = RT.block_on(d.screenshot(Quality(100))).unwrap();
    assert_eq!(&shot.bytes[1..4], b"PNG");
    assert_eq!((shot.width, shot.height), (402, 874));
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ios_taps.png");
    std::fs::write(&out, &shot.bytes).unwrap();
}

#[test]
#[ignore]
fn paste_enters_unicode_exactly() {
    let d = device();
    let udid = udid();
    testbed::launch(&udid, "text");
    tap_id(&d, "text.plain");
    std::thread::sleep(Duration::from_millis(500));
    let text = "héllo 世界 مرحبا 👋🏽";
    RT.block_on(d.text(TextInput::Paste(text.into()))).unwrap();
    testbed::wait_for(&udid, "plain text", |e| {
        e["field"] == "plain" && e["value"] == text
    });
}

#[test]
#[ignore]
fn keystrokes_backspace_and_enter() {
    let d = device();
    let udid = udid();
    testbed::launch(&udid, "text");
    tap_id(&d, "text.plain");
    std::thread::sleep(Duration::from_millis(500));
    RT.block_on(d.text(TextInput::Keystrokes("abc".into())))
        .unwrap();
    testbed::wait_for(&udid, "abc", |e| e["value"] == "abc");
    RT.block_on(d.key(Key::Backspace)).unwrap();
    testbed::wait_for(&udid, "ab", |e| e["value"] == "ab");
    tap_id(&d, "text.multi");
    std::thread::sleep(Duration::from_millis(500));
    RT.block_on(d.text(TextInput::Keystrokes("x".into())))
        .unwrap();
    RT.block_on(d.key(Key::Enter)).unwrap();
    RT.block_on(d.text(TextInput::Keystrokes("y".into())))
        .unwrap();
    testbed::wait_for(&udid, "multiline", |e| {
        e["field"] == "multi" && e["value"] == "x\ny"
    });
}

#[test]
#[ignore]
fn a_secret_is_typed_and_stays_off_the_pasteboard_and_the_runner_log() {
    let d = device();
    let udid = udid();
    testbed::launch(&udid, "text");
    RT.block_on(d.clipboard(ClipboardOp::Set("before".into())))
        .unwrap();
    tap_id(&d, "text.password");
    std::thread::sleep(Duration::from_millis(500));
    let secret = "hunter2-canary";
    RT.block_on(d.text(TextInput::PasteSecret(secret.into())))
        .unwrap();
    testbed::wait_for(&udid, "password", |e| {
        e["field"] == "password" && e["length"] == secret.len()
    });
    assert_eq!(
        RT.block_on(d.clipboard(ClipboardOp::Get))
            .unwrap()
            .as_deref(),
        Some("before")
    );
    let log =
        std::fs::read_to_string(home().join(format!("runner/{udid}.log"))).unwrap_or_default();
    assert!(!log.contains(secret));
}

#[test]
#[ignore]
fn the_clipboard_round_trips() {
    let d = device();
    RT.block_on(d.clipboard(ClipboardOp::Set("clip ✂️".into())))
        .unwrap();
    assert_eq!(
        RT.block_on(d.clipboard(ClipboardOp::Get))
            .unwrap()
            .as_deref(),
        Some("clip ✂️")
    );
}

#[test]
#[ignore]
fn home_backgrounds_the_app_and_launch_brings_it_back() {
    let d = device();
    let udid = udid();
    testbed::launch(&udid, "taps");
    RT.block_on(d.button(Button::Home)).unwrap();
    testbed::wait_for(&udid, "background", |e| e["state"] == "background");
    testbed::clear_log(&udid);
    RT.block_on(d.launch(&AppId(testbed::BUNDLE_ID.into()), false))
        .unwrap();
    testbed::wait_for(&udid, "active", |e| e["state"] == "active");
    assert_eq!(
        RT.block_on(d.foreground()).unwrap(),
        Some(AppId(testbed::BUNDLE_ID.into()))
    );
}

#[test]
#[ignore]
fn apps_lists_the_testbed_and_settings() {
    let d = device();
    let apps = RT.block_on(d.apps()).unwrap();
    let testbed_app = apps.iter().find(|a| a.id.0 == testbed::BUNDLE_ID).unwrap();
    assert!(!testbed_app.system);
    let settings = apps
        .iter()
        .find(|a| a.id.0 == "com.apple.Preferences")
        .unwrap();
    assert_eq!(settings.name, "Settings");
    assert!(settings.system);
}

#[test]
#[ignore]
fn open_url_reaches_the_testbed_without_a_confirmation() {
    let d = device();
    let udid = udid();
    testbed::launch(&udid, "taps");
    RT.block_on(d.open_url(&Url("putb://tab/nav".into()), None))
        .unwrap();
    testbed::wait_for(&udid, "tab nav", |e| e["ev"] == "tab" && e["name"] == "nav");
}

#[test]
#[ignore]
fn back_taps_the_nav_bar_back_button() {
    let d = device();
    let udid = udid();
    testbed::launch(&udid, "nav");
    for level in 1..=2 {
        tap_id(&d, &format!("nav.open{level}"));
        testbed::wait_for(&udid, "nav", |e| e["ev"] == "nav" && e["level"] == level);
        std::thread::sleep(Duration::from_millis(700));
    }
    testbed::clear_log(&udid);
    assert_eq!(RT.block_on(d.back()).unwrap(), Some("nav-button"));
    testbed::wait_for(&udid, "nav 1", |e| e["ev"] == "nav" && e["level"] == 1);
}

#[test]
#[ignore]
fn back_without_a_back_button_swipes_from_the_left_edge() {
    let d = device();
    let udid = udid();
    testbed::launch(&udid, "web");
    std::thread::sleep(Duration::from_millis(1000));
    let link = tree(&d)
        .nodes
        .iter()
        .find(|n| n.label.as_deref() == Some("Details"))
        .expect("no Details link")
        .bounds
        .center();
    tap(&d, link);
    testbed::wait_for(&udid, "page details", |e| e["page"] == "#details");
    std::thread::sleep(Duration::from_millis(700));
    testbed::clear_log(&udid);
    assert_eq!(RT.block_on(d.back()).unwrap(), Some("edge-swipe"));
    testbed::wait_for(&udid, "page form", |e| e["page"] == "#form");
}

#[test]
#[ignore]
fn a_tap_through_the_hid_port_returns_in_under_150_ms_and_lands() {
    let d = device();
    let udid = udid();
    testbed::launch(&udid, "taps");
    let at = center_of(&tree(&d), "taps.b9");
    let started = std::time::Instant::now();
    tap(&d, at);
    let took = started.elapsed();
    let log = testbed::wait_for(&udid, "tap", |e| ev(e, "tap").is_some());
    assert_eq!(
        log.iter().filter_map(|e| ev(e, "tap")).next().unwrap()["id"],
        "b9"
    );
    assert!(
        took < Duration::from_millis(150),
        "the tap took {took:?}; through XCTest it takes about 330 ms"
    );
}

#[test]
#[ignore]
fn polled_frames_report_a_screen_change_after_a_tap() {
    let d = device();
    testbed::launch(&udid(), "taps");
    let mut frames = d.frames().unwrap();
    std::thread::sleep(Duration::from_millis(800));
    frames.borrow_and_update();
    tap_id(&d, "tab.list");
    let changed = RT.block_on(async {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                frames.changed().await.unwrap();
                if frames.borrow_and_update().changed_area_pt2 > 4096.0 {
                    return;
                }
            }
        })
        .await
    });
    assert!(changed.is_ok(), "no large change within 2 s");
}

#[test]
#[ignore]
fn volume_and_recents_are_unsupported_on_the_simulator() {
    let d = device();
    for b in [Button::VolumeUp, Button::Recents] {
        assert!(
            matches!(RT.block_on(d.button(b)), Err(Error::Unsupported { .. })),
            "{b:?}"
        );
    }
}

#[test]
#[ignore]
fn the_tree_right_after_a_launch_is_the_launched_app() {
    let d = device();
    let udid = udid();
    for _ in 0..3 {
        testbed::launch(&udid, "taps");
        let _ = std::process::Command::new("xcrun")
            .args(["simctl", "terminate", &udid, "com.apple.Preferences"])
            .output();
        std::thread::sleep(Duration::from_millis(500));
        RT.block_on(d.launch(&AppId("com.apple.Preferences".into()), false))
            .unwrap();
        let t = tree(&d);
        assert_eq!(
            t.windows[0].package.as_deref(),
            Some("com.apple.Preferences")
        );
    }
}

#[test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
fn landscape_turns_an_app_that_can_turn_and_portrait_turns_it_back() {
    let d = device();
    let udid = udid();
    let screen = |d: &IosDevice| tree(d).windows[0].bounds;
    std::process::Command::new("xcrun")
        .args(["simctl", "launch", &udid, "com.apple.mobilesafari"])
        .status()
        .unwrap();
    std::thread::sleep(Duration::from_secs(2));
    let upright = screen(&d);
    assert!(upright.h > upright.w, "{upright:?}");
    RT.block_on(d.button(Button::Landscape)).unwrap();
    std::thread::sleep(Duration::from_secs(2));
    let turned = screen(&d);
    assert_eq!((turned.w, turned.h), (upright.h, upright.w));
    RT.block_on(d.button(Button::Portrait)).unwrap();
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(screen(&d), upright);
}

#[test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
fn a_tap_lands_in_an_upright_app_on_a_device_turned_on_its_side() {
    let d = device();
    let udid = udid();
    testbed::launch(&udid, "taps");
    RT.block_on(d.button(Button::Landscape)).unwrap();
    std::thread::sleep(Duration::from_secs(2));
    let t = tree(&d);
    let at = center_of(&t, "taps.b17");
    tap(&d, at);
    let log = testbed::wait_for(&udid, "tap", |e| ev(e, "tap").is_some());
    RT.block_on(d.button(Button::Portrait)).unwrap();
    let screen = t.windows[0].bounds;
    assert!(screen.h > screen.w, "the testbed turned: {screen:?}");
    let taps: Vec<&Value> = log.iter().filter_map(|e| ev(e, "tap")).collect();
    assert_eq!(taps.len(), 1);
    assert_eq!(taps[0]["id"], "b17");
}

#[test]
#[ignore]
fn paste_keeps_quotes_dashes_and_capitals_in_a_field_with_smart_punctuation() {
    let d = device();
    let udid = udid();
    testbed::launch(&udid, "text");
    tap_id(&d, "text.plain");
    std::thread::sleep(Duration::from_millis(500));
    let text = "x \"q\" 'r' -- teh ok. Gary@Mail.com";
    RT.block_on(d.text(TextInput::Paste(text.into()))).unwrap();
    testbed::wait_for(&udid, "plain text", |e| {
        e["field"] == "plain" && e["value"] == text
    });
}
