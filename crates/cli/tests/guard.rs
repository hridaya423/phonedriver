//! Decision D27: a tap that spends money or cannot be undone needs the human's word, asked by phonedriver itself.

use crate::fakes;

use std::sync::{Arc, Mutex};

use fakes::PNG;
use phonedriver::client::{Client, Failure};
use phonedriver_core::device::{DeviceEvent, FrameRef};
use phonedriver_core::error::Category;
use phonedriver_core::testing::Call;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ClientCapabilities, ClientConfig, ElicitRequestParams,
    ElicitResult, ElicitationAction, Implementation, ProtocolVersion,
};
use rmcp::service::{ClientLifecycleMode, RequestContext, RunningService};
use rmcp::{ClientHandler, ClientServiceExt, RoleClient, ServiceExt};
use serde_json::{Value, json};

use crate::guard_rig;

use guard_rig::{PLACE_ORDER, Rig, composer, rig, rig_showing};

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

/// Decision D138: a tap looks down a list for text the screen does not show. The button it finds there is held as
/// any other, and until the human's word only the drag has reached the device.
#[tokio::test]
async fn a_guarded_button_that_a_tap_scrolled_to_is_held_before_it_is_tapped() {
    let cart = |rows: [&str; 2]| {
        let row = |text: &str, top: i32| {
            format!(
                r#"{{"p":1,"w":1,"cls":"android.widget.Button","text":"{text}","b":[0,{top},360,{}],"f":"vVe","a":["tap"]}}"#,
                top + 50
            )
        };
        let json = format!(
            r#"{{"windows":[{{"id":1,"type":"application","package":"com.example.shop","bounds":[0,0,360,800]}}],"nodes":[
            {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
            {{"p":0,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,100,360,700],"f":"vVeS","a":["scroll_down"]}},
            {},{}]}}"#,
            row(rows[0], 100),
            row(rows[1], 150)
        );
        phonedriver_core::raw::RawTree::from_android_json(
            &json,
            phonedriver_core::geometry::Scale { density: 1.0 },
        )
        .unwrap()
    };
    let rig = rig_showing(cart(["2 x Pad thai", "Total $34.10"])).await;
    let below = cart(["Total $34.10", PLACE_ORDER]);
    let dragged = |g: &phonedriver_core::device::Gesture| {
        let path = &g.fingers[0];
        matches!((path.first(), path.last()), (Some(a), Some(b)) if (a.at.y - b.at.y).abs() > 20.0)
    };
    rig.fake
        .on_gesture(move |g| dragged(g).then(|| vec![below.clone()]));
    let mut c = Client::connect_to(&rig.socket, "guard-test").await.unwrap();
    let held = refused(c.call("act", json!({"action": tap("Place order")})).await);
    assert_eq!(held["label"], PLACE_ORDER);
    let touches = |rig: &Rig| -> Vec<bool> {
        rig.fake
            .calls()
            .iter()
            .filter_map(|c| match c {
                Call::Gesture(g) => Some(dragged(g)),
                _ => None,
            })
            .collect()
    };
    assert_eq!(touches(&rig), [true], "one drag and no tap");
    let r = c
        .call(
            "act",
            json!({"action": tap("Place order"), "approval": "once"}),
        )
        .await
        .unwrap();
    assert!(r["summary"].as_str().unwrap().contains("approved"), "{r}");
    assert_eq!(touches(&rig), [true, false], "the tap, with no more drags");
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

#[tokio::test]
async fn a_refusal_names_the_category_that_triggered_it() {
    let rig = rig().await;
    let data = refused(rig.act(tap("Place order"), None).await);
    assert_eq!(data["category"], "payment");
}

#[tokio::test]
async fn opt_in_categories_hold_a_tap_only_when_asked_for() {
    let rig = rig().await;
    let table = [
        ("Send", "send"),
        ("Delete photo", "delete"),
        ("Install", "install"),
    ];
    for (label, category) in table {
        let before = rig.gestures();
        rig.act(tap(label), None).await.unwrap();
        assert_eq!(
            rig.gestures(),
            before + 1,
            "{label} is not asked by default"
        );
        let others: Vec<&str> = table
            .iter()
            .map(|t| t.1)
            .filter(|c| *c != category)
            .collect();
        rig.act_asking(tap(label), None, &others).await.unwrap();
        assert_eq!(rig.gestures(), before + 2, "{label} with {others:?}");
        let data = refused(rig.act_asking(tap(label), None, &[category]).await);
        assert_eq!(data["category"], category);
        assert_eq!(data["label"], label);
        assert_eq!(
            rig.gestures(),
            before + 2,
            "{label} was tapped when asked for"
        );
        let r = rig
            .act_asking(tap(label), Some("once"), &[category])
            .await
            .unwrap();
        assert!(
            r["summary"]
                .as_str()
                .unwrap()
                .contains("approved by the human")
        );
    }
}

