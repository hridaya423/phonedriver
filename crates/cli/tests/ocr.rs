#![cfg(target_os = "macos")]

use std::time::Instant;

use phone_use::ocr::recognize;
use phone_use_core::geometry::Size;

const ANDROID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../core/tests/fixtures/android/"
);
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ocr/");
const IOS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testbeds/ios/screenshots/"
);

fn read(path: &str) -> Vec<u8> {
    std::fs::read(path).unwrap()
}

fn texts(png: &[u8], w: f32, h: f32) -> Vec<String> {
    recognize(png, Size { w, h })
        .unwrap()
        .into_iter()
        .map(|b| b.text)
        .collect()
}

fn assert_finds(found: &[String], expected: &[&str]) {
    let missing: Vec<&&str> = expected
        .iter()
        .filter(|e| {
            !found
                .iter()
                .any(|f| f.to_lowercase().contains(&e.to_lowercase()))
        })
        .collect();
    assert!(missing.is_empty(), "missing {missing:?} in {found:?}");
}

#[test]
fn the_android_testbed_taps_tab_at_one_pixel_per_point() {
    let found = texts(&read(&format!("{ANDROID}testbed_taps.png")), 411.0, 923.0);
    let mut expected = vec!["Taps", "Text", "List", "Gestures", "Busy", "Dark theme"];
    let buttons: Vec<String> = (1..12).map(|i| format!("b{i}")).collect();
    expected.extend(buttons.iter().map(String::as_str));
    assert_finds(&found, &expected);
}

#[test]
fn the_same_tab_at_twice_the_resolution_reads_the_lower_rows_too() {
    let found = texts(
        &read(&format!("{FIXTURES}testbed_taps_2x.png")),
        411.0,
        923.0,
    );
    let mut expected = vec!["Taps", "Text", "List", "Gestures", "Dark theme"];
    let buttons: Vec<String> = [
        0, 1, 2, 3, 5, 6, 7, 9, 10, 11, 13, 14, 15, 17, 18, 19, 21, 22, 23,
    ]
    .iter()
    .map(|i| format!("b{i}"))
    .collect();
    expected.extend(buttons.iter().map(String::as_str));
    assert_finds(&found, &expected);
}

#[test]
fn android_settings() {
    let found = texts(
        &read(&format!("{ANDROID}real_settings_main.png")),
        411.4,
        923.4,
    );
    assert_finds(
        &found,
        &[
            "Search Settings",
            "Google",
            "Services & preferences",
            "Network & internet",
            "Connected devices",
            "Bluetooth, pairing",
            "Apps",
            "Notifications",
            "Notification history, conversations",
            "Sound & vibration",
            "Volume and haptics",
            "Modes",
            "Do Not Disturb, Bedtime, Driving",
            "Display & touch",
            "Wallpaper & style",
            "Colors, themed icons, app grid",
            "Storage",
        ],
    );
}

#[test]
fn the_ios_testbed_gestures_canvas_and_tab_bar() {
    let found = texts(&read(&format!("{IOS}gestures.png")), 402.0, 874.0);
    assert_finds(
        &found,
        &["16:36", "Gesture pad", "Gestures", "Secure", "Dialogs"],
    );
}

#[test]
fn boxes_are_in_points_with_the_origin_at_the_top_left() {
    let boxes = recognize(
        &read(&format!("{ANDROID}testbed_taps.png")),
        Size { w: 411.0, h: 923.0 },
    )
    .unwrap();
    let dark = boxes
        .iter()
        .find(|b| b.text.contains("Dark theme"))
        .expect("Dark theme");
    assert!(
        (dark.bounds.x - 0.0).abs() < 8.0 && dark.bounds.w > 60.0 && dark.bounds.w < 110.0,
        "{dark:?}"
    );
    assert!(
        (dark.bounds.y + dark.bounds.h / 2.0 - 116.0).abs() < 8.0,
        "{dark:?}"
    );
}

#[test]
fn a_frame_of_412_by_915_takes_under_120_ms_at_the_median() {
    let png = read(&format!("{ANDROID}testbed_taps.png"));
    let size = Size { w: 411.0, h: 923.0 };
    for _ in 0..3 {
        recognize(&png, size).unwrap();
    }
    let mut ms: Vec<f64> = (0..30)
        .map(|_| {
            let t = Instant::now();
            recognize(&png, size).unwrap();
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    ms.sort_by(f64::total_cmp);
    eprintln!(
        "vision p50 {:.1} ms, min {:.1}, max {:.1}, 30 runs",
        ms[15], ms[0], ms[29]
    );
    assert!(ms[15] < 120.0, "p50 {:.1} ms", ms[15]);
}

#[test]
fn a_screenshot_vision_cannot_read_is_unsupported_and_points_to_screenshot() {
    let e = recognize(b"not a png", Size { w: 100.0, h: 100.0 }).unwrap_err();
    assert!(
        matches!(
            e,
            phone_use_core::error::Error::Unsupported {
                op: phone_use_core::error::Op::Ocr,
                ..
            }
        ),
        "{e:?}"
    );
    assert!(e.hint().contains("screenshot"), "{}", e.hint());
}
