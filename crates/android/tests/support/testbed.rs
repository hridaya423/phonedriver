use std::path::PathBuf;
use std::process::Command;

use phone_use_android::adb::Adb;
use serde_json::Value;

pub const PACKAGE: &str = "dev.phoneuse.testbed";

fn project_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testbeds/android")
}

/// Builds the testbed with Gradle and installs it on `serial` through our adb client, so `ADB_SERVER_SOCKET` is honored (Gradle's own installer ignores it).
pub async fn install(adb: &Adb, serial: &str) {
    let status = Command::new(project_dir().join("gradlew"))
        .arg("--console=plain")
        .arg("assembleDebug")
        .current_dir(project_dir())
        .env(
            "ANDROID_HOME",
            std::env::var("ANDROID_HOME")
                .unwrap_or_else(|_| "/opt/homebrew/share/android-commandlinetools".into()),
        )
        .status()
        .expect("run gradlew");
    assert!(status.success(), "gradlew assembleDebug failed");
    let apk = std::fs::read(project_dir().join("app/build/outputs/apk/debug/app-debug.apk"))
        .expect("read apk");
    adb.push(serial, &apk, "/data/local/tmp/phone-use-testbed.apk", 0o644)
        .await
        .unwrap();
    let out = adb
        .shell(
            serial,
            "pm install -r /data/local/tmp/phone-use-testbed.apk",
        )
        .await
        .unwrap();
    assert!(
        out.stdout_text().contains("Success"),
        "{}",
        out.stdout_text()
    );
    adb.shell(serial, "am force-stop dev.phoneuse.testbed")
        .await
        .unwrap();
}

pub async fn clear_log(adb: &Adb, serial: &str) {
    adb.shell(serial, "logcat -c").await.unwrap();
}

pub async fn read_log(adb: &Adb, serial: &str) -> Vec<Value> {
    let out = adb
        .shell(serial, "logcat -d -s PUTB:I -v raw")
        .await
        .unwrap();
    parse_log(&out.stdout_text())
}

pub fn parse_log(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
        .collect()
}

/// Brings the testbed to the front on the named tab (`taps`, `text`, `list`, `gestures`, `busy`, `secure`, `dialogs`, `web`).
pub async fn open_tab(adb: &Adb, serial: &str, tab: &str) {
    adb.shell(
        serial,
        &format!("am start -n {PACKAGE}/.MainActivity --es tab {tab}"),
    )
    .await
    .unwrap();
}

pub async fn wait_for_log(
    adb: &Adb,
    serial: &str,
    timeout_ms: u64,
    pred: impl Fn(&[Value]) -> bool,
) -> Vec<Value> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        let log = read_log(adb, serial).await;
        if pred(&log) || std::time::Instant::now() > deadline {
            return log;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

static INSTALLED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

pub async fn ensure_installed(adb: &Adb, serial: &str) {
    INSTALLED.get_or_init(|| install(adb, serial)).await;
}
