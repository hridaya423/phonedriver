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
use rmcp::model::{CallToolRequestParams, CallToolResult, ProgressNotificationParam};
use serde_json::{Value, json};

struct Rig {
    home: tempfile::TempDir,
    socket: PathBuf,
    client: rmcp::service::RunningService<rmcp::RoleClient, Heard>,
    heard: Heard,
}

#[derive(Clone, Default)]
struct Heard(Arc<std::sync::Mutex<Vec<ProgressNotificationParam>>>);

impl rmcp::ClientHandler for Heard {
    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: rmcp::service::NotificationContext<rmcp::RoleClient>,
    ) {
        self.0.lock().unwrap().push(params);
    }
}

async fn rig(trees: Vec<RawTree>) -> Rig {
    rig_with(pixel_device(trees)).await
}

async fn rig_with(device: Arc<dyn phonedriver_core::device::Device>) -> Rig {
    rig_on(Arc::new(OneDevice(device))).await
}

async fn rig_on(source: Arc<dyn phonedriver::daemon::DeviceSource>) -> Rig {
    rig_built(source, None).await
}

async fn rig_with_files(device: Arc<dyn phonedriver_core::device::Device>, files: &Path) -> Rig {
    rig_built(Arc::new(OneDevice(device)), Some(files)).await
}

