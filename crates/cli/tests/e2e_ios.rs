//! End-to-end tests: the real CLI and daemon against the iOS testbed on a booted simulator, checked against the
//! testbed's event log. On the Mac that hosts the simulator:
//! `PHONEDRIVER_TEST_DEVICE=sim:<udid> cargo test -p phonedriver --test e2e_ios -- --ignored --test-threads=1`.

#![cfg(unix)]
#![allow(dead_code)]

use phonedriver_ios::testbed;

use std::path::PathBuf;
use std::process::Output;
use std::sync::Once;
use std::time::{Duration, Instant};

use assert_cmd::Command;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

static INSTALL: Once = Once::new();

fn udid() -> String {
    let v =
        std::env::var("PHONEDRIVER_TEST_DEVICE").expect("set PHONEDRIVER_TEST_DEVICE=sim:<udid>");
    v.strip_prefix("sim:").unwrap_or(&v).to_string()
}

struct Rig {
    udid: String,
    home: PathBuf,
}

impl Rig {
    fn new() -> Rig {
        let udid = udid();
        INSTALL.call_once(|| testbed::install(&udid));
        let home = std::env::temp_dir().join(format!("phonedriver-e2e-ios-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        Rig { udid, home }
    }

    fn command(&self) -> Command {
        let mut c = Command::cargo_bin("phonedriver").unwrap();
        c.env("PHONEDRIVER_HOME", &self.home)
            .env_remove("PHONEDRIVER_SESSION")
            .env_remove("PHONEDRIVER_SECRET")
            .arg("--device")
            .arg(format!("sim:{}", self.udid));
        c
    }

    fn cli(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }

    fn ok(&self, args: &[&str]) -> String {
        let out = self.cli(args);
        assert!(
            out.status.success(),
            "phonedriver {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    fn json(&self, args: &[&str]) -> Value {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        serde_json::from_str(&self.ok(&all)).unwrap()
    }

    fn tab(&self, name: &str) {
        testbed::launch(&self.udid, name);
    }

    fn elements(&self) -> Vec<Value> {
        self.json(&["snapshot"])["elements"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }

    async fn daemon_pid(&self) -> u32 {
        let stream = phonedriver::daemon::socket::connect(&self.home.join("run/daemon.sock"))
            .await
            .unwrap();
        let (read, mut write) = tokio::io::split(stream);
        let req = json!({"jsonrpc": "2.0", "id": 1, "method": "hello",
                         "params": {"version": env!("CARGO_PKG_VERSION")}});
        write
            .write_all(format!("{req}\n").as_bytes())
            .await
            .unwrap();
        let mut line = String::new();
        BufReader::new(read).read_line(&mut line).await.unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        v["result"]["pid"].as_u64().unwrap() as u32
    }

    fn runners(&self) -> usize {
        let out = std::process::Command::new("pgrep")
            .args(["-f", &format!("test-without-building.*id={}", self.udid)])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).lines().count()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.cli(&["release"]);
    }
}

fn find<'a>(elements: &'a [Value], role: &str, label: &str) -> &'a Value {
    elements
        .iter()
        .find(|e| e["role"] == role && e["label"] == label)
        .unwrap_or_else(|| panic!("no {role} {label:?} in {elements:#?}"))
}

fn ref_of(e: &Value) -> String {
    format!("@{}", e["ref"].as_str().unwrap())
}

fn center(e: &Value) -> (f64, f64) {
    let b = &e["bounds"];
    (
        b["x"].as_f64().unwrap() + b["w"].as_f64().unwrap() / 2.0,
        b["y"].as_f64().unwrap() + b["h"].as_f64().unwrap() / 2.0,
    )
}

#[test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE=sim:<udid>"]
fn devices_lists_the_simulator_tier() {
    let rig = Rig::new();
    let list = rig.json(&["devices"]);
    let me = list
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == format!("sim:{}", rig.udid))
        .expect("simulator not listed");
    assert_eq!(me["tier"], "simulator");
    assert!(me["os"].as_str().unwrap().starts_with("iOS "));
}

#[test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE=sim:<udid>"]
fn snapshot_then_tap_a_ref_lands_on_its_center_and_returns_a_diff() {
    let rig = Rig::new();
    rig.tab("taps");
    let text = rig.ok(&["snapshot"]);
    assert!(
        text.starts_with(&format!("device sim:{}", rig.udid)),
        "{text}"
    );
    assert!(text.contains("app dev.phonedriver.testbed"), "{text}");
    let els = rig.elements();
    let b17 = find(&els, "button", "B17");
    let (x, y) = center(b17);
    let r = rig.json(&["tap", &ref_of(b17)]);
    let log = testbed::wait_for(&rig.udid, "tap", |e| e["ev"] == "tap");
    let taps: Vec<&Value> = log.iter().filter(|e| e["ev"] == "tap").collect();
    assert_eq!(taps.len(), 1);
    assert_eq!(taps[0]["id"], "b17");
    for t in log.iter().filter(|e| e["ev"] == "touch") {
        assert!((t["x"].as_f64().unwrap() - x).abs() <= 0.5, "{t}");
        assert!((t["y"].as_f64().unwrap() - y).abs() <= 0.5, "{t}");
    }
    assert_eq!(r["observation"]["kind"], "diff", "{r}");
    assert_eq!(r["changed"], true, "{r}");
    assert_eq!(r["settled"], true, "{r}");
    let diff = r["observation"]["text"].as_str().unwrap();
    assert!(diff.starts_with('~'), "{diff}");
    assert!(diff.contains("\"Last tap: b17\""), "{diff}");
}

