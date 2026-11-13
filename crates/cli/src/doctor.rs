//! `phonedriver doctor`: checks the machine and the devices, one line each, `ok`, `warn` or `fail`, with a fix under
//! every line that is not `ok`: the adb server, each Android device and its agent, UiAutomation and video, the iOS
//! Simulator tier, and the daemon socket's permissions.

use std::path::Path;
use std::time::Instant;

#[cfg(feature = "android")]
use phonedriver_android::adb::{Adb, AdbDevice, DeviceState};
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

/// Whether the device named with `--device` (an id, or the part after its colon) is `id`. Nothing named means every device.
fn wants(only: Option<&str>, id: &str) -> bool {
    only.is_none_or(|o| o == id || id.split_once(':').is_some_and(|(_, rest)| rest == o))
}

/// Runs every check and prints them; with `only`, probes just that device. Fails when any check fails.
pub fn run(home: &Path, only: Option<&str>) -> bool {
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        eprintln!("error: cannot start the async runtime");
        return false;
    };
    let checks = rt.block_on(async {
        #[cfg(feature = "android")]
        let mut checks = android(&Adb::from_env(), only).await;
        #[cfg(not(feature = "android"))]
        let mut checks = Vec::new();
        checks.extend(ios(home, only).await);
        checks.extend(physical(home, only).await);
        checks.extend(socket_permissions(&socket::socket_path()));
        checks
    });
    print!("{}", render(&checks));
    !checks.iter().any(|c| c.status == Status::Fail)
}

