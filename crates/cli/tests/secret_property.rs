//! Read with `.claude/skills/verify-secrets/SKILL.md`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use phonedriver::daemon::trace::TraceOptions;
use phonedriver::daemon::{Config, Daemon, DeviceSource, Listed};
use phonedriver_core::device::{Capabilities, Device, ShellReply, TextInput, WebPage};
use phonedriver_core::error::Error;
use phonedriver_core::geometry::Scale;
use phonedriver_core::raw::RawTree;
use phonedriver_core::testing::FakeDevice;
use proptest::prelude::*;
use serde_json::{Value, json};

struct One(Arc<FakeDevice>);

#[async_trait]
impl DeviceSource for One {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(vec![Listed {
            id: "fake:1".into(),
            model: "Fake".into(),
            os: None,
            tier: "fake".into(),
            capabilities: self.0.info().capabilities,
        }])
    }
    async fn connect(&self, _id: &str) -> Result<Arc<dyn Device>, Error> {
        Ok(self.0.clone())
    }
}

#[derive(Clone, Default)]
struct Screen {
    focus: Option<Field>,
    note: String,
    password: String,
    password_took: Vec<String>,
    deaf: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Field {
    Note,
    Password,
}

impl Screen {
    fn tree(&self) -> RawTree {
        let focused = |f| if self.focus == Some(f) { "F" } else { "" };
        let json = format!(
            r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[
            {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
            {{"p":0,"w":1,"cls":"android.widget.EditText","text":"{}","desc":"Note","b":[0,100,360,160],"f":"vVeE{}","a":["tap","set_text"]}},
            {{"p":0,"w":1,"cls":"android.widget.EditText","text":"{}","desc":"Password","b":[0,200,360,260],"f":"vVeEP{}","a":["tap","set_text"]}},
            {{"p":0,"w":1,"cls":"android.widget.TextView","text":"{}","b":[0,300,360,360],"f":"vVe","a":["tap"]}}]}}"#,
            self.note,
            focused(Field::Note),
            "•".repeat(self.password.chars().count()),
            focused(Field::Password),
            if self.note.is_empty() {
                "Nothing yet"
            } else {
                &self.note
            },
        );
        RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
    }

