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
use crate::commands::{Globals, Verb, execute_with};
use crate::wire::ScreenshotResult;

#[derive(Clone)]
pub struct Mcp {
    socket: Option<PathBuf>,
    session: String,
    client: Arc<Mutex<Option<Client>>>,
}

#[derive(Deserialize, JsonSchema)]
struct DeviceArgs {
    /// Device id or serial, such as `android:emulator-5554`. Omit it when one device is connected.
    device: Option<String>,
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
}

#[derive(Deserialize, JsonSchema)]
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
    /// The text is a password, code or other secret: it is never shown back, logged or traced.
    secret: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct SwipeArgs {
    device: Option<String>,
    /// up, down, left or right: where the finger moves.
    direction: String,
    /// Start at this target's center instead of the screen's.
    from: Option<String>,
    /// Share of the screen to cross, 0 to 1. Default 0.5.
    distance: Option<f32>,
    /// A quick flick that may fling.
    fast: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
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
struct PressArgs {
    device: Option<String>,
    /// home, back, recents, notifications, lock, volume-up, volume-down, enter or hide-keyboard.
    key: String,
}

#[derive(Deserialize, JsonSchema)]
struct LaunchArgs {
    device: Option<String>,
    /// An app name or package or bundle id, such as `Settings`. Pass this or `url`.
    app: Option<String>,
    /// A web link or deep link to open in the app that handles it. Pass this or `app`.
    url: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct WaitArgs {
    device: Option<String>,
    /// `settled`, a text or @ref to appear, or `gone:<text>` to leave the screen.
    condition: String,
    /// Give up after this many milliseconds. Default 5000.
    timeout_ms: Option<u64>,
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
            Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
            Err(f) => failure(&f),
        }
    }
}

fn failure(f: &Failure) -> CallToolResult {
    let text = match f.hint() {
        Some(h) => format!("{}\nhint: {h}", f.message()),
        None => f.message(),
    };
    CallToolResult::error(vec![ContentBlock::text(text)])
}

fn bad(message: &str) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message)])
}

#[tool_router]
impl Mcp {
    #[tool(
        description = "List connected devices with their id, model, OS, tier and which session holds each. Call it first when unsure which device to use."
    )]
    async fn devices(&self, Parameters(_): Parameters<DeviceArgs>) -> CallToolResult {
        self.run(None, Verb::Devices, None).await
    }

    #[tool(
        description = "Show what is on screen as lines like `@e4 button \"Save\"`, or only what changed with diff. Refs name elements for tap, type, swipe and scroll."
    )]
    async fn snapshot(&self, Parameters(a): Parameters<SnapshotArgs>) -> CallToolResult {
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
        description = "Take a screenshot and return it as an image, one pixel per point. Use it when the snapshot does not show what you need, such as a canvas, a map or an image."
    )]
    async fn screenshot(&self, Parameters(a): Parameters<DeviceArgs>) -> CallToolResult {
        let globals = Self::globals(a.device);
        let shot = self
            .with_client(async |client| {
                let mut params = json!({"inline": true});
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
            ContentBlock::text(format!(
                "{}x{} at scale {}",
                shot.width, shot.height, shot.scale
            )),
        ])
    }

    #[tool(
        description = "Tap an element by @ref, by its text, or at x,y points. Returns what changed once the screen settled. Prefer refs; use points only from a screenshot. One call at a time."
    )]
    async fn tap(&self, Parameters(a): Parameters<TapArgs>) -> CallToolResult {
        let verb = Verb::Tap {
            target: a.target,
            role: a.role,
            count: a.count,
            long: a.long,
        };
        self.run(a.device, verb, None).await
    }

    #[tool(
        name = "type",
        description = "Type text into a field and report whether the field now holds it. Set secret for passwords and codes; only type those when the user gave them for this task. One call at a time."
    )]
    async fn r#type(&self, Parameters(a): Parameters<TypeArgs>) -> CallToolResult {
        let secret = a.secret.unwrap_or(false);
        let verb = Verb::Type {
            text: if secret { None } else { Some(a.text.clone()) },
            into: a.into,
            mode: a.mode,
            submit: a.submit.unwrap_or(false),
            secret,
        };
        self.run(a.device, verb, secret.then_some(a.text)).await
    }

    #[tool(
        description = "Drag one finger across the screen the way a user swipes: to flip pages, pull down, dismiss. To read more of a list use scroll instead. One call at a time."
    )]
    async fn swipe(&self, Parameters(a): Parameters<SwipeArgs>) -> CallToolResult {
        let verb = Verb::Swipe {
            direction: a.direction,
            from: a.from,
            distance: a.distance,
            fast: a.fast.unwrap_or(false),
        };
        self.run(a.device, verb, None).await
    }

    #[tool(
        description = "Scroll a list so more content shows in the direction, or until the text or @ref in `until` is on screen. It never flings past a target. One call at a time."
    )]
    async fn scroll(&self, Parameters(a): Parameters<ScrollArgs>) -> CallToolResult {
        let verb = Verb::Scroll {
            direction: a.direction,
            within: a.r#in,
            until: a.until,
            max: a.max,
        };
        self.run(a.device, verb, None).await
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
        let verb = match (a.app, a.url) {
            (Some(app), None) => Verb::Launch { app },
            (None, Some(url)) => Verb::Open { url },
            _ => return bad("pass app or url, one of them"),
        };
        self.run(a.device, verb, None).await
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
