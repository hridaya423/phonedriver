//! Booted iOS simulators through `xcrun simctl`.

use phone_use_core::error::{Error, Op};
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

/// Parses `xcrun simctl list devices -j` and keeps the booted devices.
pub fn parse_list(json: &str) -> Result<Vec<Simulator>> {
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
            if d["state"] != "Booted" {
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

/// What `xcrun` runs for a `shell` command on the Simulator `udid`: `simctl <sub> ...` gets the device inserted after
/// the subcommand, and anything else runs in the Simulator's `/bin/sh` through `simctl spawn`.
pub fn shell_args(udid: &str, command: &str) -> std::result::Result<Vec<String>, String> {
    let words = split_words(command)?;
    if words.first().map(String::as_str) != Some("simctl") {
        return Ok(["spawn", udid, "/bin/sh", "-c", command]
            .map(String::from)
            .to_vec());
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
    let mut args = vec![sub.clone(), udid.to_string()];
    args.extend(words[2..].iter().cloned());
    Ok(args)
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

#[cfg(test)]
mod tests {
    use super::privacy_service;

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
    fn a_plain_command_runs_in_the_simulators_shell() {
        assert_eq!(
            shell_args("U", "defaults read -g AppleInterfaceStyle").unwrap(),
            [
                "spawn",
                "U",
                "/bin/sh",
                "-c",
                "defaults read -g AppleInterfaceStyle"
            ]
        );
    }

    #[test]
    fn simctl_commands_get_the_device_after_the_subcommand() {
        assert_eq!(
            shell_args("U", "simctl ui appearance dark").unwrap(),
            ["ui", "U", "appearance", "dark"]
        );
        assert_eq!(
            shell_args("U", "simctl openurl 'https://example.com/a b'").unwrap(),
            ["openurl", "U", "https://example.com/a b"]
        );
        assert_eq!(
            shell_args("U", "simctl location set 37.77,-122.41").unwrap(),
            ["location", "U", "set", "37.77,-122.41"]
        );
    }

    #[test]
    fn simctl_subcommands_that_do_not_target_this_device_are_refused_with_the_list() {
        let e = shell_args("U", "simctl erase all").unwrap_err();
        assert!(e.contains("not offered") && e.contains("openurl"), "{e}");
        assert!(shell_args("U", "simctl").is_err());
        assert!(shell_args("U", "simctl openurl 'unclosed").is_err());
    }

    #[test]
    fn garbage_is_a_transport_error() {
        assert!(matches!(
            parse_list("not json"),
            Err(Error::Transport { .. })
        ));
    }
}
