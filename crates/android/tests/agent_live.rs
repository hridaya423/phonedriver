mod support;

use std::time::Duration;

use serde_json::{Value, json};
use support::agent::{self, RawControl};
use support::testbed;
use tokio::io::AsyncReadExt;

const UNICODE: &str = "héllo 世界 مرحبا 👋🏽";

async fn start() -> (
    agent::RawAgent,
    RawControl,
    tokio::sync::MutexGuard<'static, ()>,
) {
    let guard = agent::exclusive().await;
    let (adb, serial) = (support::adb(), support::serial());
    testbed::ensure_installed(&adb, &serial).await;
    let raw = agent::launch(&adb, &serial).await;
    let control = raw.control().await;
    (raw, control, guard)
}

fn px(v: &Value) -> Vec<i64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect()
}

async fn find_node(c: &mut RawControl, id_suffix: &str) -> Value {
    for _ in 0..40 {
        let tree = c.ok(json!({"op":"tree"})).await;
        if let Some(n) = tree["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"].as_str().is_some_and(|i| i.ends_with(id_suffix)))
        {
            return n.clone();
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("no node with id ending {id_suffix}");
}

fn center(node: &Value) -> (i64, i64) {
    let b = px(&node["b"]);
    ((b[0] + b[2]) / 2, (b[1] + b[3]) / 2)
}

async fn tap(c: &mut RawControl, x: i64, y: i64) {
    c.ok(json!({"op":"gesture","pointers":[{"id":0,"path":[
        {"x":x,"y":y,"t_ms":0},{"x":x,"y":y,"t_ms":50}]}]}))
        .await;
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn main_and_control_hello_token_and_unknown_op() {
    let (raw, mut control, _g) = start().await;
    assert_eq!(control.hello["ok"], true, "{}", control.hello);
    assert!(control.hello["api"].as_i64().unwrap() >= 26);
    assert!(control.hello["display"]["w"].as_i64().unwrap() > 0);
    assert!(control.hello["display"]["h"].as_i64().unwrap() > 0);
    assert!(control.hello["density"].as_f64().unwrap() > 0.5);

    let mut wrong = raw.adb.open_abstract(&raw.serial, &raw.name).await.unwrap();
    tokio::io::AsyncWriteExt::write_all(&mut wrong, b"C")
        .await
        .unwrap();
    agent::write_frame(
        &mut wrong,
        &json!({"id":0,"op":"hello","version":agent::VERSION,"token":"nope"}),
    )
    .await;
    let mut buf = [0u8; 1];
    let n = tokio::time::timeout(Duration::from_secs(2), wrong.read(&mut buf))
        .await
        .expect("agent must close the stream")
        .unwrap_or(0);
    assert_eq!(n, 0, "no reply to a wrong token");

    let r = control.request(json!({"op":"nope"})).await;
    assert_eq!(r["ok"], false);
    assert!(r["error"].as_str().unwrap().contains("unknown op"), "{r}");
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn accessibility_tree_of_the_taps_tab_has_24_buttons() {
    let (raw, mut c, _g) = start().await;
    testbed::open_tab(&raw.adb, &raw.serial, "taps").await;
    find_node(&mut c, ":id/b23").await;
    let tree = c.ok(json!({"op":"tree"})).await;
    let windows = tree["windows"].as_array().unwrap();
    assert!(windows.iter().any(|w| w["type"] == "application"));
    let (dw, dh) = (
        c.hello["display"]["w"].as_i64().unwrap(),
        c.hello["display"]["h"].as_i64().unwrap(),
    );
    for w in windows {
        let b = px(&w["bounds"]);
        let thin_edge_bar = w["type"] == "system"
            && b[2] - b[0] == dw
            && (b[3] - b[1]) * 10 <= dh
            && (b[1] == 0 || b[3] == dh);
        assert!(!thin_edge_bar, "status or navigation bar window kept: {w}");
    }
    let nodes = tree["nodes"].as_array().unwrap();
    for i in 0..24 {
        let suffix = format!(":id/b{i}");
        let n = nodes
            .iter()
            .find(|n| n["id"].as_str().is_some_and(|id| id.ends_with(&suffix)))
            .unwrap_or_else(|| panic!("b{i} missing"));
        assert_eq!(n["id"], format!("dev.phoneuse.testbed:id/b{i}"));
        assert_eq!(n["cls"], "android.widget.Button");
        assert_eq!(n["text"], format!("b{i}"));
        for key in ["p", "w", "b", "f", "a", "n"] {
            assert!(n.get(key).is_some(), "b{i} lacks {key}: {n}");
        }
        assert_eq!(px(&n["b"]).len(), 4);
        let f = n["f"].as_str().unwrap();
        assert!(f.contains('V') && f.contains('e'), "flags {f}");
        assert!(n["a"].as_array().unwrap().contains(&json!("tap")));
        assert!(n["p"].as_i64().unwrap() >= 0);
    }
    let buttons = nodes
        .iter()
        .filter(|n| {
            n["id"].as_str().is_some_and(|id| {
                id.rsplit('/')
                    .next()
                    .is_some_and(|s| s.starts_with('b') && s[1..].parse::<u32>().is_ok())
            })
        })
        .count();
    assert_eq!(buttons, 24);
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn tap_gesture_lands_within_one_pixel_and_two_pointers_both_log() {
    let (raw, mut c, _g) = start().await;
    let density = c.hello["density"].as_f64().unwrap();
    testbed::open_tab(&raw.adb, &raw.serial, "taps").await;
    let b17 = find_node(&mut c, ":id/b17").await;
    let (x, y) = center(&b17);
    testbed::clear_log(&raw.adb, &raw.serial).await;
    tap(&mut c, x, y).await;
    let log = testbed::wait_for_log(&raw.adb, &raw.serial, 2000, |l| {
        l.iter().any(|e| e["ev"] == "tap")
    })
    .await;
    let taps: Vec<_> = log.iter().filter(|e| e["ev"] == "tap").collect();
    assert_eq!(taps.len(), 1, "{log:?}");
    assert_eq!(taps[0]["id"], "b17");
    assert!((taps[0]["x_dp"].as_f64().unwrap() * density - x as f64).abs() <= 1.0);
    assert!((taps[0]["y_dp"].as_f64().unwrap() * density - y as f64).abs() <= 1.0);

    testbed::open_tab(&raw.adb, &raw.serial, "gestures").await;
    find_node(&mut c, ":id/gesture_surface").await;
    testbed::clear_log(&raw.adb, &raw.serial).await;
    let (w, h) = (
        c.hello["display"]["w"].as_i64().unwrap(),
        c.hello["display"]["h"].as_i64().unwrap(),
    );
    let (cx, cy) = (w / 2, h / 2);
    c.ok(json!({"op":"gesture","pointers":[
        {"id":0,"path":[{"x":cx-100,"y":cy,"t_ms":0},{"x":cx-200,"y":cy,"t_ms":100},{"x":cx-200,"y":cy,"t_ms":150}]},
        {"id":1,"path":[{"x":cx+100,"y":cy,"t_ms":0},{"x":cx+200,"y":cy,"t_ms":100},{"x":cx+200,"y":cy,"t_ms":150}]}]}))
        .await;
    let log = testbed::wait_for_log(&raw.adb, &raw.serial, 2000, |l| {
        l.iter()
            .filter(|e| e["ev"] == "touch" && e["action"] == "UP")
            .count()
            >= 1
    })
    .await;
    let two = log
        .iter()
        .filter(|e| e["ev"] == "touch")
        .filter_map(|e| e["ptrs"].as_array())
        .find(|p| p.len() == 2)
        .unwrap_or_else(|| panic!("no two-pointer event in {log:?}"));
    let ids: Vec<i64> = two.iter().map(|p| p["id"].as_i64().unwrap()).collect();
    assert!(ids.contains(&0) && ids.contains(&1), "{ids:?}");
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn video_header_matches_the_display_and_a_frame_follows() {
    let (raw, c, _g) = start().await;
    let (w, h) = (
        c.hello["display"]["w"].as_u64().unwrap() as u32,
        c.hello["display"]["h"].as_u64().unwrap() as u32,
    );
    let mut v = raw.video().await;
    let mut header = [0u8; 12];
    tokio::time::timeout(Duration::from_secs(3), v.read_exact(&mut header))
        .await
        .expect("codec header")
        .unwrap();
    assert_eq!(&header[..4], b"h264");
    assert_eq!(u32::from_be_bytes(header[4..8].try_into().unwrap()), w);
    assert_eq!(u32::from_be_bytes(header[8..12].try_into().unwrap()), h);
    let mut seen_frame = false;
    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(1) && !seen_frame {
        let mut fh = [0u8; 13];
        tokio::time::timeout(Duration::from_secs(1), v.read_exact(&mut fh))
            .await
            .expect("frame header")
            .unwrap();
        let len = u32::from_be_bytes(fh[9..13].try_into().unwrap()) as usize;
        let mut body = vec![0u8; len];
        v.read_exact(&mut body).await.unwrap();
        seen_frame = fh[8] & 1 == 0 && len > 0;
    }
    assert!(
        seen_frame,
        "no picture frame within 1 s of the stream start"
    );
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn paste_types_unicode_exactly_and_restores_the_clipboard() {
    let (raw, mut c, _g) = start().await;
    testbed::open_tab(&raw.adb, &raw.serial, "text").await;
    let plain = find_node(&mut c, ":id/plain").await;
    let (x, y) = center(&plain);
    tap(&mut c, x, y).await;
    c.ok(json!({"op":"clipboard_set","text":"OLD-CLIP"})).await;
    testbed::clear_log(&raw.adb, &raw.serial).await;
    c.ok(json!({"op":"text","mode":"paste","text":UNICODE}))
        .await;
    let log = testbed::wait_for_log(&raw.adb, &raw.serial, 2000, |l| {
        l.iter().any(|e| e["ev"] == "text" && e["value"] == UNICODE)
    })
    .await;
    assert!(
        log.iter()
            .any(|e| e["ev"] == "text" && e["field"] == "plain" && e["value"] == UNICODE),
        "{log:?}"
    );
    let clip = c.ok(json!({"op":"clipboard_get"})).await;
    assert_eq!(clip["text"], "OLD-CLIP");
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn set_and_keys_modes_type_and_keys_reports_unmappable_characters() {
    let (raw, mut c, _g) = start().await;
    testbed::open_tab(&raw.adb, &raw.serial, "text").await;
    let plain = find_node(&mut c, ":id/plain").await;
    let (x, y) = center(&plain);
    tap(&mut c, x, y).await;
    for _ in 0..30 {
        if c.ok(json!({"op":"state"})).await["ime_visible"] == true {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    testbed::clear_log(&raw.adb, &raw.serial).await;
    c.ok(json!({"op":"text","mode":"keys","text":"hello"}))
        .await;
    let log = testbed::wait_for_log(&raw.adb, &raw.serial, 2000, |l| {
        l.iter().any(|e| e["ev"] == "text" && e["value"] == "hello")
    })
    .await;
    assert!(log.iter().any(|e| e["value"] == "hello"), "{log:?}");

    let bad = c
        .request(json!({"op":"text","mode":"keys","text":"hé"}))
        .await;
    assert_eq!(bad["ok"], false);
    assert!(
        bad["error"]
            .as_str()
            .unwrap()
            .contains("unmappable character U+E9"),
        "{bad}"
    );

    testbed::clear_log(&raw.adb, &raw.serial).await;
    c.ok(json!({"op":"text","mode":"set","text":UNICODE})).await;
    let log = testbed::wait_for_log(&raw.adb, &raw.serial, 2000, |l| {
        l.iter().any(|e| e["ev"] == "text" && e["value"] == UNICODE)
    })
    .await;
    assert!(log.iter().any(|e| e["value"] == UNICODE), "{log:?}");
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn apps_lists_the_testbed_and_launch_foregrounds_it() {
    let (_raw, mut c, _g) = start().await;
    let apps = c.ok(json!({"op":"apps"})).await;
    let testbed_app = apps["apps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["package"] == testbed::PACKAGE)
        .unwrap_or_else(|| panic!("testbed not listed: {apps}"));
    assert_eq!(testbed_app["label"], "phone-use testbed");
    assert_eq!(testbed_app["system"], false);

    c.ok(json!({"op":"key","keycode":3})).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    c.ok(json!({"op":"launch","package":testbed::PACKAGE}))
        .await;
    let mut foreground = Value::Null;
    for _ in 0..40 {
        foreground = c.ok(json!({"op":"state"})).await["foreground"].clone();
        if foreground == testbed::PACKAGE {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(foreground, testbed::PACKAGE);
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn state_reports_the_screen_off_after_sleep_and_on_after_wake() {
    let (_raw, mut c, _g) = start().await;
    assert_eq!(c.ok(json!({"op":"state"})).await["screen_on"], true);
    c.ok(json!({"op":"key","keycode":223})).await;
    let mut off = c.ok(json!({"op":"state"})).await["screen_on"].clone();
    for _ in 0..20 {
        if off == false {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        off = c.ok(json!({"op":"state"})).await["screen_on"].clone();
    }
    c.ok(json!({"op":"wake"})).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    let on = c.ok(json!({"op":"state"})).await;
    assert_eq!(off, false);
    assert_eq!(on["screen_on"], true);
    assert!(
        on["ime_visible"].is_boolean()
            && on["locked"].is_boolean()
            && on["secure_keyguard"].is_boolean()
    );
}

async fn focused_window(raw: &agent::RawAgent) -> String {
    let out = raw
        .adb
        .shell(&raw.serial, "dumpsys window | grep mCurrentFocus")
        .await
        .unwrap();
    out.stdout_text().trim().to_string()
}

fn is_shade(focus: &str) -> bool {
    focus.contains("NotificationShade") || focus.contains("StatusBar")
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE with su"]
async fn the_shade_kick_leaves_the_shade_closed_when_system_ui_stalls() {
    let (raw, mut c, _g) = start().await;
    let sh = |cmd: String| {
        let adb = raw.adb.clone();
        let serial = raw.serial.clone();
        async move { adb.shell(&serial, &cmd).await.unwrap().stdout_text() }
    };
    testbed::open_tab(&raw.adb, &raw.serial, "hollow").await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    sh("logcat -b events -c".into()).await;
    c.ok(json!({"op":"wake"})).await;
    let pid = sh("pidof com.android.systemui".into())
        .await
        .trim()
        .to_string();
    sh(format!("su 0 kill -STOP {pid}")).await;
    let resume = async {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        sh(format!("su 0 kill -CONT {pid}")).await;
    };
    let (tree, ()) = tokio::join!(c.ok(json!({"op":"tree"})), resume);
    let mut seen = Vec::new();
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        seen.push(focused_window(&raw).await);
    }
    let revealed = sh("logcat -b events -d -s notification_panel_revealed".into()).await;
    sh("cmd statusbar collapse".into()).await;
    assert!(
        revealed.contains("notification_panel_revealed"),
        "the hollow tab did not trigger the kick; tree: {}",
        tree
    );
    assert!(
        !is_shade(seen.last().unwrap()),
        "the shade was left open: {seen:#?}"
    );
}