async fn rig_built(
    source: Arc<dyn phonedriver::daemon::DeviceSource>,
    files: Option<&Path>,
) -> Rig {
    let home = tempfile::tempdir().unwrap();
    let daemon = Daemon::new(
        source,
        Config {
            home: home.path().to_path_buf(),
        },
    );
    let socket = test_socket(home.path(), "daemon.sock");
    let listener = socket::listen(&socket).await.unwrap();
    tokio::spawn(daemon.serve_on(listener));
    let (client_io, server_io) = tokio::io::duplex(1 << 20);
    let mut mcp = Mcp::new(Some(socket.clone()), "mcp-test".into()).in_folder(home.path().into());
    if let Some(files) = files {
        mcp = mcp.with_files(files.to_path_buf());
    }
    tokio::spawn(async move {
        let running = mcp.serve(server_io).await.unwrap();
        let _ = running.waiting().await;
    });
    let heard = Heard::default();
    let client = heard.clone().serve(client_io).await.unwrap();
    Rig {
        home,
        socket,
        client,
        heard,
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

    /// The rmcp client gives a progress token with every request, so this is what a call was told while it ran.
    async fn call_told(&self, tool: &'static str, args: Value) -> (CallToolResult, Vec<String>) {
        let before = self.heard.0.lock().unwrap().len();
        let result = self.call(tool, args).await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let heard = self.heard.0.lock().unwrap()[before..].to_vec();
        assert!(
            heard
                .windows(2)
                .all(|w| w[0].progress_token == w[1].progress_token),
            "{heard:?}"
        );
        assert!(
            heard.windows(2).all(|w| w[0].progress < w[1].progress),
            "progress only grows: {heard:?}"
        );
        let said = heard.into_iter().map(|p| p.message.unwrap()).collect();
        (result, said)
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
        ("replay", vec!["file"]),
        ("tap", vec!["target"]),
        ("type", vec!["text"]),
        ("swipe", vec![]),
        ("scroll", vec!["direction"]),
        ("press", vec!["key"]),
        ("launch", vec![]),
        ("wait", vec![]),
        ("batch", vec!["steps"]),
        ("clipboard", vec![]),
        ("files", vec![]),
        ("shell", vec!["command"]),
        ("setting", vec![]),
        ("logs", vec![]),
        ("network", vec![]),
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
async fn a_web_radio_shows_on_or_off_like_a_web_checkbox_and_a_native_one_shows_its_own_state() {
    let json = r#"{"windows":[{"id":1,"type":"application","package":"com.android.chrome","bounds":[0,0,360,800]}],"nodes":[
        {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.webkit.WebView","text":"Order","b":[0,0,360,500],"f":"vVe"},
        {"p":1,"w":1,"cls":"android.widget.RadioButton","text":"Medium","b":[10,100,30,120],"f":"vVe","a":["tap"]},
        {"p":1,"w":1,"cls":"android.widget.CheckBox","text":"Bacon","b":[10,200,30,220],"f":"vVe","a":["tap"]},
        {"p":0,"w":1,"cls":"android.widget.RadioButton","text":"Native","b":[10,600,30,620],"f":"vVecC","a":["tap"]}]}"#;
    let tree = RawTree::from_android_json(json, phonedriver_core::geometry::Scale { density: 1.0 })
        .unwrap();
    let rig = rig(vec![tree]).await;
    let text = rig.daemon("snapshot", json!({})).await.unwrap()["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("radio \"Medium\" off"), "{text}");
    assert!(text.contains("checkbox \"Bacon\" off"), "{text}");
    assert!(text.contains("radio \"Native\" on"), "{text}");
}

#[tokio::test]
async fn the_device_line_is_told_once_and_told_again_only_when_it_changes() {
    let rig = rig(vec![tree(&["One", "Two"])]).await;
    let first = texts(&rig.call("snapshot", json!({})).await).remove(0);
    assert!(first.starts_with("device fake:1 · "), "{first}");
    let second = texts(&rig.call("snapshot", json!({})).await).remove(0);
    assert!(!second.contains("device fake:1"), "{second}");
    assert!(second.contains("@e1 button \"One\""), "{second}");
    assert!(
        second.lines().next().unwrap().starts_with("app "),
        "{second}"
    );
    let find = texts(&rig.call("snapshot", json!({"find": "One"})).await).remove(0);
    assert!(find.starts_with("@e1 button"), "{find}");
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

#[tokio::test]
async fn compare_needs_an_earlier_screenshot_and_then_answers_in_text_with_no_image() {
    let rig = rig(vec![tree(&["One"])]).await;
    let r = rig.call("screenshot", json!({"compare": true})).await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    assert_eq!(
        texts(&r),
        vec![
            "`compare` needs an earlier screenshot of this device: take one, act, then compare"
                .to_string()
        ]
    );
    let r = rig.call("screenshot", json!({"compare": true})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert!(r.content.iter().all(|c| c.as_image().is_none()), "{r:?}");
    assert_eq!(texts(&r), vec!["same".to_string()]);
}

async fn remote(rig: &Rig) -> PathBuf {
    use interprocess::local_socket::traits::tokio::Listener as _;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, split};
    let proxy = test_socket(rig.home.path(), "remote.sock");
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
            "{}\nhint: Call `snapshot` to read the screen, then use its refs; refs are per session.",
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
        heard: _,
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
async fn every_tool_says_whether_it_only_reads_and_the_five_that_do_are_marked() {
    let rig = rig(vec![tree(&["One"])]).await;
    let tools = rig.client.list_all_tools().await.unwrap();
    let mut read_only = Vec::new();
    for t in &tools {
        let a = t
            .annotations
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no annotations", t.name));
        assert!(a.title.is_some(), "{}", t.name);
        match a.read_only_hint {
            Some(true) => read_only.push(t.name.to_string()),
            Some(false) => assert!(a.destructive_hint.is_some(), "{}", t.name),
            None => panic!("{} does not say", t.name),
        }
    }
    read_only.sort();
    assert_eq!(
        read_only,
        ["logs", "network", "screenshot", "snapshot", "wait"]
    );
    let then = |name: &str| {
        let t = tools.iter().find(|t| t.name == name).unwrap();
        serde_json::to_string(&*t.input_schema)
            .unwrap()
            .contains("\"then\"")
    };
    assert!(then("tap") && then("type"));
}

#[tokio::test]
async fn the_tool_list_every_turn_re_reads_fits_a_budget_and_hides_what_agents_never_used() {
    let rig = rig(vec![tree(&["One"])]).await;
    let tools = rig.client.list_all_tools().await.unwrap();
    let total: usize = tools
        .iter()
        .map(|t| {
            t.name.len()
                + t.description.as_deref().unwrap_or_default().len()
                + serde_json::to_string(&*t.input_schema).unwrap().len()
        })
        .sum();
    assert!(
        total < 6_900,
        "what a model reads of tools/list is {total} characters (6,371 in 0.1.7 and 7,383 before the cuts of decision D130; 6,877 with every tool since, `files` among them); every turn re-reads it, so raise this only for a tool or argument worth its cost"
    );
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

/// Decision D130: the list was cut by a few hundred characters. What an agent needs to find each thing added since
/// 0.1.7 is still in it: the argument's name, in the schema or in the description.
#[tokio::test]
async fn everything_added_since_0_1_7_is_still_named_in_the_tool_list() {
    let rig = rig(vec![tree(&["One"])]).await;
    let tools = rig.client.list_all_tools().await.unwrap();
    let text = |name: &str| {
        let t = tools.iter().find(|t| t.name == name).unwrap();
        serde_json::to_string(t).unwrap()
    };
    for (tool, named) in [
        ("devices", "lane"),
        ("batch", "check"),
        ("batch", "equals"),
        ("scroll", "\"collect\""),
        ("snapshot", "\"notifications\""),
        ("snapshot", "\"audit\""),
        ("wait", "\"notification\""),
        ("replay", "paths/<app id>/<name>.steps"),
        ("screenshot", "`within`"),
        (
            "setting",
            "\"restore\":{\"description\":\"Put back your writes.\",\"type\":\"boolean\"}",
        ),
        ("tap", "\"select\""),
        ("files", "\"push\""),
        ("files", "\"pull\""),
        ("files", "No argument lists"),
        ("files", "PHONEDRIVER_FILES"),
        ("files", "never contents"),
    ] {
        assert!(
            text(tool).contains(named),
            "{tool} names no {named}: {}",
            text(tool)
        );
    }
}

/// A client that loads tools on demand shows an agent the tool names and these instructions, not the descriptions:
/// asked for the phone's notifications, an agent picked `shell` by its name and never read `snapshot`'s arguments.
#[tokio::test]
async fn the_server_says_which_tool_reads_notifications_since_no_tool_name_does() {
    let rig = rig(vec![tree(&["One"])]).await;
    let info = rig.client.peer_info().unwrap();
    let text = info.instructions.clone().unwrap_or_default();
    assert!(
        text.contains("Notifications: `snapshot` with `notifications`"),
        "{text}"
    );
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

/// `launch` returns the screen it opens, so "then `snapshot` once" cost every task a call that showed it again.
#[tokio::test]
async fn the_server_does_not_ask_for_a_snapshot_after_launch() {
    let rig = rig(vec![tree(&["One"])]).await;
    let info = rig.client.peer_info().unwrap();
    let text = info.instructions.clone().unwrap_or_default();
    assert!(!text.contains("then `snapshot`"), "{text}");
    assert!(text.contains("returns the screen"), "{text}");
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
async fn devices_with_boot_starts_a_device_or_says_it_is_on_and_a_wrong_name_names_the_tool() {
    let rig = rig(vec![tree(&["One"])]).await;
    let r = rig.call("devices", json!({"boot": "fake"})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert_eq!(texts(&r), vec!["already on: fake:1 · Fake · FakeOS 1"]);
    let r = rig.call("devices", json!({"boot": "Watch"})).await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    let text = texts(&r).join("\n");
    assert!(
        text.starts_with("no device named \"Watch\" is off; nothing can be started\nhint: "),
        "{text}"
    );
    assert!(!text.contains("phonedriver"), "{text}");
}

#[tokio::test]
async fn an_argument_no_tool_has_is_refused_with_the_ones_it_has() {
    let rig = rig(vec![tree(&["One"])]).await;
    for (tool, args, has) in [
        ("clipboard", json!({"action": "set", "text": "x"}), "`set`"),
        ("screenshot", json!({"mark": true}), "`marks`"),
        ("devices", json!({"start": "Fake"}), "`boot`"),
        ("snapshot", json!({"search": "One"}), "`find`"),
        ("apps", json!({"name": "Fake"}), "`app`"),
    ] {
        let reply = rig
            .client
            .call_tool(
                CallToolRequestParams::new(tool).with_arguments(args.as_object().unwrap().clone()),
            )
            .await;
        let said = match reply {
            Ok(r) => {
                assert_eq!(r.is_error, Some(true), "{tool}: {r:?}");
                texts(&r).join("\n")
            }
            Err(e) => e.to_string(),
        };
        assert!(
            said.contains("unknown field") && said.contains(has),
            "{tool}: {said}"
        );
    }
}

#[tokio::test]
async fn an_empty_clipboard_is_said_in_words() {
    let rig = rig(vec![tree(&["One"])]).await;
    let r = rig.call("clipboard", json!({})).await;
    assert_eq!(texts(&r), vec!["the clipboard is empty"]);
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
    assert!(text.contains("\n2. typed into @e1"), "{text}");
    assert!(text.contains("\n3. pressed enter"), "{text}");
}

#[tokio::test]
async fn a_tap_with_then_taps_each_text_in_order_and_returns_one_numbered_reply() {
    let rig = rig(vec![tree(&["One", "Two", "Three"])]).await;
    rig.call("snapshot", json!({})).await;
    let r = rig
        .call("tap", json!({"target": "One", "then": ["Two", "Three"]}))
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let text = texts(&r).join("\n");
    assert!(text.starts_with("ok 3 steps"), "{text}");
    assert!(text.contains("\n1. tapped @e1"), "{text}");
    assert!(text.contains("\n2. tapped @e2"), "{text}");
    assert!(text.contains("\n3. tapped @e3"), "{text}");
}

/// Issue #50: a batch step took `then` and a wait's `timeout_ms`, ran without them and answered `ok 1 step`.
#[tokio::test]
async fn a_batch_step_taps_its_then_texts_and_a_wait_step_keeps_its_own_timeout() {
    let rig = rig(vec![tree(&["One", "Two", "Three"])]).await;
    rig.call("snapshot", json!({})).await;
    let r = rig
        .call(
            "batch",
            json!({"steps": [
                {"tool": "tap", "target": "One", "then": ["Two"]},
                {"tool": "tap", "target": "Three"},
            ]}),
        )
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let text = texts(&r).join("\n");
    assert!(text.starts_with("ok 3 steps"), "{text}");
    assert!(text.contains("\n2. tapped @e2"), "{text}");
    assert!(text.contains("\n3. tapped @e3"), "{text}");

    let started = std::time::Instant::now();
    let r = rig
        .call(
            "batch",
            json!({"steps": [{"tool": "wait", "condition": "Nope", "timeout_ms": 300}]}),
        )
        .await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(3),
        "the wait took the batch's 5 s: {:?}",
        started.elapsed()
    );
}

fn receipt(total: &str) -> RawTree {
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.widget.TextView","text":"Total","b":[0,100,180,160],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.widget.TextView","text":"{total}","b":[180,100,360,160],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.widget.Button","text":"One","b":[0,200,360,260],"f":"vVe","a":["tap"]}}]}}"#
    );
    RawTree::from_android_json(&json, phonedriver_core::geometry::Scale { density: 1.0 }).unwrap()
}

#[tokio::test]
async fn a_check_step_reads_a_value_in_the_numbered_reply_and_a_wrong_one_names_what_it_shows() {
    let rig = rig(vec![receipt("€24.99")]).await;
    rig.call("snapshot", json!({})).await;
    let r = rig
        .call(
            "batch",
            json!({"steps": [
                {"tool": "check", "target": "Total", "as": "price_a"},
                {"tool": "tap", "target": "One"},
            ]}),
        )
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let text = texts(&r).join("\n");
    assert!(
        text.contains("\n1. \"Total\" = \"€24.99\" · price_a\n2. tapped"),
        "{text}"
    );

    let r = rig
        .call(
            "batch",
            json!({"steps": [
                {"tool": "check", "target": "Total", "equals": "€19.99"},
                {"tool": "tap", "target": "One"},
            ]}),
        )
        .await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    let text = texts(&r).join("\n");
    assert!(
        text.starts_with("step 1 of 2 failed: \"Total\" shows \"€24.99\", not \"€19.99\""),
        "{text}"
    );
    let r = rig
        .call(
            "batch",
            json!({"steps": [{"tool": "check", "target": "Total", "equal": "x"}]}),
        )
        .await;
    assert!(texts(&r)[0].contains("equal"), "{r:?}");
}

/// What a check reads is screen text, which a trace keeps only when asked to (`full`).
#[tokio::test]
async fn a_value_a_check_reads_stays_out_of_the_trace() {
    let rig = rig(vec![receipt("€24.99")]).await;
    let r = rig
        .daemon(
            "check",
            json!({"target": {"text": "Total"}, "equals": "€24.99"}),
        )
        .await
        .unwrap();
    assert_eq!(r["summary"], "\"Total\" = \"€24.99\"");
    let e = rig
        .daemon(
            "check",
            json!({"target": {"text": "Total"}, "equals": "€19.99"}),
        )
        .await
        .unwrap_err();
    assert!(e.message().contains("€24.99"), "{}", e.message());
    for f in files(rig.home.path()) {
        if f.extension().is_some_and(|e| e == "sock") {
            continue;
        }
        let text = std::fs::read(&f).unwrap();
        let text = String::from_utf8_lossy(&text);
        assert!(
            !text.contains("24.99") && !text.contains("19.99"),
            "{} holds a checked value",
            f.display()
        );
    }
}

/// What a notification says is its sender's text: it is in the reply and in no file the daemon writes.
#[tokio::test]
async fn notifications_are_listed_by_snapshot_and_waited_for_by_wait_and_stay_out_of_the_trace() {
    use phonedriver_core::notice::{Notice, Notices};
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![tree(&[
        "Send code",
    ])]));
    fake.answer_notifications(vec![Notices {
        now_ms: 10_000_000,
        notices: vec![Notice {
            key: "0|com.bank|1|otp|1".into(),
            package: "com.bank".into(),
            title: Some("Your code".into()),
            text: Some("Your code is 482913".into()),
            posted_ms: 9_999_500,
            ongoing: false,
        }],
    }]);
    let rig = rig_with(fake).await;
    let r = rig.call("snapshot", json!({"notifications": true})).await;
    assert_eq!(
        texts(&r),
        ["com.bank · \"Your code\" \"Your code is 482913\" · now"],
        "{r:?}"
    );
    rig.call("tap", json!({"target": "Send code"})).await;
    let r = rig.call("wait", json!({"notification": "code"})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let text = texts(&r).join("\n");
    assert!(
        text.starts_with(
            "ok a notification from com.bank is up\n  com.bank · \"Your code\" \"Your code is 482913\""
        ),
        "{text}"
    );
    for f in files(rig.home.path()) {
        if f.extension().is_some_and(|e| e == "sock") {
            continue;
        }
        let kept = std::fs::read(&f).unwrap();
        assert!(
            !String::from_utf8_lossy(&kept).contains("482913"),
            "{} holds a notification's text",
            f.display()
        );
    }
}

fn list_page(labels: &[&str], more: bool) -> RawTree {
    let actions = if more { r#"["scroll_down"]"# } else { "[]" };
    let mut nodes = vec![
        r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}"#
            .to_string(),
        format!(
            r#"{{"p":0,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[0,100,360,700],"f":"vVeS","a":{actions}}}"#
        ),
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
    RawTree::from_android_json(&json, phonedriver_core::geometry::Scale { density: 1.0 }).unwrap()
}

#[tokio::test]
async fn a_collecting_scroll_lists_its_rows_under_the_summary_alone_and_as_a_batch_step() {
    let pages = || {
        vec![
            list_page(&["Ada", "Bea"], true),
            list_page(&["Bea", "Cy"], false),
        ]
    };
    let rig_a = rig(pages()).await;
    let r = rig_a
        .call("scroll", json!({"direction": "down", "collect": 50}))
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let text = texts(&r).join("\n");
    assert!(
        text.starts_with(
            "ok collected 3 rows in 1 scroll down: the list ended · new screen\n  cell \"Ada\"\n  cell \"Bea\"\n  cell \"Cy\"\nscreen now:\n"
        ),
        "{text}"
    );

    let rig_b = rig(pages()).await;
    let r = rig_b
        .call(
            "batch",
            json!({"steps": [{"tool": "scroll", "direction": "down", "collect": 50}]}),
        )
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let text = texts(&r).join("\n");
    assert!(
        text.contains("1. collected 3 rows in 1 scroll down: the list ended\n   cell \"Ada\"\n   cell \"Bea\"\n   cell \"Cy\""),
        "{text}"
    );
}

#[tokio::test]
async fn a_failed_tap_replies_with_the_screen_and_no_advice_to_call_snapshot() {
    let rig = rig(vec![tree(&["One", "Two"])]).await;
    rig.call("snapshot", json!({})).await;
    let r = rig.call("tap", json!({"target": "Nope"})).await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    let text = texts(&r).join("\n");
    assert!(
        text.ends_with("The screen is as you last saw it, and nothing on it matches."),
        "{text}"
    );
    assert!(!text.contains("`snapshot`"), "{text}");
}

#[tokio::test]
async fn then_written_as_a_json_string_or_one_bare_text_is_read_as_the_list() {
    let rig = rig(vec![tree(&["One", "Two", "Three"])]).await;
    rig.call("snapshot", json!({})).await;
    let r = rig
        .call(
            "tap",
            json!({"target": "One", "then": "[\"Two\",\"Three\"]"}),
        )
        .await;
    let text = texts(&r).join("\n");
    assert!(text.starts_with("ok 3 steps"), "{text}");
    let r = rig
        .call("tap", json!({"target": "One", "then": "Two"}))
        .await;
    let text = texts(&r).join("\n");
    assert!(text.starts_with("ok 2 steps"), "{text}");
    assert!(text.contains("\n2. tapped @e2"), "{text}");
}

#[tokio::test]
async fn a_then_text_that_is_not_on_the_screen_stops_there_and_names_the_step() {
    let rig = rig(vec![tree(&["One", "Two"])]).await;
    rig.call("snapshot", json!({})).await;
    let r = rig
        .call("tap", json!({"target": "One", "then": ["Nope", "Two"]}))
        .await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    let text = texts(&r).join("\n");
    assert!(text.starts_with("step 2 of 3 failed"), "{text}");
    assert!(text.contains("1. tapped @e1"), "{text}");
}

#[tokio::test]
async fn typing_with_then_types_and_taps_the_texts_after_it() {
    let rig = rig(vec![field_tree("", false), field_tree("hi", false)]).await;
    rig.call("snapshot", json!({})).await;
    let r = rig
        .call(
            "type",
            json!({"text": "hi", "into": "@e1", "then": ["@e1"]}),
        )
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let text = texts(&r).join("\n");
    assert!(text.starts_with("ok 2 steps"), "{text}");
    assert!(text.contains("\n1. typed into @e1"), "{text}");
    assert!(text.contains("\n2. tapped @e1"), "{text}");
}

#[tokio::test]
async fn a_secret_typed_as_a_batch_step_or_before_a_then_reaches_no_reply_and_no_file() {
    let rig = rig(vec![field_tree("", false), field_tree("hunter2", false)]).await;
    rig.call("snapshot", json!({})).await;
    let r = rig
        .call(
            "batch",
            json!({"steps": [
                {"tool": "type", "text": "hunter2", "into": "@e1", "secret": true},
                {"tool": "press", "key": "enter"}
            ]}),
        )
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let all = serde_json::to_string(&r).unwrap();
    assert!(!all.contains("hunter2"), "{all}");
    assert!(all.contains("1. typed into @e1"), "{all}");
    let r = rig
        .call(
            "type",
            json!({"text": "hunter2", "into": "@e1", "secret": true, "then": ["@e1"]}),
        )
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let all = serde_json::to_string(&r).unwrap();
    assert!(!all.contains("hunter2"), "{all}");
    assert!(all.contains("2. tapped @e1"), "{all}");
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
async fn a_batch_refuses_unknown_tools_and_unknown_arguments_before_acting() {
    let rig = rig(vec![field_tree("", false)]).await;
    for (step, why) in [
        (json!({"tool": "snapshot"}), "cannot be a step"),
        (
            json!({"tool": "tap", "target": "@e1", "colour": "red"}),
            "colour",
        ),
        (json!({"target": "@e1"}), "needs `tool`"),
        (
            json!({"tool": "press", "key": "back", "device": "android:emulator-9999"}),
            "`device` goes on the batch",
        ),
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
async fn a_screenshot_within_an_element_says_where_its_top_left_is_and_is_not_the_compare_baseline()
{
    let frame = phonedriver_video::frame_store::Frame::from_i420(
        360,
        800,
        vec![128; 360 * 800],
        vec![128; 360 * 200],
        vec![128; 360 * 200],
    );
    let whole = phonedriver_core::device::Image {
        width: 360,
        height: 800,
        format: phonedriver_core::device::ImageFormat::Png,
        bytes: phonedriver_video::image::encode_png(&frame, 1.0).unwrap(),
    };
    let fake = phonedriver_core::testing::FakeDevice::new(vec![tree(&["One", "Two"])])
        .with_screenshot(whole);
    let rig = rig_with(Arc::new(fake)).await;
    let r = rig.call("screenshot", json!({"within": "Two"})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert_eq!(
        texts(&r),
        ["360x76 at scale 1, top left at screen point 0,192: add it to a pixel for `tap x,y`"]
    );
    assert!(r.content.iter().any(|c| c.as_image().is_some()));
    let r = rig.call("screenshot", json!({"compare": true})).await;
    assert_eq!(
        r.is_error,
        Some(true),
        "a part of the screen is nothing to compare with: {r:?}"
    );
    let r = rig
        .call("screenshot", json!({"compare": true, "within": "@e1"}))
        .await;
    assert_eq!(
        texts(&r),
        ["`compare` is of the whole screen: leave `within` out"]
    );
}

/// Decision D129: files are a tool of their own. The clipboard takes no file, and a `files` call with no argument
/// says what a pull could name, with the way to see more of one folder.
#[tokio::test]
async fn files_are_their_own_tool_and_a_call_with_no_argument_lists_what_a_pull_can_name() {
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![tree(&[
        "One",
    ])]));
    fake.with_file_aged("/sdcard/Download/report.pdf", b"BYTESOFTHEREPORT", 7200);
    for n in 0..12 {
        fake.with_file_aged(&format!("/sdcard/DCIM/Camera/IMG_{n:02}.jpg"), b"abc", n);
    }
    let folder = tempfile::tempdir().unwrap();
    let rig = rig_with_files(fake, folder.path()).await;

    let r = rig.call("clipboard", json!({"push": "cat.png"})).await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    assert!(texts(&r)[0].contains("push"), "{r:?}");

    let r = rig.call("files", json!({})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let shown = texts(&r).join("\n");
    let lines: Vec<&str> = shown.lines().collect();
    assert_eq!(lines.len(), 13, "{shown}");
    assert_eq!(lines[0], "DCIM/Camera/IMG_00.jpg · 3 bytes · now");
    assert_eq!(lines[10], "Download/report.pdf · 16 bytes · 2h ago");
    assert_eq!(lines[11], "2 more, older");
    assert_eq!(
        lines[12],
        "hint: `list` with a folder, such as `DCIM/Camera`, shows more of it"
    );
    assert!(!shown.contains("BYTESOF"), "{shown}");

    let r = rig.call("files", json!({"list": "Music"})).await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    let r = rig.call("files", json!({"list": "Pictures"})).await;
    assert_eq!(texts(&r), ["no files in Pictures"]);

    let tools = rig.client.list_all_tools().await.unwrap();
    let files = tools.iter().find(|t| t.name == "files").unwrap();
    let text = serde_json::to_string(files).unwrap();
    assert!(
        text.contains("\"push\"") && text.contains("\"pull\""),
        "{text}"
    );
    let clipboard = tools.iter().find(|t| t.name == "clipboard").unwrap();
    assert!(
        !serde_json::to_string(clipboard).unwrap().contains("push"),
        "{clipboard:?}"
    );
}

#[tokio::test]
async fn a_file_is_refused_until_the_person_names_a_folder_and_says_how() {
    let rig = rig(vec![tree(&["One"])]).await;
    let r = rig.call("files", json!({"push": "cat.png"})).await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    assert!(
        texts(&r)[0].starts_with(
            "no folder is allowed for files: set PHONEDRIVER_FILES to the one folder files may go to and come from"
        ),
        "{r:?}"
    );
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

#[tokio::test]
async fn apps_grant_and_revoke_take_permissions_and_say_what_they_did() {
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![tree(&[
        "One",
    ])]));
    fake.install(&[("com.example.app", "Example")]);
    let rig = rig_with(fake.clone()).await;
    let r = rig
        .call(
            "apps",
            json!({"action": "grant", "app": "Example", "permissions": ["camera"]}),
        )
        .await;
    assert_eq!(texts(&r), ["ok granted camera to Example"]);
    let r = rig
        .call(
            "apps",
            json!({"action": "revoke", "app": "Example", "permissions": ["camera"]}),
        )
        .await;
    assert_eq!(texts(&r), ["ok revoked camera from Example"]);
    let tools = rig.client.list_all_tools().await.unwrap();
    let apps = tools.iter().find(|t| t.name == "apps").unwrap();
    assert!(
        serde_json::to_string(apps).unwrap().contains("revoke"),
        "{apps:?}"
    );
}

fn bare_screen() -> RawTree {
    RawTree::from_android_json(
        r#"{"windows":[{"id":1,"type":"application","package":"com.game","bounds":[0,0,360,800]}],"nodes":[
        {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.view.SurfaceView","b":[0,0,360,800],"f":"vVe"}]}"#,
        phonedriver_core::geometry::Scale { density: 1.0 },
    )
    .unwrap()
}

#[tokio::test]
async fn a_screen_with_nothing_to_act_on_comes_with_its_picture() {
    let rig = rig(vec![bare_screen()]).await;
    let r = rig.call("snapshot", json!({})).await;
    let text = texts(&r).join("\n");
    assert!(
        r.content.iter().any(|c| c.as_image().is_some()),
        "no image: {text}"
    );
    assert!(
        text.contains(
            "\nthis screen gives no element list: its picture is attached; tap a point on it as `x,y`"
        ),
        "{text}"
    );
    assert!(!text.contains("phonedriver screenshot"), "{text}");

    let r = rig
        .call("tap", json!({"target": "100,200", "autonomous": true}))
        .await;
    let text = texts(&r).join("\n");
    assert!(
        r.content.iter().any(|c| c.as_image().is_some()),
        "no image after a tap: {text}"
    );
    assert_eq!(text.matches("its picture is attached").count(), 1, "{text}");
}

#[tokio::test]
async fn a_screen_with_elements_comes_without_a_picture() {
    let rig = rig(vec![tree(&["One"])]).await;
    let r = rig.call("snapshot", json!({})).await;
    assert!(r.content.iter().all(|c| c.as_image().is_none()), "{r:?}");
    assert!(!texts(&r).join("\n").contains("picture"), "{r:?}");
    let r = rig.call("tap", json!({"target": "One"})).await;
    assert!(r.content.iter().all(|c| c.as_image().is_none()), "{r:?}");
}

#[tokio::test]
async fn snapshot_with_page_reads_the_web_page_whole_and_find_keeps_its_matching_lines() {
    use phonedriver_core::device::WebPage;
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![tree(&[
        "One",
    ])]));
    let body: String = (1..=300)
        .map(|n| format!("Line {n} of a long page, forty characters wide.\n"))
        .collect();
    fake.show_page(WebPage {
        title: "Example Domain".into(),
        url: "https://example.com/".into(),
        text: format!("Example Domain\n\nBorn\nDecember 9, 1906\n{body}"),
    });
    let rig = rig_with(fake.clone()).await;
    let r = rig.call("snapshot", json!({"page": true})).await;
    let text = &texts(&r)[0];
    assert!(
        text.starts_with("page \"Example Domain\" https://example.com/\nExample Domain\nBorn\n"),
        "{text}"
    );
    let end: Vec<&str> = text.lines().rev().take(2).collect();
    assert!(
        end[1].starts_with("… ") && end[1].contains(" more lines ("),
        "{}",
        end[1]
    );
    assert_eq!(end[0], "hint: `find` keeps only the lines with those words");
    let r = rig
        .call("snapshot", json!({"page": true, "find": "born"}))
        .await;
    assert_eq!(
        texts(&r),
        [
            "page \"Example Domain\" https://example.com/\nBorn\nDecember 9, 1906\nLine 1 of a long page, forty characters wide."
        ]
    );
}

#[tokio::test]
async fn replay_saves_what_the_session_did_and_runs_it_again_step_by_step() {
    use phonedriver_core::testing::Call;
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![tree(&[
        "One", "Two",
    ])]));
    let rig = rig_with(fake.clone()).await;
    let taps = || {
        fake.calls()
            .iter()
            .filter(|c| matches!(c, Call::Gesture(_)))
            .count()
    };
    let file = rig.home.path().join("run.steps");
    let path = file.to_str().unwrap();
    let r = rig
        .call("replay", json!({"file": path, "save": true}))
        .await;
    assert!(texts(&r)[0].starts_with("nothing to save"), "{r:?}");

    rig.call("snapshot", json!({})).await;
    rig.call("tap", json!({"target": "@e1"})).await;
    rig.call("tap", json!({"target": "Two"})).await;
    assert_eq!(taps(), 2);
    let r = rig
        .call("replay", json!({"file": path, "save": true}))
        .await;
    assert_eq!(
        texts(&r),
        [format!(
            "saved 2 steps to {path}\n1. tap \"One\"\n2. tap \"Two\"\nthe file holds one step a line: delete the line of a step that was a detour"
        )]
    );
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(
        saved
            .lines()
            .next()
            .unwrap()
            .contains(r#""target":{"role":"button","text":"One"}"#),
        "{saved}"
    );
    let r = rig
        .call("replay", json!({"file": path, "save": true}))
        .await;
    assert!(texts(&r)[0].ends_with("is as it was)"), "{r:?}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), saved);

    let r = rig.call("replay", json!({"file": path})).await;
    let text = texts(&r).join("\n");
    assert_ne!(r.is_error, Some(true), "{text}");
    assert!(text.starts_with("replayed 2 steps in "), "{text}");
    assert!(text.contains("\n1. tapped @e1 \"One\""), "{text}");
    assert!(text.contains("screen now:\n"), "{text}");
    assert_eq!(taps(), 4);

    let more = format!(
        "{saved}{}\n{}\n{}\n",
        json!({"method": "act", "params": {"action": {"kind": "tap", "target": {"text": "Gone"}}}}),
        json!({"method": "act", "params": {"action": {"kind": "tap", "target": {"text": "One"}}}, "changed": true}),
        json!({"method": "act", "params": {"action": {"kind": "tap", "target": {"text": "Two"}}}}),
    );
    std::fs::write(&file, more).unwrap();
    let r = rig.call("replay", json!({"file": path})).await;
    let text = texts(&r).join("\n");
    assert_eq!(r.is_error, Some(true), "{text}");
    assert!(
        text.starts_with("step 3 of 5 (tap \"Gone\") failed: no element matches \"Gone\""),
        "{text}"
    );
    assert!(text.contains("\n2. tapped @e2 \"Two\""), "{text}");
    assert!(
        text.contains(
            "Do step 3 yourself, then `replay` the same file with `from` 4 for the 2 steps left."
        ),
        "{text}"
    );
    assert_eq!(taps(), 6);

    let r = rig.call("replay", json!({"file": path, "from": 4})).await;
    let text = texts(&r).join("\n");
    assert!(
        text.starts_with("step 4 of 5 (tap \"One\") failed: it changed nothing this time; when it was saved it changed the screen"),
        "{text}"
    );
    let r = rig.call("replay", json!({"file": path, "from": 5})).await;
    assert!(texts(&r)[0].starts_with("replayed 1 step in "), "{r:?}");
    let r = rig.call("replay", json!({"file": path, "from": 6})).await;
    assert!(texts(&r)[0].contains("there is no step 6"), "{r:?}");
}

#[tokio::test]
async fn a_save_that_cannot_write_leaves_the_steps_for_the_next_save() {
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![tree(&[
        "One",
    ])]));
    let rig = rig_with(fake).await;
    rig.call("snapshot", json!({})).await;
    rig.call("tap", json!({"target": "One"})).await;
    let folder = rig.home.path().join("e2e");
    let file = folder.join("run.steps");
    let r = rig
        .call("replay", json!({"file": "e2e/run.steps", "save": true}))
        .await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    assert!(texts(&r)[0].starts_with("cannot write "), "{r:?}");
    assert!(!file.exists());

    std::fs::create_dir(&folder).unwrap();
    let r = rig
        .call("replay", json!({"file": "e2e/run.steps", "save": true}))
        .await;
    assert!(texts(&r)[0].starts_with("saved 1 step to "), "{r:?}");
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 1);
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);
}

/// Issue #52: three things the command line could do and an MCP agent could not, though the description or the
/// daemon already had them.
#[tokio::test]
async fn logs_take_since_and_replay_takes_screens_inside_its_folder_and_snapshot_shows_audit() {
    let rig = rig(vec![tree(&["One", "Two"])]).await;
    let r = rig.call("logs", json!({"since": 5})).await;
    assert!(
        !texts(&r).join("\n").contains("since"),
        "`since` was not taken: {r:?}"
    );

    rig.call("snapshot", json!({})).await;
    rig.call("tap", json!({"target": "One"})).await;
    rig.call("replay", json!({"file": "run.steps", "save": true}))
        .await;
    let r = rig
        .call("replay", json!({"file": "run.steps", "screens": "shots"}))
        .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let kept = files(&rig.home.path().join("shots"));
    assert!(
        kept.iter()
            .any(|f| f.extension().is_some_and(|e| e == "png")),
        "{kept:?} {:?}",
        texts(&r)
    );
    assert!(!texts(&r)[0].contains("--screens"), "{:?}", texts(&r));
    let r = rig
        .call(
            "replay",
            json!({"file": "run.steps", "screens": "../shots"}),
        )
        .await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    assert!(texts(&r)[0].contains("not inside"), "{r:?}");

    let tools = rig.client.list_all_tools().await.unwrap();
    let schema = |name: &str| {
        let t = tools.iter().find(|t| t.name == name).unwrap();
        serde_json::to_string(&*t.input_schema).unwrap()
    };
    assert!(schema("snapshot").contains("\"audit\""));
    assert!(!schema("logs").contains("\"since\"") && !schema("replay").contains("\"screens\""));
    let devices = tools.iter().find(|t| t.name == "devices").unwrap();
    assert!(devices.description.as_deref().unwrap().contains("+name"));
    for name in ["scroll", "swipe"] {
        let t = tools.iter().find(|t| t.name == name).unwrap();
        assert_eq!(
            t.input_schema["properties"]["direction"],
            json!({"enum": ["up", "down", "left", "right"]}),
            "{name}"
        );
    }
    let swipe = tools.iter().find(|t| t.name == "swipe").unwrap();
    assert!(
        !schema("swipe").contains("\"required\""),
        "{:?}",
        swipe.input_schema
    );
    let r = rig.call("swipe", json!({"pinch": "in"})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
}

#[tokio::test]
async fn replay_saves_only_a_steps_file_inside_its_folder_and_over_nothing_but_steps() {
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![tree(&[
        "One",
    ])]));
    let rig = rig_with(fake).await;
    let outside = tempfile::tempdir().unwrap();
    let env_file = outside.path().join("project.steps");
    std::fs::write(&env_file, "API_KEY=keep\n").unwrap();
    let notes = rig.home.path().join("notes.steps");
    std::fs::write(&notes, "my notes, not steps\n").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&env_file, rig.home.path().join("link.steps")).unwrap();
        std::os::unix::fs::symlink(outside.path(), rig.home.path().join("out")).unwrap();
    }
    let save = async |file: &str| {
        rig.call("snapshot", json!({})).await;
        rig.call("tap", json!({"target": "One"})).await;
        rig.call("replay", json!({"file": file, "save": true}))
            .await
    };
    let absolute = env_file.to_str().unwrap().to_string();
    let mut refused = vec![
        (absolute.as_str(), "inside"),
        ("../x.steps", "`..`"),
        ("run.txt", ".steps"),
        ("notes.steps", "not a steps file"),
    ];
    if cfg!(unix) {
        refused.push(("link.steps", "link"));
        refused.push(("out/project.steps", "inside"));
    }
    for (file, why) in refused {
        let r = save(file).await;
        let text = texts(&r).join("\n");
        assert_eq!(r.is_error, Some(true), "{file}: {text}");
        assert!(text.contains(why), "{file}: {text}");
    }
    assert_eq!(
        std::fs::read_to_string(&env_file).unwrap(),
        "API_KEY=keep\n"
    );
    assert_eq!(
        std::fs::read_to_string(&notes).unwrap(),
        "my notes, not steps\n"
    );
    assert!(!outside.path().join("x.steps").exists());
    assert!(!rig.home.path().join("run.txt").exists());

    let r = save("run.steps").await;
    assert!(texts(&r)[0].starts_with("saved "), "{r:?}");
    let r = save("run.steps").await;
    assert!(texts(&r)[0].starts_with("saved "), "{r:?}");
    let inside = rig.home.path().join("run.steps");
    let r = save(inside.to_str().unwrap()).await;
    assert!(texts(&r)[0].starts_with("saved "), "{r:?}");
}

