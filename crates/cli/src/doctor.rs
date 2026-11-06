//! `phone-use doctor`: checks the machine and the devices, one line each, `ok`, `warn` or `fail`, with a fix under
//! every line that is not `ok`: the adb server, each Android device and its agent, UiAutomation and video, the iOS
//! Simulator tier, and the daemon socket's permissions.

use std::path::Path;
use std::time::Instant;

use phone_use_android::adb::{Adb, AdbDevice, DeviceState};
use serde_json::json;

use crate::client::{Client, Failure};
use crate::daemon::socket;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Check {
    pub status: Status,
    pub name: String,
    pub detail: String,
    pub fix: Option<String>,
}

impl Check {
    fn ok(name: impl Into<String>, detail: impl Into<String>) -> Check {
        Check {
            status: Status::Ok,
            name: name.into(),
            detail: detail.into(),
            fix: None,
        }
    }

    fn bad(
        status: Status,
        name: impl Into<String>,
        detail: impl Into<String>,
        fix: impl Into<String>,
    ) -> Check {
        Check {
            status,
            name: name.into(),
            detail: detail.into(),
            fix: Some(fix.into()),
        }
    }
}

pub fn render(checks: &[Check]) -> String {
    let width = checks
        .iter()
        .map(|c| c.name.chars().count())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for c in checks {
        let status = match c.status {
            Status::Ok => "ok",
            Status::Warn => "warn",
            Status::Fail => "fail",
        };
        out.push_str(&format!("{status:<4}  {:<width$}  {}\n", c.name, c.detail));
        if let Some(fix) = &c.fix {
            out.push_str(&format!("      fix: {fix}\n"));
        }
    }
    out
}

/// Runs every check and prints them. Fails when any check fails.
pub fn run(home: &Path) -> bool {
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        eprintln!("error: cannot start the async runtime");
        return false;
    };
    let checks = rt.block_on(async {
        let mut checks = android(&Adb::from_env()).await;
        checks.extend(ios(home).await);
        checks.extend(socket_permissions(&socket::socket_path()));
        checks
    });
    print!("{}", render(&checks));
    !checks.iter().any(|c| c.status == Status::Fail)
}

/// The Simulator tier: Xcode, xcodegen, booted simulators, the runner build, and each simulator's runner answering
/// through the daemon. A runner serves one connection and a new one cancels the old, so probing its port directly
/// would cut off a daemon that holds it; the daemon connects (which says `hello`) and takes one snapshot instead.
pub async fn ios(home: &Path) -> Vec<Check> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }
    let mut checks = Vec::new();
    let xcode = tokio::process::Command::new("xcodebuild")
        .arg("-version")
        .output()
        .await;
    let xcode_ok = match xcode {
        Ok(out) if out.status.success() => {
            let v = String::from_utf8_lossy(&out.stdout);
            checks.push(Check::ok("xcode", v.lines().collect::<Vec<_>>().join(", ")));
            true
        }
        _ => {
            checks.push(Check::bad(
                Status::Fail,
                "xcode",
                "xcodebuild does not run",
                "Install Xcode, open it once, then run `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`.",
            ));
            false
        }
    };
    let xcodegen = phone_use_ios::runner::find_tool("xcodegen");
    match &xcodegen {
        Some(path) => checks.push(Check::ok("xcodegen", path.display().to_string())),
        None => checks.push(Check::bad(
            Status::Fail,
            "xcodegen",
            "not found in PATH, /opt/homebrew/bin or /usr/local/bin",
            "The runner's Xcode project is generated from its sources: `brew install xcodegen`.",
        )),
    }
    if !xcode_ok {
        return checks;
    }
    let sims = match phone_use_ios::simulator::Simulator::list().await {
        Ok(sims) if sims.is_empty() => {
            checks.push(Check::bad(
                Status::Warn,
                "simulators",
                "none booted",
                "Boot one in Simulator.app or with `xcrun simctl boot <udid>`; `xcrun simctl list devices available` lists them.",
            ));
            Vec::new()
        }
        Ok(sims) => {
            let names: Vec<String> = sims
                .iter()
                .map(|s| format!("{} ({})", s.name, s.os))
                .collect();
            checks.push(Check::ok(
                "simulators",
                format!("{} booted: {}", sims.len(), names.join(", ")),
            ));
            sims
        }
        Err(e) => {
            checks.push(Check::bad(
                Status::Fail,
                "simulators",
                format!("`simctl list` failed: {e:?}"),
                "Check that `xcrun simctl list` runs; Xcode's command line tools must be selected.",
            ));
            Vec::new()
        }
    };
    if xcodegen.is_none() || sims.is_empty() {
        return checks;
    }
    let started = Instant::now();
    match phone_use_ios::runner::build(home).await {
        Ok(b) if b.built => checks.push(Check::ok(
            "runner build",
            format!(
                "{} built in {:.1} s",
                b.hash,
                started.elapsed().as_secs_f64()
            ),
        )),
        Ok(b) => checks.push(Check::ok("runner build", format!("{} up to date", b.hash))),
        Err(e) => {
            checks.push(Check::bad(
                Status::Fail,
                "runner build",
                format!("{e:?}"),
                "Build for the iOS Simulator once in Xcode to accept the license and install components (`sudo xcodebuild -license accept`, `xcodebuild -runFirstLaunch`).",
            ));
            return checks;
        }
    }
    for s in &sims {
        checks.push(runner_answers(&format!("sim:{}", s.udid)).await);
    }
    checks
}

