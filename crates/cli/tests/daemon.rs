use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use phonedriver::daemon::rpc::RpcError;
use phonedriver::daemon::{Config, Daemon, DeviceSource, Listed};
use phonedriver_core::device::{
    AppId, Capabilities, Device, DeviceEvent, FrameRef, TextInput, Url,
};
use phonedriver_core::error::Error;
use phonedriver_core::geometry::{Point, Rect, Scale};
use phonedriver_core::raw::RawTree;
use phonedriver_core::testing::{Call, FakeDevice};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn tree(labels: &[&str]) -> RawTree {
    let mut nodes = vec![
        r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}"#
            .to_string(),
    ];
    for (i, l) in labels.iter().enumerate() {
        let top = 100 + 100 * i as i32;
        nodes.push(format!(
            r#"{{"p":0,"w":1,"cls":"android.widget.Button","text":"{l}","b":[0,{top},360,{}],"f":"vVe","a":["tap"]}}"#,
            top + 60
        ));
    }
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[{}]}}"#,
        nodes.join(",")
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

fn field_tree(value: &str, password: bool) -> RawTree {
    let flags = if password { "vVeEFP" } else { "vVeEF" };
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.widget.EditText","text":"{value}","desc":"Field","b":[0,100,360,160],"f":"{flags}","a":["tap","set_text"]}}]}}"#
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

struct FakeSource {
    devices: Mutex<Vec<(Listed, Arc<FakeDevice>)>>,
}

impl FakeSource {
    fn with(trees: Vec<RawTree>) -> (Arc<FakeSource>, Arc<FakeDevice>) {
        Self::of(FakeDevice::new(trees))
    }

    fn polled(trees: Vec<RawTree>) -> (Arc<FakeSource>, Arc<FakeDevice>) {
        Self::of(FakeDevice::new(trees).lacking(Capabilities::FRAMES))
    }

    fn of(fake: FakeDevice) -> (Arc<FakeSource>, Arc<FakeDevice>) {
        let fake = Arc::new(fake);
        let source = Arc::new(FakeSource {
            devices: Mutex::new(vec![(
                Listed {
                    id: "fake:1".into(),
                    model: "Fake".into(),
                    os: Some("FakeOS 1".into()),
                    tier: "fake".into(),
                    capabilities: fake.info().capabilities,
                },
                fake.clone(),
            )]),
        });
        (source, fake)
    }
}

#[async_trait]
impl DeviceSource for FakeSource {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(self
            .devices
            .lock()
            .unwrap()
            .iter()
            .map(|(l, _)| l.clone())
            .collect())
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        self.devices
            .lock()
            .unwrap()
            .iter()
            .find(|(l, _)| l.id == id)
            .map(|(_, d)| d.clone() as Arc<dyn Device>)
            .ok_or(Error::NoDevice {
                requested: None,
                connected: vec![],
            })
    }
}

fn daemon(source: Arc<FakeSource>) -> (Arc<Daemon>, tempfile::TempDir) {
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::new(
        source,
        Config {
            home: home.path().to_path_buf(),
        },
    );
    (d, home)
}

async fn ok(d: &Daemon, session: &str, method: &str, params: Value) -> Value {
    d.call(session, method, params)
        .await
        .unwrap_or_else(|e| panic!("{method} failed: {e:?}"))
}

async fn err(d: &Daemon, session: &str, method: &str, params: Value) -> RpcError {
    d.call(session, method, params)
        .await
        .expect_err("expected an error")
}

fn tap_ref(r: &str) -> Value {
    json!({"action": {"kind": "tap", "target": {"ref": r}}})
}

fn tap_point(x: f32, y: f32) -> Value {
    json!({"action": {"kind": "tap", "target": {"point": {"x": x, "y": y}}}})
}

fn big_frame(seq: u64) -> FrameRef {
    FrameRef {
        seq,
        changed_area_pt2: 10_000.0,
        changed_box: Some(Rect {
            x: 0.0,
            y: 100.0,
            w: 360.0,
            h: 300.0,
        }),
    }
}

#[tokio::test]
async fn hello_accepts_the_same_version_and_rejects_another_with_32010() {
    let (source, _) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    let r = ok(
        &d,
        "default",
        "hello",
        json!({"version": env!("CARGO_PKG_VERSION"), "client": "test"}),
    )
    .await;
    assert_eq!(r["version"], env!("CARGO_PKG_VERSION"));
    assert!(r["pid"].as_u64().unwrap() > 0);
    let e = err(
        &d,
        "default",
        "hello",
        json!({"version": "0.0.1", "client": "test"}),
    )
    .await;
    assert_eq!(e.code, -32010);
    assert_eq!(e.data["daemon_version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn unknown_methods_return_32601() {
    let (source, _) = FakeSource::with(vec![]);
    let (d, _home) = daemon(source);
    assert_eq!(err(&d, "default", "nope", json!({})).await.code, -32601);
}

#[tokio::test]
async fn devices_lists_fake_devices_with_tier_and_capabilities() {
    let (source, fake) = FakeSource::with(vec![]);
    let (d, _home) = daemon(source);
    let r = ok(&d, "default", "devices", json!({})).await;
    let list = r.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["id"], "fake:1");
    assert_eq!(list[0]["tier"], "fake");
    assert_eq!(list[0]["leased_by"], Value::Null);
    let caps: Capabilities = serde_json::from_value(list[0]["capabilities"].clone()).unwrap();
    assert_eq!(caps, fake.info().capabilities);
}

#[tokio::test]
async fn the_header_follows_a_device_that_reports_its_own_rotation() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    let before = ok(&d, "default", "snapshot", json!({})).await;
    assert!(
        before["text"]
            .as_str()
            .unwrap()
            .contains("360x800pt · portrait")
    );
    fake.rotate_to(phonedriver_core::geometry::Size { w: 800.0, h: 360.0 });
    let after = ok(&d, "default", "snapshot", json!({})).await;
    assert!(
        after["text"]
            .as_str()
            .unwrap()
            .contains("800x360pt · landscape"),
        "{after}"
    );
}

#[tokio::test]
async fn snapshot_returns_the_rendered_text_of_the_fake_tree() {
    let (source, _) = FakeSource::with(vec![tree(&["One", "Two", "Three"])]);
    let (d, _home) = daemon(source);
    let r = ok(&d, "default", "snapshot", json!({})).await;
    let text = r["text"].as_str().unwrap();
    assert!(
        text.starts_with("device fake:1 · Fake · FakeOS 1 · 360x800pt · portrait"),
        "{text}"
    );
    assert!(text.contains("app com.example"), "{text}");
    for (i, l) in ["One", "Two", "Three"].iter().enumerate() {
        assert!(
            text.contains(&format!("@e{} button \"{l}\"", i + 1)),
            "{text}"
        );
    }
    assert_eq!(r["kind"], "full");
}