    fn take(&mut self, text: &TextInput) {
        let Some(field) = self.focus.filter(|_| !self.deaf) else {
            return;
        };
        let (TextInput::SetValue(t)
        | TextInput::SetSecret(t)
        | TextInput::Paste(t)
        | TextInput::PasteSecret(t)
        | TextInput::Keystrokes(t)) = text;
        let value = match field {
            Field::Note => &mut self.note,
            Field::Password => {
                self.password_took.push(t.clone());
                &mut self.password
            }
        };
        match text {
            TextInput::SetValue(_) | TextInput::SetSecret(_) => *value = t.clone(),
            _ => value.push_str(t),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Aim {
    Note,
    Password,
    NoteByName,
    PasswordByName,
    Focused,
}

#[derive(Debug, Clone)]
enum Step {
    Snapshot,
    Type {
        flagged: bool,
        into: Aim,
        submit: bool,
        append: bool,
        in_batch: bool,
    },
    Shell {
        flagged: bool,
        echoes: bool,
    },
    Tap(Field),
    TapNothing,
    TapWhatTheNoteSays,
    Page,
    Notifications {
        waited: bool,
    },
    Find,
    Check {
        field: Field,
        wrong: bool,
    },
    Journal,
    /// The read a path's landmark is chosen from when a run is saved under `paths/` (decision D118).
    Landmark,
    RecordStart,
    RecordStop,
    Setting {
        restore: bool,
    },
}

fn steps() -> impl Strategy<Value = Vec<Step>> {
    let into = prop_oneof![
        Just(Aim::Note),
        Just(Aim::Password),
        Just(Aim::NoteByName),
        Just(Aim::PasswordByName),
        Just(Aim::Focused),
    ];
    let typed = (
        any::<bool>(),
        into,
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(flagged, into, submit, append, in_batch)| Step::Type {
            flagged,
            into,
            submit,
            append,
            in_batch,
        });
    let step = prop_oneof![
        4 => typed,
        2 => any::<(bool, bool)>().prop_map(|(flagged, echoes)| Step::Shell { flagged, echoes }),
        1 => Just(Step::Landmark),
        1 => Just(Step::TapNothing),
        1 => Just(Step::TapWhatTheNoteSays),
        2 => Just(Step::Snapshot),
        1 => Just(Step::Tap(Field::Note)),
        1 => Just(Step::Tap(Field::Password)),
        1 => Just(Step::Find),
        1 => (any::<bool>(), any::<bool>()).prop_map(|(note, wrong)| Step::Check {
            field: if note { Field::Note } else { Field::Password },
            wrong,
        }),
        1 => Just(Step::Journal),
        1 => Just(Step::RecordStart),
        1 => Just(Step::RecordStop),
        1 => Just(Step::Page),
        1 => any::<bool>().prop_map(|waited| Step::Notifications { waited }),
        1 => any::<bool>().prop_map(|restore| Step::Setting { restore }),
    ];
    prop::collection::vec(step, 1..7)
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(files(&p));
        } else if !p
            .extension()
            .is_some_and(|e| e == "sock" || e == "png" || e == "mp4")
        {
            out.push(p);
        }
    }
    out
}

#[derive(Default)]
struct Seen {
    typed_ok: usize,
    typed_refused: usize,
    shells: usize,
    checks: usize,
    settings: usize,
    secrets: usize,
    leaks: Vec<String>,
}

async fn session(steps: &[Step], full: bool, deaf: bool, fails: bool, home: &Path) -> Seen {
    let screen = Arc::new(Mutex::new(Screen {
        deaf,
        ..Screen::default()
    }));
    let fake = Arc::new(
        FakeDevice::new(vec![screen.lock().unwrap().tree()]).lacking(Capabilities::FRAMES),
    );
    fake.with_recorder();
    let s = screen.clone();
    fake.on_gesture(move |g| {
        let mut s = s.lock().unwrap();
        let y = g.fingers.first()?.first()?.at.y;
        s.focus = match y {
            y if (100.0..160.0).contains(&y) => Some(Field::Note),
            y if (200.0..260.0).contains(&y) => Some(Field::Password),
            _ => None,
        };
        Some(vec![s.tree()])
    });
    let s = screen.clone();
    fake.on_text(move |t| {
        let mut s = s.lock().unwrap();
        s.take(t);
        Some(vec![s.tree()])
    });
    let d = Daemon::with_trace(
        Arc::new(One(fake.clone())),
        Config {
            home: home.to_path_buf(),
        },
        TraceOptions {
            full,
            screens: false,
        },
    );
    let mut seen = Seen::default();
    let mut secrets: Vec<String> = Vec::new();
    let mut fails = fails;
    let call = async |method: &str, params: Value| -> (bool, String) {
        match d.call("s", method, params).await {
            Ok(v) => (true, v.to_string()),
            Err(e) => (false, format!("{e:?}")),
        }
    };
    let _ = call("snapshot", json!({})).await;
    for (i, step) in steps.iter().enumerate() {
        let (what, (_, said)) = match step {
            Step::Snapshot => ("snapshot", call("snapshot", json!({})).await),
            Step::Find => ("find", call("find", json!({"text": "Note"})).await),
            Step::Check { field, wrong } => {
                let name = if *field == Field::Note {
                    "Note"
                } else {
                    "Password"
                };
                let mut p = json!({"target": {"text": name}});
                if *wrong {
                    p["equals"] = json!("something it does not show");
                }
                let said = call("check", p).await;
                if said.0 {
                    seen.checks += 1;
                }
                ("check", said)
            }
            Step::Journal => ("journal", call("journal", json!({})).await),
            Step::Landmark => (
                "snapshot with elements",
                call("snapshot", json!({"elements": true})).await,
            ),
            Step::Setting { restore } => {
                let index = phonedriver_core::settings::SettingsIndex {
                    screens: vec![phonedriver_core::settings::SettingScreen {
                        key: "note".into(),
                        about: "Note".into(),
                        link: None,
                    }],
                    items: vec![phonedriver_core::settings::SettingItem {
                        key: "note/text".into(),
                        purpose: "A note".into(),
                        values: "TEXT".into(),
                        writable: true,
                        confirm: false,
                        screen: 0,
                    }],
                };
                fake.with_settings(index, &[("note/text", &secrets.join(" "))]);
                let said = if *restore {
                    call("setting", json!({"restore": []})).await
                } else {
                    call("setting", json!({"key": "note/text", "value": "plain"})).await
                };
                if said.0 {
                    seen.settings += 1;
                }
                ("setting", said)
            }
            Step::Tap(field) => {
                let name = if *field == Field::Note {
                    "Note"
                } else {
                    "Password"
                };
                (
                    "tap",
                    call(
                        "act",
                        json!({"action": {"kind": "tap", "target": {"text": name}}}),
                    )
                    .await,
                )
            }
            Step::TapNothing => (
                "tap nothing",
                call(
                    "act",
                    json!({"action": {"kind": "tap", "target": {"point": {"x": 180, "y": 600}}}}),
                )
                .await,
            ),
            Step::TapWhatTheNoteSays => {
                let (_, shown) = call("snapshot", json!({})).await;
                let label = shown
                    .split("@e")
                    .filter_map(|rest| rest.split_once(" text "))
                    .map(|(number, _)| format!("e{number}"))
                    .last()
                    .unwrap_or_default();
                (
                    "tap the label",
                    call(
                        "act",
                        json!({"action": {"kind": "tap", "target": {"ref": label}}}),
                    )
                    .await,
                )
            }
            Step::RecordStart => {
                let path = home.join(format!("rec{i}.mp4"));
                (
                    "record start",
                    call("record", json!({"action": "start", "path": path})).await,
                )
            }
            Step::RecordStop => (
                "record stop",
                call("record", json!({"action": "stop"})).await,
            ),
            Step::Page => {
                fake.show_page(WebPage {
                    title: "What you entered".into(),
                    url: "https://form.example/done".into(),
                    text: secrets.join("\n"),
                });
                ("page", call("page", json!({})).await)
            }
            Step::Notifications { waited } => {
                fake.answer_notifications(vec![phonedriver_core::notice::Notices {
                    now_ms: 10_000_000,
                    notices: vec![phonedriver_core::notice::Notice {
                        key: "0|com.example|1|echo|1".into(),
                        package: "com.example".into(),
                        title: Some("You entered".into()),
                        text: Some(secrets.join(" ")),
                        posted_ms: 10_000_000,
                        ongoing: false,
                    }],
                }]);
                if *waited {
                    (
                        "wait notification",
                        call(
                            "wait",
                            json!({"condition": {"notification": {"pattern": "entered"}}}),
                        )
                        .await,
                    )
                } else {
                    ("notifications", call("notifications", json!({})).await)
                }
            }
            Step::Shell { flagged, echoes } => {
                let command = format!("login --token Sh{i}xK9pQ2vL7");
                let earlier = if *echoes {
                    secrets.join("\n")
                } else {
                    String::new()
                };
                fake.answer_shell(ShellReply {
                    exit: 0,
                    stdout: format!("+ {command}\nok\n{earlier}"),
                    stderr: String::new(),
                    timed_out: false,
                });
                if *flagged {
                    secrets.push(command.clone());
                }
                seen.shells += 1;
                (
                    "shell",
                    call("shell", json!({"command": command, "secret": flagged})).await,
                )
            }
            Step::Type {
                flagged,
                into,
                submit,
                append,
                in_batch,
            } => {
                let text = format!("Ty{i}mW4zR8nB3c");
                let target = match into {
                    Aim::Note => Some(json!({"ref": "e1"})),
                    Aim::Password => Some(json!({"ref": "e2"})),
                    Aim::NoteByName => Some(json!({"text": "Note"})),
                    Aim::PasswordByName => Some(json!({"text": "Password"})),
                    Aim::Focused => None,
                };
                if *flagged {
                    secrets.push(text.clone());
                }
                if std::mem::take(&mut fails) {
                    fake.fail_text(vec![Error::Transport {
                        detail: format!(
                            "the agent could not run text: IllegalStateException: {text}"
                        ),
                    }]);
                }
                let mut action = json!({"kind": "type", "text": text, "secret": flagged, "submit": submit, "append": append});
                if let Some(target) = target {
                    action["into"] = target;
                }
                let said = if *in_batch {
                    call("batch", json!({"steps": [action]})).await
                } else {
                    call("act", json!({"action": action})).await
                };
                if said.0 {
                    seen.typed_ok += 1;
                } else {
                    seen.typed_refused += 1;
                }
                ("type", said)
            }
        };
        let on_disk: Vec<(PathBuf, String)> = files(home)
            .into_iter()
            .map(|f| {
                let text =
                    String::from_utf8_lossy(&std::fs::read(&f).unwrap_or_default()).into_owned();
                (f, text)
            })
            .collect();
        for secret in &secrets {
            if said.contains(secret.as_str()) {
                seen.leaks.push(format!(
                    "step {i} ({what}): the reply holds the secret {secret}: {}",
                    said.chars().take(400).collect::<String>()
                ));
            }
            for (file, text) in &on_disk {
                if text.contains(secret.as_str()) {
                    seen.leaks.push(format!(
                        "step {i} ({what}): {} holds the secret {secret}",
                        file.strip_prefix(home).unwrap_or(file).display()
                    ));
                }
            }
        }
    }
    let (_, saved) = call("journal", json!({})).await;
    for secret in &secrets {
        if saved.contains(secret.as_str()) {
            seen.leaks.push(format!(
                "the steps `save` would write hold the secret {secret}: {saved}"
            ));
        }
    }
    let debug = format!("{:?}", fake.calls());
    for secret in &secrets {
        if debug.contains(secret.as_str()) {
            seen.leaks
                .push(format!("the device's call log prints the secret {secret}"));
        }
    }
    for took in &screen.lock().unwrap().password_took {
        if !took.is_empty() && !secrets.iter().any(|s| took.ends_with(s.as_str())) {
            seen.leaks.push(format!(
                "the password field was given text that was not marked secret: {took}"
            ));
        }
    }
    seen.secrets = secrets.len();
    seen
}

fn run<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
        .block_on(f)
}