/// The Simulator tier: Xcode, xcodegen, booted simulators, the runner build, and each simulator's runner answering
/// through the daemon. A runner serves one connection and a new one cancels the old, so probing its port directly
/// would cut off a daemon that holds it; the daemon connects (which says `hello`) and takes one snapshot instead.
pub async fn ios(home: &Path, only: Option<&str>) -> Vec<Check> {
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
    let xcodegen = phonedriver_ios::runner::find_tool("xcodegen");
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
    let sims = match phonedriver_ios::simulator::Simulator::list().await {
        Ok(sims) if sims.is_empty() && only.is_none() => {
            checks.push(Check::bad(
                Status::Warn,
                "simulators",
                "none booted",
                "Boot one in Simulator.app or with `xcrun simctl boot <udid>`; `xcrun simctl list devices available` lists them.",
            ));
            Vec::new()
        }
        Ok(mut sims) => {
            sims.retain(|s| wants(only, &format!("sim:{}", s.udid)));
            if !sims.is_empty() || only.is_none() {
                let names: Vec<String> = sims
                    .iter()
                    .map(|s| format!("{} ({})", s.name, s.os))
                    .collect();
                checks.push(Check::ok(
                    "simulators",
                    format!("{} booted: {}", sims.len(), names.join(", ")),
                ));
            }
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
    match phonedriver_ios::runner::build(home).await {
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
#[cfg(feature = "android")]
pub async fn android(adb: &Adb, only: Option<&str>) -> Vec<Check> {
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
    let devices = match only {
        Some(_) => devices,
        None => phonedriver_android::wireless::connections_per_phone(adb, devices).await,
    };
    if devices.is_empty() {
        checks.push(Check::bad(
            Status::Warn,
            "android devices",
            "none connected",
            "Connect a phone with USB debugging on, pair an Android 11+ phone over Wi-Fi with `phonedriver pair`, or start an emulator; `adb devices` lists what adb sees.",
        ));
    }
    if let Some(only) = only
        && only.starts_with("android:")
        && !devices
            .iter()
            .any(|d| wants(Some(only), &format!("android:{}", d.serial)))
    {
        let listed: Vec<&str> = devices.iter().map(|d| d.serial.as_str()).collect();
        checks.push(Check::bad(
            Status::Fail,
            only,
            format!(
                "adb does not list it{}",
                if listed.is_empty() {
                    String::new()
                } else {
                    format!("; it lists {}", listed.join(", "))
                }
            ),
            "Pass `--device` an id that `adb devices` lists, or leave `--device` out to check every device.",
        ));
    }
    for d in devices
        .iter()
        .filter(|d| wants(only, &format!("android:{}", d.serial)))
    {
        let state = device_state(d);
        let ready = state.status == Status::Ok;
        checks.push(state);
        if ready {
            checks.extend(agent_checks(&format!("android:{}", d.serial)).await);
        }
    }
    checks
}

#[cfg(feature = "android")]
fn adb_server(
    addr: &str,
    configured: Option<&str>,
    version: &Result<String, phonedriver_core::error::Error>,
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

#[cfg(feature = "android")]
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
        DeviceState::Offline if d.serial.contains(':') && !d.serial.starts_with("emulator-") => {
            Check::bad(
                Status::Fail,
                name,
                "offline (Wi-Fi)",
                format!(
                    "The phone left the network or its wireless debugging port changed. Check that Wireless debugging is on and both are on the same Wi-Fi, then run `adb connect <ip>:<port>` with the address on the Wireless debugging screen, or `phonedriver pair` to pair again with a QR code. If it keeps going offline, run `adb disconnect {}` first.",
                    d.serial
                ),
            )
        }
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

/// A probe must not fail because the user's own CLI or MCP session holds the device: it reads, it does not take the lease.
#[cfg(feature = "android")]
fn probe_params(id: &str) -> serde_json::Value {
    json!({"device": id, "share": true})
}

fn probe_snapshot(id: &str) -> serde_json::Value {
    json!({"device": id, "no_settle": true, "share": true})
}

#[cfg(feature = "android")]
async fn agent_checks(id: &str) -> Vec<Check> {
    let mut client = match Client::connect("doctor".into()).await {
        Ok(c) => c,
        Err(e) => return vec![cannot_reach_daemon(format!("agent {id}"), &e)],
    };
    let started = Instant::now();
    let mut params = probe_params(id);
    crate::commands::put_display(
        &mut params,
        crate::commands::display_mode(&std::env::var("PHONEDRIVER_DISPLAY").unwrap_or_default()),
    );
    let reply = client.call("health", params).await;
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
            if let Some(display) = r["hidden_display"].as_u64() {
                checks.push(Check::ok(
                    format!("display {id}"),
                    format!(
                        "hidden mode: working on virtual display {display}, not the phone's screen"
                    ),
                ));
            }
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
                .filter(|h| !h.contains("phonedriver doctor"))
                .unwrap_or_else(|| {
                    "The daemon log is `daemon.log` in the phonedriver home.".into()
                }),
        )],
    }
}

fn cannot_reach_daemon(name: String, e: &Failure) -> Check {
    Check::bad(
        Status::Fail,
        name,
        format!("cannot reach the daemon: {}", e.message()),
        "Run `phonedriver daemon` in a terminal and read what it prints.",
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
    let reply = client.call("snapshot", probe_snapshot(id)).await;
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
                "The runner log is in the phonedriver home under runner/<udid>.log.".into()
            }),
        ),
    }
}

