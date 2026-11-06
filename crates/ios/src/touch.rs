//! Gestures and keys as HID input for the CoreDevice tier (architecture 7.2).

use idevice::core_device::TouchscreenContact;
use phone_use_core::device::{Gesture, Key};
use phone_use_core::geometry::{Point, Size};

/// Time between touch reports while a finger moves (120 Hz).
pub const TICK_MS: u32 = 8;
/// A tap has no duration of its own; the finger stays down this long.
pub const TAP_HOLD_MS: u32 = 30;
const SCALE: f32 = 65535.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub at_ms: u32,
    pub contacts: Vec<TouchscreenContact>,
}

fn normalized(p: Point, screen: Size) -> (u16, u16) {
    let unit = |v: f32, extent: f32| (v / extent * SCALE).round().clamp(0.0, SCALE) as u16;
    (unit(p.x, screen.w), unit(p.y, screen.h))
}

fn position_at(path: &[phone_use_core::device::TimedPoint], t: u32) -> Point {
    let Some(next) = path.iter().position(|p| p.t_ms >= t) else {
        return path[path.len() - 1].at;
    };
    if next == 0 || path[next].t_ms == t {
        return path[next].at;
    }
    let (a, b) = (path[next - 1], path[next]);
    let f = (t - a.t_ms) as f32 / (b.t_ms - a.t_ms) as f32;
    Point {
        x: a.at.x + (b.at.x - a.at.x) * f,
        y: a.at.y + (b.at.y - a.at.y) * f,
    }
}

/// The touch reports that play `g` on a screen of `screen` points: contacts at every tick while any finger is
/// down, and one last report with every contact lifted.
pub fn schedule(g: &Gesture, screen: Size) -> Vec<Step> {
    let fingers: Vec<_> = g.fingers.iter().filter(|f| !f.is_empty()).take(5).collect();
    let longest = fingers
        .iter()
        .map(|f| f[f.len() - 1].t_ms)
        .max()
        .unwrap_or(0);
    let duration = if longest == 0 { TAP_HOLD_MS } else { longest };
    let contacts = |t: u32, touching: bool| -> Vec<TouchscreenContact> {
        fingers
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let (x, y) = normalized(position_at(path, t), screen);
                TouchscreenContact {
                    identity: i as u8,
                    touching,
                    x,
                    y,
                }
            })
            .collect()
    };
    let mut steps = Vec::new();
    let mut t = 0;
    while t < duration {
        steps.push(Step {
            at_ms: t,
            contacts: contacts(t, true),
        });
        t += TICK_MS;
    }
    steps.push(Step {
        at_ms: duration,
        contacts: contacts(duration, false),
    });
    steps
}

pub const MOD_SHIFT: u16 = 0xE1;
pub const MOD_GUI: u16 = 0xE3;
pub const USAGE_V: u16 = 0x19;

pub fn key_usage(k: Key) -> u16 {
    match k {
        Key::Enter => 0x28,
        Key::Escape => 0x29,
        Key::Backspace => 0x2A,
        Key::Tab => 0x2B,
        Key::ArrowRight => 0x4F,
        Key::ArrowLeft => 0x50,
        Key::ArrowDown => 0x51,
        Key::ArrowUp => 0x52,
    }
}

