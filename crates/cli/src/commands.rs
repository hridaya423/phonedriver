//! The CLI verbs: argument parsing, the request each one sends, and how the reply prints.

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::builder::BoolishValueParser;
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand};
use serde_json::{Value, json};

use crate::client::{Client, Failure};
use crate::daemon::android::AdbSource;
use crate::daemon::ios::SimSource;
use crate::daemon::sources::Sources;
use crate::daemon::trace::TraceOptions;
use crate::daemon::{Config, Daemon, socket};
use crate::wire::{ActionResult, DeviceEntry, Kind, Observation, ScreenshotResult, VERSION};

const USAGE: &str = "usage: phone-use <command>\n\ncommands:\n  devices\n  snapshot\n  screenshot\n  find\n  launch\n  open\n  apps\n  swipe\n  scroll\n  gesture\n  tap\n  type\n  clear\n  clipboard\n  press\n  wait\n  release\n  daemon\n  doctor\n  mcp\n  skill\n  version";

#[derive(Parser)]
#[command(name = "phone-use", disable_help_subcommand = true)]
pub struct Cli {
    /// The device id or serial; with one device connected it is the default.
    #[arg(long, global = true, env = "PHONE_USE_DEVICE")]
    device: Option<String>,
    /// Refs, diff baselines and the device lease belong to a session.
    #[arg(
        long,
        global = true,
        env = "PHONE_USE_SESSION",
        default_value = "default"
    )]
    session: String,
    /// Print the daemon's result as JSON.
    #[arg(long, global = true)]
    json: bool,
    /// Do not wait for the screen to settle.
    #[arg(long, global = true)]
    no_settle: bool,
    /// Settle timeout in milliseconds.
    #[arg(long, global = true)]
    timeout: Option<u64>,
    /// How long to wait for an action's first visible change, in milliseconds (default 500).
    #[arg(long, global = true)]
    expect_change: Option<u64>,
    /// Use a device another session holds.
    #[arg(long, global = true)]
    share: bool,
    #[command(subcommand)]
    command: Option<Verb>,
}

