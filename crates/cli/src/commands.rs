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
use phonedriver_core::scope::AppScope;

use crate::variables;
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
        // A build without the `android` feature has no `pair` or `untether`, and clap panics when
        // `mut_subcommand` names a command that does not exist.
        if cmd.find_subcommand(verb).is_some() {
            cmd = cmd.mut_subcommand(*verb, |s| without_globals(s, unused));
        }
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
    /// Start an iOS simulator or an Android virtual device that is off, by name or id. Without a name, list the
    /// ones that can be started.
    Boot {
        /// The model (`"iPhone 17"`, `Pixel_9`; any case; the newest system when several have the name) or the id.
        name: Option<String>,
    },
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
        /// Add the screen's accessibility problems: controls with no name a screen reader can say, and tap targets
        /// under 44x44pt on iOS or 48x48 on Android.
        #[arg(long)]
        audit: bool,
        /// Save the device's own tree and a screenshot as a test fixture.
        #[arg(long)]
        save_fixture: Option<String>,
        /// Where `--save-fixture` writes; the default is the core crate's fixture directory.
        #[arg(long)]
        fixture_dir: Option<PathBuf>,
    },
    /// Save a screenshot at one pixel per point and print its path.
    Screenshot {
        /// Write the PNG to this file, as `-o` does.
        #[arg(conflicts_with = "output")]
        file: Option<PathBuf>,
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
        /// `simctl privacy` services (Simulator). A physical device only takes this for apps phonedriver installed or listed in `PHONEDRIVER_APPS_UNDER_TEST`.
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
    /// List the apps that can be launched, or `install <path>`, `stop`, `clear` (data) or `uninstall` one by name or id,
    /// or `grant` or `revoke` its permissions: `phonedriver apps revoke Maps location`, or `notify` it: send it a push
    /// notification to test how it handles one (the iOS Simulator only):
    /// `phonedriver apps notify Mail '{"aps":{"alert":"Hi"}}'` or `--file payload.json`.
    /// On a physical device `clear`, `uninstall`, `grant` and `revoke` only touch apps phonedriver installed, and the
    /// packages in `PHONEDRIVER_APPS_UNDER_TEST` (a comma list the daemon reads when it starts).
    Apps {
        /// What to do to an app. Omit it to list apps.
        #[arg(value_parser = ["install", "stop", "clear", "uninstall", "grant", "revoke", "notify"])]
        action: Option<String>,
        /// The app file to install (on the daemon's machine), or the app to act on.
        target: Option<String>,
        /// For `grant` and `revoke`: `notifications`, `camera`, `location`, or a full name such as
        /// `android.permission.CAMERA` (Android); `photos`, `location` and the other `simctl privacy` services
        /// (Simulator). For `notify`: the push notification as one JSON argument, such as
        /// `'{"aps":{"alert":"Hi"}}'`.
        permissions: Vec<String>,
        /// For `notify`: read the push notification's JSON from this file instead.
        #[arg(long, value_name = "PATH")]
        file: Option<std::path::PathBuf>,
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
    /// Run a command in the device's shell (Android: the agent's shell, no `adb` spawn, or `emu <command>` for an
    /// emulator's console; Simulator: `simctl <subcommand> ...` with the device filled in, and programs only with
    /// PHONEDRIVER_HOST_SHELL=1). Prints the exit code, trimmed output, and what changed on
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
        /// Kill the command after this many seconds (default 30, at most 600).
        #[arg(long)]
        time_limit: Option<u64>,
        /// Read the command from stdin or PHONEDRIVER_SECRET and keep it out of logs and traces.
        #[arg(long)]
        secret: bool,
    },
    /// Save what this session has done so far as a steps file that `replay` runs again with no model. The
    /// session's list of steps starts again empty.
    Save {
        /// Where to write the steps, one JSON object a line.
        file: PathBuf,
    },
    /// Run a steps file from `save`, in order, with no model. It stops at the first step that fails, or that
    /// changes nothing where it changed the screen when saved, prints the screen and says how to go on.
    Replay {
        /// The steps file.
        file: PathBuf,
        /// Start at this step (1 is the first), after doing the one before it yourself.
        #[arg(long, default_value_t = 1)]
        from: usize,
        /// Compare the screen after each step with the screenshot this folder keeps for it, and stop at the first
        /// whose text changed. A step with no screenshot there keeps its own. The status bar is left out.
        #[arg(long, value_name = "DIR")]
        screens: Option<PathBuf>,
        /// A value for `{{name}}` in the file's text, such as `--var user=ada`; repeat it for more. `{{run}}` is a
        /// short id new in every replay. A variable is not a secret: a secret is read from the environment variable
        /// the file names (`PHONEDRIVER_SECRET_1`, ...).
        #[arg(long = "var", value_name = "NAME=VALUE", value_parser = parse_var)]
        vars: Vec<(String, String)>,
    },
    /// Record the screen to an MP4 file (Android and the iOS Simulator): `record start`, the actions to show, then `record stop`, which
    /// prints the file and, beside it, a `.steps.jsonl` file of the actions done meanwhile with their times.
    Record {
        /// What to do: `start` or `stop`.
        #[arg(value_parser = ["start", "stop"])]
        action: String,
        /// With `start`: write the video to this path on the daemon's machine instead of under its `tmp` folder.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Print the last log lines of the app in front, or of the named app, and its last crash (Android).
    Logs {
        /// The app's name or id; the default is the app in front.
        app: Option<String>,
        /// Keep only lines containing one of these words, separated by `|`, ignoring case.
        #[arg(long)]
        grep: Option<String>,
        /// Keep this many of the last lines (default 40, 0 keeps all).
        #[arg(long)]
        head: Option<usize>,
        /// Read this many seconds back (default 60).
        #[arg(long)]
        since: Option<u64>,
    },
    /// Print the device's networks: which are up and by what (Wi-Fi, cellular, VPN), the default one, the app and
    /// user ids a VPN carries, DNS, and always-on VPN state (Android).
    Network,
    /// Print the web page in front as text, whole, without scrolling (Android: Chrome, and the web views of debug builds).
    Page {
        /// Keep only lines containing one of these words, separated by `|`, ignoring case.
        #[arg(long)]
        grep: Option<String>,
        /// Print the whole page, however long.
        #[arg(long)]
        all: bool,
    },
    /// A system setting without its screen (Android 17): `setting find dark` lists keys and what each accepts
    /// (`find "dark theme|media volume"` runs several searches), `setting get <key>` reads one,
    /// `setting set <key> <value> [<key> <value>...]` writes and prints the values afterwards, and
    /// `setting open <screen key>` shows a settings screen.
    Setting {
        /// What to do: `find`, `get`, `set` or `open`.
        #[arg(value_parser = ["find", "get", "set", "open"])]
        action: String,
        /// Words to look for, the key, keys each followed by its new value, or the screen's key.
        #[arg(required = true)]
        args: Vec<String>,
        /// Write a setting the device marks as one its user must confirm, on a physical device.
        #[arg(long)]
        force: bool,
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
    /// `landscape` turns the screen on its side and `portrait` upright (on Android auto-rotate goes off).
    /// On an Android emulator, `fingerprint` touches the fingerprint sensor with the enrolled finger and
    /// `wrong-fingerprint` with another. On an iOS Simulator they show Face ID a face that matches or one that
    /// does not.
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
    /// Wait for `settled`, for a text or `@ref` to appear, for `gone:<text>` (or `gone:@ref`) to leave the screen,
    /// for `vpn:up` or `vpn:down`, or for `log:<text>` (`A|B` for either) to appear in a log line written from now on.
    Wait {
        /// `settled`, a text or `@ref` to appear, `gone:<text>` (or `gone:@ref`) to leave, `vpn:up`, `vpn:down`, or
        /// `log:<text>`.
        condition: String,
        /// With `log:`, read only this app's lines: its name or id.
        #[arg(long)]
        app: Option<String>,
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
    /// Check adb, Android devices and their agent, Xcode, booted simulators and their runners, attached iPhones, and the daemon socket; each line says ok, warn or fail, with a fix. Emulators and Simulators that no live session holds are probed; a physical phone or tablet, or a device another session holds, is listed and left alone. With `--device`, only that device is probed.
    Doctor {
        /// Also probe physical phones and tablets, which are otherwise only listed. A device another live session holds is still left alone unless `--device` names it.
        #[arg(long)]
        all_devices: bool,
    },
    /// Pair an Android 11+ phone over Wi-Fi with a QR code.
    ///
    /// Shows a QR code. On the phone open Developer options > Wireless debugging > Pair device with QR code and scan it; the phone is paired and connected. Needs adb's mDNS and the phone on the same Wi-Fi; otherwise pair with the code (see docs/reference.md).
    #[cfg(feature = "android")]
    Pair,
    /// Make an Android phone reachable over any network, such as Tailscale.
    ///
    /// Runs `adb tcpip 5555` on the phone (USB or Wi-Fi), lists its addresses by interface and connects to the first that answers. adb then listens on every network the phone joins until it reboots, so use it only on networks you trust or with Tailscale; `adb usb` or a reboot turns it off.
    #[cfg(feature = "android")]
    Untether,
    /// Add phonedriver to every agent on this machine, then check the devices.
    ///
    /// Claude Code and Codex are asked with their own `mcp add`; Cursor, VS Code and Gemini CLI get the entry in their JSON file. An agent that already has it is left as it is. Run through `npx phonedriver init` the entry is `npx -y phonedriver mcp`, otherwise this binary.
    Init {
        /// Print what would be done and change nothing.
        #[arg(long)]
        dry_run: bool,
        /// Stop after the agents; do not run the doctor checks.
        #[arg(long)]
        register_only: bool,
    },
    /// Say what changed between two screenshots, as text.
    ///
    /// With one file, the screen now is compared with it. Prints `same`, or the regions whose pixels differ and the text that differs in them (`~ "Save" → "Saved" at 130,208`), with the point to tap. Text is read from the pictures, so it works for any app.
    Compare {
        /// The earlier screenshot (PNG).
        before: PathBuf,
        /// The later screenshot. Without it, the screen now.
        after: Option<PathBuf>,
        /// Also write the later picture with the changed regions boxed.
        #[arg(long)]
        image: Option<PathBuf>,
    },
    /// Serve the tools over MCP on stdio.
    Mcp,
    /// Print SKILL.md, the instructions an agent reads before it uses phonedriver.
    Skill,
    /// Prepare a physical iOS device: `setup` checks the signing team, pairing, Developer Mode and the developer disk image; `wifi on` drops the cable; `register` adds a new device to the team.
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
    /// Check the signing team and each attached device, and do what needs no one at the device: show the Developer Mode switch, mount the
    /// developer disk image with Xcode's tool (the only step that talks to Apple). Then build, install and start the runner, so the next command answers at once. Prints `ok` for steps already done.
    ///
    /// Without Xcode (Windows, Linux) there is no signing team: phonedriver mounts the image itself from Xcode's files and starts the runner you installed with Sideloadly.
    Setup {
        /// Without Xcode: download the image files once from github.com/doronz88/DeveloperDiskImage at a pinned commit, checked against pinned SHA-256 hashes.
        #[arg(long)]
        download_ddi: bool,
        /// Without Xcode: mount the image from this folder, which holds Xcode's BuildManifest.plist, Image.dmg and Image.dmg.trustcache.
        #[arg(long, value_name = "FOLDER")]
        ddi: Option<std::path::PathBuf>,
    },
    /// Add a device to the Apple developer account of the signing team and build the runner for it.
    ///
    /// Needed once for a device that team has never signed for; the error that asks for it says so. Xcode does the registration. An account holds 100 iPhones a year. With more than one device pass `--device ios:<udid>`.
    Register,
    /// Let a device answer this computer over Wi-Fi, so the cable can come off (`on`), or stop that (`off`).
    ///
    /// `on` needs the device on the cable once. It sets the switch Finder shows as "Show this iPhone when on Wi-Fi". The device then answers only computers it already trusts, on the networks it joins. With more than one device pass `--device ios:<udid>`.
    Wifi {
        /// `on` or `off`.
        #[arg(value_parser = ["on", "off"])]
        state: String,
    },
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
        Verb::Compare {
            before,
            after: Some(after),
            image,
        } => {
            let compared = read_picture(&before).and_then(|b| {
                let a = read_picture(&after)?;
                compared(&b, &a, image.as_deref())
            });
            match compared {
                Ok(text) => {
                    println!("{text}");
                    ExitCode::SUCCESS
                }
                Err(why) => {
                    eprintln!("error: {why}");
                    ExitCode::from(1)
                }
            }
        }
        Verb::Init {
            dry_run,
            register_only,
        } => crate::init::run(&socket::home(), dry_run, register_only),
        Verb::Skill => {
            print!("{}", include_str!("../../../SKILL.md"));
            ExitCode::SUCCESS
        }
        Verb::Ios {
            command: IosCommand::Setup { download_ddi, ddi },
        } => {
            if crate::doctor::ios_setup(&socket::home(), download_ddi, ddi.as_deref()) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Verb::Ios {
            command: IosCommand::Register,
        } => {
            if crate::ios_wifi::register(&socket::home(), cli.device.as_deref()) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Verb::Ios {
            command: IosCommand::Wifi { state },
        } => {
            if crate::ios_wifi::run(&socket::home(), state == "on", cli.device.as_deref()) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Verb::Doctor { all_devices } => {
            if crate::doctor::run(&socket::home(), cli.device.as_deref(), all_devices) {
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
            let hidden = client.call("hidden", json!({})).await?["count"]
                .as_u64()
                .unwrap_or(0);
            let mut out = if result.as_array().is_some_and(Vec::is_empty) {
                no_devices(&client.call("boot", json!({})).await?)
            } else {
                with_default_device(device_table(&result), g.device.as_deref())
            };
            if hidden > 0 {
                out.push_str(&format!(
                    "\n{hidden} {} hidden by the device list",
                    if hidden == 1 { "device" } else { "devices" }
                ));
            }
            Ok(out)
        }
        Verb::Boot { name } => {
            let result = client.call("boot", json!({"name": name})).await?;
            Ok(if g.json {
                result.to_string()
            } else {
                show_boot(&result)
            })
        }
        Verb::Snapshot {
            diff,
            interactive,
            within,
            max_lines,
            pixels,
            audit,
            save_fixture,
            fixture_dir,
        } => {
            let mut p = json!({
                "diff": diff, "interactive": interactive, "max_lines": max_lines, "pixels": pixels, "audit": audit,
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
            let mut out = if obs.text.is_empty() {
                "no change".to_string()
            } else {
                obs.text
            };
            if obs.empty {
                out.push('\n');
                out.push_str(EMPTY_SCREEN);
            }
            Ok(out)
        }
        Verb::Screenshot {
            file,
            output,
            marks,
        } => {
            let output = output.or(file);
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
        Verb::Compare {
            before,
            after: _,
            image,
        } => {
            let before = read_picture(&before).map_err(Failure::Local)?;
            let result = client
                .call("screenshot", with(json!({"marks": false, "inline": true})))
                .await?;
            let shot: ScreenshotResult = decode(&result)?;
            let now = base64::engine::general_purpose::STANDARD
                .decode(shot.data_b64.unwrap_or_default())
                .map_err(|e| Failure::Local(format!("the daemon sent bad image data: {e}")))?;
            compared(&before, &now, image.as_deref()).map_err(Failure::Local)
        }
        Verb::Wait { condition, app } => {
            let mut p = json!({"condition": wait_condition(&condition, app.as_deref())});
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
        Verb::Save { file } => {
            let cannot =
                |e: std::io::Error| Failure::Local(format!("cannot write {}: {e}", file.display()));
            let r = client.call("journal", with(json!({"keep": true}))).await?;
            let mut steps = r["steps"].as_array().cloned().unwrap_or_default();
            name_secrets(&mut steps);
            if !steps.is_empty() {
                let text: String = steps.iter().map(|s| format!("{s}\n")).collect();
                let mut beside = file.clone().into_os_string();
                beside.push(".tmp");
                let beside = std::path::PathBuf::from(beside);
                std::fs::write(&beside, text)
                    .and_then(|()| std::fs::rename(&beside, &file))
                    .inspect_err(|_| {
                        let _ = std::fs::remove_file(&beside);
                    })
                    .map_err(cannot)?;
                client
                    .call("journal", with(json!({"drop": steps.len()})))
                    .await?;
            }
            if g.json {
                return Ok(
                    json!({"path": file, "steps": steps.len(), "over": r["over"]}).to_string(),
                );
            }
            Ok(show_saved(
                &file.display().to_string(),
                &steps,
                r["over"] == true,
            ))
        }
        Verb::Replay {
            file,
            from,
            screens,
            vars,
        } => {
            let text = std::fs::read_to_string(&file)
                .map_err(|e| Failure::Local(format!("cannot read {}: {e}", file.display())))?;
            let steps = text
                .lines()
                .enumerate()
                .filter(|(_, l)| !l.trim().is_empty())
                .map(|(i, l)| {
                    serde_json::from_str::<Value>(l).map_err(|e| {
                        Failure::Local(format!(
                            "line {} of {} is not a step: {e}",
                            i + 1,
                            file.display()
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut values: std::collections::BTreeMap<String, String> = vars.into_iter().collect();
            if values.contains_key(variables::RUN) {
                return Err(Failure::Local(format!(
                    "`{}` is filled in by phonedriver, with a new id for each replay; choose another name",
                    variables::RUN
                )));
            }
            let id = variables::run_id();
            values.insert(variables::RUN.into(), id.clone());
            let name = file.display().to_string();
            let (steps, used) = variables::fill(&steps, from, &values)
                .map_err(|missing| missing_variables(&name, &missing))?;
            let run = Replay {
                file: name,
                steps,
                from,
                screens,
                run: used.contains(variables::RUN).then_some(id),
            };
            run.play(client, &act, &with, g.json).await
        }
        Verb::Record { action, output } => {
            let mut params = json!({"action": action});
            if let Some(out) = output {
                if action == "stop" {
                    return Err(Failure::Local(
                        "`record stop` takes no file: the recording goes where `record start` said"
                            .into(),
                    ));
                }
                let path = std::path::absolute(&out).map_err(|e| {
                    Failure::Local(format!("cannot resolve {}: {e}", out.display()))
                })?;
                params["path"] = json!(path);
            }
            let r = client.call("record", with(params)).await?;
            if g.json {
                return Ok(r.to_string());
            }
            Ok(show_record(&r))
        }
        Verb::Logs {
            app,
            grep,
            head,
            since,
        } => {
            let r = client
                .call(
                    "logs",
                    with(json!({"app": app, "grep": grep, "head": head, "since_s": since})),
                )
                .await?;
            if g.json {
                return Ok(r.to_string());
            }
            Ok(show_logs(&r, since.unwrap_or(60)))
        }
        Verb::Network => {
            let r = client.call("network", with(json!({}))).await?;
            if g.json {
                return Ok(r.to_string());
            }
            Ok(show_network(&r))
        }
        Verb::Page { grep, all } => {
            let r = client
                .call("page", with(json!({"grep": grep, "all": all})))
                .await?;
            if g.json {
                return Ok(r.to_string());
            }
            Ok(show_page(&r, grep.as_deref()))
        }
        Verb::Setting {
            action,
            args,
            force,
        } => {
            let mut p = match (action.as_str(), args.as_slice()) {
                ("find", words) => json!({"find": words.join(" ")}),
                ("get", [key]) => json!({"key": key}),
                ("set", [key, value]) => json!({"key": key, "value": value, "force": force}),
                ("set", pairs) if pairs.len() % 2 == 0 => {
                    let values: serde_json::Map<String, Value> = pairs
                        .chunks(2)
                        .map(|pair| (pair[0].clone(), json!(pair[1])))
                        .collect();
                    json!({"values": values, "force": force})
                }
                ("open", [screen]) => json!({"open": screen}),
                ("get", _) => return Err(Failure::Local("setting get takes one key".into())),
                ("set", _) => {
                    return Err(Failure::Local(
                        "setting set takes keys, each followed by its value\nexample: phonedriver setting set dark_ui_mode/dark_ui_activated true".into(),
                    ));
                }
                _ => return Err(Failure::Local("setting open takes one screen key".into())),
            };
            p["settle"] = json!(!g.no_settle);
            if let Some(t) = g.timeout {
                p["timeout_ms"] = json!(t);
            }
            if let Some(t) = g.expect_change {
                p["expect_change_ms"] = json!(t);
            }
            let r = client.call("setting", with(p)).await?;
            show_setting(&r, &args.join(" "), g.json)
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
            permissions,
            file,
        } => {
            let target = target
                .ok_or_else(|| Failure::Local(format!("apps {action} needs an app or a path")))?;
            let p = if action == "install" {
                json!({"action": action, "path": target})
            } else if action == "notify" {
                let payload = match (file, permissions.as_slice()) {
                    (Some(path), []) => std::fs::read_to_string(&path).map_err(|e| {
                        Failure::Local(format!("cannot read {}: {e}", path.display()))
                    })?,
                    (None, [json]) => json.clone(),
                    _ => {
                        return Err(Failure::Local(
                            "apps notify needs the push notification as JSON: the `payload` field over MCP, one quoted argument or `--file payload.json` on the command line".into(),
                        ));
                    }
                };
                json!({"action": action, "app": target, "payload": payload})
            } else {
                json!({"action": action, "app": target, "permissions": permissions})
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
        | Verb::Doctor { .. }
        | Verb::Init { .. }
        | Verb::Ios { .. }
        | Verb::Mcp
        | Verb::Skill
        | Verb::Version => Ok(String::new()),
    }
}

fn read_picture(path: &std::path::Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

/// The report on two screenshots, and with `image` the later one written with its changed regions boxed.
pub(crate) fn compared(
    before: &[u8],
    after: &[u8],
    image: Option<&std::path::Path>,
) -> Result<String, String> {
    let report = crate::compare::compare(before, after, 0.0)?;
    let mut text = crate::compare::render(&report);
    if let Some(path) = image.filter(|_| !report.regions.is_empty()) {
        let marks: Vec<phonedriver_video::marks::Mark> = report
            .regions
            .iter()
            .enumerate()
            .map(|(i, r)| phonedriver_video::marks::Mark {
                label: (i + 1).to_string(),
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
            })
            .collect();
        let boxed = phonedriver_video::marks::draw_marks(after, &marks)?;
        std::fs::write(path, boxed).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        text.push_str(&format!("\n{}", path.display()));
    }
    Ok(text)
}

fn wait_condition(condition: &str, app: Option<&str>) -> Value {
    if let Some(state) = condition.strip_prefix("vpn:") {
        return json!({"vpn": state});
    }
    if let Some(pattern) = condition.strip_prefix("log:") {
        return json!({"log": {"pattern": pattern, "app": app}});
    }
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
        Verb::Wait { condition, app } => {
            json!({"kind": "wait", "condition": wait_condition(condition, app.as_deref())})
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
    } else if r.observation.kind == Kind::Full {
        head.push_str(" · new screen");
    } else if r.observation.text.is_empty() {
        head.push_str(" · no element changed");
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

const NO_DEVICES: &str = "no devices\nhint: Connect a device or start an emulator; `phonedriver doctor` says what is missing.";

fn off_devices(boot: &Value) -> Vec<phonedriver_core::error::DeviceSummary> {
    serde_json::from_value(boot["off"].clone()).unwrap_or_default()
}

/// What `devices` prints with nothing connected, given the `boot` reply that lists what is off.
fn no_devices(boot: &Value) -> String {
    match off_devices(boot).as_slice() {
        [] => NO_DEVICES.into(),
        off => format!(
            "no devices\ncan be started: {}\nhint: Start one with `phonedriver boot <name>`.",
            phonedriver_core::error::can_be_started(off)
        ),
    }
}

fn show_boot(result: &Value) -> String {
    if let Some(id) = result["id"].as_str() {
        let word = match result["already"].as_bool() {
            Some(true) => "already on:",
            _ => "booted",
        };
        return format!(
            "{word} {id} · {} · {}",
            result["model"].as_str().unwrap_or_default(),
            result["os"].as_str().unwrap_or_default()
        );
    }
    let off = off_devices(result);
    if off.is_empty() {
        return "nothing to start\nhint: `boot` starts iOS simulators and Android virtual devices that are off. Create one in Xcode or Android Studio.".into();
    }
    let mut rows = vec![vec!["ID".to_string(), "MODEL".into(), "OS".into()]];
    rows.extend(off.into_iter().map(|d| vec![d.id, d.model, d.os]));
    table(&rows)
}

fn device_table(result: &Value) -> String {
    let Ok(devices) = serde_json::from_value::<Vec<DeviceEntry>>(result.clone()) else {
        return result.to_string();
    };
    if devices.is_empty() {
        return NO_DEVICES.into();
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
    table(&rows)
}

fn table(rows: &[Vec<String>]) -> String {
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

/// The last line of a reply whose screen gives nothing to act on. The MCP server attaches the picture in its place.
pub(crate) const EMPTY_SCREEN: &str = "hint: this screen gives no element list; `phonedriver screenshot` shows it, and `tap x,y` taps a point on the picture";

/// Observed in the replay evaluation (`docs/bench/replay.md`): files-new-folder saved two launches that had not worked
/// and two shell commands the agent used to look around, the reply said only "saved 8 steps", and all eight ran again
/// in every replay.
const SAVED_STEP_CHARS: usize = 60;

/// `--var name=value`.
fn parse_var(text: &str) -> Result<(String, String), String> {
    match text.split_once('=') {
        Some((name, value)) if variables::is_name(name) => Ok((name.into(), value.into())),
        _ => Err("expected NAME=VALUE, where NAME has letters, digits, `_`, `-` or `.`".into()),
    }
}

/// A file's `{{name}}`s that no `--var` gave, refused before any step runs.
fn missing_variables(file: &str, missing: &[String]) -> Failure {
    let Failure::Rpc(mut error) = refused(&format!(
        "{file} needs {} nobody gave: {}",
        plural(missing.len() as u64, "variable"),
        missing.join(", ")
    )) else {
        unreachable!()
    };
    let give: Vec<String> = missing.iter().map(|n| format!("--var {n}=VALUE")).collect();
    if let Some(data) = error.data.as_object_mut() {
        data.insert("hint".into(), json!(format!(
            "Add {} (MCP: vars). {{{{{}}}}} is filled in by phonedriver; variables are not secrets, so they may show in replies.",
            give.join(" "),
            variables::RUN
        )));
    }
    Failure::Rpc(error)
}

/// Why a saved secret cannot be typed, and what to do about it.
enum NoSecret {
    NoVariable,
    NotAllowed(String),
    Unset(String),
}

impl NoSecret {
    fn message(&self) -> String {
        match self {
            NoSecret::NoVariable => {
                "this step types a secret but names no environment variable".into()
            }
            NoSecret::NotAllowed(name) => format!(
                "{name} is not a variable a replay may type: its name must begin {}",
                variables::SECRET_PREFIX
            ),
            NoSecret::Unset(name) => format!("{name} is not set"),
        }
    }
}

/// The action of a saved step with the secret it names filled in from this process's environment. A `type` that
/// is not a secret is returned as it is.
fn with_secret(action: &Value) -> Result<Value, NoSecret> {
    let mut action = action.clone();
    if action["kind"] != "type" || action["secret"] != true {
        return Ok(action);
    }
    let name = action["secret_env"]
        .as_str()
        .ok_or(NoSecret::NoVariable)?
        .to_string();
    if !name.starts_with(variables::SECRET_PREFIX) {
        return Err(NoSecret::NotAllowed(name));
    }
    let text = std::env::var(&name)
        .ok()
        .filter(|t| !t.is_empty())
        .ok_or(NoSecret::Unset(name))?;
    if let Some(fields) = action.as_object_mut() {
        fields.remove("secret_env");
        fields.insert("text".into(), json!(text));
    }
    Ok(action)
}

fn secret_hint(why: &NoSecret, n: usize, total: usize) -> String {
    let first = match why {
        NoSecret::NoVariable => format!(
            "Add \"secret_env\": \"{}1\" to the step and set it",
            variables::SECRET_PREFIX
        ),
        NoSecret::NotAllowed(_) => format!(
            "Rename the variable in the file to start with {} and set it",
            variables::SECRET_PREFIX
        ),
        NoSecret::Unset(name) => format!("Set {name} and replay again"),
    };
    let rest = if n < total {
        format!(
            "then replay the same file from step {} for the {} left.",
            n + 1,
            plural((total - n) as u64, "step")
        )
    } else {
        "it is the last step.".into()
    };
    let sep = if n < total { "," } else { ";" };
    format!("{first}, or type the secret yourself with `type --secret`{sep} {rest}")
}

/// Gives each typed secret of the steps about to be saved the name of the environment variable a replay reads it
/// from: `PHONEDRIVER_SECRET_1`, `_2`, ... in order. A person may rename them in the file.
fn name_secrets(steps: &mut [Value]) {
    let mut n = 0;
    for step in steps {
        let action = &mut step["params"]["action"];
        if action["kind"] == "type" && action["secret"] == true && action["secret_env"].is_null() {
            n += 1;
            action["secret_env"] = json!(format!("{}{n}", variables::SECRET_PREFIX));
        }
    }
}

/// `save`: the steps that went into the file, one line each, and how many of them a replay cannot be sure of.
pub(crate) fn show_saved(path: &str, steps: &[Value], over: bool) -> String {
    if steps.is_empty() {
        return format!(
            "nothing to save: this session has done no action since its last save ({path} is as it was)"
        );
    }
    let count = |field: &str| steps.iter().filter(|s| !s[field].is_null()).count() as u64;
    let mut out = format!("saved {} to {path}", plural(steps.len() as u64, "step"));
    for (i, step) in steps.iter().enumerate() {
        let what = Replay::describe(step);
        let cut: String = what.chars().take(SAVED_STEP_CHARS).collect();
        let more = if cut.len() < what.len() { "…" } else { "" };
        out.push_str(&format!("\n{}. {cut}{more}", i + 1));
    }
    if steps.len() > 1 {
        out.push_str(
            "\nthe file holds one step a line: delete the line of a step that was a detour",
        );
    }
    let by_position = count("by_position");
    if by_position > 0 {
        out.push_str(&format!(
            "\n{by_position} of them tap a position, because no text singles out the element: they hold only while the layout does"
        ));
    }
    let names: Vec<&str> = steps
        .iter()
        .filter_map(|s| s["params"]["action"]["secret_env"].as_str())
        .collect();
    if !names.is_empty() {
        out.push_str(&format!(
            "\nthe secret text is not in the file: set {} before replaying (rename a variable in the file if you like)",
            names.join(", ")
        ));
    }
    let manual = count("manual");
    if manual > 0 {
        out.push_str(&format!(
            "\n{manual} of them need a person or a model (a secret): a replay stops there"
        ));
    }
    if over {
        out.push_str(&format!(
            "\nonly the first {} steps were kept",
            crate::daemon::journal::MAX
        ));
    }
    out
}

/// A steps file being run again.
pub(crate) struct Replay {
    pub file: String,
    pub steps: Vec<Value>,
    pub from: usize,
    pub screens: Option<PathBuf>,
    pub run: Option<String>,
}

/// The screen after a replayed step against the screenshot kept for it.
enum Screen {
    Kept,
    Same,
    /// Pixels changed in this many regions and no text did.
    Silent(usize),
}

impl Replay {
    /// What a step does, in words that do not depend on the refs of the run it was saved from.
    fn describe(step: &Value) -> String {
        if let Some(what) = step["manual"].as_str() {
            return what.to_string();
        }
        let params = &step["params"];
        let action = &params["action"];
        let target =
            |t: &Value| match (t["text"].as_str(), t["id"].as_str(), t["point"].as_object()) {
                (Some(text), ..) => format!(" \"{text}\""),
                (None, Some(id), _) => format!(" {id}"),
                (None, None, Some(p)) => format!(" {},{}", p["x"], p["y"]),
                _ => String::new(),
            };
        match step["method"].as_str().unwrap_or_default() {
            "act" => {
                let kind = action["kind"].as_str().unwrap_or("act");
                let what = ["app", "url", "key", "direction"]
                    .iter()
                    .find_map(|f| action[*f].as_str())
                    .map(|v| format!(" {v}"))
                    .unwrap_or_default();
                let typed = match (action["secret_env"].as_str(), action["text"].as_str()) {
                    (Some(name), _) => format!(" secret ${name}"),
                    (None, Some(t)) => format!(" \"{t}\""),
                    (None, None) => String::new(),
                };
                let on = ["target", "into", "until", "from", "at"]
                    .iter()
                    .map(|f| target(&action[*f]))
                    .find(|t| !t.is_empty())
                    .unwrap_or_default();
                let into = if typed.is_empty() || on.is_empty() {
                    ""
                } else {
                    " into"
                };
                format!("{kind}{what}{typed}{into}{on}")
            }
            "wait" => {
                let c = &params["condition"];
                if let Some(state) = c["vpn"].as_str() {
                    return format!("wait vpn:{state}");
                }
                if let Some(pattern) = c["log"]["pattern"].as_str() {
                    return format!("wait log:{pattern}");
                }
                match (target(&c["appears"]), target(&c["gone"])) {
                    (appears, _) if !appears.is_empty() => format!("wait{appears}"),
                    (_, gone) if !gone.is_empty() => format!("wait gone:{}", gone.trim_start()),
                    _ => "wait settled".into(),
                }
            }
            "shell" => format!("shell {}", params["command"].as_str().unwrap_or_default()),
            method => format!("{method} {params}"),
        }
    }

    pub async fn play(
        &self,
        client: &mut Client,
        act: &(dyn Fn(Value) -> Value + Sync),
        with: &(dyn Fn(Value) -> Value + Sync),
        json: bool,
    ) -> Result<String, Failure> {
        let total = self.steps.len();
        if self.from == 0 || self.from > total {
            return Err(Failure::Local(format!(
                "{} has {}; there is no step {}",
                self.file,
                plural(total as u64, "step"),
                self.from
            )));
        }
        let started = std::time::Instant::now();
        let mut done = Vec::new();
        let (mut kept, mut same) = (0u64, 0u64);
        for (i, step) in self.steps.iter().enumerate().skip(self.from - 1) {
            let mut hint = None;
            let outcome = match (step["manual"].as_str(), step["method"].as_str()) {
                (Some(what), _) => Err(refused(what)),
                (None, Some("act")) => match with_secret(&step["params"]["action"]) {
                    Ok(action) => client.call("act", act(action)).await,
                    Err(why) => {
                        hint = Some(secret_hint(&why, i + 1, total));
                        Err(refused(&why.message()))
                    }
                },
                (None, Some(method @ ("wait" | "shell" | "setting" | "app"))) => {
                    client.call(method, with(step["params"].clone())).await
                }
                _ => Err(refused("this is not a step this version can run")),
            };
            let outcome = outcome.and_then(|r| match unexpected_exit(step, &r) {
                Some(why) => Err(refused(&why)),
                None => Ok(r),
            });
            let outcome = match outcome {
                Ok(r) if step["changed"] == true && r["changed"] == false => Err(refused(
                    "it changed nothing this time; when it was saved it changed the screen",
                )),
                other => other,
            };
            let mut line = match outcome {
                Ok(r) => format!(
                    "{}. {}",
                    i + 1,
                    r["summary"]
                        .as_str()
                        .map_or_else(|| Replay::describe(step), str::to_string)
                ),
                Err(failure) => {
                    return Err(self
                        .stopped(client, with, i + 1, &done, failure, hint)
                        .await);
                }
            };
            if let Some(dir) = &self.screens {
                match screen(client, with, dir, i + 1).await {
                    Ok(Screen::Kept) => kept += 1,
                    Ok(Screen::Same) => same += 1,
                    Ok(Screen::Silent(n)) => {
                        same += 1;
                        line.push_str(&format!(
                            " · screen: {} changed, no text",
                            plural(n as u64, "region")
                        ));
                    }
                    Err((failure, hint)) => {
                        return Err(self
                            .stopped(client, with, i + 1, &done, failure, hint)
                            .await);
                    }
                }
            }
            done.push(line);
        }
        let ran = done.len() as u64;
        let ms = started.elapsed().as_millis() as u64;
        if json {
            let mut out = json!({"ok": true, "steps": ran, "ms": ms});
            if let Some(run) = &self.run {
                out["run"] = json!(run);
            }
            if self.screens.is_some() {
                out["screens"] = json!({"same": same, "kept": kept});
            }
            return Ok(out.to_string());
        }
        let run = self
            .run
            .as_ref()
            .map(|id| format!(" (run {id})"))
            .unwrap_or_default();
        let mut out = format!(
            "replayed {} in {:.1} s{run}\n{}",
            plural(ran, "step"),
            ms as f64 / 1000.0,
            done.join("\n")
        );
        if let Some(dir) = &self.screens {
            let dir = dir.display();
            out.push('\n');
            out.push_str(&match (same, kept) {
                (0, k) => format!(
                    "kept {} in {dir}; a replay with --screens {dir} compares each step's screen with them",
                    plural(k, "screen")
                ),
                (s, 0) => format!("{} matched those in {dir}", plural(s, "screen")),
                (s, k) => format!(
                    "{} matched those in {dir}, and {k} new kept there",
                    plural(s, "screen")
                ),
            });
        }
        if let Some(screen) = screen_now(client, with).await {
            out.push_str("\nscreen now:\n");
            out.push_str(&screen);
        }
        Ok(out)
    }

    /// The failure of step `n`, with what ran before it, the screen, and how to go on from here.
    async fn stopped(
        &self,
        client: &mut Client,
        with: &(dyn Fn(Value) -> Value + Sync),
        n: usize,
        done: &[String],
        failure: Failure,
        hint: Option<String>,
    ) -> Failure {
        let total = self.steps.len();
        let step = Replay::describe(&self.steps[n - 1]);
        let mut error = match failure {
            Failure::Rpc(e) => e,
            Failure::Local(m) => match refused(&m) {
                Failure::Rpc(e) => e,
                Failure::Local(_) => return Failure::Local(m),
            },
        };
        let mut text = format!("step {n} of {total} ({step}) failed: {}", error.message);
        for line in done {
            text.push('\n');
            text.push_str(line);
        }
        if let Some(screen) = screen_now(client, with).await {
            text.push_str("\nscreen now:\n");
            text.push_str(&screen);
        }
        let hint = hint.unwrap_or_else(|| {
            if n < total {
                format!(
                    "Do step {n} yourself, then replay the same file from step {} for the {} left.",
                    n + 1,
                    plural((total - n) as u64, "step")
                )
            } else {
                format!("Do step {n} yourself; it is the last one.")
            }
        });
        error.message = text;
        if let Some(data) = error.data.as_object_mut() {
            data.insert("hint".into(), json!(hint));
            data.insert("step".into(), json!(n));
        }
        Failure::Rpc(error)
    }
}

/// Observed on the API 37 emulator, about one replay in seven: a step's screenshot caught a switch's thumb halfway
/// across (read as "q•") or the app's window still black after a restart, and the same screen matched a moment later.
/// So a screen that differs is looked at again, every `LOOK_AGAIN`, until it has stayed the same for `STILL_FOR` or
/// `LOOK_FOR` has passed, and one about to be kept must first look the same twice.
const LOOK_AGAIN: std::time::Duration = std::time::Duration::from_millis(250);
const STILL_FOR: std::time::Duration = std::time::Duration::from_millis(1500);
const LOOK_FOR: std::time::Duration = std::time::Duration::from_secs(5);

/// Compares the screen after step `n` with the screenshot `dir` keeps for it, or keeps it there when there is none.
/// A failure comes with the hint for it.
async fn screen(
    client: &mut Client,
    with: &(dyn Fn(Value) -> Value + Sync),
    dir: &std::path::Path,
    n: usize,
) -> Result<Screen, (Failure, Option<String>)> {
    let path = dir.join(format!("{n:02}.png"));
    let local = |m: String| (refused(&m), None);
    let (mut now, bar) = shoot(client, with).await?;
    let still = |a: &[u8], b: &[u8]| {
        crate::compare::still(a, b, bar)
            .map_err(|e| local(format!("its screen cannot be compared with itself: {e}")))
    };
    let kept = match std::fs::read(&path) {
        Ok(kept) => kept,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let started = std::time::Instant::now();
            loop {
                tokio::time::sleep(LOOK_AGAIN).await;
                let (next, _) = shoot(client, with).await?;
                let settled = still(&now, &next)?;
                now = next;
                if settled || started.elapsed() >= LOOK_FOR {
                    break;
                }
            }
            std::fs::create_dir_all(dir)
                .and_then(|()| std::fs::write(&path, &now))
                .map_err(|e| local(format!("cannot keep {}: {e}", path.display())))?;
            return Ok(Screen::Kept);
        }
        Err(e) => return Err(local(format!("cannot read {}: {e}", path.display()))),
    };
    let started = std::time::Instant::now();
    loop {
        let report = crate::compare::compare(&kept, &now, bar).map_err(|e| {
            local(format!(
                "its screen cannot be compared with {}: {e}",
                path.display()
            ))
        })?;
        if report.regions.is_empty() {
            return Ok(Screen::Same);
        }
        if report.text.is_empty() && report.unread.is_none() {
            return Ok(Screen::Silent(report.regions.len()));
        }
        let still_since = std::time::Instant::now();
        loop {
            if still_since.elapsed() >= STILL_FOR || started.elapsed() >= LOOK_FOR {
                return Err((
                    refused(&format!(
                        "its screen is not the one in {}\n{}",
                        path.display(),
                        crate::compare::render(&report)
                    )),
                    Some(format!(
                        "If the new screen is right, delete {} and replay again to keep it.",
                        path.display()
                    )),
                ));
            }
            tokio::time::sleep(LOOK_AGAIN).await;
            let (next, _) = shoot(client, with).await?;
            if !still(&now, &next)? {
                now = next;
                break;
            }
        }
    }
}

/// A screenshot's PNG and the rows its status bar covers.
async fn shoot(
    client: &mut Client,
    with: &(dyn Fn(Value) -> Value + Sync),
) -> Result<(Vec<u8>, f32), (Failure, Option<String>)> {
    let result = client
        .call("screenshot", with(json!({"marks": false, "inline": true})))
        .await
        .map_err(|f| (f, None))?;
    let shot: ScreenshotResult = decode(&result).map_err(|f| (f, None))?;
    let png = base64::engine::general_purpose::STANDARD
        .decode(shot.data_b64.unwrap_or_default())
        .map_err(|e| {
            (
                Failure::Local(format!("the daemon sent bad image data: {e}")),
                None,
            )
        })?;
    Ok((png, shot.status_bar.unwrap_or(0) as f32))
}

fn unexpected_exit(step: &Value, reply: &Value) -> Option<String> {
    if step["method"] != "shell" {
        return None;
    }
    let expected = match step.get("expect_exit") {
        None => 0,
        Some(Value::Null) => return None,
        Some(n) => match n.as_i64() {
            Some(n) => n,
            None => {
                return Some("the step's expect_exit is neither a whole number nor null".into());
            }
        },
    };
    if reply["timed_out"] == true {
        return Some("it ran past its time limit and was stopped".into());
    }
    let exit = reply["exit"].as_i64()?;
    (exit != expected).then(|| format!("it exited {exit}; the step expects {expected}"))
}

fn refused(detail: &str) -> Failure {
    Failure::Rpc(
        phonedriver_core::error::Error::Refused {
            detail: detail.to_string(),
        }
        .into(),
    )
}

async fn screen_now(client: &mut Client, with: &(dyn Fn(Value) -> Value + Sync)) -> Option<String> {
    let reply = client.call("snapshot", with(json!({}))).await.ok()?;
    reply["text"].as_str().map(str::to_string)
}

/// `record`: where a started recording goes, or the finished file with its length, size and steps file.
pub(crate) fn show_record(result: &Value) -> String {
    let path = result["path"].as_str().unwrap_or_default();
    let Some(ms) = result["ms"].as_u64() else {
        return format!("recording the screen to {path}; `phonedriver record stop` ends it");
    };
    let bytes = result["bytes"].as_u64().unwrap_or(0);
    let size = if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1e6)
    } else {
        format!("{} kB", bytes.div_ceil(1000))
    };
    let mut out = format!(
        "{path}\n{:.1} s, {}x{}, {} frames, {size}",
        ms as f64 / 1000.0,
        result["width"],
        result["height"],
        result["frames"],
    );
    if let Some(steps) = result["steps_path"].as_str() {
        out.push_str(&format!(
            "\nsteps: {} with their times in {steps}",
            result["steps"]
        ));
    }
    if let Some(why) = result["early"].as_str() {
        out.push_str(&format!("\nended early: {why}"));
    }
    out
}

/// `logs`: the crash first, then how many earlier lines were left out, then the lines.
pub(crate) fn show_logs(result: &Value, seconds: u64) -> String {
    let app = result["app"].as_str().unwrap_or_default();
    let output = result["output"].as_str().unwrap_or_default();
    let mut out = Vec::new();
    if let Some(crash) = result["crash"].as_str() {
        out.push(format!("crashed: {crash}"));
    }
    match result["omitted"].as_u64() {
        Some(n) if n > 0 => out.push(format!(
            "… {n} earlier lines (narrow them with grep, or set head to 0 for all)"
        )),
        _ => {}
    }
    if !output.is_empty() {
        out.push(output.to_string());
    }
    if out.is_empty() {
        return format!("no log lines from {app} in the last {seconds} s");
    }
    out.join("\n")
}

/// `network`: one line for each network, then private DNS and always-on VPN, then the fields the platform did not give.
pub(crate) fn show_network(result: &Value) -> String {
    let list = |v: &Value| {
        v.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default()
    };
    let networks = result["networks"].as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    if networks.is_empty() {
        out.push("no network is up".to_string());
    }
    for n in &networks {
        let mut line = format!(
            "{} {}{}",
            n["id"],
            list(&n["transports"]).replace(',', "+"),
            if n["default"] == true {
                " (default)"
            } else {
                ""
            }
        );
        if let Some(owner) = n["owner"].as_str() {
            line.push_str(&format!(" of {owner}"));
        }
        if let Some(interface) = n["interface"].as_str() {
            line.push_str(&format!(" on {interface}"));
        }
        if n["uids"].as_array().is_some_and(|u| !u.is_empty()) {
            line.push_str(&format!(" for uids {}", list(&n["uids"])));
        }
        if let Some(under) = n["underlying"].as_array().filter(|u| !u.is_empty()) {
            let ids: Vec<String> = under.iter().map(|u| u.to_string()).collect();
            line.push_str(&format!(" over {}", ids.join(",")));
        }
        match list(&n["dns"]).as_str() {
            "" => line.push_str(" · dns not given"),
            dns => line.push_str(&format!(" · dns {dns}")),
        }
        out.push(line);
    }
    let missing: Vec<&str> = result["missing"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
        .unwrap_or_default();
    let given = |field: &str, text: String| (!missing.contains(&field)).then_some(text);
    let lines = [
        given(
            "private DNS",
            format!(
                "private dns {}",
                result["private_dns"].as_str().unwrap_or("?")
            ),
        ),
        given(
            "always-on VPN app",
            format!(
                "always-on vpn {}",
                result["always_on_vpn"].as_str().unwrap_or("none")
            ),
        ),
        given(
            "always-on VPN lockdown",
            format!(
                "lockdown {}",
                if result["lockdown"] == true {
                    "on"
                } else {
                    "off"
                }
            ),
        ),
    ];
    out.extend(lines.into_iter().flatten());
    if !missing.is_empty() {
        out.push(format!("not given: {}", missing.join(", ")));
    }
    out.join("\n")
}

/// `page`: the title and address, then the page's lines, then what was left out and how to get it.
pub(crate) fn show_page(result: &Value, grep: Option<&str>) -> String {
    let text = result["text"].as_str().unwrap_or_default();
    let mut out = format!(
        "page \"{}\" {}",
        result["title"].as_str().unwrap_or_default(),
        result["url"].as_str().unwrap_or_default()
    );
    match (text.is_empty(), grep) {
        (true, Some(words)) => out.push_str(&format!("\nno line of the page has \"{words}\"")),
        (true, None) => out.push_str("\nthe page has no text"),
        (false, _) => {
            out.push('\n');
            out.push_str(text);
        }
    }
    let left = result["omitted_lines"].as_u64().unwrap_or(0);
    let chars = result["omitted_chars"].as_u64().unwrap_or(0);
    if chars > 0 {
        let what = match left {
            0 => format!("{chars} more characters of this line"),
            1 => format!("1 more line ({chars} characters)"),
            n => format!("{n} more lines ({chars} characters)"),
        };
        out.push_str(&format!(
            "\n… {what}\nhint: `--grep words` keeps only the lines with them, `--all` prints everything"
        ));
    }
    out
}

/// `setting`: the matching lines of a search, `key = value` for a read, `ok key = value` and the screen change for
/// a write, or the action line of an opened screen.
pub(crate) fn show_setting(result: &Value, asked: &str, json: bool) -> Result<String, Failure> {
    if json {
        return Ok(result.to_string());
    }
    if let Some(lines) = result["lines"].as_array() {
        let mut out: Vec<String> = lines
            .iter()
            .filter_map(|l| l.as_str().map(String::from))
            .collect();
        if out.is_empty() {
            return Ok(format!(
                "no setting matches {asked:?}; try one other word, or `shell`"
            ));
        }
        match result["more"].as_u64() {
            Some(n) if n > 0 => out.push(format!("… {n} more; add a word to narrow")),
            _ => {}
        }
        return Ok(out.join("\n"));
    }
    let mut out = if let Some(values) = result["values"].as_object() {
        values
            .iter()
            .map(|(k, v)| format!("ok {k} = {}", v.as_str().unwrap_or_default()))
            .collect::<Vec<_>>()
            .join("\n")
    } else if let Some(key) = result["key"].as_str() {
        let value = result["value"].as_str().unwrap_or_default();
        if result.get("action").is_none() {
            return Ok(match result["hint"].as_str() {
                Some(hint) => format!("{key} = {value}\nnote: {hint}"),
                None => format!("{key} = {value}"),
            });
        }
        format!("ok {key} = {value}")
    } else {
        return show_action(result, false);
    };
    let Some(action) = result.get("action") else {
        return Ok(out);
    };
    if action["changed"] == true {
        let seen = show_action(action, false)?;
        let seen = seen.strip_prefix("ok set").unwrap_or(&seen);
        out.push_str("\nscreen: ");
        out.push_str(seen.trim_start_matches(" · "));
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
            // Benchmark passes p6 and p7 (2026-11-10, a slow emulator): 17 launches answered "not settled after", and
            // agents followed most of them with a snapshot or a screenshot to check lines that were already current.
            head.push_str(&format!(
                " · still animating after {}ms, read as it is now",
                r.settle_ms
            ));
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
        } else if r.observation.kind == Kind::Full {
            head.push_str(" · new screen");
        } else if r.observation.text.is_empty() && r.verified.is_none() {
            head.push_str(" · no element changed");
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
    if let Some(crash) = &r.crashed {
        out.push_str(&format!(
            "\ncrashed: {} stopped during this action: {}\nhint: `phonedriver logs {}` prints its last log lines",
            crash.app, crash.what, crash.app
        ));
    }
    if r.observation.empty {
        if let Some(u) = &r.unverified {
            out.push_str("\nhint: ");
            out.push_str(&u.hint);
        }
        out.push('\n');
        out.push_str(EMPTY_SCREEN);
        return Ok(out);
    }
    if let Some(n) = r.unchanged_repeats {
        out.push_str(&format!("\nhint: this action has changed nothing {n} times in a row, so repeating it will not help: `phonedriver screenshot` shows what is on screen, or act on another target"));
    } else if r.settled
        && !r.changed
        && matches!(r.summary.as_str(), "pressed landscape" | "pressed portrait")
    {
        out.push_str("\nhint: the screen did not turn: it was already that way, or the app in front supports one orientation only (an iPhone's home screen, most launchers)");
    } else if r.settled && !r.changed {
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

/// The packages in a comma list such as `com.example.app, com.example.other`; blanks are dropped.
fn apps_under_test(list: Option<&str>) -> Vec<String> {
    list.unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

fn run_daemon(trace: TraceOptions) -> ExitCode {
    let Ok(rt) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    else {
        eprintln!("error: cannot start the async runtime");
        return ExitCode::from(1);
    };
    let app_scope = match AppScope::parse(
        &std::env::var("PHONEDRIVER_APP_SCOPE").unwrap_or_default(),
    ) {
        Ok(scope) => scope,
        Err(why) => {
            eprintln!(
                "error: PHONEDRIVER_APP_SCOPE {why}: list packages such as com.example.app,com.example.other"
            );
            return ExitCode::from(1);
        }
    };
    rt.block_on(async {
        let sources = Sources(vec![
            #[cfg(feature = "android")]
            ("android:", Arc::new(AdbSource::from_env())),
            ("sim:", Arc::new(SimSource::new(socket::home()))),
            ("ios:", Arc::new(PhysicalSource::new(socket::home()))),
        ]);
        let daemon = Daemon::start(
            Arc::new(sources),
            Config {
                home: socket::home(),
            },
            trace,
            apps_under_test(std::env::var("PHONEDRIVER_APPS_UNDER_TEST").ok().as_deref()),
            crate::daemon::policy::Policy::from_env(&socket::home()),
            app_scope,
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
    #[test]
    fn the_apps_under_test_list_splits_on_commas_and_drops_blanks() {
        assert_eq!(
            super::apps_under_test(Some(" com.example.app, ,com.example.other,")),
            vec!["com.example.app", "com.example.other"]
        );
        assert!(super::apps_under_test(None).is_empty());
        assert!(super::apps_under_test(Some("")).is_empty());
    }

    use super::*;

    #[test]
    fn a_wait_step_is_described_as_the_command_that_waits() {
        let wait =
            |c: Value| Replay::describe(&json!({"method": "wait", "params": {"condition": c}}));
        assert_eq!(
            wait(json!({"appears": {"text": "Name", "role": null}})),
            "wait \"Name\""
        );
        assert_eq!(
            wait(json!({"gone": {"text": "Loading", "role": null}})),
            "wait gone:\"Loading\""
        );
        assert_eq!(
            wait(json!({"appears": {"id": "com.example:id/ok"}})),
            "wait com.example:id/ok"
        );
        assert_eq!(wait(json!({"settled": true})), "wait settled");
    }

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
    fn a_screenshot_takes_its_file_as_a_plain_argument_or_after_the_flag() {
        for words in [
            ["phonedriver", "screenshot", "shot.png"].as_slice(),
            ["phonedriver", "screenshot", "-o", "shot.png"].as_slice(),
        ] {
            let Some(Verb::Screenshot { output, file, .. }) =
                Cli::try_parse_from(words).unwrap().command
            else {
                panic!("{words:?} is a screenshot");
            };
            assert_eq!(
                output.or(file),
                Some(PathBuf::from("shot.png")),
                "{words:?}"
            );
        }
        assert!(
            Cli::try_parse_from(["phonedriver", "screenshot", "a.png", "-o", "b.png"]).is_err(),
            "two files for one picture"
        );
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
        assert_eq!(shown, "ok tapped @e1 · no element changed");
        let shown = show_action(&action(true, "diff", "~ @e1 x → y"), false).unwrap();
        assert!(!shown.contains("no element changed"), "{shown}");
        let mut typed = action(true, "diff", "");
        typed["summary"] = json!("typed into @e4");
        typed["verified"] = json!(true);
        assert_eq!(
            show_action(&typed, false).unwrap(),
            "ok typed into @e4 · verified"
        );
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
    fn a_screen_that_kept_animating_says_its_lines_were_read_after_the_wait() {
        let mut v = action(true, "full", "@e1 button \"Stay signed out\"");
        v["settled"] = json!(false);
        v["settle_ms"] = json!(9150);
        let shown = show_action(&v, false).unwrap();
        let head = shown.lines().next().unwrap();
        assert!(
            head.ends_with("still animating after 9150ms, read as it is now"),
            "{head}"
        );
    }

    #[test]
    fn wait_takes_vpn_state_and_a_log_line_beside_a_text_and_a_target_that_is_gone() {
        assert_eq!(wait_condition("vpn:up", None), json!({"vpn": "up"}));
        assert_eq!(wait_condition("vpn:down", None), json!({"vpn": "down"}));
        assert_eq!(
            wait_condition("log:code=|ready", Some("Example")),
            json!({"log": {"pattern": "code=|ready", "app": "Example"}})
        );
        assert_eq!(
            wait_condition("log:x", None),
            json!({"log": {"pattern": "x", "app": null}})
        );
        assert_eq!(
            wait_condition("gone:Loading", None),
            json!({"gone": {"text": "Loading"}})
        );
        assert_eq!(
            parse_step_line("wait log:ready --app Example").unwrap(),
            json!({"kind": "wait", "condition": {"log": {"pattern": "ready", "app": "Example"}}})
        );
        assert_eq!(
            parse_step_line("wait vpn:up").unwrap(),
            json!({"kind": "wait", "condition": {"vpn": "up"}})
        );
    }

    #[test]
    fn a_saved_vpn_or_log_wait_reads_as_one_in_a_replay() {
        let d = |c: Value| Replay::describe(&json!({"method": "wait", "params": {"condition": c}}));
        assert_eq!(d(json!({"vpn": "up"})), "wait vpn:up");
        assert_eq!(
            d(json!({"log": {"pattern": "ready", "app": null}})),
            "wait log:ready"
        );
    }

    #[test]
    fn the_network_view_prints_a_line_for_each_network_then_what_was_not_given() {
        let shown = show_network(&json!({
            "networks": [
                {"id": 101, "transports": ["wifi"], "default": true, "interface": "wlan0", "dns": ["10.0.2.3"], "uids": [], "underlying": []},
                {"id": 102, "transports": ["wifi", "vpn"], "default": false, "interface": "tun0", "dns": ["10.77.0.1"],
                 "owner": "dev.example.vpn", "uids": ["0-99999"], "underlying": [101]}],
            "always_on_vpn": "dev.example.vpn",
            "lockdown": true,
            "missing": ["private DNS"]
        }));
        assert_eq!(
            shown,
            "101 wifi (default) on wlan0 · dns 10.0.2.3\n102 wifi+vpn of dev.example.vpn on tun0 for uids 0-99999 over 101 · dns 10.77.0.1\nalways-on vpn dev.example.vpn\nlockdown on\nnot given: private DNS"
        );
        let none = show_network(&json!({"networks": [], "missing": []}));
        assert_eq!(
            none,
            "no network is up\nprivate dns ?\nalways-on vpn none\nlockdown off"
        );
    }

    #[test]
    fn a_turn_that_changed_nothing_says_the_app_may_not_turn_and_says_nothing_about_taps() {
        for key in ["landscape", "portrait"] {
            let mut v = action(false, "diff", "");
            v["summary"] = json!(format!("pressed {key}"));
            let shown = show_action(&v, false).unwrap();
            let hint = shown.lines().find(|l| l.starts_with("hint: ")).unwrap();
            assert_eq!(
                hint,
                "hint: the screen did not turn: it was already that way, or the app in front supports one orientation only (an iPhone's home screen, most launchers)"
            );
        }
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
    fn the_third_action_that_changed_nothing_is_told_to_stop_repeating_it() {
        let mut v = action(false, "diff", "");
        v["settle_ms"] = json!(502);
        v["unchanged_repeats"] = json!(3);
        let shown = show_action(&v, false).unwrap();
        let hints: Vec<&str> = shown.lines().filter(|l| l.starts_with("hint: ")).collect();
        assert_eq!(
            hints,
            [
                "hint: this action has changed nothing 3 times in a row, so repeating it will not help: `phonedriver screenshot` shows what is on screen, or act on another target"
            ],
            "{shown}"
        );
    }

    #[test]
    fn an_action_that_crashed_the_app_prints_the_crash_and_how_to_read_the_log() {
        let mut v = action(true, "full", "@e1 button \"Apps\"");
        v["crashed"] = json!({"app": "com.example", "what": "java.lang.IllegalStateException: boom at com.example.Main.onClick(Main.kt:12)"});
        let shown = show_action(&v, false).unwrap();
        assert!(
            shown.contains("\ncrashed: com.example stopped during this action: java.lang.IllegalStateException: boom at com.example.Main.onClick(Main.kt:12)\nhint: `phonedriver logs com.example` prints its last log lines"),
            "{shown}"
        );
    }

    #[test]
    fn record_prints_where_it_writes_then_the_finished_file() {
        assert_eq!(
            show_record(&json!({"path": "/t/a.mp4"})),
            "recording the screen to /t/a.mp4; `phonedriver record stop` ends it"
        );
        assert_eq!(
            show_record(&json!({
                "path": "/t/a.mp4", "ms": 12340, "frames": 345, "width": 1080, "height": 2424,
                "bytes": 2_140_000, "steps": 4, "steps_path": "/t/a.mp4.steps.jsonl",
                "early": "the screen changed size or turned"
            })),
            "/t/a.mp4\n12.3 s, 1080x2424, 345 frames, 2.1 MB\nsteps: 4 with their times in /t/a.mp4.steps.jsonl\nended early: the screen changed size or turned"
        );
        assert_eq!(
            show_record(&json!({
                "path": "/t/a.mp4", "ms": 900, "frames": 3, "width": 360, "height": 800,
                "bytes": 41_200, "steps": 0
            })),
            "/t/a.mp4\n0.9 s, 360x800, 3 frames, 42 kB"
        );
    }

    #[test]
    fn logs_with_nothing_to_show_says_so() {
        let shown = show_logs(
            &json!({"app": "com.example", "output": "", "omitted": 0, "crash": null}),
            60,
        );
        assert_eq!(shown, "no log lines from com.example in the last 60 s");
        let shown = show_logs(
            &json!({"app": "com.example", "output": "a\nb", "omitted": 3, "crash": null}),
            60,
        );
        assert_eq!(
            shown,
            "… 3 earlier lines (narrow them with grep, or set head to 0 for all)\na\nb"
        );
    }

    #[test]
    fn devices_with_nothing_connected_says_so_and_what_to_do() {
        let shown = device_table(&json!([]));
        assert!(shown.starts_with("no devices"), "{shown}");
        let off = json!({"off": [
            {"id": "sim:C", "model": "iPhone 16", "os": "iOS 26.5"},
            {"id": "sim:A", "model": "iPad", "os": "iOS 18.6"}]});
        assert_eq!(
            no_devices(&off),
            "no devices\ncan be started: iPhone 16 (iOS 26.5), iPad (iOS 18.6)\nhint: Start one with `phonedriver boot <name>`."
        );
        assert_eq!(no_devices(&json!({"off": []})), shown);
        assert_eq!(
            show_boot(&off),
            "ID     MODEL      OS\nsim:C  iPhone 16  iOS 26.5\nsim:A  iPad       iOS 18.6"
        );
        assert_eq!(
            show_boot(&json!({"off": []})),
            "nothing to start\nhint: `boot` starts iOS simulators and Android virtual devices that are off. Create one in Xcode or Android Studio."
        );
        assert_eq!(
            show_boot(&json!({"id": "sim:C", "model": "iPhone 16", "os": "iOS 26.5"})),
            "booted sim:C · iPhone 16 · iOS 26.5"
        );
        assert_eq!(
            show_boot(
                &json!({"id": "sim:C", "model": "iPhone 16", "os": "iOS 26.5", "already": true})
            ),
            "already on: sim:C · iPhone 16 · iOS 26.5"
        );
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
