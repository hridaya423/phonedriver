//! Scenario runner: `phonedriver-bench scenario --phonedriver <binary> --device <android:serial|sim:udid>
//! [--dir bench/scenarios/<android|ios>] [--only name] [--runs 3] [--out results.jsonl]`.
//!
//! A scenario is a list of verbs and expectations run through the real CLI, without an LLM (task 3.11). Steps run
//! in order and stop at the first failure; `cleanup` always runs. One schema serves both platforms and the device id
//! picks the platform. `shell` is `adb shell` on Android and a host command on an iOS Simulator (`{udid}` is
//! replaced); `testbed` and the testbed log are read through logcat on Android and the app container on iOS.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use phonedriver_android::adb::Adb;
use serde::Deserialize;
use serde_json::Value;

const EXPECT_WITHIN: Duration = Duration::from_secs(8);
const PACKAGE: &str = "dev.phonedriver.testbed";

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
    Full {
        text: Option<Texts>,
        role: Option<String>,
        contains: Option<String>,
        id: Option<String>,
        selected: Option<bool>,
    },
}

impl Target {
    fn texts(&self) -> Vec<&str> {
        match self {
            Target::Plain(t) => t.all(),
            Target::Full { text, .. } => text.as_ref().map(Texts::all).unwrap_or_default(),
        }
    }

    fn role(&self) -> Option<&str> {
        match self {
            Target::Plain(_) => None,
            Target::Full { role, .. } => role.as_deref(),
        }
    }

    /// True when the CLI's own text target (with `--role`) says it all.
    fn is_text_only(&self) -> bool {
        match self {
            Target::Plain(_) => true,
            Target::Full {
                text: Some(_),
                contains: None,
                id: None,
                selected: None,
                ..
            } => true,
            Target::Full { .. } => false,
        }
    }

    fn matches(&self, el: &Value) -> bool {
        let texts = self.texts();
        let Target::Full {
            contains,
            id,
            selected,
            ..
        } = self
        else {
            return self.role().is_none_or(|r| el["role"] == r) && any_field_has(el, &texts);
        };
        self.role().is_none_or(|r| el["role"] == r)
            && id.as_deref().is_none_or(|i| el["id"] == i)
            && selected
                .is_none_or(|w| el["state"].as_str().is_some_and(|s| s.contains("SELECTED")) == w)
            && contains.as_deref().is_none_or(|c| {
                [&el["label"], &el["value"]]
                    .iter()
                    .any(|f| f.as_str().is_some_and(|s| s.contains(c)))
            })
            && (texts.is_empty() || any_field_has(el, &texts))
    }
}

fn any_field_has(el: &Value, texts: &[&str]) -> bool {
    texts.iter().any(|t| {
        let q = t.to_lowercase();
        [&el["label"], &el["hint"], &el["value"]]
            .iter()
            .any(|f| f.as_str().is_some_and(|s| s.to_lowercase().contains(&q)))
    })
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
        into: Option<Target>,
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
    ClearLog,
    SetSwitch {
        text: String,
        state: String,
    },
    RestoreSwitch(String),
    Dismiss(Vec<String>),
    Script(String),
    Expect(Box<Expect>),
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
    answer: Option<String>,
    script: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize, Clone)]
struct ShellCheck {
    cmd: String,
    contains: String,
}

enum Platform {
    Android { adb: Adb, serial: String },
    Ios { udid: String },
}

struct Ctx {
    bin: PathBuf,
    device: String,
    platform: Platform,
    home: PathBuf,
    originals: HashMap<String, bool>,
    answer: String,
    files: JobFiles,
}

#[derive(Default)]
struct JobFiles {
    dir: PathBuf,
    answer: Option<String>,
    transcript: Option<String>,
}