#[derive(Subcommand)]
pub(crate) enum Verb {
    /// List devices, tier, capabilities and leases.
    Devices,
    /// Print what is on screen, or what changed since the last observation.
    Snapshot {
        #[arg(long)]
        diff: bool,
        #[arg(long)]
        interactive: bool,
        #[arg(long)]
        within: Option<String>,
        #[arg(long)]
        max_lines: Option<usize>,
        /// Save the device's own tree and a screenshot as a test fixture.
        #[arg(long)]
        save_fixture: Option<String>,
        #[arg(long)]
        fixture_dir: Option<PathBuf>,
    },
    /// Save a screenshot at one pixel per point and print its path.
    Screenshot {
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Tap `@ref`, `x,y` in points, or the element with this text.
    Tap {
        target: String,
        /// With a text target, only an element with this role (`button`, `cell`, `textbox`, ...).
        #[arg(long)]
        role: Option<String>,
        /// Tap this many times, 80 ms apart.
        #[arg(long)]
        count: Option<u32>,
        /// Hold the press for this many milliseconds.
        #[arg(long)]
        long: Option<u32>,
    },
    /// Play fingers with timed paths, in points: `{"fingers": [[{"x": 100, "y": 300, "t_ms": 0}, {"x": 50, "y": 300, "t_ms": 300}]]}`.
    Gesture { spec: String },
    /// Open an app by name or package and show its first screen.
    Launch { app: String },
    /// Open a web link or deep link in the app that handles it.
    Open { url: String },
    /// List the apps that can be launched.
    Apps,
    /// Drag from the middle of the screen, or of `--from`, the way a finger does.
    Swipe {
        /// up, down, left or right: where the finger moves.
        direction: String,
        #[arg(long)]
        from: Option<String>,
        /// Share of the screen to cross, 0 to 1; the default is 0.5.
        #[arg(long)]
        distance: Option<f32>,
        #[arg(long)]
        fast: bool,
    },
    /// Scroll the content so more of it shows in `direction`, or until `--until` text shows.
    Scroll {
        direction: String,
        /// The element to scroll; the default is the largest scrollable one.
        #[arg(long = "in")]
        within: Option<String>,
        #[arg(long)]
        until: Option<String>,
        #[arg(long)]
        max: Option<u32>,
    },
    /// List the elements whose text contains this text, in any case.
    Find {
        text: String,
        #[arg(long)]
        role: Option<String>,
    },
    /// Type text into a field. Secrets come from stdin or PHONE_USE_SECRET, never from arguments.
    Type {
        text: Option<String>,
        #[arg(long)]
        into: Option<String>,
        #[arg(long, value_parser = ["paste", "set", "keys"])]
        mode: Option<String>,
        #[arg(long)]
        submit: bool,
        #[arg(long)]
        secret: bool,
    },
    /// Empty a text field: `@ref`, `x,y` or the field's text.
    Clear { target: String },
    /// Print the device clipboard, or set it with `clipboard set <text>`.
    Clipboard {
        #[arg(value_parser = ["get", "set"])]
        action: Option<String>,
        text: Option<String>,
    },
    /// Press a button or key: home, back, recents, notifications, lock, volume-up, volume-down, enter.
    Press { key: String },
    /// Wait for `settled`, for a text or `@ref` to appear, or for `gone:<text>` (or `gone:@ref`) to leave the screen.
    Wait { condition: String },
    /// Give up this session's lease on the device.
    Release,
    /// Run the daemon in the foreground, or detached.
    Daemon {
        #[arg(long)]
        detach: bool,
        /// Keep observation text and text targets in the trace files.
        #[arg(long, env = "PHONE_USE_TRACE_FULL", value_parser = BoolishValueParser::new())]
        trace_full: bool,
        /// Keep a copy of every screenshot next to the trace file.
        #[arg(long, env = "PHONE_USE_TRACE_SCREENS", value_parser = BoolishValueParser::new())]
        trace_screens: bool,
    },
    /// Check Xcode, booted simulators and their runners; each line says ok, warn or fail, with a fix.
    Doctor,
    /// Serve the tools over MCP on stdio.
    Mcp,
    /// Print SKILL.md, the instructions an agent reads before it uses phone-use.
    Skill,
    /// Print the version.
    Version,
}

pub fn run(cli: Cli) -> ExitCode {
    let Some(verb) = cli.command else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match verb {
        Verb::Version => {
            println!("phone-use {VERSION}");
            ExitCode::SUCCESS
        }
        Verb::Daemon {
            trace_full,
            trace_screens,
            ..
        } => run_daemon(TraceOptions {
            full: trace_full,
            screens: trace_screens,
        }),
        Verb::Mcp => crate::mcp::run(),
        Verb::Skill => {
            print!("{}", include_str!("../../../SKILL.md"));
            ExitCode::SUCCESS
        }
        Verb::Doctor => {
            if crate::doctor::run(&socket::home()) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        verb => {
            let secret = match secret_text(&verb) {
                Ok(s) => s,
                Err(message) => Cli::command()
                    .error(ErrorKind::ArgumentConflict, message)
                    .exit(),
            };
            let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                eprintln!("error: cannot start the async runtime");
                return ExitCode::from(1);
            };
            let globals = Globals {
                device: cli.device,
                json: cli.json,
                no_settle: cli.no_settle,
                timeout: cli.timeout,
                expect_change: cli.expect_change,
                share: cli.share,
            };
            match rt.block_on(execute(verb, secret, &cli.session, &globals)) {
                Ok(out) => {
                    println!("{out}");
                    ExitCode::SUCCESS
                }
                Err(f) => {
                    eprintln!("error: {}", f.message());
                    if let Some(h) = f.hint() {
                        eprintln!("hint: {h}");
                    }
                    ExitCode::from(f.exit_code())
                }
            }
        }
    }
}

pub(crate) struct Globals {
    pub device: Option<String>,
    pub json: bool,
    pub no_settle: bool,
    pub timeout: Option<u64>,
    pub expect_change: Option<u64>,
    pub share: bool,
}

/// The text of a `type --secret`, read before any connection is made so a usage error types nothing.
fn secret_text(verb: &Verb) -> Result<Option<String>, String> {
    let Verb::Type { text, secret, .. } = verb else {
        return Ok(None);
    };
    match (secret, text) {
        (true, Some(_)) => Err(
            "with --secret the text must come from stdin or PHONE_USE_SECRET, not from the command line (arguments show in `ps` and shell history)".into(),
        ),
        (false, None) => Err("type needs the text to type".into()),
        (true, None) => {
            let text = match std::env::var("PHONE_USE_SECRET") {
                Ok(t) => t,
                Err(_) => {
                    let mut buf = String::new();
                    std::io::stdin()
                        .read_to_string(&mut buf)
                        .map_err(|e| format!("cannot read the secret from stdin: {e}"))?;
                    buf
                }
            };
            let text = text
                .strip_suffix("\r\n")
                .or_else(|| text.strip_suffix('\n'))
                .unwrap_or(&text)
                .to_string();
            Ok(Some(text))
        }
        (false, Some(_)) => Ok(None),
    }
}

pub fn parse_target(s: &str) -> Value {
    let body = s.strip_prefix('@').unwrap_or(s);
    if let Some(n) = body.strip_prefix('e')
        && !n.is_empty()
        && n.chars().all(|c| c.is_ascii_digit())
    {
        return json!({"ref": body});
    }
    if let Some((x, y)) = s.split_once(',')
        && let (Ok(x), Ok(y)) = (x.trim().parse::<f32>(), y.trim().parse::<f32>())
    {
        return json!({"point": {"x": x, "y": y}});
    }
    json!({"text": s})
}

async fn execute(
    verb: Verb,
    secret: Option<String>,
    session: &str,
    g: &Globals,
) -> Result<String, Failure> {
    let mut client = Client::connect(session.to_string()).await?;
    execute_with(&mut client, verb, secret, g).await
}

pub(crate) async fn execute_with(
    client: &mut Client,
    verb: Verb,
    secret: Option<String>,
    g: &Globals,
) -> Result<String, Failure> {
    let with = |mut params: Value| {
        if let Some(d) = &g.device {
            params["device"] = json!(d);
        }
        if g.share {
            params["share"] = json!(true);
        }
        params
    };
    let act = |action: Value| {
        let mut p = json!({"action": action, "settle": !g.no_settle});
        if let Some(t) = g.timeout {
            p["timeout_ms"] = json!(t);
        }
        if let Some(t) = g.expect_change {
            p["expect_change_ms"] = json!(t);
        }
        with(p)
    };
    match verb {
        Verb::Devices => {
            let result = client.call("devices", json!({})).await?;
            if g.json {
                return Ok(result.to_string());
            }
            Ok(device_table(&result))
        }
        Verb::Snapshot {
            diff,
            interactive,
            within,
            max_lines,
            save_fixture,
            fixture_dir,
        } => {
            let mut p = json!({
                "diff": diff, "interactive": interactive, "max_lines": max_lines,
                "no_settle": g.no_settle, "elements": g.json, "capture": save_fixture.is_some(),
            });
            if let Some(w) = within {
                p["within"] = json!(w.trim_start_matches('@'));
            }
            if let Some(t) = g.timeout {
                p["timeout_ms"] = json!(t);
            }
            let result = client.call("snapshot", with(p)).await?;
            let obs: Observation = decode(&result)?;
            if let Some(name) = save_fixture {
                save_fixture_files(client, &with(json!({})), &obs, &name, fixture_dir).await?;
            }
            if g.json {
                return Ok(result.to_string());
            }
            Ok(if obs.text.is_empty() {
                "no change".to_string()
            } else {
                obs.text
            })
        }
        Verb::Screenshot { output } => {
            let result = client.call("screenshot", with(json!({}))).await?;
            let shot: ScreenshotResult = decode(&result)?;
            let path = match output {
                Some(out) => {
                    std::fs::copy(&shot.path, &out).map_err(|e| {
                        Failure::Local(format!("cannot write {}: {e}", out.display()))
                    })?;
                    out.display().to_string()
                }
                None => shot.path.clone(),
            };
            if g.json {
                return Ok(result.to_string());
            }
            Ok(format!("{path} {}x{}", shot.width, shot.height))
        }
        Verb::Tap {
            target,
            role,
            count,
            long,
        } => {
            let mut t = parse_target(&target);
            if let Some(role) = role {
                t["role"] = role_json(&role)?;
            }
            let mut a = json!({"kind": "tap", "target": t});
            if let Some(c) = count {
                a["count"] = json!(c);
            }
            if let Some(l) = long {
                a["long"] = json!(l);
            }
            let r = client.call("act", act(a)).await?;
            show_action(&r, g.json)
        }
        Verb::Gesture { spec } => {
            let mut a: Value = serde_json::from_str(&spec)
                .map_err(|e| Failure::Local(format!("the gesture is not JSON: {e}")))?;
            a["kind"] = json!("gesture");
            let r = client.call("act", act(a)).await?;
            show_action(&r, g.json)
        }
        Verb::Swipe {
            direction,
            from,
            distance,
            fast,
        } => {
            let mut a = json!({"kind": "swipe", "direction": direction, "fast": fast});
            if let Some(f) = from {
                a["from"] = parse_target(&f);
            }
            if let Some(d) = distance {
                a["distance"] = json!(d);
            }
            let r = client.call("act", act(a)).await?;
            show_action(&r, g.json)
        }
        Verb::Scroll {
            direction,
            within,
            until,
            max,
        } => {
            let mut a = json!({"kind": "scroll", "direction": direction});
            if let Some(w) = within {
                a["in"] = parse_target(&w);
            }
            if let Some(u) = until {
                a["until"] = parse_target(&u);
            }
            if let Some(m) = max {
                a["max"] = json!(m);
            }
            let r = client.call("act", act(a)).await?;
            show_action(&r, g.json)
        }
        Verb::Launch { app } => {
            let r = client
                .call("act", act(json!({"kind": "launch", "app": app})))
                .await?;
            show_action(&r, g.json)
        }
        Verb::Open { url } => {
            let r = client
                .call("act", act(json!({"kind": "open", "url": url})))
                .await?;
            show_action(&r, g.json)
        }
        Verb::Apps => {
            let r = client.call("apps", with(json!({}))).await?;
            if g.json {
                return Ok(r.to_string());
            }
            let apps = r.as_array().cloned().unwrap_or_default();
            Ok(apps
                .iter()
                .map(|a| {
                    format!(
                        "{}  {}",
                        a["name"].as_str().unwrap_or_default(),
                        a["id"].as_str().unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
        Verb::Find { text, role } => {
            let mut p = json!({"text": text});
            if let Some(role) = role {
                p["role"] = role_json(&role)?;
            }
            let r = client.call("find", with(p)).await?;
            if g.json {
                return Ok(r.to_string());
            }
            let lines: Vec<&str> = r
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            Ok(if lines.is_empty() {
                "no match".to_string()
            } else {
                lines.join("\n")
            })
        }
        Verb::Type {
            text,
            into,
            mode,
            submit,
            secret: is_secret,
        } => {
            let text = secret.or(text).unwrap_or_default();
            let mut action =
                json!({"kind": "type", "text": text, "secret": is_secret, "submit": submit});
            if let Some(i) = into {
                action["into"] = parse_target(&i);
            }
            if let Some(m) = mode {
                action["mode"] = json!(m);
            }
            let r = client.call("act", act(action)).await?;
            show_action(&r, g.json)
        }
        Verb::Clear { target } => {
            let r = client
                .call(
                    "act",
                    act(json!({"kind": "clear", "target": parse_target(&target)})),
                )
                .await?;
            show_action(&r, g.json)
        }
        Verb::Clipboard { action, text } => {
            let mut p = json!({});
            if action.as_deref() == Some("set") {
                let text = text
                    .ok_or_else(|| Failure::Local("clipboard set needs the text to set".into()))?;
                p["set"] = json!(text);
            }
            let r = client.call("clipboard", with(p)).await?;
            if g.json {
                return Ok(r.to_string());
            }
            Ok(match r["text"].as_str() {
                Some(t) => t.to_string(),
                None if action.as_deref() == Some("set") => "ok clipboard set".into(),
                None => String::new(),
            })
        }
        Verb::Press { key } => {
            let r = client
                .call("act", act(json!({"kind": "press", "key": key})))
                .await?;
            show_action(&r, g.json)
        }
        Verb::Wait { condition } => {
            let condition = match condition.strip_prefix("gone:") {
                Some(rest) => json!({"gone": parse_target(rest)}),
                None if condition == "settled" => json!({"settled": true}),
                None => json!({"appears": parse_target(&condition)}),
            };
            let mut p = json!({"condition": condition});
            if let Some(t) = g.timeout {
                p["timeout_ms"] = json!(t);
            }
            if let Some(t) = g.expect_change {
                p["expect_change_ms"] = json!(t);
            }
            let r = client.call("wait", with(p)).await?;
            show_action(&r, g.json)
        }
        Verb::Release => {
            let r = client.call("release", with(json!({}))).await?;
            Ok(if g.json {
                r.to_string()
            } else {
                "released".into()
            })
        }
        Verb::Daemon { .. } | Verb::Doctor | Verb::Mcp | Verb::Skill | Verb::Version => {
            Ok(String::new())
        }
    }
}

fn role_json(word: &str) -> Result<Value, Failure> {
    phone_use_core::element::Role::from_word(word)
        .and_then(|r| serde_json::to_value(r).ok())
        .ok_or_else(|| Failure::Local(format!("unknown role {word:?}; use a word from a snapshot line such as button, cell, textbox or switch")))
}

fn decode<T: serde::de::DeserializeOwned>(v: &Value) -> Result<T, Failure> {
    serde_json::from_value(v.clone())
        .map_err(|e| Failure::Local(format!("the daemon sent an unexpected reply: {e}")))
}

fn device_table(result: &Value) -> String {
    let Ok(devices) = serde_json::from_value::<Vec<DeviceEntry>>(result.clone()) else {
        return result.to_string();
    };
    let mut rows = vec![["ID", "MODEL", "OS", "TIER", "LEASE"].map(String::from)];
    for d in devices {
        rows.push([
            d.id,
            d.model,
            if d.os.is_empty() { "-".into() } else { d.os },
            d.tier,
            d.leased_by.unwrap_or_else(|| "-".into()),
        ]);
    }
    let widths: Vec<usize> = (0..5)
        .map(|c| rows.iter().map(|r| r[c].chars().count()).max().unwrap_or(0))
        .collect();
    rows.iter()
        .map(|r| {
            r.iter()
                .zip(&widths)
                .map(|(cell, w)| format!("{cell:<w$}"))
                .collect::<Vec<_>>()
                .join("  ")
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn show_action(result: &Value, json: bool) -> Result<String, Failure> {
    if json {
        return Ok(result.to_string());
    }
    let r: ActionResult = decode(result)?;
    let mut head = format!("ok {}", r.summary);
    match r.verified {
        Some(true) => head.push_str(" · verified"),
        Some(false) => head.push_str(" · NOT verified"),
        None => {}
    }
    if r.settle_ms > 0 || r.settled {
        if !r.settled {
            head.push_str(&format!(" · not settled after {}ms", r.settle_ms));
            if let Some(b) = r.observation.still_changing {
                head.push_str(&format!(
                    " (still changing at {},{} {}x{})",
                    b.x.round(),
                    b.y.round(),
                    b.w.round(),
                    b.h.round()
                ));
            }
        } else if !r.changed {
            head.push_str(&format!(" · no change after {}ms", r.settle_ms));
        } else {
            head.push_str(&format!(" · settled in {}ms", r.settle_ms));
            if r.observation.kind == Kind::Full {
                head.push_str(" · new screen");
            }
        }
    }
    if r.woke {
        head.push_str(" · woke the screen first");
    }
    if r.unlocked {
        head.push_str(" · swiped the lock screen away first");
    }
    let mut out = head;
    if !r.observation.text.is_empty() {
        out.push('\n');
        out.push_str(&r.observation.text);
    }
    if r.settled && !r.changed {
        out.push_str("\nhint: nothing changed; `phone-use wait settled` waits for the screen, `phone-use snapshot` shows it");
    }
    if r.verified == Some(false) && r.via.as_deref() == Some("paste") {
        out.push_str("\nhint: the field does not show the text; retry with `--mode keys`");
    }
    Ok(out)
}

async fn save_fixture_files(
    client: &mut Client,
    common: &Value,
    obs: &Observation,
    name: &str,
    dir: Option<PathBuf>,
) -> Result<(), Failure> {
    let id = obs
        .header
        .device
        .as_ref()
        .map(|d| d.id.clone())
        .unwrap_or_default();
    let platform = match id.split(':').next() {
        Some("android") => "android",
        _ => "ios",
    };
    let dir = dir.unwrap_or_else(|| PathBuf::from("crates/core/tests/fixtures").join(platform));
    let tree = obs
        .capture
        .as_deref()
        .ok_or_else(|| Failure::Local("this device cannot capture its tree".into()))?;
    std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::write(dir.join(format!("{name}.json")), tree))
        .map_err(|e| Failure::Local(format!("cannot write the fixture: {e}")))?;
    let shot: ScreenshotResult = decode(&client.call("screenshot", common.clone()).await?)?;
    std::fs::copy(&shot.path, dir.join(format!("{name}.png")))
        .map_err(|e| Failure::Local(format!("cannot write the fixture screenshot: {e}")))?;
    Ok(())
}

fn run_daemon(trace: TraceOptions) -> ExitCode {
    let Ok(rt) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    else {
        eprintln!("error: cannot start the async runtime");
        return ExitCode::from(1);
    };
    rt.block_on(async {
        let sources = Sources(vec![
            ("android:", Arc::new(AdbSource::from_env())),
            ("sim:", Arc::new(SimSource::new(socket::home()))),
        ]);
        let daemon = Daemon::with_trace(
            Arc::new(sources),
            Config {
                home: socket::home(),
            },
            trace,
        );
        let listener = match socket::listen(&socket::socket_path()).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!(
                    "error: cannot listen on {}: {e}",
                    socket::socket_path().display()
                );
                return ExitCode::from(1);
            }
        };
        println!("ready {}", std::process::id());
        match daemon.serve_on(listener).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(1)
            }
        }
    })
}
