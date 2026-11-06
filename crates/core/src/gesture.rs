//! The timed finger paths behind `swipe` and `scroll` (architecture 10.1).

use crate::device::{Gesture, TimedPoint};
use crate::error::Direction;
use crate::geometry::{Point, Rect};

/// A scroll is a drag that moves in this many steps over `MOVE_MS`, then holds still for `HOLD_MS` before the lift,
/// so the list stops under the finger instead of flinging (competitor `android.py:444-478`).
const STEPS: u32 = 10;
const MOVE_MS: u32 = 250;
const HOLD_MS: u32 = 150;
const DOUBLE_TAP_MS: u32 = 80;
/// Android 8 flings a list that lifts after a gap with no touch samples; stationary samples every 25 ms read as stopped.
const HOLD_SAMPLE_MS: u32 = 25;
/// A fast swipe lifts while still moving.
const FAST_STEPS: u32 = 6;
const FAST_MS: u32 = 90;
/// A scroll moves the finger this share of the container, starting 20% from its edge.
pub const SCROLL_DISTANCE: f32 = 0.6;

fn along(d: Direction) -> (f32, f32) {
    match d {
        Direction::Up => (0.0, -1.0),
        Direction::Down => (0.0, 1.0),
        Direction::Left => (-1.0, 0.0),
        Direction::Right => (1.0, 0.0),
    }
}

/// One finger from `from` to `to`, then still for `hold_ms`.
pub fn drag(from: Point, to: Point, steps: u32, move_ms: u32, hold_ms: u32) -> Gesture {
    let mut path: Vec<TimedPoint> = (0..=steps)
        .map(|i| {
            let f = i as f32 / steps as f32;
            TimedPoint {
                at: Point {
                    x: from.x + (to.x - from.x) * f,
                    y: from.y + (to.y - from.y) * f,
                },
                t_ms: move_ms * i / steps,
            }
        })
        .collect();
    if hold_ms > 0 {
        let mut t = HOLD_SAMPLE_MS;
        while t < hold_ms {
            path.push(TimedPoint {
                at: to,
                t_ms: move_ms + t,
            });
            t += HOLD_SAMPLE_MS;
        }
        path.push(TimedPoint {
            at: to,
            t_ms: move_ms + hold_ms,
        });
    }
    Gesture {
        fingers: vec![path],
    }
}

/// The finger moves in `direction` across `distance` (0 to 1) of the container, centered in it.
pub fn swipe(area: Rect, direction: Direction, distance: f32, fast: bool) -> Gesture {
    let (dx, dy) = along(direction);
    let half = length(area, direction, distance) / 2.0;
    let c = area.center();
    path(
        Point {
            x: c.x - dx * half,
            y: c.y - dy * half,
        },
        direction,
        length(area, direction, distance),
        fast,
    )
}

/// Like `swipe`, but the finger goes down at `start`.
pub fn swipe_from(
    start: Point,
    area: Rect,
    direction: Direction,
    distance: f32,
    fast: bool,
) -> Gesture {
    path(start, direction, length(area, direction, distance), fast)
}

fn length(area: Rect, direction: Direction, distance: f32) -> f32 {
    let (dx, _) = along(direction);
    let extent = if dx != 0.0 { area.w } else { area.h };
    extent * distance.clamp(0.0, 1.0)
}

fn path(from: Point, direction: Direction, len: f32, fast: bool) -> Gesture {
    let (dx, dy) = along(direction);
    let to = Point {
        x: from.x + dx * len,
        y: from.y + dy * len,
    };
    if fast {
        drag(from, to, FAST_STEPS, FAST_MS, 0)
    } else {
        drag(from, to, STEPS, MOVE_MS, HOLD_MS)
    }
}

/// Shows more of what lies in `direction`: the finger moves the other way, 60% of the container, and stops before it lifts.
pub fn scroll(area: Rect, direction: Direction) -> Gesture {
    swipe(area, direction.opposite(), SCROLL_DISTANCE, false)
}

