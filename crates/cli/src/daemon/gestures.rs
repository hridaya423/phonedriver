//! Finger paths for `swipe` (architecture 10.1).

use phone_use_core::device::{Gesture, TimedPoint};
use phone_use_core::geometry::{Point, Size};

use crate::wire::Dir;

const STEPS: u32 = 10;
const MOVE_MS: u32 = 300;
const HOLD_MS: u32 = 100;
const FAST_MOVE_MS: u32 = 100;

/// A one-finger swipe in `dir` (the way the finger moves) over `distance` of the screen's extent on that axis.
/// From `from` when given, else centered on the screen. A normal swipe holds still before lifting, so lists stop
/// where the finger stopped; a fast one lifts while moving, so they fling.
pub fn swipe(screen: Size, from: Option<Point>, dir: Dir, distance: f32, fast: bool) -> Gesture {
    let distance = distance.clamp(0.05, 0.95);
    let (dx, dy) = match dir {
        Dir::Up => (0.0, -screen.h * distance),
        Dir::Down => (0.0, screen.h * distance),
        Dir::Left => (-screen.w * distance, 0.0),
        Dir::Right => (screen.w * distance, 0.0),
    };
    let start = from.unwrap_or(Point {
        x: screen.w / 2.0 - dx / 2.0,
        y: screen.h / 2.0 - dy / 2.0,
    });
    let clamp = |p: Point| Point {
        x: p.x.clamp(1.0, screen.w - 1.0),
        y: p.y.clamp(1.0, screen.h - 1.0),
    };
    let move_ms = if fast { FAST_MOVE_MS } else { MOVE_MS };
    let mut path: Vec<TimedPoint> = (0..=STEPS)
        .map(|s| {
            let f = s as f32 / STEPS as f32;
            TimedPoint {
                at: clamp(Point {
                    x: start.x + dx * f,
                    y: start.y + dy * f,
                }),
                t_ms: move_ms * s / STEPS,
            }
        })
        .collect();
    if !fast && let Some(last) = path.last().copied() {
        path.push(TimedPoint {
            at: last.at,
            t_ms: move_ms + HOLD_MS,
        });
    }
    Gesture {
        fingers: vec![path],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Size = Size { w: 400.0, h: 800.0 };

    #[test]
    fn swipe_up_moves_the_finger_up_by_half_the_height_around_the_center() {
        let g = swipe(SCREEN, None, Dir::Up, 0.5, false);
        let path = &g.fingers[0];
        assert_eq!(path[0].at, Point { x: 200.0, y: 600.0 });
        assert_eq!(path[STEPS as usize].at, Point { x: 200.0, y: 200.0 });
        assert_eq!(path.last().unwrap().t_ms, MOVE_MS + HOLD_MS);
        assert!(path.windows(2).all(|w| w[1].t_ms >= w[0].t_ms));
    }

    #[test]
    fn a_fast_swipe_lifts_while_moving() {
        let g = swipe(SCREEN, None, Dir::Left, 0.5, true);
        let path = &g.fingers[0];
        assert_eq!(path.len(), STEPS as usize + 1);
        let (a, b) = (path[path.len() - 2], path[path.len() - 1]);
        let speed = (a.at.x - b.at.x) / ((b.t_ms - a.t_ms) as f32 / 1000.0);
        assert!(speed > 1000.0, "{speed} pt/s");
    }

    #[test]
    fn a_swipe_from_an_element_starts_at_it_and_stays_on_screen() {
        let from = Point { x: 50.0, y: 700.0 };
        let g = swipe(SCREEN, Some(from), Dir::Down, 0.5, false);
        let path = &g.fingers[0];
        assert_eq!(path[0].at, from);
        assert!(path.iter().all(|p| p.at.y <= 799.0));
    }
}
