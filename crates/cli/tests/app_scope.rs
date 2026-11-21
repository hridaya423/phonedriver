use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use phonedriver::daemon::rpc::RpcError;
use phonedriver::daemon::trace::TraceOptions;
use phonedriver::daemon::{Config, Daemon, DeviceSource, Listed};
use phonedriver_core::device::{
    App, Capabilities, Device, Image, ImageFormat, LogReply, ShellReply, TextInput,
};
use phonedriver_core::error::Error;
use phonedriver_core::geometry::Scale;
use phonedriver_core::raw::RawTree;
use phonedriver_core::scope::AppScope;
use phonedriver_core::testing::{Call, FakeDevice};
use serde_json::{Value, json};

const SECRET: &str = "hunter2-from-the-chat-app";

struct One {
    id: &'static str,
    fake: Arc<FakeDevice>,
}

#[async_trait]
impl DeviceSource for One {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(vec![Listed {
            id: self.id.into(),
            model: "Fake".into(),
            os: None,
            tier: "fake".into(),
            capabilities: self.fake.info().capabilities,
        }])
    }
    async fn connect(&self, _id: &str) -> Result<Arc<dyn Device>, Error> {
        Ok(self.fake.clone())
    }
}

struct Win {
    id: i32,
    kind: &'static str,
    package: &'static str,
    bounds: [i32; 4],
    labels: Vec<&'static str>,
}

fn app(id: i32, package: &'static str, labels: &[&'static str]) -> Win {
    Win {
        id,
        kind: "application",
        package,
        bounds: [0, 0, 360, 800],
        labels: labels.to_vec(),
    }
}

