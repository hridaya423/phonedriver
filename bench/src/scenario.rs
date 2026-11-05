//! Scenarios: verbs run through the real CLI, checked against its JSON replies, the device's own state, or the
//! iOS testbed's event log. No LLM.
//!
//! `phone-use-bench scenario --phone-use <binary> --device sim:<udid> [--runs 3] <file.yaml>...`
//!
//! A scenario file:
//! ```yaml
//! name: settings-dark-appearance
//! device: ios
//! steps:
//!   - launch: Settings
//!   - tap: {text: "Dark Appearance", role: switch}
//!   - expect: {element: {text: "Dark Appearance", role: switch}, state: on}
//!   - sh: {run: "xcrun simctl ui {udid} appearance", stdout: dark}
//! cleanup:
//!   - sh: "xcrun simctl ui {udid} appearance light"
//! ```
//! Steps: `launch`, `open`, `press`, `tap`, `type`, `swipe` and `snapshot` call the CLI; a target is the CLI's
//! target string or `{text, contains, role, id}` resolved to one ref from `snapshot --json`. `expect` polls
//! `snapshot --json` until an element matches (or, with `absent: true`, none does). `sh` runs a host command
//! with `{udid}` substituted. `testbed: <tab>` starts the iOS testbed on a tab with an empty log, `clear_log`
//! empties it, and `log: {match: {...}}` waits for an event with those fields. `cleanup` always runs.

use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use serde_json::{Map, Value};

const TESTBED: &str = "dev.phoneuse.testbed";
const WAIT: Duration = Duration::from_secs(5);

struct Args {
    phone_use: PathBuf,
    device: String,
    runs: usize,
    files: Vec<PathBuf>,
}

fn args() -> Result<Args, String> {
    let mut it = std::env::args().skip(2);
    let (mut phone_use, mut device, mut runs, mut files) = (None, None, 1, Vec::new());
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--phone-use" => phone_use = Some(PathBuf::from(value()?)),
            "--device" => device = Some(value()?),
            "--runs" => runs = value()?.parse().map_err(|e| format!("--runs: {e}"))?,
            other if other.starts_with("--") => return Err(format!("unknown argument {other}")),
            file => files.push(PathBuf::from(file)),
        }
    }
    if files.is_empty() {
        return Err("name at least one scenario file".into());
    }
    Ok(Args {
        phone_use: phone_use.ok_or("--phone-use <binary> is required")?,
        device: device.ok_or("--device sim:<udid> is required")?,
        runs,
        files,
    })
}

struct Scenario {
    name: String,
    steps: Vec<Value>,
    cleanup: Vec<Value>,
}

fn load(path: &PathBuf) -> Result<Scenario, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let v: Value = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let list = |key: &str| v[key].as_array().cloned().unwrap_or_default();
    Ok(Scenario {
        name: v["name"]
            .as_str()
            .ok_or(format!("{}: no name", path.display()))?
            .to_string(),
        steps: list("steps"),
        cleanup: list("cleanup"),
    })
}

struct Rig {
    binary: PathBuf,
    home: PathBuf,
    device: String,
    udid: String,
}

fn text(out: &[u8]) -> String {
    String::from_utf8_lossy(out).trim().to_string()
}

fn string(v: &Value) -> Result<&str, String> {
    v.as_str().ok_or(format!("expected a string, got {v}"))
}

fn matches(e: &Value, spec: &Map<String, Value>) -> bool {
    spec.iter().all(|(k, want)| match k.as_str() {
        "text" => e["label"] == *want,
        "contains" => want.as_str().is_some_and(|w| {
            [&e["label"], &e["value"]]
                .iter()
                .any(|f| f.as_str().is_some_and(|s| s.contains(w)))
        }),
        "role" | "id" => e[k] == *want,
        _ => false,
    })
}

fn checked(e: &Value) -> bool {
    e["state"].as_str().is_some_and(|s| s.contains("CHECKED"))
}

