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
use phonedriver_core::device::DisplayMode;
use phonedriver_core::error::{Category, span};
use phonedriver_core::render::plural;

use crate::wire::{
    ActionResult, Approval, BatchResult, DeviceEntry, Kind, Observation, ScreenshotResult,
    ShellResult, VERSION,
};

const USAGE: &str = "usage: phonedriver <command>\n\nrun `phonedriver --help` for the options and exit codes\n\ncommands:\n  devices\n  snapshot\n  screenshot\n  find\n  launch\n  open\n  apps\n  swipe\n  drag\n  pinch\n  scroll\n  gesture\n  tap\n  type\n  clear\n  clipboard\n  shell\n  press\n  wait\n  batch\n  release\n  daemon\n  doctor\n  pair\n  untether\n  ios\n  mcp\n  skill\n  version";

const AFTER_HELP: &str = "Examples:
  phonedriver devices
  phonedriver snapshot
  phonedriver tap \"Dark theme\"
  phonedriver scroll down --until \"About\"

Exit codes:
  0  done
  1  the command failed (the message says why)
  2  the command line is wrong
  3  a person must act on the device (unlock, trust, allow); the message says what
  4  no device matches, or it was disconnected
  5  the tap spends money or cannot be undone and the human has not approved it; ask them, then add --autonomous

When something goes wrong, see Debugging in the README: traces, daemon.log and the runner log.
Settings by environment variable are in the README's table (PHONEDRIVER_HOME, PHONEDRIVER_TEAM, ...).";

#[derive(Parser)]
#[command(
    name = "phonedriver",
    version,
    disable_help_subcommand = true,
    after_help = AFTER_HELP,
    next_help_heading = "Global options"
)]
pub struct Cli {
    /// The device to use: an id from `phonedriver devices` such as `android:emulator-5554` or `sim:<udid>`, or just the part after the colon. With one device connected it is the default.
    #[arg(long, global = true, env = "PHONEDRIVER_DEVICE")]
    device: Option<String>,
    /// A name for this agent's refs, diff baselines and device lease. Give each agent that shares a daemon its own.
    #[arg(
        long,
        global = true,
        env = "PHONEDRIVER_SESSION",
        default_value = "default"
    )]
    session: String,
    /// Print the daemon's result as JSON.
    #[arg(long, global = true)]
    json: bool,
    /// Do not wait for the screen to settle after the action.
    #[arg(long, global = true)]
    no_settle: bool,
    /// How long to wait, in milliseconds: for the screen to settle after an action, or for what `wait` waits for (default 5000).
    #[arg(long, global = true)]
    timeout: Option<u64>,
    /// How long to wait for an action's first visible change, in milliseconds (default 500).
    #[arg(long, global = true)]
    expect_change: Option<u64>,
    /// Use a device that another session holds the lease on.
    #[arg(long, global = true)]
    share: bool,
    /// Which screen to work on (Android): `main`, the phone's own screen (default), or `hidden`, a second screen the person holding the phone cannot see (Android 13 and later).
    #[arg(
        long,
        global = true,
        env = "PHONEDRIVER_DISPLAY",
        value_parser = ["main", "hidden"],
        default_value = "main"
    )]
    display: String,
    #[command(subcommand)]
    command: Option<Verb>,
}

