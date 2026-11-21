use std::sync::Arc;

use async_trait::async_trait;
use phonedriver::daemon::rpc::RpcError;
use phonedriver::daemon::{Config, Daemon, DeviceSource, Listed};
use phonedriver_core::device::{AppId, Capabilities, Device};
use phonedriver_core::error::Error;
use phonedriver_core::geometry::Scale;
use phonedriver_core::network::{NetworkLink, NetworkView};
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

fn screen() -> RawTree {
    RawTree::from_android_json(
        r#"{"windows":[{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}],"nodes":[
        {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.widget.Button","text":"Connect","b":[0,100,360,160],"f":"vVe","a":["tap"]}]}"#,
        Scale { density: 1.0 },
    )
    .unwrap()
}

fn setup() -> (Arc<Daemon>, Arc<FakeDevice>, tempfile::TempDir) {
    let fake = Arc::new(FakeDevice::new(vec![screen()]).lacking(Capabilities::FRAMES));
    fake.install(&[("com.example", "Example")]);
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

fn wifi() -> NetworkLink {
    NetworkLink {
        id: 101,
        transports: vec!["wifi".into()],
        default: true,
        interface: Some("wlan0".into()),
        dns: vec!["10.0.2.3".into()],
        ..NetworkLink::default()
    }
}

fn vpn() -> NetworkLink {
    NetworkLink {
        id: 102,
        transports: vec!["wifi".into(), "vpn".into()],
        owner: Some("dev.example.vpn".into()),
        uids: vec!["0-99999".into()],
        underlying: vec![101],
        ..NetworkLink::default()
    }
}

fn down() -> NetworkView {
    NetworkView {
        networks: vec![wifi()],
        ..NetworkView::default()
    }
}

fn up() -> NetworkView {
    NetworkView {
        networks: vec![wifi(), vpn()],
        ..NetworkView::default()
    }
}

#[tokio::test(start_paused = true)]
async fn network_returns_the_networks_the_device_reports() {
    let (d, fake, _home) = setup();
    fake.answer_networks(vec![NetworkView {
        always_on_vpn: Some("dev.example.vpn".into()),
        missing: vec!["private DNS".into()],
        ..up()
    }]);
    let r = ok(&d, "network", json!({})).await;
    assert_eq!(r["networks"][1]["owner"], "dev.example.vpn");
    assert_eq!(r["networks"][1]["uids"], json!(["0-99999"]));
    assert_eq!(r["networks"][0]["default"], true);
    assert_eq!(r["always_on_vpn"], "dev.example.vpn");
    assert_eq!(r["missing"], json!(["private DNS"]));
}

#[tokio::test(start_paused = true)]
async fn a_device_that_cannot_read_its_networks_says_unsupported() {
    let (d, _fake, _home) = setup();
    let e = err(&d, "network", json!({})).await;
    assert_eq!(e.code, -32011, "{e:?}");
}

#[tokio::test(start_paused = true)]
async fn wait_vpn_up_returns_once_the_vpn_network_appears() {
    let (d, fake, _home) = setup();
    fake.answer_networks(vec![down(), down(), up()]);
    let r = ok(
        &d,
        "wait",
        json!({"condition": {"vpn": "up"}, "timeout_ms": 10000}),
    )
    .await;
    assert_eq!(r["ok"], true);
    assert_eq!(r["summary"], "a VPN is up (dev.example.vpn)");
}

#[tokio::test(start_paused = true)]
async fn wait_vpn_down_returns_once_the_vpn_network_is_gone() {
    let (d, fake, _home) = setup();
    fake.answer_networks(vec![up(), down()]);
    let r = ok(
        &d,
        "wait",
        json!({"condition": {"vpn": "down"}, "timeout_ms": 10000}),
    )
    .await;
    assert_eq!(r["summary"], "no VPN is up");
}

#[tokio::test(start_paused = true)]
async fn wait_vpn_up_that_times_out_gives_the_last_state_seen() {
    let (d, fake, _home) = setup();
    fake.answer_networks(vec![down()]);
    let e = err(
        &d,
        "wait",
        json!({"condition": {"vpn": "up"}, "timeout_ms": 3000}),
    )
    .await;
    assert_eq!(e.code, -32013, "{e:?}");
    assert!(
        e.message
            .contains("the VPN to be up; last seen: wifi (default)"),
        "{e:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn wait_log_returns_the_line_that_matches_and_reads_the_apps_log_when_one_is_named() {
    let (d, fake, _home) = setup();
    fake.answer_log_lines(&[
        "I/Other( 1): hello",
        "I/Example( 2): tunnel handshake done",
        "I/Example( 2): later",
    ]);
    let r = ok(
        &d,
        "wait",
        json!({"condition": {"log": {"pattern": "Failed|HANDSHAKE", "app": "Example"}}}),
    )
    .await;
    assert_eq!(r["ok"], true);
    assert_eq!(
        r["summary"],
        "log line: I/Example( 2): tunnel handshake done"
    );
    assert_eq!(
        fake.calls()
            .into_iter()
            .filter(|c| matches!(c, Call::LogWait(..)))
            .collect::<Vec<_>>(),
        [Call::LogWait(
            Some(AppId("com.example".into())),
            vec!["Failed".into(), "HANDSHAKE".into()]
        )]
    );
}

#[tokio::test(start_paused = true)]
async fn wait_log_that_times_out_says_how_many_lines_arrived() {
    let (d, fake, _home) = setup();
    fake.answer_log_lines(&["I/A( 1): one", "I/A( 1): two", "I/A( 1): three"]);
    let e = err(
        &d,
        "wait",
        json!({"condition": {"log": {"pattern": "never"}}, "timeout_ms": 2000}),
    )
    .await;
    assert_eq!(e.code, -32013, "{e:?}");
    assert!(
        e.message
            .contains("a log line with \"never\" (3 lines arrived, none matched)"),
        "{e:?}"
    );
    fake.answer_log_lines(&[]);
    let e = err(
        &d,
        "wait",
        json!({"condition": {"log": {"pattern": "never"}}, "timeout_ms": 2000}),
    )
    .await;
    assert!(e.message.contains("(no line arrived)"), "{e:?}");
}

#[tokio::test(start_paused = true)]
async fn wait_log_on_a_device_that_cannot_follow_its_log_says_unsupported() {
    let (d, _fake, _home) = setup();
    let e = err(&d, "wait", json!({"condition": {"log": {"pattern": "x"}}})).await;
    assert_eq!(e.code, -32011, "{e:?}");
}

#[tokio::test(start_paused = true)]
async fn wait_log_of_an_app_that_is_not_installed_says_so() {
    let (d, fake, _home) = setup();
    fake.answer_log_lines(&["x"]);
    let e = err(
        &d,
        "wait",
        json!({"condition": {"log": {"pattern": "x", "app": "Nowhere"}}}),
    )
    .await;
    assert!(e.message.to_lowercase().contains("nowhere"), "{e:?}");
}

#[tokio::test(start_paused = true)]
async fn a_secret_in_a_matched_log_line_is_masked_in_the_reply() {
    let (d, fake, _home) = setup();
    ok(
        &d,
        "shell",
        json!({"command": "sk-live-4f9a2c", "secret": true, "settle": false}),
    )
    .await;
    fake.answer_log_lines(&["I/Auth( 3): issued token sk-live-4f9a2c for user"]);
    let r = ok(
        &d,
        "wait",
        json!({"condition": {"log": {"pattern": "issued token"}}}),
    )
    .await;
    let text = r.to_string();
    assert!(!text.contains("sk-live-4f9a2c"), "{text}");
    assert_eq!(
        r["summary"],
        "log line: I/Auth( 3): issued token ••• for user"
    );
}

fn traces(home: &tempfile::TempDir) -> String {
    let mut all = String::new();
    for entry in std::fs::read_dir(home.path().join("traces"))
        .into_iter()
        .flatten()
        .flatten()
    {
        all += &std::fs::read_to_string(entry.path()).unwrap_or_default();
    }
    all
}

#[tokio::test(start_paused = true)]
async fn a_secret_given_while_a_log_wait_is_pending_is_masked_in_that_waits_reply_and_trace() {
    let (d, fake, home) = setup();
    fake.answer_log_lines(&["I/App( 1): token s3cr3t-c8-value"]);
    fake.log_wait_takes(std::time::Duration::from_millis(1500));
    let waiting = {
        let d = d.clone();
        tokio::spawn(async move {
            d.call(
                "a",
                "wait",
                json!({"condition": {"log": {"pattern": "token"}}}),
            )
            .await
        })
    };
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let typed = {
        let d = d.clone();
        tokio::spawn(async move {
            d.call(
                "b",
                "shell",
                json!({"command": "s3cr3t-c8-value", "secret": true, "settle": false, "share": true}),
            )
            .await
        })
    };
    let r = waiting.await.unwrap().unwrap();
    let _ = typed.await.unwrap();
    assert_eq!(r["summary"], "log line: I/App( 1): token •••", "{r}");
    let trace = traces(&home);
    assert!(!trace.contains("s3cr3t-c8-value"), "{trace}");
}

#[tokio::test(start_paused = true)]
async fn a_log_line_is_masked_before_it_is_cut_so_no_part_of_a_secret_comes_back() {
    let (d, fake, _home) = setup();
    let secret = "ABCDEFGHIJ-secret-tail-0123";
    ok(
        &d,
        "shell",
        json!({"command": secret, "secret": true, "settle": false}),
    )
    .await;
    let line = format!("I/App( 1): token {}{secret}", "x".repeat(373));
    assert_eq!(line.chars().count(), 417);
    fake.answer_log_lines(&[&line]);
    let r = ok(
        &d,
        "wait",
        json!({"condition": {"log": {"pattern": "token"}}}),
    )
    .await;
    let summary = r["summary"].as_str().unwrap();
    assert!(summary.ends_with("xx•••"), "{summary}");
    assert!(!summary.contains("ABC"), "{summary}");
}

#[tokio::test(start_paused = true)]
async fn a_log_wait_in_a_batch_and_in_the_saved_steps_does_not_keep_the_line() {
    let (d, fake, _home) = setup();
    fake.answer_log_lines(&["I/Auth( 3): code 424242"]);
    let r = ok(
        &d,
        "batch",
        json!({"steps": [{"kind": "wait", "condition": {"log": {"pattern": "code"}}}]}),
    )
    .await;
    assert_eq!(r["ok"], true, "{r}");
    let journal = ok(&d, "journal", json!({})).await.to_string();
    assert!(!journal.contains("424242"), "{journal}");
    assert!(journal.contains("\"log\""), "{journal}");
}
