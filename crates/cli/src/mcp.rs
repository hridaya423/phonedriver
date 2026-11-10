//! `phonedriver mcp`: the CLI verbs as MCP tools over stdio (architecture 10.2). A tool maps its arguments onto the
//! verb of the same name and returns the text the verb prints; nothing else happens here.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use phonedriver_core::error::Category;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ElicitRequest,
    ElicitRequestParams, ElicitationSchema, EnumSchemaBuilder, InputRequest, InputRequiredResult,
    ProtocolVersion,
};
use rmcp::schemars::{self, JsonSchema};
use rmcp::service::{ElicitationMode, RequestContext};
use rmcp::{Peer, RoleServer};
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use crate::client::{Client, Failure};
use crate::commands::{Globals, Verb, ask_env, execute_with, run_batch};
use crate::wire::{Approval, ScreenshotResult};

#[derive(Clone)]
pub struct Mcp {
    socket: Option<PathBuf>,
    session: String,
    device: Option<String>,
    client: Arc<Mutex<Option<Client>>>,
    ask: Vec<Category>,
    held: Arc<Mutex<Option<Held>>>,
    told: Told,
}

/// The question a 2026-07-28 call returned: the call it belongs to, the batch step it holds and what it asked.
/// A retry's answer counts only for the call that was asked.
struct Held {
    key: String,
    step: usize,
    asked: String,
}

/// The device line (`device <id> · <model> · <os> · <size> · <orientation>`) of the last reply this server sent. The
/// model keeps every reply in its context, so the line is sent again only when it changes.
#[derive(Clone, Default)]
struct Told(Arc<std::sync::Mutex<Option<String>>>);

impl Told {
    fn trim(&self, text: String) -> String {
        let Some(at) = crate::wire::device_line_at(&text) else {
            return text;
        };
        let mut lines: Vec<&str> = text.lines().collect();
        let line = lines[at].to_string();
        let mut last = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if last.as_deref() == Some(line.as_str()) {
            lines.remove(at);
            return lines.join("\n");
        }
        *last = Some(line);
        text
    }
}

#[derive(Deserialize, JsonSchema)]
struct BatchArgs {
    device: Option<String>,
    steps: Vec<serde_json::Value>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize, JsonSchema)]
