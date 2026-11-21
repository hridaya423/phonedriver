//! Booted iOS simulators through `xcrun simctl`.

use phonedriver_core::error::{Error, Op};
use phonedriver_core::network::{LogWait, LogWaited};
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Simulator {
    pub udid: String,
    pub name: String,
    /// For example `iOS 26.5`.
    pub os: String,
}

impl Simulator {
    /// The booted simulators, in `simctl`'s order.
    pub async fn list() -> Result<Vec<Simulator>> {
        let out = simctl(&["list", "devices", "booted", "-j"]).await?;
        parse_list(&out)
    }
}

impl Simulator {
    /// The iOS simulators that are shut down and can boot, newest system first.
    pub async fn off() -> Result<Vec<Simulator>> {
        parse_off(&simctl(&["list", "devices", "available", "-j"]).await?)
    }

    /// Boots the simulator and returns when it has finished booting.
    pub async fn boot(udid: &str) -> Result<()> {
        simctl(&["bootstatus", udid, "-b"]).await.map(|_| ())
    }
}

/// Parses `xcrun simctl list devices -j` and keeps the booted devices.
pub fn parse_list(json: &str) -> Result<Vec<Simulator>> {
    parse(json, "Booted")
}

/// Parses `xcrun simctl list devices -j` and keeps the iOS devices that are shut down and available, newest
/// system first. Other systems (tvOS, watchOS, visionOS) are left out: the runner is built for iOS.
pub fn parse_off(json: &str) -> Result<Vec<Simulator>> {
    let version = |s: &Simulator| -> Vec<u32> {
        s.os.split(|c: char| !c.is_ascii_digit())
            .filter_map(|n| n.parse().ok())
            .collect()
    };
    let mut off = parse(json, "Shutdown")?;
    off.retain(|s| s.os.starts_with("iOS "));
    off.sort_by_key(|s| std::cmp::Reverse(version(s)));
    Ok(off)
}

fn parse(json: &str, state: &str) -> Result<Vec<Simulator>> {
    let v: Value = serde_json::from_str(json).map_err(|e| Error::Transport {
        detail: format!("simctl printed a device list that does not parse: {e}"),
    })?;
    let mut out = Vec::new();
    let Some(runtimes) = v["devices"].as_object() else {
        return Ok(out);
    };
    for (runtime, devices) in runtimes {
        let os = runtime_name(runtime);
        for d in devices.as_array().into_iter().flatten() {
            if d["state"] != state || d["isAvailable"] == false {
                continue;
            }
            let (Some(udid), Some(name)) = (d["udid"].as_str(), d["name"].as_str()) else {
                continue;
            };
            out.push(Simulator {
                udid: udid.to_string(),
                name: name.to_string(),
                os: os.clone(),
            });
        }
    }
    Ok(out)
}

fn runtime_name(id: &str) -> String {
    let tail = id.rsplit('.').next().unwrap_or(id);
    match tail.split_once('-') {
        Some((platform, version)) => format!("{platform} {}", version.replace('-', ".")),
        None => tail.to_string(),
    }
}

/// Runs `xcrun simctl <args>` and returns its standard output.
pub async fn simctl(args: &[&str]) -> Result<String> {
    simctl_with_input(args, None).await
}

