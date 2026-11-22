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

/// Whether doctor probes a device (starts or replaces its agent, or its runner) or only lists it.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Probe,
    Skip(String),
}

/// What doctor may touch (D101): the devices the daemon hides are not shown, a device another live session holds is not
/// probed, and a physical phone or tablet is probed only with `--all-devices` or when `--device` names it.
#[derive(Debug, Default)]
pub struct Scope {
    only: Option<String>,
    all: bool,
    hidden: Vec<String>,
    hidden_count: usize,
    held: std::collections::HashMap<String, String>,
    daemon: Option<String>,
}

fn physical_id(id: &str) -> bool {
    id.starts_with("ios:") || (id.starts_with("android:") && !id.starts_with("android:emulator-"))
}

impl Scope {
    pub fn new(only: Option<&str>, all: bool) -> Scope {
        Scope {
            only: only.map(str::to_string),
            all,
            ..Scope::default()
        }
    }

    pub fn holding(mut self, id: &str, session: &str) -> Scope {
        self.held.insert(id.into(), session.into());
        self
    }

    pub fn hiding(mut self, id: &str) -> Scope {
        self.hidden.push(id.into());
        self.hidden_count += 1;
        self
    }

    async fn ask(only: Option<&str>, all: bool) -> Scope {
        let mut scope = Scope::new(only, all);
        let asked = async {
            let mut client = Client::connect("doctor".into()).await?;
            let hidden = client.call("hidden", json!({})).await?;
            let devices = match only {
                Some(_) => json!([]),
                None => client.call("devices", json!({})).await?,
            };
            Ok::<_, Failure>((hidden, devices))
        }
        .await;
        match asked {
            Ok((hidden, devices)) => {
                scope.hidden = strings(&hidden["ids"]);
                scope.hidden_count = hidden["count"].as_u64().unwrap_or(0) as usize;
                for d in devices.as_array().into_iter().flatten() {
                    if d["lease_live"] == true
                        && let (Some(id), Some(by)) = (d["id"].as_str(), d["leased_by"].as_str())
                    {
                        scope.held.insert(id.into(), by.into());
                    }
                }
            }
            Err(e) => scope.daemon = Some(e.message()),
        }
        scope
    }

    /// Devices that are not ready (unauthorized, offline) cannot be read for a serial, so the daemon judges them by id.
    #[cfg(feature = "android")]
    async fn also_ask_about(&mut self, ids: &[String]) {
        if ids.is_empty() || self.daemon.is_some() {
            return;
        }
        let Ok(mut client) = Client::connect("doctor".into()).await else {
            return;
        };
        let Ok(reply) = client.call("hidden", json!({"ids": ids})).await else {
            return;
        };
        for id in strings(&reply["ids"]) {
            if !self.hidden.contains(&id) {
                self.hidden.push(id);
                self.hidden_count += 1;
            }
        }
    }

    pub fn is_hidden(&self, id: &str) -> bool {
        self.hidden.iter().any(|h| h == id)
    }

    /// A device named with `--device` is probed, held or not, unless the daemon could not say what the device list
    /// hides: then nothing is, because the list may hide it.
    pub fn verdict(&self, id: &str) -> Verdict {
        if let Some(why) = &self.daemon {
            return Verdict::Skip(format!(
                "the daemon did not answer ({why}), so doctor cannot tell whether the device list hides it or a session holds it"
            ));
        }
        if self.only.is_some() {
            return Verdict::Probe;
        }
        if let Some(session) = self.held.get(id) {
            return Verdict::Skip(format!(
                "session {session} holds it, and probing would take its agent"
            ));
        }
        if physical_id(id) && !self.all {
            return Verdict::Skip(
                "a physical device, which doctor leaves alone unless asked".into(),
            );
        }
        Verdict::Probe
    }
}

fn strings(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(str::to_string))
        .collect()
}

fn not_probed(label: &str, id: &str, why: &str) -> Check {
    Check::bad(
        Status::Warn,
        format!("{label} {id}"),
        format!("not probed: {why}"),
        format!("Run `phonedriver doctor --device {id}` to probe it."),
    )
}