struct AppsArgs {
    device: Option<String>,
    action: Option<String>,
    app: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct DevicesArgs {
    device: Option<String>,
    release: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct ScreenshotArgs {
    device: Option<String>,
    marks: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct ClipboardArgs {
    device: Option<String>,
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
    /// An @ref: print only that element, its text whole, and its children.
    within: Option<String>,
    /// Cut the output to this many lines.
    max_lines: Option<u32>,
    /// Read the pixels with OCR and add text the tree lacks (automatic when the tree is empty).
    pixels: Option<bool>,
    find: Option<String>,
}

/// `autonomous` is not in the lean tool schema, so a model that reads the refusal text guesses its type: one sent
/// `"true"` and the call failed with `invalid type: string`.
fn flag<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
    use serde::de::Error;
    match Option::<serde_json::Value>::deserialize(d)? {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Bool(b)) => Ok(Some(b)),
        Some(serde_json::Value::String(t)) if t.eq_ignore_ascii_case("true") => Ok(Some(true)),
        Some(serde_json::Value::String(t)) if t.eq_ignore_ascii_case("false") => Ok(Some(false)),
        Some(other) => Err(D::Error::custom(format!(
            "invalid value {other}, expected true or false"
        ))),
    }
}

#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TapArgs {
    device: Option<String>,
    /// @ref, `x,y` or text.
    #[serde(alias = "ref")]
    target: String,
    /// With a text target: only an element of this role, such as button, cell, textbox or switch.
    role: Option<String>,
    /// Tap this many times; 2 is a double tap.
    count: Option<u32>,
    long: Option<u32>,
    #[serde(default, deserialize_with = "flag")]
    autonomous: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TypeArgs {
    device: Option<String>,
    text: String,
    /// @ref, `x,y` or text; default the focused field.
    into: Option<String>,
    /// `paste` (default; on a hidden display, `set` for an app's own field), `set` (Android: set the field's value) or `keys` (one key at a time, ASCII only on Android).
    mode: Option<String>,
    /// Then press enter.
    submit: Option<bool>,
    /// With `into`, add to the field's text instead of replacing it.
    append: Option<bool>,
    secret: Option<bool>,
    #[serde(default, deserialize_with = "flag")]
    autonomous: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SwipeArgs {
    device: Option<String>,
    /// up, down, left or right.
    direction: Option<String>,
    /// Start; default the screen's center.
    from: Option<String>,
    to: Option<String>,
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
    /// up, down, left or right.
    direction: String,
    /// The list to scroll: an @ref. Default is the largest scrollable element.
    r#in: Option<String>,
    /// A text, `A|B` for either, or an @ref.
    until: Option<String>,
    /// Give up after this many scrolls. Default 30.
    max: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PressArgs {
    device: Option<String>,
    key: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LaunchArgs {
    device: Option<String>,
    app: Option<String>,
    url: Option<String>,
    grant: Option<Vec<String>>,
    restart: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct WaitArgs {
    device: Option<String>,
    condition: String,
    /// Default 5000.
    timeout_ms: Option<u64>,
}

impl TapArgs {
    fn verb(self) -> Verb {
        Verb::Tap {
            target: self.target,
            role: self.role,
            count: self.count,
            long: self.long,
            autonomous: false,
        }
    }
}

impl TypeArgs {
    fn type_verb(&self) -> Verb {
        let secret = self.secret.unwrap_or(false);
        Verb::Type {
            text: if secret {
                None
            } else {
                Some(self.text.clone())
            },
            into: self.into.clone(),
            mode: self.mode.clone(),
            submit: self.submit.unwrap_or(false),
            append: self.append.unwrap_or(false),
            secret,
            autonomous: false,
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
            (Some(app), None) => Ok(Verb::Launch {
                app,
                grant: self.grant.unwrap_or_default(),
                restart: self.restart.unwrap_or(false),
            }),
            (None, Some(url)) => Ok(Verb::Open { url, app: None }),
            (Some(app), Some(url)) => Ok(Verb::Open {
                url,
                app: Some(app),
            }),
            _ => Err("pass app or url"),
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
                autonomous: false,
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
            device: None,
            client: Arc::default(),
            ask: Vec::new(),
            held: Arc::default(),
            told: Told::default(),
        }
    }

    /// Also asks the human before the taps of these opt-in categories (decision D31).
    pub fn asking(mut self, ask: Vec<Category>) -> Mcp {
        self.ask = ask;
        self
    }

    /// The same server, with every tool call that names no device sent to `device`.
    pub fn pinned(self, device: Option<String>) -> Mcp {
        Mcp { device, ..self }
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

    fn globals(&self, device: Option<String>) -> Globals {
        Globals {
            device: device
                .or_else(|| self.device.clone())
                .or_else(|| std::env::var("PHONEDRIVER_DEVICE").ok()),
            json: false,
            no_settle: false,
            timeout: None,
            expect_change: None,
            share: false,
            display: crate::commands::display_mode(
                &std::env::var("PHONEDRIVER_DISPLAY").unwrap_or_default(),
            ),
            approval: None,
            ask: self.ask.clone(),
        }
    }

    async fn run(
        &self,
        device: Option<String>,
        verb: Verb,
        secret: Option<String>,
    ) -> CallToolResult {
        self.run_with(self.globals(device), verb, secret).await
    }

    /// A tap that spends money or cannot be undone needs the human's word (decision D27), asked through the client's
    /// elicitation and never through the agent. With a connected client (`Channel::Peer`) the call waits here for
    /// the answer; with protocol 2026-07-28 (`Channel::Mrtr`) the call returns the question and the client calls
    /// again with the answer. A client that cannot ask gets the question as the tool result, and nothing is tapped.
    async fn guarded(
        &self,
        channel: Channel<'_>,
        key: String,
        device: Option<String>,
        verb: impl Fn() -> Verb,
        secret: Option<String>,
        autonomous: bool,
    ) -> Outcome {
        let mut globals = self.globals(device);
        if autonomous {
            globals.approval = Some(Approval::Autonomous);
        }
        if let Channel::Mrtr {
            answer: Some(reply),
            ..
        } = &channel
            && !autonomous
        {
            let Some(held) = self.take_held(&key).await else {
                return Outcome::Done(dismissed(None));
            };
            match decision(reply) {
                Decision::Declined => return Outcome::Done(declined_for(&held.asked)),
                Decision::Dismissed => return Outcome::Done(dismissed(Some(&held.asked))),
                Decision::Once => globals.approval = Some(Approval::Once),
                Decision::Session => globals.approval = Some(Approval::Session),
            }
        }
        let first = self
            .with_client(async |client| {
                execute_with(client, verb(), secret.clone(), &globals).await
            })
            .await;
        let Some((label, app, category, details)) = first.as_ref().err().and_then(approval_needed)
        else {
            return Outcome::Done(self.reply(first));
        };
        let asked = commitment(&label, app.as_deref(), category, &details);
        let prompt = format!("Your agent wants to do this on your phone:\n{asked}\nAllow it?");
        let reply = match channel {
            Channel::Mrtr {
                can_ask: true,
                answer: None,
            } => match question(&prompt) {
                Ok(q) => {
                    *self.held.lock().await = Some(Held {
                        key,
                        step: 0,
                        asked,
                    });
                    return Outcome::Ask(q);
                }
                Err(why) => return Outcome::Done(cannot_ask(&asked, &why)),
            },
            Channel::Peer(peer)
                if peer
                    .supported_elicitation_modes()
                    .contains(&ElicitationMode::Form) =>
            {
                match Self::ask(peer, &prompt).await {
                    Ok(r) => r,
                    Err(why) => return Outcome::Done(cannot_ask(&asked, &why)),
                }
            }
            _ => return Outcome::Done(cannot_ask(&asked, "this client cannot ask the human")),
        };
        match decision(&reply) {
            Decision::Declined => Outcome::Done(declined_for(&asked)),
            Decision::Dismissed => Outcome::Done(dismissed(Some(&asked))),
            chosen => {
                globals.approval = Some(match chosen {
                    Decision::Session => Approval::Session,
                    _ => Approval::Once,
                });
                let second = self
                    .with_client(async |client| {
                        execute_with(client, verb(), secret, &globals).await
                    })
                    .await;
                Outcome::Done(self.reply(second))
            }
        }
    }

    /// A batch whose step needs the human's word is asked like a tap (decision D27). The steps before it have run;
    /// after an approval the batch goes on from that step, and the approval covers that step only.
    async fn guarded_batch(
        &self,
        channel: Channel<'_>,
        device: Option<String>,
        steps: Vec<serde_json::Value>,
        timeout: Option<u64>,
    ) -> Outcome {
        let key = json!(["batch", &device, &steps, timeout]).to_string();
        let mut globals = self.globals(device);
        globals.timeout = timeout;
        let mut from = 0;
        if let Channel::Mrtr {
            answer: Some(reply),
            ..
        } = &channel
        {
            let Some(held) = self.take_held(&key).await else {
                return Outcome::Done(dismissed(None));
            };
            match decision(reply) {
                Decision::Declined => return Outcome::Done(declined_for(&held.asked)),
                Decision::Dismissed => return Outcome::Done(dismissed(Some(&held.asked))),
                Decision::Once => globals.approval = Some(Approval::Once),
                Decision::Session => globals.approval = Some(Approval::Session),
            }
            from = held.step;
        }
        loop {
            let rest = steps[from..].to_vec();
            let result = self
                .with_client(async |client| run_batch(client, rest, &globals).await)
                .await;
            globals.approval = None;
            let Some((label, app, category, details)) =
                result.as_ref().err().and_then(approval_needed)
            else {
                return Outcome::Done(self.reply(result));
            };
            let held = result
                .as_ref()
                .err()
                .and_then(held_step)
                .map_or(from, |n| from + n - 1);
            let asked = commitment(&label, app.as_deref(), category, &details);
            let prompt = format!("Your agent wants to do this on your phone:\n{asked}\nAllow it?");
            let reply = match &channel {
                Channel::Mrtr {
                    can_ask: true,
                    answer: _,
                } => match question(&prompt) {
                    Ok(q) => {
                        *self.held.lock().await = Some(Held {
                            key,
                            step: held,
                            asked,
                        });
                        return Outcome::Ask(q);
                    }
                    Err(why) => return Outcome::Done(cannot_ask(&asked, &why)),
                },
                Channel::Peer(peer)
                    if peer
                        .supported_elicitation_modes()
                        .contains(&ElicitationMode::Form) =>
                {
                    match Self::ask(peer, &prompt).await {
                        Ok(r) => r,
                        Err(why) => return Outcome::Done(cannot_ask(&asked, &why)),
                    }
                }
                _ => {
                    return Outcome::Done(cannot_ask(&asked, "this client cannot ask the human"));
                }
            };
            match decision(&reply) {
                Decision::Declined => return Outcome::Done(declined_for(&asked)),
                Decision::Dismissed => return Outcome::Done(dismissed(Some(&asked))),
                Decision::Once => globals.approval = Some(Approval::Once),
                Decision::Session => globals.approval = Some(Approval::Session),
            }
            from = held;
        }
    }

    /// The open question, if `key` is the call it was asked for; another call's answer leaves it open.
    async fn take_held(&self, key: &str) -> Option<Held> {
        let mut slot = self.held.lock().await;
        if slot.as_ref().is_some_and(|h| h.key == key) {
            slot.take()
        } else {
            None
        }
    }

    async fn ask(peer: &Peer<RoleServer>, prompt: &str) -> Result<serde_json::Value, String> {
        let reply = peer
            .create_elicitation(ElicitRequestParams::FormElicitationParams {
                meta: None,
                message: prompt.to_string(),
                requested_schema: schema()?,
            })
            .await
            .map_err(|e| format!("the client could not ask: {e}"))?;
        serde_json::to_value(reply).map_err(|e| e.to_string())
    }

    /// The tap and type tools for a client on protocol 2026-07-28, which may not receive requests from a server.
    async fn call_guarded(
        &self,
        request: CallToolRequestParams,
        context: &RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        let args = serde_json::Value::Object(request.arguments.clone().unwrap_or_default());
        let can_ask = context
            .client_capabilities()
            .and_then(|c| c.elicitation)
            .is_some_and(|e| e.form.is_some() || e.url.is_none());
        let answer = request
            .input_responses
            .as_ref()
            .and_then(|r| r.get(APPROVAL_KEY));
        let channel = Channel::Mrtr { can_ask, answer };
        let key = json!([&request.name, &args]).to_string();
        let invalid = |e: serde_json::Error| rmcp::ErrorData::invalid_params(e.to_string(), None);
        let outcome = if request.name == "batch" {
            let a: BatchArgs = serde_json::from_value(args).map_err(invalid)?;
            match batch_steps(a.steps) {
                Ok(steps) => {
                    self.guarded_batch(channel, a.device, steps, a.timeout_ms)
                        .await
                }
                Err(why) => Outcome::Done(bad(&why)),
            }
        } else if request.name == "tap" {
            let a: TapArgs = serde_json::from_value(args).map_err(invalid)?;
            let (device, autonomous) = (a.device.clone(), a.autonomous.unwrap_or(false));
            self.guarded(channel, key, device, || a.clone().verb(), None, autonomous)
                .await
        } else {
            let a: TypeArgs = serde_json::from_value(args).map_err(invalid)?;
            let secret = a.secret.unwrap_or(false);
            let (device, autonomous) = (a.device.clone(), a.autonomous.unwrap_or(false));
            let text = secret.then(|| a.text.clone());
            self.guarded(channel, key, device, || a.type_verb(), text, autonomous)
                .await
        };
        Ok(match outcome {
            Outcome::Done(r) => r.into(),
            Outcome::Ask(q) => q.into(),
        })
    }

    fn reply(&self, result: Result<String, Failure>) -> CallToolResult {
        match result {
            Ok(text) => text_result(self.told.trim(success_text(text))),
            Err(f) => failure(&f),
        }
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
            Ok(text) => text_result(self.told.trim(success_text(text))),
            Err(f) => failure(&f),
        }
    }
}

fn batch_steps(raw: Vec<serde_json::Value>) -> Result<Vec<serde_json::Value>, String> {
    let mut steps = Vec::new();
    for (i, step) in raw.into_iter().enumerate() {
        let json = step_verb(step)
            .and_then(|v| crate::commands::step_json(&v, None).map_err(|f| f.message()));
        match json {
            Ok(j) => steps.push(j),
            Err(why) => return Err(format!("step {}: {why}", i + 1)),
        }
    }
    if steps.is_empty() {
        return Err("a batch needs at least one step".into());
    }
    Ok(steps)
}

const NEEDS_APPROVAL: i32 = -32019;

/// The guard's refusal, when `f` is one: the button, the app, what kind of tap it is and the screen text around it.
fn approval_needed(f: &Failure) -> Option<(String, Option<String>, Category, Vec<String>)> {
    let Failure::Rpc(e) = f else { return None };
    if e.code != NEEDS_APPROVAL {
        return None;
    }
    let text = |v: &serde_json::Value| v.as_str().map(str::to_string);
    let details = e.data["details"]
        .as_array()
        .map(|a| a.iter().filter_map(text).collect())
        .unwrap_or_default();
    let category = serde_json::from_value(e.data["category"].clone()).ok()?;
    Some((
        text(&e.data["label"])?,
        text(&e.data["app"]),
        category,
        details,
    ))
}

/// The 1-based step of a batch that the guard held, from its refusal.
fn held_step(f: &Failure) -> Option<usize> {
    let Failure::Rpc(e) = f else { return None };
    e.data["step"].as_u64().map(|n| n as usize)
}

fn commitment(label: &str, app: Option<&str>, category: Category, details: &[String]) -> String {
    let kind = format!("Category: {} ({}).", category.name(), category.what());
    let mut text = match app {
        Some(app) => format!("Tap \"{label}\" in {app}. {kind}"),
        None => format!("Tap \"{label}\". {kind}"),
    };
    for d in details {
        text.push_str("\n  ");
        text.push_str(d);
    }
    text
}

const APPROVAL_KEY: &str = "approve";
const ALLOW: &str = "allow";
const ALLOW_SESSION: &str = "session";

enum Decision {
    Once,
    Session,
    Declined,
    Dismissed,
}

enum Channel<'a> {
    Peer(&'a Peer<RoleServer>),
    Mrtr {
        can_ask: bool,
        answer: Option<&'a serde_json::Value>,
    },
}

enum Outcome {
    Done(CallToolResult),
    Ask(InputRequiredResult),
}

impl Outcome {
    /// For a client that is asked over its open connection, where the question never comes back as a result.
    fn complete(self) -> CallToolResult {
        match self {
            Outcome::Done(r) => r,
            Outcome::Ask(_) => bad("the client must be asked over its connection"),
        }
    }
}

fn schema() -> Result<ElicitationSchema, String> {
    let choice = EnumSchemaBuilder::new(vec![ALLOW.into(), ALLOW_SESSION.into()])
        .enum_titles(vec!["Allow".into(), "Allow for this session".into()])
        .and_then(|b| b.with_default(ALLOW))
        .map(|b| b.title("Your answer").build())
        .map_err(|e| e.to_string())?;
    ElicitationSchema::builder()
        .required_enum_schema("choice", choice)
        .build()
        .map_err(|e| e.to_string())
}

fn question(prompt: &str) -> Result<InputRequiredResult, String> {
    let ask = ElicitRequest::new(ElicitRequestParams::FormElicitationParams {
        meta: None,
        message: prompt.to_string(),
        requested_schema: schema()?,
    });
    Ok(InputRequiredResult::from_input_requests(
        [(APPROVAL_KEY.to_string(), InputRequest::Elicitation(ask))].into(),
    ))
}

/// What the human answered: an accept with a choice, a cancel (the prompt was dismissed, which is what a headless client sends), or a decline.
fn decision(reply: &serde_json::Value) -> Decision {
    match reply["action"].as_str() {
        Some("accept") => {}
        Some("cancel") => return Decision::Dismissed,
        _ => return Decision::Declined,
    }
    match reply["content"]["choice"].as_str() {
        Some(ALLOW_SESSION) => Decision::Session,
        _ => Decision::Once,
    }
}

fn declined_for(asked: &str) -> CallToolResult {
    text_result(format!(
        "The human declined; nothing was tapped.\n{asked}\nStop, or ask the user what to do instead."
    ))
}

fn dismissed(asked: Option<&str>) -> CallToolResult {
    text_result(format!(
        "Needs the human's approval; nothing was tapped (no answer: the approval prompt was dismissed, as when nobody can answer it).\n{}Ask the user in chat. If the user's own instructions already approve this action, call this tool again with `autonomous: true`.",
        asked.map(|a| format!("{a}\n")).unwrap_or_default()
    ))
}

fn cannot_ask(asked: &str, why: &str) -> CallToolResult {
    text_result(format!(
        "Needs the human's approval; nothing was tapped ({why}).\n{asked}\nAsk the user in chat. If they say yes, call this tool again with `autonomous: true`."
    ))
}

fn text_result(text: String) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(text)])
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

/// Hints are written for the command line. The MCP agent has tools and arguments, not `phonedriver` commands and flags.
fn for_mcp(hint: &str) -> String {
    const PHRASES: &[(&str, &str)] = &[
        (
            "`phonedriver press hide-keyboard`",
            "`press` with key `hide-keyboard`",
        ),
        ("`phonedriver press enter`", "`press` with key `enter`"),
        ("`phonedriver snapshot --no-settle`", "`snapshot`"),
        (
            "`phonedriver wait settled`",
            "`wait` with condition `settled`",
        ),
        (
            "`--mode paste` or `--mode keys`",
            "`mode` `paste` or `keys`",
        ),
        ("`--mode keys`", "`mode` `keys`"),
        ("`--within <list ref>`", "`in` with the list's ref"),
        (
            "`--secret` (the text on stdin or in PHONEDRIVER_SECRET)",
            "`secret: true`",
        ),
        ("`--device <id>`", "`device`"),
        ("`--timeout`", "`timeout_ms`"),
        ("`--into`", "`into`"),
        ("`--max`", "`max`"),
        (
            "`phonedriver doctor` says what is missing",
            "ask the user to run phonedriver doctor to see what is missing",
        ),
        (
            "Run `phonedriver doctor` to check the connection.",
            "Ask the user to run `phonedriver doctor` to check the connection.",
        ),
        (
            "Run `phonedriver doctor` to see what this device supports.",
            "Try another approach; this device does not support that.",
        ),
        (
            "ask the user to run `phonedriver release`",
            "ask the user to run phonedriver release in a terminal",
        ),
        ("Run `phonedriver ", "Call `"),
    ];
    let mut out = hint.to_string();
    for (from, to) in PHRASES {
        out = out.replace(from, to);
    }
    while let Some(start) = out.find("`phonedriver ") {
        let Some(len) = out[start + 1..].find('`') else {
            break;
        };
        let inner = out[start + 1 + "phonedriver ".len()..start + 1 + len].to_string();
        let mut words = inner.split_whitespace();
        let tool = words.next().unwrap_or_default().to_string();
        let rest: Vec<&str> = words.collect();
        let shown = if tool == "scroll" && !rest.is_empty() {
            format!("`scroll` {}", rest.join(" "))
        } else if let ["--within", r] = rest[..] {
            format!("`{tool}` with `within` {r}")
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
        description = "List devices (id, model, OS, holder). With several, pass an id as `device` to the other tools. `release` gives up this session's hold."
    )]
    async fn devices(&self, Parameters(a): Parameters<DevicesArgs>) -> CallToolResult {
        if a.release.unwrap_or(false) {
            return self.run(a.device, Verb::Release, None).await;
        }
        self.run(None, Verb::Devices, None).await
    }

    #[tool(
        description = "Run a device shell command: exit code, output, screen change. Prefer it for settings: `cmd uimode night yes`, `settings put system screen_brightness 20`, `am start -a android.intent.action.VIEW -d <url>`. Simulator: `simctl ui appearance dark`. A physical iPhone has no shell."
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
        let mut globals = self.globals(a.device);
        globals.no_settle = a.settle == Some(false);
        self.run_with(globals, verb, secret.then_some(a.command))
            .await
    }

    #[tool(description = "Read the clipboard, or `set` it.")]
    async fn clipboard(&self, Parameters(a): Parameters<ClipboardArgs>) -> CallToolResult {
        let verb = Verb::Clipboard {
            action: a.set.as_ref().map(|_| "set".to_string()),
            text: a.set,
        };
        self.run(a.device, verb, None).await
    }

    #[tool(
        description = "List apps (`name  id`), or `action` install (file in `app`), stop, clear (data) or uninstall."
    )]
    async fn apps(&self, Parameters(a): Parameters<AppsArgs>) -> CallToolResult {
        let verb = Verb::Apps {
            action: a.action,
            target: a.app,
        };
        self.run(a.device, verb, None).await
    }

