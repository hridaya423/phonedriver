//! `tap` with `select`: opens a date or time picker, sets it, reads the value back from the picker and confirms
//! (decisions D122 and D125). Each kind below was set and read back on the API 37 emulator against the testbed's
//! event log (`docs/spikes/pickers.md`); a picker of another kind is refused with what is on screen, and nothing
//! is confirmed. No word of a language is in this file: months and the halves of the day are named by the device
//! (`Device::calendar_names`), and the controls are found by their ids or by where they are.

use phonedriver_core::calendar::{CalendarNames, digit, fold};

use super::*;

/// A calendar is stepped a month at a time, and each step is a tap that settles. A date further away than a year
/// goes through the picker's list of years first; this bounds the steps when a picker has no such list.
const MONTH_STEPS: i32 = 36;

/// How many pages of a list of years are scrolled before giving up (the platform's list holds 1900 to 2100, eight
/// to a page).
const YEAR_PAGES: usize = 60;

/// How often a wheel's column or a clock's minute is set again when the picker does not show it after the first
/// time. Observed on the API 37 emulator: the first text typed into a wheel after its dialog opened did not land
/// (`@e51 shows "Jan"` for `Dec`), and the second did.
const TRIES: usize = 3;

/// A wheel of an iPhone's picker moves one value when the row above or below its middle is tapped; the rows are
/// this far apart (iOS 26.5 Simulator: a tap 34 points under the middle of the day wheel took 15 to 16, and the
/// app logged it).
const ROW: f32 = 34.0;

/// How many such taps a picker gets before it is given up on: a minute is at most 30 away, a year can be further.
const WHEEL_STEPS: usize = 150;

/// What `select` asks for: `2026-12-18` or `19:45`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Want {
    Date { year: i32, month: u32, day: u32 },
    Time { hour: u32, minute: u32 },
}

impl Want {
    pub fn parse(text: &str) -> Result<Want, String> {
        let bad =
            || "`select` is a date as `2026-12-18` or a time as `19:45` (24-hour)".to_string();
        let numbers =
            |sep: char| -> Option<Vec<u32>> { text.split(sep).map(|n| n.parse().ok()).collect() };
        if let Some(n) = numbers('-').filter(|n| n.len() == 3) {
            let (year, month, day) = (n[0] as i32, n[1], n[2]);
            let long = matches!(month, 1 | 3 | 5 | 7 | 8 | 10 | 12);
            let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
            let days = match month {
                2 if leap => 29,
                2 => 28,
                _ if long => 31,
                _ => 30,
            };
            if !(1..=12).contains(&month) || !(1..=days).contains(&day) || text.len() != 10 {
                return Err(bad());
            }
            return Ok(Want::Date { year, month, day });
        }
        match numbers(':').filter(|n| n.len() == 2) {
            Some(n) if n[0] < 24 && n[1] < 60 => Ok(Want::Time {
                hour: n[0],
                minute: n[1],
            }),
            _ => Err(bad()),
        }
    }
}

impl std::fmt::Display for Want {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Want::Date { year, month, day } => write!(f, "{year:04}-{month:02}-{day:02}"),
            Want::Time { hour, minute } => write!(f, "{hour:02}:{minute:02}"),
        }
    }
}

/// The runs of digits in a text, of any script, each with how many digits it has.
fn numbers(text: &str) -> Vec<(u32, usize)> {
    let mut all = Vec::new();
    let mut run: Option<(u32, usize)> = None;
    for c in text.chars() {
        match (digit(c), run) {
            (Some(d), Some((n, len))) => {
                run = Some((n.saturating_mul(10).saturating_add(d), len + 1))
            }
            (Some(d), None) => run = Some((d, 1)),
            (None, Some(done)) => {
                all.push(done);
                run = None;
            }
            (None, None) => {}
        }
    }
    all.extend(run);
    all
}

/// The one number a text holds, when it holds exactly one.
fn number(text: &str) -> Option<u32> {
    match numbers(text).as_slice() {
        [(n, _)] => Some(*n),
        _ => None,
    }
}

/// What a text says of a date: `18 December 2026`, `Friday, December 18, 2026`, `2026年12月18日`,
/// `١٨ ديسمبر ٢٠٢٦` name a day; `Thursday, January 15` (Material Components) names one without its year;
/// `January 2026` names a month.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Said {
    year: Option<i32>,
    month: u32,
    day: Option<u32>,
}

fn said(text: &str, names: &CalendarNames) -> Option<Said> {
    let text = fold(text);
    let mut found: Option<(usize, usize, u32)> = None;
    let mut tie = false;
    for (month, forms) in names.months.iter().enumerate() {
        for form in forms {
            let form = fold(form);
            if form.is_empty() {
                continue;
            }
            for (at, _) in text.match_indices(&form) {
                let before = text[..at].chars().next_back();
                let after = text[at + form.len()..].chars().next();
                let first = form.chars().next().is_some_and(|c| c.is_ascii_digit());
                let last = form.chars().next_back().is_some_and(|c| c.is_ascii_digit());
                // `1月` is in `11月`, and `mar` is in `market`.
                let joined = |edge: Option<char>, digits: bool| {
                    edge.is_some_and(|c| {
                        if digits {
                            c.is_ascii_digit()
                        } else {
                            c.is_alphabetic()
                        }
                    })
                };
                if joined(before, first) || joined(after, last) {
                    continue;
                }
                match found {
                    Some((_, len, m)) if len == form.len() && m != month as u32 + 1 => tie = true,
                    Some((_, len, _)) if len >= form.len() => {}
                    _ => {
                        found = Some((at, form.len(), month as u32 + 1));
                        tie = false;
                    }
                }
            }
        }
    }
    let (at, len, month) = found.filter(|_| !tie)?;
    let rest = format!("{} {}", &text[..at], &text[at + len..]);
    let (mut year, mut day) = (None, None);
    for (n, digits) in numbers(&rest) {
        match digits {
            4 if year.is_none() => year = Some(n as i32),
            1 | 2 if day.is_none() && (1..=31).contains(&n) => day = Some(n),
            _ => return None,
        }
    }
    Some(Said { year, month, day })
}

fn texts(e: &Element) -> impl Iterator<Item = &str> {
    [e.label.as_deref(), e.value.as_deref()]
        .into_iter()
        .flatten()
}

fn says(e: &Element, text: &str) -> bool {
    texts(e).any(|t| t.eq_ignore_ascii_case(text))
}

/// The name of an element's id without its package: `next` of `android:id/next`.
fn id(e: &Element) -> &str {
    e.id.as_deref()
        .map_or("", |id| id.rsplit('/').next().unwrap_or(id))
}

fn chosen(e: &Element) -> bool {
    e.state
        .intersects(ElementState::SELECTED | ElementState::CHECKED)
}

fn taps(e: &Element) -> bool {
    e.actions.contains(Actions::TAP)
}

fn inside(outer: &Rect, at: Point) -> bool {
    (outer.x..outer.x + outer.w).contains(&at.x) && (outer.y..outer.y + outer.h).contains(&at.y)
}

fn at(now: &Rendered, place: Point) -> Option<&Element> {
    now.elements.iter().find(|e| {
        let c = e.bounds.center();
        taps(e) && (c.x - place.x).abs() < 1.5 && (c.y - place.y).abs() < 1.5
    })
}

/// Why wheels were not set, with what they show when they were already moved: an iPhone's wheels take each value
/// as it is set, with nothing to confirm.
fn unreadable(now: &Rendered, scope: Option<Rect>, kind: &str, moved: bool) -> Error {
    let parts = match kind {
        "date" => "a year, a month the device names and a day",
        _ => "an hour, a minute and the halves of the day as the device names them",
    };
    if !moved {
        return not_picked(format!("the {kind} wheels on screen are not {parts}"));
    }
    let shows: Vec<&str> = wheels(now, scope)
        .iter()
        .filter_map(|e| e.value.as_deref())
        .collect();
    not_picked(format!(
        "the {kind} wheels stopped reading as {parts} after they were moved, and are left at {}",
        shows.join(" ")
    ))
}