/// The HID keyboard usage and whether Shift is held for a US-layout character; `None` for characters the US
/// layout cannot type.
pub fn char_usage(c: char) -> Option<(u16, bool)> {
    Some(match c {
        'a'..='z' => (0x04 + (c as u16 - 'a' as u16), false),
        'A'..='Z' => (0x04 + (c as u16 - 'A' as u16), true),
        '1'..='9' => (0x1E + (c as u16 - '1' as u16), false),
        '0' => (0x27, false),
        '!' => (0x1E, true),
        '@' => (0x1F, true),
        '#' => (0x20, true),
        '$' => (0x21, true),
        '%' => (0x22, true),
        '^' => (0x23, true),
        '&' => (0x24, true),
        '*' => (0x25, true),
        '(' => (0x26, true),
        ')' => (0x27, true),
        '\n' => (0x28, false),
        '\t' => (0x2B, false),
        ' ' => (0x2C, false),
        '-' => (0x2D, false),
        '_' => (0x2D, true),
        '=' => (0x2E, false),
        '+' => (0x2E, true),
        '[' => (0x2F, false),
        '{' => (0x2F, true),
        ']' => (0x30, false),
        '}' => (0x30, true),
        '\\' => (0x31, false),
        '|' => (0x31, true),
        ';' => (0x33, false),
        ':' => (0x33, true),
        '\'' => (0x34, false),
        '"' => (0x34, true),
        '`' => (0x35, false),
        '~' => (0x35, true),
        ',' => (0x36, false),
        '<' => (0x36, true),
        '.' => (0x37, false),
        '>' => (0x37, true),
        '/' => (0x38, false),
        '?' => (0x38, true),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use phone_use_core::device::TimedPoint;

    use super::*;

    const SCREEN: Size = Size {
        w: 1000.0,
        h: 2000.0,
    };

    fn at(x: f32, y: f32, t_ms: u32) -> TimedPoint {
        TimedPoint {
            at: Point { x, y },
            t_ms,
        }
    }

    #[test]
    fn a_tap_is_down_then_up_after_the_hold() {
        let g = Gesture {
            fingers: vec![vec![at(500.0, 1000.0, 0), at(500.0, 1000.0, 0)]],
        };
        let steps = schedule(&g, SCREEN);
        let first = &steps[0];
        assert_eq!(first.at_ms, 0);
        assert!(first.contacts[0].touching);
        assert_eq!((first.contacts[0].x, first.contacts[0].y), (32768, 32768));
        let last = steps.last().unwrap();
        assert_eq!(last.at_ms, TAP_HOLD_MS);
        assert!(!last.contacts[0].touching);
        assert!(steps.windows(2).all(|w| w[1].at_ms > w[0].at_ms));
    }

    #[test]
    fn a_swipe_moves_in_a_line_and_lifts_at_the_end() {
        let g = Gesture {
            fingers: vec![vec![at(0.0, 0.0, 0), at(1000.0, 0.0, 80)]],
        };
        let steps = schedule(&g, SCREEN);
        let mid = steps.iter().find(|s| s.at_ms == 40).unwrap();
        assert_eq!(mid.contacts[0].x, 32768);
        let last = steps.last().unwrap();
        assert_eq!(
            (last.at_ms, last.contacts[0].x, last.contacts[0].touching),
            (80, 65535, false)
        );
    }

    #[test]
    fn two_fingers_keep_their_identities_and_the_shorter_one_holds_its_last_point() {
        let g = Gesture {
            fingers: vec![
                vec![at(100.0, 100.0, 0), at(100.0, 100.0, 24)],
                vec![at(900.0, 100.0, 0), at(500.0, 100.0, 80)],
            ],
        };
        let steps = schedule(&g, SCREEN);
        let late = steps.iter().find(|s| s.at_ms == 56).unwrap();
        assert_eq!(late.contacts.len(), 2);
        assert_eq!(late.contacts[0].identity, 0);
        assert_eq!(late.contacts[0].x, 6554);
        assert_eq!(late.contacts[1].identity, 1);
    }

    #[test]
    fn points_off_the_screen_are_clamped() {
        let g = Gesture {
            fingers: vec![vec![at(-5.0, 5000.0, 0), at(-5.0, 5000.0, 0)]],
        };
        let c = &schedule(&g, SCREEN)[0].contacts[0];
        assert_eq!((c.x, c.y), (0, 65535));
    }

    #[test]
    fn letters_digits_and_symbols_map_to_us_layout_usages() {
        assert_eq!(char_usage('a'), Some((0x04, false)));
        assert_eq!(char_usage('Z'), Some((0x1D, true)));
        assert_eq!(char_usage('0'), Some((0x27, false)));
        assert_eq!(char_usage('?'), Some((0x38, true)));
        assert_eq!(char_usage('é'), None);
        assert_eq!(key_usage(Key::Enter), 0x28);
    }
}