    #[tool(
        description = "The screen as lines like `@e4 button \"Save\"`; refs name elements for other tools. `find` lists matching lines (`A|B` for either). `under-keyboard` = hidden by the keyboard."
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
        description = "Image of the screen, for what the snapshot lacks (canvas, map, photo). `marks` boxes tappable elements with their @ref."
    )]
    async fn screenshot(&self, Parameters(a): Parameters<ScreenshotArgs>) -> CallToolResult {
        let marks = a.marks.unwrap_or(false);
        let globals = self.globals(a.device);
        let shot = self
            .with_client(async |client| {
                let mut params = json!({"inline": true, "marks": marks});
                if let Some(d) = &globals.device {
                    params["device"] = json!(d);
                }
                crate::commands::put_display(&mut params, globals.display);
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
        description = "Tap by @ref, text or x,y point (from a screenshot). `long` holds the press that many ms (context menus). The reply is the new screen: read it, and call tools one at a time (they share one screen)."
    )]
    async fn tap(
        &self,
        peer: Peer<RoleServer>,
        Parameters(a): Parameters<TapArgs>,
    ) -> CallToolResult {
        let device = a.device.clone();
        let autonomous = a.autonomous.unwrap_or(false);
        let outcome = self
            .guarded(
                Channel::Peer(&peer),
                String::new(),
                device,
                || a.clone().verb(),
                None,
                autonomous,
            )
            .await;
        outcome.complete()
    }

    #[tool(
        name = "type",
        description = "Put text in a field (replaces its content) and report what it shows; empty text clears. `secret` for any password or code."
    )]
    async fn r#type(
        &self,
        peer: Peer<RoleServer>,
        Parameters(a): Parameters<TypeArgs>,
    ) -> CallToolResult {
        let secret = a.secret.unwrap_or(false);
        if a.text.is_empty() && !secret {
            return match a.into {
                Some(target) => self.run(a.device, Verb::Clear { target }, None).await,
                None => bad("to clear a field pass `into`: the field to empty"),
            };
        }
        let autonomous = a.autonomous.unwrap_or(false);
        let device = a.device.clone();
        let text = secret.then(|| a.text.clone());
        let outcome = self
            .guarded(
                Channel::Peer(&peer),
                String::new(),
                device,
                || a.type_verb(),
                text,
                autonomous,
            )
            .await;
        outcome.complete()
    }

    #[tool(
        description = "A finger gesture: flip pages, dismiss. `to` drags `from` to a target; `pinch` (in, out) zooms. Use `scroll` for lists."
    )]
    async fn swipe(&self, Parameters(a): Parameters<SwipeArgs>) -> CallToolResult {
        let device = a.device.clone();
        match a.verb() {
            Ok(verb) => self.run(device, verb, None).await,
            Err(why) => bad(why),
        }
    }

    #[tool(description = "Scroll toward more content, or until the text or @ref in `until` shows.")]
    async fn scroll(&self, Parameters(a): Parameters<ScrollArgs>) -> CallToolResult {
        let device = a.device.clone();
        self.run(device, a.verb(), None).await
    }

    #[tool(
        description = "Press home, back, recents, notifications, lock, volume-up, volume-down, enter or hide-keyboard."
    )]
    async fn press(&self, Parameters(a): Parameters<PressArgs>) -> CallToolResult {
        self.run(a.device, Verb::Press { key: a.key }, None).await
    }

    #[tool(
        description = "Open an app (any part of its name, or its id) or a `url` (with `app`, opened in that app); returns the screen it shows, which is where the app was left unless `restart`. `grant` pre-allows permissions (`camera`, `notifications`)."
    )]
    async fn launch(&self, Parameters(a): Parameters<LaunchArgs>) -> CallToolResult {
        let device = a.device.clone();
        match a.verb() {
            Ok(verb) => self.run(device, verb, None).await,
            Err(why) => bad(why),
        }
    }

    #[tool(
        description = "Run steps in order, one diff back; stops at the first failure. A step is {\"tool\":\"tap\",\"target\":\"@e3\"}: tap, type, swipe, scroll, press, launch or wait."
    )]
    async fn batch(
        &self,
        peer: Peer<RoleServer>,
        Parameters(a): Parameters<BatchArgs>,
    ) -> CallToolResult {
        let steps = match batch_steps(a.steps) {
            Ok(s) => s,
            Err(why) => return bad(&why),
        };
        self.guarded_batch(Channel::Peer(&peer), a.device, steps, a.timeout_ms)
            .await
            .complete()
    }

    #[tool(
        description = "Wait for `settled`, a text (`A|B` for either) or @ref to appear, or `gone:<text>`."
    )]
    async fn wait(&self, Parameters(a): Parameters<WaitArgs>) -> CallToolResult {
        let mut globals = self.globals(a.device);
        globals.timeout = a.timeout_ms;
        let verb = Verb::Wait {
            condition: a.condition,
        };
        self.run_with(globals, verb, None).await
    }
}

