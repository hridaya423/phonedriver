//! `files` over MCP on a person's phone (decision D119): the human is asked through the client for each transfer,
//! shown the file, its size and its hash, and a decline copies nothing.

use crate::fakes;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use fakes::tree;
use phonedriver::daemon::{Config, Daemon, DeviceSource, Listed, socket};
use phonedriver::mcp::Mcp;
use phonedriver_core::device::Device;
use phonedriver_core::error::Error;
use phonedriver_core::testing::{Call, FakeDevice};
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ClientCapabilities, ClientConfig, ElicitRequestParams,
    ElicitResult, ElicitationAction, Implementation,
};
use rmcp::service::RequestContext;
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use serde_json::json;

const PHONE: &str = "android:R5CT123";

struct Phone(Arc<FakeDevice>);

#[async_trait]
impl DeviceSource for Phone {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(vec![Listed {
            id: PHONE.into(),
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

struct Human {
    allows: Arc<Mutex<VecDeque<bool>>>,
    asked: Arc<Mutex<Vec<String>>>,
}

impl ClientHandler for Human {
    fn get_info(&self) -> ClientConfig {
        ClientConfig::new(
            ClientCapabilities::builder().enable_elicitation().build(),
            Implementation::new("files-test", "0"),
        )
    }

    async fn create_elicitation(
        &self,
        request: ElicitRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> Result<ElicitResult, rmcp::ErrorData> {
        let ElicitRequestParams::FormElicitationParams { message, .. } = request else {
            panic!("a form was expected");
        };
        self.asked.lock().unwrap().push(message);
        Ok(match self.allows.lock().unwrap().pop_front() {
            Some(true) => ElicitResult::new(ElicitationAction::Accept)
                .with_content(json!({"choice": "session"})),
            _ => ElicitResult::new(ElicitationAction::Decline),
        })
    }
}

fn text(r: &CallToolResult) -> String {
    r.content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn on_a_phone_the_human_is_asked_for_each_file_and_a_decline_copies_nothing() {
    let folder = tempfile::tempdir().unwrap();
    std::fs::write(folder.path().join("cat.png"), b"abc").unwrap();
    std::fs::write(folder.path().join("dog.png"), b"abcd").unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let fake = Arc::new(FakeDevice::new(vec![tree(&["One"])]));
        let home = tempfile::tempdir().unwrap();
        let daemon = Daemon::new(
            Arc::new(Phone(fake.clone())),
            Config {
                home: home.path().to_path_buf(),
            },
        );
        let path = test_socket(home.path(), "daemon.sock");
        let listener = socket::listen(&path).await.unwrap();
        tokio::spawn(daemon.serve_on(listener));
        let (client_io, server_io) = tokio::io::duplex(1 << 20);
        let server = Mcp::new(Some(path), "mcp-files".into())
            .pinned(Some(PHONE.into()))
            .with_files(folder.path().to_path_buf());
        tokio::spawn(async move {
            let running = server.serve(server_io).await.unwrap();
            let _ = running.waiting().await;
        });
        let allows = Arc::new(Mutex::new(VecDeque::from([
            false, true, false, false, true,
        ])));
        let asked = Arc::new(Mutex::new(Vec::new()));
        let client = Human {
            allows,
            asked: asked.clone(),
        }
        .serve(client_io)
        .await
        .unwrap();
        let call = |args: serde_json::Value| {
            client.call_tool(
                CallToolRequestParams::new("files")
                    .with_arguments(args.as_object().unwrap().clone()),
            )
        };
        let pushes = || {
            fake.calls()
                .into_iter()
                .filter(|c| matches!(c, Call::FilePush(..)))
                .count()
        };

        let r = call(json!({"push": "cat.png"})).await.unwrap();
        assert!(text(&r).starts_with("The human declined"), "{r:?}");
        assert_eq!(pushes(), 0, "a decline copies nothing");
        let prompt = asked.lock().unwrap()[0].clone();
        assert!(
            prompt.contains("copy cat.png to the phone's Pictures folder")
                && prompt.contains("3 bytes, SHA-256 ba7816bf")
                && prompt.contains("Category: files"),
            "{prompt}"
        );
        assert!(!prompt.contains("Tap \""), "{prompt}");

        let r = call(json!({"push": "cat.png"})).await.unwrap();
        assert_ne!(r.is_error, Some(true), "{r:?}");
        assert!(
            text(&r).starts_with(
                "ok copied cat.png to /sdcard/Pictures/cat.png, where the gallery shows it · 3 bytes · sha256 ba7816bf"
            ),
            "{}",
            text(&r)
        );
        assert_eq!(pushes(), 1);

        let r = call(json!({"push": "dog.png"})).await.unwrap();
        assert!(
            text(&r).starts_with("The human declined"),
            "allowed for the session a file ago, and asked again: {r:?}"
        );
        assert_eq!(asked.lock().unwrap().len(), 3);
        assert_eq!(pushes(), 1);

        fake.with_file_aged("/sdcard/Download/report.pdf", b"BYTESOFTHEREPORT", 180);
        let lists = || {
            fake.calls()
                .into_iter()
                .filter(|c| matches!(c, Call::FileList(..)))
                .count()
        };
        let r = call(json!({})).await.unwrap();
        assert!(text(&r).starts_with("The human declined"), "{r:?}");
        assert_eq!(lists(), 0, "a decline names no file");
        let prompt = asked.lock().unwrap()[3].clone();
        assert!(
            prompt.contains(
                "list the names of the files in the phone's Download, Pictures, DCIM folders"
            ) && prompt.contains("Category: files"),
            "{prompt}"
        );
        let r = call(json!({})).await.unwrap();
        assert_eq!(
            text(&r),
            "Pictures/cat.png · 3 bytes · now\nDownload/report.pdf · 16 bytes · 3m ago"
        );
        let r = call(json!({"list": "Download"})).await.unwrap();
        assert_eq!(text(&r), "Download/report.pdf · 16 bytes · 3m ago");
        assert_eq!(
            asked.lock().unwrap().len(),
            5,
            "allowed for the session, a listing is not asked again"
        );
        let r = call(json!({"push": "dog.png"})).await.unwrap();
        assert!(
            text(&r).starts_with("cannot ask") || text(&r).contains("declined"),
            "a listing's word moves no file: {r:?}"
        );
        assert_eq!(pushes(), 1);

        let r = call(json!({"push": "cat.png", "pull": "Download/a"}))
            .await
            .unwrap();
        assert_eq!(r.is_error, Some(true), "{r:?}");
        let r = call(json!({"push": "cat.png", "to": "music"}))
            .await
            .unwrap();
        assert_eq!(text(&r), "`to` is `photos` or `downloads`");
    });
}

fn test_socket(home: &std::path::Path, name: &str) -> std::path::PathBuf {
    if cfg!(windows) {
        let scope = home.file_name().unwrap().to_string_lossy();
        std::path::PathBuf::from(format!(
            r"\\.\pipe\phonedriver-test-{}-{scope}-{name}",
            std::process::id()
        ))
    } else {
        home.join("run").join(name)
    }
}
