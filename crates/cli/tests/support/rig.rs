//! Shared rig for the live tests: the real CLI and daemon against the Android testbed.

#![allow(dead_code)]

#[path = "../../../android/tests/support/testbed.rs"]
pub mod testbed;

use std::path::PathBuf;
use std::process::Output;

use assert_cmd::Command;
use phone_use_android::adb::Adb;
use serde_json::Value;

static DEVICE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub struct Rig {
    pub adb: Adb,
    pub serial: String,
    pub home: PathBuf,
    _lock: tokio::sync::MutexGuard<'static, ()>,
}

pub fn home() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("phone-use-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

impl Rig {
    pub async fn new() -> Rig {
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

    pub fn command(&self) -> Command {
        let mut c = Command::cargo_bin("phone-use").unwrap();
        c.env("PHONE_USE_HOME", &self.home)
            .env_remove("PHONE_USE_SESSION")
            .env_remove("PHONE_USE_SECRET")
            .arg("--device")
            .arg(&self.serial);
        c
    }

    pub fn cli(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }

    pub fn ok(&self, args: &[&str]) -> String {
        let out = self.cli(args);
        assert!(
            out.status.success(),
            "phone-use {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    pub fn json(&self, args: &[&str]) -> Value {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        serde_json::from_str(&self.ok(&all)).unwrap()
    }

    pub async fn tab(&self, name: &str) {
        testbed::open_tab(&self.adb, &self.serial, name).await;
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        testbed::clear_log(&self.adb, &self.serial).await;
        self.ok(&["snapshot", "--no-settle"]);
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.cli(&["release"]);
    }
}

pub const UNICODE: &str = "héllo 世界 مرحبا 👋🏽";

pub fn elements(rig: &Rig) -> Vec<Value> {
    let r = rig.json(&["snapshot", "--no-settle"]);
    r["elements"].as_array().unwrap().clone()
}

pub fn find<'a>(elements: &'a [Value], role: &str, label: &str) -> &'a Value {
    elements
        .iter()
        .find(|e| e["role"] == role && (e["label"] == label || e["hint"] == label))
        .unwrap_or_else(|| panic!("no {role} {label:?} in {elements:#?}"))
}

pub fn center(e: &Value) -> (f64, f64) {
    let b = &e["bounds"];
    (
        b["x"].as_f64().unwrap() + b["w"].as_f64().unwrap() / 2.0,
        b["y"].as_f64().unwrap() + b["h"].as_f64().unwrap() / 2.0,
    )
}

pub async fn events(rig: &Rig, name: &'static str, count: usize) -> Vec<Value> {
    let log = testbed::wait_for_log(&rig.adb, &rig.serial, 4000, |l| {
        l.iter().filter(|e| e["ev"] == name).count() >= count
    })
    .await;
    log.into_iter().filter(|e| e["ev"] == name).collect()
}

pub fn near(got: &Value, want: f64) -> bool {
    (got.as_f64().unwrap() - want).abs() <= 1.0
}

pub fn files_under(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = entry.path();
        if p.is_dir() {
            out.extend(files_under(&p));
        } else {
            out.push(p);
        }
    }
    out
}

pub fn nothing_under_the_home_mentions(rig: &Rig, needle: &str) {
    for f in files_under(&rig.home) {
        if f.extension().is_some_and(|e| e == "png" || e == "sock") {
            continue;
        }
        let text = std::fs::read(&f).unwrap_or_default();
        assert!(
            !String::from_utf8_lossy(&text).contains(needle),
            "{} contains the secret",
            f.display()
        );
    }
}