#[tool_handler(
    name = "phonedriver",
    instructions = "Operate an Android or iOS device. System settings: `shell`. App UI: `launch` takes an app name (no `devices` or `apps` first), then `snapshot` once and act on its @refs; each action returns what changed. Send calls one at a time on the shared screen. If an error needs the user to act on the device, say so and go on when they are done. Passwords, payments, sending and deleting need the user's word for this task."
)]
impl ServerHandler for Mcp {
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<rmcp::RoleServer>,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        let mrtr = context
            .protocol_version()
            .is_some_and(|v| v >= ProtocolVersion::V_2026_07_28);
        if mrtr && matches!(request.name.as_ref(), "tap" | "type" | "batch") {
            return self.call_guarded(request, &context).await;
        }
        let call = rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
        Self::tool_router().call(call).await
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, rmcp::ErrorData> {
        let mut tools = Self::tool_router().list_all();
        for t in &mut tools {
            let mut schema = (*t.input_schema).clone();
            slim_schema(&mut schema);
            crate::lean::lean(&t.name, &mut schema);
            t.input_schema = Arc::new(schema);
        }
        Ok(rmcp::model::ListToolsResult {
            result_type: Some(rmcp::model::ResultType::COMPLETE),
            tools,
            meta: None,
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
        })
    }
}