#[tokio::test]
async fn act_tap_resolves_the_ref_on_a_fresh_tree_and_taps_its_center_in_points() {
    let (source, fake) = FakeSource::with(vec![tree(&["One", "Two", "Three"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", tap_ref("e3")).await;
    assert!(
        r["summary"]
            .as_str()
            .unwrap()
            .starts_with("tapped @e3 \"Three\" at 180,330"),
        "{r}"
    );
    let Call::Gesture(g) = fake.calls().pop().unwrap() else {
        panic!("no gesture")
    };
    assert_eq!(g.fingers.len(), 1);
    assert_eq!(g.fingers[0][0].at, Point { x: 180.0, y: 330.0 });
}

#[tokio::test]
async fn act_on_a_ref_the_screen_no_longer_has_is_stale_with_the_closest_element() {
    let (source, fake) = FakeSource::with(vec![
        tree(&["One", "Two", "Three"]),
        tree(&["One", "Two", "Four", "Three"]),
        tree(&["One", "Two"]),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let moved = ok(&d, "default", "act", tap_ref("e3")).await;
    assert!(
        moved["summary"]
            .as_str()
            .unwrap()
            .contains("\"Three\" at 180,430"),
        "{moved}"
    );
    let e = err(&d, "default", "act", tap_ref("e3")).await;
    assert_eq!(e.code, -32006);
    assert!(e.data["hint"].as_str().unwrap().contains("snapshot"));
    assert_eq!(fake.calls().len(), 1);
}

#[tokio::test]
async fn sessions_outlive_connections() {
    let (source, fake) = FakeSource::with(vec![tree(&["One", "Two", "Three"])]);
    let (d, home) = daemon(source);
    let path = local_socket(home.path(), "d.sock");
    let serving = tokio::spawn(d.clone().serve(path.clone()));
    wait_for_socket(&path).await;
    let a = rpc(&path, "default", "snapshot", json!({})).await;
    assert!(a["result"]["text"].as_str().unwrap().contains("@e3"), "{a}");
    let b = rpc(&path, "default", "act", tap_ref("e3")).await;
    assert!(b["result"]["ok"].as_bool().unwrap(), "{b}");
    assert_eq!(fake.calls().len(), 1);
    d.shutdown();
    serving.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_second_session_gets_device_busy_until_release_or_share() {
    let (source, _) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    ok(&d, "x", "snapshot", json!({})).await;
    let busy = err(&d, "y", "snapshot", json!({})).await;
    assert_eq!(busy.code, -32004);
    assert_eq!(busy.data["held_by"], "x");
    let listed = ok(&d, "y", "devices", json!({})).await;
    assert_eq!(listed[0]["leased_by"], "x");
    ok(&d, "y", "snapshot", json!({"share": true})).await;
    ok(&d, "x", "release", json!({})).await;
    ok(&d, "y", "snapshot", json!({})).await;
}

#[tokio::test(start_paused = true)]
async fn device_busy_says_how_long_the_holder_has_been_idle_and_when_its_lease_ends() {
    let (source, _) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    ok(&d, "x", "snapshot", json!({})).await;
    tokio::time::sleep(Duration::from_secs(3 * 60)).await;
    let busy = err(&d, "y", "snapshot", json!({})).await;
    let hint = busy.data["hint"].as_str().unwrap();
    assert!(
        hint.contains("idle 3m") && hint.contains("expires in 7m"),
        "{hint}"
    );
    let listed = ok(&d, "y", "devices", json!({})).await;
    assert_eq!(listed[0]["lease_idle_s"], 180);
}

#[tokio::test(start_paused = true)]
async fn a_lease_expires_after_ten_minutes_without_calls() {
    let (source, _) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    ok(&d, "x", "snapshot", json!({})).await;
    tokio::time::sleep(Duration::from_secs(9 * 60)).await;
    assert_eq!(err(&d, "y", "snapshot", json!({})).await.code, -32004);
    ok(&d, "x", "snapshot", json!({})).await;
    tokio::time::sleep(Duration::from_secs(10 * 60 + 1)).await;
    ok(&d, "y", "snapshot", json!({})).await;
}

#[tokio::test(start_paused = true)]
async fn a_settled_tap_reads_the_tree_while_the_screen_is_quiet_not_after_it() {
    let (source, fake) = FakeSource::with(vec![
        tree(&["One", "Two", "Three"]),
        tree(&["One", "Two", "Three!"]),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        fake.push_frame(big_frame(1));
    });
    let t = tokio::time::Instant::now();
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    let took = t.elapsed();
    assert_eq!(r["observation"]["kind"], "diff", "{r}");
    assert!(
        r["observation"]["text"]
            .as_str()
            .unwrap()
            .contains("Three!"),
        "{r}"
    );
    assert!(
        took <= Duration::from_millis(380),
        "the reply came {took:?} after the tap; the quiet period alone is 350 ms"
    );
}

#[tokio::test(start_paused = true)]
async fn a_tap_whose_frame_never_arrives_is_seen_changing_by_the_tree_before_the_500_ms_window_ends()
 {
    let (source, _) = FakeSource::with(vec![
        tree(&["One", "Two", "Three"]),
        tree(&["One", "Two", "Three"]),
        tree(&["One", "Two", "Three!"]),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let t = tokio::time::Instant::now();
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    let took = t.elapsed();
    assert_eq!(r["changed"], true, "{r}");
    assert!(
        r["observation"]["text"]
            .as_str()
            .unwrap()
            .contains("Three!"),
        "{r}"
    );
    assert!(
        took < Duration::from_millis(450),
        "the reply came {took:?} after the tap"
    );
}

#[tokio::test(start_paused = true)]
async fn a_settled_tap_leaves_its_phases_in_the_trace() {
    let (source, fake) = FakeSource::with(vec![
        tree(&["One", "Two", "Three"]),
        tree(&["One", "Two", "Three!"]),
    ]);
    let (d, home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        fake.push_frame(big_frame(1));
    });
    ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    let line = trace_text(home.path())
        .lines()
        .find(|l| l.contains("\"method\":\"act\""))
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .unwrap();
    let phases = line["phases"].as_object().unwrap();
    for name in ["prepare", "input", "settle", "tree", "render"] {
        assert!(phases.contains_key(name), "{name} missing in {phases:?}");
    }
    assert!(phases["settle"].as_f64().unwrap() >= 300.0, "{phases:?}");
}

#[tokio::test(start_paused = true)]
async fn act_returns_a_diff_after_the_tree_changes_and_the_screen_goes_quiet() {
    let (source, fake) = FakeSource::with(vec![
        tree(&["One", "Two", "Three"]),
        tree(&["One", "Two", "Three!"]),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        fake.push_frame(big_frame(1));
    });
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(r["settled"], true);
    assert_eq!(r["changed"], true);
    assert_eq!(r["observation"]["kind"], "diff");
    let ms = r["settle_ms"].as_u64().unwrap();
    assert!((340..=420).contains(&ms), "settle_ms {ms}");
    let text = r["observation"]["text"].as_str().unwrap();
    assert_eq!(
        text,
        "- @e3
+ @e4 button \"Three!\""
    );
}

#[tokio::test(start_paused = true)]
async fn a_reply_that_only_removes_lines_waits_up_to_two_seconds_for_the_new_window_to_show() {
    let mut trees = vec![tree(&["One", "Two"])];
    trees.extend(vec![tree(&["One"]); 12]);
    trees.push(tree(&["Select time", "OK"]));
    let (source, _fake) = FakeSource::with(trees);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    let text = r["observation"]["text"].as_str().unwrap();
    assert!(text.contains("Select time"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn a_screen_that_really_lost_lines_and_gained_none_is_reported_after_two_seconds() {
    let (source, _fake) = FakeSource::with(vec![tree(&["One", "Two"]), tree(&["One"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let started = tokio::time::Instant::now();
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(r["observation"]["text"], "- @e2");
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "{:?}",
        started.elapsed()
    );
}

fn progress_tree(labels: &[&str]) -> RawTree {
    let mut nodes = vec![
        r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}"#
            .to_string(),
        r#"{"p":0,"w":1,"cls":"android.widget.ProgressBar","b":[0,0,360,10],"f":"vVe"}"#
            .to_string(),
    ];
    for (i, l) in labels.iter().enumerate() {
        let top = 100 + 100 * i as i32;
        nodes.push(format!(
            r#"{{"p":0,"w":1,"cls":"android.widget.Button","text":"{l}","b":[0,{top},360,{}],"f":"vVe","a":["tap"]}}"#,
            top + 60
        ));
    }
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[{}]}}"#,
        nodes.join(",")
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

#[tokio::test(start_paused = true)]
async fn an_action_replies_with_the_tree_that_stopped_changing_not_the_one_caught_mid_transition() {
    let (source, fake) = FakeSource::with(vec![
        tree(&["One", "Two"]),
        tree(&["One"]),
        tree(&["Dialog", "Cancel", "OK"]),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        fake.push_frame(big_frame(1));
    });
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    let text = r["observation"]["text"].as_str().unwrap();
    assert!(text.contains("\"Dialog\""), "{text}");
    assert!(text.contains("\"OK\""), "{text}");
}

#[tokio::test(start_paused = true)]
async fn an_action_that_leaves_a_progress_bar_showing_waits_for_the_page_up_to_four_seconds() {
    let (source, fake) = FakeSource::with(vec![
        tree(&["Search"]),
        progress_tree(&["Loading"]),
        progress_tree(&["Loading"]),
        progress_tree(&["Loading"]),
        tree(&["Born December 9, 1906"]),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        fake.push_frame(big_frame(1));
    });
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    let text = r["observation"]["text"].as_str().unwrap();
    assert!(text.contains("Born December 9, 1906"), "{text}");
    assert_eq!(r["observation"]["loading"], false, "{r}");
}

#[tokio::test(start_paused = true)]
async fn a_progress_bar_that_never_goes_away_costs_at_most_four_seconds_of_waiting() {
    let (source, fake) = FakeSource::with(vec![tree(&["Search"]), progress_tree(&["Spinning"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        fake.push_frame(big_frame(1));
    });
    let started = tokio::time::Instant::now();
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(r["ok"], true);
    assert_eq!(r["observation"]["loading"], true, "{r}");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test(start_paused = true)]
async fn act_on_a_screen_that_never_stops_changing_is_ok_unsettled_and_wait_settled_fails() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let pusher = tokio::spawn(async move {
        for seq in 1..400 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            fake.push_frame(big_frame(seq));
        }
    });
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(r["ok"], true);
    assert_eq!(r["settled"], false);
    assert!(r["observation"]["still_changing"].is_object(), "{r}");
    let e = err(
        &d,
        "default",
        "wait",
        json!({"condition": {"settled": true}, "timeout_ms": 2000}),
    )
    .await;
    assert_eq!(e.code, -32012);
    pusher.abort();
}

#[tokio::test(start_paused = true)]
async fn act_on_a_screen_that_never_changes_reports_no_change_after_500_ms() {
    let (source, _) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(r["changed"], false);
    assert_eq!(r["settled"], true);
    let ms = r["settle_ms"].as_u64().unwrap();
    assert!((500..=520).contains(&ms), "settle_ms {ms}");
    assert_eq!(r["observation"]["text"], "");
}

#[tokio::test]
async fn a_window_change_event_counts_as_the_first_change() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"]), tree(&["Other"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let emitter = fake.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        emitter.emit(DeviceEvent::WindowChanged { package: None });
    });
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(r["changed"], true);
    assert_eq!(r["observation"]["kind"], "full");
}

#[tokio::test]
async fn a_new_screen_after_an_action_leaves_the_settle_time_to_the_action_line() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"]), tree(&["Other"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let emitter = fake.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        emitter.emit(DeviceEvent::WindowChanged { package: None });
    });
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(r["observation"]["kind"], "full");
    let text = r["observation"]["text"].as_str().unwrap();
    assert!(!text.contains("settled in"), "{text}");
    assert!(r["settle_ms"].as_u64().is_some(), "{r}");
}

#[cfg(unix)]
#[tokio::test]
async fn the_socket_file_is_0600_and_its_directory_0700() {
    use std::os::unix::fs::PermissionsExt;
    let (source, _) = FakeSource::with(vec![]);
    let (d, home) = daemon(source);
    let path = home.path().join("run").join("daemon.sock");
    let serving = tokio::spawn(d.clone().serve(path.clone()));
    wait_for_socket(&path).await;
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&path), 0o600);
    assert_eq!(mode(path.parent().unwrap()), 0o700);
    d.shutdown();
    serving.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn the_daemon_exits_after_fifteen_idle_minutes_with_no_clients() {
    let (source, _) = FakeSource::with(vec![]);
    let (d, home) = daemon(source);
    let path = local_socket(home.path(), "d.sock");
    let serving = tokio::spawn(d.clone().serve(path));
    tokio::time::sleep(Duration::from_secs(14 * 60)).await;
    assert!(!serving.is_finished());
    tokio::time::sleep(Duration::from_secs(2 * 60)).await;
    serving.await.unwrap().unwrap();
}

fn local_socket(home: &Path, name: &str) -> PathBuf {
    if cfg!(windows) {
        let scope = home.file_name().unwrap().to_string_lossy();
        PathBuf::from(format!(r"\\.\pipe\phonedriver-test-{scope}-{name}"))
    } else {
        home.join(name)
    }
}

async fn wait_for_socket(path: &Path) {
    for _ in 0..200 {
        if phonedriver::daemon::socket::connect(path).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("no socket at {}", path.display());
}

async fn rpc(path: &Path, session: &str, method: &str, params: Value) -> Value {
    let stream = phonedriver::daemon::socket::connect(path).await.unwrap();
    let (read, mut write) = tokio::io::split(stream);
    let mut params = params;
    params["session"] = session.into();
    let req = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    write
        .write_all(format!("{req}\n").as_bytes())
        .await
        .unwrap();
    let mut line = String::new();
    BufReader::new(read).read_line(&mut line).await.unwrap();
    serde_json::from_str(&line).unwrap()
}

fn type_params(text: &str, secret: bool) -> Value {
    json!({"action": {"kind": "type", "text": text, "into": {"ref": "e1"}, "secret": secret}})
}

fn trace_text(home: &Path) -> String {
    let mut all = String::new();
    for entry in std::fs::read_dir(home.join("traces")).unwrap() {
        all += &std::fs::read_to_string(entry.unwrap().path()).unwrap();
    }
    all
}

#[tokio::test(start_paused = true)]
async fn type_taps_the_field_pastes_and_verifies_the_text_by_reading_it_back() {
    let (source, fake) = FakeSource::with(vec![
        field_tree("", false),
        field_tree("", false),
        field_tree("héllo 世界", false),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", type_params("héllo 世界", false)).await;
    assert_eq!(r["verified"], true, "{r}");
    assert_eq!(r["via"], "paste");
    let calls = fake.calls();
    assert!(matches!(calls[0], Call::Gesture(_)), "{calls:?}");
    assert_eq!(
        calls[1],
        Call::Text(phonedriver_core::device::TextInput::Paste(
            "héllo 世界".into()
        ))
    );
    let wrong = FakeSource::with(vec![
        field_tree("", false),
        field_tree("", false),
        field_tree("something else", false),
    ]);
    let (d2, _h) = daemon(wrong.0);
    ok(&d2, "default", "snapshot", json!({})).await;
    let r = ok(&d2, "default", "act", type_params("héllo", false)).await;
    assert_eq!(r["verified"], false);
}

fn page_tree() -> RawTree {
    let json = r#"{"windows":[{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}],"nodes":[
        {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.widget.TextView","text":"Grace Hopper - Wikipedia","b":[0,0,360,60],"f":"vVe"}]}"#;
    RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap()
}

fn web_field_tree(value: &str) -> RawTree {
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.android.chrome","bounds":[0,0,360,800]}}],"nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.webkit.WebView","text":"Checkout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":1,"w":1,"cls":"android.widget.EditText","text":"{value}","desc":"Postcode","b":[0,100,360,160],"f":"vVeEF","a":["tap"]}}]}}"#
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

fn web_type(text: &str) -> Value {
    json!({"action": {"kind": "type", "text": text, "into": {"text": "Postcode"}}})
}

fn is_keys(t: &TextInput) -> bool {
    matches!(t, TextInput::Keystrokes(_))
}

#[tokio::test(start_paused = true)]
async fn a_web_field_that_ignores_a_paste_gets_the_text_key_by_key() {
    let (source, fake) = FakeSource::with(vec![web_field_tree("")]);
    fake.on_text(|t| is_keys(t).then(|| vec![web_field_tree("WC2H 7LU")]));
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", web_type("WC2H 7LU")).await;
    assert_eq!(r["verified"], true, "{r}");
    assert_eq!(r["via"], "keys", "{r}");
    let texts: Vec<Call> = fake
        .calls()
        .into_iter()
        .filter(|c| matches!(c, Call::Text(_)))
        .collect();
    assert_eq!(
        texts,
        [
            Call::Text(TextInput::Paste("WC2H 7LU".into())),
            Call::Text(TextInput::Keystrokes("WC2H 7LU".into()))
        ]
    );
}

fn two_web_fields_tree(notes: &str) -> RawTree {
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.apple.mobilesafari","bounds":[0,0,360,800]}}],"nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.webkit.WebView","text":"Order","b":[0,0,360,800],"f":"vVe"}},
        {{"p":1,"w":1,"cls":"android.widget.EditText","text":"Test Pilot","desc":"Name","b":[0,100,360,160],"f":"vVeEF","a":["tap"]}},
        {{"p":1,"w":1,"cls":"android.widget.EditText","text":"{notes}","desc":"Notes","b":[0,300,360,360],"f":"vVeEF","a":["tap"]}}]}}"#
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

#[tokio::test(start_paused = true)]
async fn a_web_page_that_calls_several_fields_focused_still_gets_the_text_in_the_field_asked_for() {
    let (source, fake) = FakeSource::with(vec![two_web_fields_tree("")]);
    fake.on_text(|_| Some(vec![two_web_fields_tree("ring twice")]));
    let (d, _home) = daemon(source);
    let shown = ok(&d, "default", "snapshot", json!({})).await;
    let notes = shown["text"]
        .as_str()
        .unwrap()
        .lines()
        .find(|l| l.contains("\"Notes\""))
        .and_then(|l| l.trim().split(' ').next())
        .unwrap()
        .trim_start_matches('@')
        .to_string();
    let params =
        json!({"action": {"kind": "type", "text": "ring twice", "into": {"text": "Notes"}}});
    let r = ok(&d, "default", "act", params).await;
    assert!(
        r["summary"]
            .as_str()
            .unwrap()
            .contains(&format!("into @{notes}")),
        "{r}"
    );
    assert_eq!(r["verified"], true, "{r}");
}

fn small_web_field_tree(value: &str, zoomed: bool) -> RawTree {
    let b = if zoomed {
        "[20,100,350,200]"
    } else {
        "[60,100,160,130]"
    };
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.apple.mobilesafari","bounds":[0,0,360,800]}}],"nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.webkit.WebView","text":"Order","b":[0,0,360,800],"f":"vVe"}},
        {{"p":1,"w":1,"cls":"android.widget.EditText","text":"{value}","desc":"Name","b":{b},"f":"vVeEF","a":["tap"]}}]}}"#
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

#[tokio::test(start_paused = true)]
async fn a_page_that_zooms_in_on_a_field_it_focuses_is_zoomed_back_out_after_typing() {
    let (source, fake) = FakeSource::with(vec![small_web_field_tree("", false)]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    fake.on_gesture(|g| (g.fingers.len() == 1).then(|| vec![small_web_field_tree("", true)]));
    fake.on_text(|_| Some(vec![small_web_field_tree("Test Pilot", false)]));
    let params =
        json!({"action": {"kind": "type", "text": "Test Pilot", "into": {"text": "Name"}}});
    let r = ok(&d, "default", "act", params).await;
    assert_eq!(r["verified"], true, "{r}");
    assert!(
        r["summary"]
            .as_str()
            .unwrap()
            .contains("zoomed the page back out"),
        "{r}"
    );
    let calls = fake.calls();
    let typed = calls
        .iter()
        .position(|c| matches!(c, Call::Text(_)))
        .unwrap();
    assert!(
        calls[typed..]
            .iter()
            .any(|c| matches!(c, Call::Gesture(g) if g.fingers.len() == 2)),
        "{calls:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_page_that_does_not_zoom_is_not_pinched() {
    let (source, fake) = FakeSource::with(vec![small_web_field_tree("", false)]);
    fake.on_text(|_| Some(vec![small_web_field_tree("Test Pilot", false)]));
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let params =
        json!({"action": {"kind": "type", "text": "Test Pilot", "into": {"text": "Name"}}});
    let r = ok(&d, "default", "act", params).await;
    assert!(!r["summary"].as_str().unwrap().contains("zoomed"), "{r}");
    assert!(
        !fake
            .calls()
            .iter()
            .any(|c| matches!(c, Call::Gesture(g) if g.fingers.len() == 2))
    );
}

#[tokio::test(start_paused = true)]
async fn a_paste_asked_for_by_name_is_not_retried_another_way() {
    let (source, fake) = FakeSource::with(vec![web_field_tree("")]);
    fake.on_text(|t| is_keys(t).then(|| vec![web_field_tree("WC2H 7LU")]));
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let params = json!({"action": {"kind": "type", "text": "WC2H 7LU", "into": {"text": "Postcode"}, "mode": "paste"}});
    let r = ok(&d, "default", "act", params).await;
    assert_eq!(r["verified"], false, "{r}");
    assert!(
        !fake
            .calls()
            .iter()
            .any(|c| matches!(c, Call::Text(t) if is_keys(t)))
    );
}

#[tokio::test(start_paused = true)]
async fn text_that_shows_in_a_web_field_a_moment_late_is_verified() {
    let (source, fake) = FakeSource::with(vec![web_field_tree("")]);
    fake.on_text(|t| {
        matches!(t, TextInput::Paste(_)).then(|| {
            let mut trees = vec![web_field_tree(""); 8];
            trees.push(web_field_tree("Sam"));
            trees
        })
    });
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", web_type("Sam")).await;
    assert_eq!(r["verified"], true, "{r}");
    assert_eq!(r["via"], "paste", "{r}");
}

#[tokio::test(start_paused = true)]
async fn typing_with_submit_into_a_field_that_closes_is_not_reported_as_unverified() {
    let (source, _fake) = FakeSource::with(vec![
        field_tree("", false),
        field_tree("", false),
        page_tree(),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let params = json!({"action": {"kind": "type", "text": "Grace Hopper", "into": {"ref": "e1"}, "submit": true}});
    let r = ok(&d, "default", "act", params).await;
    assert!(r["verified"].is_null(), "{r}");

    let (source, _fake) = FakeSource::with(vec![
        field_tree("", false),
        field_tree("", false),
        page_tree(),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", type_params("Grace Hopper", false)).await;
    assert_eq!(r["verified"], false, "{r}");
}

#[tokio::test(start_paused = true)]
async fn secret_text_never_reaches_the_trace_the_reply_or_the_debug_output() {
    let (source, fake) = FakeSource::with(vec![
        field_tree("", false),
        field_tree("", false),
        field_tree("hunter2", false),
    ]);
    let (d, home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", type_params("hunter2", true)).await;
    assert_eq!(r["verified"], true);
    assert!(!r.to_string().contains("hunter2"), "{r}");
    assert!(r["observation"]["text"].as_str().unwrap().contains("•••"));
    assert_eq!(
        fake.calls()[1],
        Call::Text(phonedriver_core::device::TextInput::PasteSecret(
            "hunter2".into()
        ))
    );
    assert!(!format!("{:?}", fake.calls()).contains("hunter2"));
    let trace = trace_text(home.path());
    assert!(trace.contains("\"secret_len\":7"), "{trace}");
    assert!(!trace.contains("hunter2"), "{trace}");
}

#[tokio::test(start_paused = true)]
async fn type_into_a_field_with_text_replaces_it_and_append_keeps_it() {
    use phonedriver_core::device::TextInput;
    let (source, fake) = FakeSource::with(vec![
        field_tree("10", false),
        field_tree("10", false),
        field_tree("07", false),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", type_params("07", false)).await;
    assert_eq!(r["verified"], true, "{r}");
    let calls = fake.calls();
    assert_eq!(calls[1], Call::Text(TextInput::SetValue(String::new())));
    assert_eq!(calls[2], Call::Text(TextInput::Paste("07".into())));

    let (source, fake) = FakeSource::with(vec![
        field_tree("ab", false),
        field_tree("ab", false),
        field_tree("abcd", false),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let mut p = type_params("cd", false);
    p["action"]["append"] = json!(true);
    let r = ok(&d, "default", "act", p).await;
    assert_eq!(r["verified"], true, "{r}");
    assert_eq!(fake.calls()[1], Call::Text(TextInput::Paste("cd".into())));
}

#[tokio::test(start_paused = true)]
async fn typed_text_settles_in_under_a_tap_s_first_change_wait() {
    let (source, _) = FakeSource::with(vec![
        field_tree("", false),
        field_tree("", false),
        field_tree("hi", false),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", type_params("hi", false)).await;
    assert!(r["settle_ms"].as_u64().unwrap() <= 200, "{r}");
}

#[tokio::test(start_paused = true)]
async fn type_into_a_group_that_holds_a_text_field_types_into_that_field() {
    let grouped = |value: &str| {
        let json = format!(
            r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[
            {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
            {{"p":0,"w":1,"cls":"android.view.ViewGroup","desc":"Search","b":[0,100,360,160],"f":"vVe","a":["tap"]}},
            {{"p":1,"w":1,"cls":"android.widget.EditText","text":"{value}","b":[10,110,350,150],"f":"vVeEF","a":["tap","set_text"]}}]}}"#
        );
        RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
    };
    let (source, fake) = FakeSource::with(vec![grouped(""), grouped(""), grouped("cats")]);
    let (d, _home) = daemon(source);
    let r = ok(&d, "default", "snapshot", json!({})).await;
    let text = r["text"].as_str().unwrap();
    assert!(text.contains("textbox"), "{text}");
    let r = ok(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "type", "text": "cats", "into": {"text": "Search"}}}),
    )
    .await;
    assert_eq!(r["verified"], true, "{r}");
    assert!(
        matches!(&fake.calls()[0], Call::Gesture(g) if (g.fingers[0][0].at.y - 130.0).abs() < 1.0),
        "{:?}",
        fake.calls()
    );
}

#[tokio::test(start_paused = true)]
async fn a_field_that_kept_its_old_text_is_reported_with_what_it_shows_and_a_hint_for_that_cause() {
    let (source, _) = FakeSource::with(vec![
        field_tree("10", false),
        field_tree("10", false),
        field_tree("10", false),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", type_params("07", false)).await;
    assert_eq!(r["verified"], false, "{r}");
    assert_eq!(r["unverified"]["shows"], "@e1 shows \"10\"", "{r}");
    assert!(r["unverified"]["hint"].is_string(), "{r}");
}

#[tokio::test(start_paused = true)]
async fn typing_a_secret_into_a_password_field_is_verified_by_length() {
    let (source, fake) = FakeSource::with(vec![
        field_tree("", true),
        field_tree("", true),
        field_tree("•••••••", true),
    ]);
    let (d, home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", type_params("hunter2", true)).await;
    assert_eq!(r["verified"], true, "{r}");
    assert_eq!(
        fake.calls()[1],
        Call::Text(phonedriver_core::device::TextInput::PasteSecret(
            "hunter2".into()
        ))
    );
    assert!(!r.to_string().contains("hunter2"), "{r}");
    assert!(!trace_text(home.path()).contains("hunter2"));
}

#[tokio::test(start_paused = true)]
async fn plain_text_is_refused_in_a_password_field_and_nothing_is_typed() {
    let (source, fake) = FakeSource::with(vec![field_tree("", true)]);
    let (d, home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let e = err(&d, "default", "act", type_params("hunter2", false)).await;
    assert_eq!(e.code, -32020, "{e:?}");
    assert_eq!(e.data["code_name"], "secret_field");
    assert!(e.message.contains("password"), "{}", e.message);
    assert!(e.message.contains("--secret"), "{}", e.message);
    assert!(e.message.contains("secret"), "{}", e.message);
    assert!(!e.to_json().to_string().contains("hunter2"));
    assert!(!trace_text(home.path()).contains("hunter2"));
    assert!(
        !fake
            .calls()
            .iter()
            .any(|c| matches!(c, Call::Text(_) | Call::Gesture(_))),
        "{:?}",
        fake.calls()
    );
}

#[tokio::test(start_paused = true)]
async fn plain_text_is_refused_in_the_focused_password_field_and_clearing_it_is_not() {
    let (source, fake) = FakeSource::with(vec![field_tree("", true)]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let e = err(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "type", "text": "hunter2"}}),
    )
    .await;
    assert_eq!(e.code, -32020, "{e:?}");
    ok(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "type", "text": "", "into": {"ref": "e1"}}, "settle": false}),
    )
    .await;
    assert!(
        fake.calls()
            .iter()
            .all(|c| !matches!(c, Call::Text(phonedriver_core::device::TextInput::Paste(_)))),
        "{:?}",
        fake.calls()
    );
}

fn coded_field(label: Option<&str>, hint: Option<&str>, id: Option<&str>) -> RawTree {
    let node = json!({
        "p": 0, "w": 1, "cls": "android.widget.EditText", "text": "", "desc": label, "hint": hint, "id": id,
        "b": [0, 100, 360, 160], "f": "vVeEF", "a": ["tap", "set_text"],
    });
    let json = json!({
        "windows": [{"id": 1, "type": "application", "package": "com.example", "bounds": [0, 0, 360, 800]}],
        "nodes": [
            {"p": -1, "w": 1, "cls": "android.widget.FrameLayout", "b": [0, 0, 360, 800], "f": "vVe"},
            node,
        ],
    });
    RawTree::from_android_json(&json.to_string(), Scale { density: 1.0 }).unwrap()
}

type FieldNames = (
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
);

const CODE_FIELDS: &[FieldNames] = &[
    (Some("Verification code"), None, None),
    (None, Some("Enter OTP"), None),
    (None, None, Some("com.shop:id/otp_input")),
    (None, None, Some("com.shop:id/verification_code")),
    (Some("One-time password"), None, None),
    (Some("SMS code"), None, None),
    (None, Some("2FA code"), None),
    (None, Some("Code"), None),
    (Some("Código de verificación"), None, None),
    (Some("Code de vérification"), None, None),
    (None, Some("Bestätigungscode"), None),
    (None, Some("Codice di verifica"), None),
    (Some("验证码"), None, None),
    (None, Some("请输入验证码"), None),
    (Some("認証コード"), None, None),
    (None, Some("인증번호 6자리"), None),
    (Some("ओटीपी"), None, None),
];

const PLAIN_FIELDS: &[FieldNames] = &[
    (Some("Promo code"), None, None),
    (None, Some("Zip code"), None),
    (None, Some("Postal code"), None),
    (Some("Coupon code"), None, None),
    (Some("Country code"), None, None),
    (None, Some("Discount code"), None),
    (None, Some("Referral code"), None),
    (None, Some("Invite code"), None),
    (Some("Barcode"), None, None),
    (Some("Search"), None, None),
    (None, Some("Your name"), None),
    (None, None, Some("com.shop:id/promo_code")),
    (Some("优惠码"), None, None),
];

#[tokio::test(start_paused = true)]
async fn plain_text_is_refused_in_a_one_time_code_field_but_a_secret_goes_in() {
    for (label, hint, id) in CODE_FIELDS {
        let (source, fake) = FakeSource::with(vec![
            coded_field(*label, *hint, *id),
            coded_field(*label, *hint, *id),
        ]);
        let (d, _home) = daemon(source);
        ok(&d, "default", "snapshot", json!({})).await;
        let e = err(&d, "default", "act", type_params("493021", false)).await;
        assert_eq!(e.code, -32020, "{label:?} {hint:?} {id:?}: {e:?}");
        assert!(e.message.contains("one-time code"), "{}", e.message);
        assert!(!e.to_json().to_string().contains("493021"));
        assert!(fake.calls().is_empty(), "{:?}", fake.calls());
        ok(&d, "default", "act", type_params("493021", true)).await;
        assert!(
            fake.calls().iter().any(|c| matches!(
                c,
                Call::Text(phonedriver_core::device::TextInput::PasteSecret(_))
            )),
            "{:?}",
            fake.calls()
        );
    }
}

#[tokio::test(start_paused = true)]
async fn plain_text_still_goes_into_fields_that_only_mention_a_code() {
    for (label, hint, id) in PLAIN_FIELDS {
        let (source, fake) = FakeSource::with(vec![
            coded_field(*label, *hint, *id),
            coded_field(*label, *hint, *id),
        ]);
        let (d, _home) = daemon(source);
        ok(&d, "default", "snapshot", json!({})).await;
        ok(&d, "default", "act", type_params("SAVE10", false)).await;
        assert!(
            fake.calls()
                .iter()
                .any(|c| matches!(c, Call::Text(phonedriver_core::device::TextInput::Paste(_)))),
            "{label:?} {hint:?} {id:?}: {:?}",
            fake.calls()
        );
    }
}

#[tokio::test(start_paused = true)]
async fn a_batch_refuses_plain_text_for_a_password_field_and_says_what_to_do() {
    let (source, fake) = FakeSource::with(vec![field_tree("", true)]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(
        &d,
        "default",
        "batch",
        json!({"steps": [{"kind": "type", "text": "hunter2", "into": {"ref": "e1"}}]}),
    )
    .await;
    assert_eq!(r["failed"]["error"]["code"], -32020, "{r}");
    assert!(
        r["failed"]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--secret"),
        "{r}"
    );
    assert!(!r.to_string().contains("hunter2"), "{r}");
    assert!(fake.calls().is_empty(), "{:?}", fake.calls());
}

#[tokio::test(start_paused = true)]
async fn a_malformed_type_action_does_not_echo_the_text_in_its_error() {
    let (source, _) = FakeSource::with(vec![field_tree("", false)]);
    let (d, _home) = daemon(source);
    let e = err(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "type", "text": "hunter2", "mode": "hunter2"}}),
    )
    .await;
    assert_eq!(e.code, -32602);
    assert!(!e.to_json().to_string().contains("hunter2"));
}

#[tokio::test(start_paused = true)]
async fn a_malformed_action_names_the_missing_field_and_never_the_value() {
    let (source, _) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    let e = err(&d, "default", "act", json!({"action": {"kind": "open"}})).await;
    assert_eq!(e.code, -32602);
    assert!(e.message.contains("missing field `url`"), "{}", e.message);
    let e = err(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "type", "text": "hunter2", "secret": "hunter2"}}),
    )
    .await;
    assert!(e.message.contains("expected a boolean"), "{}", e.message);
    assert!(!e.to_json().to_string().contains("hunter2"));
}

#[tokio::test(start_paused = true)]
async fn an_action_the_daemon_does_not_know_says_the_daemon_is_older() {
    let (source, _) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    let e = err(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "teleport", "direction": "down"}}),
    )
    .await;
    assert_eq!(e.code, -32602);
    assert!(
        e.message.contains("does not know the action `teleport`") && e.message.contains("older"),
        "{}",
        e.message
    );
}

#[tokio::test(start_paused = true)]
async fn a_snapshot_of_a_screen_that_has_been_quiet_returns_at_once_and_one_right_after_activity_waits()
 {
    let (source, fake) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    let first = ok(&d, "default", "snapshot", json!({})).await;
    let ms = first["settle_ms"].as_u64().unwrap();
    assert!(
        (200..=260).contains(&ms),
        "right after connect the quiet period is still running: {ms}"
    );
    tokio::time::sleep(Duration::from_secs(1)).await;
    let second = ok(&d, "default", "snapshot", json!({})).await;
    assert_eq!(second["settle_ms"], 0, "{second}");
    assert_eq!(second["settled"], true);
    fake.push_frame(big_frame(1));
    tokio::time::sleep(Duration::from_millis(100)).await;
    let third = ok(&d, "default", "snapshot", json!({})).await;
    let ms = third["settle_ms"].as_u64().unwrap();
    assert!(
        (140..=260).contains(&ms),
        "100 ms into a burst, about 150 ms of quiet remain: {ms}"
    );
}

#[tokio::test(start_paused = true)]
async fn act_without_settle_returns_when_the_input_is_delivered_and_reads_no_tree() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"]), tree(&["Two"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let mut params = tap_point(10.0, 10.0);
    params["settle"] = json!(false);
    let started = tokio::time::Instant::now();
    let r = ok(&d, "default", "act", params).await;
    assert_eq!(started.elapsed(), Duration::ZERO, "no settle wait");
    assert_eq!(r["ok"], true);
    assert_eq!(r["settled"], false);
    assert_eq!(r["observation"]["text"], "");
    assert_eq!(fake.calls().len(), 1);
    let next = ok(&d, "default", "snapshot", json!({"no_settle": true})).await;
    assert!(
        next["text"].as_str().unwrap().contains("Two"),
        "the second scripted tree is still unread: {next}"
    );
}

#[tokio::test(start_paused = true)]
async fn type_without_into_finds_the_focused_field_in_the_last_observation_instead_of_reading_a_tree()
 {
    let (source, fake) = FakeSource::with(vec![field_tree("", true), tree(&["Two"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "type", "text": "hunter2", "secret": true}, "settle": false}),
    )
    .await;
    assert_eq!(r["ok"], true);
    assert_eq!(
        fake.calls()[0],
        Call::Text(phonedriver_core::device::TextInput::PasteSecret(
            "hunter2".into()
        )),
        "the last observation showed a focused password field"
    );
    let next = ok(&d, "default", "snapshot", json!({"no_settle": true})).await;
    assert!(
        next["text"].as_str().unwrap().contains("Two"),
        "second tree unread: {next}"
    );
}

#[tokio::test(start_paused = true)]
async fn type_without_into_when_nothing_has_focus_types_nothing_and_names_the_fields() {
    let unfocused = || {
        let json = r#"{"windows":[{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}],"nodes":[
        {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.widget.EditText","desc":"Search","b":[0,100,360,160],"f":"vVeE","a":["tap","set_text"]}]}"#;
        RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap()
    };
    let (source, fake) = FakeSource::with(vec![unfocused(), unfocused()]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let e = err(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "type", "text": "Grace Hopper"}}),
    )
    .await;
    assert_eq!(e.code, -32023, "{e:?}");
    assert_eq!(e.data["code_name"], "not_focused");
    assert!(e.message.contains("nothing was typed"), "{}", e.message);
    let hint = e.data["hint"].as_str().unwrap_or_default();
    assert!(hint.contains("--into"), "{e:?}");
    assert!(hint.contains("textbox \"Search\""), "{e:?}");
    assert!(
        !fake.calls().iter().any(|c| matches!(c, Call::Text(_))),
        "{:?}",
        fake.calls()
    );
}

#[tokio::test(start_paused = true)]
async fn wait_settled_right_after_an_unobserved_action_waits_for_its_first_change() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    let mut params = tap_point(10.0, 10.0);
    params["settle"] = json!(false);
    ok(&d, "default", "act", params).await;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        fake.push_frame(big_frame(1));
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let r = ok(
        &d,
        "default",
        "wait",
        json!({"condition": {"settled": true}}),
    )
    .await;
    let ms = r["settle_ms"].as_u64().unwrap();
    assert!(
        (500..=600).contains(&ms),
        "the first change arrived 300 ms after the action and quiet takes 250 ms more: {ms}"
    );
}

#[tokio::test(start_paused = true)]
async fn an_unobserved_action_is_forgotten_after_the_first_change_window() {
    let (source, _) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let mut params = tap_point(10.0, 10.0);
    params["settle"] = json!(false);
    ok(&d, "default", "act", params).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let r = ok(
        &d,
        "default",
        "wait",
        json!({"condition": {"settled": true}}),
    )
    .await;
    assert_eq!(r["settle_ms"], 0, "{r}");
}

#[tokio::test(start_paused = true)]
async fn expect_change_ms_widens_the_first_change_window_of_an_action_and_of_a_wait() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let pusher = fake.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(700)).await;
        pusher.push_frame(big_frame(1));
    });
    let mut params = tap_point(10.0, 10.0);
    params["expect_change_ms"] = json!(1500);
    let r = ok(&d, "default", "act", params).await;
    assert_eq!(r["changed"], true, "{r}");
    let ms = r["settle_ms"].as_u64().unwrap();
    assert!((940..=1000).contains(&ms), "{ms}");

    let mut params = tap_point(10.0, 10.0);
    params["settle"] = json!(false);
    ok(&d, "default", "act", params).await;
    let late = fake.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(700)).await;
        late.push_frame(big_frame(2));
    });
    let r = ok(
        &d,
        "default",
        "wait",
        json!({"condition": {"settled": true}, "expect_change_ms": 1500}),
    )
    .await;
    let ms = r["settle_ms"].as_u64().unwrap();
    assert!((940..=1000).contains(&ms), "{ms}");
}

