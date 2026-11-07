#[path = "support/fakes.rs"]
mod fakes;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fakes::{OneDevice, PNG, field_tree, pixel_device, tree};
use phonedriver::client::Client;
use phonedriver::daemon::{Config, Daemon, socket};
use phonedriver::mcp::Mcp;
use phonedriver_core::raw::RawTree;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use serde_json::{Value, json};

struct Rig {
    home: tempfile::TempDir,
    socket: PathBuf,
    client: rmcp::service::RunningService<rmcp::RoleClient, ()>,
}

async fn rig(trees: Vec<RawTree>) -> Rig {
    rig_with(pixel_device(trees)).await
}

async fn rig_with(device: Arc<dyn phonedriver_core::device::Device>) -> Rig {
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
    let (client_io, server_io) = tokio::io::duplex(1 << 20);
    let mcp = Mcp::new(Some(socket.clone()), "mcp-test".into());
    tokio::spawn(async move {
        let running = mcp.serve(server_io).await.unwrap();
        let _ = running.waiting().await;
    });
    let client = ().serve(client_io).await.unwrap();
    Rig {
        home,
        socket,
        client,
    }
}

impl Rig {
    async fn call(&self, tool: &'static str, args: Value) -> CallToolResult {
        self.client
            .call_tool(
                CallToolRequestParams::new(tool).with_arguments(args.as_object().unwrap().clone()),
            )
            .await
            .unwrap()
    }

    async fn daemon(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, phonedriver::client::Failure> {
        let mut c = Client::connect_to(&self.socket, "mcp-test").await.unwrap();
        c.call(method, params).await
    }
}

fn texts(r: &CallToolResult) -> Vec<String> {
    r.content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect()
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(files(&p));
        } else {
            out.push(p);
        }
    }
    out
}

#[tokio::test]
async fn tools_list_is_the_tools_with_short_descriptions_and_the_cli_required_arguments() {
    let rig = rig(vec![tree(&["One"])]).await;
    let tools = rig.client.list_all_tools().await.unwrap();
    let mut required: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for t in &tools {
        let d = t.description.as_deref().unwrap_or_default();
        let limit = if t.name == "shell" { 450 } else { 300 };
        assert!(
            !d.is_empty() && d.chars().count() < limit,
            "{}: {d}",
            t.name
        );
        let mut r: Vec<String> = t
            .input_schema
            .get("required")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        r.sort();
        required.insert(t.name.to_string(), r);
    }
    let want: BTreeMap<String, Vec<String>> = [
        ("apps", vec![]),
        ("devices", vec![]),
        ("snapshot", vec![]),
        ("screenshot", vec![]),
        ("tap", vec!["target"]),
        ("type", vec!["text"]),
        ("swipe", vec![]),
        ("scroll", vec!["direction"]),
        ("press", vec!["key"]),
        ("launch", vec![]),
        ("wait", vec!["condition"]),
        ("batch", vec!["steps"]),
        ("clipboard", vec![]),
        ("shell", vec!["command"]),
    ]
    .into_iter()
    .map(|(n, r)| (n.to_string(), r.into_iter().map(String::from).collect()))
    .collect();
    assert_eq!(required, want);
}

#[tokio::test]
async fn snapshot_is_one_text_block_equal_to_what_the_cli_prints() {
    let rig = rig(vec![tree(&["One", "Two"])]).await;
    rig.daemon("snapshot", json!({})).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let r = rig.call("snapshot", json!({})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let got = texts(&r);
    assert_eq!(got.len(), 1, "{r:?}");
    assert_eq!(r.content.len(), 1);
    let cli = rig.daemon("snapshot", json!({})).await.unwrap();
    assert_eq!(got[0], cli["text"].as_str().unwrap());
    assert!(got[0].contains("@e1 button \"One\""), "{}", got[0]);
}

#[tokio::test]
async fn screenshot_is_one_png_image_block_and_one_text_block_with_size_and_scale() {
    let rig = rig(vec![tree(&["One"])]).await;
    let r = rig.call("screenshot", json!({})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert_eq!(r.content.len(), 2, "{r:?}");
    let image = r.content.iter().find_map(|c| c.as_image()).expect("image");
    assert_eq!(image.mime_type, "image/png");
    use base64::Engine;
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(&image.data)
            .unwrap(),
        PNG
    );
    assert_eq!(texts(&r), vec!["360x800 at scale 1".to_string()]);
}

async fn remote(rig: &Rig) -> PathBuf {
    use interprocess::local_socket::traits::tokio::Listener as _;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, split};
    let proxy = rig.home.path().join("run").join("remote.sock");
    let listener = socket::listen(&proxy).await.unwrap();
    let daemon = rig.socket.clone();
    tokio::spawn(async move {
        loop {
            let Ok(client) = listener.accept().await else {
                return;
            };
            let upstream = socket::connect(&daemon).await.unwrap();
            let (cr, mut cw) = split(client);
            let (ur, mut uw) = split(upstream);
            tokio::spawn(async move {
                let mut lines = BufReader::new(cr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    uw.write_all(format!("{line}\n").as_bytes()).await.unwrap();
                }
            });
            tokio::spawn(async move {
                let mut lines = BufReader::new(ur).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut v: Value = serde_json::from_str(&line).unwrap();
                    if v["result"]["path"].is_string() {
                        v["result"]["path"] = json!("/the-daemons-host/shot.png");
                    }
                    cw.write_all(format!("{v}\n").as_bytes()).await.unwrap();
                }
            });
        }
    });
    proxy
}

