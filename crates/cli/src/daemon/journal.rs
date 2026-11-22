//! What a session has done, kept as steps that can be run again with no model (decision D60).

use serde_json::{Value, json};

use super::session::Session;
use crate::wire::{Action, ActionResult, OnScreen, PointJson, Target, WaitCondition};

impl OnScreen {
    /// The screen the agent last saw, for an action aimed at it. A launch, a link, back and home leave whatever is
    /// in front, so they keep none.
    fn of(action: &Action, s: &Session) -> Option<OnScreen> {
        let leaves = match action {
            Action::Launch { .. } | Action::Open { .. } => true,
            Action::Press { key } => matches!(key.as_str(), "back" | "home"),
            _ => false,
        };
        let last = s.last.as_ref().filter(|_| !leaves && !s.unsure)?;
        Some(OnScreen {
            app: last.header.app.as_ref().map(|a| a.id.clone()),
            over: last.dialog_in_front(),
        })
    }
}

/// A session keeps this many steps and says so when more were done: a run that long is not one to replay blind.
pub const MAX: usize = 500;

#[derive(Default)]
pub struct Journal {
    steps: Vec<Value>,
    over: bool,
}

impl Journal {
    pub fn push(&mut self, step: Value) {
        if self.steps.len() < MAX {
            self.steps.push(step);
        } else {
            self.over = true;
        }
    }

    /// The steps so far, and whether more were done than are kept. The journal starts again empty.
    pub fn take(&mut self) -> Value {
        json!({"steps": std::mem::take(&mut self.steps), "over": std::mem::take(&mut self.over)})
    }

    /// The same as `take`, leaving the steps in the journal.
    pub fn peek(&self) -> Value {
        json!({"steps": self.steps, "over": self.over})
    }

    /// Removes the first `n` steps, the ones a client has stored after a `peek`.
    pub fn drop_first(&mut self, n: usize) {
        self.steps.drain(..n.min(self.steps.len()));
        self.over = false;
    }
}

pub fn call(method: &str, params: Value) -> Value {
    json!({"method": method, "params": params})
}

/// A step only a person can do again, with what it was.
pub fn manual(what: &str) -> Value {
    json!({"manual": what})
}

/// An action about to run, in the form it is kept in: every `@ref` replaced by what finds the same element in a
/// later run (`lasting`): its id, its text and role, or its centre (`by_position`). A typed secret is kept without
/// its text: `save` names the environment variable a replay reads it from.
pub struct Kept {
    action: Action,
    by_position: bool,
    secret: bool,
    /// The action before this one failed, so the screen it is measured against may not be the device's, and
    /// whether it changed the screen says nothing a replay can hold it to.
    unsure: bool,
    /// The screen the action was saved on (`OnScreen`), for an action aimed at that screen.
    on: Option<OnScreen>,
}

impl Kept {
    pub fn of(action: &Action, s: &Session) -> Kept {
        let secret = matches!(action, Action::Type { secret: true, .. });
        let on = OnScreen::of(action, s);
        let mut action = action.clone();
        if let Action::Type { text, .. } = &mut action
            && secret
        {
            text.clear();
        }
        let mut by_position = false;
        let mut keep = |t: &mut Target| *t = lasting(t, s, &mut by_position);
        match &mut action {
            Action::Tap { target, .. } | Action::Clear { target } => keep(target),
            Action::Type { into, .. } => into.iter_mut().for_each(keep),
            Action::Swipe { from, .. } => from.iter_mut().for_each(keep),
            Action::Pinch { at, .. } => at.iter_mut().for_each(keep),
            Action::Drag { from, to } => {
                keep(from);
                keep(to);
            }
            Action::Scroll { within, until, .. } => {
                within.iter_mut().for_each(&mut keep);
                until.iter_mut().for_each(keep);
            }
            Action::Gesture { .. } => by_position = true,
            Action::Launch { .. } | Action::Open { .. } | Action::Press { .. } => {}
        }
        Kept {
            action,
            by_position,
            secret,
            unsure: s.unsure,
            on,
        }
    }

