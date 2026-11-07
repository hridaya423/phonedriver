use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../android-agent/build.sh")
}

fn touch(path: &Path, executable: bool) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
    if executable {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

struct Sdk(tempfile::TempDir);

impl Sdk {
    fn with(build_tools: &[&str]) -> Sdk {
        let dir = tempfile::tempdir().unwrap();
        for v in build_tools {
            std::fs::create_dir_all(dir.path().join("build-tools").join(v)).unwrap();
        }
        Sdk(dir)
    }

    fn tool(&self, name: &str) {
        touch(
            &self.0.path().join("build-tools/37.0.0").join(name),
            name != "core-lambda-stubs.jar",
        );
    }

    fn platform(&self) {
        touch(
            &self.0.path().join("platforms/android-37.0/android.jar"),
            false,
        );
    }

    fn run(&self, path: &str) -> (bool, String) {
        let out = Command::new("/bin/bash")
            .arg(script())
            .env_clear()
            .env("PATH", path)
            .env("ANDROID_HOME", self.0.path())
            .output()
            .unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }
}

#[test]
fn missing_build_tools_names_the_version_what_was_found_and_the_way_out() {
    let sdk = Sdk::with(&["35.0.0", "36.0.0"]);
    let (ok, stderr) = sdk.run("/usr/bin:/bin");
    assert!(!ok);
    for needle in [
        "build-tools 37.0.0",
        "found: 35.0.0 36.0.0",
        "ANDROID_BUILD_TOOLS",
        "ANDROID_HOME",
        "sdkmanager",
        "PHONEDRIVER_AGENT_JAR",
        "--no-default-features",
    ] {
        assert!(stderr.contains(needle), "{needle} not in:\n{stderr}");
    }
}

#[test]
fn a_missing_sdk_directory_says_so() {
    let sdk = Sdk::with(&[]);
    std::fs::remove_dir_all(sdk.0.path()).unwrap();
    let (ok, stderr) = sdk.run("/usr/bin:/bin");
    assert!(!ok);
    assert!(stderr.contains("no Android SDK at"), "{stderr}");
}

#[test]
fn every_missing_tool_is_named_in_one_message() {
    let sdk = Sdk::with(&["37.0.0"]);
    let (ok, stderr) = sdk.run("/nonexistent");
    assert!(!ok);
    for needle in [
        "- aidl",
        "- d8",
        "- core-lambda-stubs.jar",
        "- android.jar",
        "- javac",
        "JDK 17",
    ] {
        assert!(stderr.contains(needle), "{needle} not in:\n{stderr}");
    }
}

#[test]
fn a_complete_sdk_without_a_jdk_names_only_the_jdk() {
    let sdk = Sdk::with(&["37.0.0"]);
    for t in ["aidl", "d8", "core-lambda-stubs.jar"] {
        sdk.tool(t);
    }
    sdk.platform();
    let (ok, stderr) = sdk.run("/nonexistent");
    assert!(!ok);
    assert!(stderr.contains("javac"), "{stderr}");
    assert!(!stderr.contains("- aidl"), "{stderr}");
    assert!(!stderr.contains("- d8"), "{stderr}");
}
