use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

fn newest(root: &Path) -> Option<SystemTime> {
    let mut newest: Option<SystemTime> = None;
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            let Ok(entries) = std::fs::read_dir(&path) else {
                continue;
            };
            for entry in entries.flatten() {
                pending.push(entry.path());
            }
        } else if let Ok(t) = meta.modified()
            && newest.is_none_or(|n| t > n)
        {
            newest = Some(t);
        }
    }
    newest
}

fn main() {
    println!("cargo:rerun-if-env-changed=PHONE_USE_AGENT_JAR");
    println!("cargo:rerun-if-env-changed=ANDROID_HOME");
    println!("cargo:rerun-if-changed=../../android-agent/src");
    println!("cargo:rerun-if-changed=../../android-agent/aidl");
    println!("cargo:rerun-if-changed=../../android-agent/build.sh");

    if let Ok(jar) = std::env::var("PHONE_USE_AGENT_JAR") {
        assert!(
            Path::new(&jar).is_file(),
            "PHONE_USE_AGENT_JAR={jar} is not a file; point it at a built phone-use-agent.jar (bash android-agent/build.sh makes one) or unset it"
        );
        println!("cargo:rustc-env=PHONE_USE_AGENT_JAR={jar}");
        return;
    }

    let agent =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../android-agent");
    let jar = agent.join("build/phone-use-agent.jar");
    let jar_time = newest(&jar);
    let mut stale = jar_time.is_none();
    for dir in ["src", "aidl", "build.sh"] {
        if let (Some(source), Some(built)) = (newest(&agent.join(dir)), jar_time)
            && source > built
        {
            stale = true;
        }
    }
    if stale {
        let status = Command::new("bash")
            .arg(agent.join("build.sh"))
            .status()
            .expect("run android-agent/build.sh");
        assert!(
            status.success(),
            "android-agent/build.sh failed (its message is above). For a build without Android: cargo build --release -p phone-use --no-default-features"
        );
    }
    let jar = jar.canonicalize().expect("agent jar after build");
    println!("cargo:rustc-env=PHONE_USE_AGENT_JAR={}", jar.display());
}