#[tokio::test(start_paused = true)]
async fn a_misspelled_text_target_is_not_found_and_names_the_close_element() {
    let (source, fake) = FakeSource::with(vec![tree(&["Row 5", "Wallpaper"])]);
    let (d, _home) = daemon(source);
    let e = err(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "tap", "target": {"text": "Rwo 5"}}}),
    )
    .await;
    assert_eq!(e.code, -32007);
    let similar = e.data["similar"].to_string();
    assert!(similar.contains("Row 5"), "{similar}");
    assert!(!similar.contains("Wallpaper"), "{similar}");
    assert!(fake.calls().is_empty());
}

#[tokio::test(start_paused = true)]
async fn find_lists_the_elements_whose_text_contains_the_query_in_any_case() {
    let (source, _) = FakeSource::with(vec![tree(&["Row 1", "Row 10", "Row 2", "Settings"])]);
    let (d, _home) = daemon(source);
    let r = ok(&d, "default", "find", json!({"text": "row 1"})).await;
    let lines: Vec<&str> = r
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap())
        .collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[0].contains("\"Row 1\"") && lines[1].contains("\"Row 10\""),
        "{lines:?}"
    );
    let r = ok(
        &d,
        "default",
        "find",
        json!({"text": "row 1", "role": "cell"}),
    )
    .await;
    assert_eq!(r.as_array().unwrap().len(), 0);
    let r = ok(
        &d,
        "default",
        "find",
        json!({"text": "row 1", "role": "button"}),
    )
    .await;
    assert_eq!(r.as_array().unwrap().len(), 2);
}

fn list_tree(labels: &[&str]) -> RawTree {
    let mut nodes = vec![
        r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}"#
            .to_string(),
        r#"{"p":0,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,100,360,700],"f":"vVeS","a":["scroll_down"],"coll":{"rows":1000,"cols":1}}"#
            .to_string(),
    ];
    for (i, l) in labels.iter().enumerate() {
        let top = 100 + 50 * i as i32;
        nodes.push(format!(
            r#"{{"p":1,"w":1,"cls":"android.widget.TextView","text":"{l}","b":[0,{top},360,{}],"f":"vVe"}}"#,
            top + 50
        ));
    }
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[{}]}}"#,
        nodes.join(",")
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

fn scroll(extra: Value) -> Value {
    let mut action = json!({"kind": "scroll", "direction": "down"});
    for (k, v) in extra.as_object().unwrap() {
        action[k] = v.clone();
    }
    json!({"action": action})
}

