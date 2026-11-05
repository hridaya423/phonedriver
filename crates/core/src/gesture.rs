//! The timed finger paths behind `swipe` and `scroll` (architecture 10.1).

use crate::device::{Gesture, TimedPoint};
use crate::error::Direction;
use crate::geometry::{Point, Rect};

/// A scroll is a drag that moves in this many steps over `MOVE_MS`, then holds still for `HOLD_MS` before the lift,
/// so the list stops under the finger instead of flinging (competitor `android.py:444-478`).
const STEPS: u32 = 10;
const MOVE_MS: u32 = 250;
const HOLD_MS: u32 = 150;
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
}