fn not_picked(detail: impl Into<String>) -> Error {
    Error::NotPicked {
        detail: detail.into(),
    }
}

/// The columns of a picker with wheels, left to right: on Android the text field each `NumberPicker` has
/// (`android:id/numberpicker_input`), on an iPhone the picker wheels themselves. When the element that was tapped
/// holds wheels they are the ones meant (a form with two inline pickers); else every wheel on screen is.
fn wheels(now: &Rendered, scope: Option<Rect>) -> Vec<&Element> {
    let all: Vec<&Element> = now
        .elements
        .iter()
        .filter(|e| {
            let field = e.role == Role::TextField && id(e) == "numberpicker_input";
            let wheel = e.role == Role::Slider
                && e.value.is_some()
                && e.bounds.h > 100.0
                && e.bounds.h > e.bounds.w;
            field || wheel
        })
        .collect();
    let within: Vec<&Element> = all
        .iter()
        .copied()
        .filter(|e| scope.is_some_and(|scope| inside(&scope, e.bounds.center())))
        .collect();
    let mut all = if within.is_empty() { all } else { within };
    all.sort_by(|a, b| a.bounds.x.total_cmp(&b.bounds.x));
    all
}

fn fields(now: &Rendered) -> Vec<&Element> {
    now.elements
        .iter()
        .filter(|e| e.role == Role::TextField && id(e) != "numberpicker_input")
        .collect()
}

/// Which half of the day an element stands for: `Some(true)` is the second.
fn half(e: &Element, names: Option<&CalendarNames>) -> Option<bool> {
    match id(e) {
        "am_label" | "material_clock_period_am_button" => return Some(false),
        "pm_label" | "material_clock_period_pm_button" => return Some(true),
        _ => {}
    }
    texts(e).find_map(|t| names?.half_named(t))
}

/// The other of two halves of the day when only `one` is in the device's words: Compose in German shows `AM`
/// beside `Nachmittags` where the device says `PM`, 48 by 40 points over 48 by 48 (API 37 emulator). It is the
/// one checkable element in the same column or row that touches `one`.
fn beside<'a>(now: &'a Rendered, one: &Element) -> Option<&'a Element> {
    let touches = |a: (f32, f32), b: (f32, f32)| {
        (a.0 + a.1 - b.0).abs() < 3.0 || (b.0 + b.1 - a.0).abs() < 3.0
    };
    let same = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 3.0 && (a.1 - b.1).abs() < 3.0;
    let mut others = now.elements.iter().filter(|e| {
        let (a, b) = (e.bounds, one.bounds);
        let column = same((a.x, a.w), (b.x, b.w)) && touches((a.y, a.h), (b.y, b.h));
        let row = same((a.y, a.h), (b.y, b.h)) && touches((a.x, a.w), (b.x, b.w));
        e.r#ref != one.r#ref
            && taps(e)
            && e.state.contains(ElementState::CHECKABLE)
            && (column || row)
    });
    let other = others.next()?;
    others.next().is_none().then_some(other)
}

/// The days a calendar shows, each with its date, and what else on the screen says a whole date (the heading that
/// names the selection).
struct Calendar<'a> {
    cells: Vec<(&'a Element, (i32, u32, u32))>,
    shown: (i32, u32),
    heads: Vec<(i32, u32, u32)>,
    top: f32,
    height: f32,
}

fn calendar<'a>(now: &'a Rendered, screen: Rect, names: &CalendarNames) -> Option<Calendar<'a>> {
    let all: Vec<(&Element, Said)> = now
        .elements
        .iter()
        .filter(|e| inside(&screen, e.bounds.center()))
        .filter_map(|e| texts(e).find_map(|t| said(t, names)).map(|said| (e, said)))
        .collect();
    let days: Vec<&(&Element, Said)> = all.iter().filter(|(_, said)| said.day.is_some()).collect();
    // A day cell is one of many of one size. Compose heads its calendar with `Current selection: Thursday,
    // January 15, 2026`, which read as the first day cell and was tapped in place of the day (API 37 emulator).
    let mut widths: Vec<f32> = days.iter().map(|(e, _)| e.bounds.w).collect();
    let mut heights: Vec<f32> = days.iter().map(|(e, _)| e.bounds.h).collect();
    widths.sort_by(f32::total_cmp);
    heights.sort_by(f32::total_cmp);
    let (w, h) = (
        *widths.get(widths.len() / 2)?,
        *heights.get(heights.len() / 2)?,
    );
    let cell = |e: &Element| (e.bounds.w - w).abs() <= 3.0 && (e.bounds.h - h).abs() <= 3.0;
    let month_year = |month: u32| {
        all.iter()
            .filter(|(e, _)| !cell(e))
            .find_map(|(_, said)| (said.month == month).then_some(said.year).flatten())
    };
    let cells: Vec<(&Element, (i32, u32, u32))> = days
        .iter()
        .filter(|(e, _)| cell(e))
        .filter_map(|(e, said)| {
            Some((
                *e,
                (
                    said.year.or_else(|| month_year(said.month))?,
                    said.month,
                    said.day?,
                ),
            ))
        })
        .collect();
    if cells.len() < 7 {
        return None;
    }
    let heads = all
        .iter()
        .filter(|(e, _)| !cell(e))
        .filter_map(|(_, said)| Some((said.year?, said.month, said.day?)))
        .collect();
    let first = cells[0].1;
    Some(Calendar {
        top: cells
            .iter()
            .map(|(e, _)| e.bounds.y)
            .fold(f32::MAX, f32::min),
        height: h,
        shown: (first.0, first.1),
        heads,
        cells,
    })
}

/// The buttons that step a calendar a month, as they were found. A picker that names them by id needs no search;
/// one that does not (Compose) has them in the row above its days, and a tap on each tells which way it goes.
#[derive(Default)]
struct Steps {
    forward: Option<Point>,
    back: Option<Point>,
    tried: Vec<Point>,
    trying: Option<(Point, i32, f32)>,
}

/// The numbers round a clock face, when that is what the screen shows: the hours `1` to `12` or `0` to `23`, or
/// the minutes in fives.
struct Face<'a> {
    marks: Vec<(&'a Element, u32)>,
    minutes: bool,
    whole_day: bool,
}

fn face(now: &Rendered) -> Option<Face<'_>> {
    let mut marks: Vec<(&Element, u32)> = now
        .elements
        .iter()
        .filter(|e| taps(e) && e.role != Role::TextField)
        .filter_map(|e| Some((e, e.label.as_deref().and_then(number)?)))
        .collect();
    // The part above the face that shows the chosen hour holds a number too (`9 o'clock`, Material Components),
    // and is several marks large.
    let area = |e: &Element| e.bounds.w * e.bounds.h;
    let mut areas: Vec<f32> = marks.iter().map(|(e, _)| area(e)).collect();
    areas.sort_by(f32::total_cmp);
    let usual = *areas.get(areas.len() / 2)?;
    marks.retain(|(e, _)| area(e) <= usual * 1.6);
    let mut set: Vec<u32> = marks.iter().map(|(_, n)| *n).collect();
    set.sort_unstable();
    set.dedup();
    if set.len() != marks.len() {
        return None;
    }
    let is = |want: &mut dyn Iterator<Item = u32>| want.eq(set.iter().copied());
    let (minutes, whole_day) = if is(&mut (0..12).map(|n| n * 5)) {
        (true, false)
    } else if is(&mut (1..=12)) {
        (false, false)
    } else if is(&mut (0..24)) {
        (false, true)
    } else {
        return None;
    };
    Some(Face {
        marks,
        minutes,
        whole_day,
    })
}

