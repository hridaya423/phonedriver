//! The CLI verbs: argument parsing, the request each one sends, and how the reply prints.

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use base64::Engine;
use clap::builder::BoolishValueParser;
use clap::{Parser, Subcommand};
use serde_json::{Value, json};

use crate::client::{Client, Failure};
#[cfg(feature = "android")]
use crate::daemon::android::AdbSource;
use crate::daemon::ios::{PhysicalSource, SimSource};
use crate::daemon::sources::Sources;
use crate::daemon::trace::TraceOptions;
use crate::daemon::{Config, Daemon, socket};
use phone_use_core::error::span;

use crate::wire::{
    ActionResult, BatchResult, DeviceEntry, Kind, Observation, ScreenshotResult, VERSION,
};

const USAGE: &str = "usage: phone-use <command>\n\nrun `phone-use --help` for the options and exit codes\n\ncommands:\n  devices\n  snapshot\n  screenshot\n  find\n  launch\n  open\n  apps\n  swipe\n  drag\n  pinch\n  scroll\n  gesture\n  tap\n  type\n  clear\n  clipboard\n  press\n  wait\n  batch\n  release\n  daemon\n  doctor\n  ios\n  mcp\n  skill\n  version";

const AFTER_HELP: &str = "Examples:
  phone-use devices
  phone-use snapshot
  phone-use tap \"Dark theme\"
  phone-use scroll down --until \"About\"

Exit codes:
  0  done
  1  the command failed (the message says why)
  2  the command line is wrong
  3  a person must act on the device (unlock, trust, allow); the message says what
  4  no device matches, or it was disconnected

When something goes wrong, see Debugging in the README: traces, daemon.log and the runner log.
Settings by environment variable are in the README's table (PHONE_USE_HOME, PHONE_USE_TEAM, ...).";

#[derive(Parser)]
#[command(
    name = "phone-use",
    version,
    disable_help_subcommand = true,
    after_help = AFTER_HELP,
    next_help_heading = "Global options"
)]
pub struct Cli {
    /// The device to use: an id from `phone-use devices` such as `android:emulator-5554` or `sim:<udid>`, or just the part after the colon. With one device connected it is the default.
    #[arg(long, global = true, env = "PHONE_USE_DEVICE")]
    device: Option<String>,
    /// A name for this agent's refs, diff baselines and device lease. Give each agent that shares a daemon its own.
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
    /// Do not wait for the screen to settle after the action.
    #[arg(long, global = true)]
    no_settle: bool,
    /// How long to wait for the screen to settle, in milliseconds.
    #[arg(long, global = true)]
    timeout: Option<u64>,
    /// How long to wait for an action's first visible change, in milliseconds (default 500).
    #[arg(long, global = true)]
    expect_change: Option<u64>,
    /// Use a device that another session holds the lease on.
    #[arg(long, global = true)]
    share: bool,
    #[command(subcommand)]
    command: Option<Verb>,
}

const UNUSED_GLOBALS: &[(&str, &[&str])] = &[
    (
        "devices",
        &["session", "no_settle", "timeout", "expect_change", "share"],
    ),
    ("snapshot", &["expect_change"]),
    ("wait", &["no_settle"]),
    ("screenshot", &["no_settle", "timeout", "expect_change"]),
    ("apps", &["no_settle", "timeout", "expect_change"]),
    ("find", &["no_settle", "timeout", "expect_change"]),
    ("clipboard", &["no_settle", "timeout", "expect_change"]),
    ("release", &["no_settle", "timeout", "expect_change"]),
    ("daemon", ALL_GLOBALS),
    ("doctor", ALL_GLOBALS),
    ("mcp", ALL_GLOBALS),
    ("skill", ALL_GLOBALS),
    ("version", ALL_GLOBALS),
    ("ios", ALL_GLOBALS),
];

const ALL_GLOBALS: &[&str] = &[
    "device",
    "session",
    "json",
    "no_settle",
    "timeout",
    "expect_change",
    "share",
];