/// The adb server at `Adb::from_env()`, then every device it lists. A device in the `device` state is probed through
/// the daemon, which deploys the agent and holds UiAutomation, so the probe must not start a second agent.
pub async fn android(adb: &Adb) -> Vec<Check> {
    let configured = std::env::var("ADB_SERVER_SOCKET").ok();
    let version = adb.version().await;
    let mut checks = vec![adb_server(adb.addr(), configured.as_deref(), &version)];
    if version.is_err() {
        return checks;
    }
    let devices = match adb.devices().await {
        Ok(d) => d,
        Err(e) => {
            checks.push(Check::bad(
                Status::Fail,
                "adb devices",
                format!("{e}"),
                "Run `adb devices` and read what it prints.",
            ));
            return checks;
        }
    };
    if devices.is_empty() {
        checks.push(Check::bad(
            Status::Warn,
            "android devices",
            "none connected",
            "Connect a phone with USB debugging on, or start an emulator; `adb devices` lists what adb sees.",
        ));
    }
    for d in &devices {
        let state = device_state(d);
        let ready = state.status == Status::Ok;
        checks.push(state);
        if ready {
            checks.extend(agent_checks(&format!("android:{}", d.serial)).await);
        }
    }
    checks
}

fn adb_server(
    addr: &str,
    configured: Option<&str>,
    version: &Result<String, phone_use_core::error::Error>,
) -> Check {
    match (version, configured) {
        (Ok(v), _) => Check::ok("adb server", format!("{v} at {addr}")),
        (Err(e), Some(spec)) => Check::bad(
            Status::Fail,
            "adb server",
            format!("cannot reach ADB_SERVER_SOCKET={spec}: {e}"),
            "Open the tunnel that ADB_SERVER_SOCKET points at (for example `ssh -N -L 5038:127.0.0.1:5037 <host>`), or unset it to use the local adb.",
        ),
        (Err(e), None) => Check::bad(
            Status::Warn,
            "adb server",
            format!("none at {addr}: {e}"),
            "Only Android needs it: install the Android platform-tools and run `adb start-server`.",
        ),
    }
}

fn device_state(d: &AdbDevice) -> Check {
    let name = format!("android:{}", d.serial);
    match d.state {
        DeviceState::Device => Check::ok(name, "connected"),
        DeviceState::Unauthorized => Check::bad(
            Status::Fail,
            name,
            "unauthorized",
            "Unlock the phone and accept the USB debugging prompt on the phone; tick \"Always allow\" to keep it.",
        ),
        DeviceState::Offline => Check::bad(
            Status::Fail,
            name,
            "offline",
            "Unplug and replug the cable, or run `adb reconnect offline`.",
        ),
        DeviceState::Other => Check::bad(
            Status::Warn,
            name,
            "not ready (booting, recovery or bootloader)",
            "Wait for the device to finish booting, then run doctor again.",
        ),
    }
}

async fn agent_checks(id: &str) -> Vec<Check> {
    let mut client = match Client::connect("doctor".into()).await {
        Ok(c) => c,
        Err(e) => return vec![cannot_reach_daemon(format!("agent {id}"), &e)],
    };
    let started = Instant::now();
    let reply = client.call("health", json!({"device": id})).await;
    let _ = client.call("release", json!({"device": id})).await;
    match reply {
        Ok(r) => {
            let took = started.elapsed().as_secs_f64();
            let mut checks = vec![
                Check::ok(
                    format!("agent {id}"),
                    format!(
                        "answers hello: {} on {}, {took:.1} s with deploy",
                        r["model"].as_str().unwrap_or("unknown model"),
                        r["os"].as_str().unwrap_or("unknown OS")
                    ),
                ),
                Check::ok(
                    format!("uiautomation {id}"),
                    "connected, the screen tree reads",
                ),
            ];
            checks.push(match r["frames_ms"].as_u64() {
                Some(ms) => Check::ok(format!("video {id}"), format!("a frame arrived after {ms} ms")),
                None => Check::bad(
                    Status::Warn,
                    format!("video {id}"),
                    "no frame within 1 s",
                    "The screen may be off: wake the device and run doctor again. Without frames, settling falls back to its timeout.",
                ),
            });
            checks
        }
        Err(Failure::Rpc(e)) if e.code == -32014 => vec![
            Check::ok(format!("agent {id}"), "deployed and started"),
            Check::bad(
                Status::Fail,
                format!("uiautomation {id}"),
                e.message.clone(),
                e.data["hint"]
                    .as_str()
                    .unwrap_or("Stop the other UiAutomation client, then run doctor again.")
                    .to_string(),
            ),
        ],
        Err(e) => vec![Check::bad(
            Status::Fail,
            format!("agent {id}"),
            e.message(),
            e.hint()
                .unwrap_or_else(|| "The daemon log is `daemon.log` in the phone-use home.".into()),
        )],
    }
}

