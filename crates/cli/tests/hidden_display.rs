#[path = "support/fakes.rs"]
mod fakes;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use fakes::tree;
use phonedriver::daemon::{Config, Daemon, DeviceSource, Listed};
use phonedriver_core::device::{Device, DisplayMode};
use phonedriver_core::error::{Error, Op};
use phonedriver_core::testing::FakeDevice;
use serde_json::{Value, json};

struct Source {
    main: Arc<FakeDevice>,
    hidden: Option<Arc<FakeDevice>>,
    connected: Mutex<Vec<DisplayMode>>,
}

#[async_trait]
impl DeviceSource for Source {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(vec![Listed {
            id: "fake:1".into(),
            model: "Fake".into(),
            os: None,
            tier: "fake".into(),
            capabilities: self.main.info().capabilities,
        }])
    }

    async fn connect(&self, _id: &str) -> Result<Arc<dyn Device>, Error> {
        self.connect_on("fake:1", DisplayMode::Main).await
    }

    async fn connect_on(&self, _id: &str, display: DisplayMode) -> Result<Arc<dyn Device>, Error> {
        self.connected.lock().unwrap().push(display);
        match (display, &self.hidden) {
            (DisplayMode::Main, _) => Ok(self.main.clone()),
            (DisplayMode::Hidden, Some(h)) => Ok(h.clone()),
            (DisplayMode::Hidden, None) => Err(Error::Unsupported {
                op: Op::Display,
                reason: "a hidden display needs Android 13 (API 33) or newer",
            }),
        }
    }
}

fn setup(hidden: bool) -> (Arc<Daemon>, Arc<Source>, tempfile::TempDir) {
    let source = Arc::new(Source {
        main: Arc::new(FakeDevice::new(vec![tree(&["Main screen"])])),
        hidden: hidden.then(|| {
            Arc::new(FakeDevice::new(vec![tree(&["Hidden screen"])]).on_hidden_display(7))
        }),
        connected: Mutex::new(Vec::new()),
    });
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::new(
        source.clone(),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    (d, source, home)
}

async fn snapshot(d: &Daemon, params: Value) -> Result<Value, phonedriver::daemon::rpc::RpcError> {
    d.call("default", "snapshot", params).await
}

#[tokio::test]
async fn a_call_for_the_hidden_display_connects_in_that_mode_and_a_later_plain_call_connects_back()
{
    let (d, source, _home) = setup(true);

    let hidden = snapshot(&d, json!({"display": "hidden"})).await.unwrap();
    assert!(hidden.to_string().contains("Hidden screen"), "{hidden}");

    snapshot(&d, json!({"display": "hidden"})).await.unwrap();
    assert_eq!(
        *source.connected.lock().unwrap(),
        vec![DisplayMode::Hidden],
        "a second hidden call reuses the connection"
    );

    let main = snapshot(&d, json!({})).await.unwrap();
    assert!(main.to_string().contains("Main screen"), "{main}");
    assert_eq!(
        *source.connected.lock().unwrap(),
        vec![DisplayMode::Hidden, DisplayMode::Main]
    );
}

#[tokio::test]
async fn devices_say_which_display_a_live_device_works_on() {
    let (d, _source, _home) = setup(true);
    let before = d.call("default", "devices", json!({})).await.unwrap();
    assert!(before[0]["hidden_display"].is_null(), "{before}");

    snapshot(&d, json!({"display": "hidden"})).await.unwrap();
    let live = d.call("default", "devices", json!({})).await.unwrap();
    assert_eq!(live[0]["hidden_display"], 7, "{live}");

    snapshot(&d, json!({})).await.unwrap();
    let back = d.call("default", "devices", json!({})).await.unwrap();
    assert!(back[0]["hidden_display"].is_null(), "{back}");
}

#[tokio::test]
async fn health_reports_the_hidden_display_id() {
    let (d, _source, _home) = setup(true);
    let r = d
        .call("default", "health", json!({"display": "hidden"}))
        .await
        .unwrap();
    assert_eq!(r["hidden_display"], 7, "{r}");
}

#[tokio::test]
async fn a_device_that_cannot_host_a_hidden_display_says_so() {
    let (d, _source, _home) = setup(false);
    let e = snapshot(&d, json!({"display": "hidden"}))
        .await
        .expect_err("hidden is unsupported here");
    assert!(e.message.contains("Android 13"), "{}", e.message);
    assert!(
        e.data["hint"]
            .as_str()
            .unwrap_or_default()
            .contains("--display hidden"),
        "{}",
        e.data
    );
}

#[tokio::test]
async fn the_default_connect_on_refuses_a_hidden_display() {
    struct Plain(Arc<FakeDevice>);
    #[async_trait]
    impl DeviceSource for Plain {
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
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::new(
        Arc::new(Plain(Arc::new(FakeDevice::new(vec![tree(&["x"])])))),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    let e = snapshot(&d, json!({"display": "hidden"}))
        .await
        .expect_err("no hidden display on this kind of device");
    assert!(e.message.contains("display"), "{}", e.message);
}