/// One tap, or `count` taps 80 ms apart, or one press that lifts after `long_ms`.
pub fn tap(at: Point, count: u32, long_ms: Option<u32>) -> Gesture {
    let up = long_ms.unwrap_or(0);
    let fingers = (0..count.max(1))
        .map(|i| {
            let t = i * (up + DOUBLE_TAP_MS);
            vec![TimedPoint { at, t_ms: t }, TimedPoint { at, t_ms: t + up }]
        })
        .collect();
    Gesture { fingers }
}

const PICK_UP_MS: u32 = 400;
const DRAG_MS: u32 = 400;
const DROP_MS: u32 = 100;
const PINCH_MS: u32 = 400;
const PINCH_STEPS: u32 = 10;
const PINCH_NEAR: f32 = 0.04;

/// Presses at `from` and holds so the app starts a drag (a list reorder, a drag and drop), moves to `to`, holds
/// there, then lifts.
pub fn press_drag(from: Point, to: Point) -> Gesture {
    let mut path: Vec<TimedPoint> = (0..PICK_UP_MS / HOLD_SAMPLE_MS)
        .map(|i| TimedPoint {
            at: from,
            t_ms: i * HOLD_SAMPLE_MS,
        })
        .collect();
    let moved = drag(from, to, STEPS, DRAG_MS, DROP_MS);
    path.extend(moved.fingers[0].iter().map(|p| TimedPoint {
        at: p.at,
        t_ms: p.t_ms + PICK_UP_MS,
    }));
    Gesture {
        fingers: vec![path],
    }
}

/// Two fingers on a horizontal line through `center`. `out` spreads them from close together to `distance` (0 to 1)
/// of the screen's shorter side apart, zooming in; otherwise they pinch from there to close together.
pub fn pinch(center: Point, screen: Rect, out: bool, distance: f32) -> Gesture {
    let side = screen.w.min(screen.h);
    let room = (center.x - screen.x)
        .min(screen.x + screen.w - center.x)
        .max(0.0);
    let near = side * PINCH_NEAR;
    let far = (side * distance.clamp(0.1, 0.9) / 2.0)
        .min((room - 4.0).max(near))
        .max(near);
    let (r0, r1) = if out { (near, far) } else { (far, near) };
    let finger = |sign: f32| -> Vec<TimedPoint> {
        (0..=PINCH_STEPS)
            .map(|i| {
                let f = i as f32 / PINCH_STEPS as f32;
                TimedPoint {
                    at: Point {
                        x: center.x + sign * (r0 + (r1 - r0) * f),
                        y: center.y,
                    },
                    t_ms: PINCH_MS * i / PINCH_STEPS,
                }
            })
            .collect()
    };
    Gesture {
        fingers: vec![finger(-1.0), finger(1.0)],
    }
}

