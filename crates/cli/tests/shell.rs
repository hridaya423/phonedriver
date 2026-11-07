#[path = "support/fakes.rs"]
mod fakes;

use std::sync::Arc;

use async_trait::async_trait;
use fakes::tree;
use phonedriver::daemon::rpc::RpcError;
use phonedriver::daemon::{Config, Daemon, DeviceSource, Listed};
use phonedriver_core::device::{Capabilities, Device, ShellReply};
use phonedriver_core::error::Error;
use phonedriver_core::testing::{Call, FakeDevice};
use serde_json::{Value, json};

struct One {
    id: &'static str,
    fake: Arc<FakeDevice>,
}

#[async_trait]
impl DeviceSource for One {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(vec![Listed {
            id: self.id.into(),
            model: "Fake".into(),
            os: None,
            tier: "fake".into(),
            capabilities: self.fake.info().capabilities,
        }])
    }
    async fn connect(&self, _id: &str) -> Result<Arc<dyn Device>, Error> {
        Ok(self.fake.clone())
    }
}

fn setup(id: &'static str, fake: FakeDevice) -> (Arc<Daemon>, Arc<FakeDevice>, tempfile::TempDir) {
    let fake = Arc::new(fake);
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::new(
        Arc::new(One {
            id,
            fake: fake.clone(),
        }),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    (d, fake, home)
}

fn polled(trees: Vec<phonedriver_core::raw::RawTree>) -> FakeDevice {
    FakeDevice::new(trees).lacking(Capabilities::FRAMES)
}

async fn ok(d: &Daemon, method: &str, params: Value) -> Value {
    d.call("default", method, params)
        .await
        .unwrap_or_else(|e| panic!("{method} failed: {e:?}"))
}

async fn err(d: &Daemon, params: Value) -> RpcError {
    d.call("default", "shell", params)
        .await
        .expect_err("expected an error")
}

fn command(c: &str) -> Value {
    json!({"command": c})
}

fn ran(fake: &FakeDevice) -> Vec<String> {
    fake.calls()
        .into_iter()
        .filter_map(|c| match c {
            Call::Shell(s) => Some(s),
            _ => None,
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn a_command_that_only_reads_returns_exit_and_trimmed_output_without_a_screen_part() {
    let (d, fake, _home) = setup("fake:1", polled(vec![tree(&["One"])]));
    fake.answer_shell(ShellReply {
        exit: 0,
        stdout: (1..=100).map(|n| format!("row {n}\r\n")).collect(),
        ..Default::default()
    });
    let r = ok(&d, "shell", command("dumpsys window")).await;
    assert_eq!(r["exit"], 0);
    assert_eq!(r["omitted"], 60);
    let out = r["output"].as_str().unwrap();
    assert!(out.starts_with("row 1\nrow 2\n"), "{out}");
    assert!(out.contains("… 60 more lines"), "{out}");
    assert!(r.get("action").is_none(), "{r}");
    let r = ok(
        &d,
        "shell",
        json!({"command": "dumpsys window", "grep": "row 9", "head": 3}),
    )
    .await;
    assert_eq!(
        r["output"],
        "row 9\nrow 90\nrow 91\n… 8 more lines (narrow it with grep, or set head to 0 for all)"
    );
}

#[tokio::test(start_paused = true)]
async fn a_command_that_changes_things_settles_and_returns_the_screen_diff_with_its_output() {
    let (d, fake, _home) = setup(
        "fake:1",
        polled(vec![
            tree(&["Wi-Fi", "Display", "Battery", "Light theme"]),
            tree(&["Wi-Fi", "Display", "Battery", "Dark theme"]),
        ]),
    );
    let r = ok(&d, "shell", command("cmd uimode night yes")).await;
    assert_eq!(r["exit"], 0);
    let action = &r["action"];
    assert_eq!(action["observation"]["kind"], "diff", "{r}");
    let text = action["observation"]["text"].as_str().unwrap();
    assert!(text.contains("Dark theme"), "{text}");
    assert_eq!(action["changed"], true);
    assert_eq!(ran(&fake), ["cmd uimode night yes"]);
}

#[tokio::test(start_paused = true)]
async fn settle_false_skips_the_wait_and_the_screen_part() {
    let (d, _fake, _home) = setup("fake:1", polled(vec![tree(&["One"])]));
    let r = ok(
        &d,
        "shell",
        json!({"command": "cmd uimode night yes", "settle": false}),
    )
    .await;
    assert!(r.get("action").is_none(), "{r}");
}

#[tokio::test(start_paused = true)]
async fn a_physical_device_refuses_clearly_destructive_commands_unless_forced() {
    let fake = FakeDevice::new(vec![tree(&["One"])]);
    fake.install(&[("com.example.installed", "Mine")]);
    let (d, fake, _home) = setup("android:R5CT123", fake);
    for c in [
        "rm -rf /sdcard",
        "rm -rf /data/*",
        "pm uninstall com.example.theirs",
        "pm clear com.example.theirs",
        "cmd package uninstall com.example.theirs",
        "recovery --wipe_data",
    ] {
        let e = err(&d, command(c)).await;
        assert_eq!(e.code, -32011, "{c}");
        assert!(
            e.message.contains("refused on a physical device"),
            "{}",
            e.message
        );
        assert!(e.message.contains("force"), "{}", e.message);
    }
    assert!(ran(&fake).is_empty());
    ok(
        &d,
        "shell",
        json!({"command": "pm uninstall com.example.theirs", "force": true}),
    )
    .await;
    ok(&d, "shell", command("rm -rf /sdcard/Download/old")).await;
    ok(
        &d,
        "app",
        json!({"action": "install", "path": "/tmp/a.apk"}),
    )
    .await;
    ok(&d, "shell", command("pm clear com.example.installed")).await;
    assert_eq!(
        ran(&fake),
        [
            "pm uninstall com.example.theirs",
            "rm -rf /sdcard/Download/old",
            "pm clear com.example.installed"
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn an_emulator_or_simulator_runs_any_command() {
    for id in ["android:emulator-5556", "sim:ABC"] {
        let (d, fake, _home) = setup(id, polled(vec![tree(&["One"])]));
        ok(&d, "shell", command("rm -rf /sdcard")).await;
        assert_eq!(ran(&fake), ["rm -rf /sdcard"], "{id}");
    }
}

#[tokio::test(start_paused = true)]
async fn a_device_without_a_shell_says_so_and_runs_nothing() {
    let fake = FakeDevice::new(vec![tree(&["One"])]).lacking(Capabilities::SHELL);
    let (d, fake, _home) = setup("fake:1", fake);
    let e = err(&d, command("echo hi")).await;
    assert_eq!(e.code, -32011);
    assert!(e.message.contains("no shell"), "{}", e.message);
    assert!(fake.calls().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_secret_command_is_never_echoed_back_in_the_summary_or_the_output() {
    let (d, fake, _home) = setup("fake:1", polled(vec![tree(&["One"]), tree(&["Two"])]));
    let c = "cmd wifi connect-network x wpa2 hunter2hunter2";
    fake.answer_shell(ShellReply {
        exit: 1,
        stdout: format!("failed: {c}\n"),
        ..Default::default()
    });
    let r = ok(&d, "shell", json!({"command": c, "secret": true})).await;
    assert_eq!(r["exit"], 1);
    assert!(!r.to_string().contains("hunter2"), "{r}");
    assert_eq!(r["output"], "failed: •••");
}