#[tokio::test]
async fn payments_are_asked_whatever_is_turned_on() {
    let rig = rig().await;
    refused(rig.act_asking(tap("Place order"), None, &["send"]).await);
    refused(rig.act_asking(tap("Place order"), None, &[]).await);
}

#[tokio::test]
async fn a_session_approval_covers_only_the_category_it_was_given_for() {
    let rig = rig().await;
    rig.act(tap("Place order"), Some("session")).await.unwrap();
    rig.act(tap("Place order"), None).await.unwrap();
    let data = refused(rig.act_asking(tap("Send"), None, &["send"]).await);
    assert_eq!(data["category"], "send");
    rig.act_asking(tap("Send"), Some("session"), &["send"])
        .await
        .unwrap();
    rig.act_asking(tap("Send"), None, &["send"]).await.unwrap();
    refused(rig.act_asking(tap("Delete photo"), None, &["delete"]).await);
}

#[tokio::test]
async fn submitting_a_form_with_a_send_button_is_held_when_send_is_asked_for() {
    let rig = rig().await;
    let mut c = Client::connect_to(&rig.socket, "guard-test").await.unwrap();
    c.call("snapshot", json!({})).await.unwrap();
    let typed = json!({"kind": "type", "text": "x", "submit": true});
    refused(rig.act(typed.clone(), None).await);
    let data = refused(rig.act_asking(typed, None, &["send"]).await);
    assert_eq!(
        data["category"], "payment",
        "the payment button comes first"
    );
}

#[tokio::test]
async fn submitting_a_message_is_held_only_when_send_is_asked_for() {
    let rig = rig_showing(composer()).await;
    let mut c = Client::connect_to(&rig.socket, "guard-test").await.unwrap();
    c.call("snapshot", json!({})).await.unwrap();
    let typed = json!({"kind": "type", "text": "hi", "submit": true});
    rig.act(typed.clone(), None).await.unwrap();
    let data = refused(rig.act_asking(typed, None, &["send"]).await);
    assert_eq!(data["category"], "send");
    assert_eq!(data["label"], "Send");
}

#[tokio::test]
async fn a_batch_carries_the_categories_asked_for() {
    let rig = rig().await;
    let steps = vec![tap("Back"), tap("Send")];
    let mut c = Client::connect_to(&rig.socket, "guard-test").await.unwrap();
    let r = c
        .call("batch", json!({"steps": steps, "ask": ["send"]}))
        .await
        .unwrap();
    assert_eq!(r["failed"]["step"], 2, "{r}");
    assert_eq!(r["failed"]["error"]["code"], -32019, "{r}");
    assert_eq!(rig.gestures(), 1);
    let r = c.call("batch", json!({"steps": steps})).await.unwrap();
    assert!(r["failed"].is_null(), "{r}");
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
    mcp_asking(answer, version, vec![]).await
}

async fn mcp_asking(
    answer: Option<(ElicitationAction, Option<Value>)>,
    version: ProtocolVersion,
    ask: Vec<Category>,
) -> Mcp2 {
    let rig = rig().await;
    let (client_io, server_io) = tokio::io::duplex(1 << 20);
    let server = rig.mcp().asking(ask);
    tokio::spawn(async move {
        let running = server.serve(server_io).await.unwrap();
        let _ = running.waiting().await;
    });
    let asked = Asked::default();
    let lifecycle = if version >= ProtocolVersion::V_2026_07_28 {
        ClientLifecycleMode::Discover {
            preferred_versions: vec![version.clone()],
        }
    } else {
        ClientLifecycleMode::Initialize
    };
    let client = Human {
        version,
        answer,
        asked: asked.clone(),
    }
    .serve_with_lifecycle(client_io, lifecycle)
    .await
    .unwrap();
    Mcp2 { rig, client, asked }
}

