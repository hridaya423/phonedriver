//! Decisions D27 and D31 as properties of the real code: a tap that spends money, deletes an account or is in an opted-in
//! category reaches the device only after the human's answer, whatever the batch, the answers, the protocol version
//! and the retries. The oracle below is written from the promise, not from `mcp.rs`.

#[path = "support/fakes.rs"]
mod fakes;
#[path = "support/guard_rig.rs"]
mod guard_rig;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, LazyLock, Mutex};

use guard_rig::{PLACE_ORDER, rig};
use phonedriver::mcp::Mcp;
use phonedriver_core::error::Category;
use phonedriver_core::testing::Call;
use proptest::prelude::*;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ClientCapabilities, ClientConfig, ElicitRequestParams,
    ElicitResult, ElicitationAction, Implementation, ProtocolVersion,
};
use rmcp::service::ClientLifecycleMode;
use rmcp::service::{RequestContext, RunningService};
use rmcp::{ClientHandler, ClientServiceExt, RoleClient, ServiceExt};
use serde_json::{Value, json};

static RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap()
});

static SESSIONS: [std::sync::atomic::AtomicUsize; 4] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; 4];
static RETRIES: [std::sync::atomic::AtomicUsize; 7] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; 7];

fn count(bank: &[std::sync::atomic::AtomicUsize], slot: usize, by: usize) {
    bank[slot].fetch_add(by, std::sync::atomic::Ordering::Relaxed);
}

