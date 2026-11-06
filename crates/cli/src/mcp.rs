//! `phone-use mcp`: the CLI verbs as MCP tools over stdio (architecture 10.2). A tool maps its arguments onto the
//! verb of the same name and returns the text the verb prints; nothing else happens here.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use crate::client::{Client, Failure};
use crate::commands::{Globals, Verb, execute_with, run_batch};
use crate::wire::ScreenshotResult;

#[derive(Clone)]
pub struct Mcp {
    socket: Option<PathBuf>,
    session: String,
    client: Arc<Mutex<Option<Client>>>,
}

#[derive(Deserialize, JsonSchema)]
struct BatchArgs {
    device: Option<String>,
    /// Run in order. Each step is an object: `tool` (tap, type, swipe, scroll, press, launch or wait) plus that tool's arguments, such as {"tool":"tap","target":"@e3"}.
    steps: Vec<serde_json::Value>,
    /// Give up on a step's settle or wait after this many milliseconds. Default 5000.
    timeout_ms: Option<u64>,
}

#[derive(Deserialize, JsonSchema)]
struct AppsArgs {
    device: Option<String>,
    /// `install` (an app file at `app`: an .apk or .app path on the machine running the daemon), `stop`, `clear` (its data) or `uninstall`. Omit it to list apps.
    action: Option<String>,
    /// The app's name or id, or for `install` the file path.
    app: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct DevicesArgs {
    device: Option<String>,
    /// Give up this session's hold on the device so another session can use it.
    release: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct ScreenshotArgs {
    device: Option<String>,
    /// Draw a box and the `@eN` ref on every element that can be tapped or typed into, so you can say `tap @e12` from the picture.
    marks: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct ClipboardArgs {
    device: Option<String>,
    /// Put this text on the clipboard. Omit it to read the clipboard.
    set: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ShellArgs {
    device: Option<String>,
    command: String,
    /// Keep only output lines containing one of these words (`a|b`), in any case.
    grep: Option<String>,
    /// Lines of output to keep; default 40, 0 for all.
    head: Option<u32>,
    /// False skips waiting for the screen to settle.
    settle: Option<bool>,
    /// Run a command refused on a physical device; only when the user asked for exactly that.
    force: Option<bool>,
    /// The command holds a secret: it is never logged or traced.
    secret: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct SnapshotArgs {
    device: Option<String>,
    /// Print only what changed since the last observation.
    diff: Option<bool>,
    /// Print only elements that can be acted on.
    interactive: Option<bool>,
    /// An @ref: print only that element and its children.
    within: Option<String>,
    /// Cut the output to this many lines.
    max_lines: Option<u32>,
    /// Read the pixels with OCR and add text the tree lacks (automatic when the tree is empty).
    pixels: Option<bool>,
    /// List only the elements whose text contains this, in any case.
    find: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TapArgs {
    device: Option<String>,
    /// `@e5` (a ref from the last snapshot), `x,y` in points, or the text of an element, such as `Dark theme`.
    target: String,
    /// With a text target: only an element of this role, such as button, cell, textbox or switch.
    role: Option<String>,
    /// Tap this many times; 2 is a double tap.
    count: Option<u32>,
    /// Hold the press for this many milliseconds.
    long: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TypeArgs {
    device: Option<String>,
    /// The text to type.
    text: String,
    /// The field: an @ref, `x,y` or the field's text. Omit it to type into the focused field.
    into: Option<String>,
    /// `paste` (default), `set` (replace the field's value) or `keys` (one key at a time, ASCII only on Android).
    mode: Option<String>,
    /// Press enter afterwards.
    submit: Option<bool>,
    /// With `into`, add to the field's text instead of replacing it.
    append: Option<bool>,
    /// The text is a password, code or other secret: it is never shown back, logged or traced.
    secret: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SwipeArgs {
    device: Option<String>,
    /// up, down, left or right: where the finger moves. Needed unless `to` or `pinch` is given.
    direction: Option<String>,
    /// Start at this target's center instead of the screen's.
    from: Option<String>,
    /// Drag from `from` to this target (press, hold, move, lift) to reorder or move an item.
    to: Option<String>,
    /// `out` zooms in, `in` zooms out: two fingers, centered on `from` or the screen.
    pinch: Option<String>,
    /// Share of the screen to cross, 0 to 1. Default 0.5.
    distance: Option<f32>,
    /// A quick flick that may fling.
    fast: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ScrollArgs {
    device: Option<String>,
    /// up, down, left or right: where more content should appear.
    direction: String,
    /// The list to scroll: an @ref. Default is the largest scrollable element.
    r#in: Option<String>,
    /// Keep scrolling until this text or @ref is on screen.
    until: Option<String>,
    /// Give up after this many scrolls. Default 30.
    max: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PressArgs {
    device: Option<String>,
    /// home, back, recents, notifications, lock, volume-up, volume-down, enter or hide-keyboard.
    key: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LaunchArgs {
    device: Option<String>,
    /// An app name or package or bundle id, such as `Settings`. Pass this or `url`.
    app: Option<String>,
    /// A web link or deep link to open in the app that handles it. Pass this or `app`.
    url: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct WaitArgs {
    device: Option<String>,
    /// `settled`, a text or @ref to appear, or `gone:<text>` to leave the screen.
    condition: String,
    /// Give up after this many milliseconds. Default 5000.
    timeout_ms: Option<u64>,
}

impl TapArgs {
    fn verb(self) -> Verb {
        Verb::Tap {
            target: self.target,
            role: self.role,
            count: self.count,
            long: self.long,
        }
    }
}

impl SwipeArgs {
    fn verb(self) -> Result<Verb, &'static str> {
        match (self.direction, self.to, self.pinch) {
            (_, Some(to), None) => Ok(Verb::Drag {
                from: self
                    .from
                    .ok_or("`to` needs `from`: where the drag starts")?,
                to,
            }),
            (_, None, Some(direction)) if direction == "in" || direction == "out" => {
                Ok(Verb::Pinch {
                    direction,
                    at: self.from,
                    distance: self.distance,
                })
            }
            (_, None, Some(_)) => Err("`pinch` is `in` or `out`"),
            (Some(direction), None, None) => Ok(Verb::Swipe {
                direction,
                from: self.from,
                distance: self.distance,
                fast: self.fast.unwrap_or(false),
            }),
            (None, None, None) => Err("pass `direction`, `to` or `pinch`"),
            _ => Err("pass one of `direction`, `to` or `pinch`"),
        }
    }
}

impl ScrollArgs {
    fn verb(self) -> Verb {
        Verb::Scroll {
            direction: self.direction,
            within: self.r#in,
            until: self.until,
            max: self.max,
        }
    }
}

impl LaunchArgs {
    fn verb(self) -> Result<Verb, &'static str> {
        match (self.app, self.url) {
            (Some(app), None) => Ok(Verb::Launch { app }),
            (None, Some(url)) => Ok(Verb::Open { url }),
            _ => Err("pass app or url, one of them"),
        }
    }
}

/// A step of `batch`: `tool` and that tool's arguments, as the same verb the tool runs. Secret text is refused,
/// because a batch's text goes through the daemon's trace like any other and a secret must be typed alone.
fn step_verb(step: serde_json::Value) -> Result<Verb, String> {
    let serde_json::Value::Object(mut fields) = step else {
        return Err("a step is an object with a `tool`".into());
    };
    let Some(serde_json::Value::String(tool)) = fields.remove("tool") else {
        return Err("a step needs `tool`".into());
    };
    let rest = serde_json::Value::Object(fields);
    let args = |e: serde_json::Error| e.to_string();
    match tool.as_str() {
        "tap" => Ok(serde_json::from_value::<TapArgs>(rest)
            .map_err(args)?
            .verb()),
        "swipe" => serde_json::from_value::<SwipeArgs>(rest)
            .map_err(args)?
            .verb()
            .map_err(String::from),
        "scroll" => Ok(serde_json::from_value::<ScrollArgs>(rest)
            .map_err(args)?
            .verb()),
        "press" => Ok(Verb::Press {
            key: serde_json::from_value::<PressArgs>(rest).map_err(args)?.key,
        }),
        "wait" => Ok(Verb::Wait {
            condition: serde_json::from_value::<WaitArgs>(rest)
                .map_err(args)?
                .condition,
        }),
        "launch" => serde_json::from_value::<LaunchArgs>(rest)
            .map_err(args)?
            .verb()
            .map_err(String::from),
        "type" => {
            let a = serde_json::from_value::<TypeArgs>(rest).map_err(args)?;
            if a.text.is_empty() {
                return match a.into {
                    Some(target) => Ok(Verb::Clear { target }),
                    None => Err("to clear a field pass `into`: the field to empty".into()),
                };
            }
            if a.secret.unwrap_or(false) {
                return Err(
                    "secret text cannot go in a batch; call `type` with secret on its own".into(),
                );
            }
            Ok(Verb::Type {
                text: Some(a.text),
                into: a.into,
                mode: a.mode,
                submit: a.submit.unwrap_or(false),
                append: a.append.unwrap_or(false),
                secret: false,
            })
        }
        other => Err(format!(
            "`{other}` cannot be a step; use tap, type, swipe, scroll, press, launch or wait"
        )),
    }
}

impl Mcp {
    /// A server that talks to the daemon at `socket`, or starts the one at the default path when it is `None`.
    pub fn new(socket: Option<PathBuf>, session: String) -> Mcp {
        Mcp {
            socket,
            session,
            client: Arc::default(),
        }
    }

    async fn connect(&self) -> Result<Client, Failure> {
        match &self.socket {
            Some(path) => Client::connect_to_as(path, &self.session, "mcp").await,
            None => Client::connect_as(self.session.clone(), "mcp").await,
        }
    }

    /// Runs `f` on the one connection this server keeps for its whole life, so the daemon sees the lease end when the
    /// connection closes, and one call at a time, because the phone is one shared screen.
    async fn with_client<T>(
        &self,
        f: impl AsyncFnOnce(&mut Client) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let mut slot = self.client.lock().await;
        let client = match slot.take() {
            Some(c) => slot.insert(c),
            None => slot.insert(self.connect().await?),
        };
        let result = f(client).await;
        if matches!(&result, Err(Failure::Rpc(e)) if e.code == -32603) {
            *slot = None;
        }
        result
    }

    fn globals(device: Option<String>) -> Globals {
        Globals {
            device: device.or_else(|| std::env::var("PHONE_USE_DEVICE").ok()),
            json: false,
            no_settle: false,
            timeout: None,
            expect_change: None,
            share: false,
        }
    }

    async fn run(
        &self,
        device: Option<String>,
        verb: Verb,
        secret: Option<String>,
    ) -> CallToolResult {
        self.run_with(Self::globals(device), verb, secret).await
    }

    async fn run_with(
        &self,
        globals: Globals,
        verb: Verb,
        secret: Option<String>,
    ) -> CallToolResult {
        let result = self
            .with_client(async |client| execute_with(client, verb, secret, &globals).await)
            .await;
        match result {
            Ok(text) => CallToolResult::success(vec![ContentBlock::text(success_text(text))]),
            Err(f) => failure(&f),
        }
    }
}

fn failure(f: &Failure) -> CallToolResult {
    let text = match f.hint() {
        Some(h) => format!("{}\nhint: {}", f.message(), for_mcp(&h)),
        None => f.message(),
    };
    CallToolResult::error(vec![ContentBlock::text(text)])
}

/// Success text with its `hint:` lines in tool vocabulary; element lines are left alone.
fn success_text(text: String) -> String {
    text.lines()
        .map(|l| match l.strip_prefix("hint: ") {
            Some(h) => format!("hint: {}", for_mcp(h)),
            None => l.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Hints are written for the command line. The MCP agent has tools and arguments, not `phone-use` commands and flags.
fn for_mcp(hint: &str) -> String {
    const PHRASES: &[(&str, &str)] = &[
        (
            "`phone-use press hide-keyboard`",
            "`press` with key `hide-keyboard`",
        ),
        ("`phone-use press enter`", "`press` with key `enter`"),
        ("`phone-use snapshot --no-settle`", "`snapshot`"),
        (
            "`phone-use wait settled`",
            "`wait` with condition `settled`",
        ),
        (
            "`--mode paste` or `--mode keys`",
            "`mode` `paste` or `keys`",
        ),
        ("`--mode keys`", "`mode` `keys`"),
        ("`--within <list ref>`", "`in` with the list's ref"),
        ("`--device <id>`", "`device`"),
        ("`--timeout`", "`timeout_ms`"),
        ("`--max`", "`max`"),
        (
            "Run `phone-use doctor` to check the connection.",
            "Ask the user to run `phone-use doctor` to check the connection.",
        ),
        (
            "Run `phone-use doctor` to see what this device supports.",
            "Try another approach; this device does not support that.",
        ),
        ("Run `phone-use ", "Call `"),
    ];
    let mut out = hint.to_string();
    for (from, to) in PHRASES {
        out = out.replace(from, to);
    }
    while let Some(start) = out.find("`phone-use ") {
        let Some(len) = out[start + 1..].find('`') else {
            break;
        };
        let inner = out[start + 1 + "phone-use ".len()..start + 1 + len].to_string();
        let mut words = inner.split_whitespace();
        let tool = words.next().unwrap_or_default().to_string();
        let rest: Vec<&str> = words.collect();
        let shown = if tool == "scroll" && !rest.is_empty() {
            format!("`scroll` {}", rest.join(" "))
        } else {
            format!("`{tool}`")
        };
        out.replace_range(start..start + len + 2, &shown);
    }
    out
}

fn bad(message: &str) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message)])
}

#[tool_router]
impl Mcp {
    #[tool(
        description = "List connected devices with their id, model, OS, tier and which session holds each. Call it first when unsure which device to use. With `release`, give up this session's hold on a device."
    )]
    async fn devices(&self, Parameters(a): Parameters<DevicesArgs>) -> CallToolResult {
        if a.release.unwrap_or(false) {
            return self.run(a.device, Verb::Release, None).await;
        }
        self.run(None, Verb::Devices, None).await
    }

    #[tool(
        description = "Run a shell command on the device: one call returns exit code, trimmed output and the screen change, faster than adb. Prefer it to UI taps for settings and intents: `cmd uimode night yes`, `settings put system screen_brightness 20`, `am start -a android.intent.action.VIEW -d <url>`, `dumpsys window | grep mCurrentFocus`, `input keyevent 3`. Simulator: `simctl ui appearance dark`. Physical devices refuse wipes and removing others' apps."
    )]
    async fn shell(&self, Parameters(a): Parameters<ShellArgs>) -> CallToolResult {
        let secret = a.secret.unwrap_or(false);
        let verb = Verb::Shell {
            command: if secret {
                None
            } else {
                Some(a.command.clone())
            },
            grep: a.grep,
            head: a.head.map(|h| h as usize),
            force: a.force.unwrap_or(false),
            time_limit: None,
            secret,
        };
        let mut globals = Self::globals(a.device);
        globals.no_settle = a.settle == Some(false);
        self.run_with(globals, verb, secret.then_some(a.command))
            .await
    }

    #[tool(description = "Read the device clipboard, or set it with `set`.")]
    async fn clipboard(&self, Parameters(a): Parameters<ClipboardArgs>) -> CallToolResult {
        let verb = Verb::Clipboard {
            action: a.set.as_ref().map(|_| "set".to_string()),
            text: a.set,
        };
        self.run(a.device, verb, None).await
    }

    #[tool(
        description = "List the apps that can be launched, one `name  id` per line (use it when `launch` says there is no app with the name you gave), or with `action` install, stop, clear data of or uninstall one. On a physical device, clear and uninstall only work on apps phone-use installed."
    )]
    async fn apps(&self, Parameters(a): Parameters<AppsArgs>) -> CallToolResult {
        let verb = Verb::Apps {
            action: a.action,
            target: a.app,
        };
        self.run(a.device, verb, None).await
    }

    #[tool(
        description = "Show what is on screen as lines like `@e4 button \"Save\"`, or only what changed with diff. Refs name elements for tap, type, swipe and scroll."
    )]
    async fn snapshot(&self, Parameters(a): Parameters<SnapshotArgs>) -> CallToolResult {
        if let Some(text) = a.find {
            return self
                .run(a.device, Verb::Find { text, role: None }, None)
                .await;
        }
        let verb = Verb::Snapshot {
            diff: a.diff.unwrap_or(false),
            interactive: a.interactive.unwrap_or(false),
            within: a.within,
            max_lines: a.max_lines.map(|n| n as usize),
            pixels: a.pixels.unwrap_or(false),
            save_fixture: None,
            fixture_dir: None,
        };
        self.run(a.device, verb, None).await
    }

    #[tool(
        description = "Take a screenshot and return it as an image, one pixel per point. Use it when the snapshot does not show what you need, such as a canvas, a map or an image. With `marks`, elements are boxed with their @refs."
    )]
    async fn screenshot(&self, Parameters(a): Parameters<ScreenshotArgs>) -> CallToolResult {
        let marks = a.marks.unwrap_or(false);
        let globals = Self::globals(a.device);
        let shot = self
            .with_client(async |client| {
                let mut params = json!({"inline": true, "marks": marks});
                if let Some(d) = &globals.device {
                    params["device"] = json!(d);
                }
                let reply = client.call("screenshot", params).await?;
                serde_json::from_value::<ScreenshotResult>(reply).map_err(|e| {
                    Failure::Local(format!("the daemon sent an unexpected reply: {e}"))
                })
            })
            .await;
        let shot = match shot {
            Ok(s) => s,
            Err(f) => return failure(&f),
        };
        let Some(data) = shot.data_b64 else {
            return bad("the daemon sent no image data");
        };
        CallToolResult::success(vec![
            ContentBlock::image(data, "image/png"),
            ContentBlock::text(match shot.marks {
                Some(n) => format!(
                    "{}x{} at scale {}, {n} marked elements",
                    shot.width, shot.height, shot.scale
                ),
                None => format!("{}x{} at scale {}", shot.width, shot.height, shot.scale),
            }),
        ])
    }

    #[tool(
        description = "Tap an element by @ref, by its text, or at x,y points. Returns what changed once the screen settled. Prefer refs; use points only from a screenshot. One call at a time."
    )]
    async fn tap(&self, Parameters(a): Parameters<TapArgs>) -> CallToolResult {
        let device = a.device.clone();
        self.run(device, a.verb(), None).await
    }

    #[tool(
        name = "type",
        description = "Type text into a field and report whether the field now holds it. Empty text with `into` clears the field. Set secret for passwords and codes; only type those when the user gave them for this task. One call at a time."
    )]
    async fn r#type(&self, Parameters(a): Parameters<TypeArgs>) -> CallToolResult {
        let secret = a.secret.unwrap_or(false);
        if a.text.is_empty() && !secret {
            return match a.into {
                Some(target) => self.run(a.device, Verb::Clear { target }, None).await,
                None => bad("to clear a field pass `into`: the field to empty"),
            };
        }
        let verb = Verb::Type {
            text: if secret { None } else { Some(a.text.clone()) },
            into: a.into,
            mode: a.mode,
            submit: a.submit.unwrap_or(false),
            append: a.append.unwrap_or(false),
            secret,
        };
        self.run(a.device, verb, secret.then_some(a.text)).await
    }

    #[tool(
        description = "Drag one finger across the screen the way a user swipes: to flip pages, pull down, dismiss. With `to`, drag an item to a target; with `pinch`, zoom with two fingers. To read more of a list use scroll instead. One call at a time."
    )]
    async fn swipe(&self, Parameters(a): Parameters<SwipeArgs>) -> CallToolResult {
        let device = a.device.clone();
        match a.verb() {
            Ok(verb) => self.run(device, verb, None).await,
            Err(why) => bad(why),
        }
    }

    #[tool(
        description = "Scroll a list so more content shows in the direction, or until the text or @ref in `until` is on screen. It never flings past a target. One call at a time."
    )]
    async fn scroll(&self, Parameters(a): Parameters<ScrollArgs>) -> CallToolResult {
        let device = a.device.clone();
        self.run(device, a.verb(), None).await
    }

    #[tool(
        description = "Press a button: home, back, recents, notifications, lock, volume-up, volume-down, enter, or hide-keyboard to close the on-screen keyboard. Returns what changed once the screen settled. One call at a time."
    )]
    async fn press(&self, Parameters(a): Parameters<PressArgs>) -> CallToolResult {
        self.run(a.device, Verb::Press { key: a.key }, None).await
    }

    #[tool(
        description = "Open an app by name, package or bundle id (`app`), or open a web link or deep link (`url`) in the app that handles it. Returns the first screen. One call at a time."
    )]
    async fn launch(&self, Parameters(a): Parameters<LaunchArgs>) -> CallToolResult {
        let device = a.device.clone();
        match a.verb() {
            Ok(verb) => self.run(device, verb, None).await,
            Err(why) => bad(why),
        }
    }

    #[tool(
        description = "Run several actions in order in one call, such as tap a field, type, press enter. Stops at the first failure and says which step failed. Returns one diff for the whole batch, so use it instead of sending calls together."
    )]
    async fn batch(&self, Parameters(a): Parameters<BatchArgs>) -> CallToolResult {
        let mut steps = Vec::new();
        for (i, step) in a.steps.into_iter().enumerate() {
            let json = step_verb(step)
                .and_then(|v| crate::commands::step_json(&v, None).map_err(|f| f.message()));
            match json {
                Ok(j) => steps.push(j),
                Err(why) => return bad(&format!("step {}: {why}", i + 1)),
            }
        }
        if steps.is_empty() {
            return bad("a batch needs at least one step");
        }
        let mut globals = Self::globals(a.device);
        globals.timeout = a.timeout_ms;
        let result = self
            .with_client(async |client| run_batch(client, steps, &globals).await)
            .await;
        match result {
            Ok(text) => CallToolResult::success(vec![ContentBlock::text(success_text(text))]),
            Err(f) => failure(&f),
        }
    }

    #[tool(
        description = "Wait until the screen settles, a text or @ref appears, or `gone:<text>` leaves. Use it instead of sleeping. Fails with a timeout that names the condition."
    )]
    async fn wait(&self, Parameters(a): Parameters<WaitArgs>) -> CallToolResult {
        let mut globals = Self::globals(a.device);
        globals.timeout = a.timeout_ms;
        let verb = Verb::Wait {
            condition: a.condition,
        };
        self.run_with(globals, verb, None).await
    }
}