#[tokio::test]
async fn screenshot_over_a_remote_socket_returns_the_image_without_reading_the_daemons_path() {
    let rig = rig(vec![tree(&["One"])]).await;
    let proxy = remote(&rig).await;
    let mcp = Mcp::new(Some(proxy), "mcp-remote".into());
    let (client_io, server_io) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        let running = mcp.serve(server_io).await.unwrap();
        let _ = running.waiting().await;
    });
    let client = ().serve(client_io).await.unwrap();
    let r = client
        .call_tool(CallToolRequestParams::new("screenshot"))
        .await
        .unwrap();
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let image = r.content.iter().find_map(|c| c.as_image()).expect("image");
    use base64::Engine;
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(&image.data)
            .unwrap(),
        PNG
    );
}

#[tokio::test]
async fn the_screenshot_rpc_sends_bytes_only_when_asked() {
    let rig = rig(vec![tree(&["One"])]).await;
    let plain = rig.daemon("screenshot", json!({})).await.unwrap();
    assert!(plain.get("data_b64").is_none(), "{plain}");
    let inline = rig
        .daemon("screenshot", json!({"inline": true}))
        .await
        .unwrap();
    use base64::Engine;
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(inline["data_b64"].as_str().unwrap())
            .unwrap(),
        PNG
    );
    assert_eq!(
        std::fs::read(inline["path"].as_str().unwrap()).unwrap(),
        PNG
    );
}

#[tokio::test]
async fn an_error_is_is_error_with_the_message_and_the_hint() {
    let rig = rig(vec![tree(&["One"])]).await;
    let r = rig.call("tap", json!({"target": "@e99"})).await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    let want = rig
        .daemon(
            "act",
            json!({"action": {"kind": "tap", "target": {"ref": "e99"}}, "settle": true}),
        )
        .await
        .unwrap_err();
    let hint = want.hint().expect("stale_ref has a hint");
    assert!(hint.contains("phonedriver snapshot"));
    assert_eq!(
        texts(&r),
        vec![format!(
            "{}\nhint: Call `snapshot` for fresh refs.",
            want.message()
        )]
    );
    assert!(texts(&r)[0].contains("e99"));
}

#[tokio::test]
async fn launch_needs_an_app_or_a_url() {
    let rig = rig(vec![tree(&["One"])]).await;
    let r = rig.call("launch", json!({})).await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    assert!(texts(&r)[0].contains("app or url"), "{r:?}");
}

#[tokio::test]
async fn a_secret_typed_over_mcp_reaches_no_reply_no_trace_and_no_file() {
    let rig = rig(vec![field_tree("", false), field_tree("hunter2", false)]).await;
    rig.call("snapshot", json!({})).await;
    let r = rig
        .call(
            "type",
            json!({"text": "hunter2", "into": "@e1", "secret": true}),
        )
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let all = serde_json::to_string(&r).unwrap();
    assert!(!all.contains("hunter2"), "{all}");
    assert!(all.contains("verified"), "{all}");
    let bad = rig
        .call(
            "type",
            json!({"text": "hunter2", "into": "@e9", "secret": true}),
        )
        .await;
    assert!(!serde_json::to_string(&bad).unwrap().contains("hunter2"));
    for f in files(rig.home.path()) {
        if f.extension().is_some_and(|e| e == "sock") {
            continue;
        }
        let text = std::fs::read(&f).unwrap();
        assert!(
            !String::from_utf8_lossy(&text).contains("hunter2"),
            "{} holds the secret",
            f.display()
        );
    }
}