#[tokio::test]
async fn a_replayed_shell_step_that_exits_other_than_it_expects_fails_the_replay() {
    use phonedriver_core::device::ShellReply;
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![tree(&[
        "One",
    ])]));
    let rig = rig_with(fake.clone()).await;
    let exits = |exit, timed_out| {
        fake.answer_shell(ShellReply {
            exit,
            timed_out,
            ..ShellReply::default()
        })
    };
    let file = rig.home.path().join("check.steps");
    let path = file.to_str().unwrap();

    exits(3, false);
    rig.call("shell", json!({"command": "exit 3", "settle": false}))
        .await;
    rig.call("replay", json!({"file": path, "save": true}))
        .await;
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(saved["expect_exit"], 3, "{saved}");
    let r = rig.call("replay", json!({"file": path})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");

    let write = |step: Value| std::fs::write(&file, format!("{step}\n")).unwrap();
    let shell = json!({"method": "shell", "params": {"command": "ping -c1 host", "settle": false}});
    let with = |extra: Value| {
        let mut step = shell.clone();
        step.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        step
    };

    exits(1, false);
    write(shell.clone());
    let r = rig.call("replay", json!({"file": path})).await;
    let text = texts(&r).join("\n");
    assert_eq!(r.is_error, Some(true), "{text}");
    assert!(
        text.starts_with(
            "step 1 of 1 (shell ping -c1 host) failed: it exited 1; the step expects 0"
        ),
        "{text}"
    );
    write(with(json!({"expect_exit": 1})));
    let r = rig.call("replay", json!({"file": path})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    write(with(json!({"expect_exit": null})));
    let r = rig.call("replay", json!({"file": path})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    write(with(json!({"expect_exit": 2})));
    let r = rig.call("replay", json!({"file": path})).await;
    let text = texts(&r).join("\n");
    assert!(
        text.starts_with(
            "step 1 of 1 (shell ping -c1 host) failed: it exited 1; the step expects 2"
        ),
        "{text}"
    );

    exits(124, true);
    write(with(json!({"expect_exit": 124})));
    let r = rig.call("replay", json!({"file": path})).await;
    let text = texts(&r).join("\n");
    assert_eq!(r.is_error, Some(true), "{text}");
    assert!(
        text.starts_with(
            "step 1 of 1 (shell ping -c1 host) failed: it ran past its time limit and was stopped"
        ),
        "{text}"
    );
}

#[tokio::test]
async fn screenshot_with_record_starts_and_stops_a_video_and_returns_text_only() {
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![tree(&[
        "One",
    ])]));
    fake.with_recorder();
    let rig = rig_with(fake.clone()).await;
    let r = rig.call("screenshot", json!({"record": "start"})).await;
    assert!(r.content.iter().all(|c| c.as_image().is_none()), "{r:?}");
    let started = texts(&r).join("\n");
    assert!(
        started.starts_with("recording the screen to ") && started.contains(".mp4"),
        "{started}"
    );
    rig.call("tap", json!({"target": "One"})).await;
    let r = rig.call("screenshot", json!({"record": "stop"})).await;
    let stopped = texts(&r).join("\n");
    assert!(stopped.contains("1.5 s, 360x800, 2 frames"), "{stopped}");
    assert!(
        stopped.contains("steps: 1 with their times in "),
        "{stopped}"
    );
    let r = rig.call("screenshot", json!({"record": "pause"})).await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
}

