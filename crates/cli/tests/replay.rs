#[path = "support/fakes.rs"]
mod fakes;

use std::path::PathBuf;
use std::sync::Arc;

use assert_cmd::Command;
use async_trait::async_trait;
use fakes::tree;
use phonedriver::daemon::{Config, Daemon, DeviceSource, Listed};
use phonedriver_core::device::{Capabilities, Device, TextInput};
use phonedriver_core::error::Error;
use phonedriver_core::geometry::Scale;
use phonedriver_core::raw::RawTree;
use phonedriver_core::testing::FakeDevice;
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

fn setup(trees: Vec<RawTree>) -> (Arc<Daemon>, Arc<FakeDevice>, tempfile::TempDir) {
    let fake = Arc::new(FakeDevice::new(trees).lacking(Capabilities::FRAMES));
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

fn tap_ref(r: &str) -> Value {
    json!({"action": {"kind": "tap", "target": {"ref": r}}})
}

#[tokio::test(start_paused = true)]
async fn the_journal_holds_each_action_that_worked_with_a_target_that_outlives_its_ref() {
    let (d, _fake, _home) = setup(vec![tree(&["One", "Two"])]);
    ok(&d, "snapshot", json!({})).await;
    ok(&d, "act", tap_ref("e1")).await;
    ok(
        &d,
        "act",
        json!({"action": {"kind": "tap", "target": {"text": "Two"}}}),
    )
    .await;
    d.call(
        "default",
        "act",
        json!({"action": {"kind": "tap", "target": {"text": "Nowhere"}}}),
    )
    .await
    .expect_err("no such element");
    ok(
        &d,
        "act",
        json!({"action": {"kind": "press", "key": "back"}}),
    )
    .await;

    let saved = ok(&d, "journal", json!({})).await;
    let steps = saved["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 3, "{saved}");
    assert_eq!(
        steps[0],
        json!({
            "method": "act",
            "params": {"action": {"kind": "tap", "target": {"text": "One", "role": "button"}, "count": null, "long": null}},
            "summary": "tapped @e1 \"One\"",
            "changed": false,
        })
    );
    assert_eq!(
        steps[1]["params"]["action"]["target"],
        json!({"text": "Two", "role": null})
    );
    assert_eq!(
        steps[2]["params"]["action"],
        json!({"kind": "press", "key": "back"})
    );
    assert_eq!(saved["over"], false);
    assert_eq!(ok(&d, "journal", json!({})).await["steps"], json!([]));
}

#[tokio::test(start_paused = true)]
async fn the_journal_can_be_read_in_place_and_emptied_only_as_far_as_the_client_saved() {
    let (d, _fake, _home) = setup(vec![tree(&["One", "Two"])]);
    ok(&d, "snapshot", json!({})).await;
    ok(&d, "act", tap_ref("e1")).await;
    ok(&d, "act", tap_ref("e2")).await;
    let read = ok(&d, "journal", json!({"keep": true})).await;
    assert_eq!(read["steps"].as_array().unwrap().len(), 2, "{read}");
    ok(&d, "act", tap_ref("e1")).await;
    ok(&d, "journal", json!({"drop": 2})).await;
    let rest = ok(&d, "journal", json!({"keep": true})).await;
    let steps = rest["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 1, "{rest}");
    assert_eq!(steps[0]["params"], read["steps"][0]["params"]);
    assert_eq!(ok(&d, "journal", json!({})).await["steps"], rest["steps"]);
    assert_eq!(ok(&d, "journal", json!({})).await["steps"], json!([]));
}

#[tokio::test(start_paused = true)]
async fn an_element_its_text_does_not_single_out_is_saved_by_position_and_marked() {
    let (d, _fake, _home) = setup(vec![tree(&["Same", "Same"])]);
    ok(&d, "snapshot", json!({})).await;
    ok(&d, "act", tap_ref("e2")).await;
    ok(
        &d,
        "act",
        json!({"action": {"kind": "tap", "target": {"point": {"x": 10.0, "y": 20.0}}}, "approval": "autonomous"}),
    )
    .await;
    let saved = ok(&d, "journal", json!({})).await;
    let steps = saved["steps"].as_array().unwrap();
    assert_eq!(
        steps[0]["params"]["action"]["target"],
        json!({"point": {"x": 180.0, "y": 230.0}})
    );
    assert_eq!(steps[0]["by_position"], true);
    assert_eq!(steps[1]["by_position"], true);
}

#[tokio::test(start_paused = true)]
async fn a_typed_secret_is_a_step_with_its_text_left_out() {
    let (d, _fake, _home) = setup(vec![field(""), field(""), field("hunter2")]);
    ok(&d, "snapshot", json!({})).await;
    ok(
        &d,
        "act",
        json!({"action": {"kind": "type", "text": "hunter2", "into": {"ref": "e1"}, "secret": true}}),
    )
    .await;
    let saved = ok(&d, "journal", json!({})).await;
    assert!(!saved.to_string().contains("hunter2"), "{saved}");
    let step = &saved["steps"][0];
    assert_eq!(step["method"], "act", "{step}");
    let action = &step["params"]["action"];
    assert_eq!(action["kind"], "type", "{step}");
    assert_eq!(action["secret"], true, "{step}");
    assert!(action.get("text").is_none(), "{step}");
    assert_eq!(action["into"]["text"], "Field", "{step}");
}

#[tokio::test(start_paused = true)]
async fn typed_text_a_wait_and_a_shell_command_are_steps_too() {
    let (d, _fake, _home) = setup(vec![field(""), field(""), field("hello")]);
    ok(&d, "snapshot", json!({})).await;
    ok(
        &d,
        "act",
        json!({"action": {"kind": "type", "text": "hello", "into": {"ref": "e1"}}}),
    )
    .await;
    ok(
        &d,
        "wait",
        json!({"condition": {"appears": {"text": "Field"}}}),
    )
    .await;
    ok(&d, "shell", json!({"command": "pm clear com.example"})).await;
    ok(
        &d,
        "shell",
        json!({"command": "login token", "secret": true}),
    )
    .await;
    let saved = ok(&d, "journal", json!({})).await;
    let steps = saved["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 4, "{saved}");
    assert_eq!(steps[0]["params"]["action"]["text"], "hello");
    assert_eq!(
        steps[0]["params"]["action"]["into"],
        json!({"text": "Field", "role": "text_field"})
    );
    assert_eq!(
        steps[1],
        json!({"method": "wait", "params": {"condition": {"appears": {"text": "Field", "role": null}}}, "summary": steps[1]["summary"]})
    );
    assert_eq!(
        steps[2],
        json!({"method": "shell", "params": {"command": "pm clear com.example"}})
    );
    assert!(
        steps[3]["manual"].as_str().unwrap().contains("shell"),
        "{saved}"
    );
    assert!(!saved.to_string().contains("login token"), "{saved}");
}

fn fields(nodes: &[&str]) -> RawTree {
    let mut all = vec![
        r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}"#
            .to_string(),
    ];
    for (i, more) in nodes.iter().enumerate() {
        let top = 100 + 100 * i as i32;
        all.push(format!(
            r#"{{"p":0,"w":1,"cls":"android.widget.EditText",{more},"b":[0,{top},360,{}],"f":"vVeEF","a":["tap","set_text"]}}"#,
            top + 60
        ));
    }
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[{}]}}"#,
        all.join(",")
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

fn type_into(target: Value) -> Value {
    json!({"action": {"kind": "type", "text": "x", "into": target}})
}

/// The Files app's "New folder" field and the testbed's web form have fields with no label; a run of the bench's
/// jobs saved both by position (2026-11-15), and the dialog sat 156 points higher in the replay.
#[tokio::test(start_paused = true)]
async fn an_element_is_saved_by_an_id_of_its_own_with_its_text_beside_it() {
    let screen = || {
        fields(&[
            r#""id":"name""#,
            r#""id":"email","hint":"Email address""#,
            r#""id":"same""#,
            r#""id":"same""#,
            r#""id":"android:id/text1""#,
            r#""hint":"Search""#,
        ])
    };
    let (d, _fake, _home) = setup(vec![screen(); 20]);
    ok(&d, "snapshot", json!({})).await;
    for r in ["e1", "e2", "e3", "e5", "e6"] {
        ok(&d, "act", type_into(json!({"ref": r}))).await;
    }
    let saved = ok(&d, "journal", json!({})).await;
    let steps = saved["steps"].as_array().unwrap();
    assert_eq!(
        steps[0]["params"]["action"]["into"],
        json!({"id": "name"}),
        "{saved}"
    );
    assert!(steps[0].get("by_position").is_none(), "{saved}");
    assert_eq!(
        steps[1]["params"]["action"]["into"],
        json!({"id": "email", "text": "Email address", "role": "text_field"}),
        "{saved}"
    );
    assert_eq!(
        steps[2]["params"]["action"]["into"],
        json!({"point": {"x": 180.0, "y": 330.0}}),
        "an id two elements share finds neither: {saved}"
    );
    assert_eq!(steps[2]["by_position"], true);
    assert_eq!(
        steps[3]["params"]["action"]["into"],
        json!({"point": {"x": 180.0, "y": 530.0}}),
        "an id of the platform's names a slot, not this field: {saved}"
    );
    assert_eq!(
        steps[4]["params"]["action"]["into"],
        json!({"text": "Search", "role": "text_field"}),
        "{saved}"
    );

    let again = ok(&d, "act", type_into(json!({"id": "name"}))).await;
    assert!(
        again["summary"].as_str().unwrap().contains("@e1"),
        "{again}"
    );
    let renamed = ok(
        &d,
        "act",
        type_into(json!({"id": "mail", "text": "Email address", "role": "text_field"})),
    )
    .await;
    assert!(
        renamed["summary"].as_str().unwrap().contains("@e2"),
        "the text finds it when the id is gone: {renamed}"
    );
    let two = d
        .call("default", "act", type_into(json!({"id": "same"})))
        .await
        .expect_err("two elements have this id");
    assert!(two.message.contains("same"), "{two:?}");
    let none = d
        .call("default", "act", type_into(json!({"id": "gone"})))
        .await
        .expect_err("no element has this id");
    assert!(none.message.contains("gone"), "{none:?}");
}

/// A daemon on its own socket for the `phonedriver` binary to talk to, as `cli.rs` serves one.
async fn served(fake: Arc<FakeDevice>) -> (tempfile::TempDir, PathBuf) {
    let home = tempfile::tempdir().unwrap();
    let daemon = Daemon::new(
        Arc::new(One(fake)),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    let socket = if cfg!(windows) {
        PathBuf::from(format!(
            r"\\.\pipe\phonedriver-test-{}-replay",
            home.path().file_name().unwrap().to_string_lossy()
        ))
    } else {
        home.path().join("run").join("d.sock")
    };
    let listener = phonedriver::daemon::socket::listen(&socket).await.unwrap();
    tokio::spawn(daemon.serve_on(listener));
    (home, socket)
}

/// Runs the CLI against that daemon: whether it succeeded, and what it printed (stdout and stderr).
async fn run(home: &tempfile::TempDir, socket: &PathBuf, args: &[&str]) -> (bool, String) {
    let mut c = Command::cargo_bin("phonedriver").unwrap();
    c.env("PHONEDRIVER_HOME", home.path())
        .env("PHONEDRIVER_SOCKET", socket)
        .env_remove("PHONEDRIVER_SESSION")
        .env_remove("PHONEDRIVER_DEVICE")
        .env_remove("PHONEDRIVER_SECRET")
        .args(args);
    let out = tokio::task::spawn_blocking(move || c.output().unwrap())
        .await
        .unwrap();
    let shown = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), shown)
}

/// Contacts pre-fills `+1` in a new phone field; a replay typed `0655555552`, the field held `+10655555552`, and the
/// replay said "replayed 6 steps". The live `type` reply says NOT verified for a field that holds other text than
/// typed; a replayed step must stop there too, saying what the field shows.
#[tokio::test(flavor = "multi_thread")]
async fn a_replayed_type_whose_field_holds_other_text_than_typed_stops_the_replay() {
    let fake = Arc::new(FakeDevice::new(vec![field("")]).lacking(Capabilities::FRAMES));
    fake.on_text(|t| match t {
        TextInput::Paste(v) | TextInput::SetValue(v) | TextInput::Keystrokes(v) if v == "hello" => {
            Some(vec![field("Mr hello")])
        }
        _ => None,
    });
    let (home, socket) = served(fake).await;
    let steps = home.path().join("form.steps");
    let typed = json!({"method": "act", "params": {"action": {"kind": "type", "text": "hello", "into": {"text": "Field", "role": "text_field"}}}, "summary": "typed \"hello\" into @e1 \"Field\""});
    let back = json!({"method": "act", "params": {"action": {"kind": "press", "key": "back"}}, "summary": "pressed back"});
    std::fs::write(&steps, format!("{typed}\n{back}\n")).unwrap();

    let (succeeded, shown) = run(&home, &socket, &["replay", steps.to_str().unwrap()]).await;
    assert!(
        !succeeded,
        "the field holds \"Mr hello\", not \"hello\", yet the replay went on: {shown}"
    );
    assert!(shown.contains("step 1 of 2"), "{shown}");
    assert!(shown.contains("Mr hello"), "{shown}");
    assert!(shown.contains("from step 2"), "{shown}");
}

/// A form whose phone field the keyboard pushes out of the list, as `daemon.rs` builds it.
fn form(keyboard: bool, phone_at: Option<f64>) -> RawTree {
    let ime = if keyboard {
        r#",{"id":2,"type":"input_method","package":"com.google.android.inputmethod.latin","bounds":[0,500,360,800]}"#
    } else {
        ""
    };
    let phone = phone_at.map_or(String::new(), |y| {
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

/// A drag of more than 20 points, as a scroll is, not a tap.
fn drags(g: &phonedriver_core::device::Gesture) -> bool {
    let path = &g.fingers[0];
    match (path.first(), path.last()) {
        (Some(a), Some(b)) => (a.at.y - b.at.y).abs() > 20.0,
        _ => false,
    }
}

/// Contacts (2026-11): the agent typed into @e24 (Company) after a type reply listed it as gone from the screen;
/// the daemon scrolled it back and typed, and the steps file kept `"into": {"ref": "e24"}`, which names nothing in
/// a later run, and `save` said nothing about it.
#[tokio::test(start_paused = true)]
#[ignore = "open bug: lasting() keeps a ref the last screen does not hold (journal.rs); remove this line with the fix"]
async fn a_typed_field_the_last_screen_had_dropped_is_not_saved_by_its_session_ref() {
    let (d, fake, _home) = setup(vec![form(false, Some(600.0))]);
    let snap = ok(&d, "snapshot", json!({})).await;
    let phone = snap["text"]
        .as_str()
        .unwrap()
        .lines()
        .find(|l| l.contains("\"Phone (Mobile)\""))
        .and_then(|l| l.trim_start().split(' ').next())
        .unwrap()
        .trim_start_matches('@')
        .to_string();
    fake.set_tree(form(true, None));
    ok(&d, "snapshot", json!({})).await;
    fake.on_gesture(|g| drags(g).then(|| vec![form(true, Some(300.0))]));
    let typed = ok(
        &d,
        "act",
        json!({"action": {"kind": "type", "text": "555 0199", "into": {"ref": phone}}}),
    )
    .await;
    assert!(
        typed["summary"].as_str().unwrap().contains("scrolled"),
        "the daemon brought the dropped field back: {typed}"
    );

    let saved = ok(&d, "journal", json!({})).await;
    let step = &saved["steps"][0];
    let into = &step["params"]["action"]["into"];
    assert!(
        into.get("ref").is_none() || step["by_position"] == true,
        "the step keeps the session-only ref @{phone} and does not mark it: {saved}"
    );
}

fn list(labels: &[&str]) -> RawTree {
    let mut nodes = vec![
        r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}"#
            .to_string(),
        r#"{"p":0,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,80,360,800],"f":"vVeS","a":["scroll_down","scroll_up"]}"#
            .to_string(),
    ];
    for (i, l) in labels.iter().enumerate() {
        let top = 100 + 100 * i as i32;
        nodes.push(format!(
            r#"{{"p":1,"w":1,"cls":"android.widget.Button","text":"{l}","b":[0,{top},360,{}],"f":"vVe","a":["tap"]}}"#,
            top + 60
        ));
    }
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[{}]}}"#,
        nodes.join(",")
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

/// A list that one drag moves to its end, where "Email" shows but "Add email" never does.
fn scrolling_list() -> Arc<FakeDevice> {
    let fake =
        Arc::new(FakeDevice::new(vec![list(&["Phone", "Name"])]).lacking(Capabilities::FRAMES));
    fake.on_gesture(|g| drags(g).then(|| vec![list(&["Website", "Email"])]));
    fake
}

/// Contacts (2026-11): `scroll down --until "Add email"` scrolled 4 times and failed (not found); the next step,
/// `tap "Email"`, worked only because of that scroll. The failed scroll was not saved, so the replay stopped at the
/// tap.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "open bug: a failed call that moved the screen is not journaled (actor.rs, journal pushed after acted?); remove this line with the fix"]
async fn a_failed_scroll_that_moved_the_list_is_kept_for_the_step_after_it() {
    let (home, socket) = served(scrolling_list()).await;
    let (scrolled, shown) = run(&home, &socket, &["scroll", "down", "--until", "Add email"]).await;
    assert!(!scrolled, "the list has no \"Add email\": {shown}");
    let (tapped, shown) = run(&home, &socket, &["tap", "Email"]).await;
    assert!(tapped, "the scroll brought \"Email\" on screen: {shown}");
    let steps = home.path().join("email.steps");
    let file = steps.to_str().unwrap();
    let (saved, shown) = run(&home, &socket, &["save", file]).await;
    assert!(saved, "{shown}");

    let (home, socket) = served(scrolling_list()).await;
    let (replayed, shown) = run(&home, &socket, &["replay", file]).await;
    let kept = std::fs::read_to_string(&steps).unwrap();
    assert!(
        replayed,
        "the tap on \"Email\" needs the scroll before it, which the file does not hold:\n{kept}\n{shown}"
    );
}

/// A saved `type --submit` sent Enter before anything could tell the text was wrong, and a field that closes on Enter
/// leaves the verdict unknown, so the replay went on. A replay checks the text first and sends Enter only after.
#[tokio::test(flavor = "multi_thread")]
async fn a_replayed_type_and_submit_whose_text_did_not_land_sends_no_enter() {
    let fake = Arc::new(FakeDevice::new(vec![field("")]).lacking(Capabilities::FRAMES));
    fake.on_text(|t| match t {
        TextInput::Paste(v) | TextInput::SetValue(v) | TextInput::Keystrokes(v)
            if v == "0655555552" =>
        {
            Some(vec![field("+10655555552")])
        }
        _ => None,
    });
    let (home, socket) = served(fake.clone()).await;
    let steps = home.path().join("search.steps");
    let typed = json!({"method": "act", "params": {"action": {"kind": "type", "text": "0655555552", "into": {"text": "Field", "role": "text_field"}, "submit": true}}, "summary": "typed into @e1"});
    std::fs::write(&steps, format!("{typed}\n")).unwrap();

    let (succeeded, shown) = run(&home, &socket, &["replay", steps.to_str().unwrap()]).await;
    assert!(!succeeded, "the field holds +10655555552: {shown}");
    assert!(shown.contains("+10655555552"), "{shown}");
    let enters = fake
        .calls()
        .into_iter()
        .filter(|c| {
            matches!(
                c,
                phonedriver_core::testing::Call::Key(phonedriver_core::device::Key::Enter)
            )
        })
        .count();
    assert_eq!(enters, 0, "Enter went out with the wrong text: {shown}");
}

/// The same step with text that landed: one Enter, and the step line says the text was checked.
#[tokio::test(flavor = "multi_thread")]
async fn a_replayed_type_and_submit_whose_text_landed_sends_enter_once_and_says_verified() {
    let fake = Arc::new(FakeDevice::new(vec![field("")]).lacking(Capabilities::FRAMES));
    fake.on_text(|t| match t {
        TextInput::Paste(v) | TextInput::SetValue(v) | TextInput::Keystrokes(v) => {
            Some(vec![field(v)])
        }
        _ => None,
    });
    let (home, socket) = served(fake.clone()).await;
    let steps = home.path().join("search.steps");
    let typed = json!({"method": "act", "params": {"action": {"kind": "type", "text": "pizza", "into": {"text": "Field", "role": "text_field"}, "submit": true}}, "summary": "typed into @e1"});
    std::fs::write(&steps, format!("{typed}\n")).unwrap();

    let (succeeded, shown) = run(&home, &socket, &["replay", steps.to_str().unwrap()]).await;
    assert!(succeeded, "{shown}");
    let enters = fake
        .calls()
        .into_iter()
        .filter(|c| {
            matches!(
                c,
                phonedriver_core::testing::Call::Key(phonedriver_core::device::Key::Enter)
            )
        })
        .count();
    assert_eq!(enters, 1, "{shown}");
    assert!(
        shown.contains("1. typed") && shown.contains("· verified"),
        "{shown}"
    );
    assert!(shown.contains("pressed enter"), "{shown}");
}