/// Runs `xcrun simctl <args>` with `input` on its standard input. Neither the input nor the output goes into errors,
/// because the input may be a secret (`pbcopy`). `pbcopy` reads its input as Mac Roman unless the locale says
/// UTF-8, and a daemon started without a locale has none.
pub async fn simctl_with_input(args: &[&str], input: Option<&str>) -> Result<String> {
    let mut cmd = Command::new("xcrun");
    cmd.arg("simctl")
        .args(args)
        .env("LC_CTYPE", "UTF-8")
        .stdin(if input.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let verb = args.first().copied().unwrap_or_default();
    let mut child = cmd.spawn().map_err(|e| Error::Transport {
        detail: format!("cannot run xcrun simctl (is Xcode installed?): {e}"),
    })?;
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin
            .write_all(text.as_bytes())
            .await
            .map_err(|e| Error::Transport {
                detail: format!("cannot write to simctl {verb}: {e}"),
            })?;
    }
    let out = child
        .wait_with_output()
        .await
        .map_err(|e| Error::Transport {
            detail: format!("simctl {verb} failed: {e}"),
        })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let first = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        return Err(Error::Transport {
            detail: format!("simctl {verb} exited with {}: {first}", out.status),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The `simctl` subcommands whose first argument is the device, which `shell` fills in on the Simulator.
const DEVICE_SUBCOMMANDS: &[&str] = &[
    "openurl",
    "ui",
    "privacy",
    "location",
    "status_bar",
    "push",
    "launch",
    "terminate",
    "install",
    "uninstall",
    "listapps",
    "appinfo",
    "get_app_container",
    "addmedia",
    "keychain",
    "pbcopy",
    "pbpaste",
    "io",
    "spawn",
];

/// The folders of a Simulator's system where a program name is looked up, as its `PATH` would.
const PROGRAM_FOLDERS: &[&str] = &["usr/bin", "bin", "usr/sbin", "sbin"];

/// What `xcrun` runs for an agent's `shell` command on the Simulator `udid` whose system is the folder `root`: `simctl
/// <sub> ...` gets the device inserted after the subcommand, and anything else runs in the Simulator as `spawn_args`
/// says. A Simulator's programs are processes of the Mac, run as the Mac's user with the Mac's files, so unless `host`
/// (the daemon was started with `PHONEDRIVER_HOST_SHELL=1`) only the `simctl` forms that act on the Simulator alone
/// are built (decision D108). The errors never quote the command, which may be a secret.
pub fn shell_args(
    udid: &str,
    command: &str,
    root: &std::path::Path,
    host: bool,
) -> std::result::Result<Vec<String>, String> {
    let words = split_words(command)?;
    if words.first().map(String::as_str) != Some("simctl") {
        if !host {
            return Err(on_the_mac(
                "A command that is not `simctl` runs a program in the Simulator",
            ));
        }
        return spawn_words(udid, command, &words, root);
    }
    let Some(sub) = words.get(1) else {
        return Err(format!(
            "simctl needs a subcommand; the device is filled in. Available: {}",
            DEVICE_SUBCOMMANDS.join(", ")
        ));
    };
    if !DEVICE_SUBCOMMANDS.contains(&sub.as_str()) {
        return Err(format!(
            "`simctl {sub}` is not offered here (it does not target this Simulator). Available: {}",
            DEVICE_SUBCOMMANDS.join(", ")
        ));
    }
    if !host && let Some(why) = mac_form(sub, &words[2..]) {
        return Err(on_the_mac(why));
    }
    let mut args = vec![sub.clone(), udid.to_string()];
    args.extend(words[2..].iter().cloned());
    Ok(args)
}

/// Why `simctl <sub> <rest>` acts on the Mac and not only on the Simulator, if it does.
fn mac_form(sub: &str, rest: &[String]) -> Option<&'static str> {
    let first = rest.first().map(String::as_str);
    match sub {
        "spawn" => Some("`simctl spawn` runs a program in the Simulator"),
        "install" => Some(
            "`simctl install` reads an app from the Mac's files; the `install` verb installs one",
        ),
        "addmedia" => Some("`simctl addmedia` reads files of the Mac"),
        "push" => {
            Some("`simctl push` reads a file of the Mac; `apps notify` sends a payload to an app")
        }
        "io" if matches!(first, Some("screenshot" | "recordVideo")) => Some(
            "`simctl io screenshot` and `recordVideo` write a file on the Mac; `screenshot` and `record` capture the screen",
        ),
        "keychain" if first != Some("reset") => {
            Some("`simctl keychain add-cert` and `add-root-cert` read a file of the Mac")
        }
        "launch"
            if rest
                .iter()
                .any(|w| w.starts_with("--stdout") || w.starts_with("--stderr")) =>
        {
            Some("`simctl launch --stdout` and `--stderr` write files on the Mac")
        }
        "openurl"
            if rest
                .iter()
                .any(|w| w.to_ascii_lowercase().starts_with("file:")) =>
        {
            Some("A `file:` URL opens a file of the Mac")
        }
        _ => None,
    }
}

fn on_the_mac(why: &str) -> String {
    format!(
        "{why}. A Simulator's programs run on this Mac as your Mac user and reach its files, so `shell` takes only what acts on the Simulator alone: `simctl ui`, `openurl` (not `file:`), `privacy`, `location`, `status_bar`, `launch`, `terminate`, `uninstall`, `listapps`, `appinfo`, `get_app_container`, `pbcopy`, `pbpaste`, `keychain reset` and `io enumerate`. To allow the rest, stop the daemon (`phonedriver daemon stop`) and start it with PHONEDRIVER_HOST_SHELL=1"
    )
}

/// What `xcrun` runs for phonedriver's own command `command` in the Simulator `udid` whose system is the folder
/// `root`: `simctl spawn`, by the program's full path under `root`. With Xcode 27 `simctl spawn` of a path inside the
/// Simulator (`/bin/sh`) fails with launchd error 111 while the same program by its path under `root` runs. iOS 26
/// Simulators have no `/bin/sh`, so there a command is one program and its arguments.
pub fn spawn_args(
    udid: &str,
    command: &str,
    root: &std::path::Path,
) -> std::result::Result<Vec<String>, String> {
    spawn_words(udid, command, &split_words(command)?, root)
}

fn spawn_words(
    udid: &str,
    command: &str,
    words: &[String],
    root: &std::path::Path,
) -> std::result::Result<Vec<String>, String> {
    let sh = root.join("bin/sh");
    if sh.exists() {
        return Ok(vec![
            "spawn".into(),
            udid.into(),
            sh.display().to_string(),
            "-c".into(),
            command.into(),
        ]);
    }
    if needs_shell(command) {
        return Err(format!(
            "this Simulator's iOS has no shell, so a command is one program and its arguments: no pipes, `;`, `&&`, redirection, `$` or wildcards. Run each program in its own call ({}), or `simctl <subcommand>`",
            examples(root)
        ));
    }
    let Some(program) = words.first() else {
        return Err("the command is empty".into());
    };
    let path = if program.starts_with('/') {
        Some(root.join(program.trim_start_matches('/'))).filter(|p| p.is_file())
    } else {
        PROGRAM_FOLDERS
            .iter()
            .map(|folder| root.join(folder).join(program))
            .find(|p| p.is_file())
    };
    let Some(path) = path else {
        return Err(format!(
            "`{program}` is not a program in this Simulator's iOS. It has {}, or use `simctl <subcommand>`",
            examples(root)
        ));
    };
    let mut args = vec!["spawn".into(), udid.into(), path.display().to_string()];
    args.extend(words[1..].iter().cloned());
    Ok(args)
}

/// Whether `command` uses what only a shell does: a pipe, a list, redirection, expansion or a wildcard outside quotes.
fn needs_shell(command: &str) -> bool {
    let mut quote: Option<char> = None;
    command.chars().any(|c| match (quote, c) {
        (Some(q), c) if c == q => {
            quote = None;
            false
        }
        (Some('\''), _) => false,
        (Some(_), c) => c == '$' || c == '`',
        (None, '\'' | '"') => {
            quote = Some(c);
            false
        }
        (None, c) => "|;&<>$`*?(){}\n".contains(c),
    })
}

/// The command that streams the unified log of a Simulator, at debug level, of one app's process when `app_folder`
/// (the `<name>.app` folder of its bundle) is given. It runs like any `shell` command.
pub fn log_stream_command(app_folder: Option<&str>) -> std::result::Result<String, String> {
    match app_folder {
        None => Ok("log stream --style compact --level debug".into()),
        Some(folder) => {
            let plain = folder.ends_with(".app")
                && folder
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || " ._-".contains(c));
            if !plain {
                return Err(format!(
                    "the app folder {folder:?} has a name this cannot scope the log to"
                ));
            }
            Ok(format!(
                "log stream --style compact --level debug --predicate 'processImagePath CONTAINS \"/{folder}/\"'"
            ))
        }
    }
}

/// Follows `xcrun simctl <args>` (a `log stream`) until a line holds one of `req`'s patterns or `req.timeout` ends.
/// The stream's own header lines come before the first line that starts with a date and are not counted.
pub async fn simctl_follow(args: &[String], req: &LogWait) -> Result<LogWaited> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let mut child = Command::new("xcrun")
        .arg("simctl")
        .args(args)
        .env("LC_CTYPE", "UTF-8")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| Error::Transport {
            detail: format!("cannot run xcrun simctl (is Xcode installed?): {e}"),
        })?;
    let mut lines = BufReader::new(child.stdout.take().ok_or_else(|| Error::Transport {
        detail: "the log stream has no output to read".into(),
    })?)
    .lines();
    let deadline = tokio::time::Instant::now() + req.timeout;
    let (mut seen, mut started) = (0usize, false);
    loop {
        match tokio::time::timeout_at(deadline, lines.next_line()).await {
            Err(_) => return Ok(LogWaited { line: None, seen }),
            Ok(Ok(Some(line))) => {
                started = started || line.starts_with(|c: char| c.is_ascii_digit());
                if !started || line.is_empty() {
                    continue;
                }
                seen += 1;
                if req.matches(&line) {
                    return Ok(LogWaited {
                        line: Some(line),
                        seen,
                    });
                }
            }
            Ok(Ok(None)) => {
                return Err(Error::Transport {
                    detail: "the Simulator's log stream ended".into(),
                });
            }
            Ok(Err(e)) => {
                return Err(Error::Transport {
                    detail: format!("the Simulator's log stream failed: {e}"),
                });
            }
        }
    }
}