#[tokio::test]
async fn logs_prints_the_crash_then_the_last_lines_of_the_app_in_front() {
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![tree(&[
        "One",
    ])]));
    fake.answer_logs(phonedriver_core::device::LogReply {
        text: "I/Example( 222): started\nE/Example( 222): failed to load\n".into(),
        crash: Some(
            "java.lang.IllegalStateException: boom at com.example.Main.onClick(Main.kt:12)".into(),
        ),
    });
    let rig = rig_with(fake.clone()).await;
    let r = rig.call("logs", json!({})).await;
    assert_eq!(
        texts(&r),
        [
            "crashed: java.lang.IllegalStateException: boom at com.example.Main.onClick(Main.kt:12)\nI/Example( 222): started\nE/Example( 222): failed to load"
        ]
    );
    let r = rig.call("logs", json!({"grep": "failed"})).await;
    assert!(
        texts(&r)[0].ends_with("E/Example( 222): failed to load"),
        "{r:?}"
    );
}

#[tokio::test]
async fn network_prints_the_networks_and_wait_takes_a_vpn_state_or_a_log_line() {
    use phonedriver_core::network::{NetworkLink, NetworkView};
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![tree(&[
        "One",
    ])]));
    fake.answer_networks(vec![NetworkView {
        networks: vec![NetworkLink {
            id: 101,
            transports: vec!["wifi".into()],
            default: true,
            dns: vec!["10.0.2.3".into()],
            ..NetworkLink::default()
        }],
        private_dns: Some("off".into()),
        ..NetworkView::default()
    }]);
    fake.answer_log_lines(&["I/Vpn( 9): tunnel up"]);
    let rig = rig_with(fake.clone()).await;
    let r = rig.call("network", json!({})).await;
    assert_eq!(
        texts(&r),
        ["101 wifi (default) · dns 10.0.2.3\nprivate dns off\nalways-on vpn none\nlockdown off"]
    );
    let r = rig.call("wait", json!({"log": "tunnel up"})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert!(
        texts(&r)
            .concat()
            .contains("log line: I/Vpn( 9): tunnel up"),
        "{r:?}"
    );
    let r = rig
        .call("wait", json!({"vpn": "up", "timeout_ms": 1500}))
        .await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    assert!(
        texts(&r).concat().contains("last seen: wifi (default)"),
        "{r:?}"
    );
    let r = rig.call("wait", json!({"vpn": "sideways"})).await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    let r = rig.call("wait", json!({"vpn": "up", "log": "x"})).await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
}