fn report(bank: &[std::sync::atomic::AtomicUsize], what: &[&str]) {
    let line: Vec<String> = what
        .iter()
        .zip(bank)
        .map(|(w, n)| format!("{w} {}", n.load(std::sync::atomic::Ordering::Relaxed)))
        .collect();
    eprintln!("coverage: {}", line.join(", "));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Btn {
    Back,
    Place,
    Send,
    Delete,
    Install,
}

const ALL_BTNS: [Btn; 5] = [Btn::Back, Btn::Place, Btn::Send, Btn::Delete, Btn::Install];

impl Btn {
    fn target(self) -> &'static str {
        match self {
            Btn::Back => "Back",
            Btn::Place => "Place order",
            Btn::Send => "Send",
            Btn::Delete => "Delete photo",
            Btn::Install => "Install",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Btn::Place => PLACE_ORDER,
            other => other.target(),
        }
    }

    fn centre_y(self) -> f64 {
        match self {
            Btn::Send => 25.0,
            Btn::Delete => 75.0,
            Btn::Back => 325.0,
            Btn::Place => 425.0,
            Btn::Install => 575.0,
        }
    }

    fn category(self) -> Option<Category> {
        match self {
            Btn::Back => None,
            Btn::Place => Some(Category::Payment),
            Btn::Send => Some(Category::Send),
            Btn::Delete => Some(Category::Delete),
            Btn::Install => Some(Category::Install),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Tap(Btn),
    Wait,
    Submit,
}

impl Step {
    fn json(self) -> Value {
        match self {
            Step::Tap(b) => json!({"tool": "tap", "target": b.target()}),
            Step::Wait => json!({"tool": "wait", "condition": "settled"}),
            Step::Submit => json!({"tool": "type", "text": "x", "submit": true}),
        }
    }

    fn category(self) -> Option<Category> {
        match self {
            Step::Tap(b) => b.category(),
            Step::Wait => None,
            Step::Submit => Some(Category::Payment),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Step::Tap(b) => b.label(),
            Step::Wait => "",
            Step::Submit => PLACE_ORDER,
        }
    }

    fn event(self) -> Option<Ev> {
        match self {
            Step::Tap(b) => Some(Ev::Tap(b)),
            Step::Wait => None,
            Step::Submit => Some(Ev::Submit),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ev {
    Tap(Btn),
    Submit,
}

#[derive(Clone, Debug)]
enum Item {
    Batch(Vec<Step>),
    Tap(Btn, bool),
    Submit(bool),
}

impl Item {
    fn steps(&self) -> Vec<Step> {
        match self {
            Item::Batch(s) => s.clone(),
            Item::Tap(b, _) => vec![Step::Tap(*b)],
            Item::Submit(_) => vec![Step::Submit],
        }
    }

    fn autonomous(&self) -> bool {
        matches!(self, Item::Tap(_, true) | Item::Submit(true))
    }

    fn tool(&self) -> &'static str {
        match self {
            Item::Batch(_) => "batch",
            Item::Tap(..) => "tap",
            Item::Submit(_) => "type",
        }
    }

    fn args(&self) -> Value {
        match self {
            Item::Batch(s) => json!({"steps": s.iter().map(|s| s.json()).collect::<Vec<_>>()}),
            Item::Tap(b, auto) => json!({"target": b.target(), "autonomous": auto}),
            Item::Submit(auto) => json!({"text": "x", "submit": true, "autonomous": auto}),
        }
    }

    fn key(&self) -> String {
        format!("{}{}", self.tool(), self.args())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ans {
    Allow,
    Session,
    Decline,
    Cancel,
    AcceptBare,
    Junk,
}

impl Ans {
    fn licenses(self) -> bool {
        matches!(self, Ans::Allow | Ans::Session | Ans::AcceptBare)
    }

    fn elicit(self) -> ElicitResult {
        match self {
            Ans::Allow => ElicitResult::new(ElicitationAction::Accept)
                .with_content(json!({"choice": "allow"})),
            Ans::Session => ElicitResult::new(ElicitationAction::Accept)
                .with_content(json!({"choice": "session"})),
            Ans::Decline | Ans::Junk => ElicitResult::new(ElicitationAction::Decline),
            Ans::Cancel => ElicitResult::new(ElicitationAction::Cancel),
            Ans::AcceptBare => ElicitResult::new(ElicitationAction::Accept),
        }
    }

    fn wire(self) -> Value {
        match self {
            Ans::Junk => json!({"x": 1}),
            other => serde_json::to_value(other.elicit()).unwrap(),
        }
    }
}

fn the_events(fake: &phonedriver_core::testing::FakeDevice) -> Vec<Ev> {
    fake.calls()
        .iter()
        .filter_map(|c| match c {
            Call::Gesture(g) => {
                let y = g.fingers[0][0].at.y;
                ALL_BTNS
                    .iter()
                    .find(|b| (b.centre_y() - f64::from(y)).abs() < 20.0)
                    .map(|b| Ev::Tap(*b))
            }
            Call::Key(phonedriver_core::device::Key::Enter) => Some(Ev::Submit),
            _ => None,
        })
        .collect()
}

#[derive(Clone)]
struct Model {
    session: Vec<Category>,
    ask: Vec<Category>,
}

enum Run {
    Done(Vec<Ev>),
    Held(Vec<Ev>, usize),
}

impl Model {
    fn is_guarded(&self, c: Category) -> bool {
        c == Category::Payment || self.ask.contains(&c)
    }

    fn held_here(&self, step: Step) -> bool {
        step.category()
            .is_some_and(|c| self.is_guarded(c) && !self.session.contains(&c))
    }

    fn run(&mut self, steps: &[Step], from: usize, license: Option<(usize, Ans)>) -> Run {
        let mut events = Vec::new();
        for (i, step) in steps.iter().enumerate().skip(from) {
            let licensed = license.filter(|(at, _)| *at == i);
            if self.held_here(*step) && licensed.is_none() {
                return Run::Held(events, i);
            }
            if let (Some((_, Ans::Session)), Some(c)) = (licensed, step.category())
                && !self.session.contains(&c)
            {
                self.session.push(c);
            }
            events.extend(step.event());
        }
        Run::Done(events)
    }
}

struct Scripted {
    can_ask: bool,
    version: ProtocolVersion,
    answers: Arc<Mutex<VecDeque<Ans>>>,
    prompts: Arc<Mutex<Vec<String>>>,
}

impl ClientHandler for Scripted {
    fn get_info(&self) -> ClientConfig {
        let caps = if self.can_ask {
            ClientCapabilities::builder().enable_elicitation().build()
        } else {
            ClientCapabilities::builder().build()
        };
        ClientConfig::new(caps, Implementation::new("guard-property", "0"))
            .with_protocol_version(self.version.clone())
    }

    async fn create_elicitation(
        &self,
        request: ElicitRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> Result<ElicitResult, rmcp::ErrorData> {
        let ElicitRequestParams::FormElicitationParams { message, .. } = request else {
            panic!("a form was expected");
        };
        self.prompts.lock().unwrap().push(message);
        let next = self.answers.lock().unwrap().pop_front();
        Ok(next.unwrap_or(Ans::Decline).elicit())
    }
}

struct World {
    rig: guard_rig::Rig,
    client: RunningService<RoleClient, Scripted>,
    answers: Arc<Mutex<VecDeque<Ans>>>,
    prompts: Arc<Mutex<Vec<String>>>,
}

async fn world(
    version: ProtocolVersion,
    can_ask: bool,
    ask: &[Category],
    answers: &[Ans],
) -> World {
    let rig = rig().await;
    let (client_io, server_io) = tokio::io::duplex(1 << 20);
    let server = Mcp::new(Some(rig.socket.clone()), "mcp-guard".into()).asking(ask.to_vec());
    tokio::spawn(async move {
        let running = server.serve(server_io).await.unwrap();
        let _ = running.waiting().await;
    });
    let answers = Arc::new(Mutex::new(answers.iter().copied().collect::<VecDeque<_>>()));
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let lifecycle = if version >= ProtocolVersion::V_2026_07_28 {
        ClientLifecycleMode::Discover {
            preferred_versions: vec![version.clone()],
        }
    } else {
        ClientLifecycleMode::Initialize
    };
    let client = Scripted {
        can_ask,
        version,
        answers: answers.clone(),
        prompts: prompts.clone(),
    }
    .serve_with_lifecycle(client_io, lifecycle)
    .await
    .unwrap();
    let w = World {
        rig,
        client,
        answers,
        prompts,
    };
    w.client
        .call_tool(CallToolRequestParams::new("snapshot"))
        .await
        .unwrap();
    w
}

fn params(item: &Item) -> CallToolRequestParams {
    CallToolRequestParams::new(item.tool()).with_arguments(item.args().as_object().unwrap().clone())
}

fn text(r: &rmcp::model::CallToolResult) -> String {
    r.content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn version(mrtr: bool) -> ProtocolVersion {
    if mrtr {
        ProtocolVersion::V_2026_07_28
    } else {
        ProtocolVersion::V_2025_11_25
    }
}

fn config(default_cases: u32) -> ProptestConfig {
    let mut c = ProptestConfig::default();
    if std::env::var("PROPTEST_CASES").is_err() {
        c.cases = default_cases;
    }
    c.failure_persistence = None;
    c.max_shrink_iters = 400;
    c.max_shrink_time = 40_000;
    c
}

fn btn() -> impl Strategy<Value = Btn> {
    prop_oneof![
        3 => Just(Btn::Place),
        2 => Just(Btn::Back),
        2 => Just(Btn::Send),
        2 => Just(Btn::Delete),
        2 => Just(Btn::Install),
    ]
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        8 => btn().prop_map(Step::Tap),
        1 => Just(Step::Wait),
        1 => Just(Step::Submit),
    ]
}

fn ans() -> impl Strategy<Value = Ans> {
    prop_oneof![
        4 => Just(Ans::Allow),
        2 => Just(Ans::Session),
        2 => Just(Ans::Decline),
        2 => Just(Ans::Cancel),
        1 => Just(Ans::AcceptBare),
        1 => Just(Ans::Junk),
    ]
}

fn ask_set() -> impl Strategy<Value = Vec<Category>> {
    proptest::collection::vec(any::<bool>(), 3).prop_map(|b| {
        [Category::Send, Category::Delete, Category::Install]
            .into_iter()
            .zip(b)
            .filter_map(|(c, on)| on.then_some(c))
            .collect()
    })
}

fn item() -> impl Strategy<Value = Item> {
    prop_oneof![
        6 => proptest::collection::vec(step(), 1..=5).prop_map(Item::Batch),
        2 => (btn(), prop::bool::weighted(0.15)).prop_map(|(b, a)| Item::Tap(b, a)),
        1 => prop::bool::weighted(0.15).prop_map(Item::Submit),
    ]
}

fn name(c: Category) -> &'static str {
    c.name()
}

async fn the_session_of_calls(
    mrtr: bool,
    can_ask: bool,
    ask: Vec<Category>,
    answers: Vec<Ans>,
    calls: Vec<Item>,
) -> Result<(), String> {
    let w = world(version(mrtr), can_ask, &ask, &answers).await;
    let mut model = Model {
        session: vec![],
        ask: ask.clone(),
    };
    let mut queue: VecDeque<Ans> = answers.iter().copied().collect();
    let mut got_taps = 0;
    let mut expected_prompts: Vec<(String, &'static str)> = vec![];
    let mut expected_events: Vec<Ev> = vec![];
    for (n, call) in calls.iter().enumerate() {
        let r = w
            .client
            .call_tool(params(call))
            .await
            .map_err(|e| format!("call {n} {call:?}: {e}"))?;
        let steps = call.steps();
        if call.autonomous() {
            expected_events.extend(steps.iter().filter_map(|s| s.event()));
        } else {
            let mut from = 0;
            let mut license = None;
            loop {
                match model.run(&steps, from, license) {
                    Run::Done(ev) => {
                        expected_events.extend(ev);
                        break;
                    }
                    Run::Held(ev, at) => {
                        expected_events.extend(ev);
                        if !can_ask {
                            break;
                        }
                        let cat = steps[at].category().unwrap();
                        expected_prompts.push((steps[at].label().to_string(), name(cat)));
                        let a = queue.pop_front().unwrap_or(Ans::Decline);
                        if !a.licenses() {
                            break;
                        }
                        license = Some((at, a));
                        from = at;
                    }
                }
            }
        }
        let got = the_events(&w.rig.fake);
        if got != expected_events {
            return Err(format!(
                "after call {n} {call:?}\n  device saw      {got:?}\n  the promise says {expected_events:?}\n  reply: {}\n  prompts: {:?}",
                text(&r),
                w.prompts.lock().unwrap()
            ));
        }
        got_taps = got.len();
    }
    let prompts = w.prompts.lock().unwrap().clone();
    let shown: Vec<(String, String)> = prompts
        .iter()
        .map(|p| {
            let label = p
                .split("Tap \"")
                .nth(1)
                .and_then(|r| r.split('"').next())
                .unwrap_or("?")
                .to_string();
            let cat = p
                .split("Category: ")
                .nth(1)
                .and_then(|r| r.split(' ').next())
                .unwrap_or("?")
                .to_string();
            (label, cat)
        })
        .collect();
    let want: Vec<(String, String)> = expected_prompts
        .into_iter()
        .map(|(l, c)| (l, c.to_string()))
        .collect();
    if shown != want {
        return Err(format!(
            "the human was asked\n  {shown:?}\nbut the promise says\n  {want:?}"
        ));
    }
    count(&SESSIONS, 0, 1);
    count(&SESSIONS, 1, calls.len());
    count(&SESSIONS, 2, want.len());
    count(&SESSIONS, 3, got_taps);
    let _ = w.answers;
    Ok(())
}

type Scenario = (bool, bool, Vec<Category>, Vec<Ans>, Vec<Item>);

fn scenario() -> impl Strategy<Value = Scenario> {
    (
        any::<bool>(),
        prop::bool::weighted(0.85),
        ask_set(),
        proptest::collection::vec(ans(), 0..=14),
        proptest::collection::vec(item(), 1..=3),
    )
}

async fn all_at_once(scenarios: Vec<Scenario>) -> Result<(), String> {
    let mut running = tokio::task::JoinSet::new();
    for (mrtr, can_ask, ask, answers, calls) in scenarios {
        running.spawn(async move {
            the_session_of_calls(mrtr, can_ask, ask.clone(), answers.clone(), calls.clone())
                .await
                .map_err(|why| {
                    format!(
                        "protocol {}, client {}, ask {ask:?}, answers {answers:?}\ncalls {calls:?}\n{why}",
                        if mrtr { "2026-07-28" } else { "2025-11-25" },
                        if can_ask { "answers prompts" } else { "cannot ask" },
                    )
                })
        });
    }
    while let Some(done) = running.join_next().await {
        done.map_err(|e| e.to_string())??;
    }
    Ok(())
}

proptest! {
    #![proptest_config(config(12))]

    #[test]
    fn a_guarded_step_reaches_the_device_only_after_the_humans_answer(
        scenarios in proptest::collection::vec(scenario(), 1..=8),
    ) {
        let outcome = RT.block_on(all_at_once(scenarios));
        report(&SESSIONS, &["scenarios", "calls", "prompts", "device inputs"]);
        prop_assert!(outcome.is_ok(), "{}", outcome.unwrap_err());
    }
}

struct Pending {
    step: usize,
    label: &'static str,
}

#[derive(Clone, Debug)]
enum Op {
    Call(usize),
    Retry(usize, Ans),
}

fn ops(pool: usize) -> impl Strategy<Value = Vec<Op>> {
    let group = prop_oneof![
        2 => (0..pool).prop_map(|i| vec![Op::Call(i)]),
        3 => ((0..pool), ans()).prop_map(|(i, a)| vec![Op::Retry(i, a)]),
        4 => ((0..pool), ans()).prop_map(|(i, a)| vec![Op::Call(i), Op::Retry(i, a)]),
    ];
    proptest::collection::vec(group, 1..=6)
        .prop_map(|groups| groups.into_iter().flatten().collect::<Vec<_>>())
}

async fn the_retries(ask: Vec<Category>, pool: Vec<Item>, ops: Vec<Op>) -> Result<(), String> {
    let w = world(ProtocolVersion::V_2026_07_28, true, &ask, &[]).await;
    let mut model = Model {
        session: vec![],
        ask,
    };
    let mut pending: HashMap<String, Pending> = HashMap::new();
    let mut after_ask_for: Option<String> = None;
    let mut story: Vec<String> = vec![];
    let mut logged = 0;
    count(&RETRIES, 0, 1);
    count(&RETRIES, 1, ops.len());
    for (n, o) in ops.iter().enumerate() {
        let (i, answer) = match o {
            Op::Call(i) => (*i, None),
            Op::Retry(i, a) => (*i, Some(*a)),
        };
        let it = &pool[i];
        let key = it.key();
        let mut p = params(it);
        if let Some(a) = answer {
            p = p.with_input_responses([("approve".to_string(), a.wire())].into());
        }
        let resp = w
            .client
            .peer()
            .call_tool_once(p)
            .await
            .map_err(|e| format!("op {n} {o:?}: {e}"))?;
        let all = the_events(&w.rig.fake);
        let got: Vec<Ev> = all[logged..].to_vec();
        logged = all.len();
        let asked_label = match &resp {
            CallToolResponse::InputRequired(q) => {
                let m = serde_json::to_value(q).unwrap().to_string();
                Some(m)
            }
            _ => None,
        };
        story.push(format!(
            "{o:?} on {} -> events {got:?}, {}",
            it.tool(),
            match &resp {
                CallToolResponse::InputRequired(_) => "asked".to_string(),
                CallToolResponse::Complete(r) => format!("done: {}", text(r)),
                _ => "task".to_string(),
            }
        ));
        let steps = it.steps();
        let before = model.clone();
        let fresh_ask = after_ask_for.take();
        let licensing = answer.filter(|a| a.licenses());
        let live = pending.remove(&key);
        let mut expect_ask: Option<Pending> = None;
        let expected: Vec<Vec<Ev>> = if it.autonomous() {
            vec![steps.iter().filter_map(|s| s.event()).collect()]
        } else {
            match (answer, licensing, live.as_ref()) {
                (None, _, _) => match model.run(&steps, 0, None) {
                    Run::Done(ev) => vec![ev],
                    Run::Held(ev, at) => {
                        expect_ask = Some(Pending {
                            step: at,
                            label: steps[at].label(),
                        });
                        vec![ev]
                    }
                },
                (Some(_), None, Some(_)) => {
                    count(&RETRIES, 6, 1);
                    vec![vec![]]
                }
                (Some(_), Some(a), Some(pend)) => {
                    let mut resumed = model.clone();
                    let exact = match resumed.run(&steps, pend.step, Some((pend.step, a))) {
                        Run::Done(ev) => ev,
                        Run::Held(ev, at) => {
                            expect_ask = Some(Pending {
                                step: at,
                                label: steps[at].label(),
                            });
                            ev
                        }
                    };
                    if fresh_ask.as_deref() == Some(key.as_str()) {
                        count(&RETRIES, 3, 1);
                        model = resumed;
                        vec![exact]
                    } else if got.is_empty() {
                        count(&RETRIES, 4, 1);
                        expect_ask = None;
                        vec![vec![]]
                    } else {
                        count(&RETRIES, 3, 1);
                        model = resumed;
                        vec![exact]
                    }
                }
                (Some(_), _, None) => {
                    count(&RETRIES, 5, 1);
                    let unlicensed_safe: Vec<Ev> = got
                        .iter()
                        .copied()
                        .take_while(|e| {
                            let s = match e {
                                Ev::Tap(b) => Step::Tap(*b),
                                Ev::Submit => Step::Submit,
                            };
                            !before.held_here(s)
                        })
                        .collect();
                    vec![unlicensed_safe]
                }
            }
        };
        if !expected.contains(&got) {
            return Err(format!(
                "op {n} {o:?} on {} {}\n  device saw {got:?}\n  allowed    {expected:?}\n  history:\n    {}",
                it.tool(),
                it.args(),
                story.join("\n    ")
            ));
        }
        match (&resp, expect_ask) {
            (CallToolResponse::InputRequired(_), Some(next)) => {
                count(&RETRIES, 2, 1);
                let m = asked_label.unwrap();
                if !m.contains(next.label) {
                    return Err(format!(
                        "op {n} {o:?}: asked about something else than {}: {m}",
                        next.label
                    ));
                }
                pending.insert(key.clone(), next);
                after_ask_for = Some(key);
            }
            (CallToolResponse::InputRequired(_), None) if !it.autonomous() && answer.is_none() => {
                return Err(format!(
                    "op {n} {o:?}: asked, but the promise says no hold\n  history:\n    {}",
                    story.join("\n    ")
                ));
            }
            (CallToolResponse::Complete(_), Some(next)) if answer.is_none() => {
                return Err(format!(
                    "op {n} {o:?}: finished without asking about {} at step {}\n  history:\n    {}",
                    next.label,
                    next.step + 1,
                    story.join("\n    ")
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn pool_and_ops(batches_only: bool) -> impl Strategy<Value = (Vec<Category>, Vec<Item>, Vec<Op>)> {
    let items = if batches_only {
        proptest::collection::vec(
            proptest::collection::vec(step(), 1..=4).prop_map(Item::Batch),
            1..=3,
        )
        .boxed()
    } else {
        proptest::collection::vec(item(), 1..=3).boxed()
    };
    (ask_set(), items).prop_flat_map(|(ask, pool)| {
        let n = pool.len();
        (Just(ask), Just(pool), ops(n))
    })
}

const EVERY_ANSWER: [Ans; 6] = [
    Ans::Allow,
    Ans::Session,
    Ans::Decline,
    Ans::Cancel,
    Ans::AcceptBare,
    Ans::Junk,
];

#[test]
fn every_answer_to_every_kind_of_held_input_on_both_protocols() {
    let held_inputs: Vec<(Vec<Category>, Item)> = vec![
        (vec![], Item::Tap(Btn::Place, false)),
        (vec![Category::Send], Item::Tap(Btn::Send, false)),
        (vec![], Item::Submit(false)),
        (
            vec![],
            Item::Batch(vec![
                Step::Tap(Btn::Back),
                Step::Tap(Btn::Place),
                Step::Tap(Btn::Back),
            ]),
        ),
        (
            vec![Category::Send, Category::Delete],
            Item::Batch(vec![
                Step::Tap(Btn::Send),
                Step::Wait,
                Step::Tap(Btn::Delete),
            ]),
        ),
    ];
    let mut scenarios: Vec<Scenario> = Vec::new();
    for mrtr in [false, true] {
        for (ask, item) in &held_inputs {
            for a in EVERY_ANSWER {
                scenarios.push((
                    mrtr,
                    true,
                    ask.clone(),
                    vec![a, a, a],
                    vec![item.clone(), item.clone()],
                ));
            }
            scenarios.push((
                mrtr,
                false,
                ask.clone(),
                vec![],
                vec![item.clone(), item.clone()],
            ));
        }
    }
    let outcome = RT.block_on(all_at_once(scenarios));
    assert!(outcome.is_ok(), "{}", outcome.unwrap_err());
}

type Retries = (Vec<Category>, Vec<Item>, Vec<Op>);

async fn all_retries_at_once(scenarios: Vec<Retries>) -> Result<(), String> {
    let mut running = tokio::task::JoinSet::new();
    for (ask, pool, ops) in scenarios {
        running.spawn(async move {
            the_retries(ask.clone(), pool.clone(), ops.clone())
                .await
                .map_err(|why| format!("ask {ask:?}\npool {pool:?}\nops {ops:?}\n{why}"))
        });
    }
    while let Some(done) = running.join_next().await {
        done.map_err(|e| e.to_string())??;
    }
    Ok(())
}

proptest! {
    #![proptest_config(config(10))]

    #[test]
    fn a_stale_or_mismatched_retry_of_a_batch_never_taps_a_guarded_step(
        scenarios in proptest::collection::vec(pool_and_ops(true), 1..=8),
    ) {
        let outcome = RT.block_on(all_retries_at_once(scenarios));
        report(&RETRIES, &["scenarios", "ops", "fresh questions", "exact resumes", "resumes lost", "answers with no question", "refused answers"]);
        prop_assert!(outcome.is_ok(), "{}", outcome.unwrap_err());
    }

    #[test]
    #[ignore = "finding F1: a tap or type retry carries its own answer and the server keeps no record of the question"]
    fn a_stale_or_mismatched_retry_of_a_tap_or_type_never_taps_a_guarded_step(
        scenarios in proptest::collection::vec(pool_and_ops(false), 1..=8),
    ) {
        let outcome = RT.block_on(all_retries_at_once(scenarios));
        prop_assert!(outcome.is_ok(), "{}", outcome.unwrap_err());
    }
}

async fn once(w: &World, item: &Item, answer: Option<Ans>) -> CallToolResponse {
    let mut p = params(item);
    if let Some(a) = answer {
        p = p.with_input_responses([("approve".to_string(), a.wire())].into());
    }
    w.client.peer().call_tool_once(p).await.unwrap()
}

#[test]
#[ignore = "finding F1: an answer to a question nobody asked taps the button"]
fn f1_an_answer_with_no_question_taps_a_payment() {
    RT.block_on(async {
        let w = world(ProtocolVersion::V_2026_07_28, true, &[], &[]).await;
        once(&w, &Item::Tap(Btn::Place, false), Some(Ans::Allow)).await;
        once(&w, &Item::Submit(false), Some(Ans::Allow)).await;
        assert_eq!(
            the_events(&w.rig.fake),
            vec![],
            "no question was asked, yet the device was driven"
        );
    });
}

#[test]
#[ignore = "finding F1: an answer to one tap approves a different tap"]
fn f1_an_answer_to_one_tap_approves_another() {
    RT.block_on(async {
        let w = world(
            ProtocolVersion::V_2026_07_28,
            true,
            &[Category::Delete],
            &[],
        )
        .await;
        let asked = once(&w, &Item::Tap(Btn::Place, false), None).await;
        assert!(matches!(asked, CallToolResponse::InputRequired(_)));
        once(&w, &Item::Tap(Btn::Delete, false), Some(Ans::Allow)).await;
        assert_eq!(
            the_events(&w.rig.fake),
            vec![],
            "the question was about Place order; Delete photo was tapped"
        );
    });
}

#[test]
#[ignore = "finding F2: a stray answer for another batch cancels the pending question"]
fn f2_a_stray_answer_does_not_cancel_the_pending_question() {
    RT.block_on(async {
        let w = world(ProtocolVersion::V_2026_07_28, true, &[], &[]).await;
        let a = Item::Batch(vec![Step::Tap(Btn::Back), Step::Tap(Btn::Place)]);
        let b = Item::Batch(vec![Step::Tap(Btn::Back)]);
        let asked = once(&w, &a, None).await;
        assert!(matches!(asked, CallToolResponse::InputRequired(_)));
        once(&w, &b, Some(Ans::Allow)).await;
        once(&w, &a, Some(Ans::Allow)).await;
        assert_eq!(
            the_events(&w.rig.fake),
            vec![Ev::Tap(Btn::Back), Ev::Tap(Btn::Place)],
            "the answer to the open question was lost"
        );
    });
}