#[test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE=sim:<udid>"]
fn a_tab_switch_is_a_new_screen() {
    let rig = Rig::new();
    rig.tab("taps");
    rig.ok(&["snapshot"]);
    let r = rig.json(&["tap", "List"]);
    testbed::wait_for(&rig.udid, "tab list", |e| {
        e["ev"] == "tab" && e["name"] == "list"
    });
    assert_eq!(r["observation"]["kind"], "full", "{r}");
    assert!(
        r["observation"]["text"].as_str().unwrap().contains("Row 0"),
        "{r}"
    );
}

#[test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE=sim:<udid>"]
fn type_unicode_into_a_field_is_exact_and_verified() {
    let rig = Rig::new();
    rig.tab("text");
    let els = rig.elements();
    let field = els
        .iter()
        .find(|e| e["id"] == "text.plain")
        .expect("no plain field");
    let text = "héllo 世界 مرحبا 👋🏽";
    let r = rig.json(&["type", text, "--into", &ref_of(field)]);
    testbed::wait_for(&rig.udid, "plain", |e| {
        e["field"] == "plain" && e["value"] == text
    });
    assert_eq!(r["verified"], true, "{r}");
}

#[test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE=sim:<udid>"]
fn a_secret_from_stdin_is_typed_and_never_written_down() {
    let rig = Rig::new();
    rig.tab("text");
    let els = rig.elements();
    let field = els
        .iter()
        .find(|e| e["id"] == "text.password")
        .expect("no password field");
    let secret = "hunter2-e2e-canary";
    let out = rig
        .command()
        .args(["--json", "type", "--secret", "--into", &ref_of(field)])
        .write_stdin(secret)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!String::from_utf8_lossy(&out.stdout).contains(secret));
    testbed::wait_for(&rig.udid, "password", |e| {
        e["field"] == "password" && e["length"] == secret.len()
    });
    for entry in walk(&rig.home) {
        let bytes = std::fs::read(&entry).unwrap_or_default();
        assert!(
            !String::from_utf8_lossy(&bytes).contains(secret),
            "{} holds the secret",
            entry.display()
        );
    }
}

fn walk(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else if p.extension().is_none_or(|x| x != "png") {
            out.push(p);
        }
    }
    out
}

#[test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE=sim:<udid>"]
fn press_back_says_it_used_the_nav_bar_button() {
    let rig = Rig::new();
    rig.tab("nav");
    rig.ok(&["tap", "Open level 1"]);
    testbed::wait_for(&rig.udid, "level 1", |e| {
        e["ev"] == "nav" && e["level"] == 1
    });
    testbed::clear_log(&rig.udid);
    let r = rig.json(&["press", "back"]);
    assert_eq!(r["via"], "nav-button", "{r}");
    assert_eq!(r["summary"], "pressed back via nav-button", "{r}");
    testbed::wait_for(&rig.udid, "level 0", |e| {
        e["ev"] == "nav" && e["level"] == 0
    });
}

#[test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE=sim:<udid>"]
fn a_screenshot_is_one_pixel_per_point() {
    let rig = Rig::new();
    rig.tab("taps");
    let r = rig.json(&["screenshot"]);
    assert_eq!(
        (r["width"].as_u64(), r["height"].as_u64()),
        (Some(402), Some(874)),
        "{r}"
    );
    let png = std::fs::read(r["path"].as_str().unwrap()).unwrap();
    assert_eq!(&png[1..4], b"PNG");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE=sim:<udid>"]
async fn a_daemon_killed_alone_leaves_its_runner_to_the_next_and_its_process_group_ends_it() {
    let rig = Rig::new();
    let _ = std::process::Command::new("pkill")
        .args(["-f", &format!("test-without-building.*id={}", rig.udid)])
        .status();
    tokio::time::sleep(Duration::from_secs(2)).await;
    rig.ok(&["snapshot", "--no-settle"]);
    assert_eq!(rig.runners(), 1);
    let first = rig.daemon_pid().await;
    std::process::Command::new("kill")
        .args(["-KILL", &first.to_string()])
        .status()
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(rig.runners(), 1, "the runner died with the daemon alone");

    let started = Instant::now();
    rig.ok(&["snapshot", "--no-settle"]);
    let took = started.elapsed();
    assert_eq!(rig.runners(), 1, "the second daemon started another runner");
    assert!(took < Duration::from_secs(3), "{took:?}");

    let second = rig.daemon_pid().await;
    assert_ne!(first, second);
    rig.ok(&["release"]);
    std::process::Command::new("kill")
        .args(["-TERM", &format!("-{first}")])
        .status()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while rig.runners() > 0 && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert_eq!(rig.runners(), 0, "the runner outlived its process group");
}

#[test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE=sim:<udid>"]
fn doctor_finds_xcode_the_simulator_and_a_runner_that_answers() {
    let rig = Rig::new();
    let out = rig
        .command()
        .env_remove("PHONEDRIVER_DEVICE")
        .arg("doctor")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "{text}");
    for name in ["xcode", "xcodegen", "simulators", "runner build"] {
        assert!(
            text.lines()
                .any(|l| l.starts_with("ok") && l.contains(name)),
            "no ok line for {name}:\n{text}"
        );
    }
    let mine = text
        .lines()
        .find(|l| l.contains(&rig.udid) && l.contains("runner"))
        .unwrap_or_else(|| panic!("no runner line for {}:\n{text}", rig.udid));
    assert!(mine.starts_with("ok"), "{text}");
    assert!(!text.lines().any(|l| l.starts_with("fail")), "{text}");
}