#[tokio::test]
async fn setting_finds_reads_and_writes_a_setting_by_key() {
    use phonedriver_core::settings::{SettingItem, SettingScreen, SettingsIndex};
    const KEY: &str = "dark_ui_mode/dark_ui_activated";
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![tree(&[
        "One",
    ])]));
    fake.with_settings(
        SettingsIndex {
            screens: vec![SettingScreen {
                key: "dark_ui_mode".into(),
                about: "Dark theme.".into(),
                link: None,
            }],
            items: vec![SettingItem {
                key: KEY.into(),
                purpose: "Whether the system dark theme is manually enabled".into(),
                values: "true | false".into(),
                writable: true,
                confirm: false,
                screen: 0,
            }],
        },
        &[(KEY, "false")],
    );
    let rig = rig_with(fake.clone()).await;
    let r = rig.call("setting", json!({"find": "dark theme"})).await;
    assert_eq!(
        texts(&r),
        [format!(
            "{KEY}  [true | false]  Whether the system dark theme is manually enabled"
        )]
    );
    let r = rig.call("setting", json!({"find": "teapot"})).await;
    assert!(
        texts(&r)[0].starts_with("no setting matches \"teapot\""),
        "{r:?}"
    );
    let r = rig.call("setting", json!({"key": KEY})).await;
    assert_eq!(texts(&r), [format!("{KEY} = false")]);
    let r = rig
        .call("setting", json!({"key": KEY, "value": "true"}))
        .await;
    assert_eq!(texts(&r), [format!("ok {KEY} = true (was false)")]);
    let r = rig.call("devices", json!({"release": true})).await;
    assert_eq!(
        texts(&r),
        [format!(
            "released\nsettings phonedriver changed and has not put back: {KEY} = true (was false)\n`setting restore` writes them back; leave them when the change was the task"
        )]
    );
    let r = rig.call("setting", json!({"restore": true})).await;
    assert_eq!(texts(&r), [format!("ok {KEY} = false (was true)")]);
    let r = rig
        .call("setting", json!({"restore": true, "key": KEY}))
        .await;
    assert_eq!(r.is_error, Some(true), "nothing of that key is left: {r:?}");
    let r = rig.call("setting", json!({"restore": "true"})).await;
    assert!(
        texts(&r)[0].starts_with("nothing to restore"),
        "an agent sent `restore` as a string when the lean schema did not show it: {r:?}"
    );
    let r = rig
        .call("setting", json!({"restore": true, "find": "dark"}))
        .await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    let r = rig.call("setting", json!({"values": {KEY: "true"}})).await;
    assert_eq!(texts(&r), [format!("ok {KEY} = true (was false)")]);
    let r = rig.call("setting", json!({"values": {KEY: "false"}})).await;
    assert_eq!(texts(&r), [format!("ok {KEY} = false")]);
    let r = rig.call("setting", json!({"value": "true"})).await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    let r = rig.call("setting", json!({"find": "a", "open": "b"})).await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
}