/// A few of the programs under `root`, for an error that says what can be run.
fn examples(root: &std::path::Path) -> String {
    let mut names: Vec<String> = PROGRAM_FOLDERS
        .iter()
        .filter_map(|folder| std::fs::read_dir(root.join(folder)).ok())
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    let preferred = ["defaults", "log", "launchctl", "notifyutil", "plutil"];
    let mut shown: Vec<&str> = preferred
        .iter()
        .copied()
        .filter(|p| names.iter().any(|n| n == p))
        .collect();
    if shown.is_empty() {
        shown = names.iter().take(5).map(String::as_str).collect();
    }
    match names.len().saturating_sub(shown.len()) {
        0 => shown.join(", "),
        more => format!("{} and {more} more", shown.join(", ")),
    }
}

/// The folder that holds the system of the Simulator `udid` (its runtime root), from `simctl list`.
pub async fn runtime_root(udid: &str) -> Result<std::path::PathBuf> {
    let devices = simctl(&["list", "devices", "-j"]).await?;
    let runtimes = simctl(&["list", "runtimes", "-j"]).await?;
    root_of(&devices, &runtimes, udid).ok_or_else(|| Error::Transport {
        detail: format!("simctl does not say where the system of Simulator {udid} is"),
    })
}

fn root_of(devices: &str, runtimes: &str, udid: &str) -> Option<std::path::PathBuf> {
    let devices: Value = serde_json::from_str(devices).ok()?;
    let runtimes: Value = serde_json::from_str(runtimes).ok()?;
    let (runtime, _) = devices["devices"].as_object()?.iter().find(|(_, list)| {
        list.as_array()
            .is_some_and(|l| l.iter().any(|d| d["udid"] == udid))
    })?;
    runtimes["runtimes"]
        .as_array()?
        .iter()
        .find(|r| r["identifier"] == runtime.as_str())?["runtimeRoot"]
        .as_str()
        .map(std::path::PathBuf::from)
}

