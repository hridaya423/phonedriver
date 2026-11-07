use std::time::Duration;

use crate::geometry::Rect;

/// Settle parameters (architecture 4.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Config {
    /// How long to wait for a first significant change; `None` skips that phase (snapshot).
    pub expect_change: Option<Duration>,
    pub quiet: Duration,
    /// Changed area in square points above which a frame counts as a change.
    pub significant_area: f32,
    /// Changed area above which a frame counts as the first change after an action.
    /// A switch toggle changes at most 432 pt², a blinking caret 96 pt² (spike A3).
    pub first_change_area: f32,
    pub timeout: Duration,
}

impl Config {
    pub fn action() -> Config {
        Config {
            expect_change: Some(Duration::from_millis(500)),
            quiet: Duration::from_millis(250),
            significant_area: 4096.0,
            first_change_area: 128.0,
            timeout: Duration::from_secs(5),
        }
    }

    pub fn launch() -> Config {
        Config {
            expect_change: Some(Duration::from_millis(3000)),
            ..Config::action()
        }
    }

    pub fn snapshot() -> Config {
        Config {
            expect_change: None,
            timeout: Duration::from_secs(2),
            ..Config::action()
        }
    }
}

/// A timestamped observation of the screen. There is deliberately no content-change input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input {
    WindowChanged,
    Frame {
        changed_area_pt2: f32,
        changed_box: Rect,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    Waiting,
    /// `settled: false` at timeout is a normal result, not an error.
    Done {
        settled: bool,
        changed: bool,
        still_changing: Option<Rect>,
        elapsed: Duration,
    },
}

/// A pure two-phase settle state machine. Time is passed in; it never reads a clock.
#[derive(Debug, Clone)]
pub struct Detector {
    config: Config,
    start: Duration,
    last_significant: Option<Duration>,
    changing: Option<Rect>,
    result: Option<State>,
}

impl Detector {
    pub fn new(config: Config, start: Duration) -> Detector {
        Detector {
            config,
            start,
            last_significant: None,
            changing: None,
            result: None,
        }
    }

    pub fn feed(&mut self, input: Input, at: Duration) {
        if self.result.is_some() {
            return;
        }
        let changed_box = match input {
            Input::WindowChanged => None,
            Input::Frame {
                changed_area_pt2,
                changed_box,
            } => {
                let first = self.last_significant.is_none()
                    && self.config.expect_change.is_some()
                    && changed_area_pt2 > self.config.first_change_area;
                if changed_area_pt2 <= self.config.significant_area && !first {
                    return;
                }
                Some(changed_box)
            }
        };
        let burst_continues = self
            .last_significant
            .is_some_and(|t| at < t + self.config.quiet);
        if let Some(b) = changed_box {
            self.changing = match self.changing {
                Some(prev) if burst_continues => Some(prev.union(b)),
                _ => Some(b),
            };
        }
        self.last_significant = Some(self.last_significant.map_or(at, |t| t.max(at)));
    }

    pub fn last_significant(&self) -> Option<Duration> {
        self.last_significant
    }

    pub fn poll(&mut self, now: Duration) -> State {
        if let Some(r) = self.result {
            return r;
        }
        let elapsed = now.saturating_sub(self.start);
        let changed = self.last_significant.is_some();
        let state = if elapsed >= self.config.timeout {
            State::Done {
                settled: false,
                changed,
                still_changing: self.changing,
                elapsed,
            }
        } else {
            let quiet_since = match (self.last_significant, self.config.expect_change) {
                (Some(t), _) => Some(t),
                (None, None) => Some(self.start),
                (None, Some(expect)) if elapsed >= expect => {
                    return self.finish(State::Done {
                        settled: true,
                        changed: false,
                        still_changing: None,
                        elapsed,
                    });
                }
                (None, Some(_)) => None,
            };
            match quiet_since {
                Some(t) if now >= t + self.config.quiet => State::Done {
                    settled: true,
                    changed,
                    still_changing: None,
                    elapsed,
                },
                _ => State::Waiting,
            }
        };
        self.finish(state)
    }

    fn finish(&mut self, state: State) -> State {
        if let State::Done { .. } = state {
            self.result = Some(state);
        }
        state
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use proptest::prelude::*;

    use super::*;
    use crate::geometry::Rect;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn frame(area: f32) -> Input {
        Input::Frame {
            changed_area_pt2: area,
            changed_box: Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: area / 100.0,
            },
        }
    }

    fn done(s: State) -> Option<(bool, bool)> {
        match s {
            State::Done {
                settled, changed, ..
            } => Some((settled, changed)),
            State::Waiting => None,
        }
    }

    #[test]
    fn defaults_match_architecture() {
        let c = Config::action();
        assert_eq!(
            (c.expect_change, c.quiet, c.significant_area, c.timeout),
            (Some(ms(500)), ms(250), 4096.0, ms(5000))
        );
        assert_eq!(Config::launch().expect_change, Some(ms(3000)));
        assert_eq!(Config::snapshot().expect_change, None);
    }

    #[test]
    fn a_toggle_sized_change_counts_as_the_first_change() {
        let mut d = Detector::new(Config::action(), ms(0));
        d.feed(frame(400.0), ms(300));
        assert_eq!(done(d.poll(ms(549))), None);
        assert_eq!(done(d.poll(ms(550))), Some((true, true)));
    }

    #[test]
    fn a_caret_sized_change_is_not_a_first_change() {
        let mut d = Detector::new(Config::action(), ms(0));
        d.feed(frame(96.0), ms(300));
        assert_eq!(done(d.poll(ms(500))), Some((true, false)));
    }

