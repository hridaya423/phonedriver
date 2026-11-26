use crate::fakes;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use fakes::tree;
use phonedriver::daemon::rpc::RpcError;
use phonedriver::daemon::{Config, Daemon, DeviceSource, Listed};
use phonedriver_core::device::{Capabilities, Device};
use phonedriver_core::error::Error;
use phonedriver_core::geometry::Scale;
use phonedriver_core::raw::RawTree;
use phonedriver_core::testing::{Call, FakeDevice};
use serde_json::{Value, json};

struct One(Arc<FakeDevice>);

#[async_trait]
impl DeviceSource for One {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(vec![Listed {
            id: "fake:1".into(),
            model: "Fake".into(),
            os: None,
            tier: "fake".into(),
            capabilities: self.0.info().capabilities,
        }])
    }
    async fn connect(&self, _id: &str) -> Result<Arc<dyn Device>, Error> {
        Ok(self.0.clone())
    }
}

fn field(value: &str) -> RawTree {
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.widget.EditText","text":"{value}","desc":"Field","b":[0,100,360,160],"f":"vVeEF","a":["tap","set_text"]}}]}}"#
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

fn setup(trees: Vec<RawTree>, recorder: bool) -> (Arc<Daemon>, Arc<FakeDevice>, tempfile::TempDir) {
    let fake = Arc::new(FakeDevice::new(trees).lacking(Capabilities::FRAMES));
    if recorder {
        fake.with_recorder();
    }
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::new(
        Arc::new(One(fake.clone())),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    (d, fake, home)
}

async fn ok(d: &Daemon, method: &str, params: Value) -> Value {
    d.call("default", method, params)
        .await
        .unwrap_or_else(|e| panic!("{method} failed: {e:?}"))
}

async fn err(d: &Daemon, method: &str, params: Value) -> RpcError {
    d.call("default", method, params)
        .await
        .expect_err("expected an error")
}

fn tap(text: &str) -> Value {
    json!({"action": {"kind": "tap", "target": {"text": text}}})
}

fn steps(path: &str) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[tokio::test(start_paused = true)]
async fn a_recording_keeps_the_actions_done_during_it_with_their_times() {
    let (d, fake, home) = setup(vec![tree(&["One", "Two"])], true);
    ok(&d, "snapshot", json!({})).await;
    ok(&d, "act", tap("One")).await;
    let started = ok(&d, "record", json!({"action": "start"})).await;
    let path = started["path"].as_str().unwrap().to_string();
    assert!(
        path.starts_with(home.path().join("tmp").join("fake_1").to_str().unwrap())
            && path.ends_with(".mp4"),
        "{path}"
    );
    tokio::time::sleep(Duration::from_millis(700)).await;
    ok(&d, "act", tap("One")).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    ok(&d, "act", tap("Two")).await;
    let e = err(&d, "act", tap("Nowhere")).await;
    assert_eq!(e.data["code_name"], "not_found");

    let r = ok(&d, "record", json!({"action": "stop"})).await;
    assert_eq!(r["path"], path.as_str());
    assert_eq!(
        (
            &r["ms"],
            &r["frames"],
            &r["width"],
            &r["height"],
            &r["bytes"]
        ),
        (&json!(1500), &json!(2), &json!(360), &json!(800), &json!(5))
    );
    assert_eq!(r["steps"], 2);
    let listed = steps(r["steps_path"].as_str().unwrap());
    assert_eq!(r["steps_path"], format!("{path}.steps.jsonl").as_str());
    assert_eq!(listed.len(), 2, "{listed:?}");
    assert_eq!(listed[0]["summary"], "tapped @e1 \"One\"");
    assert_eq!(listed[0]["at"], json!({"x": 180.0, "y": 130.0}));
    assert_eq!(listed[1]["summary"], "tapped @e2 \"Two\"");
    assert_eq!(listed[1]["at"], json!({"x": 180.0, "y": 230.0}));
    let (first, second) = (
        listed[0]["t_ms"].as_u64().unwrap(),
        listed[1]["t_ms"].as_u64().unwrap(),
    );
    assert!((700..1100).contains(&first), "{first}");
    assert!(second >= first + 400, "{first} then {second}");
    assert!(r.get("early").is_none(), "{r}");
    assert!(
        fake.calls()
            .contains(&Call::RecordStart(std::path::PathBuf::from(&path)))
    );
}

#[tokio::test(start_paused = true)]
async fn a_recording_goes_to_the_named_file_and_one_with_no_actions_has_no_steps_file() {
    let (d, _fake, home) = setup(vec![tree(&["One"])], true);
    let path = home.path().join("made/run.mp4");
    let started = ok(&d, "record", json!({"action": "start", "path": path})).await;
    assert_eq!(started["path"], path.to_str().unwrap());
    let r = ok(&d, "record", json!({"action": "stop"})).await;
    assert_eq!(r["steps"], 0);
    assert!(r.get("steps_path").is_none(), "{r}");
    assert!(!home.path().join("made/run.mp4.steps.jsonl").exists());
}

#[tokio::test(start_paused = true)]
async fn a_second_start_and_a_stop_with_none_running_are_refused() {
    let (d, _fake, _home) = setup(vec![tree(&["One"])], true);
    let e = err(&d, "record", json!({"action": "stop"})).await;
    assert_eq!(e.code, -32025);
    assert!(
        e.message.contains("no recording is running"),
        "{}",
        e.message
    );
    let first = ok(&d, "record", json!({"action": "start"})).await;
    let e = err(&d, "record", json!({"action": "start"})).await;
    assert_eq!(e.code, -32025);
    assert!(
        e.message.contains(first["path"].as_str().unwrap()),
        "{}",
        e.message
    );
    let r = ok(&d, "record", json!({"action": "stop"})).await;
    assert_eq!(r["path"], first["path"]);
}

#[tokio::test(start_paused = true)]
async fn a_start_that_cannot_clear_the_steps_file_leaves_nothing_recording() {
    let (d, _fake, home) = setup(vec![tree(&["One"])], true);
    let path = home.path().join("run.mp4");
    let sidecar = home.path().join("run.mp4.steps.jsonl");
    std::fs::create_dir(&sidecar).unwrap();
    let start = json!({"action": "start", "path": path});
    let e = err(&d, "record", start.clone()).await;
    assert!(e.message.contains("cannot be replaced"), "{}", e.message);

    std::fs::remove_dir(&sidecar).unwrap();
    ok(&d, "record", start).await;
    ok(&d, "record", json!({"action": "stop"})).await;
}

#[tokio::test(start_paused = true)]
async fn a_recording_the_device_lost_is_reported_with_its_file() {
    let (d, fake, _home) = setup(vec![tree(&["One"])], true);
    let started = ok(&d, "record", json!({"action": "start"})).await;
    fake.lose_recording();
    let e = err(&d, "record", json!({"action": "stop"})).await;
    assert_eq!(e.code, -32025);
    assert!(
        e.message.contains(started["path"].as_str().unwrap()),
        "{}",
        e.message
    );
    ok(&d, "record", json!({"action": "start"})).await;
}

#[tokio::test(start_paused = true)]
async fn a_secret_typed_while_recording_is_not_in_the_steps_file() {
    let (d, _fake, _home) = setup(vec![field(""), field(""), field("hunter2")], true);
    ok(&d, "snapshot", json!({})).await;
    ok(&d, "record", json!({"action": "start"})).await;
    ok(
        &d,
        "act",
        json!({"action": {"kind": "type", "text": "hunter2", "into": {"ref": "e1"}, "secret": true}}),
    )
    .await;
    let r = ok(&d, "record", json!({"action": "stop"})).await;
    assert_eq!(r["steps"], 1);
    let text = std::fs::read_to_string(r["steps_path"].as_str().unwrap()).unwrap();
    assert!(!text.contains("hunter2"), "{text}");
    assert!(text.contains("typed"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn a_device_without_a_video_stream_says_so_and_a_wrong_action_is_invalid() {
    let (d, _fake, _home) = setup(vec![tree(&["One"])], false);
    let e = err(&d, "record", json!({"action": "start"})).await;
    assert_eq!(e.code, -32011);
    let e = err(&d, "record", json!({"action": "pause"})).await;
    assert_eq!(e.code, -32602);
}