const UNUSED_GLOBALS: &[(&str, &[&str])] = &[
    (
        "devices",
        &[
            "session",
            "no_settle",
            "timeout",
            "expect_change",
            "share",
            "display",
        ],
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
    ("pair", ALL_GLOBALS),
    (
        "untether",
        &[
            "session",
            "json",
            "no_settle",
            "timeout",
            "expect_change",
            "share",
        ],
    ),
    (
        "mcp",
        &[
            "session",
            "json",
            "no_settle",
            "timeout",
            "expect_change",
            "share",
        ],
    ),
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
    "display",
];

fn ignored_here(id: &'static str) -> clap::Arg {
    let long: &'static str = match id {
        "no_settle" => "no-settle",
        "expect_change" => "expect-change",
        "device" => "device",
        "session" => "session",
        "json" => "json",
        "timeout" => "timeout",
        "display" => "display",
        _ => "share",
    };
    let arg = clap::Arg::new(id).long(long).hide(true);
    match id {
        "json" | "no_settle" | "share" => arg.action(clap::ArgAction::SetTrue),
        "display" => arg.num_args(1).value_parser(["main", "hidden"]),
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
    cmd.build();
    cmd.mut_subcommands(own_options_only)
}

const GLOBALS_POINTER: &str =
    "Options that apply to every command (--device, --json, --timeout, ...): `phonedriver --help`.";

/// A command's help lists its own options. The copies of the global options clap puts on it are hidden: they
/// were 14 lines on every command.
fn own_options_only(cmd: clap::Command) -> clap::Command {
    cmd.mut_args(|a| if a.is_global_set() { a.hide(true) } else { a })
        .mut_subcommands(own_options_only)
        .after_help(GLOBALS_POINTER)
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
        /// Print only this element's subtree, its own text uncut: `@ref`.
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
        /// Tap a button that spends money or cannot be undone without asking the human. Only when the user has granted full autonomy.
        #[arg(long)]
        autonomous: bool,
    },
    /// Play fingers with timed paths, in points: `{"fingers": [[{"x": 100, "y": 300, "t_ms": 0}, {"x": 50, "y": 300, "t_ms": 300}]]}`.
    Gesture {
        /// The fingers as JSON.
        spec: String,
    },
    /// Press at `from`, hold, drag to `to` and lift: reorder a list, move an item, drag a handle. Targets are `@ref`, `x,y` or text.
    Drag {
        /// Where the finger goes down.
        from: String,
        /// Where it lifts.
        to: String,
    },
    /// Two fingers: `out` spreads them to zoom in, `in` closes them to zoom out. At the screen's center, or at `--at`.
    Pinch {
        /// `out` zooms in, `in` zooms out.
        #[arg(value_parser = ["in", "out"])]
        direction: String,
        /// The center of the pinch: `@ref`, `x,y` or text. Default is the middle of the screen.
        #[arg(long)]
        at: Option<String>,
        /// How far apart the fingers end (out) or start (in), as a share of the screen's shorter side; default 0.5.
        #[arg(long)]
        distance: Option<f32>,
    },
    /// Open an app by name or package and show its first screen.
    Launch {
        /// The app's name, package or bundle id; `phonedriver apps` lists them.
        app: String,
        /// Grant these runtime permissions first, so their dialog never appears: `notifications`, `camera`,
        /// `location`, or a full name such as `android.permission.CAMERA` (Android); `photos`, `location` and the other
        /// `simctl privacy` services (Simulator). A physical device only takes this for apps phonedriver installed.
        #[arg(long, value_delimiter = ',')]
        grant: Vec<String>,
        /// Stop the app first, so it opens on its first screen instead of where it was left.
        #[arg(long)]
        restart: bool,
    },
    /// Open a web link or deep link in the app that handles it.
    Open {
        /// The link, such as `https://example.com` or `myapp://item/3`.
        url: String,
        /// Open it in this app (name or id) instead of the default handler.
        #[arg(long)]
        app: Option<String>,
    },
    /// List the apps that can be launched, or `install <path>`, `stop`, `clear` (data) or `uninstall` one by name or id.
    /// On a physical device `clear` and `uninstall` only touch apps phonedriver installed.
    Apps {
        /// What to do to an app. Omit it to list apps.
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
        /// Keep scrolling until this text (`A|B` for either) or `@ref` is on screen.
        #[arg(long)]
        until: Option<String>,
        /// Give up after this many scrolls (the default is 30).
        #[arg(long)]
        max: Option<u32>,
    },
    /// List the elements whose text contains this text, in any case.
    Find {
        /// The text to look for; `A|B` finds either.
        text: String,
        /// Only elements with this role, such as `button` or `cell`.
        #[arg(long)]
        role: Option<String>,
    },
    /// Type text into a field. With `--into` the text replaces what the field holds. Secrets come from stdin or PHONEDRIVER_SECRET, never from arguments.
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
        /// With `--into`, add to the field's text instead of replacing it.
        #[arg(long)]
        append: bool,
        /// Read the text from stdin or PHONEDRIVER_SECRET and keep it out of logs and traces.
        #[arg(long)]
        secret: bool,
        /// With `--submit`, send a form that spends money or cannot be undone without asking the human. Only when the user has granted full autonomy.
        #[arg(long)]
        autonomous: bool,
    },
    /// Empty a text field: `@ref`, `x,y` or the field's text.
    Clear {
        /// `@ref`, `x,y` or the field's text.
        target: String,
    },
    /// Run a command in the device's shell (Android: the agent's shell, no `adb` spawn; Simulator: `simctl spawn`, or
    /// `simctl <subcommand> ...` with the device filled in). Prints the exit code, trimmed output, and what changed on
    /// screen. Secrets come from stdin or PHONEDRIVER_SECRET.
    Shell {
        /// The command, such as `cmd uimode night yes`. Leave it out with `--secret`.
        command: Option<String>,
        /// Keep only output lines containing one of these words, separated by `|`, ignoring case.
        #[arg(long)]
        grep: Option<String>,
        /// Keep this many lines of output (default 40, 0 keeps all).
        #[arg(long)]
        head: Option<usize>,
        /// Run a command that is refused on a physical device (wipes, `rm -r` of storage, removing apps phonedriver did not install).
        #[arg(long)]
        force: bool,
        /// Kill the command after this many seconds (default 30).
        #[arg(long)]
        time_limit: Option<u64>,
        /// Read the command from stdin or PHONEDRIVER_SECRET and keep it out of logs and traces.
        #[arg(long)]
        secret: bool,
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
    /// `press back`.
    #[command(hide = true)]
    Back,
    /// `press home`.
    #[command(hide = true)]
    Home,
    /// Wait for `settled`, for a text or `@ref` to appear, or for `gone:<text>` (or `gone:@ref`) to leave the screen.
    Wait {
        /// `settled`, a text or `@ref` to appear, or `gone:<text>` (or `gone:@ref`) to leave.
        condition: String,
    },
    /// Run steps in order and print one diff at the end; stops at the first step that fails. Each step is a
    /// command line in quotes, such as `batch "tap @e3" "type hello --submit" "press enter"`.
    Batch {
        /// One command line per step, in quotes.
        steps: Vec<String>,
    },
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
        #[arg(long, env = "PHONEDRIVER_TRACE_FULL", value_parser = BoolishValueParser::new())]
        trace_full: bool,
        /// Keep a copy of every screenshot next to the trace file.
        #[arg(long, env = "PHONEDRIVER_TRACE_SCREENS", value_parser = BoolishValueParser::new())]
        trace_screens: bool,
    },
    /// Check adb, Android devices and their agent, Xcode, booted simulators and their runners, wired iPhones, and the daemon socket; each line says ok, warn or fail, with a fix. With `--device`, only that device is probed.
    Doctor,
    /// Pair an Android 11+ phone over Wi-Fi with a QR code.
    ///
    /// Shows a QR code. On the phone open Developer options > Wireless debugging > Pair device with QR code and scan it; the phone is paired and connected. Needs adb's mDNS and the phone on the same Wi-Fi; otherwise pair with the code (see docs/reference.md). Not yet tested on a physical phone.
    #[cfg(feature = "android")]
    Pair,
    /// Make an Android phone reachable over any network, such as Tailscale.
    ///
    /// Runs `adb tcpip 5555` on the phone (USB or Wi-Fi), lists its addresses by interface and connects to the first that answers. adb then listens on every network the phone joins until it reboots, so use it only on networks you trust or with Tailscale; `adb usb` or a reboot turns it off. Not yet tested on a physical phone.
    #[cfg(feature = "android")]
    Untether,
    /// Serve the tools over MCP on stdio.
    Mcp,
    /// Print SKILL.md, the instructions an agent reads before it uses phonedriver.
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
            println!("phonedriver {VERSION}");
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
        #[cfg(feature = "android")]
        Verb::Pair => crate::wireless::pair(),
        #[cfg(feature = "android")]
        Verb::Untether => crate::wireless::untether(cli.device.as_deref()),
        Verb::Mcp => crate::mcp::run(cli.device),
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
            if crate::doctor::run(&socket::home(), cli.device.as_deref()) {
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
            let ask = match ask_env() {
                Ok(a) => a,
                Err(message) => {
                    eprintln!("error: {message}");
                    return ExitCode::from(2);
                }
            };
            let globals = Globals {
                device: cli.device,
                json: cli.json,
                no_settle: cli.no_settle,
                timeout: cli.timeout,
                expect_change: cli.expect_change,
                share: cli.share,
                display: display_mode(&cli.display),
                approval: None,
                ask,
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
    pub display: DisplayMode,
    pub approval: Option<Approval>,
    pub ask: Vec<Category>,
}

/// The opt-in categories of tap the human wants to be asked about, from `PHONEDRIVER_ASK` (decision D30). A name that
/// is not one of them is an error: a typo must not leave a category unguarded without saying so.
pub(crate) fn parse_ask(env: Option<&str>) -> Result<Vec<Category>, String> {
    let mut ask = Vec::new();
    for word in env.unwrap_or_default().split(',').map(str::trim) {
        if word.is_empty() {
            continue;
        }
        let Some(category) = Category::from_opt_in(word) else {
            let valid: Vec<_> = Category::OPT_IN.iter().map(|c| c.name()).collect();
            return Err(format!(
                "PHONEDRIVER_ASK has \"{word}\", which is not a category; use a comma list of {}",
                valid.join(", ")
            ));
        };
        if !ask.contains(&category) {
            ask.push(category);
        }
    }
    Ok(ask)
}

pub(crate) fn ask_env() -> Result<Vec<Category>, String> {
    parse_ask(std::env::var("PHONEDRIVER_ASK").ok().as_deref())
}

pub(crate) fn display_mode(name: &str) -> DisplayMode {
    if name == "hidden" {
        DisplayMode::Hidden
    } else {
        DisplayMode::Main
    }
}

pub(crate) fn put_display(params: &mut Value, display: DisplayMode) {
    if display == DisplayMode::Hidden {
        params["display"] = json!("hidden");
    }
}

/// The guard's answer that does not come from asking: `--autonomous`, or `PHONEDRIVER_GUARD=off` for the whole process.
fn guard_override(verb: &Verb, given: Option<Approval>, env: Option<&str>) -> Option<Approval> {
    let flag = matches!(
        verb,
        Verb::Tap {
            autonomous: true,
            ..
        } | Verb::Type {
            autonomous: true,
            ..
        }
    );
    if flag || guard_off(env) {
        Some(Approval::Autonomous)
    } else {
        given
    }
}

fn guard_off(env: Option<&str>) -> bool {
    env.is_some_and(|v| v.eq_ignore_ascii_case("off"))
}

fn guard_env() -> Option<String> {
    std::env::var("PHONEDRIVER_GUARD").ok()
}

/// The text of a `type --secret`, read before any connection is made so a usage error types nothing.
fn secret_text(verb: &Verb) -> Result<Option<String>, String> {
    let (text, secret) = match verb {
        Verb::Type { text, secret, .. } => (text, secret),
        Verb::Shell {
            command, secret, ..
        } => (command, secret),
        _ => return Ok(None),
    };
    match (secret, text) {
        (true, Some(_)) => Err(
            "with --secret the text must come from stdin or PHONEDRIVER_SECRET, not from the command line (arguments show in `ps` and shell history)\nexample: echo -n \"$PW\" | phonedriver type --secret --into @e3".into(),
        ),
        (false, None) if matches!(verb, Verb::Shell { .. }) => Err("shell needs a command\nexample: phonedriver shell \"cmd uimode night yes\"".into()),
        (false, None) => Err("type needs the text to type\nexample: phonedriver type \"hello\" --into @e3".into()),
        (true, None) => {
            let text = match std::env::var("PHONEDRIVER_SECRET") {
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
    let out = execute_with(&mut client, verb, secret, g).await?;
    if g.json {
        return Ok(out);
    }
    Ok(without_repeated_device_line(&mut client, out).await)
}

/// A CLI call is its own process, so the daemon remembers per session which device line was last shown. The agent
/// keeps every reply, so a line it already has is not printed again.
async fn without_repeated_device_line(client: &mut Client, out: String) -> String {
    let Some(at) = crate::wire::device_line_at(&out) else {
        return out;
    };
    let mut lines: Vec<&str> = out.lines().collect();
    if client.told(lines[at]).await.unwrap_or(false) {
        lines.remove(at);
        return lines.join("\n");
    }
    out
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
        put_display(&mut params, g.display);
        params
    };
    let approval = guard_override(&verb, g.approval, guard_env().as_deref());
    let act = |action: Value| {
        let mut p = json!({"action": action, "settle": !g.no_settle});
        if let Some(a) = approval {
            p["approval"] = json!(a);
        }
        if !g.ask.is_empty() {
            p["ask"] = json!(g.ask);
        }
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
                Some(n) => format!(
                    "{path} {}x{} · {}",
                    shot.width,
                    shot.height,
                    plural(n.into(), "mark")
                ),
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
        | Verb::Press { .. }
        | Verb::Back
        | Verb::Home) => {
            let action = step_json(&verb, secret.as_deref())?;
            let r = client.call("act", act(action)).await?;
            show_action(&r, g.json)
        }
        Verb::Shell {
            command,
            grep,
            head,
            force,
            time_limit,
            secret: is_secret,
        } => {
            let command = secret.or(command).unwrap_or_default();
            let mut p = json!({"command": command, "settle": !g.no_settle, "force": force, "secret": is_secret});
            if let Some(v) = grep {
                p["grep"] = json!(v);
            }
            if let Some(v) = head {
                p["head"] = json!(v);
            }
            if let Some(v) = time_limit {
                p["time_limit_s"] = json!(v);
            }
            if let Some(t) = g.timeout {
                p["timeout_ms"] = json!(t);
            }
            if let Some(t) = g.expect_change {
                p["expect_change_ms"] = json!(t);
            }
            let r = client.call("shell", with(p)).await?;
            show_shell(&r, g.json)
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
                format!("no match on this screen; to look further, scroll with until: {text:?}")
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
        #[cfg(feature = "android")]
        Verb::Pair | Verb::Untether => Ok(String::new()),
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
            ..
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
        Verb::Launch {
            app,
            grant,
            restart,
        } => {
            json!({"kind": "launch", "app": app, "grant": grant, "restart": restart})
        }
        Verb::Open { url, app } => json!({"kind": "open", "url": url, "app": app}),
        Verb::Type {
            text,
            into,
            mode,
            submit,
            append,
            secret: is_secret,
            ..
        } => {
            let text = secret.or(text.as_deref()).unwrap_or_default();
            let mut a = json!({"kind": "type", "text": text, "secret": is_secret, "submit": submit, "append": append});
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
        Verb::Back => json!({"kind": "press", "key": "back"}),
        Verb::Home => json!({"kind": "press", "key": "home"}),
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

/// One step of `phonedriver batch`, written as the command line it would be on its own.
fn parse_step_line(line: &str) -> Result<Value, String> {
    let words = split_words(line)?;
    let cli = Cli::try_parse_from(std::iter::once("phonedriver".to_string()).chain(words))
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
    if guard_off(guard_env().as_deref()) {
        p["approval"] = json!(Approval::Autonomous);
    } else if let Some(a) = g.approval {
        p["approval"] = json!(a);
    }
    if !g.ask.is_empty() {
        p["ask"] = json!(g.ask);
    }
    if let Some(d) = &g.device {
        p["device"] = json!(d);
    }
    if g.share {
        p["share"] = json!(true);
    }
    put_display(&mut p, g.display);
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
        let verified = match (&s.verified, &s.unverified) {
            (Some(true), _) => " · verified".to_string(),
            (Some(false), Some(u)) => format!(" · NOT verified: {}", u.shows),
            (Some(false), None) => " · NOT verified".to_string(),
            _ => String::new(),
        };
        lines.push(format!("{}. {}{verified}", i + 1, s.summary));
        if let Some(u) = &s.unverified {
            lines.push(format!("   hint: {}", u.hint));
        }
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
        let mut data = f.error["data"].clone();
        if let Some(o) = data.as_object_mut() {
            o.insert("step".into(), json!(f.step));
        }
        return Err(Failure::Rpc(crate::daemon::rpc::RpcError::new(
            code, text, data,
        )));
    }
    if json {
        return Ok(result.to_string());
    }
    let mut head = format!("ok {}", plural(total as u64, "step"));
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
    phonedriver_core::element::Role::from_word(word)
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
    if devices.is_empty() {
        return "no devices\nhint: Connect a device or start an emulator; `phonedriver doctor` says what is missing."
            .into();
    }
    let any_hidden = devices.iter().any(|d| d.hidden_display.is_some());
    let mut head = vec!["ID", "MODEL", "OS", "TIER", "LEASE"];
    if any_hidden {
        head.push("DISPLAY");
    }
    let mut rows = vec![head.into_iter().map(String::from).collect::<Vec<_>>()];
    for d in devices {
        let mut row = vec![
            d.id,
            d.model,
            if d.os.is_empty() { "-".into() } else { d.os },
            d.tier,
            match (d.leased_by, d.lease_idle_s) {
                (Some(by), Some(idle)) => format!("{by} (idle {})", span(idle)),
                (Some(by), None) => by,
                (None, _) => "-".into(),
            },
        ];
        if any_hidden {
            row.push(match d.hidden_display {
                Some(id) => format!("hidden (display {id})"),
                None => "main".into(),
            });
        }
        rows.push(row);
    }
    let columns = rows[0].len();
    let widths: Vec<usize> = (0..columns)
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

pub(crate) fn show_shell(result: &Value, json: bool) -> Result<String, Failure> {
    if json {
        return Ok(result.to_string());
    }
    let r: ShellResult = decode(result)?;
    let mut out = if r.timed_out {
        format!("exit {} · timed out after {}ms", r.exit, r.ms)
    } else {
        format!("exit {} · {}ms", r.exit, r.ms)
    };
    if !r.output.is_empty() {
        out.push('\n');
        out.push_str(&r.output);
    }
    if let Some(action) = &r.action {
        let seen = show_action(&serde_json::to_value(action).unwrap_or_default(), false)?;
        let seen = seen.strip_prefix("ok ").unwrap_or(&seen);
        let seen = seen
            .strip_prefix("ran")
            .unwrap_or(seen)
            .trim_start_matches(" · ");
        let seen: Vec<&str> = seen
            .lines()
            .filter(|l| !l.starts_with("hint: nothing changed;"))
            .collect();
        out.push_str("\nscreen: ");
        out.push_str(&seen.join("\n"));
    }
    Ok(out)
}

fn show_action(result: &Value, json: bool) -> Result<String, Failure> {
    if json {
        return Ok(result.to_string());
    }
    let r: ActionResult = decode(result)?;
    let mut head = format!("ok {}", r.summary);
    match (&r.verified, &r.unverified) {
        (Some(true), _) => head.push_str(" · verified"),
        (Some(false), Some(u)) => head.push_str(&format!(" · NOT verified: {}", u.shows)),
        (Some(false), None) => head.push_str(" · NOT verified"),
        _ => {}
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
        out.push_str("\nhint: nothing changed; the tap may have missed, or changed only what the element list does not show (a web checkbox): `phonedriver screenshot` shows the picture");
    }
    if let Some(u) = &r.unverified {
        out.push_str("\nhint: ");
        out.push_str(&u.hint);
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
    let mut params = common.clone();
    params["inline"] = json!(true);
    let shot: ScreenshotResult = decode(&client.call("screenshot", params).await?)?;
    let bytes = match &shot.data_b64 {
        Some(data) => base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| Failure::Local(format!("the daemon sent bad image data: {e}")))?,
        None => std::fs::read(&shot.path)
            .map_err(|e| Failure::Local(format!("cannot read the fixture screenshot: {e}")))?,
    };
    std::fs::write(dir.join(format!("{name}.png")), bytes)
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

    fn tap(autonomous: bool) -> Verb {
        Verb::Tap {
            target: "Pay".into(),
            role: None,
            count: None,
            long: None,
            autonomous,
        }
    }

    #[test]
    fn back_and_home_are_presses() {
        assert_eq!(
            parse_step_line("back").unwrap(),
            json!({"kind": "press", "key": "back"})
        );
        assert_eq!(
            parse_step_line("home").unwrap(),
            json!({"kind": "press", "key": "home"})
        );
        let help = command().render_help().to_string();
        assert!(!help.contains("\n  back"), "{help}");
    }

    #[test]
    fn the_mcp_server_takes_the_device_flag() {
        use clap::FromArgMatches;
        let cli = |args: &[&str]| {
            Cli::from_arg_matches(&command().try_get_matches_from(args).unwrap()).unwrap()
        };
        let pinned = cli(&["phonedriver", "--device", "android:emulator-5554", "mcp"]);
        assert_eq!(pinned.device.as_deref(), Some("android:emulator-5554"));
        let after = cli(&["phonedriver", "mcp", "--device", "android:emulator-5556"]);
        assert_eq!(after.device.as_deref(), Some("android:emulator-5556"));
    }

    #[test]
    fn the_guard_is_off_only_by_the_flag_or_the_environment() {
        assert_eq!(guard_override(&tap(false), None, None), None);
        assert_eq!(guard_override(&tap(false), None, Some("on")), None);
        assert_eq!(guard_override(&tap(false), None, Some("")), None);
        assert_eq!(
            guard_override(&tap(true), None, None),
            Some(Approval::Autonomous)
        );
        assert_eq!(
            guard_override(&tap(false), None, Some("off")),
            Some(Approval::Autonomous)
        );
        assert_eq!(
            guard_override(&tap(false), None, Some("OFF")),
            Some(Approval::Autonomous)
        );
        assert_eq!(
            guard_override(&tap(false), Some(Approval::Once), None),
            Some(Approval::Once)
        );
    }

    #[test]
    fn the_ask_list_takes_the_opt_in_categories_and_refuses_the_rest() {
        use phonedriver_core::error::Category::{Delete, Install, Send};
        assert_eq!(parse_ask(None), Ok(vec![]));
        assert_eq!(parse_ask(Some("")), Ok(vec![]));
        assert_eq!(parse_ask(Some("send")), Ok(vec![Send]));
        assert_eq!(
            parse_ask(Some(" Send, delete ,INSTALL,send,")),
            Ok(vec![Send, Delete, Install])
        );
        for bad in ["sned", "all", "payment", "send;delete"] {
            let e = parse_ask(Some(bad)).unwrap_err();
            assert!(
                e.contains("PHONEDRIVER_ASK") && e.contains("send, delete, install"),
                "{e}"
            );
        }
    }

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
    fn a_shell_command_that_leaves_the_screen_alone_gets_no_missed_tap_hint() {
        let mut action = action(false, "diff", "");
        action["settle_ms"] = json!(250);
        action["summary"] = json!("ran");
        let shell = json!({"exit": 0, "output": "dark", "omitted": 0, "ms": 81, "timed_out": false, "action": action});
        let shown = show_shell(&shell, false).unwrap();
        assert!(shown.contains("screen: no change after 250ms"), "{shown}");
        assert!(!shown.contains("tap"), "{shown}");
    }

    #[test]
    fn a_no_change_hint_does_not_send_the_reader_to_wait_for_a_screen_that_has_settled() {
        let mut v = action(false, "diff", "");
        v["settle_ms"] = json!(502);
        let shown = show_action(&v, false).unwrap();
        assert!(shown.contains("no change after 502ms"), "{shown}");
        let hint = shown.lines().find(|l| l.starts_with("hint: ")).unwrap();
        assert!(!hint.contains("wait settled"), "{hint}");
        assert!(hint.contains("`phonedriver screenshot`"), "{hint}");
    }

    #[test]
    fn devices_with_nothing_connected_says_so_and_what_to_do() {
        let shown = device_table(&json!([]));
        assert!(shown.starts_with("no devices"), "{shown}");
        assert!(shown.contains("hint: Connect a device"), "{shown}");
        assert!(shown.contains("phonedriver doctor"), "{shown}");
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