#[tokio::test]
async fn scroll_over_mcp_reaches_the_daemon_with_every_argument_shape() {
    let rig = rig(vec![tree(&["One", "Two"])]).await;
    rig.call("snapshot", json!({})).await;
    for args in [
        json!({"direction": "down"}),
        json!({"direction": "down", "until": "Two"}),
        json!({"direction": "down", "in": "@e1"}),
    ] {
        let r = rig.call("scroll", args.clone()).await;
        assert_ne!(r.is_error, Some(true), "{args}: {r:?}");
    }
}

async fn leased_by(socket: &Path) -> Value {
    let mut c = Client::connect_to(socket, "probe").await.unwrap();
    c.call("devices", json!({})).await.unwrap()[0]["leased_by"].clone()
}

#[tokio::test]
async fn closing_the_mcp_connection_releases_its_lease_without_a_release_call() {
    let rig = rig(vec![tree(&["One"])]).await;
    rig.call("snapshot", json!({})).await;
    assert_eq!(leased_by(&rig.socket).await, "mcp-test");
    let Rig {
        socket,
        client,
        home: _home,
    } = rig;
    client.cancel().await.unwrap();
    for _ in 0..100 {
        if leased_by(&socket).await.is_null() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("the lease of the closed MCP server is still held");
}

#[tokio::test]
async fn every_mcp_call_rides_one_connection_so_the_lease_survives_between_calls() {
    let rig = rig(vec![tree(&["One"])]).await;
    for _ in 0..3 {
        rig.call("snapshot", json!({})).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(leased_by(&rig.socket).await, "mcp-test");
    }
}

#[tokio::test]
async fn a_cli_connection_closing_keeps_its_lease_because_every_cli_call_is_a_new_connection() {
    let rig = rig(vec![tree(&["One"])]).await;
    let mut c = Client::connect_to(&rig.socket, "keeper").await.unwrap();
    c.call("snapshot", json!({})).await.unwrap();
    drop(c);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(leased_by(&rig.socket).await, "keeper");
}

fn travel(call: &phonedriver_core::testing::Call) -> f32 {
    let phonedriver_core::testing::Call::Gesture(g) = call else {
        panic!("{call:?}")
    };
    let path = &g.fingers[0];
    (path[0].at.y - path[path.len() - 1].at.y).abs()
}

#[test]
fn calls_sent_together_run_one_at_a_time_in_the_order_they_were_sent() {
    for _ in 0..5 {
        let rt = phonedriver::mcp::server_runtime().unwrap();
        rt.block_on(async {
            let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![tree(&[
                "One",
            ])]));
            let rig = rig_with(fake.clone()).await;
            let swipe = |d: f64| rig.call("swipe", json!({"direction": "up", "distance": d}));
            let (a, b, c, d) = tokio::join!(swipe(0.1), swipe(0.2), swipe(0.3), swipe(0.4));
            for r in [a, b, c, d] {
                assert_ne!(r.is_error, Some(true), "{r:?}");
            }
            let spans: Vec<f32> = fake.calls().iter().map(travel).collect();
            assert_eq!(spans.len(), 4);
            assert!(spans.windows(2).all(|w| w[0] < w[1]), "{spans:?}");
        });
    }
}

#[tokio::test]
async fn argument_schemas_carry_no_boilerplate() {
    let rig = rig(vec![tree(&["One"])]).await;
    for t in rig.client.list_all_tools().await.unwrap() {
        let text = serde_json::to_string(&*t.input_schema).unwrap();
        for waste in ["$schema", "\"format\"", "\"minimum\"", "\"null\""] {
            assert!(!text.contains(waste), "{}: {waste} in {text}", t.name);
        }
    }
}

#[tokio::test]
async fn tap_says_its_reply_is_the_new_screen_and_that_tools_go_one_at_a_time() {
    let rig = rig(vec![tree(&["One"])]).await;
    let tools = rig.client.list_all_tools().await.unwrap();
    let tap = tools.iter().find(|t| t.name == "tap").unwrap();
    let d = tap.description.as_deref().unwrap_or_default();
    assert!(
        d.contains("one at a time") && d.contains("new screen"),
        "{d}"
    );
}

#[tokio::test]
async fn the_tool_list_every_turn_re_reads_fits_a_budget_and_hides_what_agents_never_used() {
    let rig = rig(vec![tree(&["One"])]).await;
    let tools = rig.client.list_all_tools().await.unwrap();
    let total: usize = tools
        .iter()
        .map(|t| serde_json::to_string(t).unwrap().len())
        .sum();
    assert!(total < 4_300, "tools/list is {total} characters");
    for t in &tools {
        let text = serde_json::to_string(&*t.input_schema).unwrap();
        for hidden in ["\"device\"", "\"max_lines\"", "\"role\"", "\"distance\""] {
            assert!(!text.contains(hidden), "{}: {hidden}", t.name);
        }
    }
    let r = rig
        .call("snapshot", json!({"max_lines": 2, "interactive": true}))
        .await;
    assert_ne!(r.is_error, Some(true), "{:?}", texts(&r));
}