#[tokio::test(start_paused = true)]
async fn scroll_drags_inside_the_scrollable_container_60_percent_of_its_height_and_holds_before_the_lift()
 {
    let (source, fake) = FakeSource::with(vec![
        list_tree(&["Row 0", "Row 1"]),
        list_tree(&["Row 5", "Row 6"]),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", scroll(json!({}))).await;
    assert_eq!(r["ok"], true);
    let Call::Gesture(g) = &fake.calls()[0] else {
        panic!("{:?}", fake.calls())
    };
    let p = &g.fingers[0];
    assert!(
        (p[0].at.y - 580.0).abs() < 0.1,
        "20% above the bottom edge: {p:?}"
    );
    assert!((p[10].at.y - 220.0).abs() < 0.1, "{p:?}");
    assert!((p[0].at.x - 180.0).abs() < 0.1);
    assert_eq!(p.last().unwrap().t_ms - p[10].t_ms, 150);
}

#[tokio::test(start_paused = true)]
async fn scroll_until_stops_when_the_text_shows_and_reports_its_ref() {
    let (source, fake) = FakeSource::with(vec![
        list_tree(&["Row 0", "Row 1"]),
        list_tree(&["Row 5", "Row 6"]),
        list_tree(&["Row 9", "Row 10"]),
    ]);
    let (d, _home) = daemon(source);
    let r = ok(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Row 10"}})),
    )
    .await;
    let summary = r["summary"].as_str().unwrap();
    assert!(
        summary.contains("2 times") && summary.contains("Row 10"),
        "{summary}"
    );
    assert_eq!(fake.calls().len(), 2);
}

#[tokio::test]
async fn told_says_whether_the_session_was_last_shown_the_same_device_line() {
    let (source, _fake) = FakeSource::with(vec![list_tree(&["Row 0"])]);
    let (d, _home) = daemon(source);
    let said = |line: &'static str, session: &'static str| {
        let d = &d;
        async move {
            ok(d, session, "told", json!({"line": line})).await["same"]
                .as_bool()
                .unwrap()
        }
    };
    assert!(!said("device a · portrait", "one").await);
    assert!(said("device a · portrait", "one").await);
    assert!(!said("device a · landscape", "one").await);
    assert!(!said("device a · portrait", "one").await);
    assert!(!said("device a · portrait", "two").await);
}

#[tokio::test(start_paused = true)]
async fn scroll_until_row_3_does_not_stop_at_row_364() {
    let (source, fake) = FakeSource::with(vec![
        list_tree(&["Row 0", "Row 1"]),
        list_tree(&["Row 364", "Row 365"]),
        list_tree(&["Row 2", "Row 3"]),
    ]);
    let (d, _home) = daemon(source);
    let r = ok(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Row 3"}})),
    )
    .await;
    let summary = r["summary"].as_str().unwrap();
    assert!(
        summary.contains("2 times") && summary.contains("\"Row 3\""),
        "{summary}"
    );
    assert_eq!(fake.calls().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn scroll_until_text_already_on_screen_twice_stops_at_the_first_without_scrolling() {
    let (source, fake) = FakeSource::with(vec![list_tree(&["Row 1", "Row 2"])]);
    let (d, _home) = daemon(source);
    let r = ok(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Row"}})),
    )
    .await;
    let summary = r["summary"].as_str().unwrap();
    assert!(summary.contains("\"Row 1\""), "{summary}");
    assert!(summary.contains("already on screen"), "{summary}");
    assert_eq!(
        r["settled"], false,
        "no settle ran, so no 'no change' hint: {r}"
    );
    assert!(fake.calls().is_empty());
}

fn page_under_a_toolbar(rows: &[&str]) -> RawTree {
    let mut nodes = vec![
        r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}"#.to_string(),
        r#"{"p":0,"w":1,"cls":"android.widget.EditText","hint":"Search Google or type URL","b":[56,30,300,90],"f":"vVe","a":["tap","set_text"]}"#.to_string(),
        r#"{"p":0,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,100,360,700],"f":"vVeS","a":["scroll_down","scroll_up"]}"#.to_string(),
    ];
    for (i, l) in rows.iter().enumerate() {
        let top = 100 + 50 * i as i32;
        nodes.push(format!(
            r#"{{"p":2,"w":1,"cls":"android.widget.Button","text":"{l}","b":[0,{top},360,{}],"f":"vVe","a":["tap"]}}"#,
            top + 50
        ));
    }
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[{}]}}"#,
        nodes.join(",")
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

fn page_without_its_toolbar(rows: &[&str]) -> RawTree {
    let mut nodes = vec![
        r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}"#.to_string(),
        r#"{"p":0,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,0,360,700],"f":"vVeS","a":["scroll_down","scroll_up"]}"#.to_string(),
    ];
    for (i, l) in rows.iter().enumerate() {
        let top = 100 + 50 * i as i32;
        nodes.push(format!(
            r#"{{"p":1,"w":1,"cls":"android.widget.Button","text":"{l}","b":[0,{top},360,{}],"f":"vVe","a":["tap"]}}"#,
            top + 50
        ));
    }
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[{}]}}"#,
        nodes.join(",")
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

#[tokio::test(start_paused = true)]
async fn scroll_until_ignores_a_toolbar_that_slides_back_over_the_page() {
    let (source, fake) = FakeSource::with(vec![
        page_without_its_toolbar(&["of chips", "made by Qualcomm"]),
        page_under_a_toolbar(&["Snapdragon is a suite", "of chips"]),
        page_under_a_toolbar(&["Wikipedia", "Search"]),
    ]);
    let (d, _home) = daemon(source);
    let r = ok(
        &d,
        "default",
        "act",
        scroll(json!({"direction": "up", "until": {"text": "Search"}})),
    )
    .await;
    let summary = r["summary"].as_str().unwrap();
    assert!(summary.contains("2 times"), "{summary}");
    assert!(summary.ends_with("\"Search\""), "{summary}");
    assert_eq!(fake.calls().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn scroll_until_looks_inside_what_it_scrolls_not_at_a_toolbar_above_it() {
    let (source, fake) = FakeSource::with(vec![
        page_under_a_toolbar(&["Snapdragon is a suite", "of chips"]),
        page_under_a_toolbar(&["Wikipedia", "Search"]),
    ]);
    let (d, _home) = daemon(source);
    let r = ok(
        &d,
        "default",
        "act",
        scroll(json!({"direction": "up", "until": {"text": "Search"}})),
    )
    .await;
    let summary = r["summary"].as_str().unwrap();
    assert!(summary.contains("1 time"), "{summary}");
    assert!(summary.ends_with("\"Search\""), "{summary}");
    assert_eq!(fake.calls().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_tap_on_a_target_that_is_not_there_fails_without_the_daemon_waiting() {
    let (source, fake) = FakeSource::with(vec![list_tree(&["Row 0", "Row 1"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let started = tokio::time::Instant::now();
    let e = err(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "tap", "target": {"text": "Nope"}}}),
    )
    .await;
    assert_eq!(e.code, -32007, "{e:?}");
    assert_eq!(started.elapsed(), Duration::ZERO);
    assert!(fake.calls().is_empty());
}

#[tokio::test(start_paused = true)]
async fn scroll_until_after_one_gesture_says_one_time() {
    let (source, _fake) = FakeSource::with(vec![
        list_tree(&["Row 0", "Row 1"]),
        list_tree(&["Row 5", "Row 6"]),
    ]);
    let (d, _home) = daemon(source);
    let r = ok(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Row 6"}})),
    )
    .await;
    let summary = r["summary"].as_str().unwrap();
    assert!(summary.contains("down 1 time to"), "{summary}");
}

#[tokio::test(start_paused = true)]
async fn scroll_until_reports_unsettled_when_the_last_scroll_never_settled() {
    let (source, fake) = FakeSource::with(vec![
        list_tree(&["Row 0", "Row 1"]),
        list_tree(&["Row 5", "Row 6"]),
        list_tree(&["Row 9", "Row 10"]),
    ]);
    let (d, _home) = daemon(source);
    let pusher = tokio::spawn({
        let fake = fake.clone();
        async move {
            for seq in 1..400 {
                tokio::time::sleep(Duration::from_millis(50)).await;
                fake.push_frame(big_frame(seq));
            }
        }
    });
    let r = ok(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Row 10"}})),
    )
    .await;
    pusher.abort();
    assert_eq!(r["ok"], true);
    assert_eq!(r["settled"], false, "{r}");
    assert!(r["summary"].as_str().unwrap().contains("Row 10"), "{r}");
}

#[tokio::test(start_paused = true)]
async fn scroll_until_says_the_list_ended_where_it_ended_after_it_had_moved() {
    let (source, fake) = FakeSource::with(vec![
        list_tree(&["Row 0", "Row 1"]),
        list_tree(&["Row 5", "Row 6"]),
        list_tree(&["Row 998", "Row 999"]),
    ]);
    let (d, _home) = daemon(source);
    let e = err(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Zebra"}})),
    )
    .await;
    assert_eq!(e.code, -32016, "{e:?}");
    assert!(
        e.message.contains("scrolled 4 times")
            && e.message.contains("the list ended at \"Row 999\"")
            && e.message.contains("\"Zebra\""),
        "{}",
        e.message
    );
    assert_eq!(fake.calls().len(), 4);
}

#[tokio::test(start_paused = true)]
async fn a_list_end_is_named_by_a_row_never_by_a_floating_toolbar() {
    let (source, _fake) = FakeSource::with(vec![
        toolbar_list(300, "General"),
        toolbar_list(300, "Developer"),
    ]);
    let (d, _home) = daemon(source);
    let e = err(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Zebra"}})),
    )
    .await;
    assert!(
        e.message.contains("ended at \"Developer\""),
        "not the Toolbar: {}",
        e.message
    );
}

#[tokio::test(start_paused = true)]
async fn scroll_until_says_the_list_did_not_move_when_nothing_ever_changed() {
    let (source, fake) = FakeSource::with(vec![list_tree(&["Row 998", "Row 999"])]);
    let (d, _home) = daemon(source);
    let e = err(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Row 5000"}})),
    )
    .await;
    assert_eq!(e.code, -32016, "{e:?}");
    assert!(
        e.message.contains("the list did not move") && e.message.contains("2 scrolls"),
        "{}",
        e.message
    );
    assert!(!e.message.contains("no element matches"), "{}", e.message);
    assert_eq!(
        fake.calls().len(),
        2,
        "the end is two scrolls that change nothing"
    );
}

#[tokio::test(start_paused = true)]
async fn scroll_until_gives_up_after_max_gestures_and_says_how_many() {
    let trees: Vec<RawTree> = (0..10)
        .map(|i| list_tree(&[&format!("Row {}", i * 2), &format!("Row {}", i * 2 + 1)]))
        .collect();
    let (source, fake) = FakeSource::with(trees);
    let (d, _home) = daemon(source);
    let e = err(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Row 99"}, "max": 3})),
    )
    .await;
    assert_eq!(e.code, -32016, "{e:?}");
    assert!(
        e.message.contains("scrolled 3 times") && e.message.contains("still not on screen"),
        "{}",
        e.message
    );
    assert_eq!(fake.calls().len(), 3);
}

fn is_fling(call: &Call) -> bool {
    let Call::Gesture(g) = call else {
        panic!("{call:?}")
    };
    g.fingers[0].last().unwrap().t_ms <= 100
}

#[tokio::test(start_paused = true)]
async fn scroll_until_a_far_numbered_row_flings_on_a_device_with_a_frame_stream() {
    let (source, fake) = FakeSource::with(vec![list_tree(&["Row 0", "Row 1"])]);
    let (d, _home) = daemon(source);
    let _ = err(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Row 500"}})),
    )
    .await;
    assert!(fake.calls().iter().all(is_fling), "{:?}", fake.calls());
}

#[tokio::test(start_paused = true)]
async fn scroll_until_a_name_far_down_an_alphabetical_list_flings_and_a_near_one_drags() {
    let apps = [
        "Activity Launcher",
        "Adobe Scan",
        "Airlearn",
        "Amazon Shopping",
        "Android Auto",
    ];
    let first_gesture = |target: &'static str, rows: &'static [&'static str]| async move {
        let (source, fake) = FakeSource::with(vec![list_tree(rows)]);
        let (d, _home) = daemon(source);
        let _ = d
            .call(
                "default",
                "act",
                scroll(json!({"until": {"text": target}, "max": 1})),
            )
            .await;
        fake.calls().first().map(is_fling)
    };
    let sorted: &'static [&'static str] = Box::leak(Box::new(apps));
    assert_eq!(first_gesture("YouTube", sorted).await, Some(true));
    assert_eq!(first_gesture("Bluetooth", sorted).await, Some(false));
    let mixed: &'static [&'static str] = Box::leak(Box::new([
        "Wi-Fi",
        "Bluetooth",
        "Apps",
        "Battery",
        "Display",
    ]));
    assert_eq!(first_gesture("YouTube", mixed).await, Some(false));
}

/// A sorted list of 39 names, A to M without "Gmail", five on screen. A drag moves it three rows and a fling twelve.
struct AlphaList {
    inner: Arc<FakeDevice>,
    rows: Vec<String>,
    top: Mutex<usize>,
}

impl AlphaList {
    fn new() -> AlphaList {
        let rows = ('A'..='M')
            .flat_map(|c| (0..3).map(move |k| format!("{c}{}app{k}", c.to_ascii_lowercase())))
            .collect();
        AlphaList {
            inner: Arc::new(FakeDevice::new(vec![tree(&["unused"])])),
            rows,
            top: Mutex::new(0),
        }
    }
}

#[async_trait]
impl Device for AlphaList {
    fn info(&self) -> &phonedriver_core::device::DeviceInfo {
        self.inner.info()
    }
    async fn tree(&self) -> Result<RawTree, Error> {
        let top = *self.top.lock().unwrap();
        let shown: Vec<&str> = self.rows[top..(top + 5).min(self.rows.len())]
            .iter()
            .map(String::as_str)
            .collect();
        Ok(list_tree(&shown))
    }
    fn frames(&self) -> Option<tokio::sync::watch::Receiver<FrameRef>> {
        self.inner.frames()
    }
    async fn screenshot(
        &self,
        q: phonedriver_core::device::Quality,
    ) -> Result<phonedriver_core::device::Image, Error> {
        self.inner.screenshot(q).await
    }
    fn events(&self) -> tokio::sync::broadcast::Receiver<DeviceEvent> {
        self.inner.events()
    }
    async fn gesture(&self, g: phonedriver_core::device::Gesture) -> Result<(), Error> {
        let path = &g.fingers[0];
        let (a, b) = (path.first().unwrap(), path.last().unwrap());
        let rows = if b.t_ms - a.t_ms < 150 { 12 } else { 3 };
        let moved = {
            let mut top = self.top.lock().unwrap();
            let before = *top;
            let last = self.rows.len() - 5;
            *top = if b.at.y < a.at.y {
                (*top + rows).min(last)
            } else {
                top.saturating_sub(rows)
            };
            *top != before
        };
        if moved {
            let seq = self.inner.calls().len() as u64 + 1;
            self.inner.push_frame(big_frame(seq));
        }
        self.inner.gesture(g).await
    }
    async fn key(&self, k: phonedriver_core::device::Key) -> Result<(), Error> {
        self.inner.key(k).await
    }
    async fn button(&self, b: phonedriver_core::device::Button) -> Result<(), Error> {
        self.inner.button(b).await
    }
    async fn open_url(
        &self,
        u: &phonedriver_core::device::Url,
        app: Option<&AppId>,
    ) -> Result<(), Error> {
        self.inner.open_url(u, app).await
    }
    async fn text(&self, t: phonedriver_core::device::TextInput) -> Result<(), Error> {
        self.inner.text(t).await
    }
    async fn clipboard(
        &self,
        op: phonedriver_core::device::ClipboardOp,
    ) -> Result<Option<String>, Error> {
        self.inner.clipboard(op).await
    }
    async fn apps(&self) -> Result<Vec<phonedriver_core::device::App>, Error> {
        self.inner.apps().await
    }
    async fn launch(&self, app: &phonedriver_core::device::AppId) -> Result<(), Error> {
        self.inner.launch(app).await
    }
    async fn foreground(&self) -> Result<Option<phonedriver_core::device::AppId>, Error> {
        self.inner.foreground().await
    }
}

#[tokio::test(start_paused = true)]
async fn scroll_until_a_name_missing_from_an_alphabetical_list_goes_on_one_way_to_the_end() {
    let list = Arc::new(AlphaList::new());
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::new(
        Arc::new(OneDevice(list.clone())),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    let found = ok(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Kkapp1"}})),
    )
    .await;
    assert!(
        found["summary"].as_str().unwrap().ends_with("\"Kkapp1\""),
        "{found}"
    );
    *list.top.lock().unwrap() = 0;
    let e = err(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Gmail"}})),
    )
    .await;
    assert!(e.message.contains("list ended"), "{e:?}");
}

#[tokio::test(start_paused = true)]
async fn scroll_until_never_flings_on_a_polled_device_where_a_fling_moves_nothing() {
    let (source, fake) = FakeSource::polled(vec![list_tree(&["Row 0", "Row 1"])]);
    let (d, _home) = daemon(source);
    let _ = err(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Row 500"}})),
    )
    .await;
    let calls = fake.calls();
    assert!(!calls.is_empty());
    assert!(calls.iter().all(|c| !is_fling(c)), "{calls:?}");
}

#[tokio::test(start_paused = true)]
async fn scroll_until_a_far_numbered_row_on_a_polled_device_gets_more_than_thirty_scrolls() {
    let trees: Vec<RawTree> = (0..200)
        .map(|i| list_tree(&[&format!("Row {}", i * 8), &format!("Row {}", i * 8 + 7)]))
        .collect();
    let (source, fake) = FakeSource::polled(trees);
    let (d, _home) = daemon(source);
    let r = ok(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Row 640"}})),
    )
    .await;
    assert!(r["summary"].as_str().unwrap().contains("Row 640"), "{r}");
    assert!(fake.calls().len() > 30, "{}", fake.calls().len());
}

fn act_with(action: Value) -> Value {
    json!({"action": action})
}

#[tokio::test(start_paused = true)]
async fn tap_count_2_sends_two_taps_80_ms_apart_and_long_sends_one_press_that_lifts_after_the_time()
{
    let (source, fake) = FakeSource::with(vec![list_tree(&["Row 0"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let point = json!({"point": {"x": 50.0, "y": 60.0}});
    ok(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "tap", "target": point, "count": 2})),
    )
    .await;
    ok(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "tap", "target": point, "long": 800})),
    )
    .await;
    let calls = fake.calls();
    let gestures: Vec<_> = calls
        .iter()
        .filter_map(|c| match c {
            Call::Gesture(g) => Some(g),
            _ => None,
        })
        .collect();
    assert_eq!(gestures[0].fingers.len(), 2, "{gestures:?}");
    assert_eq!(gestures[0].fingers[1][0].t_ms, 80);
    let p = &gestures[1].fingers[0];
    assert_eq!(p.last().unwrap().t_ms - p[0].t_ms, 800);
}

#[tokio::test(start_paused = true)]
async fn a_gesture_plays_the_fingers_it_is_given_and_a_finger_with_one_point_is_invalid() {
    let (source, fake) = FakeSource::with(vec![list_tree(&["Row 0"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let pinch = json!({"kind": "gesture", "fingers": [
        [{"x": 100.0, "y": 300.0, "t_ms": 0}, {"x": 50.0, "y": 300.0, "t_ms": 300}],
        [{"x": 200.0, "y": 300.0, "t_ms": 0}, {"x": 250.0, "y": 300.0, "t_ms": 300}]
    ]});
    ok(&d, "default", "act", act_with(pinch)).await;
    let Some(Call::Gesture(g)) = fake
        .calls()
        .into_iter()
        .find(|c| matches!(c, Call::Gesture(_)))
    else {
        panic!("no gesture")
    };
    assert_eq!(g.fingers.len(), 2);
    assert_eq!(g.fingers[1][1].at.x, 250.0);
    let bad = json!({"kind": "gesture", "fingers": [[{"x": 1.0, "y": 1.0, "t_ms": 0}]]});
    let e = err(&d, "default", "act", act_with(bad)).await;
    assert_eq!(e.code, -32602, "{e:?}");
}

fn installed() -> (Arc<FakeSource>, Arc<FakeDevice>) {
    let (source, fake) = FakeSource::with(vec![
        list_tree(&["Row 0"]),
        list_tree(&["Calculator", "7", "8"]),
    ]);
    fake.install(&[
        ("dev.phonedriver.testbed", "phonedriver testbed"),
        ("com.android.calculator2", "Calculator"),
        ("com.android.settings", "Settings"),
    ]);
    (source, fake)
}

#[tokio::test(start_paused = true)]
async fn apps_lists_the_launchable_apps_with_their_names() {
    let (source, _fake) = installed();
    let (d, _home) = daemon(source);
    let r = ok(&d, "default", "apps", json!({})).await;
    let names: Vec<&str> = r
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["phonedriver testbed", "Calculator", "Settings"]);
}

#[tokio::test(start_paused = true)]
async fn launch_by_name_in_any_case_or_by_package_foregrounds_the_app_and_returns_a_full_snapshot()
{
    for app in [
        "phonedriver testbed",
        "PHONEDRIVER TESTBED",
        "dev.phonedriver.testbed",
    ] {
        let (source, fake) = installed();
        let (d, _home) = daemon(source);
        ok(&d, "default", "snapshot", json!({})).await;
        let r = ok(
            &d,
            "default",
            "act",
            act_with(json!({"kind": "launch", "app": app})),
        )
        .await;
        assert_eq!(r["observation"]["kind"], "full", "{app}: {r}");
        assert!(
            fake.calls()
                .contains(&Call::Launch(AppId("dev.phonedriver.testbed".into()))),
            "{app}: {:?}",
            fake.calls()
        );
    }
}

#[tokio::test(start_paused = true)]
async fn a_launch_that_is_the_first_call_on_a_polled_device_settles_without_waiting_out_its_window()
{
    let (source, fake) = FakeSource::polled(vec![list_tree(&["Calculator", "7", "8"])]);
    fake.install(&[("com.android.calculator2", "Calculator")]);
    let (d, _home) = daemon(source);
    let r = ok(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "launch", "app": "Calculator"})),
    )
    .await;
    assert!(r["settle_ms"].as_u64().unwrap() < 1500, "{r}");
}

#[tokio::test(start_paused = true)]
async fn a_launch_with_restart_stops_the_app_first_so_it_opens_on_its_first_screen() {
    let (source, fake) = installed();
    let (d, _home) = daemon(source);
    let r = ok(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "launch", "app": "Calculator", "restart": true})),
    )
    .await;
    assert!(
        r["summary"]
            .as_str()
            .unwrap()
            .starts_with("restarted Calculator"),
        "{r}"
    );
    let calls = fake.calls();
    let stop = calls
        .iter()
        .position(|c| matches!(c, Call::AppOp(phonedriver_core::device::AppOp::Stop, _)))
        .expect("a stop");
    let launch = calls
        .iter()
        .position(|c| matches!(c, Call::Launch(_)))
        .unwrap();
    assert!(stop < launch, "{calls:?}");
}