impl Mcp2 {
    async fn batch(&self, steps: Vec<Value>) -> CallToolResult {
        let args = json!({ "steps": steps });
        self.client
            .call_tool(
                CallToolRequestParams::new("batch")
                    .with_arguments(args.as_object().unwrap().clone()),
            )
            .await
            .unwrap()
    }

    async fn call(&self, tool: &'static str, args: Value) -> CallToolResult {
        self.client
            .call_tool(
                CallToolRequestParams::new(tool).with_arguments(args.as_object().unwrap().clone()),
            )
            .await
            .unwrap()
    }

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

async fn declining_taps_nothing_and_says_the_human_declined(v: ProtocolVersion) {
    let m = mcp(Some((ElicitationAction::Decline, None)), v).await;
    let r = m.tap(json!({"target": "Place order"})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert!(text(&r).contains("The human declined"), "{}", text(&r));
    assert!(text(&r).contains(PLACE_ORDER));
    assert!(!text(&r).contains("autonomous"), "{}", text(&r));
    assert_eq!(m.rig.gestures(), 0);
}

async fn a_dismissed_prompt_is_no_answer_and_names_the_way_forward(v: ProtocolVersion) {
    let m = mcp(Some((ElicitationAction::Cancel, None)), v).await;
    let r = m.tap(json!({"target": "Place order"})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let t = text(&r);
    assert!(!t.contains("The human declined"), "{t}");
    assert!(
        t.contains("Needs the human's approval; nothing was tapped") && t.contains(PLACE_ORDER),
        "{t}"
    );
    assert!(
        t.contains("no answer") && t.contains("autonomous: true"),
        "{t}"
    );
    assert_eq!(m.rig.gestures(), 0);
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

async fn the_prompt_names_the_category_that_triggered_it(v: ProtocolVersion) {
    let m = mcp_asking(accept("allow"), v, vec![Category::Send, Category::Install]).await;
    m.tap(json!({"target": "Send"})).await;
    m.tap(json!({"target": "Place order"})).await;
    m.tap(json!({"target": "Delete photo"})).await;
    let asked = m.asked.lock().unwrap().clone();
    let messages: Vec<String> = asked
        .iter()
        .map(|a| match a {
            ElicitRequestParams::FormElicitationParams { message, .. } => message.clone(),
            _ => panic!("a form was expected"),
        })
        .collect();
    assert_eq!(messages.len(), 2, "{messages:?}: delete was not turned on");
    assert!(messages[0].contains("Category: send"), "{}", messages[0]);
    assert!(messages[0].contains("Tap \"Send\""), "{}", messages[0]);
    assert!(messages[1].contains("Category: payment"), "{}", messages[1]);
    assert_eq!(m.rig.gestures(), 3);
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

fn step(target: &str) -> Value {
    json!({"tool": "tap", "target": target})
}

async fn a_held_step_in_a_batch_is_asked_and_the_batch_resumes_there(v: ProtocolVersion) {
    let m = mcp(accept("allow"), v).await;
    let r = m.batch(vec![step("Back"), step("Place order")]).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert!(text(&r).contains("approved by the human"), "{}", text(&r));
    assert_eq!(m.asked.lock().unwrap().len(), 1);
    assert_eq!(m.rig.gestures(), 2, "the step before the held one ran once");
}

async fn declining_a_held_batch_step_stops_the_batch_there(v: ProtocolVersion) {
    let m = mcp(Some((ElicitationAction::Decline, None)), v).await;
    let r = m
        .batch(vec![step("Back"), step("Place order"), step("Back")])
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert!(text(&r).contains("The human declined"), "{}", text(&r));
    assert_eq!(m.rig.gestures(), 1);
}

async fn an_approval_in_a_batch_covers_only_the_step_it_was_asked_for(v: ProtocolVersion) {
    let m = mcp(accept("allow"), v).await;
    let r = m
        .batch(vec![step("Place order"), step("Back"), step("Place order")])
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert_eq!(m.asked.lock().unwrap().len(), 2);
    assert_eq!(m.rig.gestures(), 3);
}

async fn a_then_tap_that_needs_approval_is_asked_and_the_chain_goes_on(v: ProtocolVersion) {
    let m = mcp(accept("allow"), v).await;
    let r = m
        .tap(json!({"target": "Back", "then": ["Place order"]}))
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert!(text(&r).contains("approved by the human"), "{}", text(&r));
    assert_eq!(m.asked.lock().unwrap().len(), 1);
    assert_eq!(m.rig.gestures(), 2);
}

fn zero_swipe() -> Value {
    json!({"direction": "up", "from": "Place order", "distance": 0, "fast": true})
}

async fn a_held_swipe_press_or_clear_is_asked_and_runs_inside_the_same_call(v: ProtocolVersion) {
    let m = mcp(accept("allow"), v).await;
    let r = m.call("swipe", zero_swipe()).await;
    assert!(text(&r).contains("approved by the human"), "{}", text(&r));
    assert_eq!(m.rig.gestures(), 1);
    let r = m.call("press", json!({"key": "enter"})).await;
    assert!(text(&r).contains("approved by the human"), "{}", text(&r));
    let r = m
        .call("type", json!({"text": "", "into": "Place order"}))
        .await;
    assert!(text(&r).contains("approved by the human"), "{}", text(&r));
    assert_eq!(m.asked.lock().unwrap().len(), 3);
}

async fn a_declined_swipe_sends_no_touch(v: ProtocolVersion) {
    let m = mcp(Some((ElicitationAction::Decline, None)), v).await;
    let r = m.call("swipe", zero_swipe()).await;
    assert!(text(&r).contains("The human declined"), "{}", text(&r));
    let r = m
        .call("scroll", json!({"direction": "down", "in": "Place order"}))
        .await;
    assert!(text(&r).contains("The human declined"), "{}", text(&r));
    assert_eq!(m.rig.gestures(), 0);
}

async fn a_client_that_cannot_ask_gets_the_question_for_a_swipe_too(v: ProtocolVersion) {
    let m = mcp(None, v).await;
    let r = m.call("swipe", zero_swipe()).await;
    assert!(text(&r).contains("autonomous: true"), "{}", text(&r));
    assert_eq!(m.rig.gestures(), 0);
    let mut autonomous = zero_swipe();
    autonomous["autonomous"] = json!(true);
    m.call("swipe", autonomous).await;
    assert_eq!(m.rig.gestures(), 1);
}

/// `autonomous` is the agent's word that the user approved, and holds for every touch tool, so a batch and a `then`
/// chain take it too: the batch tool refused the argument, and a chain dropped it on the floor.
async fn autonomous_holds_for_a_batch_and_a_then_chain(v: ProtocolVersion) {
    let m = mcp(None, v).await;
    let r = m
        .call(
            "batch",
            json!({"steps": [step("Back"), step("Place order")], "autonomous": true}),
        )
        .await;
    assert!(!text(&r).contains("Needs the human"), "{}", text(&r));
    assert_eq!(m.rig.gestures(), 2);
    let r = m
        .tap(json!({"target": "Back", "then": ["Place order"], "autonomous": true}))
        .await;
    assert!(!text(&r).contains("Needs the human"), "{}", text(&r));
    assert_eq!(m.rig.gestures(), 4);
    let r = m
        .call(
            "type",
            json!({"text": "x", "into": "Place order", "then": ["Back"], "autonomous": true}),
        )
        .await;
    assert!(!text(&r).contains("Needs the human"), "{}", text(&r));
    let r = m
        .call(
            "batch",
            json!({"steps": [{"tool": "tap", "target": "Place order", "autonomous": true}]}),
        )
        .await;
    assert!(text(&r).contains("on the batch"), "{}", text(&r));
    assert!(m.asked.lock().unwrap().is_empty());
}

async fn empty_text_clears_the_field_on_either_protocol(v: ProtocolVersion) {
    let m = mcp(None, v).await;
    m.call("type", json!({"text": "", "into": "180,525"})).await;
    let cleared = m
        .rig
        .fake
        .calls()
        .iter()
        .any(|c| matches!(c, Call::Text(phonedriver_core::device::TextInput::SetValue(t)) if t.is_empty()));
    assert!(cleared, "{:?}", m.rig.fake.calls());
}

/// A serde error quotes the value it could not read, and a misplaced secret is exactly that value.
async fn a_malformed_argument_is_named_without_its_value(v: ProtocolVersion) {
    let m = mcp(None, v).await;
    let said = |r: Result<CallToolResult, rmcp::ServiceError>| match r {
        Ok(r) => text(&r),
        Err(e) => e.to_string(),
    };
    let r = m
        .client
        .call_tool(
            CallToolRequestParams::new("type").with_arguments(
                json!({"text": "x", "secret": "hunter2pass"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await;
    let t = said(r);
    assert!(!t.contains("hunter2pass"), "{t}");
    let r = m
        .client
        .call_tool(
            CallToolRequestParams::new("batch").with_arguments(
                json!({"steps": [{"tool": "type", "text": "x", "into": "Field", "secret": "hunter2pass"}]})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await;
    let t = said(r);
    assert!(!t.contains("hunter2pass"), "{t}");
    assert!(t.contains("expected"), "{t}");
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
    declining_taps_nothing_and_says_the_human_declined,
    a_dismissed_prompt_is_no_answer_and_names_the_way_forward,
    a_client_that_cannot_ask_gets_the_question_and_nothing_is_tapped,
    the_prompt_names_the_category_that_triggered_it,
    autonomous_never_asks,
    a_held_step_in_a_batch_is_asked_and_the_batch_resumes_there,
    declining_a_held_batch_step_stops_the_batch_there,
    an_approval_in_a_batch_covers_only_the_step_it_was_asked_for,
    a_then_tap_that_needs_approval_is_asked_and_the_chain_goes_on,
    a_held_swipe_press_or_clear_is_asked_and_runs_inside_the_same_call,
    a_declined_swipe_sends_no_touch,
    a_client_that_cannot_ask_gets_the_question_for_a_swipe_too,
    autonomous_holds_for_a_batch_and_a_then_chain,
    empty_text_clears_the_field_on_either_protocol,
    a_malformed_argument_is_named_without_its_value
);

#[allow(dead_code)]
const _: &[u8] = PNG;

fn point(y: f64) -> Value {
    json!({"kind": "tap", "target": {"point": {"x": 180.0, "y": y}}})
}

#[tokio::test(start_paused = true)]
async fn point_taps_in_a_row_read_the_screen_once_until_the_screen_is_reshaped() {
    let rig = rig().await;
    rig.act(point(700.0), None).await.unwrap();
    rig.act(point(700.0), None).await.unwrap();
    rig.act(point(720.0), None).await.unwrap();
    assert_eq!(rig.fake.tree_reads(), 1);
    rig.fake.push_frame(FrameRef {
        seq: 1,
        changed_area_pt2: 10_000.0,
        changed_box: None,
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    rig.act(point(700.0), None).await.unwrap();
    assert_eq!(rig.fake.tree_reads(), 1);
    rig.fake.push_frame(FrameRef {
        seq: 2,
        changed_area_pt2: 1_000_000.0,
        changed_box: None,
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    rig.act(point(700.0), None).await.unwrap();
    assert_eq!(rig.fake.tree_reads(), 2);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    rig.fake.emit(DeviceEvent::WindowChanged { package: None });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    rig.act(point(700.0), None).await.unwrap();
    assert_eq!(rig.fake.tree_reads(), 3);
    refused(rig.act(point(420.0), None).await);
}

fn transfer(enabled: bool, focused: bool) -> phonedriver_core::raw::RawTree {
    let pay = if enabled { "vVe" } else { "vV" };
    let field = if focused { "vVeEF" } else { "vVeE" };
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example.bank","bounds":[0,0,360,800]}}],"nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.widget.TextView","text":"Total €250.00","b":[0,100,360,150],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.widget.EditText","text":"","id":"iban","b":[0,300,360,350],"f":"{field}","a":["tap","set_text"]}},
        {{"p":0,"w":1,"cls":"android.widget.Button","text":"Pay €250.00","b":[0,400,360,450],"f":"{pay}","a":["tap"]}}]}}"#
    );
    phonedriver_core::raw::RawTree::from_android_json(
        &json,
        phonedriver_core::geometry::Scale { density: 1.0 },
    )
    .unwrap()
}

fn sent(rig: &Rig) -> (usize, usize, usize) {
    let calls = rig.fake.calls();
    let count = |f: fn(&Call) -> bool| calls.iter().filter(|c| f(c)).count();
    (
        count(|c| matches!(c, Call::Gesture(_))),
        count(|c| matches!(c, Call::Text(_))),
        count(|c| matches!(c, Call::Key(_))),
    )
}

#[tokio::test]
async fn typing_into_a_guarded_button_is_held_before_its_focus_tap() {
    let rig = rig().await;
    let typed = json!({"kind": "type", "text": "x", "into": {"text": "Place order"}});
    let data = refused(rig.act(typed, None).await);
    assert_eq!(data["label"], PLACE_ORDER);
    assert_eq!(sent(&rig), (0, 0, 0));
}

#[tokio::test]
async fn clearing_a_guarded_button_is_held_before_its_focus_tap() {
    let rig = rig().await;
    let data = refused(
        rig.act(
            json!({"kind": "clear", "target": {"text": "Place order"}}),
            None,
        )
        .await,
    );
    assert_eq!(data["label"], PLACE_ORDER);
    assert_eq!(sent(&rig), (0, 0, 0));
}

#[tokio::test]
async fn submitting_a_form_whose_pay_button_waits_for_the_text_is_held_before_anything_is_typed() {
    let rig = rig_showing(transfer(false, false)).await;
    rig.fake.on_text(|_| Some(vec![transfer(true, true)]));
    let typed = json!({"kind": "type", "text": "NL91ABNA", "into": {"id": "iban"}, "submit": true});
    let data = refused(rig.act(typed.clone(), None).await);
    assert_eq!(data["label"], "Pay €250.00");
    assert_eq!(sent(&rig), (0, 0, 0));
    rig.act(typed, Some("once")).await.unwrap();
    assert_eq!(sent(&rig).2, 1, "enter after the human's word");
}

#[tokio::test]
async fn a_first_call_with_no_screen_read_yet_is_still_held() {
    let rig = rig_showing(transfer(true, true)).await;
    let data = refused(
        rig.act(json!({"kind": "type", "text": "x", "submit": true}), None)
            .await,
    );
    assert_eq!(data["label"], "Pay €250.00");
    assert_eq!(sent(&rig), (0, 0, 0));
}

#[tokio::test]
async fn press_enter_after_typing_into_the_form_is_held() {
    let rig = rig_showing(transfer(false, false)).await;
    rig.fake.on_text(|_| Some(vec![transfer(true, true)]));
    rig.act(
        json!({"kind": "type", "text": "NL91ABNA", "into": {"id": "iban"}}),
        None,
    )
    .await
    .unwrap();
    let data = refused(
        rig.act(json!({"kind": "press", "key": "enter"}), None)
            .await,
    );
    assert_eq!(data["label"], "Pay €250.00");
    assert_eq!(sent(&rig).2, 0, "no enter reached the device");
    rig.act(json!({"kind": "press", "key": "back"}), None)
        .await
        .unwrap();
}

#[tokio::test]
async fn swipes_drags_pinches_and_gestures_that_click_a_guarded_button_are_held() {
    let rig = rig().await;
    let place = json!({"text": "Place order"});
    let held = [
        json!({"kind": "swipe", "direction": "up", "from": place, "distance": 0.0, "fast": true}),
        json!({"kind": "drag", "from": place, "to": place}),
        json!({"kind": "pinch", "direction": "out", "at": place, "distance": 0.1}),
        json!({"kind": "gesture", "fingers": [[{"x": 180.0, "y": 425.0, "t_ms": 0}, {"x": 181.0, "y": 425.0, "t_ms": 60}]]}),
    ];
    for action in held {
        let data = refused(rig.act(action.clone(), None).await);
        assert_eq!(data["label"], PLACE_ORDER, "{action}");
    }
    assert_eq!(rig.gestures(), 0);
    rig.act(
        json!({"kind": "swipe", "direction": "up", "from": place}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(rig.gestures(), 1, "a swipe that leaves the button scrolls");
    let r = rig
        .act(
            json!({"kind": "swipe", "direction": "up", "from": place, "distance": 0.0}),
            Some("autonomous"),
        )
        .await
        .unwrap();
    assert!(!r["summary"].as_str().unwrap().contains("approved"), "{r}");
    assert_eq!(rig.gestures(), 2);
}

#[tokio::test]
async fn dragging_onto_an_opted_in_delete_button_is_held() {
    let rig = rig().await;
    let drag = json!({"kind": "drag", "from": {"text": "Back"}, "to": {"text": "Delete photo"}});
    rig.act(drag.clone(), None).await.unwrap();
    let data = refused(rig.act_asking(drag, None, &["delete"]).await);
    assert_eq!(data["category"], "delete");
    assert_eq!(rig.gestures(), 1);
}

fn slider() -> phonedriver_core::raw::RawTree {
    let json = r#"{"windows":[{"id":1,"type":"application","package":"com.example.ride","bounds":[0,0,360,800]}],"nodes":[
        {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.widget.TextView","text":"Total $18.40","b":[0,100,360,150],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.widget.SeekBar","text":"Slide to pay","b":[20,600,340,660],"f":"vVe","a":["scroll_fwd"]}]}"#;
    phonedriver_core::raw::RawTree::from_android_json(
        json,
        phonedriver_core::geometry::Scale { density: 1.0 },
    )
    .unwrap()
}

#[tokio::test]
async fn a_slide_to_pay_track_is_held_whatever_verb_moves_it() {
    let rig = rig_showing(slider()).await;
    let held = [
        json!({"kind": "swipe", "direction": "right", "from": {"text": "Slide to pay"}}),
        json!({"kind": "drag", "from": {"point": {"x": 30.0, "y": 630.0}}, "to": {"point": {"x": 330.0, "y": 630.0}}}),
        json!({"kind": "scroll", "direction": "right", "in": {"text": "Slide to pay"}}),
    ];
    for action in held {
        let data = refused(rig.act(action.clone(), None).await);
        assert_eq!(data["label"], "Slide to pay", "{action}");
        assert_eq!(data["category"], "payment");
    }
    assert_eq!(rig.gestures(), 0);
}

#[tokio::test]
async fn an_approval_covers_only_the_button_and_details_the_human_was_shown() {
    let rig = rig().await;
    let shown = refused(rig.act(tap("Place order"), None).await);
    let approved = |label: &str| {
        json!({"action": tap("Place order"), "settle": false, "approval": "once",
               "approved": {"label": label, "details": shown["details"]}})
    };
    let call = async |p: Value| {
        let mut c = Client::connect_to(&rig.socket, "guard-test").await.unwrap();
        c.call("act", p).await
    };
    refused(call(approved("Place order · $3.41")).await);
    assert_eq!(rig.gestures(), 0, "a changed amount is asked again");
    call(approved(PLACE_ORDER)).await.unwrap();
    assert_eq!(rig.gestures(), 1);
}

#[tokio::test]
async fn an_ordinary_swipe_or_scroll_reads_the_screen_no_more_than_with_the_guard_off() {
    let rig = rig().await;
    let mut c = Client::connect_to(&rig.socket, "guard-test").await.unwrap();
    c.call("snapshot", json!({})).await.unwrap();
    let reads = |action: Value, approval: Option<&'static str>| {
        let rig = &rig;
        async move {
            let before = rig.fake.tree_reads();
            rig.act(action, approval).await.unwrap();
            rig.fake.tree_reads() - before
        }
    };
    for action in [
        json!({"kind": "swipe", "direction": "up"}),
        json!({"kind": "swipe", "direction": "up", "from": {"text": "Back"}}),
        json!({"kind": "scroll", "direction": "down"}),
    ] {
        let off = reads(action.clone(), Some("autonomous")).await;
        let on = reads(action.clone(), None).await;
        assert_eq!(on, off, "{action}");
    }
}

fn one_button(label: &str) -> phonedriver_core::raw::RawTree {
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example.shop","bounds":[0,0,360,800]}}],"nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.widget.Button","text":"{label}","id":"cta","b":[0,400,360,450],"f":"vVe","a":["tap"]}}]}}"#
    );
    phonedriver_core::raw::RawTree::from_android_json(
        &json,
        phonedriver_core::geometry::Scale { density: 1.0 },
    )
    .unwrap()
}

/// A ref keeps its element when the element's label changes in place, so the guard must judge the label the screen
/// shows now. It judged the label the agent was last shown: "Add to list" became "Pay $20.00" and the tap went out.
#[tokio::test]
async fn a_ref_whose_button_now_says_pay_is_judged_by_what_it_says_now() {
    let rig = rig_showing(one_button("Add to list")).await;
    let mut c = Client::connect_to(&rig.socket, "guard-test").await.unwrap();
    let shown = c.call("snapshot", json!({})).await.unwrap();
    assert!(
        shown["text"]
            .as_str()
            .unwrap()
            .contains("@e1 button \"Add to list\""),
        "{shown}"
    );
    rig.fake.set_tree(one_button("Pay $20.00"));
    let data = refused(
        rig.act(json!({"kind": "tap", "target": {"ref": "e1"}}), None)
            .await,
    );
    assert_eq!(data["label"], "Pay $20.00");
    assert_eq!(rig.gestures(), 0);
}