fn ignored_here(id: &'static str) -> clap::Arg {
    let long: &'static str = match id {
        "no_settle" => "no-settle",
        "expect_change" => "expect-change",
        "device" => "device",
        "session" => "session",
        "json" => "json",
        "timeout" => "timeout",
        _ => "share",
    };
    let arg = clap::Arg::new(id).long(long).hide(true);
    match id {
        "json" | "no_settle" | "share" => arg.action(clap::ArgAction::SetTrue),
        _ => arg.num_args(1),
    }
}

fn without_globals(cmd: clap::Command, unused: &[&'static str]) -> clap::Command {
    cmd.args(unused.iter().map(|id| ignored_here(id)))
}

/// The command line, with each global option left out of the help of the commands that ignore it. A command that
/// declares an option of the same name keeps clap from handing it the global one.
pub fn command() -> clap::Command {
    let mut cmd = <Cli as clap::CommandFactory>::command();
    for (verb, unused) in UNUSED_GLOBALS {
        cmd = cmd.mut_subcommand(*verb, |s| without_globals(s, unused));
    }
    cmd
}

const DIRECTIONS: [&str; 4] = ["up", "down", "left", "right"];

#[derive(Subcommand)]
pub(crate) enum Verb {
    /// List devices, tier, capabilities and leases.
    Devices,
    /// Print what is on screen, or what changed since the last observation.
    Snapshot {
        /// Print only what changed since the last observation.
        #[arg(long)]
        diff: bool,
        /// Print only the elements that can be tapped, typed into or scrolled.
        #[arg(long)]
        interactive: bool,
        /// Print only this element's subtree: `@ref`.
        #[arg(long, alias = "in")]
        within: Option<String>,
        /// Cut the output to this many lines.
        #[arg(long)]
        max_lines: Option<usize>,
        /// Read the screen's pixels with OCR and add the text the tree lacks. Done automatically when the tree is empty or a web view has no children.
        #[arg(long)]
        pixels: bool,
        /// Save the device's own tree and a screenshot as a test fixture.
        #[arg(long)]
        save_fixture: Option<String>,
        /// Where `--save-fixture` writes; the default is the core crate's fixture directory.
        #[arg(long)]
        fixture_dir: Option<PathBuf>,
    },
    /// Save a screenshot at one pixel per point and print its path.
    Screenshot {
        /// Write the PNG here, on this machine, instead of printing the path on the daemon's machine.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Draw a box and the `@eN` ref on every element that can be tapped or typed into.
        #[arg(long)]
        marks: bool,
    },
    /// Tap `@ref`, `x,y` in points, or the element with this text.
    Tap {
        /// `@ref` from a snapshot, `x,y` in points, or the text of the element.
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
    Gesture {
        /// The fingers as JSON.
        spec: String,
    },
    /// Press at `from`, hold, drag to `to` and lift: reorder a list, move an item, drag a handle. Targets are `@ref`, `x,y` or text.
    Drag { from: String, to: String },
    /// Two fingers: `out` spreads them to zoom in, `in` closes them to zoom out. At the screen's center, or at `--at`.
    Pinch {
        #[arg(value_parser = ["in", "out"])]
        direction: String,
        #[arg(long)]
        at: Option<String>,
        /// How far apart the fingers end (out) or start (in), as a share of the screen's shorter side; default 0.5.
        #[arg(long)]
        distance: Option<f32>,
    },
    /// Open an app by name or package and show its first screen.
    Launch {
        /// The app's name, package or bundle id; `phone-use apps` lists them.
        app: String,
    },
    /// Open a web link or deep link in the app that handles it.
    Open {
        /// The link, such as `https://example.com` or `myapp://item/3`.
        url: String,
    },
    /// List the apps that can be launched, or `install <path>`, `stop`, `clear` (data) or `uninstall` one by name or id.
    /// On a physical device `clear` and `uninstall` only touch apps phone-use installed.
    Apps {
        #[arg(value_parser = ["install", "stop", "clear", "uninstall"])]
        action: Option<String>,
        /// The app file to install (on the daemon's machine), or the app to stop, clear or uninstall.
        target: Option<String>,
    },
    /// Drag from the middle of the screen, or of `--from`, the way a finger does.
    Swipe {
        /// Where the finger moves.
        #[arg(value_parser = DIRECTIONS)]
        direction: String,
        /// Start the drag at this element or point: `@ref`, `x,y` or text.
        #[arg(long)]
        from: Option<String>,
        /// Share of the screen to cross, 0 to 1; the default is 0.5.
        #[arg(long)]
        distance: Option<f32>,
        /// Drag quickly, which lets a list fling.
        #[arg(long)]
        fast: bool,
    },
    /// Scroll the content so more of it shows in `direction`, or until `--until` text shows.
    Scroll {
        /// Which way more content should show.
        #[arg(value_parser = DIRECTIONS)]
        direction: String,
        /// The element to scroll: `@ref` or text. The default is the largest scrollable one.
        #[arg(long, alias = "in")]
        within: Option<String>,
        /// Keep scrolling until this text or `@ref` is on screen.
        #[arg(long)]
        until: Option<String>,
        /// Give up after this many scrolls (the default is 30).
        #[arg(long)]
        max: Option<u32>,
    },
    /// List the elements whose text contains this text, in any case.
    Find {
        /// The text to look for.
        text: String,
        /// Only elements with this role, such as `button` or `cell`.
        #[arg(long)]
        role: Option<String>,
    },
    /// Type text into a field. Secrets come from stdin or PHONE_USE_SECRET, never from arguments.
    Type {
        /// The text. Leave it out with `--secret`.
        text: Option<String>,
        /// Tap this field first: `@ref`, `x,y` or the field's text.
        #[arg(long)]
        into: Option<String>,
        /// How to enter it: `paste` (fast), `set` (replace the field's value) or `keys` (one key at a time).
        #[arg(long, value_parser = ["paste", "set", "keys"])]
        mode: Option<String>,
        /// Press enter after the text.
        #[arg(long)]
        submit: bool,
        /// Read the text from stdin or PHONE_USE_SECRET and keep it out of logs and traces.
        #[arg(long)]
        secret: bool,
    },
    /// Empty a text field: `@ref`, `x,y` or the field's text.
    Clear {
        /// `@ref`, `x,y` or the field's text.
        target: String,
    },
    /// Print the device clipboard, or set it with `clipboard set <text>`.
    Clipboard {
        /// `get` (the default) or `set`.
        #[arg(value_parser = ["get", "set"])]
        action: Option<String>,
        /// The text for `set`.
        text: Option<String>,
    },
    /// Press a button or key: home, back, recents, notifications, lock, volume-up, volume-down, enter, hide-keyboard.
    Press {
        /// The key or button.
        key: String,
    },
    /// Wait for `settled`, for a text or `@ref` to appear, or for `gone:<text>` (or `gone:@ref`) to leave the screen.
    Wait {
        /// `settled`, a text or `@ref` to appear, or `gone:<text>` (or `gone:@ref`) to leave.
        condition: String,
    },
    /// Run steps in order and print one diff at the end; stops at the first step that fails. Each step is a
    /// command line in quotes, such as `batch "tap @e3" "type hello --submit" "press enter"`.
    Batch { steps: Vec<String> },
    /// Give up this session's lease on the device.
    Release,
    /// Run the daemon in the foreground or detached; `daemon stop` ends the running one.
    Daemon {
        #[command(subcommand)]
        command: Option<DaemonCommand>,
        /// Run detached from the terminal; the CLI starts the daemon this way when none is running.
        #[arg(long)]
        detach: bool,
        /// Keep observation text and text targets in the trace files.
        #[arg(long, env = "PHONE_USE_TRACE_FULL", value_parser = BoolishValueParser::new())]
        trace_full: bool,
        /// Keep a copy of every screenshot next to the trace file.
        #[arg(long, env = "PHONE_USE_TRACE_SCREENS", value_parser = BoolishValueParser::new())]
        trace_screens: bool,
    },
    /// Check adb, Android devices and their agent, Xcode, booted simulators and their runners, wired iPhones, and the daemon socket; each line says ok, warn or fail, with a fix.
    Doctor,
    /// Serve the tools over MCP on stdio.
    Mcp,
    /// Print SKILL.md, the instructions an agent reads before it uses phone-use.
    Skill,
    /// Prepare a physical iOS device: pairing, Developer Mode, the developer disk image.
    Ios {
        #[command(subcommand)]
        command: IosCommand,
    },
    /// Print the version.
    Version,
}

#[derive(Subcommand)]
pub(crate) enum DaemonCommand {
    /// Stop the running daemon and the iOS runners it started. Succeeds when none is running.
    Stop,
}

#[derive(Subcommand, Debug)]
pub enum IosCommand {
    /// Check each wired device and do what needs no one at the device: show the Developer Mode switch, mount the
    /// developer disk image with Xcode's tool (the only step that talks to Apple). Prints `ok` for steps already done.
    Setup,
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
            command: Some(DaemonCommand::Stop),
            ..
        } => stop_daemon(),
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
        Verb::Ios {
            command: IosCommand::Setup,
        } => {
            if crate::doctor::ios_setup() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
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
                Err(message) => {
                    eprintln!("error: {message}");
                    return ExitCode::from(2);
                }
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
            "with --secret the text must come from stdin or PHONE_USE_SECRET, not from the command line (arguments show in `ps` and shell history)\nexample: echo -n \"$PW\" | phone-use type --secret --into @e3".into(),
        ),
        (false, None) => Err("type needs the text to type\nexample: phone-use type \"hello\" --into @e3".into()),
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
            Ok(with_default_device(
                device_table(&result),
                g.device.as_deref(),
            ))
        }
        Verb::Snapshot {
            diff,
            interactive,
            within,
            max_lines,
            pixels,
            save_fixture,
            fixture_dir,
        } => {
            let mut p = json!({
                "diff": diff, "interactive": interactive, "max_lines": max_lines, "pixels": pixels,
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
        Verb::Screenshot { output, marks } => {
            let mut params = json!({"marks": marks});
            if output.is_some() {
                params["inline"] = json!(true);
            }
            let result = client.call("screenshot", with(params)).await?;
            let shot: ScreenshotResult = decode(&result)?;
            let path = match (output, &shot.data_b64) {
                (Some(out), Some(data)) => {
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(data)
                        .map_err(|e| {
                            Failure::Local(format!("the daemon sent bad image data: {e}"))
                        })?;
                    std::fs::write(&out, bytes).map_err(|e| {
                        Failure::Local(format!("cannot write {}: {e}", out.display()))
                    })?;
                    out.display().to_string()
                }
                _ => shot.path.clone(),
            };
            if g.json {
                return Ok(result.to_string());
            }
            Ok(match shot.marks {
                Some(n) => format!("{path} {}x{} · {n} marks", shot.width, shot.height),
                None => format!("{path} {}x{}", shot.width, shot.height),
            })
        }
        Verb::Wait { condition } => {
            let mut p = json!({"condition": wait_condition(&condition)});
            if let Some(t) = g.timeout {
                p["timeout_ms"] = json!(t);
            }
            if let Some(t) = g.expect_change {
                p["expect_change_ms"] = json!(t);
            }
            let r = client.call("wait", with(p)).await?;
            show_action(&r, g.json)
        }
        verb @ (Verb::Tap { .. }
        | Verb::Gesture { .. }
        | Verb::Swipe { .. }
        | Verb::Drag { .. }
        | Verb::Pinch { .. }
        | Verb::Scroll { .. }
        | Verb::Launch { .. }
        | Verb::Open { .. }
        | Verb::Type { .. }
        | Verb::Clear { .. }
        | Verb::Press { .. }) => {
            let action = step_json(&verb, secret.as_deref())?;
            let r = client.call("act", act(action)).await?;
            show_action(&r, g.json)
        }
        Verb::Batch { steps } => {
            let steps = steps
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    parse_step_line(line)
                        .map_err(|why| Failure::Local(format!("step {}: {why}", i + 1)))
                })
                .collect::<Result<Vec<_>, _>>()?;
            run_batch(client, steps, g).await
        }
        Verb::Apps {
            action: Some(action),
            target,
        } => {
            let target = target
                .ok_or_else(|| Failure::Local(format!("apps {action} needs an app or a path")))?;
            let p = if action == "install" {
                json!({"action": action, "path": target})
            } else {
                json!({"action": action, "app": target})
            };
            let r = client.call("app", with(p)).await?;
            if g.json {
                return Ok(r.to_string());
            }
            Ok(format!("ok {}", r["summary"].as_str().unwrap_or_default()))
        }
        Verb::Apps { .. } => {
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
        Verb::Release => {
            let r = client.call("release", with(json!({}))).await?;
            Ok(if g.json {
                r.to_string()
            } else {
                "released".into()
            })
        }
        Verb::Daemon { .. }
        | Verb::Doctor
        | Verb::Ios { .. }
        | Verb::Mcp
        | Verb::Skill
        | Verb::Version => Ok(String::new()),
    }
}

fn wait_condition(condition: &str) -> Value {
    match condition.strip_prefix("gone:") {
        Some(rest) => json!({"gone": parse_target(rest)}),
        None if condition == "settled" => json!({"settled": true}),
        None => json!({"appears": parse_target(condition)}),
    }
}

/// The wire form of a verb that acts on the screen: an `act` action, or `{"kind": "wait", ...}` for `wait`, which a
/// batch carries as a step. `secret` is the text of a `type --secret`.
pub(crate) fn step_json(verb: &Verb, secret: Option<&str>) -> Result<Value, Failure> {
    Ok(match verb {
        Verb::Tap {
            target,
            role,
            count,
            long,
        } => {
            let mut t = parse_target(target);
            if let Some(role) = role {
                t["role"] = role_json(role)?;
            }
            let mut a = json!({"kind": "tap", "target": t});
            if let Some(c) = count {
                a["count"] = json!(c);
            }
            if let Some(l) = long {
                a["long"] = json!(l);
            }
            a
        }
        Verb::Gesture { spec } => {
            let mut a: Value = serde_json::from_str(spec)
                .map_err(|e| Failure::Local(format!("the gesture is not JSON: {e}")))?;
            a["kind"] = json!("gesture");
            a
        }
        Verb::Swipe {
            direction,
            from,
            distance,
            fast,
        } => {
            let mut a = json!({"kind": "swipe", "direction": direction, "fast": fast});
            if let Some(f) = from {
                a["from"] = parse_target(f);
            }
            if let Some(d) = distance {
                a["distance"] = json!(d);
            }
            a
        }
        Verb::Scroll {
            direction,
            within,
            until,
            max,
        } => {
            let mut a = json!({"kind": "scroll", "direction": direction});
            if let Some(w) = within {
                a["in"] = parse_target(w);
            }
            if let Some(u) = until {
                a["until"] = parse_target(u);
            }
            if let Some(m) = max {
                a["max"] = json!(m);
            }
            a
        }
        Verb::Drag { from, to } => {
            json!({"kind": "drag", "from": parse_target(from), "to": parse_target(to)})
        }
        Verb::Pinch {
            direction,
            at,
            distance,
        } => {
            let mut a = json!({"kind": "pinch", "direction": direction});
            if let Some(t) = at {
                a["at"] = parse_target(t);
            }
            if let Some(d) = distance {
                a["distance"] = json!(d);
            }
            a
        }
        Verb::Launch { app } => json!({"kind": "launch", "app": app}),
        Verb::Open { url } => json!({"kind": "open", "url": url}),
        Verb::Type {
            text,
            into,
            mode,
            submit,
            secret: is_secret,
        } => {
            let text = secret.or(text.as_deref()).unwrap_or_default();
            let mut a =
                json!({"kind": "type", "text": text, "secret": is_secret, "submit": submit});
            if let Some(i) = into {
                a["into"] = parse_target(i);
            }
            if let Some(m) = mode {
                a["mode"] = json!(m);
            }
            a
        }
        Verb::Clear { target } => json!({"kind": "clear", "target": parse_target(target)}),
        Verb::Press { key } => json!({"kind": "press", "key": key}),
        Verb::Wait { condition } => {
            json!({"kind": "wait", "condition": wait_condition(condition)})
        }
        _ => {
            return Err(Failure::Local(
                "that command cannot be a step of a batch".into(),
            ));
        }
    })
}

/// Splits a command line into words: spaces separate them, single or double quotes keep spaces in one, a backslash
/// quotes the next character outside single quotes.
fn split_words(line: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') | (None, '\\') => {
                cur.push(chars.next().ok_or("the line ends with a backslash")?);
            }
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

/// One step of `phone-use batch`, written as the command line it would be on its own.
fn parse_step_line(line: &str) -> Result<Value, String> {
    let words = split_words(line)?;
    let cli = Cli::try_parse_from(std::iter::once("phone-use".to_string()).chain(words))
        .map_err(|e| e.to_string().lines().next().unwrap_or_default().to_string())?;
    let verb = cli.command.ok_or("the step is empty")?;
    if matches!(&verb, Verb::Type { secret: true, .. }) {
        return Err("secret text cannot go in a batch; run `type --secret` on its own".into());
    }
    step_json(&verb, None).map_err(|f| f.message())
}

/// Sends the steps as one `batch` call and prints the numbered steps and the one diff, or fails naming the step.
pub(crate) async fn run_batch(
    client: &mut Client,
    steps: Vec<Value>,
    g: &Globals,
) -> Result<String, Failure> {
    let sent = steps.len();
    let mut p = json!({"steps": steps});
    if let Some(d) = &g.device {
        p["device"] = json!(d);
    }
    if g.share {
        p["share"] = json!(true);
    }
    if let Some(t) = g.timeout {
        p["timeout_ms"] = json!(t);
    }
    if let Some(t) = g.expect_change {
        p["expect_change_ms"] = json!(t);
    }
    let r = client.call("batch", p).await?;
    show_batch(&r, g.json, sent)
}

fn show_batch(result: &Value, json: bool, sent: usize) -> Result<String, Failure> {
    let r: BatchResult = decode(result)?;
    let total = result["steps"].as_array().map_or(0, Vec::len);
    let mut lines = Vec::new();
    for (i, s) in r.steps.iter().enumerate() {
        let verified = match s.verified {
            Some(true) => " · verified",
            Some(false) => " · NOT verified",
            None => "",
        };
        lines.push(format!("{}. {}{verified}", i + 1, s.summary));
    }
    if let Some(f) = &r.failed {
        let code = f.error["code"].as_i64().unwrap_or(-32603) as i32;
        let message = f.error["message"].as_str().unwrap_or_default();
        let mut text = format!("step {} of {sent} failed: {message}", f.step);
        for l in &lines {
            text.push('\n');
            text.push_str(l);
        }
        if !r.observation.text.is_empty() {
            text.push_str("\nscreen now, against what you last saw:\n");
            text.push_str(&r.observation.text);
        }
        return Err(Failure::Rpc(crate::daemon::rpc::RpcError::new(
            code,
            text,
            f.error["data"].clone(),
        )));
    }
    if json {
        return Ok(result.to_string());
    }
    let mut head = format!("ok {total} steps");
    if !r.settled {
        head.push_str(&format!(" · not settled after {}ms", r.settle_ms));
    } else if !r.changed {
        head.push_str(&format!(" · no change after {}ms", r.settle_ms));
    } else {
        head.push_str(&format!(" · settled in {}ms", r.settle_ms));
        if r.observation.kind == Kind::Full {
            head.push_str(" · new screen");
        } else if r.observation.text.is_empty() {
            head.push_str(" · no element changed");
        }
    }
    let mut out = vec![head];
    out.extend(lines);
    if !r.observation.text.is_empty() {
        out.push(r.observation.text);
    }
    Ok(out.join("\n"))
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

fn with_default_device(table: String, default: Option<&str>) -> String {
    match default {
        Some(id) => format!("{table}\ndefault device: {id} (used when `device` is omitted)"),
        None => table,
    }
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
            match (d.leased_by, d.lease_idle_s) {
                (Some(by), Some(idle)) => format!("{by} (idle {})", span(idle)),
                (Some(by), None) => by,
                (None, _) => "-".into(),
            },
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
            } else if r.observation.text.is_empty() {
                head.push_str(" · no element changed");
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

fn stop_daemon() -> ExitCode {
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        eprintln!("error: cannot start the async runtime");
        return ExitCode::from(1);
    };
    match rt.block_on(crate::client::stop_daemon(&socket::socket_path())) {
        Ok(Some(pid)) => println!("stopped the daemon (pid {pid})"),
        Ok(None) => println!("no daemon is running"),
        Err(f) => {
            eprintln!("error: {}", f.message());
            return ExitCode::from(f.exit_code());
        }
    }
    ExitCode::SUCCESS
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
            #[cfg(feature = "android")]
            ("android:", Arc::new(AdbSource::from_env())),
            ("sim:", Arc::new(SimSource::new(socket::home()))),
            ("ios:", Arc::new(PhysicalSource::new(socket::home()))),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn action(changed: bool, kind: &str, text: &str) -> Value {
        json!({
            "ok": true, "summary": "tapped @e1", "settled": true, "settle_ms": 400, "changed": changed,
            "observation": {
                "header": {}, "text": text, "kind": kind, "changed": changed, "settled": true,
                "settle_ms": 400, "loading": false
            }
        })
    }

    #[test]
    fn a_tap_that_redraws_without_changing_any_element_says_so() {
        let shown = show_action(&action(true, "diff", ""), false).unwrap();
        assert_eq!(
            shown,
            "ok tapped @e1 · settled in 400ms · no element changed"
        );
        let shown = show_action(&action(true, "diff", "~ @e1 x → y"), false).unwrap();
        assert!(!shown.contains("no element changed"), "{shown}");
    }

    #[test]
    fn a_step_line_splits_like_a_shell_line_and_parses_as_the_verb_it_names() {
        assert_eq!(
            split_words(r#"type "hello world" --into 'Name field' a\ b"#).unwrap(),
            ["type", "hello world", "--into", "Name field", "a b"]
        );
        assert_eq!(split_words(r#"tap """#).unwrap(), ["tap", ""]);
        assert!(split_words("tap 'x").is_err());
        let step = parse_step_line(r#"type "hello world" --into @e3 --submit"#).unwrap();
        assert_eq!(step["kind"], "type");
        assert_eq!(step["text"], "hello world");
        assert_eq!(step["into"], json!({"ref": "e3"}));
        assert_eq!(step["submit"], true);
        assert_eq!(
            parse_step_line("wait gone:Loading").unwrap(),
            json!({"kind": "wait", "condition": {"gone": {"text": "Loading"}}})
        );
        assert!(
            parse_step_line("type --secret")
                .unwrap_err()
                .contains("secret")
        );
        assert!(parse_step_line("devices").unwrap_err().contains("batch"));
        assert!(parse_step_line("").is_err());
        assert!(parse_step_line("tap").is_err());
    }

    #[test]
    fn devices_names_the_default_device_when_one_is_set() {
        let table = "ID\nandroid:a".to_string();
        assert!(
            with_default_device(table.clone(), Some("android:a"))
                .ends_with("default device: android:a (used when `device` is omitted)")
        );
        assert_eq!(with_default_device(table.clone(), None), table);
    }
}