fn test_socket(home: &std::path::Path, name: &str) -> std::path::PathBuf {
    if cfg!(windows) {
        let scope = home.file_name().unwrap().to_string_lossy();
        std::path::PathBuf::from(format!(r"\\.\pipe\phonedriver-test-{scope}-{name}"))
    } else {
        home.join("run").join(name)
    }
}

struct TwoDevices(Vec<(String, Arc<dyn phonedriver_core::device::Device>)>);

#[async_trait::async_trait]
impl phonedriver::daemon::DeviceSource for TwoDevices {
    async fn list(
        &self,
    ) -> Result<Vec<phonedriver::daemon::Listed>, phonedriver_core::error::Error> {
        Ok(self
            .0
            .iter()
            .map(|(id, device)| phonedriver::daemon::Listed {
                id: id.clone(),
                model: "Fake".into(),
                os: Some("FakeOS 1".into()),
                tier: "fake".into(),
                capabilities: device.info().capabilities,
            })
            .collect())
    }

    async fn connect(
        &self,
        id: &str,
    ) -> Result<Arc<dyn phonedriver_core::device::Device>, phonedriver_core::error::Error> {
        Ok(self.0.iter().find(|(d, _)| d == id).unwrap().1.clone())
    }
}

#[tokio::test]
async fn devices_use_sends_the_calls_that_follow_to_that_device() {
    let rig = rig_on(Arc::new(TwoDevices(vec![
        ("fake:1".into(), pixel_device(vec![tree(&["First"])])),
        ("fake:2".into(), pixel_device(vec![tree(&["Second"])])),
    ])))
    .await;
    let r = rig.call("snapshot", json!({})).await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    let r = rig.call("devices", json!({"use": "fake:2"})).await;
    assert!(
        texts(&r)
            .join("\n")
            .contains("default device: fake:2 (used when `device` is omitted)"),
        "{r:?}"
    );
    let r = rig.call("snapshot", json!({})).await;
    assert!(texts(&r).join("\n").contains("Second"), "{r:?}");
    rig.call("devices", json!({"use": "fake:1"})).await;
    let r = rig.call("snapshot", json!({})).await;
    assert!(texts(&r).join("\n").contains("First"), "{r:?}");
}

