use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use phone_use::daemon::progress::after_a_second;
use phone_use::daemon::{Config, Daemon, DeviceSource, Listed, socket};
use phone_use_core::device::{Capabilities, Device};
use phone_use_core::error::Error;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Slow(Duration);

#[async_trait]
impl DeviceSource for Slow {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(vec![Listed {
            id: "fake:1".into(),
            model: "Fake".into(),
            os: None,
            tier: "fake".into(),
            capabilities: Capabilities::all(),
        }])
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        after_a_second(
            || "starting the fake runner".into(),
            tokio::time::sleep(self.0),
        )
        .await;
        Err(Error::DeviceGone { id: id.into() })
    }
}

async fn lines_for(slow: Duration, progress: bool) -> Vec<Value> {
    let _alone = ONE_AT_A_TIME.lock().await;
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("run").join("d.sock");
    let daemon = Daemon::new(
        Arc::new(Slow(slow)),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    let listener = socket::listen(&path).await.unwrap();
    tokio::spawn(daemon.serve_on(listener));
    let stream = socket::connect(&path).await.unwrap();
    let (read, mut write) = tokio::io::split(stream);
    let mut lines = BufReader::new(read).lines();
    let send = |id: u64, method: &str, params: Value| {
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        format!("{line}\n")
    };
    write
        .write_all(
            send(
                1,
                "hello",
                json!({"version": env!("CARGO_PKG_VERSION"), "client": "cli", "progress": progress}),
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    lines.next_line().await.unwrap().unwrap();
    write
        .write_all(send(2, "snapshot", json!({"session": "s"})).as_bytes())
        .await
        .unwrap();
    let mut out = Vec::new();
    while let Some(line) = lines.next_line().await.unwrap() {
        let v: Value = serde_json::from_str(&line).unwrap();
        let reply = v["id"] == json!(2);
        out.push(v);
        if reply {
            break;
        }
    }
    out
}

#[tokio::test]
async fn a_slow_step_sends_a_progress_line_before_the_reply_to_a_client_that_asked() {
    let lines = lines_for(Duration::from_millis(1300), true).await;
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[0]["method"], json!("progress"));
    assert_eq!(
        lines[0]["params"]["message"],
        json!("starting the fake runner")
    );
    assert!(lines[0].get("id").is_none());
    assert_eq!(lines[1]["id"], json!(2));
}

#[tokio::test]
async fn a_client_that_did_not_ask_gets_only_the_reply() {
    let lines = lines_for(Duration::from_millis(1300), false).await;
    assert_eq!(lines.len(), 1, "{lines:?}");
}

#[tokio::test]
async fn a_fast_step_says_nothing() {
    let lines = lines_for(Duration::from_millis(10), true).await;
    assert_eq!(lines.len(), 1, "{lines:?}");
}
