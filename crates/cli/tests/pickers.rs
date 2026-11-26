#[path = "support/fakes.rs"]
mod fakes;

use std::sync::{Arc, Mutex};

use fakes::OneDevice;
use phonedriver::daemon::{Config, Daemon};
use phonedriver_core::calendar::CalendarNames;
use phonedriver_core::device::TextInput;
use phonedriver_core::geometry::{Point, Scale};
use phonedriver_core::raw::RawTree;
use phonedriver_core::testing::{FakeDevice, english_names};
use serde_json::{Value, json};

#[derive(Clone)]
struct Node {
    key: String,
    class: &'static str,
    text: String,
    value: String,
    id: &'static str,
    at: [f32; 4],
    flags: String,
    tap: bool,
}

fn node(key: impl Into<String>, class: &'static str, at: [f32; 4]) -> Node {
    let key = key.into();
    Node {
        text: key.clone(),
        value: String::new(),
        key,
        class,
        id: "",
        at: [at[0], at[1], at[0] + at[2], at[1] + at[3]],
        flags: String::new(),
        tap: true,
    }
}

const BUTTON: &str = "android.widget.Button";
const TEXT: &str = "android.widget.TextView";
const FIELD: &str = "android.widget.EditText";

impl Node {
    fn id(mut self, id: &'static str) -> Node {
        self.id = id;
        self
    }
    fn text(mut self, text: impl Into<String>) -> Node {
        self.text = text.into();
        self
    }
    fn value(mut self, value: impl Into<String>) -> Node {
        self.value = value.into();
        self
    }
    fn flags(mut self, flags: &str) -> Node {
        self.flags = flags.into();
        self
    }
    fn inert(mut self) -> Node {
        self.tap = false;
        self
    }
}

fn drawn(nodes: &[Node]) -> RawTree {
    let mut all = vec![
        json!({"p": -1, "w": 1, "cls": "android.widget.FrameLayout", "b": [0, 0, 360, 800], "f": "vVe"}),
    ];
    for n in nodes {
        let mut one = json!({"p": 0, "w": 1, "cls": n.class, "text": n.text, "b": n.at, "f": format!("vVe{}", n.flags)});
        if n.tap {
            one["a"] = json!(["tap"]);
        }
        if !n.id.is_empty() {
            one["id"] = json!(n.id);
        }
        all.push(one);
    }
    let whole = json!({
        "windows": [{"id": 1, "type": "application", "package": "com.example", "bounds": [0, 0, 360, 800]}],
        "nodes": all,
    });
    RawTree::from_android_json(&whole.to_string(), Scale { density: 1.0 }).unwrap()
}

fn drawn_ios(nodes: &[Node]) -> RawTree {
    let mut all = vec![
        json!({"p": -1, "cls": "XCUIElementTypeApplication", "b": [0, 0, 360, 800], "f": "vVe"}),
    ];
    for n in nodes {
        let mut one = json!({"p": 0, "cls": n.class, "text": n.text, "b": n.at, "f": format!("vVe{}", n.flags)});
        if !n.value.is_empty() {
            one["sd"] = json!(n.value);
        }
        if n.class != WHEEL {
            one["a"] = json!(["tap"]);
        }
        if !n.id.is_empty() {
            one["id"] = json!(n.id);
        }
        all.push(one);
    }
    let mut tree = RawTree::from_ios_json(&json!({"nodes": all}).to_string()).unwrap();
    tree.windows.push(phonedriver_core::raw::RawWindow {
        id: 0,
        kind: "application".into(),
        package: Some("com.example".into()),
        layer: 0,
        bounds: phonedriver_core::geometry::Rect {
            x: 0.0,
            y: 0.0,
            w: 360.0,
            h: 800.0,
        },
    });
    tree
}

trait World: Send + 'static {
    const IPHONE: bool = false;
    fn draw(&self) -> Vec<Node>;
    fn tap(&mut self, key: &str);
    fn touch(&mut self, _at: Point) -> bool {
        false
    }
    fn tree(&self) -> RawTree {
        if Self::IPHONE {
            drawn_ios(&self.draw())
        } else {
            drawn(&self.draw())
        }
    }
    fn text(&mut self, _text: &str) {}
    fn drag(&mut self, _from: Point, _to: Point) {}
}

struct Rig<W> {
    daemon: Arc<Daemon>,
    world: Arc<Mutex<W>>,
    taps: Arc<Mutex<Vec<String>>>,
    fake: Arc<FakeDevice>,
    _home: tempfile::TempDir,
}