#[tool_handler(
    name = "phone-use",
    instructions = "Operate a phone or tablet. Start with `snapshot`, act on the @refs it prints, and read the lines each action returns: they say what changed once the screen settled. Calls act on one shared screen: send one at a time and read each reply before the next. Use `wait` instead of sleeping. Use `screenshot` only when the snapshot lacks what you need."
)]
impl ServerHandler for Mcp {}

/// The runtime the server runs on. rmcp starts one task per request in the order requests arrive; on one thread the
/// tasks take the call lock in that order, so calls an agent sends together act in the order it sent them.
pub fn server_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
}

pub fn run() -> ExitCode {
    let Ok(rt) = server_runtime() else {
        eprintln!("error: cannot start the async runtime");
        return ExitCode::from(1);
    };
    rt.block_on(async {
        let mcp = Mcp::new(None, format!("mcp-{}", std::process::id()));
        let held = mcp.clone();
        let running = match mcp.serve(rmcp::transport::stdio()).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: cannot start the MCP server: {e}");
                return ExitCode::from(1);
            }
        };
        let _ = running.waiting().await;
        held.run(None, Verb::Release, None).await;
        ExitCode::SUCCESS
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_name_tools_and_arguments() {
        let cases = [
            (
                "Run `phone-use snapshot` for fresh refs; closest now: @e9 button \"X\"",
                "Call `snapshot` for fresh refs; closest now: @e9 button \"X\"",
            ),
            (
                "Hide the keyboard with `phone-use press hide-keyboard`, or `phone-use scroll down` to bring the element above it.",
                "Hide the keyboard with `press` with key `hide-keyboard`, or `scroll` down to bring the element above it.",
            ),
            (
                "Use `--mode paste` or `--mode keys`; `set` works only where the device can set field values.",
                "Use `mode` `paste` or `keys`; `set` works only where the device can set field values.",
            ),
            (
                "Pass `--device <id>` with one of: a, b.",
                "Pass `device` with one of: a, b.",
            ),
            (
                "Increase `--timeout`, or run `phone-use snapshot` to check the screen.",
                "Increase `timeout_ms`, or run `snapshot` to check the screen.",
            ),
            (
                "nothing changed; `phone-use wait settled` waits for the screen, `phone-use snapshot` shows it",
                "nothing changed; `wait` with condition `settled` waits for the screen, `snapshot` shows it",
            ),
        ];
        for (from, to) in cases {
            assert_eq!(for_mcp(from), to);
        }
        assert!(
            !for_mcp("Run `phone-use doctor` to check the connection.").contains("Run `phone-use")
        );
    }

    #[test]
    fn only_hint_lines_of_a_success_are_rewritten() {
        let text = "ok tapped\n@e1 button \"phone-use snapshot\"\nhint: retry with `--mode keys`"
            .to_string();
        assert_eq!(
            success_text(text),
            "ok tapped\n@e1 button \"phone-use snapshot\"\nhint: retry with `mode` `keys`"
        );
    }
}
