use crate::fakes;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use fakes::tree;
use phonedriver::daemon::{Config, Daemon, DeviceSource, Listed};
use phonedriver_core::device::{Device, DisplayMode};
use phonedriver_core::error::Error;
use phonedriver_core::testing::FakeDevice;
use serde_json::{Value, json};

struct Source {
    screens: Vec<(&'static str, Arc<FakeDevice>)>,
    connected: Mutex<Vec<(String, DisplayMode)>>,
}

#[async_trait]
impl DeviceSource for Source {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(vec![Listed {
            id: "fake:1".into(),
            model: "Fake".into(),
            os: None,
            tier: "fake".into(),
            capabilities: self.screens[0].1.info().capabilities,
        }])
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        self.connect_on(id, DisplayMode::Main).await
    }

    async fn connect_on(&self, id: &str, display: DisplayMode) -> Result<Arc<dyn Device>, Error> {
        self.connected
            .lock()
            .unwrap()
            .push((id.to_string(), display));
        let (_, device) = self
            .screens
            .iter()
            .find(|(known, _)| *known == id)
            .unwrap_or_else(|| panic!("the daemon asked for {id}"));
        Ok(device.clone())
    }
}

fn setup() -> (Arc<Daemon>, Arc<Source>, tempfile::TempDir) {
    let source = Arc::new(Source {
        screens: vec![
            (
                "fake:1",
                Arc::new(FakeDevice::new(vec![tree(&["Main screen"])])),
            ),
            (
                "fake:1+a",
                Arc::new(FakeDevice::new(vec![tree(&["Lane a screen"])]).on_hidden_display(7)),
            ),
            (
                "fake:1+b",
                Arc::new(FakeDevice::new(vec![tree(&["Lane b screen"])]).on_hidden_display(8)),
            ),
        ],
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

async fn snapshot(d: &Daemon, session: &str, device: Option<&str>) -> Value {
    d.call(session, "snapshot", json!({"device": device}))
        .await
        .unwrap_or_else(|e| panic!("snapshot on {device:?} failed: {e:?}"))
}

#[tokio::test]
async fn a_device_named_with_a_lane_is_a_hidden_display_of_its_own_beside_the_main_screen() {
    let (d, source, _home) = setup();

    let lane = snapshot(&d, "one", Some("fake:1+a")).await;
    assert!(lane.to_string().contains("Lane a screen"), "{lane}");
    let main = snapshot(&d, "one", None).await;
    assert!(main.to_string().contains("Main screen"), "{main}");
    let again = snapshot(&d, "one", Some("fake:1+a")).await;
    assert!(again.to_string().contains("Lane a screen"), "{again}");

    assert_eq!(
        *source.connected.lock().unwrap(),
        vec![
            ("fake:1+a".to_string(), DisplayMode::Hidden),
            ("fake:1".to_string(), DisplayMode::Main),
        ],
        "the lane kept its connection while the main screen was used"
    );
}

#[tokio::test]
async fn two_sessions_work_on_two_lanes_at_once_and_a_lane_is_held_by_one() {
    let (d, _source, _home) = setup();

    let a = snapshot(&d, "one", Some("fake:1+a")).await;
    let b = snapshot(&d, "two", Some("fake:1+b")).await;
    assert!(a.to_string().contains("Lane a screen"), "{a}");
    assert!(b.to_string().contains("Lane b screen"), "{b}");

    let busy = d
        .call("two", "snapshot", json!({"device": "fake:1+a"}))
        .await
        .expect_err("lane a is held by session one");
    assert!(busy.message.contains("one"), "{busy:?}");
}

#[tokio::test]
async fn a_bare_lane_is_a_lane_of_the_only_device() {
    let (d, source, _home) = setup();
    let lane = snapshot(&d, "one", Some("+b")).await;
    assert!(lane.to_string().contains("Lane b screen"), "{lane}");
    assert_eq!(
        source.connected.lock().unwrap()[0],
        ("fake:1+b".to_string(), DisplayMode::Hidden)
    );
}

#[tokio::test]
async fn devices_lists_a_lane_in_use_under_its_device() {
    let (d, _source, _home) = setup();
    snapshot(&d, "one", Some("fake:1+a")).await;

    let listed = d.call("one", "devices", json!({})).await.unwrap();
    let rows = listed.as_array().unwrap();
    assert_eq!(rows.len(), 2, "{listed}");
    assert_eq!(rows[0]["id"], "fake:1");
    assert!(rows[0]["leased_by"].is_null(), "{listed}");
    assert_eq!(rows[1]["id"], "fake:1+a");
    assert_eq!(rows[1]["leased_by"], "one");
    assert_eq!(rows[1]["hidden_display"], 7);
}

#[tokio::test]
async fn a_suffix_that_is_not_a_lane_name_is_part_of_a_device_nobody_has() {
    let (d, source, _home) = setup();
    let missing = d
        .call("one", "snapshot", json!({"device": "fake:1+Not a lane"}))
        .await
        .expect_err("no such device");
    assert!(missing.message.contains("fake:1+Not a lane"), "{missing:?}");
    assert!(source.connected.lock().unwrap().is_empty());
}

fn new_lane() -> (Arc<Daemon>, Arc<FakeDevice>, tempfile::TempDir) {
    let lane = Arc::new(
        FakeDevice::new(vec![tree(&["Home"]), tree(&["Calculator", "7"])]).on_hidden_display(7),
    );
    lane.install(&[("com.android.calculator2", "Calculator")]);
    let source = Arc::new(Source {
        screens: vec![
            ("fake:1", Arc::new(FakeDevice::new(vec![tree(&["Main"])]))),
            ("fake:1+a", lane.clone()),
        ],
        connected: Mutex::new(Vec::new()),
    });
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::new(
        source,
        Config {
            home: home.path().to_path_buf(),
        },
    );
    (d, lane, home)
}

async fn launch_calculator(d: &Daemon) -> Value {
    d.call(
        "one",
        "act",
        json!({"device": "fake:1+a", "action": {"kind": "launch", "app": "Calculator"}}),
    )
    .await
    .unwrap()
}

#[tokio::test(start_paused = true)]
async fn a_launch_on_a_lane_with_nothing_in_front_yet_waits_for_the_app() {
    let (d, lane, _home) = new_lane();
    lane.set_launch_takes(12);
    let started = tokio::time::Instant::now();
    let r = launch_calculator(&d).await;
    assert_eq!(r["summary"], "launched Calculator", "{r}");
    assert!(
        started.elapsed() > std::time::Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test(start_paused = true)]
async fn a_launch_on_a_lane_that_stays_empty_says_nothing_is_in_front() {
    let (d, lane, _home) = new_lane();
    lane.set_quits_at_launch(true);
    let r = launch_calculator(&d).await;
    let summary = r["summary"].as_str().unwrap();
    assert!(
        summary.contains("Calculator is not in front") && summary.contains("nothing is"),
        "{summary}"
    );
}
