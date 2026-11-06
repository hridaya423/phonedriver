use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use phone_use::client::Client;
use phone_use::daemon::{Config, Daemon, DeviceSource, Listed, socket};
use phone_use::mcp::Mcp;
use phone_use_core::device::{
    App, AppId, Button, Capabilities, ClipboardOp, Device, DeviceEvent, DeviceInfo, FrameRef,
    Gesture, Image, ImageFormat, Key, Quality, TextInput, Url,
};
use phone_use_core::error::Error;
use phone_use_core::geometry::Scale;
use phone_use_core::raw::RawTree;
use phone_use_core::testing::FakeDevice;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use serde_json::{Value, json};

const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52,
];

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

fn field_tree(value: &str) -> RawTree {
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.widget.EditText","text":"{value}","desc":"Field","b":[0,100,360,160],"f":"vVeEF","a":["tap","set_text"]}}]}}"#
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

struct Pixels(Arc<FakeDevice>, DeviceInfo);

#[async_trait]
impl Device for Pixels {
    fn info(&self) -> &DeviceInfo {
        &self.1
    }
    async fn tree(&self) -> Result<RawTree, Error> {
        self.0.tree().await
    }
    fn frames(&self) -> Option<tokio::sync::watch::Receiver<FrameRef>> {
        self.0.frames()
    }
    async fn screenshot(&self, _q: Quality) -> Result<Image, Error> {
        Ok(Image {
            width: 360,
            height: 800,
            format: ImageFormat::Png,
            bytes: PNG.to_vec(),
        })
    }
    fn events(&self) -> tokio::sync::broadcast::Receiver<DeviceEvent> {
        self.0.events()
    }
    async fn gesture(&self, g: Gesture) -> Result<(), Error> {
        self.0.gesture(g).await
    }
    async fn key(&self, k: Key) -> Result<(), Error> {
        self.0.key(k).await
    }
    async fn button(&self, b: Button) -> Result<(), Error> {
        self.0.button(b).await
    }
    async fn open_url(&self, u: &Url) -> Result<(), Error> {
        self.0.open_url(u).await
    }
    async fn text(&self, t: TextInput) -> Result<(), Error> {
        self.0.text(t).await
    }
    async fn clipboard(&self, op: ClipboardOp) -> Result<Option<String>, Error> {
        self.0.clipboard(op).await
    }
    async fn apps(&self) -> Result<Vec<App>, Error> {
        self.0.apps().await
    }
    async fn launch(&self, app: &AppId) -> Result<(), Error> {
        self.0.launch(app).await
    }
    async fn foreground(&self) -> Result<Option<AppId>, Error> {
        self.0.foreground().await
    }
}

struct OneDevice(Arc<dyn Device>);

#[async_trait]
impl DeviceSource for OneDevice {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(vec![Listed {
            id: "fake:1".into(),
            model: "Fake".into(),
            os: Some("FakeOS 1".into()),
            tier: "fake".into(),
            capabilities: self.0.info().capabilities,
        }])
    }
    async fn connect(&self, _id: &str) -> Result<Arc<dyn Device>, Error> {
        Ok(self.0.clone())
    }
}

struct Rig {
    home: tempfile::TempDir,
    socket: PathBuf,
    client: rmcp::service::RunningService<rmcp::RoleClient, ()>,
}

async fn rig(trees: Vec<RawTree>) -> Rig {
    let fake = Arc::new(FakeDevice::new(trees));
    let mut info = fake.info().clone();
    info.capabilities |= Capabilities::SCREENSHOT;
    let home = tempfile::tempdir().unwrap();
    let daemon = Daemon::new(
        Arc::new(OneDevice(Arc::new(Pixels(fake, info)))),
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
    ) -> Result<Value, phone_use::client::Failure> {
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
async fn tools_list_is_the_ten_tools_with_short_descriptions_and_the_cli_required_arguments() {
    let rig = rig(vec![tree(&["One"])]).await;
    let tools = rig.client.list_all_tools().await.unwrap();
    let mut required: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for t in &tools {
        let d = t.description.as_deref().unwrap_or_default();
        assert!(!d.is_empty() && d.chars().count() < 300, "{}: {d}", t.name);
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
        ("devices", vec![]),
        ("snapshot", vec![]),
        ("screenshot", vec![]),
        ("tap", vec!["target"]),
        ("type", vec!["text"]),
        ("swipe", vec!["direction"]),
        ("scroll", vec!["direction"]),
        ("press", vec!["key"]),
        ("launch", vec![]),
        ("wait", vec!["condition"]),
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
    assert_eq!(texts(&r), vec![format!("{}\nhint: {hint}", want.message())]);
    assert!(texts(&r)[0].contains("e99"));
}

#[tokio::test]
async fn launch_needs_exactly_one_of_app_and_url() {
    let rig = rig(vec![tree(&["One"])]).await;
    for args in [
        json!({}),
        json!({"app": "Clock", "url": "https://example.com"}),
    ] {
        let r = rig.call("launch", args).await;
        assert_eq!(r.is_error, Some(true), "{r:?}");
        assert!(texts(&r)[0].contains("app or url"), "{r:?}");
    }
}

#[tokio::test]
async fn a_secret_typed_over_mcp_reaches_no_reply_no_trace_and_no_file() {
    let rig = rig(vec![field_tree(""), field_tree("hunter2")]).await;
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
