use serde::{Deserialize, Serialize};

/// A position in logical points (iOS points, Android dp), origin top-left.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// A width and height in logical points.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

/// An axis-aligned rectangle in logical points.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn center(&self) -> Point {
        Point {
            x: self.x + self.w / 2.0,
            y: self.y + self.h / 2.0,
        }
    }

    pub fn area(&self) -> f32 {
        self.w * self.h
    }

    /// The part of `self` inside `bounds`, or `None` when nothing of it is visible.
    pub fn clip(&self, bounds: Rect) -> Option<Rect> {
        let x0 = self.x.max(bounds.x);
        let y0 = self.y.max(bounds.y);
        let x1 = (self.x + self.w).min(bounds.x + bounds.w);
        let y1 = (self.y + self.h).min(bounds.y + bounds.h);
        (x1 > x0 && y1 > y0).then_some(Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        })
    }

    /// The smallest rectangle containing both.
    pub fn union(&self, other: Rect) -> Rect {
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = (self.x + self.w).max(other.x + other.w);
        let y1 = (self.y + self.h).max(other.y + other.h);
        Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
    }

    pub fn contains(&self, other: Rect) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.x + other.w <= self.x + self.w
            && other.y + other.h <= self.y + self.h
    }
}

impl Point {
    pub fn distance(&self, other: Point) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

/// Conversion between logical points and device units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Scale {
    /// Device pixels per logical point.
    pub density: f32,
}

impl Scale {
    pub fn to_device(&self, p: Point) -> Point {
        Point {
            x: p.x * self.density,
            y: p.y * self.density,
        }
    }

    pub fn to_points(&self, p: Point) -> Point {
        Point {
            x: p.x / self.density,
            y: p.y / self.density,
        }
    }

    /// iOS HID digitizer coordinates: 0..=65535 across the screen on each axis.
    pub fn to_hid(&self, p: Point, screen: Size) -> (u16, u16) {
        let axis = |v: f32, max: f32| ((v / max).clamp(0.0, 1.0) * 65535.0).round() as u16;
        (axis(p.x, screen.w), axis(p.y, screen.h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn center_of_rect() {
        let r = Rect {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 40.0,
        };
        assert_eq!(r.center(), Point { x: 60.0, y: 40.0 });
    }

    #[test]
    fn clip_keeps_visible_part() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 800.0,
        };
        let half = Rect {
            x: 350.0,
            y: 100.0,
            w: 100.0,
            h: 50.0,
        };
        assert_eq!(
            half.clip(screen),
            Some(Rect {
                x: 350.0,
                y: 100.0,
                w: 50.0,
                h: 50.0
            })
        );
        let gone = Rect {
            x: 500.0,
            y: 100.0,
            w: 100.0,
            h: 50.0,
        };
        assert_eq!(gone.clip(screen), None);
    }

    #[test]
    fn to_device_scales_by_density() {
        let s = Scale { density: 2.625 };
        assert_eq!(
            s.to_device(Point { x: 100.0, y: 200.0 }),
            Point { x: 262.5, y: 525.0 }
        );
    }

    #[test]
    fn to_hid_maps_corners() {
        let s = Scale { density: 3.0 };
        let screen = Size { w: 393.0, h: 852.0 };
        assert_eq!(s.to_hid(Point { x: 0.0, y: 0.0 }, screen), (0, 0));
        assert_eq!(
            s.to_hid(Point { x: 393.0, y: 852.0 }, screen),
            (65535, 65535)
        );
    }

    proptest! {
        #[test]
        fn device_round_trip(x in 0f32..10_000.0, y in 0f32..10_000.0, d in 0.75f32..4.0) {
            let s = Scale { density: d };
            let p = Point { x, y };
            let back = s.to_points(s.to_device(p));
            prop_assert!((back.x - x).abs() < 0.01 && (back.y - y).abs() < 0.01);
        }
    }
}
