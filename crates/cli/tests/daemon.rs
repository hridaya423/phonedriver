use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use phone_use::daemon::rpc::RpcError;
use phone_use::daemon::{Config, Daemon, DeviceSource, Listed};
use phone_use_core::device::{AppId, Capabilities, Device, DeviceEvent, FrameRef, Url};
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
    fake.rotate_to(phone_use_core::geometry::Size { w: 800.0, h: 360.0 });
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
    assert!(trace.contains("\"secret_len\":7"), "{trace}");
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
        ("dev.phoneuse.testbed", "phone-use testbed"),
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
    assert_eq!(names, ["phone-use testbed", "Calculator", "Settings"]);
}

#[tokio::test(start_paused = true)]
async fn launch_by_name_in_any_case_or_by_package_foregrounds_the_app_and_returns_a_full_snapshot()
{
    for app in [
        "phone-use testbed",
        "PHONE-USE TESTBED",
        "dev.phoneuse.testbed",
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
                .contains(&Call::Launch(AppId("dev.phoneuse.testbed".into()))),
            "{app}: {:?}",
            fake.calls()
        );
    }
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
    assert_eq!(e.code, -32007, "{e:?}");
    assert!(
        e.data["similar"].to_string().contains("Calculator"),
        "{e:?}"
    );
    assert!(fake.calls().is_empty());
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
        vec![Call::OpenUrl(Url("https://example.com".into()))]
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
        Call::Text(phone_use_core::device::TextInput::SetValue(String::new()))
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

fn screen(on: bool, locked: bool, secure: bool) -> phone_use_core::device::ScreenState {
    phone_use_core::device::ScreenState {
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
        vec![Call::Button(phone_use_core::device::Button::Lock)]
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
    assert!(fake.calls().is_empty(), "{:?}", fake.calls());

    ok(
        &d,
        "default",
        "act",
        json!({"action": {"kind": "tap", "target": {"text": "Cancel"}}}),
    )
    .await;
    assert_eq!(fake.calls().len(), 1);
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
            .contains(&Call::Button(phone_use_core::device::Button::Back))
    );
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
        {"p":0,"w":1,"cls":"android.widget.Button","text":"Developer","b":[0,760,360,820],"f":"vVe","a":["tap"]},
        {"p":0,"w":1,"cls":"android.widget.Button","text":"Privacy","b":[0,100,360,160],"f":"vVe","a":["tap"]},
        {"p":0,"w":1,"cls":"androidx.appcompat.widget.Toolbar","desc":"Toolbar","b":[0,700,360,800],"f":"vVe"},
        {"p":3,"w":1,"cls":"android.widget.EditText","text":"","desc":"Search","b":[10,720,350,780],"f":"vVeE","a":["tap","set_text"]}]}"#;
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
}

fn toolbar_list(row_top: i32) -> RawTree {
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,100,360,800],"f":"vVeS","a":["scroll_down"]}},
        {{"p":1,"w":1,"cls":"android.widget.Button","text":"Developer","b":[0,{row_top},360,{}],"f":"vVe","a":["tap"]}},
        {{"p":0,"w":1,"cls":"androidx.appcompat.widget.Toolbar","desc":"Toolbar","b":[0,700,360,800],"f":"vVe"}}]}}"#,
        row_top + 40
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

#[tokio::test(start_paused = true)]
async fn scroll_until_keeps_going_while_the_target_is_under_the_toolbar() {
    let (source, fake) = FakeSource::with(vec![toolbar_list(750), toolbar_list(400)]);
    let (d, _home) = daemon(source);
    let r = ok(
        &d,
        "default",
        "act",
        scroll(json!({"until": {"text": "Developer"}})),
    )
    .await;
    assert!(
        r["summary"].as_str().unwrap().contains("1 times"),
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
            .contains(&Call::Button(phone_use_core::device::Button::Back))
    );
    assert!(!r["observation"]["header"]["keyboard"].is_object(), "{r}");
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