/// A drag that ends moving fast enough to fling the list, for crossing long distances.
pub fn fling(area: Rect, direction: Direction) -> Gesture {
    swipe(area, direction.opposite(), 0.8, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect {
        x: 0.0,
        y: 100.0,
        w: 400.0,
        h: 800.0,
    };

    fn path(g: &Gesture) -> &[TimedPoint] {
        &g.fingers[0]
    }

    #[test]
    fn scrolling_down_drags_up_from_20_percent_below_the_middle_and_holds_before_the_lift() {
        let g = scroll(AREA, Direction::Down);
        let p = path(&g);
        assert_eq!(g.fingers.len(), 1);
        assert_eq!((p[0].at.x, p[0].t_ms), (200.0, 0));
        assert!((p[0].at.y - 740.0).abs() < 0.01 && (p[10].at.y - 260.0).abs() < 0.01);
        assert_eq!(p[10].t_ms, 250);
        let last = p.last().unwrap();
        assert_eq!((last.at.y, last.t_ms), (p[10].at.y, 400));
        let held: Vec<u32> = p[11..].iter().map(|t| t.t_ms).collect();
        assert!(
            held.windows(2).all(|w| w[1] - w[0] <= 25) && held[0] <= 275,
            "{held:?}"
        );
        assert!(p.windows(2).all(|w| w[1].at.y <= w[0].at.y));
        assert!(p.windows(2).all(|w| w[1].t_ms >= w[0].t_ms));
    }

    #[test]
    fn a_drag_and_drop_holds_before_it_moves_and_after_it_arrives() {
        let g = press_drag(Point { x: 50.0, y: 200.0 }, Point { x: 300.0, y: 600.0 });
        let p = path(&g);
        assert_eq!(g.fingers.len(), 1);
        let first_move = p.iter().find(|t| t.at != p[0].at).unwrap();
        assert!(first_move.t_ms >= 400, "{first_move:?}");
        assert!(p[..16].iter().all(|t| t.at == p[0].at));
        let last = p.last().unwrap();
        assert_eq!((last.at.x, last.at.y), (300.0, 600.0));
        assert!(last.t_ms >= 400 + 400 + 100);
        assert!(p.windows(2).all(|w| w[1].t_ms >= w[0].t_ms));
    }

    #[test]
    fn a_pinch_out_spreads_two_fingers_apart_and_a_pinch_in_closes_them() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 800.0,
        };
        let c = Point { x: 200.0, y: 400.0 };
        let out = pinch(c, screen, true, 0.5);
        assert_eq!(out.fingers.len(), 2);
        let gap = |g: &Gesture, i: usize| g.fingers[1][i].at.x - g.fingers[0][i].at.x;
        assert!(gap(&out, 0) < 40.0 && (gap(&out, 10) - 200.0).abs() < 0.01);
        let inn = pinch(c, screen, false, 0.5);
        assert!((gap(&inn, 0) - 200.0).abs() < 0.01 && gap(&inn, 10) < 40.0);
        assert!(out.fingers[0].iter().all(|t| t.at.y == 400.0));
        let edge = pinch(Point { x: 20.0, y: 400.0 }, screen, true, 0.9);
        assert!(edge.fingers[0].iter().all(|t| t.at.x >= 0.0));
    }

    #[test]
    fn a_fast_swipe_lifts_while_moving_at_over_a_thousand_points_a_second() {
        let g = swipe(AREA, Direction::Left, 0.5, true);
        let p = path(&g);
        let n = p.len();
        assert!(
            p.windows(2).all(|w| w[1].at.x < w[0].at.x),
            "monotonic left"
        );
        let last =
            (p[n - 2].at.x - p[n - 1].at.x) / ((p[n - 1].t_ms - p[n - 2].t_ms) as f32 / 1000.0);
        assert!(last > 1000.0, "{last}");
    }

    #[test]
    fn a_swipe_from_a_point_starts_exactly_there() {
        let g = swipe_from(Point { x: 50.0, y: 600.0 }, AREA, Direction::Up, 0.5, false);
        assert_eq!((path(&g)[0].at.x, path(&g)[0].at.y), (50.0, 600.0));
        assert!((path(&g)[10].at.y - 200.0).abs() < 0.01);
    }

    #[test]
    fn swipe_distance_is_a_share_of_the_container_along_the_axis() {
        let g = swipe(AREA, Direction::Up, 0.5, false);
        let p = path(&g);
        assert!((p[0].at.y - p[10].at.y - 400.0).abs() < 0.01);
        let g = swipe(AREA, Direction::Right, 0.5, false);
        assert!((path(&g)[10].at.x - path(&g)[0].at.x - 200.0).abs() < 0.01);
    }

    #[test]
    fn a_double_tap_is_two_fingers_80_ms_apart_that_never_overlap() {
        let g = tap(Point { x: 10.0, y: 20.0 }, 2, None);
        assert_eq!(g.fingers.len(), 2);
        assert_eq!(g.fingers[0].first().unwrap().t_ms, 0);
        assert_eq!(g.fingers[0].last().unwrap().t_ms, 0);
        assert_eq!(g.fingers[1].first().unwrap().t_ms, 80);
        assert_eq!(g.fingers[1].last().unwrap().t_ms, 80);
    }

    #[test]
    fn a_long_press_lifts_after_the_requested_time() {
        let g = tap(Point { x: 10.0, y: 20.0 }, 1, Some(800));
        let p = path(&g);
        assert_eq!((p[0].t_ms, p.last().unwrap().t_ms), (0, 800));
        assert_eq!(p[0].at, p.last().unwrap().at);
    }
}