impl Rig {
    fn cli(&self, args: &[&str]) -> Output {
        Command::new(&self.binary)
            .env("PHONE_USE_HOME", &self.home)
            .env_remove("PHONE_USE_SESSION")
            .args(["--json", "--device", &self.device])
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("cannot run {}: {e}", self.binary.display()))
    }

    fn ok(&self, args: &[&str]) -> Result<Value, String> {
        let out = self.cli(args);
        if !out.status.success() {
            return Err(format!("phone-use {}: {}", args[0], text(&out.stderr)));
        }
        serde_json::from_slice(&out.stdout).map_err(|e| format!("phone-use {}: {e}", args[0]))
    }

    fn elements(&self) -> Result<Vec<Value>, String> {
        Ok(self.ok(&["snapshot"])?["elements"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    fn target(&self, v: &Value) -> Result<String, String> {
        let Some(spec) = v.as_object() else {
            return string(v).map(str::to_string);
        };
        let els = self.elements()?;
        let found: Vec<&Value> = els.iter().filter(|e| matches(e, spec)).collect();
        match found.as_slice() {
            [one] => Ok(format!("@{}", string(&one["ref"])?)),
            _ => Err(format!("{} elements match {v}", found.len())),
        }
    }

    fn step(&self, step: &Value) -> Result<(), String> {
        let (verb, arg) = step
            .as_object()
            .and_then(|m| m.iter().next())
            .ok_or(format!("a step is a map with one key, got {step}"))?;
        match verb.as_str() {
            "launch" | "open" | "press" => self.ok(&[verb, string(arg)?]).map(drop),
            "snapshot" => self.ok(&["snapshot"]).map(drop),
            "tap" => self.ok(&["tap", &self.target(arg)?]).map(drop),
            "type" => self.type_text(arg),
            "swipe" => self.swipe(arg),
            "expect" => self.expect(arg),
            "sh" => self.sh(arg),
            "testbed" => self.testbed(string(arg)?),
            "clear_log" => self.clear_log(),
            "log" => self.log(arg),
            other => Err(format!("unknown step {other}")),
        }
    }

    fn type_text(&self, arg: &Value) -> Result<(), String> {
        let body = arg["text"]
            .as_str()
            .or(arg.as_str())
            .ok_or("type needs text")?;
        let mut args = vec!["type".to_string(), body.to_string()];
        if let Some(into) = arg.get("into") {
            args.extend(["--into".into(), self.target(into)?]);
        }
        if arg["submit"] == true {
            args.push("--submit".into());
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let r = self.ok(&refs)?;
        if r["verified"] == false {
            return Err(format!("typed text not verified: {}", r["summary"]));
        }
        Ok(())
    }

    fn swipe(&self, arg: &Value) -> Result<(), String> {
        let dir = arg["direction"]
            .as_str()
            .or(arg.as_str())
            .ok_or("swipe needs a direction")?;
        let mut args = vec!["swipe".to_string(), dir.to_string()];
        if let Some(from) = arg.get("from") {
            args.extend(["--from".into(), self.target(from)?]);
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.ok(&refs).map(drop)
    }

    fn expect(&self, arg: &Value) -> Result<(), String> {
        let spec = arg["element"]
            .as_object()
            .ok_or("expect needs an element")?;
        let absent = arg["absent"] == true;
        let state = arg["state"].as_str();
        let wait = arg["timeout_ms"]
            .as_u64()
            .map_or(WAIT, Duration::from_millis);
        let deadline = Instant::now() + wait;
        loop {
            let els = self.elements()?;
            let found: Vec<&Value> = els.iter().filter(|e| matches(e, spec)).collect();
            let met = if absent {
                found.is_empty()
            } else {
                found.iter().any(|e| match state {
                    Some("on") => checked(e),
                    Some("off") => !checked(e),
                    _ => true,
                })
            };
            if met {
                return Ok(());
            }
            if Instant::now() >= deadline {
                let seen: Vec<String> = found
                    .iter()
                    .map(|e| format!("{} {} {}", e["role"], e["label"], e["state"]))
                    .collect();
                return Err(format!(
                    "expected {}{arg} within {} ms; matching: {seen:?}",
                    if absent { "no " } else { "" },
                    wait.as_millis()
                ));
            }
        }
    }

    fn sh(&self, arg: &Value) -> Result<(), String> {
        let run = arg["run"]
            .as_str()
            .or(arg.as_str())
            .ok_or("sh needs a command")?;
        let run = run.replace("{udid}", &self.udid);
        let out = Command::new("sh")
            .args(["-c", &run])
            .output()
            .map_err(|e| format!("{run}: {e}"))?;
        if !out.status.success() {
            return Err(format!("{run}: {}", text(&out.stderr)));
        }
        match arg["stdout"].as_str() {
            Some(want) if !text(&out.stdout).contains(want) => Err(format!(
                "{run}: printed {:?}, expected {want:?}",
                text(&out.stdout)
            )),
            _ => Ok(()),
        }
    }

    fn simctl(&self, args: &[&str]) -> Result<String, String> {
        let out = Command::new("xcrun")
            .arg("simctl")
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("simctl {}: {}", args[0], text(&out.stderr)));
        }
        Ok(text(&out.stdout))
    }

    fn log_path(&self) -> Result<PathBuf, String> {
        let data = self.simctl(&["get_app_container", &self.udid, TESTBED, "data"])?;
        Ok(PathBuf::from(data).join("Documents/putb.log"))
    }

    fn read_log(&self) -> Result<Vec<Value>, String> {
        let body = std::fs::read_to_string(self.log_path()?).unwrap_or_default();
        Ok(body
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    }

    fn clear_log(&self) -> Result<(), String> {
        let path = self.log_path()?;
        if path.exists() {
            std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .and_then(|f| f.set_len(0))
                .map_err(|e| format!("{}: {e}", path.display()))?;
        }
        Ok(())
    }

    fn wait_log(&self, want: &Map<String, Value>, wait: Duration) -> Result<(), String> {
        let deadline = Instant::now() + wait;
        loop {
            let log = self.read_log()?;
            if log
                .iter()
                .any(|e| want.iter().all(|(k, v)| e.get(k) == Some(v)))
            {
                return Ok(());
            }
            if Instant::now() >= deadline {
                let tail = &log[log.len().saturating_sub(6)..];
                return Err(format!(
                    "no testbed event {} within {} ms; log tail: {}",
                    Value::Object(want.clone()),
                    wait.as_millis(),
                    Value::from(tail.to_vec())
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn testbed(&self, tab: &str) -> Result<(), String> {
        let _ = self.simctl(&["terminate", &self.udid, TESTBED]);
        self.simctl(&["launch", &self.udid, TESTBED, "-tab", tab])?;
        let mut want = Map::new();
        want.insert("ev".into(), "tab".into());
        want.insert("name".into(), tab.into());
        self.wait_log(&want, WAIT)?;
        self.clear_log()
    }

    fn log(&self, arg: &Value) -> Result<(), String> {
        let want = arg["match"].as_object().ok_or("log needs a match")?;
        let wait = arg["timeout_ms"]
            .as_u64()
            .map_or(WAIT, Duration::from_millis);
        self.wait_log(want, wait)
    }

    /// Keeps the screen and the snapshot of a failed step next to the daemon's traces.
    fn keep_evidence(&self, name: &str, run: usize) -> String {
        let base = self.home.join(format!("fail-{name}-{run}"));
        let png = base.with_extension("png");
        let _ = self.cli(&["screenshot", "-o", &png.to_string_lossy()]);
        let out = Command::new(&self.binary)
            .env("PHONE_USE_HOME", &self.home)
            .args(["--device", &self.device, "snapshot", "--no-settle"])
            .output();
        if let Ok(out) = out {
            let _ = std::fs::write(base.with_extension("txt"), out.stdout);
        }
        base.display().to_string()
    }

    fn run(&self, s: &Scenario, run: usize) -> Result<(), String> {
        let mut result =
            s.steps.iter().enumerate().try_for_each(|(i, step)| {
                self.step(step).map_err(|e| format!("step {}: {e}", i + 1))
            });
        if let Err(e) = &result {
            result = Err(format!(
                "{e} (evidence {}.*)",
                self.keep_evidence(&s.name, run)
            ));
        }
        for (i, step) in s.cleanup.iter().enumerate() {
            if let (Err(e), Ok(())) = (self.step(step), &result) {
                result = Err(format!("cleanup {}: {e}", i + 1));
            }
        }
        result
    }
}

pub fn main() {
    let args = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };
    let scenarios: Vec<Scenario> = match args.files.iter().map(load).collect() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };
    let home = std::env::temp_dir().join(format!("phone-use-scenarios-{}", std::process::id()));
    std::fs::create_dir_all(&home).expect("create the scenario home");
    let rig = Rig {
        binary: args.phone_use,
        home,
        udid: args
            .device
            .strip_prefix("sim:")
            .unwrap_or(&args.device)
            .to_string(),
        device: args.device,
    };
    let mut all = true;
    println!("| run | scenario | result | s | failure |");
    println!("|---|---|---|---|---|");
    for run in 1..=args.runs {
        let mut passed = 0;
        for s in &scenarios {
            let started = Instant::now();
            let result = rig.run(s, run);
            let secs = started.elapsed().as_secs_f64();
            let (mark, why) = match &result {
                Ok(()) => ("pass", String::new()),
                Err(e) => ("FAIL", e.replace('|', "\\|").replace('\n', " ")),
            };
            passed += usize::from(result.is_ok());
            println!("| {run} | {} | {mark} | {secs:.1} | {why} |", s.name);
        }
        all &= passed == scenarios.len();
        eprintln!("run {run}: {passed}/{} passed", scenarios.len());
    }
    let _ = rig.cli(&["release"]);
    if !all {
        std::process::exit(1);
    }
}