#[tokio::test]
async fn the_server_tells_the_agent_that_calls_share_one_screen() {
    let rig = rig(vec![tree(&["One"])]).await;
    let info = rig.client.peer_info().unwrap();
    let text = info.instructions.clone().unwrap_or_default();
    assert!(
        text.contains("one at a time") && text.contains("shared screen"),
        "{text}"
    );
}

#[tokio::test]
async fn the_server_says_launch_takes_a_name_so_devices_and_apps_are_not_a_first_step() {
    let rig = rig(vec![tree(&["One"])]).await;
    let info = rig.client.peer_info().unwrap();
    let text = info.instructions.clone().unwrap_or_default();
    assert!(
        text.contains("`launch` takes an app name") && text.contains("`devices`"),
        "{text}"
    );
}

#[tokio::test]
async fn what_the_lean_list_leaves_out_is_still_named_where_an_agent_looks() {
    let rig = rig(vec![tree(&["One"])]).await;
    let tools = rig.client.list_all_tools().await.unwrap();
    let text = |name: &str| {
        let t = tools.iter().find(|t| t.name == name).unwrap();
        serde_json::to_string(t).unwrap()
    };
    assert!(text("tap").contains("long"), "{}", text("tap"));
    assert!(text("type").contains("secret"), "{}", text("type"));
    assert!(text("devices").contains("`device`"), "{}", text("devices"));
    assert!(text("launch").contains("grant"), "{}", text("launch"));
    assert!(
        text("screenshot").contains("marks"),
        "{}",
        text("screenshot")
    );
    assert!(text("scroll").contains("until"), "{}", text("scroll"));
}