static COVERAGE: Mutex<(usize, usize, usize, usize)> = Mutex::new((0, 0, 0, 0));

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, failure_persistence: None, ..ProptestConfig::default() })]

    #[test]
    fn a_secret_reaches_no_reply_error_or_file_and_plain_text_no_password_field(
        steps in steps(), full in any::<bool>(), deaf in prop::bool::weighted(0.15), fails in prop::bool::weighted(0.15),
    ) {
        let home = tempfile::tempdir().unwrap();
        let seen = run(session(&steps, full, deaf, fails, home.path()));
        {
            let mut c = COVERAGE.lock().unwrap();
            c.0 += seen.typed_ok;
            c.1 += seen.typed_refused;
            c.2 += seen.shells;
            c.3 += seen.secrets;
            eprintln!("coverage: {} types that worked, {} that were refused or failed, {} shell commands, {} secrets", c.0, c.1, c.2, c.3);
        }
        prop_assert!(seen.leaks.is_empty(), "\n{}\nsteps: {steps:?}, full trace {full}, deaf fields {deaf}, failing device {fails}", seen.leaks.join("\n"));
    }
}

fn typed(flagged: bool, into: Aim, in_batch: bool) -> Step {
    Step::Type {
        flagged,
        into,
        submit: false,
        append: false,
        in_batch,
    }
}