#[tokio::test]
async fn replay_takes_vars_and_refuses_a_file_whose_variables_are_not_all_given() {
    let rig = rig(vec![tree(&["One", "Two"])]).await;
    let file = rig.home.path().join("vars.steps");
    let path = file.to_str().unwrap();
    let tap = json!({"method": "act", "params": {"action": {"kind": "tap", "target": {"text": "{{which}}"}}}});
    std::fs::write(&file, format!("{tap}\n")).unwrap();

    let r = rig.call("replay", json!({"file": path})).await;
    let text = texts(&r).join("\n");
    assert_eq!(r.is_error, Some(true), "{text}");
    assert!(
        text.contains("needs 1 variable nobody gave: which"),
        "{text}"
    );
    assert!(text.contains("Add --var which=VALUE"), "{text}");

    let r = rig
        .call("replay", json!({"file": path, "vars": {"which": "Two"}}))
        .await;
    let text = texts(&r).join("\n");
    assert_ne!(r.is_error, Some(true), "{text}");
    assert!(text.contains("1. tapped @e2 \"Two\""), "{text}");
}

#[tokio::test]
async fn a_shell_tool_call_and_a_replayed_shell_step_hand_the_device_the_same_command() {
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![tree(&[
        "One",
    ])]));
    let info = phonedriver_core::device::Device::info(&*fake).clone();
    let rig = rig_with(Arc::new(fakes::Pixels(fake.clone(), info))).await;
    let command = "simctl io screenshot /Users/me/a.png";
    let steps = rig.home.path().join("shell.steps");
    std::fs::write(
        &steps,
        format!(
            "{}\n",
            json!({"method": "shell", "params": {"command": command}})
        ),
    )
    .unwrap();
    for (tool, args) in [
        ("shell", json!({"command": command})),
        ("replay", json!({"file": steps.to_str().unwrap()})),
    ] {
        let r = rig.call(tool, args).await;
        assert_ne!(r.is_error, Some(true), "{tool}: {r:?}");
    }
    let sent: Vec<String> = fake
        .calls()
        .into_iter()
        .filter_map(|c| match c {
            phonedriver_core::testing::Call::Shell(s) => Some(s.command),
            _ => None,
        })
        .collect();
    assert_eq!(sent, [command, command]);
}

/// Decision D118: a path needs no tool of its own. `replay` saves one where `launch` looks for it, `launch` names
/// it, and `replay` goes down it and answers with the screen it arrived on.
#[tokio::test(flavor = "multi_thread")]
async fn a_path_is_saved_named_by_launch_and_gone_down_with_the_tools_there_are() {
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![tree(&[
        "One", "Two",
    ])]));
    fake.install(&[("com.example", "Example")]);
    let rig = rig_with(fake).await;
    rig.call("snapshot", json!({})).await;
    rig.call("tap", json!({"target": "Two"})).await;
    let file = "paths/com.example/two.steps";
    let r = rig
        .call("replay", json!({"file": file, "save": true}))
        .await;
    assert_eq!(r.is_error, Some(false), "{r:?}");
    assert!(texts(&r)[0].contains("\"One\""), "{r:?}");
    assert!(rig.home.path().join(file).exists());

    let r = rig.call("launch", json!({"app": "com.example"})).await;
    let shown = texts(&r).join("\n");
    assert!(
        shown.contains("paths: two · replay paths/com.example/<name>.steps"),
        "{shown}"
    );

    let r = rig.call("replay", json!({"file": file})).await;
    let shown = texts(&r).join("\n");
    assert_eq!(r.is_error, Some(false), "{shown}");
    assert!(shown.starts_with("arrived at \"One\""), "{shown}");
    assert!(shown.contains("\"Two\""), "{shown}");
}