#[tokio::test(start_paused = true)]
async fn the_first_taps_after_a_launch_wait_for_a_cold_app_that_reacts_after_900_ms() {
    let (source, fake) = installed();
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    ok(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "launch", "app": "Calculator"})),
    )
    .await;
    let late = fake.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(900)).await;
        late.push_frame(big_frame(1));
    });
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(r["changed"], true, "{r}");
}

#[tokio::test(start_paused = true)]
async fn a_tap_whose_ripple_settles_before_the_next_page_draws_waits_for_the_page() {
    let (source, fake) =
        FakeSource::with(vec![tree_of("com.android.settings", &["Apps", "Battery"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let late = fake.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        late.push_frame(big_frame(1));
        tokio::time::sleep(Duration::from_millis(670)).await;
        late.set_tree(tree_of(
            "com.android.settings",
            &["App launch", "Default apps"],
        ));
        late.push_frame(big_frame(2));
    });
    let r = ok(&d, "default", "act", tap_point(10.0, 120.0)).await;
    let text = r["observation"]["text"].as_str().unwrap();
    assert!(text.contains("Default apps"), "{r}");
}

#[tokio::test(start_paused = true)]
async fn a_tap_long_after_a_launch_still_reports_no_change_after_500_ms() {
    let (source, fake) = installed();
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    ok(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "launch", "app": "Calculator"})),
    )
    .await;
    tokio::time::sleep(Duration::from_secs(60)).await;
    let late = fake.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(900)).await;
        late.push_frame(big_frame(1));
    });
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(r["changed"], false, "{r}");
}

#[tokio::test(start_paused = true)]
async fn a_launch_that_never_settles_still_answers_with_the_screen_lines_once_the_tree_has_them() {
    let hollow = || tree_of("com.android.calculator2", &[]);
    let (source, fake) = FakeSource::with(vec![
        hollow(),
        hollow(),
        hollow(),
        hollow(),
        tree_of("com.android.calculator2", &["Calculator", "7", "8"]),
    ]);
    fake.install(&[("com.android.calculator2", "Calculator")]);
    let (d, _home) = daemon(source);
    let busy = fake.clone();
    tokio::spawn(async move {
        for seq in 1..400 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            busy.push_frame(big_frame(seq));
        }
    });
    let r = ok(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "launch", "app": "Calculator"})),
    )
    .await;
    assert_eq!(r["settled"], false, "{r}");
    let text = r["observation"]["text"].as_str().unwrap();
    assert!(text.contains("@e"), "{r}");
    assert!(text.contains("\"Calculator\""), "{r}");
}

fn fake_png() -> phonedriver_core::device::Image {
    let (w, h) = (360usize, 800usize);
    let frame = phonedriver_video::frame_store::Frame::from_i420(
        w as u32,
        h as u32,
        vec![128; w * h],
        vec![128; w * h / 4],
        vec![128; w * h / 4],
    );
    phonedriver_core::device::Image {
        width: w as u32,
        height: h as u32,
        format: phonedriver_core::device::ImageFormat::Png,
        bytes: phonedriver_video::image::encode_png(&frame, 1.0).unwrap(),
    }
}

#[tokio::test(start_paused = true)]
async fn a_screenshot_with_marks_is_taken_after_the_tree_it_is_marked_with() {
    let (source, fake) =
        FakeSource::of(FakeDevice::new(vec![tree(&["One"])]).with_screenshot(fake_png()));
    let (d, _home) = daemon(source);
    ok(&d, "default", "screenshot", json!({"marks": true})).await;
    assert_eq!(fake.order(), ["tree", "screenshot"]);
}

#[tokio::test(start_paused = true)]
async fn a_screenshot_whose_stream_was_gone_reconnects_once_and_answers() {
    let (source, fake) =
        FakeSource::of(FakeDevice::new(vec![tree(&["One"])]).with_screenshot(fake_png()));
    fake.fail_screenshots(vec![Error::DeviceGone {
        id: "fake:1".into(),
    }]);
    let (d, _home) = daemon(source);
    let r = ok(&d, "default", "screenshot", json!({})).await;
    assert_eq!(r["width"], 360, "{r}");
    assert_eq!(fake.order(), ["screenshot", "screenshot"]);
}

#[tokio::test(start_paused = true)]
async fn a_screenshot_that_is_gone_twice_is_an_error() {
    let (source, fake) =
        FakeSource::of(FakeDevice::new(vec![tree(&["One"])]).with_screenshot(fake_png()));
    let gone = || Error::DeviceGone {
        id: "fake:1".into(),
    };
    fake.fail_screenshots(vec![gone(), gone()]);
    let (d, _home) = daemon(source);
    let e = err(&d, "default", "screenshot", json!({})).await;
    assert_eq!(e.code, -32003, "{e:?}");
}

fn tree_of(package: &str, labels: &[&str]) -> RawTree {
    let mut nodes = vec![
        r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}"#
            .to_string(),
    ];
    for (i, l) in labels.iter().enumerate() {
        let top = 100 + 100 * i as i32;
        nodes.push(format!(
            r#"{{"p":0,"w":1,"cls":"android.widget.Button","text":"{l}","b":[0,{top},360,{}],"f":"vVe","a":["tap"]}}"#,
            top + 60
        ));
    }
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"{package}","bounds":[0,0,360,800]}}],"nodes":[{}]}}"#,
        nodes.join(",")
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

#[tokio::test(start_paused = true)]
async fn opening_a_link_from_the_launcher_waits_for_another_app_to_come_to_the_front() {
    let home = || tree_of("com.launcher", &["Gmail", "Chrome"]);
    let (source, fake) = FakeSource::with(vec![
        home(),
        home(),
        home(),
        home(),
        tree_of("com.chrome", &["Stay signed out"]),
    ]);
    fake.launch(&AppId("com.launcher".into())).await.unwrap();
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "open", "url": "https://example.org"})),
    )
    .await;
    let text = r["observation"]["text"].as_str().unwrap();
    assert!(text.contains("com.chrome"), "{text}");
    assert!(text.contains("Stay signed out"), "{text}");
}

fn web_tree(loaded: bool) -> RawTree {
    let page = if loaded {
        r#",{"p":1,"w":1,"cls":"android.view.View","text":"Example Domain","b":[0,100,360,160],"f":"vVe"}"#
    } else {
        ""
    };
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.webkit.WebView","b":[0,100,360,700],"f":"vVe"}}{page}]}}"#
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

#[tokio::test(start_paused = true)]
async fn an_action_that_opens_a_cold_webview_waits_up_to_ten_seconds_for_its_page() {
    let mut trees = vec![tree(&["Web"])];
    trees.extend(vec![web_tree(false); 70]);
    trees.push(web_tree(true));
    let (source, fake) = FakeSource::with(trees);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        fake.push_frame(big_frame(1));
    });
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    let text = r["observation"]["text"].as_str().unwrap();
    assert!(text.contains("Example Domain"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn a_tap_the_pixels_barely_show_still_waits_for_the_web_view_it_opened() {
    let mut trees = vec![tree(&["Web"])];
    trees.extend(vec![web_tree(false); 20]);
    trees.push(web_tree(true));
    let (source, _fake) = FakeSource::with(trees);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    let text = r["observation"]["text"].as_str().unwrap();
    assert!(text.contains("Example Domain"), "{text}");
}

fn launch_settings() -> Value {
    act_with(json!({"kind": "launch", "app": "Settings"}))
}

#[tokio::test(start_paused = true)]
async fn launch_waits_for_a_webview_to_have_children_before_the_first_snapshot() {
    let mut trees = vec![web_tree(false); 6];
    trees.push(web_tree(true));
    let (source, fake) = FakeSource::with(trees);
    fake.install(&[("com.android.settings", "Settings")]);
    let (d, _home) = daemon(source);
    let r = ok(&d, "default", "act", launch_settings()).await;
    let text = r["observation"]["text"].as_str().unwrap();
    assert!(text.contains("Example Domain"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn launch_on_a_polled_device_waits_for_three_equal_node_counts_in_a_row() {
    let mut trees = vec![list_tree(&["Search"]); 4];
    trees.push(list_tree(&["Toolbar", "Search"]));
    let (source, fake) = FakeSource::polled(trees);
    fake.install(&[("com.android.settings", "Settings")]);
    let (d, _home) = daemon(source);
    let r = ok(&d, "default", "act", launch_settings()).await;
    let text = r["observation"]["text"].as_str().unwrap();
    assert!(text.contains("Toolbar"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn launching_a_misspelled_app_is_not_found_and_names_the_close_app() {
    let (source, fake) = installed();
    let (d, _home) = daemon(source);
    let e = err(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "launch", "app": "Calculatr"})),
    )
    .await;
    assert_eq!(e.code, -32017, "{e:?}");
    assert!(
        e.data["similar"].to_string().contains("Calculator"),
        "{e:?}"
    );
    assert!(fake.calls().is_empty());
}

#[tokio::test(start_paused = true)]
async fn launching_an_unknown_app_says_no_app_and_lists_apps_that_share_a_word() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"])]);
    fake.install(&[
        ("dev.phonedriver.testbed", "Testbed"),
        ("com.apple.Preferences", "Settings"),
    ]);
    let (d, _home) = daemon(source);
    let e = err(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "launch", "app": "testbed app"})),
    )
    .await;
    assert_eq!(e.code, -32017, "{e:?}");
    assert_eq!(e.message, "no app named \"testbed app\"");
    let hint = e.data["hint"].as_str().unwrap();
    assert!(hint.contains("Testbed (dev.phonedriver.testbed)"), "{hint}");
    assert!(!hint.contains("snapshot"), "{hint}");
}

#[tokio::test(start_paused = true)]
async fn launching_an_app_a_short_list_lacks_names_every_installed_app() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"])]);
    fake.install(&[
        ("com.apple.reminders", "Reminders"),
        ("com.apple.Preferences", "Settings"),
        ("com.apple.mobilesafari", "Safari"),
    ]);
    let (d, _home) = daemon(source);
    let e = err(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "launch", "app": "Notes"})),
    )
    .await;
    let hint = e.data["hint"].as_str().unwrap();
    assert!(hint.contains("Reminders, Safari, Settings"), "{hint}");
    assert!(!hint.contains("com.apple"), "{hint}");
}

fn chrome_tree(page: Option<&str>) -> RawTree {
    let content = match page {
        Some(text) => format!(
            r#"{{"p":1,"w":1,"cls":"android.webkit.WebView","b":[0,100,360,800],"f":"vVe"}},
            {{"p":2,"w":1,"cls":"android.widget.TextView","text":"{text}","b":[0,120,360,160],"f":"vVe"}}"#
        ),
        None => r#"{"p":1,"w":1,"cls":"android.widget.FrameLayout","desc":"Web View","b":[0,100,360,800],"f":"vVeF"}"#.to_string(),
    };
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.android.chrome","bounds":[0,0,360,800]}}],"nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {content}]}}"#
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

#[tokio::test(start_paused = true)]
async fn an_opened_page_is_in_the_reply_not_chromes_empty_web_view() {
    let mut trees: Vec<RawTree> = (0..12).map(|_| chrome_tree(None)).collect();
    trees.extend((0..4).map(|_| chrome_tree(Some("Android is a mobile operating system"))));
    let (source, _fake) = FakeSource::with(trees);
    let (d, _home) = daemon(source);
    let r = ok(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "open", "url": "https://en.wikipedia.org/wiki/Android"})),
    )
    .await;
    let text = r["observation"]["text"].as_str().unwrap();
    assert!(text.contains("Android is a mobile operating system"), "{r}");
}