#[test]
fn the_sessions_that_leaked_before_the_fixes_stay_closed() {
    let found: [(&str, Vec<Step>, bool); 6] = [
        (
            "a check of a field that shows the secret, right and wrong",
            vec![
                typed(true, Aim::Note, false),
                Step::Check {
                    field: Field::Note,
                    wrong: false,
                },
                Step::Check {
                    field: Field::Note,
                    wrong: true,
                },
            ],
            false,
        ),
        (
            "F1, a later look at a plain field",
            vec![typed(true, Aim::Note, false), Step::Snapshot, Step::Find],
            false,
        ),
        (
            "F1, the device prints it back",
            vec![
                typed(true, Aim::Note, false),
                Step::Shell {
                    flagged: false,
                    echoes: true,
                },
            ],
            false,
        ),
        ("F2, a batch", vec![typed(true, Aim::Note, true)], false),
        (
            "F5, a tap on a label that shows the secret, while recording",
            vec![
                Step::RecordStart,
                typed(true, Aim::Note, false),
                Step::TapWhatTheNoteSays,
                Step::RecordStop,
            ],
            false,
        ),
        (
            "F3, the focus moved under a step that failed",
            vec![
                Step::Tap(Field::Note),
                typed(true, Aim::PasswordByName, false),
                typed(false, Aim::Focused, false),
            ],
            true,
        ),
    ];
    for (name, steps, fails) in found {
        let home = tempfile::tempdir().unwrap();
        let seen = run(session(&steps, true, false, fails, home.path()));
        assert!(seen.leaks.is_empty(), "{name}:\n{}", seen.leaks.join("\n"));
    }
}