fn split_words(s: &str) -> std::result::Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in s.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started || !cur.is_empty() {
                    words.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if quote.is_some() {
        return Err("a quote is not closed".into());
    }
    if started || !cur.is_empty() {
        words.push(cur);
    }
    Ok(words)
}

/// `xcrun simctl <args>` to completion: the exit status and both streams, killed after `timeout`.
pub async fn simctl_shell(
    args: &[String],
    timeout: std::time::Duration,
) -> Result<(i32, String, String, bool)> {
    let mut cmd = Command::new("xcrun");
    cmd.arg("simctl")
        .args(args)
        .env("LC_CTYPE", "UTF-8")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let child = cmd.spawn().map_err(|e| Error::Transport {
        detail: format!("cannot run xcrun simctl (is Xcode installed?): {e}"),
    })?;
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => Ok((
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            false,
        )),
        Ok(Err(e)) => Err(Error::Transport {
            detail: format!("simctl failed: {e}"),
        }),
        Err(_) => Ok((124, String::new(), String::new(), true)),
    }
}

/// One key of a property list file, through `plutil`.
pub async fn plutil_extract(file: &str, key: &str) -> Result<String> {
    let out = Command::new("plutil")
        .args(["-extract", key, "raw", "-o", "-", "--", file])
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| Error::Transport {
            detail: format!("cannot run plutil: {e}"),
        })?;
    if !out.status.success() {
        return Err(Error::Transport {
            detail: format!("{file} has no {key}"),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Converts the old-style property list `simctl listapps` prints into JSON with `plutil`.
pub async fn plist_to_json(plist: &str) -> Result<Value> {
    let mut child = Command::new("plutil")
        .args(["-convert", "json", "-o", "-", "--", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| Error::Transport {
            detail: format!("cannot run plutil: {e}"),
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(plist.as_bytes())
            .await
            .map_err(|e| Error::Transport {
                detail: format!("cannot write to plutil: {e}"),
            })?;
    }
    let out = child
        .wait_with_output()
        .await
        .map_err(|e| Error::Transport {
            detail: format!("plutil failed: {e}"),
        })?;
    serde_json::from_slice(&out.stdout).map_err(|e| Error::Transport {
        detail: format!(
            "plutil did not print JSON ({e}): {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ),
    })
}

/// The `simctl privacy` service for a permission name; `notifications` has none (Simulator apps ask on launch).
pub fn privacy_service(name: &str) -> Result<&'static str> {
    const SERVICES: &[&str] = &[
        "all",
        "calendar",
        "contacts-limited",
        "contacts",
        "location",
        "location-always",
        "photos-add",
        "photos",
        "media-library",
        "microphone",
        "motion",
        "reminders",
        "siri",
    ];
    SERVICES
        .iter()
        .copied()
        .find(|s| *s == name)
        .ok_or(Error::Unsupported {
            op: Op::Launch,
            reason: "the Simulator can grant all, calendar, contacts, contacts-limited, location, location-always, photos, photos-add, media-library, microphone, motion, reminders and siri; notifications and the camera cannot be pre-granted",
        })
}

pub const PHYSICAL_PUSH: &str = "a physical iPhone takes pushes only from Apple's push service with the app's own certificate, which phonedriver does not hold; the Simulator can be sent one with `apps notify`";

/// Refuses a push payload that is not a JSON object with an `aps` object, which is what APNs and `simctl push` need.
pub fn check_push_payload(payload: &str) -> Result<()> {
    let value: serde_json::Value = serde_json::from_str(payload).map_err(|e| Error::Refused {
        detail: format!("the push payload is not valid JSON ({e}); nothing was sent"),
    })?;
    let Some(object) = value.as_object() else {
        return Err(Error::Refused {
            detail: "the push payload must be a JSON object such as {\"aps\": {\"alert\": \"Hello\"}}; nothing was sent".into(),
        });
    };
    if !object.get("aps").is_some_and(serde_json::Value::is_object) {
        return Err(Error::Refused {
            detail: "the push payload needs an `aps` object, such as {\"aps\": {\"alert\": \"Hello\"}}; nothing was sent".into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{check_push_payload, privacy_service};

    fn why(payload: &str) -> String {
        match check_push_payload(payload).unwrap_err() {
            phonedriver_core::error::Error::Refused { detail } => detail,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_push_payload_must_be_json_with_an_aps_object() {
        check_push_payload(r#"{"aps":{"alert":{"title":"Hi"},"badge":1},"order":"7"}"#).unwrap();
        assert!(
            why("{\"aps\": ").contains("not valid JSON"),
            "{}",
            why("{\"aps\": ")
        );
        assert!(why("[1]").contains("object"), "{}", why("[1]"));
        assert!(why(r#"{"alert":"Hi"}"#).contains("`aps`"));
        assert!(why(r#"{"aps":"Hi"}"#).contains("`aps`"));
        assert!(why(r#"{"aps":"Hi"}"#).contains("object"));
    }

    #[test]
    fn a_privacy_service_maps_and_notifications_and_camera_do_not() {
        assert_eq!(privacy_service("photos").unwrap(), "photos");
        assert_eq!(
            privacy_service("location-always").unwrap(),
            "location-always"
        );
        assert!(privacy_service("notifications").is_err());
        assert!(privacy_service("camera").is_err());
    }

    use super::*;

    const LIST: &str = r#"{"devices":{
      "com.apple.CoreSimulator.SimRuntime.iOS-26-5":[
        {"udid":"280D26B7-7201-4907-877C-7101DE5FE4AE","name":"iPhone 17","state":"Booted","isAvailable":true},
        {"udid":"11111111-2222-3333-4444-555555555555","name":"iPhone Air","state":"Shutdown","isAvailable":true}],
      "com.apple.CoreSimulator.SimRuntime.watchOS-12-0":[]}}"#;

    #[test]
    fn keeps_booted_devices_with_their_os() {
        assert_eq!(
            parse_list(LIST).unwrap(),
            vec![Simulator {
                udid: "280D26B7-7201-4907-877C-7101DE5FE4AE".into(),
                name: "iPhone 17".into(),
                os: "iOS 26.5".into(),
            }]
        );
    }

    #[test]
    fn the_simulators_that_are_off_are_the_ios_ones_that_can_boot_newest_system_first() {
        let list = r#"{"devices":{
          "com.apple.CoreSimulator.SimRuntime.iOS-18-6":[
            {"udid":"A","name":"iPhone 16","state":"Shutdown","isAvailable":true}],
          "com.apple.CoreSimulator.SimRuntime.iOS-26-5":[
            {"udid":"B","name":"iPhone 17","state":"Booted","isAvailable":true},
            {"udid":"C","name":"iPhone 16","state":"Shutdown","isAvailable":true},
            {"udid":"D","name":"iPad Air","state":"Shutdown","isAvailable":false}],
          "com.apple.CoreSimulator.SimRuntime.tvOS-26-0":[
            {"udid":"E","name":"Apple TV","state":"Shutdown","isAvailable":true}]}}"#;
        let off: Vec<(String, String)> = parse_off(list)
            .unwrap()
            .into_iter()
            .map(|s| (s.udid, s.os))
            .collect();
        assert_eq!(
            off,
            [
                ("C".into(), "iOS 26.5".into()),
                ("A".into(), "iOS 18.6".into())
            ]
        );
    }

    fn system(with_shell: bool) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        for program in ["usr/bin/defaults", "usr/bin/log", "bin/launchctl", "bin/df"] {
            let path = root.path().join(program);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        if with_shell {
            std::fs::write(root.path().join("bin/sh"), "").unwrap();
        }
        root
    }

    #[test]
    fn a_command_runs_one_program_by_its_path_in_the_simulators_system() {
        let root = system(false);
        let args = shell_args("U", "defaults read -g 'Apple Locale'", root.path(), true).unwrap();
        assert_eq!(args[..2], ["spawn", "U"]);
        assert_eq!(
            std::path::Path::new(&args[2]),
            root.path().join("usr/bin/defaults")
        );
        assert_eq!(args[3..], ["read", "-g", "Apple Locale"]);
        assert_eq!(
            std::path::Path::new(&shell_args("U", "/bin/df -h", root.path(), true).unwrap()[2]),
            root.path().join("bin/df")
        );
    }

    #[test]
    fn a_system_with_a_shell_runs_the_command_in_it() {
        let root = system(true);
        let args = shell_args("U", "defaults read x | head -1", root.path(), true).unwrap();
        assert_eq!(args[..2], ["spawn", "U"]);
        assert_eq!(std::path::Path::new(&args[2]), root.path().join("bin/sh"));
        assert_eq!(args[3..], ["-c", "defaults read x | head -1"]);
    }

    #[test]
    fn without_a_shell_what_needs_one_is_refused_and_says_what_can_run() {
        let root = system(false);
        for command in [
            "defaults read x | head -1",
            "log show; ls",
            "echo $HOME",
            "ls *.plist",
            "defaults read \"$KEY\"",
        ] {
            let e = shell_args("U", command, root.path(), true).unwrap_err();
            assert!(e.contains("has no shell"), "{command}: {e}");
            assert!(e.contains("defaults, log, launchctl"), "{e}");
        }
        assert!(
            shell_args(
                "U",
                "log show --predicate 'process == \"a|b\"'",
                root.path(),
                true
            )
            .is_ok()
        );
        let e = shell_args("U", "ls /", root.path(), true).unwrap_err();
        assert!(e.starts_with("`ls` is not a program in this Simulator's iOS. It has defaults, log, launchctl and 1 more"), "{e}");
    }

    #[test]
    fn the_system_folder_is_the_root_of_the_simulators_runtime() {
        let devices = r#"{"devices":{"com.apple.CoreSimulator.SimRuntime.iOS-26-5":[{"udid":"U","name":"iPhone 17","state":"Booted"}],"com.apple.CoreSimulator.SimRuntime.iOS-18-6":[{"udid":"A"}]}}"#;
        let runtimes = r#"{"runtimes":[{"identifier":"com.apple.CoreSimulator.SimRuntime.iOS-18-6","runtimeRoot":"/r/18"},{"identifier":"com.apple.CoreSimulator.SimRuntime.iOS-26-5","runtimeRoot":"/r/26"}]}"#;
        assert_eq!(root_of(devices, runtimes, "U"), Some("/r/26".into()));
        assert_eq!(root_of(devices, runtimes, "A"), Some("/r/18".into()));
        assert_eq!(root_of(devices, runtimes, "Z"), None);
    }

    #[test]
    fn simctl_commands_get_the_device_after_the_subcommand() {
        assert_eq!(
            shell_args(
                "U",
                "simctl ui appearance dark",
                std::path::Path::new("/r"),
                false
            )
            .unwrap(),
            ["ui", "U", "appearance", "dark"]
        );
        assert_eq!(
            shell_args(
                "U",
                "simctl openurl 'https://example.com/a b'",
                std::path::Path::new("/r"),
                false
            )
            .unwrap(),
            ["openurl", "U", "https://example.com/a b"]
        );
        assert_eq!(
            shell_args(
                "U",
                "simctl location set 37.77,-122.41",
                std::path::Path::new("/r"),
                false
            )
            .unwrap(),
            ["location", "U", "set", "37.77,-122.41"]
        );
    }

    #[test]
    fn simctl_subcommands_that_do_not_target_this_device_are_refused_with_the_list() {
        let e = shell_args("U", "simctl erase all", std::path::Path::new("/r"), true).unwrap_err();
        assert!(e.contains("not offered") && e.contains("openurl"), "{e}");
        assert!(shell_args("U", "simctl", std::path::Path::new("/r"), true).is_err());
        assert!(
            shell_args(
                "U",
                "simctl openurl 'unclosed",
                std::path::Path::new("/r"),
                true
            )
            .is_err()
        );
    }

    #[test]
    fn the_log_stream_is_one_program_that_runs_with_or_without_a_shell_in_the_system() {
        let command = log_stream_command(None).unwrap();
        assert_eq!(command, "log stream --style compact --level debug");
        let scoped = log_stream_command(Some("Testbed.app")).unwrap();
        assert!(
            scoped.ends_with("--predicate 'processImagePath CONTAINS \"/Testbed.app/\"'"),
            "{scoped}"
        );
        for with_shell in [false, true] {
            let root = system(with_shell);
            let args = spawn_args("U", &scoped, root.path()).unwrap();
            assert_eq!(args[..2], ["spawn", "U"]);
            assert!(
                args.iter().any(|a| a == "--level" || a.contains("--level")),
                "{args:?}"
            );
        }
        let root = system(false);
        let args = spawn_args("U", &scoped, root.path()).unwrap();
        assert_eq!(
            args.last().unwrap(),
            "processImagePath CONTAINS \"/Testbed.app/\""
        );
    }

    #[test]
    fn an_app_folder_that_could_end_the_predicate_is_refused() {
        for folder in ["a'b.app", "a\"b.app", "a$b.app", "Testbed"] {
            assert!(log_stream_command(Some(folder)).is_err(), "{folder}");
        }
    }

    #[test]
    fn garbage_is_a_transport_error() {
        assert!(matches!(
            parse_list("not json"),
            Err(Error::Transport { .. })
        ));
    }

    const MAC_PATHS: &[&str] = &[
        "simctl spawn defaults write /private/tmp/simwrite probe written-from-simulator",
        "simctl io screenshot /Users/me/shot.png",
        "simctl io recordVideo /Users/me/a.mp4",
        "simctl addmedia /Users/me/photo.jpg",
        "simctl install /Users/me/Other.app",
        "simctl keychain add-root-cert /Users/me/ca.pem",
        "simctl keychain add-cert /Users/me/c.pem",
        "simctl push com.example /Users/me/payload.json",
        "simctl launch --stdout=/Users/me/out.txt com.example",
        "simctl launch com.example --stderr=/Users/me/err.txt",
        "simctl openurl file:///Users/me/.ssh/id_ed25519",
        "simctl openurl FILE:///Users/me/notes.txt",
    ];

    #[test]
    fn a_program_in_the_simulator_runs_on_the_mac_so_it_waits_for_the_persons_switch() {
        for with_shell in [false, true] {
            let root = system(with_shell);
            for command in [
                "defaults read -g AppleLocale",
                "launchctl list",
                "/bin/df -h",
            ] {
                let e = shell_args("U", command, root.path(), false).unwrap_err();
                assert!(e.contains("as your Mac user"), "{command}: {e}");
                assert!(e.contains("PHONEDRIVER_HOST_SHELL=1"), "{command}: {e}");
                assert!(e.contains("simctl ui"), "{command}: {e}");
                assert!(!e.contains("AppleLocale"), "{command}: {e}");
                let args = shell_args("U", command, root.path(), true).unwrap();
                assert_eq!(args[..2], ["spawn", "U"], "{command}");
            }
        }
    }

    #[test]
    fn simctl_forms_that_run_a_mac_program_or_name_a_mac_file_wait_for_the_persons_switch() {
        let root = std::path::Path::new("/r");
        for command in MAC_PATHS {
            let e = shell_args("U", command, root, false).unwrap_err();
            assert!(e.contains("PHONEDRIVER_HOST_SHELL=1"), "{command}: {e}");
            assert!(!e.contains("/Users/me"), "{command}: {e}");
            let args = shell_args("U", command, root, true).unwrap();
            assert_eq!(args[1], "U", "{command}");
        }
        let e = shell_args("U", "simctl io screenshot /tmp/a.png", root, false).unwrap_err();
        assert!(e.contains("`screenshot`"), "{e}");
        let e = shell_args("U", "simctl push com.example /tmp/p.json", root, false).unwrap_err();
        assert!(e.contains("`apps notify`"), "{e}");
        let e = shell_args("U", "simctl install /tmp/A.app", root, false).unwrap_err();
        assert!(e.contains("`install`"), "{e}");
    }

    #[test]
    fn simctl_forms_that_act_only_on_the_simulator_need_no_switch() {
        let root = std::path::Path::new("/r");
        for command in [
            "simctl ui appearance dark",
            "simctl openurl https://example.com",
            "simctl openurl myapp://settings",
            "simctl privacy grant photos com.example",
            "simctl location set 37.77,-122.41",
            "simctl status_bar override --time 9:41",
            "simctl launch com.example",
            "simctl launch --terminate-running-process com.example -flag",
            "simctl terminate com.example",
            "simctl uninstall com.example",
            "simctl listapps",
            "simctl appinfo com.example",
            "simctl get_app_container com.example data",
            "simctl keychain reset",
            "simctl pbcopy",
            "simctl pbpaste",
            "simctl io enumerate",
        ] {
            let args =
                shell_args("U", command, root, false).unwrap_or_else(|e| panic!("{command}: {e}"));
            assert_eq!(args[1], "U", "{command}");
        }
    }
}
