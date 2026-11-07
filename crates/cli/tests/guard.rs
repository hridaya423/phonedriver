//! Decision D27: a tap that spends money or cannot be undone needs the human's word, asked by phone-use itself.

#[path = "support/fakes.rs"]
mod fakes;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fakes::{OneDevice, PNG, Pixels};
use phone_use::client::{Client, Failure};
use phone_use::daemon::{Config, Daemon, socket};
use phone_use::mcp::Mcp;
use phone_use_core::device::{Capabilities, Device, Gesture};
use phone_use_core::geometry::Scale;
use phone_use_core::raw::RawTree;
use phone_use_core::testing::{Call, FakeDevice};
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ClientCapabilities, ClientConfig, ElicitRequestParams,
    ElicitResult, ElicitationAction, Implementation, ProtocolVersion,
};
use rmcp::service::{RequestContext, RunningService};
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use serde_json::{Value, json};

const PLACE_ORDER: &str = "Place order · $34.10";

fn checkout() -> RawTree {
    let node = |cls: &str, text: &str, top: i32, flags: &str, acts: &str| {
        format!(
            r#"{{"p":0,"w":1,"cls":"{cls}","text":"{text}","b":[0,{top},360,{}],"f":"{flags}","a":[{acts}]}}"#,
            top + 50
        )
    };
    let nodes = [
        r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}"#
            .to_string(),
        node("android.widget.TextView", "2 x Pad thai", 100, "vVe", ""),
        node(
            "android.widget.TextView",
            "Deliver to 12 Elm St",
            150,
            "vVe",
            "",
        ),
        node("android.widget.TextView", "Total $34.10", 200, "vVe", ""),
        node("android.widget.Button", "Back", 300, "vVe", r#""tap""#),
        node("android.widget.Button", PLACE_ORDER, 400, "vVe", r#""tap""#),
        node(
            "android.widget.EditText",
            "",
            500,
            "vVeEF",
            r#""tap","set_text""#,
        ),
    ];
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example.shop","bounds":[0,0,360,800]}}],"nodes":[{}]}}"#,
        nodes.join(",")
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

struct Rig {
    fake: Arc<FakeDevice>,
    socket: PathBuf,
    _home: tempfile::TempDir,
}

async fn rig() -> Rig {
    let fake = Arc::new(FakeDevice::new(vec![checkout()]));
    let mut info = fake.info().clone();
    info.capabilities |= Capabilities::SCREENSHOT;
    let device: Arc<dyn Device> = Arc::new(Pixels(fake.clone(), info));
    let home = tempfile::tempdir().unwrap();
    let daemon = Daemon::new(
        Arc::new(OneDevice(device)),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    let socket = home.path().join("run").join("daemon.sock");
    let listener = socket::listen(&socket).await.unwrap();
    tokio::spawn(daemon.serve_on(listener));
    Rig {
        fake,
        socket,
        _home: home,
    }
}

impl Rig {
    async fn act(&self, action: Value, approval: Option<&str>) -> Result<Value, Failure> {
        let mut c = Client::connect_to(&self.socket, "guard-test")
            .await
            .unwrap();
        let mut p = json!({"action": action, "settle": false});
        if let Some(a) = approval {
            p["approval"] = json!(a);
        }
        c.call("act", p).await
    }

    fn gestures(&self) -> usize {
        self.fake
            .calls()
            .iter()
            .filter(|c| matches!(c, Call::Gesture(Gesture { .. })))
            .count()
    }
}

fn tap(text: &str) -> Value {
    json!({"kind": "tap", "target": {"text": text}})
}

fn refused(r: Result<Value, Failure>) -> Value {
    match r {
        Err(Failure::Rpc(e)) => {
            assert_eq!(e.code, -32019, "{e:?}");
            e.data
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn a_guarded_tap_is_refused_with_the_app_the_label_and_what_the_screen_says() {
    let rig = rig().await;
    let data = refused(rig.act(tap("Place order"), None).await);
    assert_eq!(data["label"], PLACE_ORDER);
    assert_eq!(data["app"], "com.example.shop");
    let details = data["details"].to_string();
    for want in ["Total $34.10", "Deliver to 12 Elm St", "2 x Pad thai"] {
        assert!(details.contains(want), "{details}");
    }
    assert!(!details.contains("Back"), "{details}");
    assert_eq!(rig.gestures(), 0);
}

#[tokio::test]
async fn an_unguarded_tap_is_not_asked_about() {
    let rig = rig().await;
    let r = rig.act(tap("Back"), None).await.unwrap();
    assert!(!r["summary"].as_str().unwrap().contains("approved"), "{r}");
    assert_eq!(rig.gestures(), 1);
}

#[tokio::test]
async fn once_runs_one_tap_and_says_so_and_is_not_remembered() {
    let rig = rig().await;
    let r = rig.act(tap("Place order"), Some("once")).await.unwrap();
    assert!(
        r["summary"]
            .as_str()
            .unwrap()
            .contains("approved by the human"),
        "{r}"
    );
    assert_eq!(rig.gestures(), 1);
    refused(rig.act(tap("Place order"), None).await);
    assert_eq!(rig.gestures(), 1);
}

#[tokio::test]
async fn session_approval_covers_later_taps_but_not_a_new_daemon() {
    let rig = rig().await;
    rig.act(tap("Place order"), Some("session")).await.unwrap();
    let r = rig.act(tap("Place order"), None).await.unwrap();
    assert!(
        r["summary"].as_str().unwrap().contains("for this session"),
        "{r}"
    );
    assert_eq!(rig.gestures(), 2);
    let restarted = self::rig().await;
    refused(restarted.act(tap("Place order"), None).await);
    assert_eq!(restarted.gestures(), 0);
}

#[tokio::test]
async fn autonomous_runs_without_a_note_and_is_not_remembered() {
    let rig = rig().await;
    let r = rig
        .act(tap("Place order"), Some("autonomous"))
        .await
        .unwrap();
    assert!(!r["summary"].as_str().unwrap().contains("approved"), "{r}");
    assert_eq!(rig.gestures(), 1);
    refused(rig.act(tap("Place order"), None).await);
}

#[tokio::test]
async fn a_point_on_a_guarded_button_is_guarded() {
    let rig = rig().await;
    let on_button = json!({"kind": "tap", "target": {"point": {"x": 180.0, "y": 420.0}}});
    refused(rig.act(on_button, None).await);
    let elsewhere = json!({"kind": "tap", "target": {"point": {"x": 180.0, "y": 700.0}}});
    rig.act(elsewhere, None).await.unwrap();
    assert_eq!(rig.gestures(), 1);
}

#[tokio::test]
async fn submitting_a_field_next_to_a_guarded_button_is_guarded() {
    let rig = rig().await;
    let snapshot = {
        let mut c = Client::connect_to(&rig.socket, "guard-test").await.unwrap();
        c.call("snapshot", json!({})).await.unwrap()
    };
    assert!(snapshot.to_string().contains("textbox"), "{snapshot}");
    let submit = json!({"kind": "type", "text": "x", "into": {"text": "Pad"}, "submit": true});
    let _ = submit;
    let typed = json!({"kind": "type", "text": "x", "submit": true});
    refused(rig.act(typed.clone(), None).await);
    let without = json!({"kind": "type", "text": "x"});
    rig.act(without, None).await.unwrap();
    rig.act(typed, Some("once")).await.unwrap();
}

#[tokio::test]
async fn a_batch_stops_at_a_guarded_step_unless_autonomous() {
    let rig = rig().await;
    let steps = vec![tap("Back"), tap("Place order")];
    let mut c = Client::connect_to(&rig.socket, "guard-test").await.unwrap();
    let r = c.call("batch", json!({"steps": steps})).await.unwrap();
    assert_eq!(r["failed"]["step"], 2, "{r}");
    assert_eq!(r["failed"]["error"]["code"], -32019, "{r}");
    assert_eq!(rig.gestures(), 1);
    let r = c
        .call("batch", json!({"steps": steps, "approval": "autonomous"}))
        .await
        .unwrap();
    assert!(r["failed"].is_null(), "{r}");
    assert_eq!(rig.gestures(), 3);
}

type Asked = Arc<Mutex<Vec<ElicitRequestParams>>>;

struct Human {
    version: ProtocolVersion,
    answer: Option<(ElicitationAction, Option<Value>)>,
    asked: Asked,
}

impl ClientHandler for Human {
    fn get_info(&self) -> ClientConfig {
        let caps = if self.answer.is_some() {
            ClientCapabilities::builder().enable_elicitation().build()
        } else {
            ClientCapabilities::builder().build()
        };
        ClientConfig::new(caps, Implementation::new("guard-test", "0"))
            .with_protocol_version(self.version.clone())
    }

    async fn create_elicitation(
        &self,
        request: ElicitRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> Result<ElicitResult, rmcp::ErrorData> {
        self.asked.lock().unwrap().push(request);
        let (action, content) = self.answer.clone().expect("a client without elicitation");
        let mut r = ElicitResult::new(action);
        if let Some(c) = content {
            r = r.with_content(c);
        }
        Ok(r)
    }
}

struct Mcp2 {
    rig: Rig,
    client: RunningService<RoleClient, Human>,
    asked: Asked,
}

async fn mcp(answer: Option<(ElicitationAction, Option<Value>)>, version: ProtocolVersion) -> Mcp2 {
    let rig = rig().await;
    let (client_io, server_io) = tokio::io::duplex(1 << 20);
    let server = Mcp::new(Some(rig.socket.clone()), "mcp-guard".into());
    tokio::spawn(async move {
        let running = server.serve(server_io).await.unwrap();
        let _ = running.waiting().await;
    });
    let asked = Asked::default();
    let client = Human {
        version,
        answer,
        asked: asked.clone(),
    }
    .serve(client_io)
    .await
    .unwrap();
    Mcp2 { rig, client, asked }
}

impl Mcp2 {
    async fn tap(&self, args: Value) -> CallToolResult {
        self.client
            .call_tool(
                CallToolRequestParams::new("tap").with_arguments(args.as_object().unwrap().clone()),
            )
            .await
            .unwrap()
    }
}

fn text(r: &CallToolResult) -> String {
    r.content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn accept(choice: &str) -> Option<(ElicitationAction, Option<Value>)> {
    Some((ElicitationAction::Accept, Some(json!({"choice": choice}))))
}

async fn the_human_allows_and_the_tap_runs_inside_the_same_call(v: ProtocolVersion) {
    let m = mcp(accept("allow"), v).await;
    let r = m.tap(json!({"target": "Place order"})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert!(text(&r).contains("approved by the human"), "{}", text(&r));
    assert_eq!(m.rig.gestures(), 1);
    let first = m.asked.lock().unwrap()[0].clone();
    let ElicitRequestParams::FormElicitationParams {
        message,
        requested_schema,
        ..
    } = &first
    else {
        panic!("a form was expected");
    };
    for want in [
        PLACE_ORDER,
        "com.example.shop",
        "Total $34.10",
        "Deliver to 12 Elm St",
    ] {
        assert!(message.contains(want), "{message}");
    }
    let schema = serde_json::to_string(requested_schema).unwrap();
    assert!(schema.contains("Allow for this session"), "{schema}");
    let again = m.tap(json!({"target": "Place order"})).await;
    assert!(text(&again).contains("approved by the human"));
    assert_eq!(
        m.asked.lock().unwrap().len(),
        2,
        "allow once asks every time"
    );
}

async fn allow_for_this_session_asks_once(v: ProtocolVersion) {
    let m = mcp(accept("session"), v).await;
    for _ in 0..2 {
        let r = m.tap(json!({"target": "Place order"})).await;
        assert!(text(&r).contains("for this session"), "{}", text(&r));
    }
    assert_eq!(m.rig.gestures(), 2);
    assert_eq!(m.asked.lock().unwrap().len(), 1);
}

async fn declining_or_cancelling_taps_nothing_and_says_the_human_declined(v: ProtocolVersion) {
    for action in [ElicitationAction::Decline, ElicitationAction::Cancel] {
        let m = mcp(Some((action, None)), v.clone()).await;
        let r = m.tap(json!({"target": "Place order"})).await;
        assert_ne!(r.is_error, Some(true), "{r:?}");
        assert!(text(&r).contains("The human declined"), "{}", text(&r));
        assert!(text(&r).contains(PLACE_ORDER));
        assert_eq!(m.rig.gestures(), 0);
    }
}

async fn a_client_that_cannot_ask_gets_the_question_and_nothing_is_tapped(v: ProtocolVersion) {
    let m = mcp(None, v).await;
    let r = m.tap(json!({"target": "Place order"})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let t = text(&r);
    assert!(
        t.contains("Needs the human's approval; nothing was tapped"),
        "{t}"
    );
    assert!(
        t.contains("Total $34.10") && t.contains("autonomous: true"),
        "{t}"
    );
    assert_eq!(m.rig.gestures(), 0);
    let r = m
        .tap(json!({"target": "Place order", "autonomous": true}))
        .await;
    assert!(!text(&r).contains("Needs the human"), "{}", text(&r));
    assert_eq!(m.rig.gestures(), 1);
}

async fn autonomous_never_asks(v: ProtocolVersion) {
    let m = mcp(accept("allow"), v).await;
    let r = m
        .tap(json!({"target": "Place order", "autonomous": true}))
        .await;
    assert!(!text(&r).contains("approved"), "{}", text(&r));
    assert_eq!(m.rig.gestures(), 1);
    assert!(m.asked.lock().unwrap().is_empty());
}

macro_rules! on_both_protocols {
    ($($name:ident),*) => {
        mod legacy {
            use super::*;
            $(#[tokio::test]
            async fn $name() {
                super::$name(ProtocolVersion::V_2025_11_25).await
            })*
        }
        mod mrtr {
            use super::*;
            $(#[tokio::test]
            async fn $name() {
                super::$name(ProtocolVersion::V_2026_07_28).await
            })*
        }
    };
}

on_both_protocols!(
    the_human_allows_and_the_tap_runs_inside_the_same_call,
    allow_for_this_session_asks_once,
    declining_or_cancelling_taps_nothing_and_says_the_human_declined,
    a_client_that_cannot_ask_gets_the_question_and_nothing_is_tapped,
    autonomous_never_asks
);

#[allow(dead_code)]
const _: &[u8] = PNG;