#[test]
fn every_kind_of_step_works_on_the_fake_phone() {
    let home = tempfile::tempdir().unwrap();
    let steps = vec![
        Step::Tap(Field::Note),
        Step::Type {
            flagged: true,
            into: Aim::Note,
            submit: false,
            append: false,
            in_batch: false,
        },
        Step::Type {
            flagged: true,
            into: Aim::Password,
            submit: false,
            append: false,
            in_batch: false,
        },
        Step::Type {
            flagged: true,
            into: Aim::Focused,
            submit: false,
            append: true,
            in_batch: false,
        },
        Step::Shell {
            flagged: true,
            echoes: true,
        },
        Step::RecordStart,
        Step::Snapshot,
        Step::Find,
        Step::Check {
            field: Field::Note,
            wrong: false,
        },
        Step::Check {
            field: Field::Note,
            wrong: true,
        },
        Step::RecordStop,
        Step::Page,
        Step::Notifications { waited: false },
        Step::Notifications { waited: true },
        Step::Landmark,
        Step::Setting { restore: false },
        Step::Setting { restore: true },
        Step::Journal,
    ];
    let seen = run(session(&steps, true, false, false, home.path()));
    assert_eq!(
        (
            seen.typed_ok,
            seen.typed_refused,
            seen.shells,
            seen.checks,
            seen.settings,
            seen.secrets
        ),
        (3, 0, 1, 1, 2, 4)
    );
}

#[test]
#[ignore = "prints every kind of leak in 3000 sessions; the property stops at the first"]
fn survey() {
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;
    let mut runner = TestRunner::deterministic();
    let mut kinds: std::collections::BTreeMap<String, (usize, String)> = Default::default();
    for n in 0..3000 {
        let steps = steps().new_tree(&mut runner).unwrap().current();
        let home = tempfile::tempdir().unwrap();
        let seen = run(session(
            &steps,
            n % 2 == 0,
            n % 7 == 0,
            n % 5 == 0,
            home.path(),
        ));
        for leak in seen.leaks {
            let kind: String = leak
                .split(" holds the secret ")
                .next()
                .unwrap_or(&leak)
                .split_once(": ")
                .map_or(leak.as_str(), |(_, rest)| rest)
                .to_string();
            let kind = match kind.find("traces/") {
                Some(at) => format!("{}a trace file", &kind[..at]),
                None => kind,
            };
            let step = leak
                .split(')')
                .next()
                .unwrap_or("")
                .split('(')
                .nth(1)
                .unwrap_or("")
                .to_string();
            let entry = kinds
                .entry(format!("{step}: {kind}"))
                .or_insert((0, format!("{steps:?}")));
            entry.0 += 1;
        }
    }
    for (kind, (count, first)) in &kinds {
        let first: String = first.chars().take(330).collect();
        println!("{count:5} x {kind}\n        first: {first}");
    }
    println!("{} kinds of leak", kinds.len());
}