/// Runs every check and prints them; with `only`, probes just that device, and without it, probes the emulators and
/// Simulators that no live session holds, plus every device with `all_devices`. Fails when any check fails.
pub fn run(home: &Path, only: Option<&str>, all_devices: bool) -> bool {
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        eprintln!("error: cannot start the async runtime");
        return false;
    };
    let checks = rt.block_on(async {
        let scope = &mut Scope::ask(only, all_devices).await;
        #[cfg(feature = "android")]
        let mut checks = android(&Adb::from_env(), only, scope).await;
        #[cfg(not(feature = "android"))]
        let mut checks = Vec::new();
        checks.extend(ios(home, only, scope).await);
        checks.extend(physical(home, only, scope).await);
        checks.extend(socket_permissions(&socket::socket_path()));
        if let Some(why) = &scope.daemon {
            checks.push(Check::bad(
                Status::Warn,
                "daemon",
                format!("did not answer: {why}"),
                "Run `phonedriver daemon` in a terminal and read what it prints; no device was probed.",
            ));
        }
        if scope.hidden_count > 0 {
            checks.push(Check::ok(
                "device list",
                format!(
                    "{} {} hidden by the device list and not probed",
                    scope.hidden_count,
                    if scope.hidden_count == 1 { "device" } else { "devices" }
                ),
            ));
        }
        checks
    });
    print!("{}", render(&checks));
    !checks.iter().any(|c| c.status == Status::Fail)
}