#[tokio::test]
async fn apps_over_mcp_lists_the_launchable_apps_with_their_ids() {
    let rig = rig(vec![tree(&["One"])]).await;
    let r = rig.call("apps", json!({})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert!(
        texts(&r)[0].is_empty() || texts(&r)[0].contains("  "),
        "{r:?}"
    );
}

#[tokio::test]
async fn failures_and_hints_over_mcp_name_tools_and_arguments_not_cli_commands() {
    let rig = rig(vec![tree(&["One"])]).await;
    rig.call("snapshot", json!({})).await;
    let mut seen = Vec::new();
    seen.push(rig.call("tap", json!({"target": "@e99"})).await);
    seen.push(rig.call("tap", json!({"target": "Nope"})).await);
    seen.push(rig.call("press", json!({"key": "nope"})).await);
    seen.push(
        rig.call("scroll", json!({"direction": "down", "until": "Zebra"}))
            .await,
    );
    seen.push(
        rig.call("wait", json!({"condition": "Zebra", "timeout_ms": 100}))
            .await,
    );
    for r in seen {
        assert_eq!(r.is_error, Some(true), "{r:?}");
        let text = texts(&r).join("\n");
        assert!(
            !text.contains("phonedriver") && !text.contains(" --"),
            "{text}"
        );
        assert!(text.contains("hint:"), "{text}");
    }
}

#[tokio::test]
async fn a_batch_over_mcp_runs_the_steps_and_returns_one_numbered_reply() {
    let rig = rig(vec![field_tree("", false), field_tree("hi", false)]).await;
    rig.call("snapshot", json!({})).await;
    let r = rig
        .call(
            "batch",
            json!({"steps": [
                {"tool": "tap", "target": "@e1"},
                {"tool": "type", "text": "hi", "submit": true},
                {"tool": "press", "key": "enter"},
            ]}),
        )
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let text = texts(&r).join("\n");
    assert!(text.starts_with("ok 3 steps"), "{text}");
    assert!(text.contains("\n1. tapped @e1"), "{text}");
    assert!(text.contains("\n2. typed 2 chars"), "{text}");
    assert!(text.contains("\n3. pressed enter"), "{text}");
}

#[tokio::test]
async fn a_failing_batch_step_is_an_error_naming_the_step_with_hints_in_tool_words() {
    let rig = rig(vec![tree(&["One"])]).await;
    rig.call("snapshot", json!({})).await;
    let r = rig
        .call(
            "batch",
            json!({"steps": [
                {"tool": "tap", "target": "@e1"},
                {"tool": "tap", "target": "@e99"},
                {"tool": "press", "key": "home"},
            ]}),
        )
        .await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    let text = texts(&r).join("\n");
    assert!(text.starts_with("step 2 of 3 failed"), "{text}");
    assert!(text.contains("1. tapped @e1"), "{text}");
    assert!(!text.contains("phonedriver "), "{text}");
}

#[tokio::test]
async fn a_batch_refuses_secrets_unknown_tools_and_unknown_arguments_before_acting() {
    let rig = rig(vec![field_tree("", false)]).await;
    for (step, why) in [
        (
            json!({"tool": "type", "text": "hunter2", "secret": true}),
            "secret",
        ),
        (json!({"tool": "snapshot"}), "cannot be a step"),
        (
            json!({"tool": "tap", "target": "@e1", "colour": "red"}),
            "colour",
        ),
        (json!({"target": "@e1"}), "needs `tool`"),
    ] {
        let r = rig
            .call(
                "batch",
                json!({"steps": [{"tool": "press", "key": "home"}, step]}),
            )
            .await;
        assert_eq!(r.is_error, Some(true), "{why}: {r:?}");
        let text = texts(&r).join("\n");
        assert!(text.starts_with("step 2:") && text.contains(why), "{text}");
        assert!(!text.contains("hunter2"), "{text}");
    }
}

#[tokio::test]
async fn text_typed_in_a_batch_stays_out_of_the_trace() {
    let rig = rig(vec![
        field_tree("", false),
        field_tree("correct horse", false),
    ])
    .await;
    rig.call("snapshot", json!({})).await;
    let r = rig
        .call(
            "batch",
            json!({"steps": [{"tool": "type", "text": "correct horse", "into": "@e1"}]}),
        )
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    for f in files(rig.home.path()) {
        if f.extension().is_some_and(|e| e == "sock") {
            continue;
        }
        let text = std::fs::read(&f).unwrap();
        assert!(
            !String::from_utf8_lossy(&text).contains("correct horse"),
            "{} holds the typed text",
            f.display()
        );
    }
}

#[tokio::test]
async fn clipboard_find_release_clear_and_drag_are_reachable_from_existing_tools() {
    let rig = rig(vec![field_tree("hello", false), field_tree("", false)]).await;
    let r = rig.call("clipboard", json!({"set": "copied"})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let r = rig.call("clipboard", json!({})).await;
    assert_eq!(texts(&r), ["copied"]);

    let r = rig.call("snapshot", json!({"find": "FIELD"})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert!(texts(&r)[0].contains("textbox"), "{r:?}");

    let r = rig.call("type", json!({"text": "", "into": "@e1"})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert!(texts(&r)[0].starts_with("ok cleared"), "{r:?}");
    let r = rig.call("type", json!({"text": ""})).await;
    assert_eq!(r.is_error, Some(true));

    let r = rig
        .call("swipe", json!({"from": "@e1", "to": "200,500"}))
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert!(texts(&r)[0].starts_with("ok dragged"), "{r:?}");
    let r = rig.call("swipe", json!({"pinch": "out"})).await;
    assert!(texts(&r)[0].starts_with("ok pinched out"), "{r:?}");
    for bad in [
        json!({}),
        json!({"to": "200,500"}),
        json!({"pinch": "sideways"}),
    ] {
        let r = rig.call("swipe", bad.clone()).await;
        assert_eq!(r.is_error, Some(true), "{bad}");
    }

    assert_eq!(leased_by(&rig.socket).await, "mcp-test");
    let r = rig.call("devices", json!({"release": true})).await;
    assert_eq!(texts(&r), ["released"]);
    assert_eq!(leased_by(&rig.socket).await, Value::Null);
}

#[tokio::test]
async fn a_screenshot_with_marks_boxes_every_tappable_element_and_says_how_many() {
    let rig = rig(vec![tree(&["One", "Two"])]).await;
    let r = rig.call("screenshot", json!({"marks": true})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert_eq!(texts(&r), ["360x800 at scale 1, 2 marked elements"]);
    let image = r.content.iter().find_map(|c| c.as_image()).expect("image");
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&image.data)
        .unwrap();
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    assert_ne!(bytes, PNG, "the picture was redrawn");
}

#[tokio::test]
async fn apps_with_an_action_stops_an_app_and_refuses_a_missing_path() {
    let rig = rig(vec![tree(&["One"])]).await;
    let r = rig
        .call("apps", json!({"action": "stop", "app": "nothing"}))
        .await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    let r = rig.call("apps", json!({"action": "install"})).await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
}