impl Face<'_> {
    fn mark(&self, n: u32) -> Option<&Element> {
        self.marks.iter().find(|(_, m)| *m == n).map(|(e, _)| *e)
    }

    /// Where on the face a minute is, on the circle of its marks; `minute` may lie between two whole minutes.
    fn place(&self, minute: f32) -> Option<Point> {
        let minute = minute.rem_euclid(60.0);
        let base = (minute / 5.0).floor() as u32 * 5;
        let center = |n: u32| self.mark(n % 60).map(|e| e.bounds.center());
        let (a, b) = (center(base)?, center(base + 5)?);
        let count = self.marks.len() as f32;
        let middle = Point {
            x: self
                .marks
                .iter()
                .map(|(e, _)| e.bounds.center().x)
                .sum::<f32>()
                / count,
            y: self
                .marks
                .iter()
                .map(|(e, _)| e.bounds.center().y)
                .sum::<f32>()
                / count,
        };
        let angle = |p: Point| (p.y - middle.y).atan2(p.x - middle.x);
        let radius = |p: Point| (p.x - middle.x).hypot(p.y - middle.y);
        let mut turn = angle(b) - angle(a);
        if turn > std::f32::consts::PI {
            turn -= 2.0 * std::f32::consts::PI;
        } else if turn < -std::f32::consts::PI {
            turn += 2.0 * std::f32::consts::PI;
        }
        let part = (minute - base as f32) / 5.0;
        let (angle, radius) = (angle(a) + turn * part, (radius(a) + radius(b)) / 2.0);
        Some(Point {
            x: middle.x + radius * angle.cos(),
            y: middle.y + radius * angle.sin(),
        })
    }
}