/// Each attached iOS device: paired, Developer Mode, developer disk image, tier and services, and the runner with the
/// expiry of the profile that signed the local build.
pub async fn physical(home: &Path, only: Option<&str>) -> Vec<Check> {
    use phonedriver_core::error::{Error, UserAction};
    let Ok(attached) = crate::ios_wifi::attached(home).await else {
        return Vec::new();
    };
    let mut checks = Vec::new();
    if attached
        .iter()
        .any(|d| wants(only, &format!("ios:{}", d.udid)))
    {
        checks.push(match phonedriver_ios::team::signing_team() {
            Ok(team) => Check::ok("ios signing team", team),
            Err(e) => Check::bad(Status::Fail, "ios signing team", e.to_string(), e.hint()),
        });
    }
    for device in attached {
        let name = format!("ios:{}", device.udid);
        if !wants(only, &name) {
            continue;
        }
        let found = match phonedriver_ios::discovery::describe(&device).await {
            Ok(f) => f,
            Err(Error::NeedsUser {
                action: UserAction::TrustComputer,
            }) => {
                checks.push(Check::bad(
                    Status::Fail,
                    format!("{name} paired"),
                    "this Mac is not trusted by the device",
                    "Unlock the device and tap Trust when it asks, then run `phonedriver ios setup`.",
                ));
                continue;
            }
            Err(e) => {
                checks.push(Check::bad(
                    Status::Fail,
                    format!("{name} reachable"),
                    format!("{e:?}"),
                    "Unlock the device. If it is on Wi-Fi, put it on the same network as this computer or plug it in; `phonedriver ios setup` shows what is missing.",
                ));
                continue;
            }
        };
        checks.push(Check::ok(
            format!("{name} paired"),
            format!(
                "{} ({}) iOS {}, tier {}, over {}",
                found.name,
                found.model,
                found.os,
                found.tier.name(),
                match device.link {
                    phonedriver_ios::transport::Link::Usb { .. } => "USB",
                    phonedriver_ios::transport::Link::Network { .. } => "Wi-Fi",
                }
            ),
        ));
        match phonedriver_ios::setup::status(&device).await {
            Ok(s) => {
                checks.push(if s.developer_mode {
                    Check::ok(format!("{name} developer mode"), "on")
                } else {
                    Check::bad(
                        Status::Fail,
                        format!("{name} developer mode"),
                        "off",
                        "Run `phonedriver ios setup`, then turn on Settings > Privacy & Security > Developer Mode and restart the device.",
                    )
                });
                checks.push(if s.image_mounted {
                    Check::ok(format!("{name} developer disk image"), "mounted")
                } else {
                    Check::bad(
                        Status::Fail,
                        format!("{name} developer disk image"),
                        "not mounted",
                        "Run `phonedriver ios setup`; it mounts the image with Xcode's tool.",
                    )
                });
            }
            Err(e) => checks.push(Check::bad(
                Status::Fail,
                format!("{name} developer services"),
                format!("{e:?}"),
                "Run `phonedriver ios setup`.",
            )),
        }
        checks.push(runner_expiry(&name, home));
    }
    checks
}

/// The expiry of the provisioning profile inside the newest device build of the runner under `home`.
fn runner_expiry(name: &str, home: &Path) -> Check {
    let label = format!("{name} runner");
    let root = home.join("runner");
    let profile = std::fs::read_dir(&root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| {
            e.path()
                .join("dd/Build/Products/Debug-iphoneos/PhonedriverRunner-Runner.app/embedded.mobileprovision")
        })
        .find(|p| p.is_file());
    let Some(profile) = profile else {
        return Check::bad(
            Status::Warn,
            label,
            "not built for devices yet",
            "Run any phonedriver command on the device; the first one builds and signs the runner. `phonedriver ios setup` checks the signing team, pairing and Developer Mode first.",
        );
    };
    let decoded = std::process::Command::new("security")
        .args(["cms", "-D", "-i"])
        .arg(&profile)
        .output();
    let expiry = decoded.ok().and_then(|o| {
        let text = String::from_utf8_lossy(&o.stdout).into_owned();
        let after = text.split("<key>ExpirationDate</key>").nth(1)?;
        let date = after.split("<date>").nth(1)?.split("</date>").next()?;
        Some(date.to_string())
    });
    match expiry {
        Some(date) => Check::ok(label, format!("signed, profile expires {date}")),
        None => Check::bad(
            Status::Warn,
            label,
            "the profile's expiry cannot be read",
            "Run `security cms -D -i <runner>.app/embedded.mobileprovision` and read ExpirationDate.",
        ),
    }
}

/// `phonedriver ios setup`: for each attached device, check pairing, Developer Mode and the developer disk image, and do
/// what needs no one at the device. Prints `ok` for a step already done.
/// The first command on a device builds, installs and starts the runner. `ios setup` does it through the daemon, in
/// the session the next command line uses, and keeps the device: after a `release` the daemon drops its connection
/// and the next command spent 3.2 s connecting again (iPhone 13 Pro Max, 2026-11-13).
async fn start_runner(id: &str) -> Result<(), Failure> {
    let session = std::env::var("PHONEDRIVER_SESSION").unwrap_or_else(|_| "default".into());
    let mut client = Client::connect(session).await?;
    client
        .call("snapshot", json!({"device": id, "no_settle": true}))
        .await
        .map(|_| ())
}

