//! `phone-use doctor`: checks the machine and the devices, one line each, `ok`, `warn` or `fail`, with a fix under
//! every line that is not `ok`. iOS Simulator checks for now (task 5.7); Android checks come with Phase 4.

use std::path::Path;
use std::time::Instant;

use serde_json::json;

use crate::client::Client;

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
    let checks = rt.block_on(ios(home));
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
    if xcodegen.is_none() {
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

async fn runner_answers(id: &str) -> Check {
    let name = format!("runner {id}");
    let started = Instant::now();
    let mut client = match Client::connect("doctor".into()).await {
        Ok(c) => c,
        Err(e) => {
            return Check::bad(
                Status::Fail,
                name,
                format!("cannot reach the daemon: {}", e.message()),
                "Run `phone-use daemon` in a terminal and read what it prints.",
            );
        }
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
}
