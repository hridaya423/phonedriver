//! Booted iOS simulators through `xcrun simctl`.

use phone_use_core::error::Error;
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
/// because the input may be a secret (`pbcopy`).
pub async fn simctl_with_input(args: &[&str], input: Option<&str>) -> Result<String> {
    let mut cmd = Command::new("xcrun");
    cmd.arg("simctl")
        .args(args)
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

#[cfg(test)]
mod tests {
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
    fn garbage_is_a_transport_error() {
        assert!(matches!(
            parse_list("not json"),
            Err(Error::Transport { .. })
        ));
    }
}