/// The Simulator tier: Xcode, booted simulators, the runner build, and each simulator's runner answering
/// through the daemon. A runner serves one connection and a new one cancels the old, so probing its port directly
/// would cut off a daemon that holds it; the daemon connects (which says `hello`) and takes one snapshot instead.
pub async fn ios(home: &Path, only: Option<&str>, scope: &Scope) -> Vec<Check> {
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
    if !xcode_ok {
        return checks;
    }
    let sims = match phonedriver_ios::simulator::Simulator::list().await {
        Ok(sims) if sims.is_empty() && only.is_none() => {
            checks.push(Check::bad(
                Status::Warn,
                "simulators",
                "none booted",
                "`phonedriver boot` lists the ones that can be started; `phonedriver boot <name>` starts one.",
            ));
            Vec::new()
        }
        Ok(mut sims) => {
            sims.retain(|s| {
                let id = format!("sim:{}", s.udid);
                wants(only, &id) && !scope.is_hidden(&id)
            });
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
    let (probed, left): (Vec<_>, Vec<_>) = sims
        .iter()
        .partition(|s| scope.verdict(&format!("sim:{}", s.udid)) == Verdict::Probe);
    if probed.is_empty() {
        checks.extend(left_alone(
            "runner",
            left.iter().map(|s| format!("sim:{}", s.udid)),
            scope,
        ));
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
    for s in probed {
        checks.push(runner_answers(&format!("sim:{}", s.udid)).await);
    }
    checks.extend(left_alone(
        "runner",
        left.iter().map(|s| format!("sim:{}", s.udid)),
        scope,
    ));
    checks
}

fn left_alone(label: &str, ids: impl Iterator<Item = String>, scope: &Scope) -> Vec<Check> {
    ids.filter_map(|id| match scope.verdict(&id) {
        Verdict::Skip(why) => Some(not_probed(label, &id, &why)),
        Verdict::Probe => None,
    })
    .collect()
}

/// The adb server at `Adb::from_env()`, then every device it lists. A device in the `device` state is probed through
/// the daemon, which deploys the agent and holds UiAutomation, so the probe must not start a second agent.
#[cfg(feature = "android")]
pub async fn android(adb: &Adb, only: Option<&str>, scope: &mut Scope) -> Vec<Check> {
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
    let not_ready: Vec<String> = devices
        .iter()
        .filter(|d| d.state != DeviceState::Device)
        .map(|d| format!("android:{}", d.serial))
        .collect();
    scope.also_ask_about(&not_ready).await;
    let devices: Vec<AdbDevice> = devices
        .into_iter()
        .filter(|d| !scope.is_hidden(&format!("android:{}", d.serial)))
        .collect();
    let devices = match only {
        Some(_) => devices,
        None => phonedriver_android::wireless::connections_per_phone(adb, devices).await,
    };
    if devices.is_empty() {
        checks.push(Check::bad(
            Status::Warn,
            "android devices",
            "none connected",
            "Connect a phone with USB debugging on, pair an Android 11+ phone over Wi-Fi with `phonedriver pair`, or start an emulator (`phonedriver boot` lists the ones on this machine); `adb devices` lists what adb sees.",
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
    if only.is_none() {
        let probed: Vec<String> = devices
            .iter()
            .filter(|d| d.state == DeviceState::Device)
            .map(|d| format!("android:{}", d.serial))
            .filter(|id| scope.verdict(id) == Verdict::Probe)
            .collect();
        if !probed.is_empty() {
            eprintln!(
                "starting the agent on {}; `--device <id>` checks only one",
                probed.join(", ")
            );
        }
    }
    for d in devices
        .iter()
        .filter(|d| wants(only, &format!("android:{}", d.serial)))
    {
        let state = device_state(d);
        let ready = state.status == Status::Ok;
        checks.push(state);
        if ready {
            let id = format!("android:{}", d.serial);
            match scope.verdict(&id) {
                Verdict::Probe => checks.extend(agent_checks(&id).await),
                Verdict::Skip(why) => checks.push(not_probed("agent", &id, &why)),
            }
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
        Ok(r) => answered(id, &r, started.elapsed().as_secs_f64()),
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
            fix_for(
                &e,
                "The daemon log is `daemon.log` in the phonedriver home.",
            ),
        )],
    }
}

#[cfg(feature = "android")]
/// The checks of an agent that answered the health call `r` after `took` seconds.
fn answered(id: &str, r: &serde_json::Value, took: f64) -> Vec<Check> {
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
            "connected, the screen tree reads; while phonedriver holds it, `uiautomator dump` and other UiAutomation clients are killed (`phonedriver daemon stop` frees it)",
        ),
    ];
    if let Some(display) = r["hidden_display"].as_u64() {
        checks.push(Check::ok(
            format!("display {id}"),
            format!("hidden mode: working on virtual display {display}, not the phone's screen"),
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

fn fix_for(e: &Failure, otherwise: &str) -> String {
    own_fix(e.hint(), otherwise)
}

/// The error's own hint, unless that hint is to run `doctor`. Seen on a loaded Mac: a simulator's probe timed out
/// and doctor printed ``fix: Run `phonedriver doctor` to check the connection``.
fn own_fix(hint: Option<String>, otherwise: &str) -> String {
    hint.filter(|h| !h.contains("phonedriver doctor"))
        .unwrap_or_else(|| otherwise.into())
}

const REACH_FIX: &str = "Unlock the device. If it is on Wi-Fi, keep it on the same network as this computer, or plug it in.";

/// An iPhone or iPad check that failed with `e`, said after `what`.
fn ios_failed(name: String, what: &str, e: &phonedriver_core::error::Error) -> Check {
    Check::bad(
        Status::Fail,
        name,
        format!("{what}{e}"),
        own_fix(Some(e.hint()), REACH_FIX),
    )
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
            fix_for(
                &e,
                "The runner log is in the phonedriver home under runner/<udid>.log.",
            ),
        ),
    }
}

/// Each attached iOS device: paired, Developer Mode, developer disk image, tier and services, and the runner with the
/// expiry of the profile that signed the local build.
pub async fn physical(home: &Path, only: Option<&str>, scope: &Scope) -> Vec<Check> {
    use phonedriver_core::error::{Error, UserAction};
    use phonedriver_ios::setup::XCODE;
    let attached = match crate::ios_wifi::attached(home).await {
        Ok(attached) => attached,
        Err(e) if !XCODE && only.is_none_or(|o| o.starts_with("ios:")) => {
            return vec![Check::bad(Status::Warn, "ios usbmuxd", e, USBMUXD_FIX)];
        }
        Err(_) => return Vec::new(),
    };
    let attached: Vec<_> = attached
        .into_iter()
        .filter(|d| !scope.is_hidden(&format!("ios:{}", d.udid)))
        .collect();
    let mut checks = Vec::new();
    if !XCODE && only.is_none_or(|o| o.starts_with("ios:")) {
        checks.push(Check::ok(
            "ios usbmuxd",
            format!("answers; {} iPhone or iPad attached", attached.len()),
        ));
    }
    if XCODE
        && attached.iter().any(|d| {
            let id = format!("ios:{}", d.udid);
            wants(only, &id) && scope.verdict(&id) == Verdict::Probe
        })
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
        if let Verdict::Skip(why) = scope.verdict(&name) {
            checks.push(not_probed("device", &name, &why));
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
                checks.push(ios_failed(format!("{name} reachable"), "", &e));
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
        match phonedriver_ios::setup::status(&device, home).await {
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
                        if XCODE {
                            "Run `phonedriver ios setup`; it mounts the image with Xcode's tool."
                        } else {
                            "Run `phonedriver ios setup`; it mounts the image (the first time add `--download-ddi`, or `--ddi <folder>` with Xcode's files). A restart of the device unmounts it."
                        },
                    )
                });
            }
            Err(e) => checks.push(ios_failed(format!("{name} developer services"), "", &e)),
        }
        checks.push(if XCODE {
            runner_expiry(&name, home)
        } else {
            runner_installed(&name, &device, home).await
        });
    }
    checks
}

const USBMUXD_FIX: &str = "Install Apple Devices from the Microsoft Store (or iTunes), which runs the Apple Mobile Device Service that reaches an iPhone or iPad on the cable, then plug the device in, unlock it and tap Trust.";

fn install_runner_fix() -> String {
    format!(
        "Download {} and install it on the device with Sideloadly (sideloadly.io), signed with your Apple ID; Sideloadly signs it again before the 7 days of a free Apple ID run out. Then turn on Developer Mode if the device asks.",
        phonedriver_ios::runner::RUNNER_IPA
    )
}

/// Off a Mac: whether the runner the person installs is on the device.
async fn runner_installed(
    name: &str,
    device: &phonedriver_ios::transport::Attached,
    home: &Path,
) -> Check {
    let label = format!("{name} runner");
    match find_runner(device, home).await {
        Ok(Some(bundle)) => Check::ok(label, format!("installed as {bundle}")),
        Ok(None) => Check::bad(Status::Fail, label, "not installed", install_runner_fix()),
        Err(e) => ios_failed(label, "cannot list the device's apps: ", &e),
    }
}

/// The installed runner's bundle id, read where the device's services answer: lockdown on the cable, the remote
/// pairing's tunnel on Wi-Fi off a Mac.
async fn find_runner(
    device: &phonedriver_ios::transport::Attached,
    home: &Path,
) -> Result<Option<String>, phonedriver_core::error::Error> {
    let mut services = phonedriver_ios::transport::Services::open(device, home).await?;
    phonedriver_ios::runner::find_installed(&mut services)
        .await
        .map_err(|e| phonedriver_ios::transport::classify(&format!("ios:{}", device.udid), &e))
}

/// The expiry of the signing profile inside this version's device build of the runner under `home`.
fn runner_expiry(name: &str, home: &Path) -> Check {
    use phonedriver_ios::runner::{DEVICE_PROFILE, build_hash, expired, profile_expiry};
    let label = format!("{name} runner");
    let profile = home
        .join("runner")
        .join(format!("{}-device", build_hash()))
        .join("dd/Build/Products")
        .join(DEVICE_PROFILE);
    if !profile.is_file() {
        return Check::bad(
            Status::Warn,
            label,
            "not built for devices yet",
            "Run any phonedriver command on the device; the first one builds and signs the runner. `phonedriver ios setup` checks the signing team, pairing and Developer Mode first.",
        );
    }
    match profile_expiry(&profile) {
        Some(date) if expired(&date, std::time::SystemTime::now()) => Check::bad(
            Status::Warn,
            label,
            format!("the signing profile ran out on {date}"),
            "Nothing to do: the next phonedriver command on the device signs the runner again and installs it, which takes about a minute. A free Apple ID's profile lasts seven days.",
        ),
        Some(date) => Check::ok(label, format!("signed, profile expires {date}")),
        None => Check::bad(
            Status::Warn,
            label,
            "the profile's expiry cannot be read",
            format!("Read ExpirationDate in {}.", profile.display()),
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

pub fn ios_setup(home: &Path, download_ddi: bool, ddi: Option<&Path>) -> bool {
    use phonedriver_core::error::{Error, UserAction};
    use phonedriver_ios::setup::XCODE;
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        eprintln!("error: cannot start the async runtime");
        return false;
    };
    rt.block_on(async {
        let mut all_ok = !XCODE
            || match phonedriver_ios::team::signing_team() {
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
            Err(e) if !XCODE => {
                println!("need  usbmuxd: {e}. {USBMUXD_FIX}");
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
            let status = match phonedriver_ios::setup::status(&device, home).await {
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
                Err(Error::NeedsUser { action }) => {
                    println!("need  {name}: {} {}", action.message(), action.hint());
                    all_ok = false;
                    continue;
                }
                Err(e) => {
                    println!("fail  {name}: {e} {}", own_fix(Some(e.hint()), REACH_FIX));
                    all_ok = false;
                    continue;
                }
            };
            if status.developer_mode {
                println!("ok    {name} developer mode");
            } else {
                match phonedriver_ios::setup::reveal_developer_mode(&device, home).await {
                    Ok(()) => println!("need  {name} developer mode: turn on Settings > Privacy & Security > Developer Mode, restart the device, confirm"),
                    Err(e) => println!("fail  {name} developer mode: {e}"),
                }
                all_ok = false;
            }
            if status.image_mounted {
                println!("ok    {name} developer disk image");
            } else if XCODE && ddi.is_none() {
                println!("      {name} developer disk image: mounting with Xcode's devicectl (talks to Apple's signing server)");
                match phonedriver_ios::setup::mount_image(&name).await {
                    Ok(()) => println!("ok    {name} developer disk image"),
                    Err(e) => {
                        println!("fail  {name} developer disk image: {e}");
                        all_ok = false;
                        continue;
                    }
                }
            } else {
                let files = match (ddi, phonedriver_ios::ddi::cached(home)) {
                    (Some(dir), _) => phonedriver_ios::ddi::in_dir(dir),
                    (None, Some(files)) => Ok(files),
                    (None, None) if download_ddi => {
                        println!(
                            "      developer disk image: downloading it from {}",
                            phonedriver_ios::ddi::source()
                        );
                        phonedriver_ios::ddi::download(home).await
                    }
                    (None, None) => {
                        println!(
                            "need  {name} developer disk image: without Xcode, phonedriver needs Xcode's image files. Run `phonedriver ios setup --download-ddi` to download them once from {} (checked against pinned hashes), or `phonedriver ios setup --ddi <folder>` with the three files from a Mac's Xcode",
                            phonedriver_ios::ddi::source()
                        );
                        all_ok = false;
                        continue;
                    }
                };
                let mounted = match files {
                    Ok(_) if phonedriver_ios::setup::mount_blocked(&status).is_some() => {
                        println!("need  {name} developer disk image: it mounts once Developer Mode is on. Run this again after the restart");
                        all_ok = false;
                        continue;
                    }
                    Ok(files) => {
                        println!("      {name} developer disk image: mounting (asks Apple's signing server for this device's ticket)");
                        phonedriver_ios::setup::mount_files(&device, home, &files).await
                    }
                    Err(e) => Err(e),
                };
                match mounted {
                    Ok(()) => println!("ok    {name} developer disk image"),
                    Err(e) => {
                        println!("fail  {name} developer disk image: {} {}", e, e.hint());
                        all_ok = false;
                        continue;
                    }
                }
            }
            if !status.developer_mode || !team_ok {
                continue;
            }
            if XCODE {
                println!("      {name} runner: building, installing and starting it (a few minutes the first time)");
            } else {
                match find_runner(&device, home).await {
                    Ok(Some(bundle)) => {
                        println!("      {name} runner: starting {bundle}")
                    }
                    Ok(None) => {
                        println!("need  {name} runner: not installed. {}", install_runner_fix());
                        all_ok = false;
                        continue;
                    }
                    Err(e) => {
                        println!(
                            "fail  {name} runner: cannot list the device's apps: {e} {}",
                            own_fix(Some(e.hint()), REACH_FIX)
                        );
                        all_ok = false;
                        continue;
                    }
                }
            }
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

    /// Issue #43: QA scripts that call `adb shell uiautomator dump` broke (exit 137) while the agent ran, and nothing
    /// said why or how to get it back.
    #[cfg(feature = "android")]
    #[test]
    fn a_healthy_agent_says_it_holds_uiautomation_and_how_to_free_it() {
        let checks = answered(
            "android:emulator-5554",
            &json!({"model": "Pixel", "os": "Android 16", "frames_ms": 40}),
            1.2,
        );
        let ui = checks
            .iter()
            .find(|c| c.name == "uiautomation android:emulator-5554")
            .unwrap();
        assert!(ui.detail.contains("`uiautomator dump`"), "{ui:?}");
        assert!(
            ui.detail.contains("`phonedriver daemon stop` frees it"),
            "{ui:?}"
        );
    }

    #[test]
    fn a_runner_whose_profile_has_run_out_says_the_next_command_signs_it_again() {
        let home = tempfile::tempdir().unwrap();
        let profile = home
            .path()
            .join("runner")
            .join(format!("{}-device", phonedriver_ios::runner::build_hash()))
            .join("dd/Build/Products")
            .join(phonedriver_ios::runner::DEVICE_PROFILE);
        std::fs::create_dir_all(profile.parent().unwrap()).unwrap();
        let write = |date: &str| {
            let text = format!("<key>ExpirationDate</key><date>{date}</date>");
            std::fs::write(&profile, text).unwrap();
        };
        write("2026-11-09T10:00:00Z");
        let check = runner_expiry("iPhone", home.path());
        assert_eq!(check.status, Status::Warn, "{check:?}");
        assert_eq!(
            check.detail,
            "the signing profile ran out on 2026-11-09T10:00:00Z"
        );
        assert!(check.fix.unwrap().contains("signs the runner again"));
        write("2999-01-01T00:00:00Z");
        let check = runner_expiry("iPhone", home.path());
        assert_eq!(check.status, Status::Ok, "{check:?}");
        assert_eq!(check.detail, "signed, profile expires 2999-01-01T00:00:00Z");
    }

    fn skipped(v: Verdict) -> String {
        match v {
            Verdict::Skip(why) => why,
            Verdict::Probe => panic!("expected the device to be left alone"),
        }
    }

    #[test]
    fn a_physical_phone_is_listed_and_not_probed_unless_asked() {
        let default = Scope::new(None, false);
        for id in [
            "android:R5CT123",
            "android:192.168.1.20:5555",
            "ios:00008110-ABC",
        ] {
            assert!(skipped(default.verdict(id)).contains("physical"), "{id}");
        }
        let all = Scope::new(None, true);
        assert_eq!(all.verdict("android:R5CT123"), Verdict::Probe);
        let named = Scope::new(Some("android:R5CT123"), false);
        assert_eq!(named.verdict("android:R5CT123"), Verdict::Probe);
        let line = not_probed("agent", "android:R5CT123", "a physical device");
        assert_eq!(line.status, Status::Warn);
        assert!(line.detail.starts_with("not probed: "));
        assert_eq!(
            line.fix.as_deref(),
            Some("Run `phonedriver doctor --device android:R5CT123` to probe it.")
        );
    }

    #[test]
    fn an_emulator_or_a_simulator_nobody_holds_is_probed() {
        let scope = Scope::new(None, false);
        assert_eq!(scope.verdict("android:emulator-5554"), Verdict::Probe);
        assert_eq!(scope.verdict("sim:ABCD"), Verdict::Probe);
    }

    #[test]
    fn a_device_another_session_holds_is_left_alone_even_with_all_devices() {
        let held = |all| {
            Scope::new(None, all)
                .holding("android:emulator-5554", "4242")
                .holding("android:R5CT123", "4242")
        };
        for all in [false, true] {
            let scope = held(all);
            assert!(skipped(scope.verdict("android:emulator-5554")).contains("4242"));
            assert!(skipped(scope.verdict("android:R5CT123")).contains("4242"));
            assert_eq!(scope.verdict("android:emulator-5558"), Verdict::Probe);
        }
        let named = Scope::new(Some("android:emulator-5554"), false)
            .holding("android:emulator-5554", "4242");
        assert_eq!(named.verdict("android:emulator-5554"), Verdict::Probe);
    }

    #[test]
    fn a_device_the_list_hides_is_not_shown() {
        let scope = Scope::new(None, true).hiding("android:R5CT123");
        assert!(scope.is_hidden("android:R5CT123"));
        assert!(!scope.is_hidden("android:emulator-5554"));
    }

    #[test]
    fn without_the_daemon_nothing_is_probed_unless_named() {
        let scope = Scope {
            daemon: Some("no daemon".into()),
            ..Scope::default()
        };
        assert!(skipped(scope.verdict("android:emulator-5554")).contains("no daemon"));
    }

    #[test]
    fn a_named_device_is_not_probed_when_the_device_list_could_not_be_read() {
        let scope = Scope {
            only: Some("ios:00008110-ABC".into()),
            daemon: Some("the device list file cannot be used".into()),
            ..Scope::default()
        };
        assert!(skipped(scope.verdict("ios:00008110-ABC")).contains("device list file"));
    }

    #[test]
    fn a_fix_never_sends_the_reader_back_to_doctor() {
        let failed = |hint: &str| {
            Failure::Rpc(crate::daemon::rpc::RpcError {
                code: -32000,
                message: "connection error".into(),
                data: json!({"hint": hint}),
            })
        };
        assert_eq!(
            fix_for(
                &failed("Run `phonedriver doctor` to check the connection."),
                "The runner log is there."
            ),
            "The runner log is there."
        );
        assert_eq!(
            fix_for(&failed("Unlock the phone."), "The runner log is there."),
            "Unlock the phone."
        );
    }

    #[test]
    fn a_locked_device_on_wifi_is_told_in_words_with_a_fix_that_names_neither_setup_nor_doctor() {
        use phonedriver_core::error::Error;
        let locked = Error::Transport {
            detail: "ios:A answers on Wi-Fi but does not announce its remote pairing service; unlock it and keep it on the same network".into(),
        };
        for check in [
            ios_failed("ios:A developer services".into(), "", &locked),
            ios_failed(
                "ios:A runner".into(),
                "cannot list the device's apps: ",
                &locked,
            ),
        ] {
            assert!(
                check.detail.starts_with(
                    "cannot list the device's apps: connection error: ios:A answers on Wi-Fi"
                ) || check
                    .detail
                    .starts_with("connection error: ios:A answers on Wi-Fi"),
                "{check:?}"
            );
            assert!(!check.detail.contains("Transport {"), "{check:?}");
            assert!(!check.detail.contains("phonedriver doctor"), "{check:?}");
            let fix = check.fix.unwrap();
            assert!(fix.starts_with("Unlock the device"), "{fix}");
            assert!(!fix.contains("ios setup"), "{fix}");
            assert!(!fix.contains("phonedriver doctor"), "{fix}");
        }
    }

    #[test]
    fn a_failure_that_needs_the_person_keeps_its_own_message_and_fix() {
        use phonedriver_core::error::{Error, UserAction};
        let check = ios_failed(
            "ios:A developer services".into(),
            "",
            &Error::NeedsUser {
                action: UserAction::PairForWifi,
            },
        );
        assert_eq!(check.detail, UserAction::PairForWifi.message());
        assert_eq!(check.fix.unwrap(), UserAction::PairForWifi.hint());
    }

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