fn picker<W: World>(world: W) -> Rig<W> {
    let world = Arc::new(Mutex::new(world));
    let taps = Arc::new(Mutex::new(Vec::new()));
    let fake = Arc::new(FakeDevice::new(vec![world.lock().unwrap().tree()]));
    let (w, log) = (world.clone(), taps.clone());
    fake.on_gesture(move |g| {
        let path = g.fingers.first()?;
        let (from, to) = (path.first()?.at, path.last()?.at);
        let mut world = w.lock().unwrap();
        if (from.x - to.x).hypot(from.y - to.y) < 4.0 && world.touch(from) {
            log.lock().unwrap().push("wheel".into());
        } else if (from.x - to.x).hypot(from.y - to.y) < 4.0 {
            let hit = world.draw().into_iter().rev().find(|n| {
                n.tap
                    && (n.at[0]..n.at[2]).contains(&from.x)
                    && (n.at[1]..n.at[3]).contains(&from.y)
            });
            match hit {
                Some(hit) => {
                    log.lock().unwrap().push(hit.key.clone());
                    world.tap(&hit.key);
                }
                None => log.lock().unwrap().push(format!("{},{}", from.x, from.y)),
            }
        } else {
            log.lock().unwrap().push("drag".into());
            world.drag(from, to);
        }
        Some(vec![world.tree()])
    });
    let w = world.clone();
    fake.on_text(move |t| {
        let (TextInput::Paste(text) | TextInput::SetValue(text) | TextInput::Keystrokes(text)) = t
        else {
            return None;
        };
        let mut world = w.lock().unwrap();
        world.text(text);
        Some(vec![world.tree()])
    });
    let home = tempfile::tempdir().unwrap();
    let daemon = Daemon::new(
        Arc::new(OneDevice(fake.clone())),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    Rig {
        daemon,
        world,
        taps,
        fake,
        _home: home,
    }
}

impl<W: World> Rig<W> {
    async fn select(&self, target: &str, value: &str) -> Result<Value, String> {
        self.daemon.call("s", "snapshot", json!({})).await.unwrap();
        self.taps.lock().unwrap().clear();
        let action = json!({"kind": "tap", "target": {"text": target}, "select": value});
        self.daemon
            .call("s", "act", json!({"action": action}))
            .await
            .map_err(|e| format!("{e:?}"))
    }

    fn taps(&self) -> Vec<String> {
        self.taps.lock().unwrap().clone()
    }
}

fn days_in(year: i32, month: u32) -> u32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn names(locale: &str, months: [&str; 12], am: &str, pm: &str) -> CalendarNames {
    CalendarNames {
        locale: locale.into(),
        months: months
            .iter()
            .map(|m| {
                let short: String = m.chars().take(3).collect();
                vec![m.to_string(), m.to_string(), short.clone(), short]
            })
            .collect(),
        am: vec![am.into()],
        pm: vec![pm.into()],
        digits: String::new(),
    }
}

fn german() -> CalendarNames {
    names(
        "de-DE",
        [
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
        ],
        "AM",
        "PM",
    )
}

fn japanese() -> CalendarNames {
    names(
        "ja-JP",
        [
            "1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月",
        ],
        "午前",
        "午後",
    )
}

#[derive(Clone)]
struct Style {
    names: CalendarNames,
    cell: fn(&CalendarNames, i32, u32, u32) -> String,
    ids: bool,
    marks: bool,
    arrows: [&'static str; 2],
    confirm: &'static str,
    toggle: Option<f32>,
}

struct Calendar {
    style: Style,
    open: bool,
    years: Option<i32>,
    typing: bool,
    shown: (i32, u32),
    chosen: (i32, u32, u32),
    picked: String,
}

impl Calendar {
    fn new(style: Style) -> Calendar {
        Calendar {
            style,
            open: false,
            years: None,
            typing: false,
            shown: (2026, 1),
            chosen: (2026, 1, 15),
            picked: "nothing picked".into(),
        }
    }
}

impl World for Calendar {
    fn draw(&self) -> Vec<Node> {
        if !self.open {
            return vec![
                node("Date", BUTTON, [0.0, 60.0, 180.0, 48.0]),
                node("picked", TEXT, [0.0, 120.0, 360.0, 40.0])
                    .text(&self.picked)
                    .inert(),
            ];
        }
        let s = &self.style;
        let (year, month) = self.shown;
        let (cy, cm, cd) = self.chosen;
        let month_name = &s.names.months[month as usize - 1][0];
        let mut all = vec![
            node("heading", TEXT, [20.0, 150.0, 300.0, 40.0])
                .text(format!(
                    "Auswahl: {cd}. {} {cy}",
                    s.names.months[cm as usize - 1][2]
                ))
                .inert(),
            node("year", BUTTON, [10.0, 240.0, 140.0, 48.0])
                .text(if month_name.ends_with('月') {
                    format!("{year}年{month_name}")
                } else {
                    format!("{month_name} {year}")
                })
                .id(if s.ids {
                    "android:id/date_picker_header_year"
                } else {
                    ""
                }),
        ];
        if let Some(y) = s.toggle {
            let y = if self.typing { y - 60.0 } else { y };
            all.insert(
                0,
                node("toggle", BUTTON, [200.0, y, 48.0, 48.0])
                    .text("In den anderen Eingabemodus wechseln"),
            );
        }
        if self.typing {
            all.truncate(2);
        } else if let Some(top) = self.years {
            for (i, y) in (top..top + 8).enumerate() {
                all.push(
                    node(
                        format!("year {y}"),
                        BUTTON,
                        [40.0, 300.0 + 48.0 * i as f32, 280.0, 48.0],
                    )
                    .text(y.to_string()),
                );
            }
        } else {
            all.push(
                node("back", BUTTON, [250.0, 240.0, 48.0, 48.0])
                    .text(s.arrows[0])
                    .id(if s.ids { "android:id/prev" } else { "" }),
            );
            all.push(
                node("forward", BUTTON, [300.0, 240.0, 48.0, 48.0])
                    .text(s.arrows[1])
                    .id(if s.ids { "android:id/next" } else { "" }),
            );
            for day in 1..=days_in(year, month) {
                let i = day - 1;
                let on = s.marks && (year, month, day) == self.chosen;
                all.push(
                    node(
                        format!("day {day}"),
                        BUTTON,
                        [
                            12.0 + 48.0 * (i % 7) as f32,
                            300.0 + 48.0 * (i / 7) as f32,
                            48.0,
                            48.0,
                        ],
                    )
                    .text((s.cell)(&s.names, year, month, day))
                    .flags(if on { "s" } else { "" }),
                );
            }
        }
        all.push(
            node("Abbrechen", BUTTON, [150.0, 700.0, 100.0, 48.0]).id(if s.ids {
                "android:id/button2"
            } else {
                ""
            }),
        );
        all.push(
            node("confirm", BUTTON, [260.0, 700.0, 90.0, 48.0])
                .text(s.confirm)
                .id(if s.ids { "android:id/button1" } else { "" }),
        );
        all
    }

    fn tap(&mut self, key: &str) {
        let step = |(year, month): (i32, u32), by: i32| {
            let at = year * 12 + month as i32 - 1 + by;
            (at.div_euclid(12), at.rem_euclid(12) as u32 + 1)
        };
        match key.split_once(' ') {
            _ if key == "Date" => self.open = true,
            _ if key == "toggle" => self.typing = !self.typing,
            _ if key == "back" => self.shown = step(self.shown, -1),
            _ if key == "forward" => self.shown = step(self.shown, 1),
            _ if key == "year" => {
                self.years = match self.years {
                    Some(_) => None,
                    None => Some(self.shown.0 - 3),
                }
            }
            _ if key == "confirm" => {
                self.open = false;
                let (y, m, d) = self.chosen;
                self.picked = format!("picked {y:04}-{m:02}-{d:02}");
            }
            Some(("year", y)) => {
                self.shown.0 = y.parse().unwrap();
                self.years = None;
            }
            Some(("day", d)) => self.chosen = (self.shown.0, self.shown.1, d.parse().unwrap()),
            _ => self.open = false,
        }
    }

    fn drag(&mut self, from: Point, to: Point) {
        if let Some(top) = self.years.as_mut() {
            *top += if to.y > from.y { -6 } else { 6 };
        }
    }
}

fn platform(names: CalendarNames) -> Style {
    Style {
        names,
        cell: |n, y, m, d| format!("{d:02} {} {y}", n.months[m as usize - 1][0]),
        ids: true,
        marks: true,
        arrows: ["Vorheriger Monat", "Nächster Monat"],
        confirm: "Fertig",
        toggle: None,
    }
}

#[tokio::test]
async fn a_calendar_in_another_language_is_read_in_the_devices_month_names_and_stepped_by_its_ids()
{
    let rig = picker(Calendar::new(Style {
        cell: |n, y, m, d| format!("Freitag, {d}. {} {y}", n.months[m as usize - 1][0]),
        ..platform(german())
    }));
    rig.fake.answer_calendar_names(Some(german()));
    let r = rig.select("Date", "2026-03-09").await.unwrap();
    assert_eq!(rig.world.lock().unwrap().picked, "picked 2026-03-09", "{r}");
    assert_eq!(
        rig.taps(),
        ["Date", "forward", "forward", "day 9", "confirm"]
    );
    assert!(
        r["summary"]
            .as_str()
            .unwrap()
            .contains("set the picker to 2026-03-09, then tapped"),
        "{r}"
    );
}

#[tokio::test]
async fn a_calendar_that_names_months_as_numbers_and_writes_other_digits_is_read() {
    let rig = picker(Calendar::new(Style {
        cell: |_, y, m, d| format!("{y}年{m}月{d}日"),
        ..platform(japanese())
    }));
    rig.fake.answer_calendar_names(Some(japanese()));
    rig.select("Date", "2026-11-01").await.unwrap();
    assert_eq!(rig.world.lock().unwrap().picked, "picked 2026-11-01");

    let arabic = names(
        "ar-EG",
        [
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
        ],
        "ص",
        "م",
    );
    let rig = picker(Calendar::new(Style {
        cell: |n, y, m, d| {
            let other = |n: u32| -> String {
                n.to_string()
                    .chars()
                    .map(|c| char::from_u32(0x0660 + c.to_digit(10).unwrap()).unwrap())
                    .collect()
            };
            format!(
                "{} {} {}",
                other(d),
                n.months[m as usize - 1][0],
                other(y as u32)
            )
        },
        ..platform(arabic.clone())
    }));
    rig.fake.answer_calendar_names(Some(arabic));
    rig.select("Date", "2025-12-18").await.unwrap();
    assert_eq!(rig.world.lock().unwrap().picked, "picked 2025-12-18");
}

#[tokio::test]
async fn a_calendar_with_no_ids_and_no_english_has_its_arrows_found_by_trying_the_row_above_its_days()
 {
    let rig = picker(Calendar::new(Style {
        ids: false,
        arrows: [
            "Zum vorherigen Monat wechseln",
            "Zum nächsten Monat wechseln",
        ],
        confirm: "OK",
        ..platform(german())
    }));
    rig.fake.answer_calendar_names(Some(german()));
    rig.select("Date", "2025-11-03").await.unwrap();
    assert_eq!(rig.world.lock().unwrap().picked, "picked 2025-11-03");
    let taps = rig.taps();
    assert_eq!(taps.iter().filter(|t| *t == "back").count(), 2, "{taps:?}");
    assert!(
        taps.iter().filter(|t| *t == "forward").count() <= 1,
        "an arrow that goes the wrong way is tried once: {taps:?}"
    );
    assert_eq!(taps.last().unwrap(), "confirm");
}

#[tokio::test]
async fn a_button_beside_the_arrows_that_swaps_the_calendar_for_a_text_field_is_tried_last_and_undone()
 {
    for (toggle, tried) in [(180.0, 0), (240.0, 2)] {
        let rig = picker(Calendar::new(Style {
            ids: false,
            arrows: [
                "Zum vorherigen Monat wechseln",
                "Zum nächsten Monat wechseln",
            ],
            confirm: "OK",
            toggle: Some(toggle),
            ..platform(german())
        }));
        rig.fake.answer_calendar_names(Some(german()));
        rig.select("Date", "2026-12-18").await.unwrap();
        let world = rig.world.lock().unwrap();
        assert_eq!(world.picked, "picked 2026-12-18");
        assert!(!world.typing);
        let taps = rig.taps();
        assert_eq!(
            taps.iter().filter(|t| *t == "toggle").count(),
            tried,
            "the buttons nearest the days are tried first, and one that hid the calendar is tapped again where it went: {taps:?}"
        );
    }
}

#[tokio::test]
async fn a_calendar_whose_days_name_no_year_and_mark_no_selection_is_read_from_its_headings() {
    let rig = picker(Calendar::new(Style {
        cell: |n, _, m, d| format!("Donnerstag, {d}. {}", n.months[m as usize - 1][0]),
        marks: false,
        ..platform(german())
    }));
    rig.fake.answer_calendar_names(Some(german()));
    rig.select("Date", "2026-02-27").await.unwrap();
    assert_eq!(rig.world.lock().unwrap().picked, "picked 2026-02-27");
    assert_eq!(rig.taps(), ["Date", "forward", "day 27", "confirm"]);
}

#[tokio::test]
async fn a_date_years_away_goes_through_the_list_of_years() {
    let rig = picker(Calendar::new(platform(english_names())));
    rig.select("Date", "1990-05-01").await.unwrap();
    assert_eq!(rig.world.lock().unwrap().picked, "picked 1990-05-01");
    let taps = rig.taps();
    assert_eq!(taps[..2], ["Date", "year"], "{taps:?}");
    assert!(taps.contains(&"year 1990".to_string()), "{taps:?}");
    assert_eq!(
        taps.iter().filter(|t| *t == "forward").count(),
        4,
        "January to May, and no month of the 36 years between: {taps:?}"
    );
}

#[tokio::test]
async fn the_button_that_opens_the_years_is_the_one_that_says_the_month_shown_in_any_order() {
    let rig = picker(Calendar::new(Style {
        ids: false,
        cell: |_, y, m, d| format!("{y}年{m}月{d}日"),
        arrows: ["前月に変更", "翌月に変更"],
        confirm: "OK",
        ..platform(japanese())
    }));
    rig.fake.answer_calendar_names(Some(japanese()));
    rig.select("Date", "1990-05-01").await.unwrap();
    assert_eq!(rig.world.lock().unwrap().picked, "picked 1990-05-01");
    assert!(
        rig.taps().contains(&"year 1990".to_string()),
        "{:?}",
        rig.taps()
    );
}

#[tokio::test]
async fn a_confirm_button_that_is_not_known_is_left_for_the_caller_and_said() {
    let rig = picker(Calendar::new(Style {
        ids: false,
        confirm: "Übernehmen",
        arrows: ["Previous month", "Next month"],
        ..platform(english_names())
    }));
    let r = rig.select("Date", "2026-01-20").await.unwrap();
    let summary = r["summary"].as_str().unwrap();
    assert!(
        summary.contains("set the picker to 2026-01-20 · it shows no button that confirms it"),
        "{summary}"
    );
    assert_eq!(rig.world.lock().unwrap().picked, "nothing picked");
    assert_eq!(rig.world.lock().unwrap().chosen, (2026, 1, 20));
}

#[tokio::test]
async fn a_device_that_does_not_name_its_months_gets_no_date_set() {
    let rig = picker(Calendar::new(platform(english_names())));
    rig.fake.answer_calendar_names(None);
    let e = rig.select("Date", "2026-03-09").await.unwrap_err();
    assert!(e.contains("does not tell the names of its months"), "{e}");
    assert_eq!(rig.taps(), ["Date"], "nothing on the picker was touched");
}

/// Date wheels as the platform's: a typed column moves its neighbours when it goes from one end to the other
/// (`Jan` to `Dec` is one month back, into the year before), and a day the month does not have is not taken.
struct DateWheels {
    names: CalendarNames,
    open: bool,
    at: (i32, u32, u32),
    numbered: bool,
    focus: Option<&'static str>,
    picked: String,
}

impl World for DateWheels {
    fn draw(&self) -> Vec<Node> {
        if !self.open {
            return vec![
                node("Date", BUTTON, [0.0, 60.0, 180.0, 48.0]),
                node("picked", TEXT, [0.0, 120.0, 360.0, 40.0])
                    .text(&self.picked)
                    .inert(),
            ];
        }
        let (y, m, d) = self.at;
        let column = |key: &'static str, x: f32, value: String| {
            node(key, FIELD, [x, 348.0, 80.0, 48.0])
                .text(value)
                .id("android:id/numberpicker_input")
                .flags(if self.focus == Some(key) { "EF" } else { "E" })
        };
        let ok = node("OK", BUTTON, [260.0, 700.0, 90.0, 48.0]).id("android:id/button1");
        if self.numbered {
            return vec![
                column("year", 40.0, y.to_string()),
                column("month", 140.0, m.to_string()),
                column("day", 240.0, d.to_string()),
                ok,
            ];
        }
        vec![
            column("month", 40.0, self.names.months[m as usize - 1][2].clone()),
            column("day", 140.0, format!("{d:02}")),
            column("year", 240.0, y.to_string()),
            ok,
        ]
    }

    fn tap(&mut self, key: &str) {
        match key {
            "Date" => self.open = true,
            "month" => self.focus = Some("month"),
            "day" => self.focus = Some("day"),
            "year" => self.focus = Some("year"),
            _ => {
                self.open = false;
                let (y, m, d) = self.at;
                self.picked = format!("picked {y:04}-{m:02}-{d:02}");
            }
        }
    }

    fn text(&mut self, text: &str) {
        let (y, m, d) = &mut self.at;
        match self.focus {
            Some("year") => *y = text.parse().unwrap_or(*y),
            Some("month") => {
                let named = match text.parse::<u32>() {
                    Ok(n) if self.numbered => Some((n + 11) % 12 + 1),
                    _ => self.names.month_named(text),
                };
                let Some(to) = named else {
                    return;
                };
                match (*m, to) {
                    (1, 12) => *y -= 1,
                    (12, 1) => *y += 1,
                    _ => {}
                }
                *m = to;
            }
            Some("day") => match text.parse() {
                Ok(to) if (1..=days_in(*y, *m)).contains(&to) => *d = to,
                _ => {}
            },
            _ => {}
        }
        *d = (*d).min(days_in(*y, *m));
    }
}

fn date_wheels(names: CalendarNames) -> DateWheels {
    DateWheels {
        names,
        open: false,
        at: (2026, 1, 15),
        numbered: false,
        focus: None,
        picked: "nothing picked".into(),
    }
}

#[tokio::test]
async fn a_month_wheel_of_numbers_is_told_from_the_day_wheel_by_which_of_them_takes_13() {
    for (at, date) in [
        ((2026, 1, 15), "2031-12-31"),
        ((2026, 5, 7), "2027-03-09"),
        ((2026, 5, 7), "2026-05-07"),
        ((2026, 7, 5), "2026-05-07"),
    ] {
        let rig = picker(DateWheels {
            at,
            numbered: true,
            ..date_wheels(japanese())
        });
        rig.fake.answer_calendar_names(Some(japanese()));
        let r = rig.select("Date", date).await;
        assert_eq!(
            rig.world.lock().unwrap().picked,
            format!("picked {date}"),
            "{r:?}"
        );
    }
}

#[tokio::test]
async fn date_wheels_are_typed_into_until_one_read_shows_the_whole_date() {
    for date in ["2031-12-31", "2028-02-29", "1990-03-07", "2026-01-15"] {
        let rig = picker(date_wheels(english_names()));
        rig.select("Date", date).await.unwrap();
        assert_eq!(rig.world.lock().unwrap().picked, format!("picked {date}"));
    }
    let rig = picker(date_wheels(german()));
    rig.fake.answer_calendar_names(Some(german()));
    rig.select("Date", "2027-03-31").await.unwrap();
    assert_eq!(rig.world.lock().unwrap().picked, "picked 2027-03-31");
}

/// Time wheels: an hour, a minute and the device's word for the half of the day; the hour typed from 11 to 12
/// turns the half over, as the platform's does.
struct TimeWheels {
    names: CalendarNames,
    open: bool,
    at: (u32, u32, bool),
    from_zero: bool,
    focus: Option<&'static str>,
    picked: String,
}

impl World for TimeWheels {
    fn draw(&self) -> Vec<Node> {
        if !self.open {
            return vec![node("Time", BUTTON, [0.0, 60.0, 180.0, 48.0])];
        }
        let (h, m, pm) = self.at;
        let column = |key: &'static str, x: f32, value: String| {
            node(key, FIELD, [x, 348.0, 80.0, 48.0])
                .text(value)
                .id("android:id/numberpicker_input")
                .flags(if self.focus == Some(key) { "EF" } else { "E" })
        };
        let half = if pm {
            &self.names.pm[0]
        } else {
            &self.names.am[0]
        };
        vec![
            column("hour", 40.0, h.to_string()),
            column("minute", 140.0, format!("{m:02}")),
            column("half", 240.0, half.clone()),
            node("OK", BUTTON, [260.0, 700.0, 90.0, 48.0]).id("android:id/button1"),
        ]
    }

    fn tap(&mut self, key: &str) {
        match key {
            "Time" => self.open = true,
            "hour" => self.focus = Some("hour"),
            "minute" => self.focus = Some("minute"),
            "half" => self.focus = Some("half"),
            _ => {
                self.open = false;
                let (h, m, pm) = self.at;
                self.picked = format!("picked {:02}:{m:02}", h % 12 + if pm { 12 } else { 0 });
            }
        }
    }

    fn text(&mut self, text: &str) {
        let (h, m, pm) = &mut self.at;
        match self.focus {
            Some("hour") => {
                let Ok(to) = text.parse::<u32>() else { return };
                if self.from_zero && to > 11 {
                    return;
                }
                if (*h, to) == (11, 12) || (*h, to) == (12, 11) {
                    *pm = !*pm;
                }
                *h = to;
            }
            Some("minute") => *m = text.parse().unwrap_or(*m),
            Some("half") => {
                if let Some(second) = self.names.half_named(text) {
                    *pm = second;
                }
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn time_wheels_are_typed_into_and_the_half_of_the_day_is_the_devices_word() {
    for (names, time) in [
        (english_names(), "19:45"),
        (english_names(), "00:00"),
        (english_names(), "12:30"),
        (japanese(), "23:59"),
    ] {
        let rig = picker(TimeWheels {
            names: names.clone(),
            open: false,
            at: (11, 5, false),
            from_zero: false,
            focus: None,
            picked: String::new(),
        });
        rig.fake.answer_calendar_names(Some(names));
        rig.select("Time", time).await.unwrap();
        assert_eq!(rig.world.lock().unwrap().picked, format!("picked {time}"));
    }
}

#[tokio::test]
async fn time_wheels_that_count_the_hours_from_0_take_0_where_12_is_not_taken() {
    for time in ["00:00", "12:30", "19:45"] {
        let rig = picker(TimeWheels {
            names: japanese(),
            open: false,
            at: (9, 5, false),
            from_zero: true,
            focus: None,
            picked: String::new(),
        });
        rig.fake.answer_calendar_names(Some(japanese()));
        let r = rig.select("Time", time).await;
        assert_eq!(
            rig.world.lock().unwrap().picked,
            format!("picked {time}"),
            "{r:?}"
        );
    }
}

/// The platform's time picker: a clock, one button that turns it into two fields and a menu for the half of the
/// day, and back. Its ids are the platform's; its words are German.
struct TypedTime {
    typing: bool,
    open: bool,
    menu: bool,
    at: (String, String, bool),
    own_digits: bool,
    closes: Closes,
    focus: Option<&'static str>,
    picked: String,
}

#[derive(PartialEq)]
enum Closes {
    Always,
    UnlessTheHourIs12,
    Never,
}

impl World for TypedTime {
    fn draw(&self) -> Vec<Node> {
        if !self.open {
            return vec![
                node("Time", BUTTON, [0.0, 60.0, 180.0, 48.0]),
                node("picked", TEXT, [0.0, 120.0, 360.0, 40.0])
                    .text(&self.picked)
                    .inert(),
            ];
        }
        let mut all = vec![
            node("mode", BUTTON, [20.0, 700.0, 48.0, 48.0])
                .text("Zur Texteingabe wechseln")
                .id("android:id/toggle_mode"),
            node("OK", BUTTON, [260.0, 700.0, 90.0, 48.0]).id("android:id/button1"),
        ];
        if !self.typing {
            return all;
        }
        let field = |key: &'static str, x: f32, value: &str| {
            node(key, FIELD, [x, 400.0, 60.0, 56.0])
                .text(value)
                .flags(if self.focus == Some(key) { "EF" } else { "E" })
        };
        all.push(field("hour", 60.0, &self.at.0).id("android:id/input_hour"));
        all.push(field("minute", 130.0, &self.at.1).id("android:id/input_minute"));
        all.push(
            node("half", "android.widget.Spinner", [230.0, 400.0, 90.0, 56.0])
                .text(if self.at.2 { "PM" } else { "AM" })
                .id("android:id/am_pm_spinner"),
        );
        if self.menu {
            all.push(node(
                "AM",
                "android.widget.CheckedTextView",
                [230.0, 460.0, 90.0, 48.0],
            ));
            all.push(node(
                "PM",
                "android.widget.CheckedTextView",
                [230.0, 510.0, 90.0, 48.0],
            ));
        }
        all
    }

    fn tap(&mut self, key: &str) {
        match key {
            "Time" => self.open = true,
            "mode" => self.typing = !self.typing,
            "hour" => self.focus = Some("hour"),
            "minute" => self.focus = Some("minute"),
            "half" => self.menu = true,
            "AM" | "PM" => {
                self.at.2 = key == "PM";
                self.menu = false;
            }
            _ if self.closes == Closes::Never
                || (self.closes == Closes::UnlessTheHourIs12 && self.at.0 == "12") => {}
            _ => {
                self.open = false;
                let plain = |text: &str| -> String {
                    text.chars()
                        .filter_map(phonedriver_core::calendar::digit)
                        .map(|d| char::from_digit(d, 10).unwrap())
                        .collect()
                };
                let hour: u32 = plain(&self.at.0).parse().unwrap_or(99);
                self.picked = format!(
                    "picked {:02}:{}",
                    hour % 12 + if self.at.2 { 12 } else { 0 },
                    plain(&self.at.1)
                );
            }
        }
    }

    fn text(&mut self, text: &str) {
        let text = if self.own_digits && text.chars().any(|c| c.is_ascii_digit()) {
            ""
        } else {
            text
        };
        match self.focus {
            Some("hour") => self.at.0 = text.into(),
            Some("minute") => self.at.1 = text.into(),
            _ => {}
        }
    }
}

#[tokio::test]
async fn a_time_picker_is_turned_to_its_text_input_typed_into_read_back_and_confirmed() {
    for time in ["19:45", "00:30", "12:05"] {
        let rig = picker(TypedTime {
            typing: false,
            open: false,
            menu: false,
            at: ("9".into(), "05".into(), false),
            own_digits: false,
            closes: Closes::Always,
            focus: None,
            picked: String::new(),
        });
        let r = rig.select("Time", time).await.unwrap();
        assert_eq!(
            rig.world.lock().unwrap().picked,
            format!("picked {time}"),
            "{r}"
        );
        assert_eq!(rig.taps()[..2], ["Time", "mode"]);
        assert_eq!(rig.taps().last().unwrap(), "OK");
    }
}

#[tokio::test]
async fn a_time_field_that_takes_only_the_languages_own_digits_is_typed_into_in_them() {
    let egypt = CalendarNames {
        digits: "٠١٢٣٤٥٦٧٨٩".into(),
        ..english_names()
    };
    let rig = picker(TypedTime {
        typing: false,
        open: false,
        menu: false,
        at: ("٩".into(), "٠٥".into(), false),
        own_digits: true,
        closes: Closes::Always,
        focus: None,
        picked: String::new(),
    });
    rig.fake.answer_calendar_names(Some(egypt));
    let r = rig.select("Time", "19:45").await;
    assert_eq!(rig.world.lock().unwrap().picked, "picked 19:45", "{r:?}");
}

#[tokio::test]
async fn a_picker_that_stays_open_when_its_button_is_tapped_is_not_reported_as_set() {
    let typed = |closes: Closes| TypedTime {
        typing: false,
        open: false,
        menu: false,
        at: ("9".into(), "05".into(), false),
        own_digits: false,
        closes,
        focus: None,
        picked: String::new(),
    };
    for time in ["00:30", "12:05"] {
        let rig = picker(typed(Closes::UnlessTheHourIs12));
        let r = rig.select("Time", time).await;
        assert_eq!(
            rig.world.lock().unwrap().picked,
            format!("picked {time}"),
            "a field that counts the hours from 0 is given 0: {r:?}"
        );
    }
    let rig = picker(typed(Closes::Never));
    let e = rig.select("Time", "19:45").await.unwrap_err();
    assert!(
        e.contains("stayed open when the button that confirms it was tapped"),
        "{e}"
    );
    assert!(rig.world.lock().unwrap().open);
}

/// Material Components' time picker as the Clock app shows it (API 37 emulator, 2026-11-26): in its text input
/// the part that is being typed into is a chip with a text field over it at the same place, and the other is a
/// chip alone that gets the field when it is tapped.
struct ChipTime {
    typing: bool,
    open: bool,
    at: (String, String, bool),
    focus: &'static str,
    picked: String,
}

impl World for ChipTime {
    fn draw(&self) -> Vec<Node> {
        if !self.open {
            return vec![node("Time", BUTTON, [0.0, 60.0, 180.0, 48.0])];
        }
        let mut all = vec![
            node("mode", BUTTON, [20.0, 700.0, 48.0, 48.0])
                .text("Switch to text input mode for the time input.")
                .id("com.example:id/material_timepicker_mode_button"),
            node("OK", BUTTON, [260.0, 700.0, 90.0, 48.0])
                .id("com.example:id/material_timepicker_ok_button"),
            node(
                "AM",
                "android.widget.RadioButton",
                [290.0, 250.0, 52.0, 48.0],
            )
            .id("com.example:id/material_clock_period_am_button")
            .flags(if self.at.2 { "c" } else { "cC" }),
            node(
                "PM",
                "android.widget.RadioButton",
                [290.0, 300.0, 52.0, 46.0],
            )
            .id("com.example:id/material_clock_period_pm_button")
            .flags(if self.at.2 { "cC" } else { "c" }),
        ];
        if !self.typing {
            return all;
        }
        for (key, x, says, value) in [
            (
                "hour",
                60.0,
                format!("Hour {} o'clock", self.at.0),
                &self.at.0,
            ),
            (
                "minute",
                180.0,
                format!("Minute {} minutes", self.at.1),
                &self.at.1,
            ),
        ] {
            let place = [x, 260.0, 96.0, 80.0];
            let on = self.focus == key;
            all.push(
                node(format!("{key} chip"), BUTTON, place)
                    .text(says)
                    .flags(if on { "cC" } else { "c" }),
            );
            if on {
                all.push(node(key, FIELD, place).text(value).flags("EF"));
            }
        }
        all
    }

    fn tap(&mut self, key: &str) {
        match key {
            "Time" => self.open = true,
            "mode" => {
                self.typing = !self.typing;
                self.focus = "hour";
            }
            "hour" | "hour chip" => self.focus = "hour",
            "minute" | "minute chip" => self.focus = "minute",
            "AM" | "PM" => self.at.2 = key == "PM",
            _ => {
                self.open = false;
                let hour: u32 = self.at.0.parse().unwrap_or(99);
                self.picked = format!(
                    "picked {:02}:{}",
                    hour % 12 + if self.at.2 { 12 } else { 0 },
                    self.at.1
                );
            }
        }
    }

    fn text(&mut self, text: &str) {
        match self.focus {
            "hour" => self.at.0 = text.into(),
            _ => self.at.1 = text.into(),
        }
    }
}

fn chip_time(open: bool) -> ChipTime {
    ChipTime {
        typing: false,
        open,
        at: ("9".into(), "00".into(), true),
        focus: "hour",
        picked: String::new(),
    }
}

#[tokio::test]
async fn a_time_field_drawn_over_its_chip_is_the_one_typed_into() {
    for time in ["07:30", "19:45"] {
        let rig = picker(chip_time(false));
        let r = rig.select("Time", time).await;
        assert_eq!(
            rig.world.lock().unwrap().picked,
            format!("picked {time}"),
            "{r:?}"
        );
    }
}

#[tokio::test]
async fn select_on_the_button_that_confirms_an_open_picker_sets_the_picker_before_that_button_is_tapped()
 {
    let rig = picker(chip_time(true));
    let r = rig.select("OK", "07:30").await;
    assert_eq!(rig.world.lock().unwrap().picked, "picked 07:30", "{r:?}");
    let taps = rig.taps();
    assert_eq!(taps.iter().filter(|t| *t == "OK").count(), 1, "{taps:?}");
    assert_eq!(taps.last().unwrap(), "OK");
    let reply = r.unwrap().to_string();
    assert!(reply.contains("set the open picker to 07:30"), "{reply}");
}

/// A clock with nothing but its face, as Compose Material 3 draws it: the hours, then by itself the minutes in
/// fives; a tap between two marks goes to the nearer one and a drag stays where it ends, a minute short at first.
struct Clock {
    names: CalendarNames,
    whole_day: bool,
    open: bool,
    minutes: bool,
    at: (u32, u32, bool),
    short: bool,
    picked: String,
}

const MIDDLE: (f32, f32) = (180.0, 450.0);

impl World for Clock {
    fn draw(&self) -> Vec<Node> {
        if !self.open {
            return vec![node("Time", BUTTON, [0.0, 60.0, 180.0, 48.0])];
        }
        let (h, m, pm) = self.at;
        let chip = |key: &'static str, x: f32, says: String, on: bool| {
            node(key, BUTTON, [x, 240.0, 96.0, 80.0])
                .text(if on { says } else { key.to_string() })
                .flags(if on { "cC" } else { "c" })
        };
        let mut all = vec![
            chip("Stunde", 60.0, format!("{h} Uhr"), !self.minutes),
            chip("Minuten", 170.0, format!("{m} Minuten"), self.minutes),
            node("OK", BUTTON, [260.0, 700.0, 90.0, 48.0]),
        ];
        if !self.whole_day {
            for (second, name) in [(false, &self.names.am[0]), (true, &self.names.pm[0])] {
                all.push(
                    node(
                        name.clone(),
                        BUTTON,
                        [
                            290.0,
                            240.0 + if second { 40.0 } else { 0.0 },
                            48.0,
                            if second { 48.0 } else { 40.0 },
                        ],
                    )
                    .flags(if pm == second { "cC" } else { "c" }),
                );
            }
        }
        let marks: Vec<u32> = match (self.minutes, self.whole_day) {
            (true, _) => (0..12).map(|n| n * 5).collect(),
            (false, false) => (1..=12).collect(),
            (false, true) => (0..24).collect(),
        };
        for n in marks {
            let turn = if self.minutes {
                n as f32 / 60.0
            } else {
                (n % 12) as f32 / 12.0
            };
            let angle = turn * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            let radius = if !self.minutes && self.whole_day && (n == 0 || n > 12) {
                70.0
            } else {
                110.0
            };
            let (x, y) = (
                MIDDLE.0 + radius * angle.cos(),
                MIDDLE.1 + radius * angle.sin(),
            );
            all.push(
                node(
                    format!("mark {n}"),
                    BUTTON,
                    [x - 24.0, y - 24.0, 48.0, 48.0],
                )
                .text(format!(
                    "{n} {}",
                    if self.minutes { "Minuten" } else { "Uhr" }
                ))
                .flags("c"),
            );
        }
        all
    }

    fn tap(&mut self, key: &str) {
        match key.split_once(' ') {
            _ if key == "Time" => self.open = true,
            _ if key == "Stunde" => self.minutes = false,
            _ if key == "Minuten" => self.minutes = true,
            _ if key == "OK" => {
                self.open = false;
                let (h, m, pm) = self.at;
                let hour = if self.whole_day {
                    h
                } else {
                    h % 12 + if pm { 12 } else { 0 }
                };
                self.picked = format!("picked {hour:02}:{m:02}");
            }
            Some(("mark", n)) if self.minutes => self.at.1 = n.parse().unwrap(),
            Some(("mark", n)) => {
                self.at.0 = n.parse().unwrap();
                self.minutes = true;
            }
            _ => self.at.2 = self.names.half_named(key).unwrap_or(self.at.2),
        }
    }

    fn drag(&mut self, _from: Point, to: Point) {
        if !self.minutes {
            return;
        }
        let angle = (to.y - MIDDLE.1).atan2(to.x - MIDDLE.0) + std::f32::consts::FRAC_PI_2;
        let minute = angle.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU * 60.0;
        let short = if std::mem::take(&mut self.short) {
            1.0
        } else {
            0.0
        };
        self.at.1 = (minute - short).round().rem_euclid(60.0) as u32;
    }
}

#[tokio::test]
async fn a_clock_with_only_its_face_is_set_by_its_marks_and_a_minute_between_two_by_a_drag_that_is_read_back()
 {
    for (names, whole_day, time) in [
        (english_names(), false, "19:45"),
        (english_names(), false, "00:30"),
        (english_names(), false, "07:47"),
        (japanese(), false, "12:01"),
        (english_names(), true, "23:59"),
        (english_names(), true, "00:07"),
    ] {
        let rig = picker(Clock {
            names: names.clone(),
            whole_day,
            open: false,
            minutes: false,
            at: (9, 5, false),
            short: true,
            picked: String::new(),
        });
        rig.fake.answer_calendar_names(Some(names));
        let r = rig.select("Time", time).await;
        assert_eq!(
            rig.world.lock().unwrap().picked,
            format!("picked {time}"),
            "{r:?} {:?}",
            rig.taps()
        );
    }
}

#[tokio::test]
async fn a_clock_whose_halves_are_not_named_as_the_device_names_them_is_not_set() {
    let rig = picker(Clock {
        names: japanese(),
        whole_day: false,
        open: false,
        minutes: false,
        at: (9, 5, false),
        short: false,
        picked: String::new(),
    });
    let e = rig.select("Time", "19:45").await.unwrap_err();
    assert!(
        e.contains("nothing on it is named as the device names the halves of the day"),
        "{e}"
    );
    assert_eq!(rig.taps(), ["Time"], "no hour was tapped");
}

#[tokio::test]
async fn a_clock_that_names_one_half_as_the_device_does_has_the_other_half_beside_it() {
    let mut drawn = german();
    drawn.pm = vec!["Nachmittags".into()];
    for (afternoon, time) in [(false, "19:45"), (true, "07:30"), (true, "13:00")] {
        let rig = picker(Clock {
            names: drawn.clone(),
            whole_day: false,
            open: false,
            minutes: false,
            at: (9, 5, afternoon),
            short: true,
            picked: String::new(),
        });
        rig.fake.answer_calendar_names(Some(german()));
        let r = rig.select("Time", time).await;
        assert_eq!(
            rig.world.lock().unwrap().picked,
            format!("picked {time}"),
            "{r:?} {:?}",
            rig.taps()
        );
    }
}

const WHEEL: &str = "XCUIElementTypePickerWheel";
const IOS_BUTTON: &str = "XCUIElementTypeButton";

/// An iPhone's pickers: a compact date picker that opens a calendar in a popover (its arrows and its year button
/// named by id, its days without a year, a month wheel and a year wheel behind the year button, and a region over
/// the whole screen that closes it), and an inline time picker of two or three wheels. A wheel moves one value
/// when the row above or below its middle is touched, and each value is taken as it is set.
struct Phone {
    names: CalendarNames,
    popover: bool,
    years: bool,
    date: (i32, u32, u32),
    shown: (i32, u32),
    time: (u32, u32),
    halves: bool,
}

impl Phone {
    fn new(names: CalendarNames, halves: bool) -> Phone {
        Phone {
            names,
            popover: false,
            years: false,
            date: (2026, 1, 15),
            shown: (2026, 1),
            time: (9, 5),
            halves,
        }
    }

    fn wheels(&self) -> Vec<(&'static str, [f32; 4], String)> {
        if self.popover && self.years {
            let month = self.names.months[self.shown.1 as usize - 1][0].clone();
            return vec![
                ("month", [60.0, 250.0, 130.0, 290.0], month),
                (
                    "year",
                    [200.0, 250.0, 80.0, 290.0],
                    self.shown.0.to_string(),
                ),
            ];
        }
        if self.popover {
            return vec![];
        }
        let (h, m) = self.time;
        let mut all = vec![
            ("hour", [120.0, 420.0, 60.0, 290.0], {
                let shown = if self.halves { (h + 11) % 12 + 1 } else { h };
                format!("{shown} o’clock")
            }),
            (
                "minute",
                [190.0, 420.0, 50.0, 290.0],
                format!("{m:02} minutes"),
            ),
        ];
        if self.halves {
            let half = if h >= 12 {
                &self.names.pm[0]
            } else {
                &self.names.am[0]
            };
            all.push(("half", [250.0, 420.0, 50.0, 290.0], half.clone()));
        }
        all
    }
}

impl World for Phone {
    const IPHONE: bool = true;

    fn draw(&self) -> Vec<Node> {
        let mut all = Vec::new();
        if self.popover {
            all.push(
                node("dismiss", IOS_BUTTON, [0.0, 0.0, 360.0, 800.0])
                    .text("dismiss popup")
                    .id("PopoverDismissRegion"),
            );
            let heading = format!(
                "{} {}",
                self.names.months[self.shown.1 as usize - 1][0],
                self.shown.0
            );
            all.push(
                node("years", IOS_BUTTON, [60.0, 200.0, 140.0, 40.0])
                    .text("Show year picker")
                    .value(heading)
                    .id(if self.years {
                        "DatePicker.Hide"
                    } else {
                        "DatePicker.Show"
                    }),
            );
            all.push(
                node("back", IOS_BUTTON, [250.0, 200.0, 44.0, 40.0])
                    .text("Vorheriger Monat")
                    .id("DatePicker.PreviousMonth"),
            );
            all.push(
                node("forward", IOS_BUTTON, [300.0, 200.0, 44.0, 40.0])
                    .text("Nächster Monat")
                    .id("DatePicker.NextMonth"),
            );
            if !self.years {
                let (year, month) = self.shown;
                for day in 1..=days_in(year, month) {
                    let i = day - 1;
                    all.push(
                        node(
                            format!("day {day}"),
                            IOS_BUTTON,
                            [
                                40.0 + 42.0 * (i % 7) as f32,
                                260.0 + 45.0 * (i / 7) as f32,
                                42.0,
                                45.0,
                            ],
                        )
                        .text(format!(
                            "Donnerstag, {day}. {}",
                            self.names.months[month as usize - 1][0]
                        ))
                        .flags(if (year, month, day) == self.date {
                            "s"
                        } else {
                            ""
                        }),
                    );
                }
            }
        } else {
            let (y, m, d) = self.date;
            all.push(
                node("Date Picker", IOS_BUTTON, [200.0, 100.0, 140.0, 40.0])
                    .value(format!("{d:02}.{m:02}.{y}")),
            );
            all.push(node(
                "Time wheel",
                "XCUIElementTypeCell",
                [10.0, 410.0, 340.0, 310.0],
            ));
        }
        for (key, at, value) in self.wheels() {
            all.push(node(key, WHEEL, at).text("").value(value));
        }
        all
    }

    fn touch(&mut self, at: Point) -> bool {
        let hit = self.wheels().into_iter().find(|(_, b, _)| {
            (b[0]..b[0] + b[2]).contains(&at.x) && (b[1]..b[1] + b[3]).contains(&at.y)
        });
        let Some((key, b, _)) = hit else {
            return false;
        };
        let rows = ((at.y - (b[1] + b[3] / 2.0)) / 34.0).round() as i32;
        let step = |n: u32, span: u32| (n as i32 + rows).rem_euclid(span as i32) as u32;
        match key {
            "year" => self.shown.0 += rows,
            "month" => self.shown.1 = step(self.shown.1 - 1, 12) + 1,
            "hour" if self.halves => {
                self.time.0 = self.time.0 / 12 * 12 + step(self.time.0 % 12, 12);
            }
            "hour" => self.time.0 = step(self.time.0, 24),
            "minute" => self.time.1 = step(self.time.1, 60),
            _ => self.time.0 = (self.time.0 % 12) + if rows > 0 { 12 } else { 0 },
        }
        true
    }

    fn tap(&mut self, key: &str) {
        let step = |(year, month): (i32, u32), by: i32| {
            let at = year * 12 + month as i32 - 1 + by;
            (at.div_euclid(12), at.rem_euclid(12) as u32 + 1)
        };
        match key.split_once(' ') {
            _ if key == "Date Picker" => self.popover = true,
            _ if key == "dismiss" => (self.popover, self.years) = (false, false),
            _ if key == "years" => self.years = !self.years,
            _ if key == "back" => self.shown = step(self.shown, -1),
            _ if key == "forward" => self.shown = step(self.shown, 1),
            Some(("day", d)) => self.date = (self.shown.0, self.shown.1, d.parse().unwrap()),
            _ => {}
        }
    }
}

#[tokio::test]
async fn an_iphones_calendar_is_stepped_by_its_ids_set_through_its_year_wheels_and_closed_beside_its_popover()
 {
    let rig = picker(Phone::new(german(), false));
    rig.fake.answer_calendar_names(Some(german()));
    let r = rig.select("Date Picker", "2026-03-09").await.unwrap();
    assert_eq!(rig.world.lock().unwrap().date, (2026, 3, 9), "{r}");
    assert!(
        !rig.world.lock().unwrap().popover,
        "the popover was closed: {:?}",
        rig.taps()
    );
    assert_eq!(
        rig.taps(),
        ["Date Picker", "forward", "forward", "day 9", "dismiss"]
    );
    assert!(
        r["summary"]
            .as_str()
            .unwrap()
            .ends_with("set the picker to 2026-03-09, then closed it"),
        "{r}"
    );

    let r = rig.select("Date Picker", "1990-05-01").await.unwrap();
    assert_eq!(rig.world.lock().unwrap().date, (1990, 5, 1), "{r}");
    let taps = rig.taps();
    assert_eq!(
        taps.iter().filter(|t| *t == "years").count(),
        2,
        "opened and closed: {taps:?}"
    );
    assert_eq!(
        taps.iter().filter(|t| *t == "wheel").count(),
        36 + 2,
        "{taps:?}"
    );
    assert_eq!(taps.last().unwrap(), "dismiss");
}

#[tokio::test]
async fn an_iphones_time_wheels_are_stepped_a_row_at_a_time_the_short_way_round() {
    for (halves, time, steps) in [
        (false, "19:45", 10 + 20),
        (false, "00:07", 9 + 2),
        (true, "23:59", 2 + 6 + 1),
    ] {
        let rig = picker(Phone::new(english_names(), halves));
        let r = rig.select("Time wheel", time).await.unwrap();
        let (h, m) = rig.world.lock().unwrap().time;
        assert_eq!(format!("{h:02}:{m:02}"), time, "{r}");
        assert_eq!(rig.taps().len(), 1 + steps, "{:?}", rig.taps());
        assert!(
            r["summary"]
                .as_str()
                .unwrap()
                .contains("it shows no button that confirms it"),
            "{r}"
        );
    }
}