/// An argument schema without what a model gets nothing from: the `$schema` URL, integer `format` and `minimum`
/// boilerplate, and `null` in the type of an optional argument (an argument left out is already none).
fn slim_schema(schema: &mut serde_json::Map<String, serde_json::Value>) {
    schema.remove("$schema");
    schema.remove("additionalProperties");
    slim_value(schema);
}

fn slim_value(m: &mut serde_json::Map<String, serde_json::Value>) {
    m.remove("format");
    if m.get("items") == Some(&json!(true)) {
        m.remove("items");
    }
    if m.get("minimum") == Some(&json!(0)) {
        m.remove("minimum");
    }
    if let Some(serde_json::Value::Array(types)) = m.get("type")
        && let [only, serde_json::Value::String(null)] = types.as_slice()
        && null == "null"
    {
        let only = only.clone();
        m.insert("type".into(), only);
    }
    if let Some(serde_json::Value::Object(props)) = m.get_mut("properties") {
        for v in props.values_mut() {
            if let serde_json::Value::Object(p) = v {
                slim_value(p);
            }
        }
    }
}

/// The runtime the server runs on. rmcp starts one task per request in the order requests arrive; on one thread the
/// tasks take the call lock in that order, so calls an agent sends together act in the order it sent them.
pub fn server_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
}

