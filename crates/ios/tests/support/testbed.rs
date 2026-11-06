//! The iOS testbed on a simulator: build and install, launch on a tab, and read its event log (task 5.1).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::Value;

pub const BUNDLE_ID: &str = "dev.phoneuse.testbed";

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testbeds/ios")
}

fn run(cmd: &mut Command) -> String {
    let out = cmd.output().expect("cannot run command");
    assert!(
        out.status.success(),
        "{cmd:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn xcodegen() -> PathBuf {
    phone_use_ios::runner::find_tool("xcodegen").expect("xcodegen is not installed")
}

/// Builds the testbed with the current sources and installs it.
pub fn install(udid: &str) {
    let dir = dir();
    run(Command::new(xcodegen())
        .args(["generate", "--quiet"])
        .current_dir(&dir));
    run(Command::new("xcodebuild")
        .args(["-project", "Testbed.xcodeproj", "-scheme", "Testbed"])
        .args(["-destination", &format!("id={udid}")])
        .args(["-derivedDataPath", "build", "-quiet", "build"])
        .current_dir(&dir));
    let app = dir.join("build/Build/Products/Debug-iphonesimulator/Testbed.app");
    run(Command::new("xcrun")
        .args(["simctl", "install", udid])
        .arg(app));
}

/// Starts the testbed fresh on `tab` and empties its log.
pub fn launch(udid: &str, tab: &str) {
    let _ = Command::new("xcrun")
        .args(["simctl", "terminate", udid, BUNDLE_ID])
        .output();
    run(Command::new("xcrun").args(["simctl", "launch", udid, BUNDLE_ID, "-tab", tab]));
    std::thread::sleep(Duration::from_millis(1500));
    clear_log(udid);
}

fn log_path(udid: &str) -> PathBuf {
    let data =
        run(Command::new("xcrun").args(["simctl", "get_app_container", udid, BUNDLE_ID, "data"]));
    PathBuf::from(data.trim()).join("Documents/putb.log")
}

pub fn clear_log(udid: &str) {
    let path = log_path(udid);
    if path.exists() {
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_len(0)
            .unwrap();
    }
}

pub fn read_log(udid: &str) -> Vec<Value> {
    std::fs::read_to_string(log_path(udid))
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// Waits until an event matches, and returns the whole log.
pub fn wait_for(udid: &str, what: &str, pred: impl Fn(&Value) -> bool) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let log = read_log(udid);
        if log.iter().any(&pred) {
            return log;
        }
        assert!(
            Instant::now() < deadline,
            "no {what} event within 5 s; log tail: {:?}",
            &log[log.len().saturating_sub(10)..]
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