#[tokio::test(start_paused = true)]
async fn open_hands_the_url_to_the_device() {
    let (source, fake) = installed();
    let (d, _home) = daemon(source);
    ok(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "open", "url": "https://example.com"})),
    )
    .await;
    assert_eq!(
        fake.calls(),
        vec![Call::OpenUrl(Url("https://example.com".into()), None)]
    );
}

#[tokio::test(start_paused = true)]
async fn open_with_an_app_hands_the_url_to_that_app() {
    let (source, fake) = installed();
    let (d, _home) = daemon(source);
    ok(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "open", "url": "https://example.com", "app": "Settings"})),
    )
    .await;
    assert_eq!(
        fake.calls(),
        vec![Call::OpenUrl(
            Url("https://example.com".into()),
            Some(AppId("com.android.settings".into()))
        )]
    );
}

#[tokio::test(start_paused = true)]
async fn clear_taps_the_field_sets_its_value_to_nothing_and_verifies_it_is_empty() {
    let (source, fake) = FakeSource::with(vec![
        field_tree("hello", false),
        field_tree("hello", false),
        field_tree("", false),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "clear", "target": {"ref": "e1"}}}),
    )
    .await;
    assert_eq!(r["verified"], true, "{r}");
    assert_eq!(r["via"], "set");
    let calls = fake.calls();
    assert!(matches!(calls[0], Call::Gesture(_)), "{calls:?}");
    assert_eq!(
        calls[1],
        Call::Text(phonedriver_core::device::TextInput::SetValue(String::new()))
    );
}

#[tokio::test(start_paused = true)]
async fn clear_reports_not_verified_when_the_field_still_holds_text() {
    let (source, _) = FakeSource::with(vec![field_tree("stuck", false)]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "clear", "target": {"ref": "e1"}}}),
    )
    .await;
    assert_eq!(r["verified"], false, "{r}");
}

#[tokio::test(start_paused = true)]
async fn clipboard_sets_and_reads_the_device_clipboard_and_the_trace_keeps_neither() {
    let (source, _) = FakeSource::with(vec![]);
    let (d, home) = daemon(source);
    let r = ok(&d, "default", "clipboard", json!({"set": "clip-secret-1"})).await;
    assert_eq!(r, json!({}));
    let r = ok(&d, "default", "clipboard", json!({})).await;
    assert_eq!(r["text"], "clip-secret-1");
    assert!(!trace_text(home.path()).contains("clip-secret-1"));
}

fn screen(on: bool, locked: bool, secure: bool) -> phonedriver_core::device::ScreenState {
    phonedriver_core::device::ScreenState {
        on,
        locked,
        secure,
        focused: true,
    }
}

#[tokio::test(start_paused = true)]
async fn a_dark_screen_is_woken_before_the_action_and_the_reply_says_so() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"])]);
    fake.set_screen(screen(false, false, false));
    let (d, _home) = daemon(source);
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(r["woke"], true, "{r}");
    assert!(r.get("unlocked").is_none(), "{r}");
    let calls = fake.calls();
    assert_eq!(calls[0], Call::Wake, "{calls:?}");
    assert!(matches!(calls[1], Call::Gesture(_)), "{calls:?}");
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert!(r.get("woke").is_none(), "an awake screen is not woken: {r}");
}

#[tokio::test(start_paused = true)]
async fn the_power_state_is_read_once_for_taps_in_quick_succession() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    for _ in 0..3 {
        ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    }
    assert_eq!(fake.screen_reads(), 1);
}

#[tokio::test(start_paused = true)]
async fn the_power_state_is_read_again_after_a_window_event_a_pause_or_a_lock_press() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    fake.emit(phonedriver_core::device::DeviceEvent::WindowChanged { package: None });
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(fake.screen_reads(), 2);
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(fake.screen_reads(), 3);
    ok(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "press", "key": "lock"}}),
    )
    .await;
    ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(fake.screen_reads(), 4);
}

#[tokio::test(start_paused = true)]
async fn a_swipe_only_keyguard_is_swiped_away_before_the_action() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"])]);
    fake.set_screen(screen(true, true, false));
    let (d, _home) = daemon(source);
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(r["unlocked"], true, "{r}");
    let calls = fake.calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    let Call::Gesture(swipe) = &calls[0] else {
        panic!("{calls:?}")
    };
    let path = &swipe.fingers[0];
    assert!(path.last().unwrap().at.y < path[0].at.y, "{swipe:?}");
    assert!(matches!(calls[1], Call::Gesture(_)));
}

#[tokio::test(start_paused = true)]
async fn a_pin_keyguard_fails_needs_user_unlock_and_sends_no_input() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"])]);
    fake.set_screen(screen(false, true, true));
    let (d, _home) = daemon(source);
    let e = err(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(e.code, -32005, "{e:?}");
    assert_eq!(e.data["action"]["kind"], "unlock_device", "{e:?}");
    assert_eq!(fake.calls(), vec![Call::Wake]);
}

#[tokio::test(start_paused = true)]
async fn pressing_lock_does_not_wake_the_screen_first() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"])]);
    fake.set_screen(screen(false, false, false));
    let (d, _home) = daemon(source);
    ok(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "press", "key": "lock"}}),
    )
    .await;
    assert_eq!(
        fake.calls(),
        vec![Call::Button(phonedriver_core::device::Button::Lock)]
    );
}

fn keyboard_tree() -> RawTree {
    let json = r#"{"windows":[
        {"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]},
        {"id":2,"type":"input_method","package":"com.google.ime","bounds":[0,500,360,800]}],
      "nodes":[
        {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.widget.EditText","desc":"Field","b":[0,100,360,160],"f":"vVeEF","a":["tap","set_text"]},
        {"p":0,"w":1,"cls":"android.widget.Button","text":"Send","b":[0,600,360,660],"f":"vVe","a":["tap"]},
        {"p":0,"w":1,"cls":"android.widget.Button","text":"Cancel","b":[0,300,360,360],"f":"vVe","a":["tap"]},
        {"p":-1,"w":2,"cls":"android.inputmethodservice.SoftInputWindow","b":[0,500,360,800],"f":"vVe"}]}"#;
    RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap()
}

fn half_covered_tree() -> RawTree {
    let json = r#"{"windows":[
        {"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]},
        {"id":2,"type":"input_method","package":"com.google.ime","bounds":[0,500,360,800]}],
      "nodes":[
        {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,80,360,800],"f":"vVeS","a":["scroll_down","scroll_up"]},
        {"p":1,"w":1,"cls":"android.widget.EditText","desc":"Last name","b":[0,100,360,160],"f":"vVeEF","a":["tap","set_text"]},
        {"p":1,"w":1,"cls":"android.widget.Button","text":"add phone","b":[0,470,360,530],"f":"vVe","a":["tap"]},
        {"p":1,"w":1,"cls":"android.widget.Button","text":"add email","b":[0,495,360,555],"f":"vVe","a":["tap"]},
        {"p":-1,"w":2,"cls":"android.inputmethodservice.SoftInputWindow","b":[0,500,360,800],"f":"vVe"}]}"#;
    RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap()
}

#[tokio::test(start_paused = true)]
async fn a_row_the_keyboard_half_covers_is_tapped_above_the_keyboard_and_a_sliver_is_under_it() {
    let (source, fake) = FakeSource::with(vec![half_covered_tree()]);
    let (d, _home) = daemon(source);
    let snap = ok(&d, "default", "snapshot", json!({})).await;
    let text = snap["text"].as_str().unwrap();
    let phone = text.lines().find(|l| l.contains("add phone")).unwrap();
    assert!(!phone.contains("under-keyboard"), "{phone}");
    let email = text.lines().find(|l| l.contains("add email")).unwrap();
    assert!(email.ends_with("under-keyboard"), "{email}");
    ok(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "tap", "target": {"text": "add phone"}}, "settle": false}),
    )
    .await;
    let Some(Call::Gesture(g)) = fake.calls().into_iter().next() else {
        panic!("{:?}", fake.calls())
    };
    let y = g.fingers[0][0].at.y;
    assert!(
        (470.0..500.0).contains(&y),
        "the tap must land above the keyboard: {y}"
    );
    let e = err(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "tap", "target": {"text": "add email"}}}),
    )
    .await;
    assert_eq!(e.data["behind_keyboard"], true, "{e:?}");
    assert_eq!(
        fake.calls()[1..],
        [Call::Button(phonedriver_core::device::Button::Back)],
        "a keyboard that stays is not tapped through"
    );
}

fn form_tree(keyboard: bool, phone_y: Option<f32>) -> RawTree {
    let ime = if keyboard {
        r#",{"id":2,"type":"input_method","package":"com.google.ime","bounds":[0,500,360,800]}"#
    } else {
        ""
    };
    let phone = phone_y.map_or(String::new(), |y| {
        format!(
            r#",{{"p":1,"w":1,"cls":"android.widget.EditText","desc":"Phone (Mobile)","b":[0,{y},360,{}],"f":"vVeE","a":["tap","set_text"]}}"#,
            y + 60.0
        )
    });
    let keys = if keyboard {
        r#",{"p":-1,"w":2,"cls":"android.inputmethodservice.SoftInputWindow","b":[0,500,360,800],"f":"vVe"}"#
    } else {
        ""
    };
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}{ime}],
      "nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,80,360,800],"f":"vVeS","a":["scroll_down","scroll_up"]}},
        {{"p":1,"w":1,"cls":"android.widget.EditText","desc":"First name","b":[0,100,360,160],"f":"vVeE{}","a":["tap","set_text"]}}{phone}{keys}]}}"#,
        if keyboard { "F" } else { "" }
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

#[tokio::test(start_paused = true)]
async fn type_into_a_field_the_keyboard_pushed_out_of_the_list_scrolls_it_back_and_types_there() {
    let (source, fake) = FakeSource::with(vec![form_tree(false, Some(600.0))]);
    let (d, _home) = daemon(source);
    let snap = ok(&d, "default", "snapshot", json!({})).await;
    let phone = snap["text"]
        .as_str()
        .unwrap()
        .lines()
        .find(|l| l.contains("\"Phone (Mobile)\""))
        .and_then(|l| l.trim_start().split(' ').next())
        .unwrap()
        .trim_start_matches('@')
        .to_string();
    fake.set_tree(form_tree(true, None));
    ok(&d, "default", "snapshot", json!({})).await;
    fake.on_gesture(|g| {
        let path = &g.fingers[0];
        let moved = path.first()?.at.y - path.last()?.at.y;
        (moved > 20.0).then(|| vec![form_tree(true, Some(300.0))])
    });
    let r = ok(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "type", "text": "555 0199", "into": {"ref": phone}}}),
    )
    .await;
    let summary = r["summary"].as_str().unwrap();
    assert!(summary.contains("Phone (Mobile)"), "{r}");
    assert!(summary.contains("scrolled"), "{r}");
    let calls = fake.calls();
    let drag = calls
        .iter()
        .position(|c| matches!(c, Call::Gesture(g) if g.fingers[0].len() > 2))
        .unwrap_or_else(|| panic!("no drag: {calls:?}"));
    let text = calls
        .iter()
        .position(|c| matches!(c, Call::Text(_)))
        .unwrap_or_else(|| panic!("nothing typed: {calls:?}"));
    assert!(drag < text, "{calls:?}");
    let text = r["observation"]["text"].as_str().unwrap();
    assert!(
        text.contains("Phone (Mobile)"),
        "the agent sees the field: {text}"
    );
}

#[tokio::test(start_paused = true)]
async fn type_into_the_text_of_a_field_the_keyboard_pushed_out_scrolls_it_back_too() {
    let (source, fake) = FakeSource::with(vec![form_tree(false, Some(600.0))]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    fake.set_tree(form_tree(true, None));
    ok(&d, "default", "snapshot", json!({})).await;
    fake.on_gesture(|g| {
        let path = &g.fingers[0];
        let moved = path.first()?.at.y - path.last()?.at.y;
        (moved > 20.0).then(|| vec![form_tree(true, Some(300.0))])
    });
    let r = ok(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "type", "text": "555 0199", "into": {"text": "phone (mobile)"}}}),
    )
    .await;
    assert!(
        r["summary"]
            .as_str()
            .unwrap()
            .starts_with("scrolled down 1 time to"),
        "{r}"
    );
    let e = err(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "type", "text": "x", "into": {"text": "Fax"}}}),
    )
    .await;
    assert_eq!(e.data["code_name"], "not_found", "{e:?}");
}

#[tokio::test(start_paused = true)]
async fn a_field_that_renames_itself_once_typed_into_is_named_by_its_new_ref() {
    let field = |label: &str, value: &str, focused: bool| {
        let json = format!(
            r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[
            {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
            {{"p":0,"w":1,"cls":"android.widget.EditText","desc":"{label}","text":"{value}","b":[0,100,360,160],"f":"vVeE{}","a":["tap","set_text"]}}]}}"#,
            if focused { "F" } else { "" }
        );
        RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
    };
    let (source, fake) = FakeSource::with(vec![field("Phone (Mobile)", "", false)]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let after = field("Phone", "555", true);
    fake.on_gesture(move |_| Some(vec![after.clone()]));
    let r = ok(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "type", "text": "555", "into": {"ref": "e1"}}}),
    )
    .await;
    assert_eq!(r["verified"], true, "{r}");
    let text = r["observation"]["text"].as_str().unwrap();
    let new = text
        .lines()
        .find(|l| l.contains("textbox \"Phone\""))
        .and_then(|l| l.split_whitespace().next())
        .unwrap_or_else(|| panic!("{text}"));
    assert!(
        r["summary"]
            .as_str()
            .unwrap()
            .ends_with(&format!("into {new}")),
        "{r}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_scroll_with_the_keyboard_up_drags_above_the_keyboard() {
    let (source, fake) = FakeSource::with(vec![half_covered_tree()]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    ok(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "scroll", "direction": "down"}, "settle": false}),
    )
    .await;
    let Some(Call::Gesture(g)) = fake.calls().into_iter().next() else {
        panic!("{:?}", fake.calls())
    };
    let ys: Vec<f32> = g.fingers[0].iter().map(|p| p.at.y).collect();
    assert!(ys.iter().all(|y| *y < 500.0), "{ys:?}");
}

