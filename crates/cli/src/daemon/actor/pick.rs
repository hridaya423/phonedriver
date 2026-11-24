//! `tap` with `select`: opens a date or time picker, sets it, reads the value back from the picker and confirms
//! (decision D122). Each kind below was set and read back on the API 37 emulator against the testbed's event log
//! (`docs/spikes/pickers.md`); a picker of another kind is refused with what is on screen, and nothing is confirmed.

use super::*;

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

const WEEKDAYS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

/// A calendar is stepped a month at a time, and each step is a tap that settles. Further away than this the year
/// is quicker to choose by hand first.
const MONTH_STEPS: i32 = 36;

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

/// The date a calendar's day cell names: `18 December 2026` (the platform's) or `Friday, December 18, 2026`
/// (Compose Material 3). A month heading such as `December 2026` names no day and is none, and so is a line that
/// says more than a date: Compose heads its calendar with `Current selection: Thursday, January 15, 2026`, which
/// read as the first day cell and was tapped in place of the day (API 37 emulator).
fn cell_date(label: &str) -> Option<(i32, u32, u32)> {
    let (mut year, mut month, mut day) = (None, None, None);
    for word in label.split(|c: char| !c.is_alphanumeric()) {
        let lower = word.to_lowercase();
        if let Some(m) = MONTHS.iter().position(|m| *m == lower) {
            month = Some(m as u32 + 1);
        } else if let Ok(n) = word.parse::<u32>() {
            match word.len() {
                4 => year = Some(n as i32),
                1 | 2 => day = Some(n),
                _ => return None,
            }
        } else if !word.is_empty() && !WEEKDAYS.contains(&lower.as_str()) {
            return None;
        }
    }
    Some((year?, month?, day?))
}

fn texts(e: &Element) -> impl Iterator<Item = &str> {
    [e.label.as_deref(), e.value.as_deref()]
        .into_iter()
        .flatten()
}

fn says(e: &Element, text: &str) -> bool {
    texts(e).any(|t| t.eq_ignore_ascii_case(text))
}

fn starts(e: &Element, text: &str) -> bool {
    texts(e).any(|t| t.to_lowercase().starts_with(text))
}

fn chosen(e: &Element) -> bool {
    e.state
        .intersects(ElementState::SELECTED | ElementState::CHECKED)
}