fn screen(wins: Vec<Win>) -> RawTree {
    let mut windows = Vec::new();
    let mut nodes = Vec::new();
    for w in wins {
        windows.push(format!(
            r#"{{"id":{},"type":"{}","package":"{}","layer":0,"bounds":[{},{},{},{}]}}"#,
            w.id, w.kind, w.package, w.bounds[0], w.bounds[1], w.bounds[2], w.bounds[3]
        ));
        let root = nodes.len();
        nodes.push(format!(
            r#"{{"p":-1,"w":{},"cls":"android.widget.FrameLayout","b":[{},{},{},{}],"f":"vVe"}}"#,
            w.id, w.bounds[0], w.bounds[1], w.bounds[2], w.bounds[3]
        ));
        for (i, l) in w.labels.iter().enumerate() {
            let top = w.bounds[1] + 10 + 70 * i as i32;
            nodes.push(format!(
                r#"{{"p":{root},"w":{},"cls":"android.widget.Button","text":"{l}","b":[{},{top},{},{}],"f":"vVe","a":["tap"]}}"#,
                w.id,
                w.bounds[0],
                w.bounds[2],
                top + 60
            ));
        }
    }
    let json = format!(
        r#"{{"windows":[{}],"nodes":[{}]}}"#,
        windows.join(","),
        nodes.join(",")
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

fn mine(labels: &[&'static str]) -> RawTree {
    screen(vec![app(1, "com.mine", labels)])
}

fn chat() -> RawTree {
    screen(vec![app(1, "com.chat", &[SECRET, "Reply"])])
}

fn heads_up() -> Win {
    Win {
        id: 3,
        kind: "system",
        package: "com.android.systemui",
        bounds: [10, 40, 350, 140],
        labels: vec![SECRET],
    }
}

fn png() -> Image {
    Image {
        width: 1,
        height: 1,
        format: ImageFormat::Png,
        bytes: vec![0x89, 0x50, 0x4e, 0x47],
    }
}

fn setup_as(
    id: &'static str,
    trees: Vec<RawTree>,
    scope: Option<&str>,
) -> (Arc<Daemon>, Arc<FakeDevice>, tempfile::TempDir) {
    let fake = Arc::new(
        FakeDevice::new(trees)
            .lacking(Capabilities::FRAMES)
            .with_screenshot(png())
            .with_id(id),
    );
    fake.install(&[
        ("com.mine", "Mine"),
        ("com.other", "Other"),
        ("com.chat", "Chat"),
        ("com.android.chrome", "Chrome"),
    ]);
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::start(
        Arc::new(One {
            id,
            fake: fake.clone(),
        }),
        Config {
            home: home.path().to_path_buf(),
        },
        TraceOptions {
            full: true,
            screens: false,
        },
        Vec::new(),
        phonedriver::daemon::policy::Policy::default(),
        scope.map(|s| AppScope::parse(s).unwrap().unwrap()),
    );
    (d, fake, home)
}

fn setup(trees: Vec<RawTree>) -> (Arc<Daemon>, Arc<FakeDevice>, tempfile::TempDir) {
    setup_as("android:fake", trees, Some("com.mine,com.other"))
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

fn act(action: Value) -> Value {
    json!({"action": action})
}

fn tap_point() -> Value {
    act(json!({"kind": "tap", "target": {"point": {"x": 50.0, "y": 60.0}}}))
}

fn gestures(fake: &FakeDevice) -> usize {
    fake.calls()
        .iter()
        .filter(|c| matches!(c, Call::Gesture(_)))
        .count()
}

fn everything_under(dir: &Path) -> String {
    let mut all = String::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            all.push_str(&everything_under(&path));
        } else if let Ok(text) = std::fs::read_to_string(&path) {
            all.push_str(&text);
        }
    }
    all
}

#[tokio::test(start_paused = true)]
async fn a_screen_of_an_app_outside_the_scope_shows_no_text_and_names_the_app() {
    let (d, _fake, _home) = setup(vec![chat()]);
    let r = ok(&d, "snapshot", json!({})).await;
    let text = r["text"].as_str().unwrap();
    assert!(
        text.contains("app com.chat (outside the app scope)"),
        "{text}"
    );
    assert!(!text.contains(SECRET), "{text}");
    assert!(!text.contains("Reply"), "{text}");
    assert!(text.contains("`press home`"), "{text}");
    assert!(r.get("empty").is_none(), "{r}");
    let found = ok(&d, "find", json!({"text": "Reply"})).await;
    assert!(!found.to_string().contains(SECRET), "{found}");
    let e = err(
        &d,
        "wait",
        json!({"condition": {"appears": {"text": "Reply"}}, "timeout_ms": 300}),
    )
    .await;
    assert_eq!(e.code, -32013, "{e:?}");
}

#[tokio::test(start_paused = true)]
async fn a_screen_of_an_app_in_the_scope_reads_as_it_did() {
    let (d, _fake, _home) = setup(vec![mine(&["Save"])]);
    let r = ok(&d, "snapshot", json!({})).await;
    let text = r["text"].as_str().unwrap();
    assert!(text.contains("app com.mine"), "{text}");
    assert!(!text.contains("outside"), "{text}");
    assert!(text.contains("\"Save\""), "{text}");
}

#[tokio::test(start_paused = true)]
async fn nothing_touches_the_screen_while_an_app_outside_the_scope_is_in_front() {
    let (d, fake, _home) = setup(vec![chat()]);
    ok(&d, "snapshot", json!({})).await;
    let attempts = [
        tap_point(),
        act(json!({"kind": "type", "text": "hello"})),
        act(json!({"kind": "swipe", "direction": "up"})),
        act(json!({"kind": "scroll", "direction": "down"})),
        act(
            json!({"kind": "gesture", "fingers": [[{"x": 1.0, "y": 1.0, "t_ms": 0}, {"x": 9.0, "y": 9.0, "t_ms": 50}]]}),
        ),
        act(
            json!({"kind": "drag", "from": {"point": {"x": 1.0, "y": 1.0}}, "to": {"point": {"x": 9.0, "y": 9.0}}}),
        ),
        act(json!({"kind": "pinch", "direction": "out"})),
        act(json!({"kind": "press", "key": "enter"})),
    ];
    for attempt in attempts {
        let e = err(&d, "act", attempt.clone()).await;
        assert_eq!(e.code, -32025, "{attempt}: {e:?}");
        assert!(e.message.contains("outside the app scope"), "{}", e.message);
        assert!(!e.to_json().to_string().contains(SECRET), "{e:?}");
    }
    let r = ok(
        &d,
        "batch",
        json!({"steps": [{"kind": "tap", "target": {"point": {"x": 5.0, "y": 5.0}}}]}),
    )
    .await;
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(r["failed"]["error"]["code"], -32025, "{r}");
    assert!(!r.to_string().contains(SECRET), "{r}");
    let e = err(&d, "clipboard", json!({"set": "x"})).await;
    assert_eq!(e.code, -32025, "{e:?}");
    assert_eq!(gestures(&fake), 0);
    assert!(
        !fake
            .calls()
            .iter()
            .any(|c| matches!(c, Call::Text(_) | Call::Key(_))),
        "{:?}",
        fake.calls()
    );
}

#[tokio::test(start_paused = true)]
async fn back_and_home_work_from_an_app_outside_the_scope_and_launching_one_inside_it_does_too() {
    let (d, fake, _home) = setup(vec![chat()]);
    fake.set_launch_takes(0);
    ok(&d, "act", act(json!({"kind": "press", "key": "back"}))).await;
    ok(&d, "act", act(json!({"kind": "press", "key": "home"}))).await;
    ok(&d, "act", act(json!({"kind": "launch", "app": "Mine"}))).await;
    let calls = fake.calls();
    assert!(
        calls.contains(&Call::Launch(phonedriver_core::device::AppId(
            "com.mine".into()
        ))),
        "{calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, Call::Button(phonedriver_core::device::Button::Home))),
        "{calls:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn launching_or_opening_what_the_scope_leaves_out_is_refused() {
    let (d, fake, _home) = setup(vec![mine(&["Save"])]);
    for app in ["Chrome", "com.chat", "Chat"] {
        let e = err(&d, "act", act(json!({"kind": "launch", "app": app}))).await;
        assert_eq!(e.code, -32025, "{app}: {e:?}");
        assert!(e.message.contains("app scope"), "{}", e.message);
    }
    let e = err(
        &d,
        "act",
        act(json!({"kind": "open", "url": "https://example.com"})),
    )
    .await;
    assert!(e.message.contains("--app"), "{}", e.message);
    let e = err(
        &d,
        "act",
        act(json!({"kind": "open", "url": "https://example.com", "app": "Chrome"})),
    )
    .await;
    assert_eq!(e.code, -32025, "{e:?}");
    ok(
        &d,
        "act",
        act(json!({"kind": "open", "url": "mine://home", "app": "Mine"})),
    )
    .await;
    let calls = fake.calls();
    assert!(
        !calls
            .iter()
            .any(|c| matches!(c, Call::Launch(a) if a.0 != "com.mine")),
        "{calls:?}"
    );
    assert!(
        calls.iter().any(
            |c| matches!(c, Call::OpenUrl(u, Some(a)) if u.0 == "mine://home" && a.0 == "com.mine")
        ),
        "{calls:?}"
    );
    assert!(
        !calls
            .iter()
            .any(|c| matches!(c, Call::OpenUrl(u, _) if u.0.starts_with("https"))),
        "{calls:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_permission_dialog_over_an_app_in_the_scope_is_shown_and_can_be_tapped() {
    let dialog = Win {
        id: 2,
        kind: "application",
        package: "com.google.android.permissioncontroller",
        bounds: [20, 200, 340, 500],
        labels: vec!["Allow", "Don't allow"],
    };
    let (d, fake, _home) = setup(vec![screen(vec![app(1, "com.mine", &["Save"]), dialog])]);
    let r = ok(&d, "snapshot", json!({})).await;
    let text = r["text"].as_str().unwrap();
    assert!(!text.contains("outside"), "{text}");
    assert!(text.contains("\"Allow\""), "{text}");
    ok(
        &d,
        "act",
        act(json!({"kind": "tap", "target": {"text": "Allow"}})),
    )
    .await;
    assert_eq!(gestures(&fake), 1);
}

#[tokio::test(start_paused = true)]
async fn the_keyboard_over_an_app_in_the_scope_does_not_stop_typing() {
    let ime = Win {
        id: 2,
        kind: "input_method",
        package: "com.google.android.inputmethod.latin",
        bounds: [0, 500, 360, 800],
        labels: vec![],
    };
    let (d, fake, _home) = setup(vec![screen(vec![app(1, "com.mine", &["Save"]), ime])]);
    ok(&d, "snapshot", json!({})).await;
    ok(&d, "act", act(json!({"kind": "type", "text": "hello"}))).await;
    assert!(
        fake.calls()
            .iter()
            .any(|c| matches!(c, Call::Text(TextInput::Paste(t)) if t == "hello")),
        "{:?}",
        fake.calls()
    );
}

#[tokio::test(start_paused = true)]
async fn shell_is_refused_with_the_way_to_lift_the_scope() {
    let (d, fake, _home) = setup(vec![mine(&["Save"])]);
    let e = err(
        &d,
        "shell",
        json!({"command": "am start -n com.chat/.Main"}),
    )
    .await;
    assert_eq!(e.code, -32025, "{e:?}");
    assert!(e.message.contains("PHONEDRIVER_APP_SCOPE"), "{}", e.message);
    assert!(e.message.contains("daemon stop"), "{}", e.message);
    assert!(
        !fake.calls().iter().any(|c| matches!(c, Call::Shell(_))),
        "{:?}",
        fake.calls()
    );
}

#[tokio::test(start_paused = true)]
async fn apps_lists_only_the_scope_and_changes_to_other_apps_are_refused() {
    let (d, fake, _home) = setup(vec![mine(&["Save"])]);
    let r = ok(&d, "apps", json!({})).await;
    let ids: Vec<&str> = r
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["com.mine", "com.other"], "{r}");
    for action in ["clear", "uninstall", "stop"] {
        let e = err(&d, "app", json!({"action": action, "app": "Chat"})).await;
        assert_eq!(e.code, -32025, "{action}: {e:?}");
    }
    let e = err(
        &d,
        "app",
        json!({"action": "grant", "app": "com.chat", "permissions": ["camera"]}),
    )
    .await;
    assert_eq!(e.code, -32025, "{e:?}");
    ok(&d, "app", json!({"action": "stop", "app": "Mine"})).await;
    let calls = fake.calls();
    assert!(
        !calls
            .iter()
            .any(|c| matches!(c, Call::AppOp(_, a) if a.0 == "com.chat")),
        "{calls:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn logs_are_read_only_for_apps_in_the_scope() {
    let (d, fake, _home) = setup(vec![mine(&["Save"])]);
    fake.answer_logs(LogReply {
        text: "line from mine".into(),
        crash: None,
    });
    fake.answer_log_lines(&["tick"]);
    let e = err(&d, "logs", json!({"app": "Chat"})).await;
    assert_eq!(e.code, -32025, "{e:?}");
    let r = ok(&d, "logs", json!({"app": "Mine"})).await;
    assert_eq!(r["output"], "line from mine");
    let e = err(
        &d,
        "wait",
        json!({"condition": {"log": {"pattern": "tick"}}, "timeout_ms": 500}),
    )
    .await;
    assert_eq!(e.code, -32025, "a scope of two apps needs --app: {e:?}");
    let e = err(
        &d,
        "wait",
        json!({"condition": {"log": {"pattern": "tick", "app": "Chat"}}, "timeout_ms": 500}),
    )
    .await;
    assert_eq!(e.code, -32025, "{e:?}");
}

#[tokio::test(start_paused = true)]
async fn a_scope_of_one_app_reads_its_log_without_naming_it() {
    let (d, fake, _home) = setup_as("android:fake", vec![mine(&["Save"])], Some("com.mine"));
    fake.answer_log_lines(&["tick"]);
    ok(
        &d,
        "wait",
        json!({"condition": {"log": {"pattern": "tick"}}, "timeout_ms": 500}),
    )
    .await;
    assert!(
        fake.calls()
            .iter()
            .any(|c| matches!(c, Call::LogWait(Some(a), _) if a.0 == "com.mine")),
        "{:?}",
        fake.calls()
    );
}

#[tokio::test(start_paused = true)]
async fn a_screenshot_is_refused_while_a_window_of_another_app_is_on_screen() {
    let (d, fake, _home) = setup(vec![screen(vec![
        app(1, "com.mine", &["Save"]),
        heads_up(),
    ])]);
    let e = err(&d, "screenshot", json!({})).await;
    assert_eq!(e.code, -32025, "{e:?}");
    assert!(e.message.contains("com.android.systemui"), "{}", e.message);
    assert!(!e.to_json().to_string().contains(SECRET), "{e:?}");
    let r = ok(&d, "snapshot", json!({})).await;
    let text = r["text"].as_str().unwrap();
    assert!(!text.contains(SECRET), "{text}");
    let e = err(&d, "act", tap_point()).await;
    assert_eq!(e.code, -32025, "{e:?}");
    fake.set_tree(mine(&["Save"]));
    let shot = ok(&d, "screenshot", json!({})).await;
    assert!(shot["path"].as_str().is_some(), "{shot}");
}

#[tokio::test(start_paused = true)]
async fn a_notification_that_arrives_while_the_picture_is_taken_is_not_returned() {
    let (d, fake, _home) = setup(vec![mine(&["Save"])]);
    ok(&d, "snapshot", json!({})).await;
    fake.after_screenshot(vec![screen(vec![
        app(1, "com.mine", &["Save"]),
        heads_up(),
    ])]);
    let e = err(&d, "screenshot", json!({})).await;
    assert_eq!(e.code, -32025, "{e:?}");
}

#[tokio::test(start_paused = true)]
async fn an_iphone_takes_no_screenshot_under_a_scope_and_an_unnamed_app_is_outside_it() {
    let (d, _fake, _home) = setup_as("ios:UDID", vec![mine(&["Save"])], Some("com.mine"));
    let e = err(&d, "screenshot", json!({})).await;
    assert_eq!(e.code, -32025, "{e:?}");
    assert!(e.message.contains("snapshot"), "{}", e.message);
    let nameless = RawTree::from_ios_json(
        r#"{"nodes":[{"p":-1,"cls":"XCUIElementTypeApplication","b":[0,0,360,800]},
        {"p":0,"cls":"XCUIElementTypeStaticText","text":"secret-ios","b":[0,0,100,50]}]}"#,
    )
    .unwrap();
    let mut nameless = nameless;
    nameless.windows.push(phonedriver_core::raw::RawWindow {
        id: 0,
        kind: "application".into(),
        package: None,
        layer: 0,
        bounds: phonedriver_core::geometry::Rect {
            x: 0.0,
            y: 0.0,
            w: 360.0,
            h: 800.0,
        },
    });
    let (d, _fake, _home) = setup_as("ios:UDID", vec![nameless], Some("com.mine"));
    let r = ok(&d, "snapshot", json!({})).await;
    let text = r["text"].as_str().unwrap();
    assert!(!text.contains("secret-ios"), "{text}");
    assert!(text.contains("outside the app scope"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn the_clipboard_is_never_read_under_a_scope() {
    let (d, _fake, _home) = setup(vec![mine(&["Save"])]);
    let e = err(&d, "clipboard", json!({})).await;
    assert_eq!(e.code, -32025, "{e:?}");
    ok(&d, "clipboard", json!({"set": "hello"})).await;
}

#[tokio::test(start_paused = true)]
async fn the_web_page_is_read_only_while_the_screen_holds_apps_in_the_scope() {
    let (d, fake, _home) = setup(vec![chat()]);
    fake.show_page(phonedriver_core::device::WebPage {
        title: SECRET.into(),
        url: "https://x".into(),
        text: SECRET.into(),
    });
    let e = err(&d, "page", json!({})).await;
    assert_eq!(e.code, -32025, "{e:?}");
    assert!(!e.to_json().to_string().contains(SECRET), "{e:?}");
}

#[tokio::test(start_paused = true)]
async fn recording_and_saving_the_raw_tree_are_refused() {
    let (d, fake, _home) = setup(vec![mine(&["Save"])]);
    fake.with_recorder();
    let e = err(&d, "record", json!({"action": "start"})).await;
    assert_eq!(e.code, -32025, "{e:?}");
    let e = err(&d, "snapshot", json!({"capture": true})).await;
    assert_eq!(e.code, -32025, "{e:?}");
}

#[tokio::test(start_paused = true)]
async fn the_networks_name_an_app_outside_the_scope_only_as_one() {
    use phonedriver_core::network::{NetworkLink, NetworkView};
    let (d, fake, _home) = setup(vec![mine(&["Save"])]);
    fake.answer_networks(vec![NetworkView {
        networks: vec![NetworkLink {
            id: 1,
            transports: vec!["vpn".into()],
            owner: Some("com.secret.vpn".into()),
            ..Default::default()
        }],
        ..Default::default()
    }]);
    let r = ok(&d, "network", json!({})).await;
    assert!(!r.to_string().contains("com.secret.vpn"), "{r}");
    assert!(r.to_string().contains("outside the app scope"), "{r}");
}

#[tokio::test(start_paused = true)]
async fn a_whole_session_with_a_chat_app_in_front_lets_none_of_its_text_out() {
    let (d, fake, home) = setup(vec![chat()]);
    fake.show_page(phonedriver_core::device::WebPage {
        title: SECRET.into(),
        url: "https://x".into(),
        text: SECRET.into(),
    });
    fake.answer_logs(LogReply {
        text: SECRET.into(),
        crash: None,
    });
    fake.answer_shell(ShellReply {
        stdout: SECRET.into(),
        ..Default::default()
    });
    let mut replies: Vec<String> = Vec::new();
    let mut record = |r: Result<Value, RpcError>| match r {
        Ok(v) => replies.push(v.to_string()),
        Err(e) => replies.push(e.to_json().to_string()),
    };
    let call = |m: &'static str, p: Value| {
        let d = d.clone();
        async move { d.call("default", m, p).await }
    };
    record(call("snapshot", json!({})).await);
    record(call("snapshot", json!({"diff": true})).await);
    record(call("snapshot", json!({"interactive": true, "pixels": true})).await);
    record(call("find", json!({"text": "Reply"})).await);
    record(call("find", json!({"text": "Reply"})).await);
    record(
        call(
            "wait",
            json!({"condition": {"appears": {"text": "Reply"}}, "timeout_ms": 200}),
        )
        .await,
    );
    record(
        call(
            "wait",
            json!({"condition": {"gone": {"text": "Reply"}}, "timeout_ms": 200}),
        )
        .await,
    );
    record(call("screenshot", json!({"inline": true})).await);
    record(call("screenshot", json!({"marks": true})).await);
    record(call("page", json!({})).await);
    record(call("logs", json!({})).await);
    record(call("logs", json!({"app": "Chat"})).await);
    record(call("shell", json!({"command": "dumpsys window"})).await);
    record(call("clipboard", json!({})).await);
    record(call("act", tap_point()).await);
    record(
        call(
            "act",
            act(json!({"kind": "tap", "target": {"text": "Reply"}})),
        )
        .await,
    );
    record(call("act", act(json!({"kind": "tap", "target": {"ref": "e1"}}))).await);
    record(
        call(
            "act",
            act(json!({"kind": "type", "text": "x", "into": {"text": "Reply"}})),
        )
        .await,
    );
    record(
        call(
            "act",
            act(json!({"kind": "scroll", "direction": "down", "until": {"text": "Reply"}})),
        )
        .await,
    );
    record(call("act", act(json!({"kind": "launch", "app": "Chat"}))).await);
    record(call("act", act(json!({"kind": "press", "key": "back"}))).await);
    record(
        call(
            "batch",
            json!({"steps": [{"kind": "tap", "target": {"text": "Reply"}}, {"kind": "press", "key": "back"}]}),
        )
        .await,
    );
    record(call("apps", json!({})).await);
    record(call("journal", json!({})).await);
    for (i, r) in replies.iter().enumerate() {
        assert!(!r.contains(SECRET), "reply {i} leaks: {r}");
    }
    let on_disk = everything_under(home.path());
    assert!(!on_disk.contains(SECRET), "a file under the home holds it");
    assert!(on_disk.contains("snapshot"), "the trace was written");
}

#[tokio::test(start_paused = true)]
async fn a_client_that_asks_for_a_scope_the_daemon_was_not_started_with_is_refused() {
    let (d, _fake, _home) = setup(vec![mine(&["Save"])]);
    let version = phonedriver::wire::VERSION;
    let hello = |scope: &str| json!({"version": version, "client": "cli", "app_scope": scope});
    ok(&d, "hello", hello("com.mine,com.other")).await;
    ok(&d, "hello", hello("")).await;
    let e = err(&d, "hello", hello("com.mine")).await;
    assert_eq!(e.code, phonedriver::daemon::rpc::SCOPE_MISMATCH, "{e:?}");
    assert!(e.message.contains("com.mine,com.other"), "{}", e.message);
    let (d, _fake, _home) = setup_as("android:fake", vec![mine(&["Save"])], None);
    let e = err(&d, "hello", hello("com.mine")).await;
    assert_eq!(e.code, phonedriver::daemon::rpc::SCOPE_MISMATCH, "{e:?}");
    assert!(e.message.contains("without an app scope"), "{}", e.message);
}

#[tokio::test(start_paused = true)]
async fn without_a_scope_nothing_changes() {
    let (d, fake, _home) = setup_as("android:fake", vec![chat()], None);
    let r = ok(&d, "snapshot", json!({})).await;
    assert!(r["text"].as_str().unwrap().contains(SECRET));
    ok(&d, "shell", json!({"command": "id"})).await;
    ok(&d, "act", tap_point()).await;
    assert_eq!(gestures(&fake), 1);
    let apps: Vec<App> = serde_json::from_value(ok(&d, "apps", json!({})).await).unwrap();
    assert_eq!(apps.len(), 4);
}

fn opens(fake: &FakeDevice) -> Vec<String> {
    fake.calls()
        .iter()
        .filter_map(|c| match c {
            Call::OpenUrl(u, _) => Some(u.0.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn a_scoped_open_of_an_intent_url_that_names_another_component_is_refused() {
    let (d, fake, _home) = setup(vec![mine(&["Save"])]);
    for url in [
        "intent:#Intent;component=com.android.chrome/com.google.android.apps.chrome.Main;S.x=1;end",
        "intent://x#Intent;scheme=mine;SEL;component=com.chat/.Main;end",
        "INTENT:#Intent;component=com.chat/.Main;end",
    ] {
        let e = err(
            &d,
            "act",
            act(json!({"kind": "open", "url": url, "app": "Mine"})),
        )
        .await;
        assert_eq!(e.code, -32025, "{url}: {e:?}");
        assert!(e.message.contains("component"), "{}", e.message);
    }
    assert!(opens(&fake).is_empty(), "{:?}", fake.calls());
    ok(
        &d,
        "act",
        act(json!({"kind": "open", "url": "intent://home#Intent;scheme=mine;end", "app": "Mine"})),
    )
    .await;
    assert_eq!(opens(&fake), ["intent://home#Intent;scheme=mine;end"]);
}

#[tokio::test(start_paused = true)]
async fn an_iphone_or_simulator_opens_no_link_under_a_scope_because_it_ignores_the_app() {
    for id in ["ios:UDID", "sim:UDID"] {
        let (d, fake, _home) = setup_as(id, vec![mine(&["Save"])], Some("com.mine"));
        let e = err(
            &d,
            "act",
            act(json!({"kind": "open", "url": "mine://home", "app": "Mine"})),
        )
        .await;
        assert_eq!(e.code, -32025, "{id}: {e:?}");
        assert!(e.message.contains("launch"), "{}", e.message);
        assert!(opens(&fake).is_empty(), "{id}: {:?}", fake.calls());
    }
}

#[tokio::test(start_paused = true)]
async fn an_empty_tree_is_outside_the_scope_until_a_tree_names_the_app() {
    let empty = RawTree::from_android_json(r#"{"windows":[],"nodes":[]}"#, Scale { density: 1.0 })
        .unwrap();
    let (d, fake, _home) = setup(vec![empty]);
    let e = err(&d, "act", act(json!({"kind": "press", "key": "enter"}))).await;
    assert_eq!(e.code, -32025, "{e:?}");
    let e = err(&d, "act", tap_point()).await;
    assert_eq!(e.code, -32025, "{e:?}");
    let e = err(&d, "screenshot", json!({})).await;
    assert_eq!(e.code, -32025, "{e:?}");
    assert!(
        !fake
            .calls()
            .iter()
            .any(|c| matches!(c, Call::Gesture(_) | Call::Key(_))),
        "{:?}",
        fake.calls()
    );
    fake.set_tree(mine(&["Save"]));
    ok(&d, "screenshot", json!({})).await;
}

#[tokio::test(start_paused = true)]
async fn the_page_is_read_from_the_app_the_check_approved() {
    let (d, fake, _home) = setup(vec![mine(&["Save"])]);
    fake.show_page(phonedriver_core::device::WebPage {
        title: "Mine".into(),
        url: "https://mine".into(),
        text: "hello".into(),
    });
    ok(&d, "page", json!({})).await;
    let asked: Vec<Option<String>> = fake
        .calls()
        .iter()
        .filter_map(|c| match c {
            Call::Page(app) => Some(app.as_ref().map(|a| a.0.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(asked, [Some("com.mine".to_string())]);
}

#[tokio::test(start_paused = true)]
async fn a_bar_shaped_window_of_another_app_is_outside() {
    let overlay = Win {
        id: 4,
        kind: "system",
        package: "com.chat",
        bounds: [0, 0, 360, 60],
        labels: vec![SECRET],
    };
    let (d, fake, _home) = setup(vec![screen(vec![app(1, "com.mine", &["Save"]), overlay])]);
    let e = err(&d, "screenshot", json!({})).await;
    assert_eq!(e.code, -32025, "{e:?}");
    assert!(e.message.contains("com.chat"), "{}", e.message);
    let e = err(&d, "act", tap_point()).await;
    assert_eq!(e.code, -32025, "{e:?}");
    assert_eq!(gestures(&fake), 0);
}

#[tokio::test(start_paused = true)]
async fn on_an_iphone_back_and_hiding_the_keyboard_are_refused_while_another_app_is_in_front() {
    let (d, fake, _home) = setup_as("ios:UDID", vec![chat()], Some("com.mine"));
    let e = err(&d, "act", act(json!({"kind": "press", "key": "back"}))).await;
    assert_eq!(e.code, -32025, "{e:?}");
    assert!(e.message.contains("press home"), "{}", e.message);
    assert!(
        !fake
            .calls()
            .iter()
            .any(|c| matches!(c, Call::Gesture(_) | Call::Button(_))),
        "{:?}",
        fake.calls()
    );
    ok(&d, "act", act(json!({"kind": "press", "key": "home"}))).await;
    fake.set_tree(mine(&["Save"]));
    ok(&d, "act", act(json!({"kind": "press", "key": "back"}))).await;
    assert!(
        fake.calls().iter().any(|c| matches!(
            c,
            Call::Button(phonedriver_core::device::Button::Back)
        )),
        "{:?}",
        fake.calls()
    );
}
