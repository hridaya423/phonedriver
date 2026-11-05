use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use phone_use::daemon::rpc::RpcError;
use phone_use::daemon::{Config, Daemon, DeviceSource, Listed};
use phone_use_core::device::{Capabilities, Device, DeviceEvent, FrameRef};
use phone_use_core::error::Error;
use phone_use_core::geometry::{Point, Rect, Scale};
use phone_use_core::raw::RawTree;
use phone_use_core::testing::{Call, FakeDevice};
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
        let fake = Arc::new(FakeDevice::new(trees));
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
            .ok_or(Error::NoDevice { connected: vec![] })
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
        PathBuf::from(format!(r"\\.\pipe\phone-use-test-{scope}-{name}"))
    } else {
        home.join(name)
    }
}

async fn wait_for_socket(path: &Path) {
    for _ in 0..200 {
        if phone_use::daemon::socket::connect(path).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("no socket at {}", path.display());
}

async fn rpc(path: &Path, session: &str, method: &str, params: Value) -> Value {
    let stream = phone_use::daemon::socket::connect(path).await.unwrap();
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
    let (source, fake) =
        FakeSource::with(vec![field_tree("", false), field_tree("héllo 世界", false)]);
    let (d, _home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", type_params("héllo 世界", false)).await;
    assert_eq!(r["verified"], true, "{r}");
    assert_eq!(r["via"], "paste");
    let calls = fake.calls();
    assert!(matches!(calls[0], Call::Gesture(_)), "{calls:?}");
    assert_eq!(
        calls[1],
        Call::Text(phone_use_core::device::TextInput::Paste(
            "héllo 世界".into()
        ))
    );
    let wrong = FakeSource::with(vec![
        field_tree("", false),
        field_tree("something else", false),
    ]);
    let (d2, _h) = daemon(wrong.0);
    ok(&d2, "default", "snapshot", json!({})).await;
    let r = ok(&d2, "default", "act", type_params("héllo", false)).await;
    assert_eq!(r["verified"], false);
}

#[tokio::test(start_paused = true)]
async fn secret_text_never_reaches_the_trace_the_reply_or_the_debug_output() {
    let (source, fake) =
        FakeSource::with(vec![field_tree("", false), field_tree("hunter2", false)]);
    let (d, home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", type_params("hunter2", true)).await;
    assert_eq!(r["verified"], true);
    assert!(!r.to_string().contains("hunter2"), "{r}");
    assert!(r["observation"]["text"].as_str().unwrap().contains("•••"));
    assert_eq!(
        fake.calls()[1],
        Call::Text(phone_use_core::device::TextInput::PasteSecret(
            "hunter2".into()
        ))
    );
    assert!(!format!("{:?}", fake.calls()).contains("hunter2"));
    let trace = trace_text(home.path());
    assert!(trace.contains("act type 7 chars"), "{trace}");
    assert!(!trace.contains("hunter2"), "{trace}");
}

#[tokio::test(start_paused = true)]
async fn typing_into_a_password_field_is_secret_without_the_flag_and_verified_by_length() {
    let (source, fake) = FakeSource::with(vec![field_tree("", true), field_tree("•••••••", true)]);
    let (d, home) = daemon(source);
    ok(&d, "default", "snapshot", json!({})).await;
    let r = ok(&d, "default", "act", type_params("hunter2", false)).await;
    assert_eq!(r["verified"], true, "{r}");
    assert_eq!(
        fake.calls()[1],
        Call::Text(phone_use_core::device::TextInput::PasteSecret(
            "hunter2".into()
        ))
    );
    assert!(!r.to_string().contains("hunter2"), "{r}");
    assert!(!trace_text(home.path()).contains("hunter2"));
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
        json!({"action": {"kind": "type", "text": "hunter2"}, "settle": false}),
    )
    .await;
    assert_eq!(r["ok"], true);
    assert_eq!(
        fake.calls()[0],
        Call::Text(phone_use_core::device::TextInput::PasteSecret(
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
            .contains(&Call::Button(phone_use_core::device::Button::Back))
    );
}

#[tokio::test(start_paused = true)]
async fn swipe_up_from_a_ref_sends_one_finger_moving_up_from_its_center() {
    let (source, fake) = FakeSource::with(vec![tree(&["One", "Two", "Three"])]);
    let (d, _home) = daemon(source);
    ok(&d, "s", "snapshot", json!({})).await;
    let r = ok(
        &d,
        "s",
        "act",
        json!({"action": {"kind": "swipe", "direction": "up", "from": {"ref": "e3"}}, "settle": false}),
    )
    .await;
    assert_eq!(r["summary"], "swiped up");
    let path = fake
        .calls()
        .into_iter()
        .find_map(|c| match c {
            Call::Gesture(g) => Some(g.fingers[0].clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(path[0].at, Point { x: 180.0, y: 330.0 });
    assert!(path.last().unwrap().at.y < path[0].at.y);
}

#[tokio::test(start_paused = true)]
async fn open_hands_the_url_to_the_device() {
    let (source, fake) = FakeSource::with(vec![tree(&["A"])]);
    let (d, _home) = daemon(source);
    let r = ok(&d, "s", "open", json!({"url": "https://example.com"})).await;
    assert_eq!(r["summary"], "opened https://example.com");
    assert!(
        fake.calls()
            .contains(&Call::OpenUrl(phone_use_core::device::Url(
                "https://example.com".into()
            )))
    );
}

#[tokio::test(start_paused = true)]
async fn launching_an_app_the_device_does_not_list_is_not_found() {
    let (source, fake) = FakeSource::with(vec![tree(&["A"])]);
    let (d, _home) = daemon(source);
    let e = err(
        &d,
        "s",
        "act",
        json!({"action": {"kind": "launch", "app": "Nope"}}),
    )
    .await;
    assert_eq!(e.code, -32007, "{e:?}");
    assert!(fake.calls().is_empty());
}

/// A device that polls for frames (no `FRAMES`) and whose gestures take 400 ms to be confirmed, like the iOS runner.
struct SlowPolled {
    inner: Arc<FakeDevice>,
    info: phone_use_core::device::DeviceInfo,
    after: RawTree,
}

#[async_trait]
impl Device for SlowPolled {
    fn info(&self) -> &phone_use_core::device::DeviceInfo {
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
        q: phone_use_core::device::Quality,
    ) -> Result<phone_use_core::device::Image, Error> {
        self.inner.screenshot(q).await
    }
    fn events(&self) -> tokio::sync::broadcast::Receiver<DeviceEvent> {
        self.inner.events()
    }
    async fn gesture(&self, g: phone_use_core::device::Gesture) -> Result<(), Error> {
        tokio::time::sleep(Duration::from_millis(400)).await;
        self.inner.gesture(g).await
    }
    async fn key(&self, k: phone_use_core::device::Key) -> Result<(), Error> {
        self.inner.key(k).await
    }
    async fn button(&self, b: phone_use_core::device::Button) -> Result<(), Error> {
        self.inner.button(b).await
    }
    async fn open_url(&self, u: &phone_use_core::device::Url) -> Result<(), Error> {
        self.inner.open_url(u).await
    }
    async fn text(&self, t: phone_use_core::device::TextInput) -> Result<(), Error> {
        self.inner.text(t).await
    }
    async fn clipboard(
        &self,
        op: phone_use_core::device::ClipboardOp,
    ) -> Result<Option<String>, Error> {
        self.inner.clipboard(op).await
    }
    async fn apps(&self) -> Result<Vec<phone_use_core::device::App>, Error> {
        self.inner.apps().await
    }
    async fn launch(&self, app: &phone_use_core::device::AppId) -> Result<(), Error> {
        self.inner.launch(app).await
    }
    async fn foreground(&self) -> Result<Option<phone_use_core::device::AppId>, Error> {
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