    /// The steps for a `scroll --until` that failed after moving the list `times` times: a step after it may need
    /// the list where the scrolls left it, so they are kept as plain scrolls (issue #41). Each may move nothing in a
    /// replay that starts nearer the list's end, so none says it changed the screen.
    pub fn scrolled(&self, times: u32) -> Vec<Value> {
        let Action::Scroll {
            direction, within, ..
        } = &self.action
        else {
            return Vec::new();
        };
        let plain = Action::Scroll {
            direction: *direction,
            within: within.clone(),
            until: None,
            max: None,
        };
        (0..times)
            .map(|_| {
                let mut step = call("act", json!({"action": plain}));
                step["summary"] = json!(format!("scrolled {}", direction.name()));
                if let Some(on) = &self.on {
                    step["on"] = json!(on);
                }
                if self.by_position {
                    step["by_position"] = json!(true);
                }
                step
            })
            .collect()
    }

    /// The step for this action once it has worked. `changed` lets a replay tell a step that did nothing this time.
    pub fn step(self, result: &ActionResult) -> Value {
        let mut step = call("act", json!({"action": self.action}));
        if self.secret {
            step["params"]["action"]
                .as_object_mut()
                .map(|a| a.remove("text"));
        }
        step["summary"] = json!(result.summary);
        if !self.unsure {
            step["changed"] = json!(result.changed);
        }
        if self.by_position {
            step["by_position"] = json!(true);
        }
        if let Some(on) = &self.on {
            step["on"] = json!(on);
        }
        step
    }
}

/// The step for a wait about to run; its `@ref` is replaced as an action's is.
pub fn wait(condition: &WaitCondition, s: &Session) -> Value {
    let mut by_position = false;
    let condition = match condition {
        WaitCondition::Settled(v) => WaitCondition::Settled(*v),
        WaitCondition::Appears(t) => WaitCondition::Appears(lasting(t, s, &mut by_position)),
        WaitCondition::Gone(t) => WaitCondition::Gone(lasting(t, s, &mut by_position)),
        WaitCondition::Vpn(v) => WaitCondition::Vpn(*v),
        WaitCondition::Log(l) => WaitCondition::Log(l.clone()),
    };
    call("wait", json!({"condition": condition}))
}

/// What finds the element of `target` in a later run, best first: an id the app gave it (with its text beside it,
/// for a run where the id is gone), its label or a hint with its role, and last its centre. Each is used only when
/// it singles the element out on the screen the agent saw. `android:id/…` names a slot every app has (`button1`
/// is the confirming button of any dialog), not a thing, so it is never used. A text field the last screen dropped
/// (a keyboard that opened shrank its list) is named by its label, which a replay brings back as `type` does live.
fn lasting(target: &Target, s: &Session, by_position: &mut bool) -> Target {
    let r = match target {
        Target::Text { .. } | Target::Id { .. } => return target.clone(),
        Target::Point { .. } => {
            *by_position = true;
            return target.clone();
        }
        Target::Ref { r#ref } => *r#ref,
    };
    let Some((last, element)) = s
        .last
        .as_ref()
        .and_then(|last| Some((last, &last.elements[last.index_of(r)?])))
    else {
        return match s.gone_field(r) {
            Some(gone) => Target::Text {
                text: gone.label.clone().unwrap_or_default(),
                role: Some(gone.role),
            },
            None => target.clone(),
        };
    };
    let finds_it = |named: &Target| {
        s.resolve(named, last)
            .is_ok_and(|found| found.element.is_some_and(|e| e.r#ref == r))
    };
    let named = element
        .label
        .as_deref()
        .into_iter()
        .chain(
            element
                .hint
                .as_deref()
                .into_iter()
                .flat_map(|h| h.split(" · ")),
        )
        .filter(|text| !text.trim().is_empty())
        .map(|text| (text.to_string(), Some(element.role)))
        .find(|(text, role)| {
            finds_it(&Target::Text {
                text: text.clone(),
                role: *role,
            })
        });
    let id = element
        .id
        .clone()
        .filter(|id| !id.starts_with("android:id/"))
        .filter(|id| {
            finds_it(&Target::Id {
                id: id.clone(),
                text: None,
                role: None,
            })
        });
    match (id, named) {
        (Some(id), named) => {
            let (text, role) = named.unzip();
            Target::Id {
                id,
                text,
                role: role.flatten(),
            }
        }
        (None, Some((text, role))) => Target::Text { text, role },
        (None, None) => {
            *by_position = true;
            Target::Point {
                point: PointJson {
                    x: element.bounds.x + element.bounds.w / 2.0,
                    y: element.bounds.y + element.bounds.h / 2.0,
                },
            }
        }
    }
}