#[tokio::test(start_paused = true)]
async fn a_tap_on_a_disabled_button_fails_at_once_and_a_point_tap_still_goes() {
    let json = r#"{"windows":[{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}],"nodes":[
        {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.widget.Button","text":"Add","b":[300,40,360,90],"f":"vV","a":["tap"]}]}"#;
    let disabled = RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap();
    let (source, fake) = FakeSource::with(vec![disabled]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let started = tokio::time::Instant::now();
    let e = err(&d, "default", "act", tap_text("Add")).await;
    assert_eq!(e.data["code_name"], "disabled", "{e:?}");
    assert!(e.message.contains("disabled"), "{e:?}");
    assert_eq!(started.elapsed(), Duration::ZERO);
    assert!(fake.calls().is_empty(), "{:?}", fake.calls());
    ok(&d, "default", "act", json!({"action": {"kind": "tap", "target": {"point": {"x": 330.0, "y": 65.0}}}, "settle": false})).await;
    assert_eq!(fake.calls().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_tap_on_a_button_under_the_keyboard_hides_the_keyboard_and_then_taps_it() {
    let (source, fake) = FakeSource::with(vec![keyboard_tree()]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let watch = fake.clone();
    tokio::spawn(async move {
        loop {
            if watch
                .calls()
                .contains(&Call::Button(phonedriver_core::device::Button::Back))
            {
                watch.set_tree(tree(&["Field", "Cancel", "Send"]));
                watch.push_frame(big_frame(1));
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    });
    let r = ok(&d, "default", "act", tap_text("Send")).await;
    assert!(
        r["summary"]
            .as_str()
            .unwrap()
            .starts_with("hid the keyboard, then tapped"),
        "{r}"
    );
    let taps: Vec<f32> = fake
        .calls()
        .iter()
        .filter_map(|c| match c {
            Call::Gesture(g) => Some(g.fingers[0][0].at.y),
            _ => None,
        })
        .collect();
    assert_eq!(
        taps,
        [330.0],
        "the tap goes where Send is once the keyboard is gone"
    );
}

#[tokio::test(start_paused = true)]
async fn a_button_under_the_keyboard_is_marked_and_a_tap_on_it_fails_off_screen_with_a_way_out() {
    let (source, fake) = FakeSource::with(vec![keyboard_tree()]);
    let (d, _home) = daemon(source);
    let snap = ok(&d, "default", "snapshot", json!({})).await;
    let text = snap["text"].as_str().unwrap();
    assert!(text.contains("keyboard shown"), "{text}");
    let send = text.lines().find(|l| l.contains("\"Send\"")).unwrap();
    assert!(send.ends_with("under-keyboard"), "{send}");
    let cancel = text.lines().find(|l| l.contains("\"Cancel\"")).unwrap();
    assert!(!cancel.contains("under-keyboard"), "{cancel}");

    let e = err(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "tap", "target": {"text": "Send"}}}),
    )
    .await;
    assert_eq!(e.code, -32009, "{e:?}");
    assert_eq!(e.data["behind_keyboard"], true, "{e:?}");
    assert!(e.message.contains("under the keyboard"), "{e:?}");
    let hint = e.data["hint"].as_str().unwrap();
    assert!(
        hint.contains("press hide-keyboard") && hint.contains("scroll"),
        "{hint}"
    );
    assert_eq!(
        fake.calls(),
        [Call::Button(phonedriver_core::device::Button::Back)],
        "it tried to hide the keyboard, which stayed, and did not tap"
    );

    ok(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "tap", "target": {"text": "Cancel"}}}),
    )
    .await;
    assert_eq!(fake.calls().len(), 2);
}

async fn wait_with_frames(
    d: &Daemon,
    fake: &FakeDevice,
    params: Value,
    frames_at_ms: &[u64],
) -> Result<Value, RpcError> {
    let driver = async {
        let mut last = 0;
        for (i, at) in frames_at_ms.iter().enumerate() {
            tokio::time::sleep(Duration::from_millis(at - last)).await;
            last = *at;
            fake.push_frame(big_frame(i as u64 + 1));
        }
    };
    let (r, ()) = tokio::join!(d.call("default", "wait", params), driver);
    r
}

#[tokio::test(start_paused = true)]
async fn wait_for_text_reads_the_tree_once_per_frame_that_changed_and_returns_when_it_appears() {
    let (source, fake) = FakeSource::with(vec![
        tree(&["One"]),
        tree(&["One"]),
        tree(&["One"]),
        tree(&["One", "Two"]),
    ]);
    let (d, _home) = daemon(source);
    let r = wait_with_frames(
        &d,
        &fake,
        json!({"condition": {"appears": {"text": "Two"}}, "timeout_ms": 5000}),
        &[400, 800, 1200],
    )
    .await
    .unwrap();
    assert!(
        r["summary"]
            .as_str()
            .unwrap()
            .contains("after 4 tree reads"),
        "{r}"
    );
    assert!(
        r["observation"]["text"]
            .as_str()
            .unwrap()
            .contains("\"Two\""),
        "{r}"
    );
}

#[tokio::test(start_paused = true)]
async fn wait_for_a_text_already_on_screen_returns_after_one_read() {
    let (source, _) = FakeSource::with(vec![tree(&["Row 640"])]);
    let (d, _home) = daemon(source);
    let r = ok(
        &d,
        "default",
        "wait",
        json!({"condition": {"appears": {"text": "Row 640"}}}),
    )
    .await;
    assert!(
        r["summary"].as_str().unwrap().contains("after 1 tree read"),
        "{r}"
    );
}

#[tokio::test(start_paused = true)]
async fn wait_gone_returns_when_the_text_leaves() {
    let (source, fake) = FakeSource::with(vec![tree(&["Loading", "Done"]), tree(&["Done"])]);
    let (d, _home) = daemon(source);
    let r = wait_with_frames(
        &d,
        &fake,
        json!({"condition": {"gone": {"text": "Loading"}}, "timeout_ms": 5000}),
        &[1000],
    )
    .await
    .unwrap();
    assert!(r["summary"].as_str().unwrap().contains("gone"), "{r}");
    assert!(
        !r["observation"]["text"]
            .as_str()
            .unwrap()
            .contains("Loading"),
        "{r}"
    );
}

#[tokio::test(start_paused = true)]
async fn wait_for_text_that_never_shows_times_out_naming_the_condition_and_reads_once() {
    let (source, _) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    let started = tokio::time::Instant::now();
    let e = err(
        &d,
        "default",
        "wait",
        json!({"condition": {"appears": {"text": "Never"}}, "timeout_ms": 500}),
    )
    .await;
    assert_eq!(e.code, -32013, "{e:?}");
    assert!(e.message.contains("\"Never\""), "{e:?}");
    assert!(started.elapsed() < Duration::from_millis(700));
}

#[tokio::test(start_paused = true)]
async fn snapshot_within_a_ref_renders_only_that_subtree_and_a_missing_ref_is_stale() {
    let (source, _) = FakeSource::with(vec![tree(&["One", "Two"])]);
    let (d, _home) = daemon(source);
    let all = ok(&d, "default", "snapshot", json!({})).await;
    assert!(all["text"].as_str().unwrap().contains("\"Two\""));
    let one = ok(&d, "default", "snapshot", json!({"within": "e1"})).await;
    let text = one["text"].as_str().unwrap();
    assert!(
        text.contains("\"One\"") && !text.contains("\"Two\""),
        "{text}"
    );
    let e = err(&d, "default", "snapshot", json!({"within": "e99"})).await;
    assert_eq!(e.code, -32006, "{e:?}");
}

#[tokio::test(start_paused = true)]
async fn a_text_shared_by_an_inert_label_and_one_field_resolves_to_the_field() {
    let json = r#"{"windows":[{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}],"nodes":[
        {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.view.View","desc":"Name","b":[0,100,360,130],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.widget.EditText","desc":"Name","b":[0,140,360,200],"f":"vVeE","a":["tap","set_text"]},
        {"p":0,"w":1,"cls":"android.widget.Button","text":"Same","b":[0,300,360,360],"f":"vVe","a":["tap"]},
        {"p":0,"w":1,"cls":"android.widget.Button","text":"Same","b":[0,400,360,460],"f":"vVe","a":["tap"]}]}"#;
    let tree = RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap();
    let (source, fake) = FakeSource::with(vec![tree]);
    let (d, _home) = daemon(source);
    ok(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "tap", "target": {"text": "Name"}}}),
    )
    .await;
    let Call::Gesture(g) = &fake.calls()[0] else {
        panic!()
    };
    assert_eq!(
        g.fingers[0][0].at.y, 170.0,
        "the tap goes to the field, not the label"
    );
    let e = err(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "tap", "target": {"text": "Same"}}}),
    )
    .await;
    assert_eq!(e.code, -32008, "two buttons are still ambiguous: {e:?}");
}

#[tokio::test(start_paused = true)]
async fn press_back_on_a_device_with_one_way_back_presses_the_button_and_has_no_via() {
    let (source, fake) = FakeSource::with(vec![tree(&["A"])]);
    let (d, _home) = daemon(source);
    let r = ok(
        &d,
        "s",
        "act",
        json!({"action": {"kind": "press", "key": "back"}, "settle": false}),
    )
    .await;
    assert!(r.get("via").is_none(), "{r}");
    assert!(
        fake.calls()
            .contains(&Call::Button(phonedriver_core::device::Button::Back))
    );
}

/// A device that polls for frames (no `FRAMES`) and whose gestures take 400 ms to be confirmed, like the iOS runner.
struct SlowPolled {
    inner: Arc<FakeDevice>,
    info: phonedriver_core::device::DeviceInfo,
    after: RawTree,
}

#[async_trait]
impl Device for SlowPolled {
    fn info(&self) -> &phonedriver_core::device::DeviceInfo {
        &self.info
    }
    async fn tree(&self) -> Result<RawTree, Error> {
        let moved = self
            .inner
            .frames()
            .is_some_and(|f| f.borrow().changed_area_pt2 > 0.0);
        if moved {
            Ok(self.after.clone())
        } else {
            self.inner.tree().await
        }
    }
    fn frames(&self) -> Option<tokio::sync::watch::Receiver<FrameRef>> {
        self.inner.frames()
    }
    async fn screenshot(
        &self,
        q: phonedriver_core::device::Quality,
    ) -> Result<phonedriver_core::device::Image, Error> {
        self.inner.screenshot(q).await
    }
    fn events(&self) -> tokio::sync::broadcast::Receiver<DeviceEvent> {
        self.inner.events()
    }
    async fn gesture(&self, g: phonedriver_core::device::Gesture) -> Result<(), Error> {
        tokio::time::sleep(Duration::from_millis(400)).await;
        self.inner.gesture(g).await
    }
    async fn key(&self, k: phonedriver_core::device::Key) -> Result<(), Error> {
        self.inner.key(k).await
    }
    async fn button(&self, b: phonedriver_core::device::Button) -> Result<(), Error> {
        self.inner.button(b).await
    }
    async fn open_url(
        &self,
        u: &phonedriver_core::device::Url,
        app: Option<&AppId>,
    ) -> Result<(), Error> {
        self.inner.open_url(u, app).await
    }
    async fn text(&self, t: phonedriver_core::device::TextInput) -> Result<(), Error> {
        self.inner.text(t).await
    }
    async fn clipboard(
        &self,
        op: phonedriver_core::device::ClipboardOp,
    ) -> Result<Option<String>, Error> {
        self.inner.clipboard(op).await
    }
    async fn apps(&self) -> Result<Vec<phonedriver_core::device::App>, Error> {
        self.inner.apps().await
    }
    async fn launch(&self, app: &phonedriver_core::device::AppId) -> Result<(), Error> {
        self.inner.launch(app).await
    }
    async fn foreground(&self) -> Result<Option<phonedriver_core::device::AppId>, Error> {
        self.inner.foreground().await
    }
}

struct OneDevice(Arc<dyn Device>);

#[async_trait]
impl DeviceSource for OneDevice {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(vec![Listed {
            id: "fake:1".into(),
            model: "Fake".into(),
            os: None,
            tier: "polled".into(),
            capabilities: self.0.info().capabilities,
        }])
    }
    async fn connect(&self, _id: &str) -> Result<Arc<dyn Device>, Error> {
        Ok(self.0.clone())
    }
}

#[tokio::test(start_paused = true)]
async fn on_a_polled_device_the_first_change_window_starts_when_the_input_is_delivered() {
    let fake = Arc::new(FakeDevice::new(vec![tree(&["One", "Two", "Three"])]));
    let mut info = fake.info().clone();
    info.capabilities -= Capabilities::FRAMES;
    let slow = Arc::new(SlowPolled {
        inner: fake.clone(),
        info,
        after: tree(&["One", "Two", "Three!"]),
    });
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::new(
        Arc::new(OneDevice(slow)),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    ok(&d, "default", "snapshot", json!({})).await;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(650)).await;
        fake.push_frame(big_frame(1));
    });
    let r = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    assert_eq!(r["changed"], true, "{r}");
    assert!(
        r["observation"]["text"]
            .as_str()
            .unwrap()
            .contains("Three!"),
        "{r}"
    );
}

#[tokio::test(start_paused = true)]
async fn health_reads_the_tree_and_times_the_next_frame() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    let probe = {
        let d = d.clone();
        tokio::spawn(async move { ok(&d, "doctor", "health", json!({})).await })
    };
    tokio::time::sleep(Duration::from_millis(150)).await;
    fake.push_frame(big_frame(1));
    let r = probe.await.unwrap();
    assert_eq!(r["id"], "fake:1", "{r}");
    assert_eq!(r["model"], "Fake");
    assert_eq!(r["os"], "FakeOS 1");
    let ms = r["frames_ms"].as_u64().expect("a frame arrived");
    assert!(ms < 1000, "{r}");
}

#[tokio::test(start_paused = true)]
async fn health_says_null_when_no_frame_arrives_within_a_second() {
    let (source, _fake) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    let r = ok(&d, "doctor", "health", json!({})).await;
    assert!(r["frames_ms"].is_null(), "{r}");
    assert_eq!(r["id"], "fake:1");
}

fn ime_tree(shown: bool) -> RawTree {
    let keyboard_window = if shown {
        r#",{"id":2,"type":"input_method","bounds":[0,500,360,800]}"#
    } else {
        ""
    };
    let keyboard_node = if shown {
        r#",{"p":-1,"w":2,"cls":"android.widget.FrameLayout","b":[0,500,360,800],"f":"vVe"}"#
    } else {
        ""
    };
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}{keyboard_window}],"nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.widget.EditText","text":"","desc":"Field","b":[0,100,360,160],"f":"vVeEF","a":["tap","set_text"]}}{keyboard_node}]}}"#
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

fn covered_tree() -> RawTree {
    let json = r#"{"windows":[{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}],"nodes":[
        {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,90,360,800],"f":"vVeS","a":["scroll_down"]},
        {"p":1,"w":1,"cls":"android.widget.Button","text":"Developer","b":[0,760,360,820],"f":"vVe","a":["tap"]},
        {"p":1,"w":1,"cls":"android.widget.Button","text":"Privacy","b":[0,100,360,160],"f":"vVe","a":["tap"]},
        {"p":0,"w":1,"cls":"androidx.appcompat.widget.Toolbar","desc":"Toolbar","b":[0,700,360,800],"f":"vVe"},
        {"p":4,"w":1,"cls":"android.widget.EditText","text":"","desc":"Search","b":[10,720,350,780],"f":"vVeE","a":["tap","set_text"]},
        {"p":0,"w":1,"cls":"android.widget.Button","text":"Back","b":[10,30,60,74],"f":"vVe","a":["tap"]},
        {"p":0,"w":1,"cls":"androidx.appcompat.widget.Toolbar","desc":"Toolbar","b":[0,50,360,98],"f":"vVe"}]}"#;
    RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap()
}

#[tokio::test(start_paused = true)]
async fn tapping_a_row_whose_center_is_under_a_floating_toolbar_is_off_screen_with_a_scroll_hint() {
    let (source, fake) = FakeSource::with(vec![covered_tree()]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let e = err(&d, "default", "act", tap_text("Developer")).await;
    assert_eq!(e.code, -32009, "{e:?}");
    assert!(e.message.contains("under the Toolbar"), "{}", e.message);
    assert!(
        e.data["hint"].as_str().unwrap().contains("scroll down"),
        "{}",
        e.data["hint"]
    );
    assert!(fake.calls().is_empty(), "{:?}", fake.calls());
    ok(&d, "default", "act", tap_text("Privacy")).await;
    ok(&d, "default", "act", tap_text("Search")).await;
    assert_eq!(
        fake.calls().len(),
        2,
        "an element inside the toolbar is tappable"
    );
    ok(&d, "default", "act", tap_text("Back")).await;
    assert_eq!(
        fake.calls().len(),
        3,
        "a bar button outside any list is never under a toolbar (iOS 26 contact card)"
    );
}

fn toolbar_list(row_top: i32, label: &str) -> RawTree {
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,100,360,800],"f":"vVeS","a":["scroll_down"]}},
        {{"p":1,"w":1,"cls":"android.widget.Button","text":"{label}","b":[0,{row_top},360,{}],"f":"vVe","a":["tap"]}},
        {{"p":0,"w":1,"cls":"androidx.appcompat.widget.Toolbar","desc":"Toolbar","b":[0,700,360,800],"f":"vVe"}},
        {{"p":3,"w":1,"cls":"android.widget.Button","text":"Dictate","b":[10,720,60,780],"f":"vVe","a":["tap"]}}]}}"#,
        row_top + 40
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

#[tokio::test(start_paused = true)]
async fn scroll_until_keeps_going_while_the_target_is_under_the_toolbar() {
    let (source, fake) = FakeSource::with(vec![
        toolbar_list(750, "Developer"),
        toolbar_list(400, "Developer"),
    ]);
    let (d, _home) = daemon(source);
    let r = ok(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Developer"}})),
    )
    .await;
    assert!(
        r["summary"].as_str().unwrap().contains("down 1 time to"),
        "{}",
        r["summary"]
    );
    assert_eq!(fake.calls().len(), 1);
}

fn tap_text(text: &str) -> Value {
    json!({"action": {"kind": "tap", "target": {"text": text}}})
}

fn press_key(key: &str) -> Value {
    json!({"action": {"kind": "press", "key": key}})
}

#[tokio::test(start_paused = true)]
async fn press_hide_keyboard_hides_a_shown_keyboard_and_says_how() {
    let (source, fake) = FakeSource::with(vec![ime_tree(true), ime_tree(false)]);
    let (d, _home) = daemon(source);
    let r = ok(&d, "default", "act", press_key("hide-keyboard")).await;
    assert!(
        r["summary"].as_str().unwrap().contains("hid the keyboard"),
        "{r}"
    );
    assert!(
        fake.calls()
            .contains(&Call::Button(phonedriver_core::device::Button::Back))
    );
    assert!(!r["observation"]["header"]["keyboard"].is_object(), "{r}");
}

#[tokio::test(start_paused = true)]
async fn hiding_the_keyboard_says_so_in_one_line_instead_of_listing_what_it_covered() {
    let hidden = {
        let json = r#"{"windows":[
            {"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}],
          "nodes":[
            {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.EditText","desc":"Field","b":[0,100,360,160],"f":"vVeEF","a":["tap","set_text"]},
            {"p":0,"w":1,"cls":"android.widget.Button","text":"Send","b":[0,600,360,660],"f":"vVe","a":["tap"]},
            {"p":0,"w":1,"cls":"android.widget.Button","text":"Cancel","b":[0,300,360,360],"f":"vVe","a":["tap"]}]}"#;
        RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap()
    };
    let (source, _fake) = FakeSource::with(vec![keyboard_tree(), keyboard_tree(), hidden]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", press_key("hide-keyboard")).await;
    let text = r["observation"]["text"].as_str().unwrap();
    assert!(
        text.lines().any(|l| l == "keyboard hidden") && !text.contains("under-keyboard"),
        "{text}"
    );
}

fn dialog_tree() -> RawTree {
    let json = r#"{"windows":[{"id":2,"type":"application","package":"com.example","bounds":[0,300,360,500]}],"nodes":[
        {"p":-1,"w":2,"cls":"android.widget.FrameLayout","b":[0,300,360,500],"f":"vVe"},
        {"p":0,"w":2,"cls":"android.widget.TextView","text":"Force stop?","b":[0,300,360,340],"f":"vVe"},
        {"p":0,"w":2,"cls":"android.widget.Button","text":"Cancel","b":[0,400,180,460],"f":"vVe","a":["tap"]},
        {"p":0,"w":2,"cls":"android.widget.Button","text":"OK","b":[180,400,360,460],"f":"vVe","a":["tap"]}]}"#;
    RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap()
}