/// The number a time picker's chip shows: `Hour 7 o'clock`, `Minute 45 minutes`.
fn chip(now: &Rendered, name: &str) -> Option<u32> {
    now.elements
        .iter()
        .filter(|e| e.role != Role::TextField)
        .flat_map(texts)
        .find_map(|t| {
            t.to_lowercase()
                .strip_prefix(name)?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}

fn fields(now: &Rendered) -> Vec<&Element> {
    now.elements
        .iter()
        .filter(|e| e.role == Role::TextField)
        .collect()
}

fn not_picked(detail: impl Into<String>) -> Error {
    Error::NotPicked {
        detail: detail.into(),
    }
}

/// The hour and minute a time picker in text input shows, and whether it shows the wanted half of the day.
fn time_shown(now: &Rendered, half: Option<&str>) -> (Option<u32>, Option<u32>, bool) {
    let typed = |e: &&Element| e.value.as_deref().and_then(|v| v.parse().ok());
    let boxes = fields(now);
    let (hour, minute) = match boxes.as_slice() {
        [hour, minute] => (typed(hour), typed(minute)),
        _ => (chip(now, "hour "), chip(now, "minute ")),
    };
    let half = half.is_none_or(|half| {
        now.elements.iter().any(|e| {
            says(e, half) && (e.role == Role::Menu || (e.role == Role::Radio && chosen(e)))
        })
    });
    (hour, minute, half)
}

impl Actor {
    /// The tap that opens the picker, then the picker. `sub` makes the parameters of each tap and text this sends,
    /// with the caller's approval and time limit, so every one of them is guarded and settled as its own action is.
    pub(super) async fn pick(
        &mut self,
        s: &mut Session,
        open: ActParams,
        want: Want,
        sub: impl Fn(Action) -> ActParams,
    ) -> Result<ActionResult, Error> {
        let before = s.last.clone();
        let opened = Box::pin(self.act_now(s, open)).await?;
        match want {
            Want::Date { year, month, day } => self.pick_date(s, (year, month, day), &sub).await?,
            Want::Time { hour, minute } => self.pick_time(s, hour, minute, &sub).await?,
        }
        let now = self.fresh(s).await?;
        let confirm = now
            .elements
            .iter()
            .find(|e| {
                e.role == Role::Button && ["ok", "set", "done"].iter().any(|word| says(e, word))
            })
            .map(|e| e.r#ref);
        let Some(confirm) = confirm else {
            let mut left = opened;
            left.summary = format!(
                "{}, then set the picker to {want} · no OK button on it",
                left.summary
            );
            left.verified = Some(true);
            left.changed = true;
            return Ok(left);
        };
        s.set_last(before);
        let mut done = Box::pin(self.act_now(s, sub(tap(confirm)))).await?;
        done.summary = format!(
            "{}, set the picker to {want}, then {}",
            opened.summary, done.summary
        );
        done.verified = Some(true);
        done.changed = true;
        Ok(done)
    }

    async fn pick_date(
        &mut self,
        s: &mut Session,
        want: (i32, u32, u32),
        sub: &impl Fn(Action) -> ActParams,
    ) -> Result<(), Error> {
        let months = |(year, month, _): (i32, u32, u32)| year * 12 + month as i32;
        let mut stepped = None;
        for _ in 0..MONTH_STEPS + 3 {
            let now = self.fresh(s).await?;
            // Compose keeps the months beside the shown one in the tree, off screen (API 37 emulator: after `Change
            // to next month` the first dated cell was still January's).
            let screen = self.screen_rect();
            let cells: Vec<(&Element, (i32, u32, u32))> = now
                .elements
                .iter()
                .filter(|e| {
                    let at = e.bounds.center();
                    (screen.x..screen.x + screen.w).contains(&at.x)
                        && (screen.y..screen.y + screen.h).contains(&at.y)
                })
                .filter_map(|e| texts(e).find_map(cell_date).map(|date| (e, date)))
                .collect();
            let Some(&(_, shown)) = cells.first() else {
                return Err(not_picked(
                    "no calendar with dated days is on screen: a date picker with wheels, or in another language than English, is set by hand",
                ));
            };
            if let Some((cell, _)) = cells.iter().find(|(_, date)| *date == want) {
                if chosen(cell) {
                    return Ok(());
                }
                Box::pin(self.act_now(s, sub(tap(cell.r#ref)))).await?;
                continue;
            }
            let away = months(want) - months(shown);
            let name = MONTHS[shown.1 as usize - 1];
            let month = format!("{}{} {}", name[..1].to_uppercase(), &name[1..], shown.0);
            if away.abs() > MONTH_STEPS {
                return Err(not_picked(format!(
                    "the calendar shows {month}, and {}-{:02} is more than {MONTH_STEPS} months from it: tap the year at the top and choose it, then `select` again",
                    want.0, want.1
                )));
            }
            if away == 0 || stepped == Some(months(shown)) {
                return Err(not_picked(format!(
                    "the calendar shows {month} and did not move to {}-{:02}-{:02}",
                    want.0, want.1, want.2
                )));
            }
            stepped = Some(months(shown));
            let way = if away > 0 {
                "next month"
            } else {
                "previous month"
            };
            let step = now
                .elements
                .iter()
                .find(|e| texts(e).any(|t| t.to_lowercase().ends_with(way)))
                .ok_or_else(|| {
                    not_picked("the calendar on screen has no button to another month")
                })?;
            Box::pin(self.act_now(s, sub(tap(step.r#ref)))).await?;
        }
        Err(not_picked(
            "the calendar did not select the day that was tapped",
        ))
    }

    async fn pick_time(
        &mut self,
        s: &mut Session,
        hour: u32,
        minute: u32,
        sub: &impl Fn(Action) -> ActParams,
    ) -> Result<(), Error> {
        let typing = |now: &Rendered| {
            now.elements
                .iter()
                .any(|e| starts(e, "switch to clock mode"))
        };
        let mut now = self.fresh(s).await?;
        if !typing(&now) {
            let mode = now
                .elements
                .iter()
                .find(|e| starts(e, "switch to text input mode"))
                .map(|e| e.r#ref);
            if let Some(mode) = mode {
                Box::pin(self.act_now(s, sub(tap(mode)))).await?;
                now = self.fresh(s).await?;
            }
        }
        if !typing(&now) {
            return Err(not_picked(
                "no time picker that takes typed digits is on screen: one with wheels, a Compose clock, or one in another language than English, is set by hand",
            ));
        }
        let halves = now.elements.iter().any(|e| says(e, "AM") || says(e, "PM"));
        let half = halves.then_some(if hour >= 12 { "PM" } else { "AM" });
        let shown_hour = match (halves, hour % 12) {
            (true, 0) => 12,
            (true, h) => h,
            (false, _) => hour,
        };
        let Some(first) = fields(&now).first().map(|e| e.r#ref) else {
            return Err(not_picked(
                "the time picker showed no field to type the hour into",
            ));
        };
        self.type_digits(s, first, shown_hour.to_string(), sub)
            .await?;
        now = self.fresh(s).await?;
        let second = match fields(&now).as_slice() {
            [_, minute] => minute.r#ref,
            _ => {
                let chip = now
                    .elements
                    .iter()
                    .find(|e| e.role != Role::TextField && starts(e, "minute "))
                    .ok_or_else(|| {
                        not_picked("the time picker showed no field to type the minute into")
                    })?;
                Box::pin(self.act_now(s, sub(tap(chip.r#ref)))).await?;
                now = self.fresh(s).await?;
                fields(&now).first().map(|e| e.r#ref).ok_or_else(|| {
                    not_picked("the time picker showed no field to type the minute into")
                })?
            }
        };
        self.type_digits(s, second, format!("{minute:02}"), sub)
            .await?;
        now = self.fresh(s).await?;
        if let Some(half) = half
            && !time_shown(&now, Some(half)).2
        {
            let other = if half == "PM" { "AM" } else { "PM" };
            let radio = now
                .elements
                .iter()
                .find(|e| e.role == Role::Radio && says(e, half));
            let menu = now
                .elements
                .iter()
                .find(|e| e.role == Role::Menu && says(e, other));
            match (radio, menu) {
                (Some(radio), _) => {
                    Box::pin(self.act_now(s, sub(tap(radio.r#ref)))).await?;
                }
                (None, Some(menu)) => {
                    Box::pin(self.act_now(s, sub(tap(menu.r#ref)))).await?;
                    let open = self.fresh(s).await?;
                    let item = open
                        .elements
                        .iter()
                        .find(|e| e.role != Role::Menu && says(e, half))
                        .ok_or_else(|| not_picked("the AM and PM menu did not open"))?;
                    Box::pin(self.act_now(s, sub(tap(item.r#ref)))).await?;
                }
                (None, None) => {
                    return Err(not_picked(
                        "the time picker shows AM or PM and no control to change it",
                    ));
                }
            }
            now = self.fresh(s).await?;
        }
        let (h, m, half_ok) = time_shown(&now, half);
        if h != Some(shown_hour) || m != Some(minute) || !half_ok {
            let part = |n: Option<u32>| n.map_or("?".to_string(), |n| format!("{n:02}"));
            return Err(not_picked(format!(
                "the time picker shows {}:{}{}, not {shown_hour:02}:{minute:02}{}",
                part(h),
                part(m),
                if half_ok {
                    ""
                } else {
                    " in the other half of the day"
                },
                half.map(|h| format!(" {h}")).unwrap_or_default()
            )));
        }
        Ok(())
    }

    /// Types `digits` into a field of a time picker and holds it to what the field then shows.
    async fn type_digits(
        &mut self,
        s: &mut Session,
        field: Ref,
        digits: String,
        sub: &impl Fn(Action) -> ActParams,
    ) -> Result<(), Error> {
        let typing = |field: Ref| Action::Type {
            text: digits.clone(),
            into: Some(Target::Ref { r#ref: field }),
            mode: None,
            secret: false,
            submit: false,
            append: false,
        };
        let typed = Box::pin(self.act_now(s, sub(typing(field)))).await?;
        if typed.verified == Some(true) {
            return Ok(());
        }
        // Observed on the API 37 emulator, a web page's time picker: the hour field took `12`, the dialog moved as
        // the keyboard came up, and the minute typed right after did not land (`@e585 shows nothing`, and the app
        // logged 00:05 for 00:30). Typed again after a fresh read, 9 of 9 landed.
        let now = self.fresh(s).await?;
        let again = now
            .index_of(field)
            .map(|_| field)
            .or_else(|| fields(&now).last().map(|e| e.r#ref))
            .ok_or_else(|| not_picked("the time picker's field left the screen"))?;
        let typed = Box::pin(self.act_now(s, sub(typing(again)))).await?;
        if typed.verified == Some(true) {
            return Ok(());
        }
        Err(not_picked(format!(
            "the time picker's field did not take `{digits}`{}",
            typed
                .unverified
                .map(|u| format!(": {}", u.shows))
                .unwrap_or_default()
        )))
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn a_day_cell_names_its_date_and_a_month_heading_names_none() {
        assert_eq!(cell_date("18 December 2026"), Some((2026, 12, 18)));
        assert_eq!(cell_date("Friday, December 18, 2026"), Some((2026, 12, 18)));
        assert_eq!(cell_date("03 November 2025"), Some((2025, 11, 3)));
        assert_eq!(cell_date("December 2026"), None);
        assert_eq!(cell_date("2026"), None);
        assert_eq!(cell_date("Fri, Dec 18"), None);
        assert_eq!(cell_date("18 décembre 2026"), None);
        assert_eq!(
            cell_date("Current selection: Thursday, January 15, 2026"),
            None
        );
        assert_eq!(cell_date("Entered date: Thursday, January 15, 2026"), None);
    }
}