/// The two parts of a time picker that hold the hour and the minute, left to right: text fields, or the chips that
/// show a number and turn into a field when tapped. Material Components keeps the chip under the field it turned
/// into, at the same place (the Clock app's `Add alarm` on the API 37 emulator, 2026-11-26), and there the field
/// is the part.
fn slots<'a>(
    now: &'a Rendered,
    names: Option<&CalendarNames>,
    face: Option<&Face>,
) -> Vec<&'a Element> {
    let mut all: Vec<&Element> = now
        .elements
        .iter()
        .filter(|e| {
            let on_face = face.is_some_and(|f| f.marks.iter().any(|(m, _)| m.r#ref == e.r#ref));
            let field = e.role == Role::TextField && id(e) != "numberpicker_input";
            let chip = e.state.contains(ElementState::CHECKABLE) && half(e, names).is_none();
            !on_face && (field || chip)
        })
        .collect();
    // The chips are the largest of the parts that can be chosen; an AM and PM the device names otherwise is smaller.
    let area = |e: &Element| e.bounds.w * e.bounds.h;
    let largest = all.iter().map(|e| area(e)).fold(0.0, f32::max);
    all.retain(|e| area(e) >= largest * 0.6);
    let chip = |e: &Element| e.role != Role::TextField;
    all.sort_by(|a, b| {
        a.bounds
            .x
            .total_cmp(&b.bounds.x)
            .then(chip(a).cmp(&chip(b)))
    });
    all.dedup_by(|a, b| (a.bounds.x - b.bounds.x).abs() < 2.0);
    all
}

/// The button that confirms a picker: the platform's, Material Components', or one that says so.
fn confirm(now: &Rendered) -> Option<&Element> {
    now.elements
        .iter()
        .find(|e| {
            matches!(
                id(e),
                "button1" | "confirm_button" | "material_timepicker_ok_button"
            )
        })
        .or_else(|| {
            now.elements.iter().find(|e| {
                e.role == Role::Button && ["ok", "set", "done"].iter().any(|word| says(e, word))
            })
        })
}

/// The number a slot shows: what its field holds, what its chip says (`Minute 45 minutes`), or the text drawn in it.
fn slot_number(now: &Rendered, slot: &Element) -> Option<u32> {
    if slot.role == Role::TextField {
        return slot.value.as_deref().and_then(number);
    }
    texts(slot).find_map(number).or_else(|| {
        now.elements
            .iter()
            .filter(|e| e.r#ref != slot.r#ref && inside(&slot.bounds, e.bounds.center()))
            .flat_map(texts)
            .find_map(number)
    })
}

impl Actor {
    /// The tap that opens the picker, then the picker. `sub` makes the parameters of each tap and text this sends,
    /// with the caller's approval and time limit, so every one of them is guarded and settled as its own action is.
    /// A target that is the button confirming a picker already open is not tapped first: agents sent
    /// `{"target": "OK", "select": "07:30"}` to the Clock app's open picker in 5 of 10 calls (2026-11-26), and the
    /// tap confirmed the time the picker happened to show.
    pub(super) async fn pick(
        &mut self,
        s: &mut Session,
        open: ActParams,
        want: Want,
        sub: impl Fn(Action) -> ActParams,
    ) -> Result<ActionResult, Error> {
        let before = s.last.clone();
        let (scope, confirms) = match &open.action {
            Action::Tap { target, .. } => {
                let now = self.fresh(s).await?;
                let tapped = s.resolve(target, &now).ok().and_then(|found| found.element);
                let confirms = tapped
                    .as_ref()
                    .is_some_and(|e| confirm(&now).is_some_and(|c| c.r#ref == e.r#ref));
                (tapped.map(|e| e.bounds), confirms)
            }
            _ => (None, false),
        };
        let (opened, scope) = if confirms {
            (None, None)
        } else {
            (Some(Box::pin(self.act_now(s, open)).await?), scope)
        };
        let lead = opened.as_ref().map_or_else(
            || format!("set the open picker to {want}"),
            |opened| format!("{}, set the picker to {want}", opened.summary),
        );
        let names = self.device()?.calendar_names(None).await.ok().flatten();
        match want {
            Want::Date { year, month, day } => {
                self.pick_date(s, (year, month, day), names.as_ref(), scope, &sub)
                    .await?
            }
            Want::Time { hour, minute } => {
                self.pick_time(s, (hour, minute), names.as_ref(), scope, false, &sub)
                    .await?
            }
        }
        let now = self.fresh(s).await?;
        // An iPhone's picker in a popover takes each value as it is set, and a tap beside the popover closes it.
        // The region that takes that tap covers the screen, popover included: tapped in its middle it turned the
        // month wheel under it, and at 10,860 it did nothing (iOS 26.5 Simulator), so the tap goes to a place on
        // the left edge that nothing else covers.
        if let Some(region) = now
            .elements
            .iter()
            .find(|e| id(e) == "PopoverDismissRegion")
        {
            let free = [0.8, 0.6, 0.4, 0.2]
                .into_iter()
                .map(|part| Point {
                    x: region.bounds.x + 10.0,
                    y: region.bounds.y + region.bounds.h * part,
                })
                .find(|place| {
                    !now.elements
                        .iter()
                        .any(|e| e.r#ref != region.r#ref && inside(&e.bounds, *place))
                });
            if let Some(free) = free {
                s.set_last(before);
                let beside = Action::Tap {
                    target: Target::Point {
                        point: crate::wire::PointJson {
                            x: free.x,
                            y: free.y,
                        },
                    },
                    count: None,
                    long: None,
                    select: None,
                };
                let mut done = Box::pin(self.act_now(s, sub(beside))).await?;
                done.summary = format!("{lead}, then closed it");
                done.verified = Some(true);
                done.changed = true;
                return Ok(done);
            }
        }
        let Some(confirm) = confirm(&now).map(|e| e.r#ref) else {
            let Some(mut left) = opened else {
                return Err(not_picked(
                    "the button that confirms the picker left the screen while the picker was set",
                ));
            };
            left.summary = format!(
                "{}, then set the picker to {want} · it shows no button that confirms it: if it needs one, tap it",
                left.summary
            );
            left.verified = Some(true);
            left.changed = true;
            return Ok(left);
        };
        s.set_last(before.clone());
        let mut done = Box::pin(self.act_now(s, sub(tap(confirm)))).await?;
        // A picker that keeps its button after the tap did not take the value. The platform's time fields in
        // Japanese count a twelve-hour clock from 0: they showed the 12 typed for 00:30, and on `OK` stayed open
        // under `有効な時刻を入力してください` while the reply said the time was set (API 37 emulator). So
        // midnight and noon are typed once more as 0, and anything still open is an error with the screen.
        let open = |now: &Rendered| now.elements.iter().any(|e| e.r#ref == confirm);
        let mut after = self.fresh(s).await?;
        if let (true, Want::Time { hour, minute }) = (open(&after), want)
            && hour % 12 == 0
        {
            self.pick_time(s, (hour, minute), names.as_ref(), scope, true, &sub)
                .await?;
            s.set_last(before);
            done = Box::pin(self.act_now(s, sub(tap(confirm)))).await?;
            after = self.fresh(s).await?;
        }
        if open(&after) {
            return Err(not_picked(format!(
                "the picker shows {want} and stayed open when the button that confirms it was tapped"
            )));
        }
        done.summary = format!("{lead}, then {}", done.summary);
        done.verified = Some(true);
        done.changed = true;
        Ok(done)
    }

    async fn pick_date(
        &mut self,
        s: &mut Session,
        want: (i32, u32, u32),
        names: Option<&CalendarNames>,
        scope: Option<Rect>,
        sub: &impl Fn(Action) -> ActParams,
    ) -> Result<(), Error> {
        let Some(names) = names else {
            return Err(not_picked(
                "this device does not tell the names of its months, so a date picker on it cannot be read",
            ));
        };
        let months = |(year, month): (i32, u32)| year * 12 + month as i32;
        let (mut steps, mut tapped, mut jumped) = (Steps::default(), 0, false);
        for _ in 0..MONTH_STEPS * 2 + 12 {
            let now = self.fresh(s).await?;
            if wheels(&now, scope).len() >= 2 && calendar(&now, self.screen_rect(), names).is_none()
            {
                let want = (want.0, want.1, Some(want.2));
                return self.wheel_date(s, want, names, scope, sub).await;
            }
            let Some(cal) = calendar(&now, self.screen_rect(), names) else {
                // The button that was tried opened something else (the list of years, the text input); it closes it.
                // Compose's button to its text input sits lower once the calendar is gone (German, API 37
                // emulator), so it is found again in its column.
                if let Some((place, _, width)) = steps.trying.take() {
                    steps.tried.push(place);
                    let again = now
                        .elements
                        .iter()
                        .filter(|e| taps(e) && e.role != Role::TextField)
                        .filter(|e| {
                            (e.bounds.center().x - place.x).abs() < 1.5
                                && (e.bounds.w - width).abs() < 1.5
                        })
                        .min_by(|a, b| {
                            (a.bounds.center().y - place.y)
                                .abs()
                                .total_cmp(&(b.bounds.center().y - place.y).abs())
                        });
                    if let Some(e) = again {
                        steps.tried.push(e.bounds.center());
                        Box::pin(self.act_now(s, sub(tap(e.r#ref)))).await?;
                        continue;
                    }
                }
                return Err(not_picked(
                    "no calendar with dated days and no date wheels are on screen",
                ));
            };
            if let Some((place, before, _)) = steps.trying.take() {
                match months(cal.shown) - before {
                    moved if moved > 0 => steps.forward = Some(place),
                    moved if moved < 0 => steps.back = Some(place),
                    _ => {}
                }
                steps.tried.push(place);
            }
            if let Some((cell, _)) = cal.cells.iter().find(|(_, date)| *date == want) {
                // Material Components marks no day as selected; its heading names the selection.
                if chosen(cell) || (tapped > 0 && cal.heads.contains(&want)) {
                    return Ok(());
                }
                if tapped == TRIES {
                    return Err(not_picked(
                        "the calendar did not select the day that was tapped",
                    ));
                }
                tapped += 1;
                Box::pin(self.act_now(s, sub(tap(cell.r#ref)))).await?;
                continue;
            }
            let away = months((want.0, want.1)) - months(cal.shown);
            if away.abs() > 12 && !jumped {
                jumped = true;
                self.jump_year(s, &now, &cal, (want.0, want.1), names, sub)
                    .await?;
                continue;
            }
            let shown = format!("{:04}-{:02}", cal.shown.0, cal.shown.1);
            if away == 0 {
                return Err(not_picked(format!(
                    "the calendar shows {shown} without the day {}",
                    want.2
                )));
            }
            if away.abs() > MONTH_STEPS {
                return Err(not_picked(format!(
                    "the calendar shows {shown}, {}-{:02} is more than {MONTH_STEPS} months from it, and no list of years opened: choose the year by hand, then `select` again",
                    want.0, want.1
                )));
            }
            let named = |ids: [&str; 3], end: &str| {
                now.elements
                    .iter()
                    .find(|e| ids.contains(&id(e)))
                    .or_else(|| {
                        now.elements
                            .iter()
                            .find(|e| texts(e).any(|t| t.to_lowercase().ends_with(end)))
                    })
                    .map(|e| e.bounds.center())
            };
            let known = if away > 0 {
                named(
                    ["next", "month_navigation_next", "DatePicker.NextMonth"],
                    "next month",
                )
                .or(steps.forward)
            } else {
                named(
                    [
                        "prev",
                        "month_navigation_previous",
                        "DatePicker.PreviousMonth",
                    ],
                    "previous month",
                )
                .or(steps.back)
            };
            let place = match known {
                Some(place) => place,
                None => {
                    // The row above the days holds the two arrows (and the button that opens the years); nothing
                    // in it confirms or cancels.
                    let row = Rect {
                        x: 0.0,
                        y: cal.top - cal.height * 3.0,
                        w: self.screen_rect().w,
                        h: cal.height * 3.0,
                    };
                    let mut row: Vec<&Element> = now
                        .elements
                        .iter()
                        .filter(|e| taps(e) && e.role != Role::TextField)
                        .filter(|e| inside(&row, e.bounds.center()))
                        .filter(|e| {
                            let c = e.bounds.center();
                            !steps
                                .tried
                                .iter()
                                .any(|t| (t.x - c.x).abs() < 1.5 && (t.y - c.y).abs() < 1.5)
                        })
                        .collect();
                    // The arrows are the nearest to the days; a button that names a number opens the years.
                    row.sort_by(|a, b| {
                        let numbered = |e: &Element| texts(e).any(|t| !numbers(t).is_empty());
                        numbered(a)
                            .cmp(&numbered(b))
                            .then(b.bounds.center().y.total_cmp(&a.bounds.center().y))
                    });
                    let Some(next) = row.first() else {
                        return Err(not_picked(format!(
                            "the calendar shows {shown} and no button on it moves it {}",
                            if away > 0 { "forward" } else { "back" }
                        )));
                    };
                    steps.trying = Some((next.bounds.center(), months(cal.shown), next.bounds.w));
                    next.bounds.center()
                }
            };
            let Some(step) = at(&now, place) else {
                return Err(not_picked(format!(
                    "the calendar shows {shown} and its button to another month is gone"
                )));
            };
            Box::pin(self.act_now(s, sub(tap(step.r#ref)))).await?;
        }
        Err(not_picked("the calendar did not reach the month"))
    }

    /// Opens the picker's list of years and taps `year` in it. `false` when the picker has no such list.
    async fn jump_year(
        &mut self,
        s: &mut Session,
        now: &Rendered,
        cal: &Calendar<'_>,
        (year, month): (i32, u32),
        names: &CalendarNames,
        sub: &impl Fn(Action) -> ActParams,
    ) -> Result<bool, Error> {
        let shows_year = |e: &Element| {
            // `January 2026`, `2026年1月` (Compose in Japanese), or the year alone.
            texts(e).any(|t| match said(t, names) {
                Some(said) => said.day.is_none() && said.year == Some(cal.shown.0),
                None => numbers(t).as_slice() == [(cal.shown.0 as u32, 4)],
            })
        };
        let opener = now
            .elements
            .iter()
            .find(|e| {
                matches!(
                    id(e),
                    "date_picker_header_year"
                        | "month_navigation_fragment_toggle"
                        | "DatePicker.Show"
                )
            })
            .or_else(|| {
                now.elements
                    .iter()
                    .find(|e| taps(e) && e.bounds.center().y < cal.top && shows_year(e))
            });
        let Some(opener) = opener else {
            return Ok(false);
        };
        let place = opener.bounds.center();
        Box::pin(self.act_now(s, sub(tap(opener.r#ref)))).await?;
        let mut seen: Vec<u32> = Vec::new();
        for page in 0..YEAR_PAGES {
            let now = self.fresh(s).await?;
            // An iPhone's calendar opens a wheel of months and one of years in place of a list, and the same
            // button closes them (`DatePicker.Show`, `DatePicker.Hide`, iOS 26.5 Simulator).
            if page == 0 && wheels(&now, None).len() >= 2 {
                self.wheel_date(s, (year, month, None), names, None, sub)
                    .await?;
                let now = self.fresh(s).await?;
                let close = now
                    .elements
                    .iter()
                    .find(|e| id(e) == "DatePicker.Hide")
                    .or_else(|| at(&now, place))
                    .map(|e| e.r#ref);
                if let Some(close) = close {
                    Box::pin(self.act_now(s, sub(tap(close)))).await?;
                }
                return Ok(true);
            }
            let years: Vec<(&Element, u32)> = now
                .elements
                .iter()
                .filter(|e| taps(e) && e.bounds.center() != place)
                .filter(|e| texts(e).all(|t| said(t, names).is_none()))
                .filter_map(|e| match texts(e).map(numbers).next() {
                    Some(all) if all.len() == 1 && all[0].1 == 4 => Some((e, all[0].0)),
                    _ => None,
                })
                .collect();
            if years.len() < 3 {
                return Ok(false);
            }
            if let Some((e, _)) = years.iter().find(|(_, y)| *y as i32 == year) {
                Box::pin(self.act_now(s, sub(tap(e.r#ref)))).await?;
                return Ok(true);
            }
            let shown: Vec<u32> = years.iter().map(|(_, y)| *y).collect();
            if shown == seen {
                return Err(not_picked(format!(
                    "the picker's list of years ends at {} and does not reach {year}",
                    if (year as u32) < shown[0] {
                        shown[0]
                    } else {
                        shown[shown.len() - 1]
                    }
                )));
            }
            let direction = if (year as u32) < *shown.iter().min().unwrap_or(&0) {
                Direction::Up
            } else {
                Direction::Down
            };
            seen = shown;
            Box::pin(self.act_now(
                s,
                sub(Action::Scroll {
                    direction,
                    within: None,
                    until: None,
                    max: None,
                    collect: None,
                }),
            ))
            .await?;
        }
        Err(not_picked(format!(
            "the picker's list of years did not reach {year}"
        )))
    }

    /// Moves one column of a picker with wheels: types `text` into it and enters it (Android, where the field
    /// holds the text as typed until it loses the keyboard), or taps the row beside its middle (an iPhone).
    async fn turn(
        &mut self,
        s: &mut Session,
        wheel: (Ref, bool, Point),
        text: String,
        forward: bool,
        sub: &impl Fn(Action) -> ActParams,
    ) -> Result<(), Error> {
        let (field, typed, middle) = wheel;
        let action = if typed {
            Action::Type {
                text,
                into: Some(Target::Ref { r#ref: field }),
                mode: None,
                secret: false,
                submit: true,
                append: false,
            }
        } else {
            Action::Tap {
                target: Target::Point {
                    point: crate::wire::PointJson {
                        x: middle.x,
                        y: middle.y + if forward { ROW } else { -ROW },
                    },
                },
                count: None,
                long: None,
                select: None,
            }
        };
        Box::pin(self.act_now(s, sub(action))).await?;
        Ok(())
    }

    /// A wheel that stops saying a number on the way is moved once more the same way, up to three times: an
    /// iPhone in Arabic says its minute wheel in words at one and two (`دقيقة واحدة`, iOS 26.5 Simulator), and the
    /// wheels are held to what they show at the end either way.
    async fn turn_again(
        &mut self,
        s: &mut Session,
        last: &Option<((Ref, bool, Point), String, bool)>,
        blind: &mut usize,
        sub: &impl Fn(Action) -> ActParams,
    ) -> Result<bool, Error> {
        match last {
            Some((wheel, text, forward)) if !wheel.1 && *blind < TRIES => {
                *blind += 1;
                self.turn(s, *wheel, text.clone(), *forward, sub).await?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Sets the wheels of a date picker, and holds them all to one read of the picker; `day` is `None` for the
    /// month and year wheels an iPhone's calendar opens. A wheel moves its neighbours: typed from `Jan` to `Dec`
    /// it counted as one month back and took the year to 2030 for 2031-12-31, which was confirmed and logged by
    /// the app (API 37 emulator, first live run). So a column that is off is set again, round after round, until
    /// a read shows the whole date.
    async fn wheel_date(
        &mut self,
        s: &mut Session,
        want: (i32, u32, Option<u32>),
        names: &CalendarNames,
        scope: Option<Rect>,
        sub: &impl Fn(Action) -> ActParams,
    ) -> Result<(), Error> {
        #[derive(Clone, Copy, PartialEq)]
        enum Part {
            Month,
            Day,
            Year,
        }
        let mut shows: String;
        let (mut round, mut left) = (0, None);
        let mut last: Option<((Ref, bool, Point), String, bool)> = None;
        let mut blind = 0;
        let (mut day_at, mut probed): (Option<f32>, Option<f32>) = (None, None);
        loop {
            let now = self.fresh(s).await?;
            let mut columns: Vec<(Part, &Element, u32, usize)> = Vec::new();
            for wheel in wheels(&now, scope) {
                let value = wheel.value.as_deref().unwrap_or_default();
                let form = names.months.iter().enumerate().find_map(|(m, forms)| {
                    let at = forms.iter().position(|f| fold(f) == fold(value))?;
                    Some((m as u32 + 1, at))
                });
                match (form, numbers(value).as_slice()) {
                    (Some((month, at)), _) => columns.push((Part::Month, wheel, month, at)),
                    (None, [(n, 4)]) => columns.push((Part::Year, wheel, *n, 0)),
                    (None, [(n, 1 | 2)]) => columns.push((Part::Day, wheel, *n, 0)),
                    _ => {}
                }
            }
            // In Japanese the month wheel shows a number as the day wheel does (`2026`, `1`, `15`, API 37
            // emulator). Only a day can be over 12, so the wheel that shows such a number is the day's, and when
            // neither does, 13 is typed into one: the day's takes it.
            let small: Vec<usize> = (0..columns.len())
                .filter(|i| columns[*i].0 == Part::Day)
                .collect();
            if let (false, [a, b]) = (columns.iter().any(|c| c.0 == Part::Month), small.as_slice())
            {
                let x = |i: usize| columns[i].1.bounds.center().x;
                let is = |i: usize, at: f32| (x(i) - at).abs() < 1.5;
                let over: Vec<usize> = small
                    .iter()
                    .copied()
                    .filter(|i| columns[*i].2 > 12)
                    .collect();
                let day = match (day_at, over.as_slice(), probed) {
                    (Some(at), _, _) => small.iter().copied().find(|i| is(*i, at)),
                    (None, [day], _) => Some(*day),
                    (None, _, Some(at)) => small.iter().copied().find(|i| !is(*i, at)),
                    (None, _, None) => None,
                };
                match day {
                    Some(day) => {
                        day_at = Some(x(day));
                        let month = if day == *a { *b } else { *a };
                        columns[month].0 = Part::Month;
                        columns[month].3 = usize::MAX;
                    }
                    None if columns[*a].1.role == Role::TextField => {
                        probed = Some(x(*a));
                        let wheel = (columns[*a].1.r#ref, true, columns[*a].1.bounds.center());
                        self.turn(s, wheel, "13".to_string(), true, sub).await?;
                        continue;
                    }
                    None => {}
                }
            }
            let one = |part: Part| {
                let mut of = columns.iter().filter(|c| c.0 == part);
                of.next().filter(|_| of.next().is_none()).copied()
            };
            let (Some(month), Some(year), day) =
                (one(Part::Month), one(Part::Year), one(Part::Day))
            else {
                if self.turn_again(s, &last, &mut blind, sub).await? {
                    continue;
                }
                return Err(unreadable(&now, scope, "date", last.is_some()));
            };
            if want.2.is_some() && day.is_none() {
                if self.turn_again(s, &last, &mut blind, sub).await? {
                    continue;
                }
                return Err(unreadable(&now, scope, "date", last.is_some()));
            }
            blind = 0;
            let day = day.filter(|_| want.2.is_some());
            let off = |part: Part| match part {
                Part::Year => year.2 as i32 != want.0,
                Part::Month => month.2 != want.1,
                Part::Day => day.map(|d| d.2) != want.2,
            };
            if !off(Part::Year) && !off(Part::Month) && !off(Part::Day) {
                return Ok(());
            }
            shows = format!(
                "{:04}-{:02}{}",
                year.2,
                month.2,
                day.map(|d| format!("-{:02}", d.2)).unwrap_or_default()
            );
            let typed = year.1.role == Role::TextField;
            let left = left.get_or_insert(if typed { TRIES * 3 } else { WHEEL_STEPS });
            if *left == 0 {
                break;
            }
            *left -= 1;
            let part = if typed {
                round += 1;
                [Part::Month, Part::Day, Part::Year][(round - 1) % 3]
            } else {
                [Part::Year, Part::Month, Part::Day]
                    .into_iter()
                    .find(|part| off(*part))
                    .unwrap_or(Part::Day)
            };
            if !off(part) {
                continue;
            }
            let (column, text, forward) = match (part, day) {
                (Part::Year, _) => (year, want.0.to_string(), want.0 > year.2 as i32),
                (Part::Month, _) => (
                    month,
                    names.months[want.1 as usize - 1]
                        .get(month.3)
                        .cloned()
                        .unwrap_or_else(|| want.1.to_string()),
                    (want.1 + 12 - month.2) % 12 <= 6,
                ),
                (Part::Day, Some(day)) => {
                    let to = want.2.unwrap_or(day.2);
                    (day, to.to_string(), to > day.2)
                }
                (Part::Day, None) => continue,
            };
            let wheel = (column.1.r#ref, typed, column.1.bounds.center());
            last = Some((wheel, text.clone(), forward));
            self.turn(s, wheel, text, forward, sub).await?;
        }
        Err(not_picked(format!(
            "the date wheels show {shows} and did not take the date"
        )))
    }

    /// Sets the wheels of a time picker the same way. The hour is the column on the left of the minute; the column
    /// that holds the device's own word for a half of the day is the third. These wheels move each other too: the
    /// hour typed from 11 to 12 changes the half of the day.
    async fn wheel_time(
        &mut self,
        s: &mut Session,
        hour: u32,
        minute: u32,
        names: Option<&CalendarNames>,
        scope: Option<Rect>,
        sub: &impl Fn(Action) -> ActParams,
    ) -> Result<(), Error> {
        let mut shows: String;
        let (mut round, mut left) = (0, None);
        let mut last: Option<((Ref, bool, Point), String, bool)> = None;
        let mut blind = 0;
        let (mut typed_twelve, mut from_zero) = (false, false);
        loop {
            let now = self.fresh(s).await?;
            let all = wheels(&now, scope);
            let halves: Vec<&Element> = all
                .iter()
                .copied()
                .filter(|e| half(e, names).is_some())
                .collect();
            let digits: Vec<&Element> = all
                .iter()
                .copied()
                .filter(|e| e.value.as_deref().and_then(number).is_some())
                .collect();
            let ([hours, minutes], true) = (
                digits.as_slice(),
                halves.len() + 2 == all.len() && halves.len() <= 1,
            ) else {
                if self.turn_again(s, &last, &mut blind, sub).await? {
                    continue;
                }
                return Err(unreadable(&now, scope, "time", last.is_some()));
            };
            blind = 0;
            let read = |e: &Element| e.value.as_deref().and_then(number);
            // In Japanese the wheel of a twelve-hour clock runs from 0 to 11 and kept its 9 when 12 was typed
            // (API 37 emulator), so midnight and noon are its 0.
            if std::mem::take(&mut typed_twelve) && read(hours) != Some(12) {
                from_zero = true;
            }
            let (span, shown_hour) = match (halves.is_empty(), hour % 12) {
                (false, 0) if from_zero || read(hours) == Some(0) => (12, 0),
                (false, 0) => (12, 12),
                (false, h) => (12, h),
                (true, _) => (24, hour),
            };
            let half_off = halves
                .first()
                .is_some_and(|e| half(e, names) != Some(hour >= 12));
            let off = [
                read(hours) != Some(shown_hour),
                read(minutes) != Some(minute),
                half_off,
            ];
            if off == [false; 3] {
                return Ok(());
            }
            shows = all
                .iter()
                .map(|e| e.value.as_deref().unwrap_or_default())
                .collect::<Vec<_>>()
                .join(" ");
            let typed = hours.role == Role::TextField;
            let left = left.get_or_insert(if typed { TRIES * 3 } else { WHEEL_STEPS });
            if *left == 0 {
                break;
            }
            *left -= 1;
            let part = if typed {
                round += 1;
                (round - 1) % 3
            } else {
                off.iter().position(|off| *off).unwrap_or(0)
            };
            if !off[part] {
                continue;
            }
            let ahead = |to: u32, at: Option<u32>, span: u32| {
                (to + span - at.unwrap_or(0) % span) % span <= span / 2
            };
            let (column, text, forward) = match (part, halves.first(), names) {
                (0, _, _) => (
                    *hours,
                    shown_hour.to_string(),
                    ahead(shown_hour % span, read(hours), span),
                ),
                (1, _, _) => (
                    *minutes,
                    format!("{minute:02}"),
                    ahead(minute, read(minutes), 60),
                ),
                (_, Some(halves), Some(names)) => {
                    let value = halves.value.as_deref().unwrap_or_default();
                    let (from, to) = if hour >= 12 {
                        (&names.am, &names.pm)
                    } else {
                        (&names.pm, &names.am)
                    };
                    let form = from.iter().position(|f| fold(f) == fold(value));
                    (
                        *halves,
                        to.get(form.unwrap_or(0)).cloned().unwrap_or_default(),
                        hour >= 12,
                    )
                }
                _ => continue,
            };
            let wheel = (column.r#ref, typed, column.bounds.center());
            typed_twelve = typed && part == 0 && shown_hour == 12 && !halves.is_empty();
            last = Some((wheel, text.clone(), forward));
            self.turn(s, wheel, text, forward, sub).await?;
        }
        Err(not_picked(format!(
            "the time wheels show {shows} and did not take the time"
        )))
    }

    async fn pick_time(
        &mut self,
        s: &mut Session,
        (hour, minute): (u32, u32),
        names: Option<&CalendarNames>,
        scope: Option<Rect>,
        from_zero: bool,
        sub: &impl Fn(Action) -> ActParams,
    ) -> Result<(), Error> {
        let mut now = self.fresh(s).await?;
        if wheels(&now, scope).len() >= 2 {
            return self.wheel_time(s, hour, minute, names, scope, sub).await;
        }
        // The platform's picker and Material Components' keep one button for both ways in, under one id; a picker
        // shows text fields only once it is in its text input.
        let mode = |now: &Rendered| {
            now.elements
                .iter()
                .find(|e| matches!(id(e), "toggle_mode" | "material_timepicker_mode_button"))
                .or_else(|| {
                    now.elements.iter().find(|e| {
                        texts(e).any(|t| {
                            let t = t.to_lowercase();
                            t.starts_with("switch to text input mode")
                                || t.starts_with("switch to clock mode")
                        })
                    })
                })
                .map(|e| e.r#ref)
        };
        if let (true, Some(mode)) = (fields(&now).is_empty(), mode(&now)) {
            Box::pin(self.act_now(s, sub(tap(mode)))).await?;
            now = self.fresh(s).await?;
        }
        if mode(&now).is_none() || fields(&now).is_empty() {
            if face(&now).is_some() {
                return self.clock_time(s, hour, minute, names, sub).await;
            }
            return Err(not_picked(
                "no time picker is on screen that takes typed digits, has wheels, or has a clock face with its hours on it",
            ));
        }
        let halves = now.elements.iter().any(|e| half(e, names).is_some());
        let shown_hour = match (halves, hour % 12) {
            (true, 0) if from_zero => 0,
            (true, 0) => 12,
            (true, h) => h,
            (false, _) => hour,
        };
        // Observed on the API 37 emulator, a web page's time picker: the hour field took `12`, the dialog moved as
        // the keyboard came up, and the minute typed right after did not land (the app logged 00:05 for 00:30).
        // Typed again after a fresh read it landed, so the pair is typed again when the picker does not show it.
        let mut shows = (None, None);
        for _ in 0..TRIES {
            for (part, digits) in [shown_hour.to_string(), format!("{minute:02}")]
                .into_iter()
                .enumerate()
            {
                now = self.fresh(s).await?;
                let all = slots(&now, names, None);
                let Some(slot) = all.get(part).filter(|_| all.len() == 2) else {
                    return Err(not_picked(
                        "the time picker does not show an hour and a minute to type into",
                    ));
                };
                let field = if slot.role == Role::TextField {
                    slot.r#ref
                } else {
                    Box::pin(self.act_now(s, sub(tap(slot.r#ref)))).await?;
                    now = self.fresh(s).await?;
                    let all = slots(&now, names, None);
                    all.get(part)
                        .filter(|slot| all.len() == 2 && slot.role == Role::TextField)
                        .map(|slot| slot.r#ref)
                        .ok_or_else(|| not_picked("the time picker showed no field to type into"))?
                };
                let digits = names.map_or(digits.clone(), |names| names.written(&digits));
                Box::pin(self.act_now(s, sub(typing_plain(field, digits)))).await?;
            }
            if halves {
                self.pick_half(s, hour >= 12, names, sub).await?;
            }
            shows = self.typed_time(s, names, sub).await?;
            if shows == (Some(shown_hour), Some(minute)) {
                return Ok(());
            }
        }
        let part = |n: Option<u32>| n.map_or("?".to_string(), |n| format!("{n:02}"));
        Err(not_picked(format!(
            "the time picker shows {}:{}, not {shown_hour:02}:{minute:02}",
            part(shows.0),
            part(shows.1),
        )))
    }

    /// The hour and the minute a time picker in its text input shows. Material Components shows one of the two as
    /// a field that reads `Hour` whatever it holds, and as a chip with its number once the other is tapped.
    async fn typed_time(
        &mut self,
        s: &mut Session,
        names: Option<&CalendarNames>,
        sub: &impl Fn(Action) -> ActParams,
    ) -> Result<(Option<u32>, Option<u32>), Error> {
        let now = self.fresh(s).await?;
        let all = slots(&now, names, None);
        let [hours, minutes] = all.as_slice() else {
            return Ok((None, None));
        };
        let mut shows = (slot_number(&now, hours), slot_number(&now, minutes));
        let other = match shows {
            (None, Some(_)) if minutes.role != Role::TextField => Some(minutes.r#ref),
            (Some(_), None) if hours.role != Role::TextField => Some(hours.r#ref),
            _ => None,
        };
        if let Some(other) = other {
            Box::pin(self.act_now(s, sub(tap(other)))).await?;
            let now = self.fresh(s).await?;
            if let [hours, minutes] = slots(&now, names, None).as_slice() {
                shows.0 = shows.0.or_else(|| slot_number(&now, hours));
                shows.1 = shows.1.or_else(|| slot_number(&now, minutes));
            }
        }
        Ok(shows)
    }

    /// Chooses the half of the day on a picker that shows both, or that shows one in a menu, and holds it to what
    /// the picker then shows.
    async fn pick_half(
        &mut self,
        s: &mut Session,
        second: bool,
        names: Option<&CalendarNames>,
        sub: &impl Fn(Action) -> ActParams,
    ) -> Result<(), Error> {
        for attempt in 0..2 {
            let now = self.fresh(s).await?;
            let named: Vec<&Element> = now
                .elements
                .iter()
                .filter(|e| half(e, names).is_some())
                .collect();
            let menu = named.iter().find(|e| e.role == Role::Menu);
            let wanted = named
                .iter()
                .find(|e| e.role != Role::Menu && half(e, names) == Some(second))
                .copied()
                .or_else(|| match named.as_slice() {
                    [one] if menu.is_none() => beside(&now, one),
                    _ => None,
                });
            let shown = match (menu, wanted) {
                (Some(menu), _) => half(menu, names) == Some(second),
                (None, Some(wanted)) => chosen(wanted),
                (None, None) => false,
            };
            if shown {
                return Ok(());
            }
            if attempt == 1 {
                break;
            }
            match (menu, wanted) {
                (Some(menu), _) => {
                    Box::pin(self.act_now(s, sub(tap(menu.r#ref)))).await?;
                    let open = self.fresh(s).await?;
                    let item = open
                        .elements
                        .iter()
                        .find(|e| e.role != Role::Menu && half(e, names) == Some(second))
                        .ok_or_else(|| {
                            not_picked("the menu of the halves of the day did not open")
                        })?;
                    Box::pin(self.act_now(s, sub(tap(item.r#ref)))).await?;
                }
                (None, Some(wanted)) => {
                    Box::pin(self.act_now(s, sub(tap(wanted.r#ref)))).await?;
                }
                (None, None) => break,
            }
        }
        Err(not_picked(
            "the time picker did not take the half of the day",
        ))
    }

    /// Sets a time picker that has only its clock face (Compose Material 3): the hour's mark, then the minute's
    /// mark or the place between two marks, each read back from the picker's own display of the hour and minute.
    async fn clock_time(
        &mut self,
        s: &mut Session,
        hour: u32,
        minute: u32,
        names: Option<&CalendarNames>,
        sub: &impl Fn(Action) -> ActParams,
    ) -> Result<(), Error> {
        let no_face = || not_picked("the clock face left the screen");
        let no_chips = || {
            not_picked(
                "the clock does not show its hour and its minute as two parts to choose between",
            )
        };
        let mut now = self.fresh(s).await?;
        if face(&now).ok_or_else(no_face)?.minutes {
            let chips = slots(&now, names, face(&now).as_ref());
            let hours = chips
                .first()
                .filter(|_| chips.len() == 2)
                .ok_or_else(no_chips)?;
            Box::pin(self.act_now(s, sub(tap(hours.r#ref)))).await?;
            now = self.fresh(s).await?;
        }
        let hours = face(&now).filter(|f| !f.minutes).ok_or_else(no_face)?;
        let halves = now.elements.iter().any(|e| half(e, names).is_some());
        if !hours.whole_day && !halves {
            return Err(not_picked(
                "the clock has twelve hours and nothing on it is named as the device names the halves of the day",
            ));
        }
        let shown_hour = match (hours.whole_day, hour % 12) {
            (true, _) => hour,
            (false, 0) => 12,
            (false, h) => h,
        };
        let whole_day = hours.whole_day;
        let mark = hours.mark(shown_hour).ok_or_else(no_face)?.r#ref;
        Box::pin(self.act_now(s, sub(tap(mark)))).await?;
        // The face turns from hours to minutes by itself, and for a moment it shows the hours' numbers as minutes
        // (`1 minutes` to `12 minutes`, Compose on the API 37 emulator).
        for _ in 0..TRIES {
            now = self.fresh(s).await?;
            if face(&now).is_some_and(|f| f.minutes) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        if !face(&now).ok_or_else(no_face)?.minutes {
            let chips = slots(&now, names, face(&now).as_ref());
            let minutes = chips
                .get(1)
                .filter(|_| chips.len() == 2)
                .ok_or_else(no_chips)?;
            Box::pin(self.act_now(s, sub(tap(minutes.r#ref)))).await?;
        }
        // Observed on the API 37 emulator (Compose, round 1 of the spike): a tap on `45 minutes` sent while the
        // face was still turning from hours to minutes was reported and chose nothing, 07:05 for 07:45.
        let mut shows = None;
        let (mut aim, mut gain) = (minute as f32, 0.5);
        for _ in 0..=TRIES + 2 {
            now = self.fresh(s).await?;
            let minutes = face(&now).filter(|f| f.minutes).ok_or_else(no_face)?;
            let chips = slots(&now, names, Some(&minutes));
            let chip = chips
                .get(1)
                .filter(|_| chips.len() == 2)
                .ok_or_else(no_chips)?;
            let before = shows;
            shows = slot_number(&now, chip);
            if shows == Some(minute) {
                break;
            }
            // A tap between two marks goes to the nearer mark (07:05 for 07:07), and a finger drawn only as far as
            // from a mark to its neighbour is taken as a tap. A finger drawn from a quarter of the face away
            // leaves the hand where it lifts; it landed a minute short four times of four (46 for 47, 58 for 59),
            // and aimed a whole minute on it landed a minute long (48), so the place aimed at moves by half of what
            // the picker was off by, then a quarter (Compose on the API 37 emulator).
            if let (Some(_), Some(shows)) = (before, shows) {
                let off = (minute as f32 - shows as f32 + 30.0).rem_euclid(60.0) - 30.0;
                aim += off * gain;
                gain /= 2.0;
            }
            let action = match (
                minutes.mark(minute),
                minutes.mark((minute / 5 * 5 + 45) % 60),
            ) {
                (Some(mark), _) => tap(mark.r#ref),
                (None, Some(from)) => {
                    let place = minutes.place(aim).ok_or_else(no_face)?;
                    Action::Drag {
                        from: Target::Ref { r#ref: from.r#ref },
                        to: Target::Point {
                            point: crate::wire::PointJson {
                                x: place.x,
                                y: place.y,
                            },
                        },
                    }
                }
                (None, None) => return Err(no_face()),
            };
            Box::pin(self.act_now(s, sub(action))).await?;
        }
        now = self.fresh(s).await?;
        let (mut shows_hour, hours) = {
            let on = face(&now).ok_or_else(no_face)?;
            let chips = slots(&now, names, Some(&on));
            let [hours, minutes] = chips.as_slice() else {
                return Err(no_chips());
            };
            shows = slot_number(&now, minutes).or(shows);
            (slot_number(&now, hours), hours.r#ref)
        };
        if shows == Some(minute) && shows_hour.is_none() {
            // The part that is not chosen says its number only once it is: look at the hour again.
            Box::pin(self.act_now(s, sub(tap(hours)))).await?;
            let now = self.fresh(s).await?;
            let on = face(&now).ok_or_else(no_face)?;
            shows_hour = slots(&now, names, Some(&on))
                .first()
                .and_then(|chip| slot_number(&now, chip));
        }
        if shows != Some(minute) || shows_hour != Some(shown_hour) {
            let part = |n: Option<u32>| n.map_or("?".to_string(), |n| format!("{n:02}"));
            return Err(not_picked(format!(
                "the clock shows {}:{}, not {shown_hour:02}:{minute:02}",
                part(shows_hour),
                part(shows),
            )));
        }
        if !whole_day {
            self.pick_half(s, hour >= 12, names, sub).await?;
        }
        Ok(())
    }
}

fn tap(r#ref: Ref) -> Action {
    Action::Tap {
        target: Target::Ref { r#ref },
        count: None,
        long: None,
        select: None,
    }
}

fn typing_plain(field: Ref, text: String) -> Action {
    Action::Type {
        text,
        into: Some(Target::Ref { r#ref: field }),
        mode: None,
        secret: false,
        submit: false,
        append: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phonedriver_core::testing::english_names;

    #[test]
    fn select_takes_a_date_or_a_24_hour_time_and_nothing_else() {
        assert_eq!(
            Want::parse("2026-12-18"),
            Ok(Want::Date {
                year: 2026,
                month: 12,
                day: 18
            })
        );
        assert_eq!(Want::parse("07:05"), Ok(Want::Time { hour: 7, minute: 5 }));
        assert_eq!(Want::parse("2028-02-29").unwrap().to_string(), "2028-02-29");
        assert_eq!(Want::parse("0:00").unwrap().to_string(), "00:00");
        for bad in [
            "2026-02-29",
            "2026-13-01",
            "2026-1-5",
            "24:00",
            "7:60",
            "7pm",
            "tomorrow",
            "",
            "18/12/2026",
        ] {
            let why = Want::parse(bad).unwrap_err();
            assert!(why.contains("2026-12-18") && why.contains("19:45"), "{why}");
        }
    }

    fn date(said: Option<Said>) -> Option<(Option<i32>, u32, Option<u32>)> {
        said.map(|s| (s.year, s.month, s.day))
    }

    #[test]
    fn a_text_names_a_day_a_day_without_its_year_or_a_month() {
        let en = english_names();
        let full = |y, m, d| Some((Some(y), m, Some(d)));
        assert_eq!(date(said("18 December 2026", &en)), full(2026, 12, 18));
        assert_eq!(
            date(said("Friday, December 18, 2026", &en)),
            full(2026, 12, 18)
        );
        assert_eq!(date(said("03 November 2025", &en)), full(2025, 11, 3));
        assert_eq!(
            date(said("Current selection: Jan 15, 2026", &en)),
            full(2026, 1, 15)
        );
        assert_eq!(
            date(said("Thursday, January 15", &en)),
            Some((None, 1, Some(15)))
        );
        assert_eq!(
            date(said("December 2026", &en)),
            Some((Some(2026), 12, None))
        );
        assert_eq!(said("2026", &en), None);
        assert_eq!(said("18 décembre 2026", &en), None);
        assert_eq!(said("Maybe 18, 2026", &en), None);
        assert_eq!(said("March 18, 2026 at 10:30", &en), None);
    }

    fn names(months: [&str; 12]) -> CalendarNames {
        CalendarNames {
            locale: String::new(),
            months: months.iter().map(|m| vec![m.to_string()]).collect(),
            am: vec![],
            pm: vec![],
            digits: String::new(),
        }
    }

    #[test]
    fn a_date_is_read_in_the_devices_own_month_names_and_digits() {
        let ja = names([
            "1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月",
        ]);
        assert_eq!(
            date(said("2026年12月18日", &ja)),
            Some((Some(2026), 12, Some(18)))
        );
        assert_eq!(
            date(said("2026年1月11日", &ja)),
            Some((Some(2026), 1, Some(11)))
        );
        assert_eq!(
            date(said("2026年11月1日", &ja)),
            Some((Some(2026), 11, Some(1)))
        );
        let ar = names([
            "يناير",
            "فبراير",
            "مارس",
            "أبريل",
            "مايو",
            "يونيو",
            "يوليو",
            "أغسطس",
            "سبتمبر",
            "أكتوبر",
            "نوفمبر",
            "ديسمبر",
        ]);
        assert_eq!(
            date(said("١٨ ديسمبر ٢٠٢٦", &ar)),
            Some((Some(2026), 12, Some(18)))
        );
        let de = names([
            "Januar",
            "Februar",
            "März",
            "April",
            "Mai",
            "Juni",
            "Juli",
            "August",
            "September",
            "Oktober",
            "November",
            "Dezember",
        ]);
        assert_eq!(
            date(said("Freitag, 18. Dezember 2026", &de)),
            Some((Some(2026), 12, Some(18)))
        );
        assert_eq!(said("18 December 2026", &de), None);
    }

    #[test]
    fn numbers_are_read_in_any_script() {
        assert_eq!(numbers("7 o'clock"), [(7, 1)]);
        assert_eq!(number("Minute 45 minutes"), Some(45));
        assert_eq!(number("٤٥"), Some(45));
        assert_eq!(number("05"), Some(5));
        assert_eq!(number("10:30"), None);
        assert_eq!(number("AM"), None);
    }
}
