use std::sync::Arc;

use async_trait::async_trait;
use phone_use::daemon::{Config, Daemon, DeviceSource, Listed};
use phone_use_core::device::{Capabilities, Device};
use phone_use_core::error::Error;
use serde_json::json;

struct Devices {
    ids: Vec<&'static str>,
    blocked: Vec<&'static str>,
}

#[async_trait]
impl DeviceSource for Devices {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(self
            .ids
            .iter()
            .map(|id| Listed {
                id: (*id).into(),
                model: format!("Model {id}"),
                os: None,
                tier: "fake".into(),
                capabilities: Capabilities::all(),
            })
            .collect())
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        Err(Error::DeviceGone { id: id.into() })
    }

    fn blocker(&self, id: &str) -> Option<&'static str> {
        self.blocked.contains(&id).then_some("blocked")
    }
}

fn daemon(ids: &[&'static str], blocked: &[&'static str]) -> (Arc<Daemon>, tempfile::TempDir) {
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::new(
        Arc::new(Devices {
            ids: ids.to_vec(),
            blocked: blocked.to_vec(),
        }),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    (d, home)
}

#[tokio::test]
async fn a_wrong_device_id_is_named_and_the_connected_ones_are_listed() {
    let (d, _home) = daemon(&["sim:A", "android:B"], &[]);
    let e = d
        .call("s", "snapshot", json!({"device": "sim:BAD"}))
        .await
        .unwrap_err();
    assert_eq!(e.code, -32001);
    assert_eq!(
        e.message,
        "no device \"sim:BAD\"; connected: sim:A (Model sim:A), android:B (Model android:B)"
    );
    assert!(
        e.data["hint"]
            .as_str()
            .unwrap()
            .contains("sim:A, android:B")
    );
}

#[tokio::test]
async fn with_no_device_connected_a_named_one_says_none_is_connected() {
    let (d, _home) = daemon(&[], &[]);
    let e = d
        .call("s", "snapshot", json!({"device": "sim:BAD"}))
        .await
        .unwrap_err();
    assert_eq!(e.message, "no device \"sim:BAD\"; none is connected");
    let e = d.call("s", "snapshot", json!({})).await.unwrap_err();
    assert_eq!(e.message, "no device is connected");
}

#[tokio::test]
async fn the_default_is_the_one_device_that_can_be_driven() {
    let (d, _home) = daemon(&["ios:phone", "sim:A"], &["ios:phone"]);
    let e = d.call("s", "snapshot", json!({})).await.unwrap_err();
    assert_eq!(e.code, -32003, "{e:?}");
    assert_eq!(e.message, "device sim:A disconnected");
}

#[tokio::test]
async fn two_drivable_devices_or_none_stay_ambiguous() {
    for (ids, blocked) in [
        (vec!["sim:A", "sim:B"], vec![]),
        (vec!["ios:x", "ios:y"], vec!["ios:x", "ios:y"]),
    ] {
        let (d, _home) = daemon(&ids, &blocked);
        let e = d.call("s", "snapshot", json!({})).await.unwrap_err();
        assert_eq!(e.code, -32002, "{ids:?}: {e:?}");
    }
}

#[tokio::test]
async fn a_bad_scroll_direction_says_the_valid_ones_not_that_the_daemon_is_old() {
    let (d, _home) = daemon(&["sim:A"], &[]);
    let e = d
        .call(
            "s",
            "act",
            json!({"action": {"kind": "scroll", "direction": "sideways"}}),
        )
        .await
        .unwrap_err();
    for needle in ["scroll", "up", "down", "left", "right"] {
        assert!(e.message.contains(needle), "{needle} not in {}", e.message);
    }
    assert!(!e.message.contains("older"), "{}", e.message);
    assert!(!e.message.contains("sideways"), "{}", e.message);
}

#[tokio::test]
async fn an_unknown_action_still_says_to_stop_the_old_daemon_and_how() {
    let (d, _home) = daemon(&["sim:A"], &[]);
    let e = d
        .call("s", "act", json!({"action": {"kind": "teleport"}}))
        .await
        .unwrap_err();
    assert!(e.message.contains("older"), "{}", e.message);
    assert!(e.message.contains("phone-use daemon stop"), "{}", e.message);
}
