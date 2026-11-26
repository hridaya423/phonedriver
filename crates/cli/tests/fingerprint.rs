use crate::fakes;

use std::sync::Arc;

use async_trait::async_trait;
use fakes::tree;
use phonedriver::daemon::{Config, Daemon, DeviceSource, Listed};
use phonedriver_core::device::{Button, Capabilities, Device, ScreenState};
use phonedriver_core::error::Error;
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

fn setup() -> (Arc<Daemon>, Arc<FakeDevice>, tempfile::TempDir) {
    let fake = Arc::new(FakeDevice::new(vec![tree(&["One"])]).lacking(Capabilities::FRAMES));
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::new(
        Arc::new(One(fake.clone())),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    (d, fake, home)
}

fn press(key: &str) -> Value {
    json!({"action": {"kind": "press", "key": key}})
}

#[tokio::test(start_paused = true)]
async fn fingerprint_and_wrong_fingerprint_are_keys_that_touch_the_sensor() {
    let (d, fake, _home) = setup();
    let r = d
        .call("default", "act", press("fingerprint"))
        .await
        .unwrap();
    assert_eq!(r["summary"], "pressed fingerprint");
    d.call("default", "act", press("wrong-fingerprint"))
        .await
        .unwrap();
    let touches: Vec<Call> = fake
        .calls()
        .into_iter()
        .filter(|c| matches!(c, Call::Button(_)))
        .collect();
    assert_eq!(
        touches,
        [
            Call::Button(Button::Fingerprint),
            Call::Button(Button::WrongFingerprint)
        ]
    );
    let e = d.call("default", "act", press("thumb")).await.unwrap_err();
    assert!(
        e.data["valid"]
            .as_array()
            .unwrap()
            .contains(&json!("fingerprint")),
        "{e:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_fingerprint_is_pressed_on_a_locked_screen_that_other_actions_are_refused_on() {
    let (d, fake, _home) = setup();
    fake.set_screen(ScreenState {
        on: true,
        locked: true,
        secure: true,
        focused: true,
    });
    let e = d.call("default", "act", press("home")).await.unwrap_err();
    assert_eq!(e.data["code_name"], "needs_user", "{e:?}");
    d.call("default", "act", press("fingerprint"))
        .await
        .unwrap();
    assert!(fake.calls().contains(&Call::Button(Button::Fingerprint)));
}