#[tokio::test(start_paused = true)]
async fn a_dialog_that_closes_is_told_as_the_screen_being_back_not_as_a_new_screen() {
    let screen = || {
        tree(&[
            "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight",
        ])
    };
    let mut trees = vec![screen()];
    trees.extend((0..4).map(|_| dialog_tree()));
    trees.extend((0..4).map(|_| screen()));
    let (source, fake) = FakeSource::with(trees);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let pusher = tokio::spawn({
        let fake = fake.clone();
        async move {
            for seq in 1..=2 {
                tokio::time::sleep(Duration::from_millis(100)).await;
                fake.push_frame(big_frame(seq));
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    });
    let open = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    let shown = open["observation"]["text"].as_str().unwrap();
    assert!(shown.contains("Force stop?"), "{shown}");
    let close = ok(&d, "default", "act", tap_point(10.0, 10.0)).await;
    pusher.abort();
    let text = close["observation"]["text"].as_str().unwrap();
    assert_eq!(close["observation"]["kind"], "diff", "{close}");
    assert_eq!(
        text, "the screen before the last one is back, unchanged",
        "{text}"
    );
}

#[tokio::test(start_paused = true)]
async fn press_hide_keyboard_with_no_keyboard_sends_nothing_because_back_would_leave_the_screen() {
    let (source, fake) = FakeSource::with(vec![ime_tree(false)]);
    let (d, _home) = daemon(source);
    let r = ok(&d, "default", "act", press_key("hide-keyboard")).await;
    assert!(
        r["summary"].as_str().unwrap().contains("already hidden"),
        "{r}"
    );
    assert!(fake.calls().is_empty(), "{:?}", fake.calls());
}

#[tokio::test(start_paused = true)]
async fn press_hide_keyboard_fails_with_a_way_forward_when_the_keyboard_stays() {
    let (source, _fake) = FakeSource::with(vec![ime_tree(true)]);
    let (d, _home) = daemon(source);
    let e = err(&d, "default", "act", press_key("hide-keyboard")).await;
    assert_eq!(e.code, -32011, "{e:?}");
    assert!(e.message.contains("kept the keyboard"), "{}", e.message);
    assert!(e.data["hint"].as_str().unwrap().contains("press enter"));
}

fn batch(steps: Value) -> Value {
    json!({"steps": steps})
}

#[tokio::test(start_paused = true)]
async fn a_batch_runs_its_steps_in_order_and_answers_with_one_diff_from_where_the_agent_last_looked()
 {
    let (source, fake) = FakeSource::with(vec![
        tree(&["One", "Two", "Three"]),
        tree(&["One", "Two", "Three"]),
        tree(&["One", "Two", "Three!"]),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(
        &d,
        "default",
        "batch",
        batch(json!([
            {"kind": "tap", "target": {"point": {"x": 10.0, "y": 10.0}}},
            {"kind": "tap", "target": {"point": {"x": 20.0, "y": 20.0}}},
        ])),
    )
    .await;
    assert_eq!(r["ok"], true, "{r}");
    let steps = r["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2);
    assert!(
        steps[0]["summary"]
            .as_str()
            .unwrap()
            .starts_with("tapped at 10,10")
    );
    assert!(
        steps[1]["summary"]
            .as_str()
            .unwrap()
            .starts_with("tapped at 20,20")
    );
    assert_eq!(r["observation"]["kind"], "diff");
    assert_eq!(
        r["observation"]["text"],
        "- @e3
+ @e4 button \"Three!\""
    );
    let taps = fake
        .calls()
        .iter()
        .filter(|c| matches!(c, Call::Gesture(_)))
        .count();
    assert_eq!(taps, 2);
}

#[tokio::test(start_paused = true)]
async fn a_batch_stops_at_the_first_failing_step_and_says_which_one_failed_and_what_is_on_screen() {
    let (source, fake) = FakeSource::with(vec![tree(&["One", "Two"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(
        &d,
        "default",
        "batch",
        batch(json!([
            {"kind": "tap", "target": {"point": {"x": 10.0, "y": 10.0}}},
            {"kind": "tap", "target": {"ref": "e99"}},
            {"kind": "tap", "target": {"point": {"x": 30.0, "y": 30.0}}},
        ])),
    )
    .await;
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(r["steps"].as_array().unwrap().len(), 1);
    assert_eq!(r["failed"]["step"], 2);
    assert_eq!(r["failed"]["error"]["code"], -32006);
    assert!(
        r["failed"]["error"]["data"]["hint"]
            .as_str()
            .unwrap()
            .contains("snapshot")
    );
    assert!(r["observation"]["text"].is_string());
    let taps = fake
        .calls()
        .iter()
        .filter(|c| matches!(c, Call::Gesture(_)))
        .count();
    assert_eq!(taps, 1, "the step after the failure must not run");
}

#[tokio::test]
async fn a_ref_from_another_session_is_named_as_such_and_a_batch_does_not_dump_the_screen() {
    let (source, fake) = FakeSource::with(vec![tree(&["One", "Two"])]);
    let (d, _home) = daemon(source);
    ok(&d, "first", "snapshot", json!({})).await;
    let mut tap = tap_ref("e9");
    tap["share"] = json!(true);
    let e = err(&d, "second", "act", tap).await;
    assert_eq!(e.code, -32006, "{e:?}");
    assert!(
        e.message.contains("earlier screen or another session"),
        "{}",
        e.message
    );
    assert_eq!(e.data["unseen"], true, "{e:?}");
    assert!(e.data["hint"].as_str().unwrap().contains("snapshot"));
    let mut steps = batch(json!([{"kind": "tap", "target": {"ref": "e9"}}]));
    steps["share"] = json!(true);
    let r = ok(&d, "third", "batch", steps).await;
    assert_eq!(r["failed"]["step"], 1, "{r}");
    assert_eq!(r["failed"]["error"]["data"]["unseen"], true, "{r}");
    assert_eq!(r["observation"]["text"], "", "{r}");
    assert!(fake.calls().is_empty());
}

#[tokio::test]
async fn a_stale_ref_in_a_batch_still_shows_what_changed_in_this_session() {
    let (source, _fake) = FakeSource::with(vec![
        tree(&["One", "Two", "Three"]),
        tree(&["One", "Two", "Three"]),
        tree(&["Other"]),
    ]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(
        &d,
        "default",
        "batch",
        batch(json!([
            {"kind": "tap", "target": {"ref": "e1"}},
            {"kind": "tap", "target": {"ref": "e3"}},
        ])),
    )
    .await;
    assert_eq!(r["failed"]["step"], 2, "{r}");
    assert_eq!(r["failed"]["error"]["data"]["unseen"], false, "{r}");
    assert!(
        !r["observation"]["text"].as_str().unwrap().is_empty(),
        "{r}"
    );
}

#[tokio::test]
async fn a_batch_with_a_malformed_step_runs_nothing_and_names_the_step() {
    let (source, fake) = FakeSource::with(vec![tree(&["One"])]);
    let (d, _home) = daemon(source);
    let e = err(
        &d,
        "default",
        "batch",
        batch(json!([
            {"kind": "tap", "target": {"point": {"x": 10.0, "y": 10.0}}},
            {"kind": "teleport"},
        ])),
    )
    .await;
    assert_eq!(e.code, -32602);
    assert!(e.message.contains("step 2"), "{}", e.message);
    assert!(fake.calls().is_empty());
    let e = err(&d, "default", "batch", batch(json!([]))).await;
    assert_eq!(e.code, -32602);
}

#[tokio::test(start_paused = true)]
async fn drag_presses_at_the_first_target_holds_then_moves_to_the_second_and_pinch_plays_two_fingers()
 {
    let (source, fake) = FakeSource::with(vec![tree(&["One", "Two"])]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "drag", "from": {"ref": "e1"}, "to": {"point": {"x": 300.0, "y": 700.0}}})),
    )
    .await;
    assert!(
        r["summary"].as_str().unwrap().starts_with("dragged @e1"),
        "{r}"
    );
    let Call::Gesture(g) = fake.calls().pop().unwrap() else {
        panic!("no gesture");
    };
    assert_eq!(g.fingers.len(), 1);
    let path = &g.fingers[0];
    assert_eq!((path[0].at.x, path[0].at.y), (180.0, 130.0));
    assert!(path.iter().find(|p| p.at != path[0].at).unwrap().t_ms >= 400);
    let end = path.last().unwrap().at;
    assert_eq!((end.x, end.y), (300.0, 700.0));

    let r = ok(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "pinch", "direction": "out"})),
    )
    .await;
    assert_eq!(r["summary"], "pinched out");
    let Call::Gesture(g) = fake.calls().pop().unwrap() else {
        panic!("no gesture");
    };
    assert_eq!(g.fingers.len(), 2);
    let gap = |i: usize| g.fingers[1][i].at.x - g.fingers[0][i].at.x;
    assert!(gap(10) > gap(0) * 4.0);
    assert_eq!(g.fingers[0][0].at.x + g.fingers[1][0].at.x, 360.0);
}

#[tokio::test]
async fn pinch_needs_multi_touch() {
    let (source, _) =
        FakeSource::of(FakeDevice::new(vec![tree(&["One"])]).lacking(Capabilities::MULTI_TOUCH));
    let (d, _home) = daemon(source);
    let e = err(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "pinch", "direction": "in"})),
    )
    .await;
    assert_eq!(e.code, -32011);
}

fn app_call(action: &str, app: &str) -> Value {
    json!({"action": action, "app": app})
}

#[tokio::test]
async fn stop_clear_and_uninstall_on_an_emulator_reach_the_device_by_name() {
    let (source, fake) = installed();
    let (d, _home) = daemon(source);
    let r = ok(&d, "default", "app", app_call("stop", "settings")).await;
    assert!(r["summary"].as_str().unwrap().starts_with("stopped"), "{r}");
    ok(&d, "default", "app", app_call("clear", "settings")).await;
    ok(&d, "default", "app", app_call("uninstall", "settings")).await;
    let ops: Vec<Call> = fake
        .calls()
        .into_iter()
        .filter(|c| matches!(c, Call::AppOp(..)))
        .collect();
    use phonedriver_core::device::AppOp;
    let id = AppId("com.android.settings".into());
    assert_eq!(
        ops,
        [
            Call::AppOp(AppOp::Stop, id.clone()),
            Call::AppOp(AppOp::ClearData, id.clone()),
            Call::AppOp(AppOp::Uninstall, id)
        ]
    );
    let e = err(&d, "default", "app", app_call("stop", "nonexistent app")).await;
    assert_eq!(e.code, -32017);
    let e = err(&d, "default", "app", json!({"action": "install"})).await;
    assert_eq!(e.code, -32011);
}

fn physical() -> (Arc<FakeSource>, Arc<FakeDevice>) {
    let fake = Arc::new(FakeDevice::new(vec![tree(&["One"])]));
    fake.install(&[
        ("com.example.installed", "Mine"),
        ("com.example.theirs", "Theirs"),
    ]);
    let source = Arc::new(FakeSource {
        devices: Mutex::new(vec![(
            Listed {
                id: "android:R5CT123".into(),
                model: "Pixel".into(),
                os: None,
                tier: "fake".into(),
                capabilities: fake.info().capabilities,
            },
            fake.clone(),
        )]),
    });
    (source, fake)
}

#[tokio::test]
async fn on_a_physical_device_only_apps_phonedriver_installed_can_be_cleared_or_removed() {
    let (source, fake) = physical();
    let (d, home) = daemon(source);
    for action in ["clear", "uninstall"] {
        let e = err(&d, "default", "app", app_call(action, "Theirs")).await;
        assert_eq!(e.code, -32011, "{action}");
        assert!(e.message.contains("did not install"), "{}", e.message);
        let e = err(&d, "default", "app", app_call(action, "Mine")).await;
        assert_eq!(e.code, -32011, "{action} before install");
    }
    ok(&d, "default", "app", app_call("stop", "Theirs")).await;
    let r = ok(
        &d,
        "default",
        "app",
        json!({"action": "install", "path": "/tmp/a.apk"}),
    )
    .await;
    assert_eq!(r["summary"], "installed com.example.installed");
    ok(&d, "default", "app", app_call("clear", "Mine")).await;
    ok(&d, "default", "app", app_call("uninstall", "Mine")).await;
    let e = err(&d, "default", "app", app_call("uninstall", "Mine")).await;
    assert_eq!(e.code, -32011, "the record ends with the uninstall");
    let removed = fake
        .calls()
        .iter()
        .filter(|c| {
            matches!(
                c,
                Call::AppOp(phonedriver_core::device::AppOp::Uninstall, _)
            )
        })
        .count();
    assert_eq!(removed, 1);
    assert!(home.path().join("installed.json").exists());
}

#[tokio::test(start_paused = true)]
async fn launch_with_grant_grants_before_it_launches_and_says_so() {
    let (source, fake) = installed();
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(
        &d,
        "default",
        "act",
        act_with(
            json!({"kind": "launch", "app": "Settings", "grant": ["notifications", "camera"]}),
        ),
    )
    .await;
    let calls = fake.calls();
    let grant = calls
        .iter()
        .position(|c| matches!(c, Call::Grant(..)))
        .expect("granted");
    let launch = calls
        .iter()
        .position(|c| matches!(c, Call::Launch(..)))
        .expect("launched");
    assert!(grant < launch, "{calls:?}");
    assert_eq!(
        calls[grant],
        Call::Grant(
            AppId("com.android.settings".into()),
            vec!["notifications".into(), "camera".into()]
        )
    );
    assert!(
        r["summary"]
            .as_str()
            .unwrap()
            .contains("(granted notifications, camera)"),
        "{r}"
    );
}

#[tokio::test]
async fn grant_on_a_physical_device_needs_an_app_phonedriver_installed() {
    let (source, fake) = physical();
    let (d, _home) = daemon(source);
    let e = err(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "launch", "app": "Theirs", "grant": ["camera"]})),
    )
    .await;
    assert_eq!(e.code, -32011);
    assert!(e.message.contains("will not grant"), "{}", e.message);
    assert!(
        !fake
            .calls()
            .iter()
            .any(|c| matches!(c, Call::Grant(..) | Call::Launch(..))),
        "{:?}",
        fake.calls()
    );
    ok(
        &d,
        "default",
        "app",
        json!({"action": "install", "path": "/tmp/a.apk"}),
    )
    .await;
    ok(
        &d,
        "default",
        "act",
        act_with(json!({"kind": "launch", "app": "Mine", "grant": ["camera"]})),
    )
    .await;
    assert!(
        fake.calls()
            .iter()
            .any(|c| matches!(c, Call::Grant(a, _) if a.0 == "com.example.installed"))
    );
}

#[tokio::test]
async fn a_system_dialog_over_an_app_is_named_in_the_header_and_is_not_the_app() {
    let json = r#"{"windows":[
        {"id":9,"type":"application","package":"com.google.android.permissioncontroller","bounds":[28,767,1052,1735]},
        {"id":3,"type":"application","package":"com.example","bounds":[0,0,360,800]}],"nodes":[
        {"p":-1,"w":9,"cls":"android.widget.LinearLayout","b":[28,767,1052,1735],"f":"vVe"},
        {"p":0,"w":9,"cls":"android.widget.TextView","id":"com.google.android.permissioncontroller:id/permission_message","text":"Allow Clock to send notifications?","b":[100,800,900,900],"f":"vVeH"},
        {"p":0,"w":9,"cls":"android.widget.Button","text":"Allow","b":[100,1000,900,1100],"f":"vVe","a":["tap"]},
        {"p":-1,"w":3,"cls":"android.widget.TextView","text":"Clock","b":[0,0,360,100],"f":"vVe"}]}"#;
    let tree = RawTree::from_android_json(json, Scale { density: 3.0 }).unwrap();
    let (source, _fake) = FakeSource::with(vec![tree]);
    let (d, _home) = daemon(source);
    let r = ok(&d, "default", "snapshot", json!({})).await;
    let text = r["text"].as_str().unwrap();
    assert!(text.contains("app com.example"), "{text}");
    assert!(
        text.contains(
            "alert com.google.android.permissioncontroller: \"Allow Clock to send notifications?\""
        ),
        "{text}"
    );
    assert!(text.contains("button \"Allow\""), "{text}");
}