pub fn ios_setup(home: &Path) -> bool {
    use phonedriver_core::error::{Error, UserAction};
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        eprintln!("error: cannot start the async runtime");
        return false;
    };
    rt.block_on(async {
        let mut all_ok = match phonedriver_ios::team::signing_team() {
            Ok(team) => {
                println!("ok    signing team {team}");
                true
            }
            Err(e) => {
                println!("need  signing team: {e} {}", e.hint());
                false
            }
        };
        let team_ok = all_ok;
        let attached = match crate::ios_wifi::attached(home).await {
            Ok(w) if !w.is_empty() => w,
            Ok(_) => {
                println!("need  a device: plug in an iPhone or iPad, unlock it and tap Trust, then run this again");
                return false;
            }
            Err(e) => {
                println!("{e}");
                return false;
            }
        };
        let remembered = phonedriver_ios::lan::remembered(home);
        for device in attached {
            let name = device.udid.clone();
            let cabled = matches!(device.link, phonedriver_ios::transport::Link::Usb { .. });
            let status = match phonedriver_ios::setup::status(&device).await {
                Ok(s) => {
                    println!("ok    {name} paired");
                    s
                }
                Err(Error::NeedsUser {
                    action: UserAction::TrustComputer,
                }) => {
                    println!("need  {name} paired: unlock the device and tap Trust, then run this again");
                    all_ok = false;
                    continue;
                }
                Err(e) => {
                    println!("fail  {name}: {e:?}");
                    all_ok = false;
                    continue;
                }
            };
            if status.developer_mode {
                println!("ok    {name} developer mode");
            } else {
                match phonedriver_ios::setup::reveal_developer_mode(&device).await {
                    Ok(()) => println!("need  {name} developer mode: turn on Settings > Privacy & Security > Developer Mode, restart the device, confirm"),
                    Err(e) => println!("fail  {name} developer mode: {e:?}"),
                }
                all_ok = false;
            }
            if status.image_mounted {
                println!("ok    {name} developer disk image");
            } else {
                println!("      {name} developer disk image: mounting with Xcode's devicectl (talks to Apple's signing server)");
                match phonedriver_ios::setup::mount_image(&name).await {
                    Ok(()) => println!("ok    {name} developer disk image"),
                    Err(e) => {
                        println!("fail  {name} developer disk image: {e:?}");
                        all_ok = false;
                        continue;
                    }
                }
            }
            if !status.developer_mode || !team_ok {
                continue;
            }
            println!("      {name} runner: building, installing and starting it (a few minutes the first time)");
            match start_runner(&format!("ios:{name}")).await {
                Ok(()) => println!("ok    {name} runner"),
                Err(e) => {
                    let word = if e.exit_code() == 3 { "need" } else { "fail" };
                    println!(
                        "{word}  {name} runner: {} {}",
                        e.message(),
                        e.hint().unwrap_or_default()
                    );
                    all_ok = false;
                    continue;
                }
            }
            if cabled && !remembered.contains(&name) {
                println!("tip   {name}: `phonedriver ios wifi on` lets you unplug the cable");
            }
        }
        if all_ok {
            println!("ready. `phonedriver snapshot` shows the screen.");
        }
        all_ok
    })
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
    fn a_probe_shares_a_device_another_session_holds() {
        #[cfg(feature = "android")]
        assert_eq!(probe_params("android:a")["share"], json!(true));
        assert_eq!(probe_snapshot("sim:a")["share"], json!(true));
    }

    #[test]
    #[cfg(feature = "android")]
    fn an_offline_device_over_tcp_gets_the_wifi_hint_and_a_cable_one_does_not() {
        let dev = |serial: &str| AdbDevice {
            serial: serial.into(),
            state: DeviceState::Offline,
            model: None,
            transport_id: None,
        };
        let wifi = device_state(&dev("192.168.1.20:37041"));
        assert_eq!(wifi.detail, "offline (Wi-Fi)");
        assert!(wifi.fix.as_deref().unwrap().contains("adb connect"));
        assert!(wifi.fix.as_deref().unwrap().contains("phonedriver pair"));
        let cable = device_state(&dev("R5CT1234ABC"));
        assert!(cable.fix.as_deref().unwrap().contains("cable"));
        let emulator = device_state(&dev("emulator-5554"));
        assert!(emulator.fix.as_deref().unwrap().contains("cable"));
    }

    #[test]
    #[cfg(feature = "android")]
    fn without_a_configured_adb_server_a_missing_one_is_a_warning() {
        let down = Err(phonedriver_core::error::Error::Transport {
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