#[tokio::test]
async fn a_long_call_tells_a_client_that_gave_a_progress_token_what_it_waits_for() {
    let rig = rig(vec![tree(&["One", "Two"])]).await;
    rig.call("snapshot", json!({})).await;

    let (r, said) = rig.call_told("snapshot", json!({})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    assert_eq!(said, [""; 0], "a call that is over at once says nothing");

    let (r, said) = rig
        .call_told("wait", json!({"condition": "Zebra", "timeout_ms": 7400}))
        .await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    assert_eq!(
        said,
        [
            "wait: the condition has not held yet · 2s of 7s",
            "wait: the condition has not held yet · 7s of 7s"
        ]
    );
}

#[tokio::test]
async fn a_client_that_gave_no_progress_token_is_sent_no_progress() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    let rig = rig(vec![tree(&["One", "Two"])]).await;
    let (client_io, server_io) = tokio::io::duplex(1 << 20);
    let mcp = Mcp::new(Some(rig.socket.clone()), "mcp-raw".into());
    tokio::spawn(async move {
        let running = mcp.serve(server_io).await.unwrap();
        let _ = running.waiting().await;
    });
    let (read, mut write) = tokio::io::split(client_io);
    let mut lines = tokio::io::BufReader::new(read).lines();
    let mut send = async |message: Value| {
        write
            .write_all(format!("{message}\n").as_bytes())
            .await
            .unwrap()
    };
    send(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "raw", "version": "0"}
    }}))
    .await;
    lines.next_line().await.unwrap().unwrap();
    send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).await;
    let wait = json!({"name": "wait", "arguments": {"condition": "Zebra", "timeout_ms": 2400}});
    let mut until_reply = async |id: u64| {
        let mut sent = Vec::new();
        loop {
            let line: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            if line["id"] == id {
                return sent;
            }
            sent.push(line);
        }
    };

    send(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": wait})).await;
    assert_eq!(until_reply(2).await, [Value::Null; 0]);

    let mut told = wait.clone();
    told["_meta"] = json!({"progressToken": "mine"});
    send(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": told})).await;
    assert_eq!(
        until_reply(3).await,
        [
            json!({"jsonrpc": "2.0", "method": "notifications/progress", "params": {
                "progressToken": "mine", "progress": 2.0, "total": 2.0,
                "message": "wait: the condition has not held yet · 2s of 2s"
            }})
        ]
    );
}

#[tokio::test]
async fn progress_names_the_replay_step_and_never_carries_text_given_to_the_call() {
    let rig = rig(vec![
        field_tree("", false),
        field_tree("hunter2-typed", false),
    ])
    .await;
    rig.call("snapshot", json!({})).await;
    let file = rig.home.path().join("run.steps");
    std::fs::write(
        &file,
        format!(
            "{}\n{}\n",
            json!({"method": "act", "params": {"action": {"kind": "type", "text": "hunter2-typed"}}}),
            json!({"method": "wait", "params": {"condition": {"appears": {"text": "hunter2-awaited", "role": null}}, "timeout_ms": 2400}}),
        ),
    )
    .unwrap();
    let (r, said) = rig
        .call_told("replay", json!({"file": file.to_str().unwrap()}))
        .await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    assert_eq!(said, ["replay: step 2 of 2 · 2s"]);

    let (_, said) = rig
        .call_told(
            "wait",
            json!({"condition": "hunter2-awaited", "timeout_ms": 2400}),
        )
        .await;
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(!said[0].contains("hunter2"), "{said:?}");
}

/// A date picker as the platform draws it: a month of dated day cells, a button to each neighbouring month, and
/// `OK`. It opens on 15 January 2026 and keeps what was tapped, as the real one does.
fn fake_calendar() -> (
    Arc<phonedriver_core::testing::FakeDevice>,
    Arc<std::sync::Mutex<Vec<String>>>,
) {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    #[derive(Clone)]
    struct Shown {
        open: bool,
        month: i32,
        chosen: (i32, u32),
        picked: String,
    }
    fn buttons(shown: &Shown) -> Vec<(String, bool)> {
        if !shown.open {
            return vec![("Date".into(), false), (shown.picked.clone(), false)];
        }
        let (year, month) = (shown.month.div_euclid(12), shown.month.rem_euclid(12));
        let mut all = vec![
            ("Previous month".to_string(), false),
            ("Next month".to_string(), false),
        ];
        for day in 1..=28 {
            all.push((
                format!("{day:02} {} {year}", MONTHS[month as usize]),
                shown.chosen == (shown.month, day),
            ));
        }
        all.push(("OK".into(), false));
        all
    }
    fn drawn(shown: &Shown) -> RawTree {
        let mut nodes = vec![
            r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}"#
                .to_string(),
        ];
        for (i, (label, chosen)) in buttons(shown).iter().enumerate() {
            let (left, top) = (90 * (i as i32 % 4), 100 + 60 * (i as i32 / 4));
            nodes.push(format!(
                r#"{{"p":0,"w":1,"cls":"android.widget.Button","text":"{label}","b":[{left},{top},{},{}],"f":"vVe{}","a":["tap"]}}"#,
                left + 90,
                top + 60,
                if *chosen { "s" } else { "" }
            ));
        }
        let json = format!(
            r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[{}]}}"#,
            nodes.join(",")
        );
        RawTree::from_android_json(&json, phonedriver_core::geometry::Scale { density: 1.0 })
            .unwrap()
    }
    let state = Arc::new(std::sync::Mutex::new(Shown {
        open: false,
        month: 2026 * 12,
        chosen: (2026 * 12, 15),
        picked: "nothing picked".into(),
    }));
    let taps = Arc::new(std::sync::Mutex::new(Vec::new()));
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![drawn(
        &state.lock().unwrap(),
    )]));
    let tapped = taps.clone();
    fake.on_gesture(move |g| {
        let at = g.fingers[0][0].at;
        let mut shown = state.lock().unwrap();
        let hit = buttons(&shown)
            .into_iter()
            .enumerate()
            .find(|(i, _)| {
                let (left, top) = (90.0 * (*i % 4) as f32, 100.0 + 60.0 * (*i / 4) as f32);
                (left..left + 90.0).contains(&at.x) && (top..top + 60.0).contains(&at.y)
            })
            .map(|(_, (label, _))| label)?;
        tapped.lock().unwrap().push(hit.clone());
        match hit.as_str() {
            "Date" => shown.open = true,
            "Previous month" => shown.month -= 1,
            "Next month" => shown.month += 1,
            "OK" => {
                shown.open = false;
                shown.picked = format!(
                    "picked {}-{:02}-{:02}",
                    shown.chosen.0.div_euclid(12),
                    shown.chosen.0.rem_euclid(12) + 1,
                    shown.chosen.1
                );
            }
            day => shown.chosen = (shown.month, day[..2].parse().ok()?),
        }
        Some(vec![drawn(&shown)])
    });
    (fake, taps)
}

#[tokio::test]
async fn tap_with_select_steps_a_calendar_to_the_date_reads_it_back_and_confirms() {
    let (fake, taps) = fake_calendar();
    let rig = rig_with(fake).await;
    rig.call("snapshot", json!({})).await;

    let r = rig
        .call("tap", json!({"target": "Date", "select": "18/12/2026"}))
        .await;
    assert_eq!(r.is_error, Some(true), "{r:?}");
    assert!(
        texts(&r)[0].contains("`select` is a date as `2026-12-18`"),
        "{r:?}"
    );
    assert!(
        !texts(&r)[0].contains("18/12"),
        "the value is not quoted back: {r:?}"
    );
    assert_eq!(*taps.lock().unwrap(), [""; 0], "refused before any touch");

    let r = rig
        .call("tap", json!({"target": "Date", "select": "2026-03-09"}))
        .await;
    let text = texts(&r).join("\n");
    assert_ne!(r.is_error, Some(true), "{text}");
    let first = text.lines().next().unwrap();
    assert!(
        first.starts_with("ok tapped @e1 \"Date\", set the picker to 2026-03-09, then tapped @e")
            && first.contains("\"OK\" · verified"),
        "{text}"
    );
    assert!(text.contains("\"picked 2026-03-09\""), "{text}");
    assert_eq!(
        *taps.lock().unwrap(),
        ["Date", "Next month", "Next month", "09 March 2026", "OK"]
    );

    taps.lock().unwrap().clear();
    let r = rig
        .call("tap", json!({"target": "Date", "select": "2025-11-03"}))
        .await;
    assert!(texts(&r)[0].contains("\"picked 2025-11-03\""), "{r:?}");
    assert_eq!(taps.lock().unwrap().len(), 7, "{taps:?}");

    taps.lock().unwrap().clear();
    let r = rig
        .call("tap", json!({"target": "Date", "select": "1990-05-01"}))
        .await;
    let text = texts(&r).join("\n");
    assert_eq!(r.is_error, Some(true), "{text}");
    assert!(
        text.contains("the calendar shows 2025-11, 1990-05 is more than 36 months from it, and no list of years opened"),
        "{text}"
    );
    assert_eq!(*taps.lock().unwrap(), ["Date"], "nothing was confirmed");

    let r = rig
        .call("tap", json!({"target": "OK", "select": "19:45"}))
        .await;
    let text = texts(&r).join("\n");
    assert_eq!(r.is_error, Some(true), "{text}");
    assert!(
        text.contains(
            "no time picker is on screen that takes typed digits, has wheels, or has a clock face"
        ),
        "{text}"
    );
}

struct Asked(OneDevice, Arc<std::sync::Mutex<Vec<Option<String>>>>);

#[async_trait::async_trait]
impl phonedriver::daemon::DeviceSource for Asked {
    async fn list(
        &self,
    ) -> Result<Vec<phonedriver::daemon::Listed>, phonedriver_core::error::Error> {
        self.1.lock().unwrap().push(None);
        self.0.list().await
    }
    async fn connect(
        &self,
        id: &str,
    ) -> Result<Arc<dyn phonedriver_core::device::Device>, phonedriver_core::error::Error> {
        self.0.connect(id).await
    }
    async fn list_for(
        &self,
        requested: Option<&str>,
    ) -> Result<Vec<phonedriver::daemon::Listed>, phonedriver_core::error::Error> {
        self.1.lock().unwrap().push(requested.map(str::to_string));
        self.0.list().await
    }
}

#[tokio::test]
async fn a_replay_asks_which_device_is_the_default_once_and_names_it_on_every_step() {
    let asked = Arc::new(std::sync::Mutex::new(Vec::new()));
    let source = Asked(
        OneDevice(pixel_device(vec![tree(&["One", "Two"])])),
        asked.clone(),
    );
    let rig = rig_on(Arc::new(source)).await;
    rig.call("snapshot", json!({})).await;
    let tap = |text: &str| json!({"method": "act", "params": {"action": {"kind": "tap", "target": {"text": text}}}});
    let file = rig.home.path().join("run.steps");
    std::fs::write(
        &file,
        format!("{}\n{}\n{}\n", tap("One"), tap("Two"), tap("One")),
    )
    .unwrap();
    asked.lock().unwrap().clear();
    let r = rig
        .call("replay", json!({"file": file.to_str().unwrap()}))
        .await;
    assert!(texts(&r)[0].starts_with("replayed 3 steps in "), "{r:?}");
    let asked = asked.lock().unwrap().clone();
    assert_eq!(asked[0], None, "{asked:?}");
    assert!(
        asked.len() >= 4 && asked[1..].iter().all(|d| d.as_deref() == Some("fake:1")),
        "{asked:?}"
    );
}
