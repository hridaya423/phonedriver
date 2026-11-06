//! Scenario runner: `phone-use-bench scenario --phone-use <binary> --serial <serial> [--dir bench/scenarios/android]
//! [--only name] [--runs 3] [--out results.jsonl]`.
//!
//! A scenario is a list of verbs and expectations run through the real CLI, without an LLM (task 3.11). Steps run
//! in order and stop at the first failure; `cleanup` always runs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use phone_use_android::adb::Adb;
use serde::Deserialize;
use serde_json::Value;

const EXPECT_WITHIN: Duration = Duration::from_secs(8);
const PACKAGE: &str = "dev.phoneuse.testbed";

#[derive(Deserialize)]
struct Scenario {
    name: String,
    #[serde(default, with = "serde_yaml::with::singleton_map_recursive")]
    steps: Vec<Step>,
    #[serde(default, with = "serde_yaml::with::singleton_map_recursive")]
    cleanup: Vec<Step>,
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
enum Texts {
    One(String),
    Many(Vec<String>),
}

impl Texts {
    fn all(&self) -> Vec<&str> {
        match self {
            Texts::One(s) => vec![s],
            Texts::Many(v) => v.iter().map(String::as_str).collect(),
        }
    }
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
enum Target {
    Plain(Texts),
    Full { text: Texts, role: Option<String> },
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
enum Step {
    Launch(String),
    Open(String),
    Press(String),
    Wait(String),
    Shell(String),
    SleepMs(u64),
    Tap(Target),
    LongTap {
        text: String,
        ms: u32,
    },
    Type {
        text: String,
        into: Option<String>,
        mode: Option<String>,
    },
    Clear(String),
    Scroll {
        direction: String,
        until: Option<String>,
    },
    Swipe {
        direction: String,
        from: Option<String>,
    },
    Testbed(String),
    SetSwitch {
        text: String,
        state: String,
    },
    RestoreSwitch(String),
    Dismiss(Vec<String>),
    Expect(Expect),
    /// Steps that only exist on newer Android: `{at_least: 29, then: [...], otherwise: [...]}`.
    IfApi {
        at_least: u32,
        #[serde(default)]
        then: Vec<Step>,
        #[serde(default)]
        otherwise: Vec<Step>,
    },
}

#[derive(Deserialize, Clone, Default)]
struct Expect {
    text: Option<Texts>,
    absent: Option<String>,
    element: Option<Target>,
    state: Option<String>,
    log: Option<Value>,
    no_log: Option<Value>,
    shell: Option<ShellCheck>,
}

#[derive(Deserialize, Clone)]
struct ShellCheck {
    cmd: String,
    contains: String,
}

struct Ctx {
    bin: PathBuf,
    serial: String,
    home: PathBuf,
    adb: Adb,
    originals: HashMap<String, bool>,
}

impl Ctx {
    fn cli(&self, args: &[&str]) -> Result<String, String> {
        let out = Command::new(&self.bin)
            .env("PHONE_USE_HOME", &self.home)
            .env_remove("PHONE_USE_SESSION")
            .arg("--device")
            .arg(&self.serial)
            .args(args)
            .output()
            .map_err(|e| format!("cannot run {}: {e}", self.bin.display()))?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        if out.status.success() {
            Ok(stdout)
        } else {
            Err(format!(
                "phone-use {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }

    async fn shell(&self, cmd: &str) -> Result<String, String> {
        self.adb
            .shell(&self.serial, cmd)
            .await
            .map(|o| o.stdout_text())
            .map_err(|e| format!("adb shell {cmd}: {e}"))
    }

    fn elements(&self) -> Result<Vec<Value>, String> {
        let out = self.cli(&["--json", "snapshot", "--no-settle"])?;
        let v: Value = serde_json::from_str(&out).map_err(|e| e.to_string())?;
        Ok(v["elements"].as_array().cloned().unwrap_or_default())
    }

    async fn log(&self) -> Vec<Value> {
        let out = self
            .shell("logcat -d -s PUTB:I -v raw")
            .await
            .unwrap_or_default();
        out.lines()
            .filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
            .collect()
    }

    async fn run(&mut self, step: &Step) -> Result<(), String> {
        match step {
            Step::Launch(app) => self.cli(&["launch", app]).map(drop),
            Step::Open(url) => self.cli(&["open", url]).map(drop),
            Step::Press(key) => self.cli(&["press", key]).map(drop),
            Step::Wait(what) => self.cli(&["--timeout", "10000", "wait", what]).map(drop),
            Step::Shell(cmd) => self.shell(cmd).await.map(drop),
            Step::SleepMs(ms) => {
                tokio::time::sleep(Duration::from_millis(*ms)).await;
                Ok(())
            }
            Step::Tap(target) => self.tap(target),
            Step::LongTap { text, ms } => self
                .cli(&["tap", text, "--long", &ms.to_string()])
                .map(drop),
            Step::Type { text, into, mode } => {
                let mut args = vec!["type", text.as_str()];
                if let Some(i) = into {
                    args.extend(["--into", i]);
                }
                if let Some(m) = mode {
                    args.extend(["--mode", m]);
                }
                let out = self.cli(&args)?;
                if out.contains("NOT verified") {
                    return Err(format!("the field does not show the text: {out}"));
                }
                Ok(())
            }
            Step::Clear(target) => self.cli(&["clear", target]).map(drop),
            Step::Scroll { direction, until } => {
                let mut args = vec!["scroll", direction.as_str()];
                if let Some(u) = until {
                    args.extend(["--until", u]);
                }
                self.cli(&args).map(drop)
            }
            Step::Swipe { direction, from } => {
                let mut args = vec!["swipe", direction.as_str()];
                if let Some(f) = from {
                    args.extend(["--from", f]);
                }
                self.cli(&args).map(drop)
            }
            Step::Testbed(tab) => {
                self.shell(&format!(
                    "am start -n {PACKAGE}/.MainActivity --es tab {tab}"
                ))
                .await?;
                let started = Instant::now();
                loop {
                    let shown = self.cli(&["snapshot", "--no-settle"])?;
                    if shown.contains(&format!("app {PACKAGE}")) && shown.matches('@').count() > 6 {
                        break;
                    }
                    if started.elapsed() > EXPECT_WITHIN {
                        return Err(format!("the testbed's {tab} tab did not show:\n{shown}"));
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                tokio::time::sleep(Duration::from_millis(1000)).await;
                self.shell("logcat -c").await.map(drop)
            }
            Step::SetSwitch { text, state } => {
                let want = state == "on";
                let (r, now) = self.find_switch(text)?;
                self.originals.entry(text.clone()).or_insert(now);
                if now != want {
                    self.cli(&["tap", &format!("@{r}")])?;
                }
                Ok(())
            }
            Step::RestoreSwitch(text) => {
                let Some(original) = self.originals.get(text).copied() else {
                    return Ok(());
                };
                let (r, now) = self.find_switch(text)?;
                if now != original {
                    self.cli(&["tap", &format!("@{r}")])?;
                }
                Ok(())
            }
            Step::Dismiss(texts) => {
                let mut dismissed = false;
                let mut misses = 0;
                let allowed = |dismissed: bool| if dismissed { 3 } else { 12 };
                while misses < allowed(dismissed) {
                    let shown = self.cli(&["snapshot", "--no-settle"])?;
                    let hit = texts.iter().find(|t| shown.contains(&format!("\"{t}\"")));
                    match hit {
                        Some(t) => {
                            self.cli(&["tap", t])?;
                            dismissed = true;
                            misses = 0;
                            tokio::time::sleep(Duration::from_millis(800)).await;
                        }
                        None => {
                            misses += 1;
                            tokio::time::sleep(Duration::from_millis(1000)).await;
                        }
                    }
                }
                Ok(())
            }
            Step::Expect(e) => self.expect(e).await,
            Step::IfApi { .. } => Err("IfApi must be flattened before running".into()),
        }
    }

    fn tap(&self, target: &Target) -> Result<(), String> {
        let (texts, role) = match target {
            Target::Plain(t) => (t.all(), None),
            Target::Full { text, role } => (text.all(), role.as_deref()),
        };
        let mut last = String::new();
        for t in &texts {
            let mut args = vec!["tap", t];
            if let Some(r) = role {
                args.extend(["--role", r]);
            }
            match self.cli(&args) {
                Ok(_) => return Ok(()),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    fn find_switch(&self, text: &str) -> Result<(String, bool), String> {
        let q = text.to_lowercase();
        self.elements()?
            .iter()
            .find(|e| {
                e["role"] == "switch"
                    && [&e["label"], &e["hint"]]
                        .iter()
                        .any(|f| f.as_str().is_some_and(|s| s.to_lowercase().contains(&q)))
            })
            .map(|e| (e["ref"].as_str().unwrap_or_default().to_string(), is_on(e)))
            .ok_or_else(|| format!("no switch labelled {text:?} on screen"))
    }

    async fn expect(&self, e: &Expect) -> Result<(), String> {
        let started = Instant::now();
        loop {
            match self.check(e).await {
                Ok(()) => return Ok(()),
                Err(why) if started.elapsed() > EXPECT_WITHIN => return Err(why),
                Err(_) => tokio::time::sleep(Duration::from_millis(500)).await,
            }
        }
    }

    async fn check(&self, e: &Expect) -> Result<(), String> {
        if e.text.is_some() || e.absent.is_some() || e.element.is_some() {
            let shown = self.cli(&["snapshot"])?;
            if let Some(t) = &e.text
                && !t.all().iter().any(|t| shown.contains(*t))
            {
                return Err(format!("none of {:?} is on screen:\n{shown}", t.all()));
            }
            if let Some(a) = &e.absent
                && shown.contains(a.as_str())
            {
                return Err(format!("{a:?} is still on screen:\n{shown}"));
            }
            if let Some(target) = &e.element {
                let (texts, role) = match target {
                    Target::Plain(t) => (t.all(), None),
                    Target::Full { text, role } => (text.all(), role.as_deref()),
                };
                let els = self.elements()?;
                let found = els.iter().find(|el| {
                    role.is_none_or(|r| el["role"] == r)
                        && texts.iter().any(|t| {
                            let q = t.to_lowercase();
                            [&el["label"], &el["hint"], &el["value"]]
                                .iter()
                                .any(|f| f.as_str().is_some_and(|s| s.to_lowercase().contains(&q)))
                        })
                });
                let Some(el) = found else {
                    return Err(format!("no element {texts:?} with role {role:?}"));
                };
                if let Some(state) = &e.state {
                    let on = is_on(el);
                    if (state == "on") != on {
                        return Err(format!(
                            "{texts:?} is {}, expected {state}",
                            if on { "on" } else { "off" }
                        ));
                    }
                }
            }
        }
        if let Some(check) = &e.shell {
            let out = self.shell(&check.cmd).await?;
            if !out.contains(&check.contains) {
                return Err(format!(
                    "`{}` printed no {:?}: {}",
                    check.cmd,
                    check.contains,
                    out.trim()
                ));
            }
        }
        if let Some(pattern) = &e.log {
            let log = self.log().await;
            if !log.iter().any(|ev| matches_pattern(ev, pattern)) {
                return Err(format!(
                    "the testbed log has no event like {pattern}: {log:?}"
                ));
            }
        }
        if let Some(pattern) = &e.no_log {
            let log = self.log().await;
            if log.iter().any(|ev| matches_pattern(ev, pattern)) {
                return Err(format!("the testbed log has an event like {pattern}"));
            }
        }
        Ok(())
    }
}

fn is_on(el: &Value) -> bool {
    el["state"].as_str().is_some_and(|s| s.contains("CHECKED")) || el["value"] == "ON"
}

fn stop_daemon(home: &Path) {
    let socket = home.join("run").join("daemon.sock");
    let Ok(out) = Command::new("lsof").arg("-t").arg(&socket).output() else {
        return;
    };
    for pid in String::from_utf8_lossy(&out.stdout).split_whitespace() {
        let _ = Command::new("kill").arg(pid).status();
    }
}

fn matches_pattern(event: &Value, pattern: &Value) -> bool {
    let (Some(e), Some(p)) = (event.as_object(), pattern.as_object()) else {
        return false;
    };
    p.iter().all(|(k, want)| match (e.get(k), want) {
        (Some(Value::Number(a)), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Some(got), want) => got == want,
        (None, _) => false,
    })
}

fn flatten(steps: &[Step], api: u32) -> Vec<Step> {
    steps
        .iter()
        .flat_map(|s| match s {
            Step::IfApi {
                at_least,
                then,
                otherwise,
            } => flatten(if api >= *at_least { then } else { otherwise }, api),
            other => vec![other.clone()],
        })
        .collect()
}

struct Args {
    phone_use: PathBuf,
    serial: String,
    dir: PathBuf,
    only: Option<String>,
    runs: usize,
    out: Option<PathBuf>,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut it = args.iter();
    let (mut phone_use, mut serial, mut dir, mut only, mut runs, mut out) = (
        None,
        None,
        PathBuf::from("bench/scenarios/android"),
        None,
        1usize,
        None,
    );
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--phone-use" => phone_use = Some(PathBuf::from(value()?)),
            "--serial" => serial = Some(value()?),
            "--dir" => dir = PathBuf::from(value()?),
            "--only" => only = Some(value()?),
            "--runs" => runs = value()?.parse().map_err(|e| format!("--runs: {e}"))?,
            "--out" => out = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args {
        phone_use: phone_use.ok_or("--phone-use <binary> is required")?,
        serial: serial.ok_or("--serial <adb serial> is required")?,
        dir,
        only,
        runs,
        out,
    })
}

fn load(dir: &Path) -> Result<Vec<Scenario>, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|f| {
            let text = std::fs::read_to_string(f).map_err(|e| format!("{}: {e}", f.display()))?;
            serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", f.display()))
        })
        .collect()
}

pub async fn main(args: &[String]) -> i32 {
    let args = match parse(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let scenarios = match load(&args.dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let home = std::env::temp_dir().join(format!("phone-use-scenarios-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&home);
    let mut ctx = Ctx {
        bin: args.phone_use,
        serial: args.serial,
        home,
        adb: Adb::from_env(),
        originals: HashMap::new(),
    };
    let _ = ctx.cli(&["release"]);
    let api: u32 = ctx
        .shell("getprop ro.build.version.sdk")
        .await
        .ok()
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(0);
    let mut lines = Vec::new();
    let mut failed = 0;
    for run in 1..=args.runs {
        for s in scenarios
            .iter()
            .filter(|s| args.only.as_ref().is_none_or(|o| &s.name == o))
        {
            let started = Instant::now();
            ctx.originals.clear();
            let mut failure = None;
            for (i, step) in flatten(&s.steps, api).iter().enumerate() {
                if let Err(e) = ctx.run(step).await {
                    failure = Some(format!("step {}: {e}", i + 1));
                    break;
                }
            }
            for step in &flatten(&s.cleanup, api) {
                if let Err(e) = ctx.run(step).await {
                    eprintln!("  cleanup of {} failed: {e}", s.name);
                }
            }
            let ms = started.elapsed().as_millis() as u64;
            match &failure {
                None => println!("run {run} PASS {} {ms} ms", s.name),
                Some(why) => {
                    failed += 1;
                    println!(
                        "run {run} FAIL {} {ms} ms\n  {}",
                        s.name,
                        why.replace('\n', "\n  ")
                    );
                }
            }
            let _ = std::io::Write::flush(&mut std::io::stdout());
            lines.push(serde_json::json!({
                "run": run, "scenario": s.name, "pass": failure.is_none(), "ms": ms,
                "failure": failure,
            }));
        }
    }
    let _ = ctx.cli(&["release"]);
    stop_daemon(&ctx.home);
    if let Some(out) = args.out {
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        if let Err(e) = std::fs::write(&out, text) {
            eprintln!("cannot write {}: {e}", out.display());
        }
    }
    println!(
        "{} of {} scenario runs passed",
        lines.len() - failed,
        lines.len()
    );
    i32::from(failed > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plans_example_scenario_parses() {
        let yaml = r#"
name: settings-dark-theme
steps:
  - launch: com.android.settings
  - tap: "Display"
  - tap: {text: "Dark theme", role: switch}
  - tap: ["Wi-Fi", "Internet"]
  - type: {text: "hi", into: "plain"}
  - expect: {element: {text: "Dark theme", role: switch}, state: on}
  - expect: {log: {ev: toggle, on: true}}
cleanup:
  - shell: "cmd uimode night no"
"#;
        let s: Scenario = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(s.name, "settings-dark-theme");
        assert_eq!(s.steps.len(), 7);
        assert_eq!(s.cleanup.len(), 1);
        assert!(matches!(&s.steps[3], Step::Tap(Target::Plain(Texts::Many(v))) if v.len() == 2));
        assert!(
            matches!(&s.steps[2], Step::Tap(Target::Full { role: Some(r), .. }) if r == "switch")
        );
    }

    #[test]
    fn a_log_pattern_matches_a_subset_of_an_events_fields_and_numbers_by_value() {
        let ev: Value = serde_json::json!({"ev": "row", "n": 750, "extra": "x"});
        assert!(matches_pattern(
            &ev,
            &serde_json::json!({"ev": "row", "n": 750})
        ));
        assert!(!matches_pattern(
            &ev,
            &serde_json::json!({"ev": "row", "n": 751})
        ));
        assert!(!matches_pattern(&ev, &serde_json::json!({"missing": 1})));
    }

    #[test]
    fn an_unknown_step_is_a_parse_error_not_a_silent_skip() {
        let yaml = "name: x\nsteps:\n  - teleport: home\n";
        assert!(serde_yaml::from_str::<Scenario>(yaml).is_err());
    }
}