pub fn run(device: Option<String>) -> ExitCode {
    let Ok(rt) = server_runtime() else {
        eprintln!("error: cannot start the async runtime");
        return ExitCode::from(1);
    };
    rt.block_on(async {
        let ask = match ask_env() {
            Ok(a) => a,
            Err(message) => {
                eprintln!("error: {message}");
                return ExitCode::from(2);
            }
        };
        let mcp = Mcp::new(None, format!("mcp-{}", std::process::id()))
            .asking(ask)
            .pinned(device);
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
    fn a_pinned_server_sends_calls_that_name_no_device_to_its_device() {
        let mcp = Mcp::new(None, "s".into()).pinned(Some("android:emulator-5554".into()));
        assert_eq!(
            mcp.globals(None).device.as_deref(),
            Some("android:emulator-5554")
        );
        assert_eq!(
            mcp.globals(Some("android:emulator-5556".into()))
                .device
                .as_deref(),
            Some("android:emulator-5556")
        );
    }

    #[test]
    fn tap_takes_ref_as_another_name_for_target_and_the_schema_names_only_target() {
        let a: TapArgs = serde_json::from_value(json!({"ref": "@e27"})).unwrap();
        assert_eq!(a.target, "@e27");
        let schema = serde_json::to_string(&schemars::schema_for!(TapArgs)).unwrap();
        assert!(!schema.contains("\"ref\""), "{schema}");
    }

    #[test]
    fn autonomous_takes_true_as_a_boolean_or_a_string_and_nothing_else() {
        let tap = |v: serde_json::Value| serde_json::from_value::<TapArgs>(v);
        let on = |v: serde_json::Value| tap(json!({"target": "x", "autonomous": v}));
        assert_eq!(on(json!(true)).unwrap().autonomous, Some(true));
        assert_eq!(on(json!("true")).unwrap().autonomous, Some(true));
        assert_eq!(on(json!("False")).unwrap().autonomous, Some(false));
        assert!(on(json!("yes")).is_err());
        assert!(on(json!(1)).is_err());
        assert_eq!(tap(json!({"target": "x"})).unwrap().autonomous, None);
    }

    #[test]
    fn hints_name_tools_and_arguments() {
        let cases = [
            (
                "Run `phonedriver snapshot` for fresh refs; closest now: @e9 button \"X\"",
                "Call `snapshot` for fresh refs; closest now: @e9 button \"X\"",
            ),
            (
                "Hide the keyboard with `phonedriver press hide-keyboard`, or `phonedriver scroll down` to bring the element above it.",
                "Hide the keyboard with `press` with key `hide-keyboard`, or `scroll` down to bring the element above it.",
            ),
            (
                "Use `--mode paste` or `--mode keys`; `set` works only where the device can set field values.",
                "Use `mode` `paste` or `keys`; `set` works only where the device can set field values.",
            ),
            (
                "then type it on its own with `--secret` (the text on stdin or in PHONEDRIVER_SECRET); a batch cannot carry secret text.",
                "then type it on its own with `secret: true`; a batch cannot carry secret text.",
            ),
            (
                "Pass `--device <id>` with one of: a, b.",
                "Pass `device` with one of: a, b.",
            ),
            (
                "Increase `--timeout`, or run `phonedriver snapshot` to check the screen.",
                "Increase `timeout_ms`, or run `snapshot` to check the screen.",
            ),
            (
                "Connect a device or start an emulator; `phonedriver doctor` says what is missing (adb, USB debugging, a dropped tunnel).",
                "Connect a device or start an emulator; ask the user to run phonedriver doctor to see what is missing (adb, USB debugging, a dropped tunnel).",
            ),
            (
                "nothing changed; the tap may have missed, or changed only what the element list does not show (a web checkbox): `phonedriver screenshot` shows the picture",
                "nothing changed; the tap may have missed, or changed only what the element list does not show (a web checkbox): `screenshot` shows the picture",
            ),
        ];
        for (from, to) in cases {
            assert_eq!(for_mcp(from), to);
        }
        assert_eq!(
            for_mcp("`phonedriver snapshot --within @e45` prints @e45 whole"),
            "`snapshot` with `within` @e45 prints @e45 whole"
        );
        let busy = phonedriver_core::error::Error::DeviceBusy {
            id: "sim:A".into(),
            held_by: "default".into(),
            idle_s: 10,
            expires_s: 590,
        }
        .hint();
        assert!(
            for_mcp(&busy).ends_with("or ask the user to run phonedriver release in a terminal."),
            "{}",
            for_mcp(&busy)
        );
        assert!(
            !for_mcp("Run `phonedriver doctor` to check the connection.")
                .contains("Run `phonedriver")
        );
    }

    #[test]
    fn a_device_line_is_dropped_when_it_repeats_the_last_one_told() {
        let told = Told::default();
        let line = "device android:a · Pixel · Android API 37 · 1280x800pt · landscape";
        let text = format!("ok tapped @e1\n{line}\napp x · keyboard hidden\n@e1 button \"A\"");
        assert_eq!(told.trim(text.clone()), text);
        assert_eq!(
            told.trim(text.clone()),
            "ok tapped @e1\napp x · keyboard hidden\n@e1 button \"A\""
        );
        let turned = text.replace("landscape", "portrait");
        assert_eq!(told.trim(turned.clone()), turned);
        assert_eq!(told.trim(turned.clone()).lines().count(), 3);
        let other = text.replace("android:a", "android:b");
        assert_eq!(told.trim(other.clone()), other);
        let table = "ID  MODEL\nandroid:a  Pixel".to_string();
        assert_eq!(told.trim(table.clone()), table);
        assert_eq!(told.trim(table.clone()), table);
    }

    #[test]
    fn only_hint_lines_of_a_success_are_rewritten() {
        let text = "ok tapped\n@e1 button \"phonedriver snapshot\"\nhint: retry with `--mode keys`"
            .to_string();
        assert_eq!(
            success_text(text),
            "ok tapped\n@e1 button \"phonedriver snapshot\"\nhint: retry with `mode` `keys`"
        );
    }
}