impl Ctx {
    fn cli(&self, args: &[&str]) -> Result<String, String> {
        let out = Command::new(&self.bin)
            .env("PHONEDRIVER_HOME", &self.home)
            .env("PHONEDRIVER_SESSION", format!("scenario-{}", self.device))
            .arg("--device")
            .arg(&self.device)
            .args(args)
            .output()
            .map_err(|e| format!("cannot run {}: {e}", self.bin.display()))?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        if out.status.success() {
            Ok(stdout)
        } else {
            Err(format!(
                "phonedriver {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }

    fn script(&self, cmd: &str) -> Result<(), String> {
        let Platform::Android { serial, .. } = &self.platform else {
            return Err("script steps run on Android only".into());
        };
        let mut c = Command::new("sh");
        c.args(["-c", cmd])
            .current_dir(&self.files.dir)
            .env("SERIAL", serial)
            .env("PHONEDRIVER_BIN", &self.bin)
            .env("PHONEDRIVER_DEVICE", &self.device);
        if let Some(a) = &self.files.answer {
            c.env("ANSWER_FILE", a);
        }
        if let Some(t) = &self.files.transcript {
            c.env("TRANSCRIPT_FILE", t);
        }
        let out = c.output().map_err(|e| format!("cannot run `{cmd}`: {e}"))?;
        if out.status.success() {
            return Ok(());
        }
        Err(format!(
            "`{cmd}` failed: {}{}",
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }

    async fn shell(&self, cmd: &str) -> Result<String, String> {
        match &self.platform {
            Platform::Android { adb, serial } => adb
                .shell(serial, cmd)
                .await
                .map(|o| o.stdout_text())
                .map_err(|e| format!("adb shell {cmd}: {e}")),
            Platform::Ios { udid } => {
                let cmd = cmd.replace("{udid}", udid);
                let out = Command::new("sh")
                    .args(["-c", &cmd])
                    .output()
                    .map_err(|e| format!("{cmd}: {e}"))?;
                if out.status.success() {
                    Ok(String::from_utf8_lossy(&out.stdout).to_string())
                } else {
                    Err(format!(
                        "{cmd}: {}",
                        String::from_utf8_lossy(&out.stderr).trim()
                    ))
                }
            }
        }
    }

    fn simctl(&self, udid: &str, args: &[&str]) -> Result<String, String> {
        let out = Command::new("xcrun")
            .arg("simctl")
            .args(args)
            .output()
            .map_err(|e| format!("simctl {udid}: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            Err(format!(
                "simctl {}: {}",
                args[0],
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }

    fn ios_log_path(&self, udid: &str) -> Result<PathBuf, String> {
        let data = self.simctl(udid, &["get_app_container", udid, PACKAGE, "data"])?;
        Ok(PathBuf::from(data).join("Documents/putb.log"))
    }

    async fn clear_log(&self) -> Result<(), String> {
        match &self.platform {
            Platform::Android { .. } => self.shell("logcat -c").await.map(drop),
            Platform::Ios { udid } => {
                let path = self.ios_log_path(udid)?;
                if path.exists() {
                    std::fs::OpenOptions::new()
                        .write(true)
                        .open(&path)
                        .and_then(|f| f.set_len(0))
                        .map_err(|e| format!("{}: {e}", path.display()))?;
                }
                Ok(())
            }
        }
    }

    async fn testbed(&self, tab: &str) -> Result<(), String> {
        match &self.platform {
            Platform::Android { .. } => {
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
                self.clear_log().await
            }
            Platform::Ios { udid } => {
                let _ = self.simctl(udid, &["terminate", udid, PACKAGE]);
                self.simctl(udid, &["launch", udid, PACKAGE, "-tab", tab])?;
                let want = serde_json::json!({"ev": "tab", "name": tab});
                let started = Instant::now();
                while !self.log().await.iter().any(|e| matches_pattern(e, &want)) {
                    if started.elapsed() > EXPECT_WITHIN {
                        return Err(format!("the testbed did not log the {tab} tab"));
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                self.clear_log().await
            }
        }
    }

    fn target_arg(&self, target: &Target) -> Result<String, String> {
        if let (true, [text]) = (target.is_text_only(), target.texts().as_slice()) {
            return Ok(text.to_string());
        }
        let els = self.elements()?;
        let found: Vec<&Value> = els.iter().filter(|e| target.matches(e)).collect();
        match found.as_slice() {
            [one] => Ok(format!("@{}", one["ref"].as_str().unwrap_or_default())),
            _ => Err(format!(
                "{} elements match {:?} with role {:?}",
                found.len(),
                target.texts(),
                target.role()
            )),
        }
    }

    fn elements(&self) -> Result<Vec<Value>, String> {
        let out = self.cli(&["--json", "snapshot", "--no-settle"])?;
        let v: Value = serde_json::from_str(&out).map_err(|e| e.to_string())?;
        Ok(v["elements"].as_array().cloned().unwrap_or_default())
    }

    async fn log(&self) -> Vec<Value> {
        let out = match &self.platform {
            Platform::Android { .. } => self
                .shell("logcat -d -s PUTB:I -v raw")
                .await
                .unwrap_or_default(),
            Platform::Ios { udid } => self
                .ios_log_path(udid)
                .and_then(|p| std::fs::read_to_string(p).map_err(|e| e.to_string()))
                .unwrap_or_default(),
        };
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
                let into = into.as_ref().map(|t| self.target_arg(t)).transpose()?;
                if let Some(i) = &into {
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
            Step::Testbed(tab) => self.testbed(tab).await,
            Step::ClearLog => self.clear_log().await,
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
            Step::Script(cmd) => self.script(cmd),
            Step::Expect(e) => self.expect(e).await,
            Step::IfApi { .. } => Err("IfApi must be flattened before running".into()),
        }
    }

    fn tap(&self, target: &Target) -> Result<(), String> {
        if !target.is_text_only() {
            return self.cli(&["tap", &self.target_arg(target)?]).map(drop);
        }
        let mut last = String::new();
        for t in target.texts() {
            let mut args = vec!["tap", t];
            if let Some(r) = target.role() {
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
        let within = e.timeout_ms.map_or(EXPECT_WITHIN, Duration::from_millis);
        loop {
            match self.check(e).await {
                Ok(()) => return Ok(()),
                Err(why) if started.elapsed() > within => return Err(why),
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
                let els = self.elements()?;
                let Some(el) = els.iter().find(|el| target.matches(el)) else {
                    return Err(format!(
                        "no element {:?} with role {:?}",
                        target.texts(),
                        target.role()
                    ));
                };
                let texts = target.texts();
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
        if let Some(a) = &e.answer
            && !self.answer.contains(a.as_str())
        {
            return Err(format!("the agent's answer has no {a:?}: {}", self.answer));
        }
        if let Some(cmd) = &e.script {
            self.script(cmd)?;
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
    phonedriver: PathBuf,
    device: String,
    dir: Option<PathBuf>,
    only: Option<String>,
    runs: usize,
    out: Option<PathBuf>,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut it = args.iter();
    let (mut phonedriver, mut device, mut dir, mut only, mut runs, mut out) =
        (None, None, None, None, 1usize, None);
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--phonedriver" => phonedriver = Some(PathBuf::from(value()?)),
            "--device" => device = Some(value()?),
            "--dir" => dir = Some(PathBuf::from(value()?)),
            "--only" => only = Some(value()?),
            "--runs" => runs = value()?.parse().map_err(|e| format!("--runs: {e}"))?,
            "--out" => out = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args {
        phonedriver: phonedriver.ok_or("--phonedriver <binary> is required")?,
        device: device.ok_or("--device <android:serial|sim:udid> is required")?,
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

#[derive(Deserialize)]
struct Tasks {
    tasks: Vec<Task>,
}

#[derive(Deserialize)]
pub struct Task {
    pub name: String,
    pub set: String,
    pub prompt: String,
    #[serde(default, with = "serde_yaml::with::singleton_map_recursive")]
    reset: Vec<Step>,
    #[serde(default, with = "serde_yaml::with::singleton_map_recursive")]
    check: Vec<Step>,
    #[serde(default, with = "serde_yaml::with::singleton_map_recursive")]
    cleanup: Vec<Step>,
}

pub fn load_tasks(path: &Path) -> Result<Vec<Task>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_yaml::from_str::<Tasks>(&text)
        .map(|t| t.tasks)
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// `phonedriver-bench eval list|prompt|reset|check|cleanup ...` for `bench/agent-eval/run.sh`.
/// `list <tasks.yaml> <set>` prints task names; `prompt <tasks.yaml> <name>` prints a prompt;
/// `reset|check|cleanup <tasks.yaml> <name> <phonedriver> <device> [answer-file]` runs that part of a task and exits 1 on failure.
pub async fn eval_main(args: &[String]) -> i32 {
    let [verb, file, rest @ ..] = args else {
        eprintln!("usage: eval list|prompt|reset|check|cleanup <tasks.yaml> ...");
        return 2;
    };
    let tasks = match load_tasks(Path::new(file)) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    match (verb.as_str(), rest) {
        ("list", [set]) => {
            tasks
                .iter()
                .filter(|t| &t.set == set)
                .for_each(|t| println!("{}", t.name));
            0
        }
        ("prompt", [name]) => match tasks.iter().find(|t| &t.name == name) {
            Some(t) => {
                println!("{}", t.prompt);
                0
            }
            None => 2,
        },
        (phase @ ("reset" | "check" | "cleanup"), [name, bin, device, tail @ ..]) => {
            let Some(task) = tasks.iter().find(|t| &t.name == name) else {
                eprintln!("no task {name}");
                return 2;
            };
            let serial = device
                .strip_prefix("android:")
                .unwrap_or(device)
                .to_string();
            let home =
                std::env::temp_dir().join(format!("phonedriver-eval-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&home);
            let mut ctx = Ctx {
                bin: PathBuf::from(bin),
                device: format!("android:{serial}"),
                platform: Platform::Android {
                    adb: Adb::from_env(),
                    serial,
                },
                home,
                originals: HashMap::new(),
                answer: tail
                    .first()
                    .and_then(|f| std::fs::read_to_string(f).ok())
                    .unwrap_or_default(),
                files: JobFiles {
                    dir: Path::new(file)
                        .parent()
                        .map(Path::to_path_buf)
                        .unwrap_or_default(),
                    answer: tail.first().cloned(),
                    transcript: tail.get(1).cloned(),
                },
            };
            let steps = match phase {
                "reset" => &task.reset,
                "check" => &task.check,
                _ => &task.cleanup,
            };
            let mut code = 0;
            for (i, step) in steps.iter().enumerate() {
                if let Err(e) = ctx.run(step).await {
                    println!("{phase} step {}: {e}", i + 1);
                    code = 1;
                    break;
                }
            }
            let _ = ctx.cli(&["release"]);
            stop_daemon(&ctx.home);
            code
        }
        _ => {
            eprintln!("bad eval arguments");
            2
        }
    }
}

/// Totals for one `claude -p --output-format stream-json` transcript.
pub fn transcript_stats(jsonl: &str) -> Value {
    let (mut calls, mut result) = (0u64, Value::Null);
    for line in jsonl.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match v["type"].as_str() {
            Some("assistant") => {
                calls += v["message"]["content"].as_array().map_or(0, |c| {
                    c.iter().filter(|b| b["type"] == "tool_use").count() as u64
                })
            }
            Some("result") => result = v,
            _ => {}
        }
    }
    let u = &result["usage"];
    let n = |k: &str| u[k].as_u64().unwrap_or(0);
    serde_json::json!({
        "tool_calls": calls,
        "turns": result["num_turns"],
        "input_tokens": n("input_tokens"),
        "output_tokens": n("output_tokens"),
        "cache_creation_tokens": n("cache_creation_input_tokens"),
        "cache_read_tokens": n("cache_read_input_tokens"),
        "total_tokens": n("input_tokens") + n("output_tokens") + n("cache_creation_input_tokens") + n("cache_read_input_tokens"),
        "cost_usd": result["total_cost_usd"],
        "duration_ms": result["duration_ms"],
        "is_error": result["is_error"],
        "answer": result["result"],
    })
}

pub async fn main(args: &[String]) -> i32 {
    let args = match parse(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let platform = match args.device.strip_prefix("sim:") {
        Some(udid) => Platform::Ios {
            udid: udid.to_string(),
        },
        None => Platform::Android {
            adb: Adb::from_env(),
            serial: args
                .device
                .strip_prefix("android:")
                .unwrap_or(&args.device)
                .to_string(),
        },
    };
    let dir = args.dir.unwrap_or_else(|| match platform {
        Platform::Android { .. } => PathBuf::from("bench/scenarios/android"),
        Platform::Ios { .. } => PathBuf::from("bench/scenarios/ios"),
    });
    let scenarios = match load(&dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let shared_home = std::env::var_os("PHONEDRIVER_HOME").map(PathBuf::from);
    let home = shared_home.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("phonedriver-scenarios-{}", std::process::id()))
    });
    let _ = std::fs::create_dir_all(&home);
    let device = match &platform {
        Platform::Android { serial, .. } => format!("android:{serial}"),
        Platform::Ios { udid } => format!("sim:{udid}"),
    };
    let mut ctx = Ctx {
        bin: args.phonedriver,
        device,
        platform,
        home,
        originals: HashMap::new(),
        answer: String::new(),
        files: JobFiles::default(),
    };
    let _ = ctx.cli(&["release"]);
    let api: u32 = match ctx.platform {
        Platform::Android { .. } => ctx
            .shell("getprop ro.build.version.sdk")
            .await
            .ok()
            .and_then(|t| t.trim().parse().ok())
            .unwrap_or(0),
        Platform::Ios { .. } => 0,
    };
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
    if shared_home.is_none() {
        stop_daemon(&ctx.home);
    }
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
    fn the_shipped_agent_eval_tasks_parse_five_and_five() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("agent-eval/tasks.yaml");
        let tasks = load_tasks(&path).unwrap();
        assert_eq!(tasks.iter().filter(|t| t.set == "tuning").count(), 5);
        assert_eq!(tasks.iter().filter(|t| t.set == "heldout").count(), 5);
        assert!(
            tasks
                .iter()
                .all(|t| !t.check.is_empty() && !t.reset.is_empty())
        );
    }

    fn script_ctx(dir: &Path) -> Ctx {
        Ctx {
            bin: PathBuf::from("phonedriver"),
            device: "android:emulator-5558".into(),
            platform: Platform::Android {
                adb: Adb::from_env(),
                serial: "emulator-5558".into(),
            },
            home: std::env::temp_dir(),
            originals: HashMap::new(),
            answer: String::new(),
            files: JobFiles {
                dir: dir.to_path_buf(),
                answer: Some("a.txt".into()),
                transcript: Some("t.jsonl".into()),
            },
        }
    }

    #[test]
    fn a_script_step_gets_the_device_and_the_run_files_and_its_failure_text_comes_back() {
        let ctx = script_ctx(&std::env::temp_dir());
        ctx.script(
            r#"test "$SERIAL $ANSWER_FILE $TRANSCRIPT_FILE" = "emulator-5558 a.txt t.jsonl""#,
        )
        .unwrap();
        let err = ctx.script("echo no cart total >&2; exit 1").unwrap_err();
        assert!(err.contains("no cart total"), "{err}");
    }

    #[test]
    fn a_script_expect_and_step_parse() {
        let y = "name: x\nsteps:\n  - script: \"seed.sh\"\n  - expect: {script: \"check.sh\"}\n";
        let s: Scenario = serde_yaml::from_str(y).unwrap();
        assert!(matches!(&s.steps[0], Step::Script(c) if c == "seed.sh"));
        assert!(matches!(&s.steps[1], Step::Expect(e) if e.script.as_deref() == Some("check.sh")));
    }

    #[test]
    fn the_jobs_sets_have_seven_ui_only_and_two_mixed_tasks_with_a_check_script_each() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("agent-eval");
        let tasks = load_tasks(&dir.join("tasks.yaml")).unwrap();
        assert_eq!(tasks.iter().filter(|t| t.set == "jobs").count(), 7);
        assert_eq!(tasks.iter().filter(|t| t.set == "jobs-mixed").count(), 2);
        assert!(dir.join("jobs/jobs.py").exists());
        for t in tasks.iter().filter(|t| t.set.starts_with("jobs")) {
            assert!(
                t.check
                    .iter()
                    .any(|s| matches!(s, Step::Expect(e) if e.script.as_deref().is_some_and(|c| c.ends_with(&t.name))))
            );
        }
    }

    #[test]
    fn transcript_stats_count_tool_calls_and_read_the_result_event() {
        let jsonl = r#"{"type":"system"}
{"type":"assistant","message":{"content":[{"type":"text","text":"x"},{"type":"tool_use","name":"a"},{"type":"tool_use","name":"b"}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"c"}]}}
{"type":"result","num_turns":4,"duration_ms":900,"total_cost_usd":0.5,"is_error":false,"result":"done","usage":{"input_tokens":10,"output_tokens":20,"cache_creation_input_tokens":30,"cache_read_input_tokens":40}}"#;
        let s = transcript_stats(jsonl);
        assert_eq!(s["tool_calls"], 3);
        assert_eq!(s["total_tokens"], 100);
        assert_eq!(s["answer"], "done");
        assert_eq!(s["duration_ms"], 900);
    }

    #[test]
    fn an_unknown_step_is_a_parse_error_not_a_silent_skip() {
        let yaml = "name: x\nsteps:\n  - teleport: home\n";
        assert!(serde_yaml::from_str::<Scenario>(yaml).is_err());
    }

    #[test]
    fn every_shipped_scenario_parses() {
        for platform in ["android", "ios"] {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("scenarios")
                .join(platform);
            let scenarios = load(&dir).unwrap();
            assert!(!scenarios.is_empty(), "{platform}");
        }
    }

    #[test]
    fn a_target_matches_by_id_role_and_a_case_sensitive_contains() {
        let el = serde_json::json!({"role": "text_field", "label": "Address", "value": "example.com", "id": "url"});
        let t: Target = serde_yaml::from_str("{text: Address, contains: example.com}").unwrap();
        assert!(t.matches(&el));
        let t: Target = serde_yaml::from_str("{id: url, role: text_field}").unwrap();
        assert!(t.matches(&el));
        let t: Target = serde_yaml::from_str("{contains: Example}").unwrap();
        assert!(!t.matches(&el));
    }
}