    #[test]
    fn small_frames_after_the_first_change_do_not_extend_quiet() {
        let mut d = Detector::new(Config::action(), ms(0));
        d.feed(frame(20_000.0), ms(100));
        d.feed(frame(400.0), ms(300));
        assert_eq!(done(d.poll(ms(350))), Some((true, true)));
    }

    #[test]
    fn no_change_after_action_waits_for_first_change_window() {
        let mut d = Detector::new(Config::action(), ms(0));
        assert_eq!(
            done(d.poll(ms(250))),
            None,
            "must not settle before expect_change elapses"
        );
        assert_eq!(done(d.poll(ms(499))), None);
        assert_eq!(done(d.poll(ms(500))), Some((true, false)));
    }

    #[test]
    fn change_then_quiet_settles_after_quiet() {
        let mut d = Detector::new(Config::action(), ms(0));
        d.feed(frame(20_000.0), ms(400));
        assert_eq!(done(d.poll(ms(500))), None);
        assert_eq!(done(d.poll(ms(649))), None);
        assert!(
            matches!(d.poll(ms(650)), State::Done { settled: true, changed: true, elapsed, .. } if elapsed == ms(650))
        );
    }

    #[test]
    fn launch_waits_for_cold_start() {
        let mut d = Detector::new(Config::launch(), ms(0));
        assert_eq!(done(d.poll(ms(1_000))), None);
        d.feed(frame(200_000.0), ms(1_800));
        assert_eq!(done(d.poll(ms(2_049))), None);
        assert_eq!(done(d.poll(ms(2_050))), Some((true, true)));
    }

    #[test]
    fn snapshot_mode_only_waits_for_quiet() {
        let mut d = Detector::new(Config::snapshot(), ms(0));
        assert_eq!(done(d.poll(ms(249))), None);
        assert_eq!(done(d.poll(ms(250))), Some((true, false)));
    }

    #[test]
    fn window_changes_extend_quiet() {
        let mut d = Detector::new(Config::action(), ms(0));
        d.feed(Input::WindowChanged, ms(100));
        d.feed(Input::WindowChanged, ms(300));
        assert_eq!(done(d.poll(ms(549))), None);
        assert_eq!(done(d.poll(ms(550))), Some((true, true)));
    }

    #[test]
    fn small_changes_are_not_significant() {
        let mut d = Detector::new(Config::action(), ms(0));
        let mut t = 0;
        while t < 600 {
            d.feed(frame(96.0), ms(t));
            if t == 495 {
                assert_eq!(done(d.poll(ms(t))), None);
            }
            t += 33;
        }
        assert_eq!(done(d.poll(ms(600))), Some((true, false)));
    }

    #[test]
    fn a_spinner_sized_change_counts_once_then_never_blocks_quiet() {
        let mut d = Detector::new(Config::action(), ms(0));
        let mut t = 0;
        while t < 2_000 {
            d.feed(frame(480.0), ms(t));
            t += 33;
        }
        assert_eq!(done(d.poll(ms(250))), Some((true, true)));
    }

    #[test]
    fn endless_change_times_out_with_the_changing_box() {
        let mut d = Detector::new(Config::action(), ms(0));
        let mut t = 0;
        while t <= 5_000 {
            d.feed(
                Input::Frame {
                    changed_area_pt2: 50_000.0,
                    changed_box: Rect {
                        x: (t % 2) as f32 * 10.0,
                        y: 100.0,
                        w: 250.0,
                        h: 200.0,
                    },
                },
                ms(t),
            );
            t += 33;
        }
        match d.poll(ms(5_000)) {
            State::Done {
                settled: false,
                changed: true,
                still_changing: Some(r),
                elapsed,
            } => {
                assert_eq!(elapsed, ms(5_000));
                assert_eq!(
                    r,
                    Rect {
                        x: 0.0,
                        y: 100.0,
                        w: 260.0,
                        h: 200.0
                    }
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn result_latches() {
        let mut d = Detector::new(Config::snapshot(), ms(0));
        assert!(done(d.poll(ms(300))).is_some());
        d.feed(Input::WindowChanged, ms(310));
        assert_eq!(done(d.poll(ms(320))), Some((true, false)));
    }

    proptest! {
        #[test]
        fn settles_between_last_significant_plus_quiet_and_timeout(
            inputs in prop::collection::vec((0u64..6_000, prop_oneof![Just(None), (0f32..60_000.0).prop_map(Some)]), 0..60),
            launch in any::<bool>(),
        ) {
            let config = if launch { Config::launch() } else { Config::action() };
            let mut inputs = inputs;
            inputs.sort_by_key(|(t, _)| *t);
            let mut d = Detector::new(config, ms(0));
            let mut last_significant: Option<u64> = None;
            let mut next = 0;
            for now in 0..=6_000u64 {
                while next < inputs.len() && inputs[next].0 <= now {
                    let (t, area) = inputs[next];
                    let input = match area { None => Input::WindowChanged, Some(a) => frame(a) };
                    if area.is_none_or(|a| a > config.significant_area || (last_significant.is_none() && config.expect_change.is_some() && a > config.first_change_area)) { last_significant = Some(t); }
                    d.feed(input, ms(t));
                    next += 1;
                }
                if let State::Done { settled, .. } = d.poll(ms(now)) {
                    prop_assert!(now <= 5_000, "done at {now}, after timeout");
                    if settled {
                        if let Some(l) = last_significant { prop_assert!(now >= l + 250, "settled at {now}, last change {l}"); }
                    } else {
                        prop_assert_eq!(now, 5_000);
                    }
                    break;
                }
                prop_assert!(now < 5_000, "still waiting at timeout");
            }
        }
    }
}
