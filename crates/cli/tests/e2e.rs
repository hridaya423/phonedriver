//! End-to-end tests: the real CLI and daemon against the Android testbed, checked against the testbed's own event log.
//! Run with `ADB_SERVER_SOCKET=... PHONE_USE_TEST_DEVICE=android:emulator-5554 blaze test -p phone-use --test e2e -- --ignored`.

#![allow(dead_code)]

#[path = "../../android/tests/support/testbed.rs"]
mod testbed;

use std::path::PathBuf;
use std::process::Output;

use assert_cmd::Command;
use phone_use_android::adb::Adb;
use serde_json::Value;

static DEVICE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Rig {
    adb: Adb,
    serial: String,
    home: PathBuf,
    _lock: tokio::sync::MutexGuard<'static, ()>,
}

fn home() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("phone-use-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

impl Rig {
    async fn new() -> Rig {
        let lock = DEVICE_LOCK.lock().await;
        let target = std::env::var("PHONE_USE_TEST_DEVICE")
            .expect("set PHONE_USE_TEST_DEVICE, for example android:emulator-5554");
        let serial = target
            .strip_prefix("android:")
            .unwrap_or(&target)
            .to_string();
        let adb = Adb::from_env();
        testbed::ensure_installed(&adb, &serial).await;
        let rig = Rig {
            adb,
            serial,
            home: home(),
            _lock: lock,
        };
        rig.cli(&["release"]);
        rig
    }

    fn command(&self) -> Command {
        let mut c = Command::cargo_bin("phone-use").unwrap();
        c.env("PHONE_USE_HOME", &self.home)
            .env("PHONE_USE_SOCKET", self.home.join("d.sock"))
            .env_remove("PHONE_USE_SESSION")
            .env_remove("PHONE_USE_SECRET")
            .arg("--device")
            .arg(&self.serial);
        c
    }

    fn cli(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }

    fn ok(&self, args: &[&str]) -> String {
        let out = self.cli(args);
        assert!(
            out.status.success(),
            "phone-use {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    fn json(&self, args: &[&str]) -> Value {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        serde_json::from_str(&self.ok(&all)).unwrap()
    }

    async fn tab(&self, name: &str) {
        testbed::open_tab(&self.adb, &self.serial, name).await;
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        testbed::clear_log(&self.adb, &self.serial).await;
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.cli(&["release"]);
    }
}

#[tokio::test]
#[ignore = "needs PHONE_USE_TEST_DEVICE"]
async fn snapshot_json_save_fixture_writes_the_tree_and_a_screenshot() {
    let rig = Rig::new().await;
    rig.tab("taps").await;
    let dir = std::env::temp_dir().join(format!("phone-use-fixture-{}", std::process::id()));
    let result = rig.json(&[
        "snapshot",
        "--save-fixture",
        "settings_main",
        "--fixture-dir",
        dir.to_str().unwrap(),
    ]);
    assert!(result["text"].as_str().unwrap().contains("@e"));
    let tree: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("settings_main.json")).unwrap())
            .unwrap();
    assert!(tree["nodes"].as_array().unwrap().len() > 10);
    assert!(tree["density"].as_f64().unwrap() > 1.0);
    let png = std::fs::read(dir.join("settings_main.png")).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
}