fn cannot_reach_daemon(name: String, e: &Failure) -> Check {
    Check::bad(
        Status::Fail,
        name,
        format!("cannot reach the daemon: {}", e.message()),
        "Run `phone-use daemon` in a terminal and read what it prints.",
    )
}

/// The socket is the only way in to the daemon, so only its owner may use it: mode 0600 in a directory of mode 0700.
pub fn socket_permissions(path: &Path) -> Option<Check> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).map(|m| m.permissions().mode() & 0o777);
        let name = "daemon socket";
        let Ok(file) = mode(path) else {
            return Some(Check::ok(
                name,
                format!(
                    "{} does not exist; the daemon starts on first use",
                    path.display()
                ),
            ));
        };
        let dir = path.parent().map(mode).and_then(Result::ok).unwrap_or(0);
        if file & 0o077 == 0 && dir & 0o077 == 0 {
            Some(Check::ok(
                name,
                format!(
                    "{} mode {file:03o} in a directory of mode {dir:03o}",
                    path.display()
                ),
            ))
        } else {
            Some(Check::bad(
                Status::Fail,
                name,
                format!(
                    "{} mode {file:03o} in a directory of mode {dir:03o}; other users can reach it",
                    path.display()
                ),
                format!(
                    "Run `chmod 600 {}` and `chmod 700 {}`.",
                    path.display(),
                    path.parent()
                        .map_or(String::new(), |p| p.display().to_string())
                ),
            ))
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

async fn runner_answers(id: &str) -> Check {
    let name = format!("runner {id}");
    let started = Instant::now();
    let mut client = match Client::connect("doctor".into()).await {
        Ok(c) => c,
        Err(e) => return cannot_reach_daemon(name, &e),
    };
    let reply = client
        .call("snapshot", json!({"device": id, "no_settle": true}))
        .await;
    let _ = client.call("release", json!({"device": id})).await;
    match reply {
        Ok(r) => {
            let app = r["header"]["app"]["id"].as_str().unwrap_or("no app");
            Check::ok(
                name,
                format!(
                    "answers: {app} in front, {:.1} s",
                    started.elapsed().as_secs_f64()
                ),
            )
        }
        Err(e) => Check::bad(
            Status::Fail,
            name,
            e.message(),
            e.hint().unwrap_or_else(|| {
                "The runner log is in the phone-use home under runner/<udid>.log.".into()
            }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_check_is_one_aligned_line_and_a_fix_follows_a_problem() {
        let checks = vec![
            Check::ok("xcode", "Xcode 27.0"),
            Check::bad(Status::Warn, "simulators", "none booted", "Boot one."),
            Check::bad(
                Status::Fail,
                "runner build",
                "xcodebuild failed",
                "Open Xcode once.",
            ),
        ];
        assert_eq!(
            render(&checks),
            "ok    xcode         Xcode 27.0\n\
             warn  simulators    none booted\n      fix: Boot one.\n\
             fail  runner build  xcodebuild failed\n      fix: Open Xcode once.\n"
        );
    }

    #[test]
    fn without_a_configured_adb_server_a_missing_one_is_a_warning() {
        let down = Err(phone_use_core::error::Error::Transport {
            detail: "refused".into(),
        });
        let local = adb_server("127.0.0.1:5037", None, &down);
        assert_eq!(local.status, Status::Warn);
        let tunnel = adb_server("127.0.0.1:5038", Some("tcp:127.0.0.1:5038"), &down);
        assert_eq!(tunnel.status, Status::Fail);
        assert!(
            tunnel
                .detail
                .contains("ADB_SERVER_SOCKET=tcp:127.0.0.1:5038")
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_socket_must_be_private_to_its_owner() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("daemon.sock");
        let set = |p: &Path, m: u32| {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(m)).unwrap()
        };
        assert_eq!(socket_permissions(&sock).unwrap().status, Status::Ok);
        std::fs::write(&sock, "").unwrap();
        set(dir.path(), 0o700);
        set(&sock, 0o600);
        assert_eq!(socket_permissions(&sock).unwrap().status, Status::Ok);
        set(&sock, 0o666);
        let loose = socket_permissions(&sock).unwrap();
        assert_eq!(loose.status, Status::Fail);
        assert!(loose.fix.unwrap().contains("chmod 600"));
        set(&sock, 0o600);
        set(dir.path(), 0o755);
        assert_eq!(socket_permissions(&sock).unwrap().status, Status::Fail);
    }
}
