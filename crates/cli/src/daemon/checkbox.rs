//! Whether a web checkbox or radio button is ticked, read from the screenshot.
//!
//! Chromium's accessibility node for a web page's checkbox or radio button says neither checkable nor checked (checked,
//! checkable, `getChecked`, state description and extras all stay empty on API 37, before and after it is toggled), so
//! the tree cannot tell a ticked one from an empty one. A ticked box is drawn filled with the page's accent color, a
//! ticked radio has a dot of it, an empty one has neither.

use phonedriver_core::error::{Error, Op};
use phonedriver_core::geometry::Rect;

const INSET: f32 = 0.2;
const COLORED_SHARE: f32 = 0.2;
const COLORED_SPREAD: i32 = 60;

fn unreadable() -> Error {
    Error::Unsupported {
        op: Op::Screenshot,
        reason: "the screenshot could not be read to tell whether a checkbox is ticked",
    }
}

/// A box can be read from the screenshot only when all of it is drawn there: Chrome zoomed in on a focused field
/// reports the checkboxes past the left edge, and their pixels were read from whatever the edge showed.
pub fn readable(b: Rect, screen: Rect, keyboard: Option<Rect>) -> bool {
    let bottom = keyboard.map_or(screen.y + screen.h, |k| k.y);
    b.x >= screen.x && b.y >= screen.y && b.x + b.w <= screen.x + screen.w && b.y + b.h <= bottom
}

/// For each box in pixels, whether most of its inside is a saturated color. `png` is a screenshot at one pixel per point.
pub fn ticked(png: &[u8], boxes: &[Rect]) -> Result<Vec<bool>, Error> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .map_err(|_| unreadable())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or_else(unreadable)?];
    let info = reader.next_frame(&mut buf).map_err(|_| unreadable())?;
    let channels = match (info.color_type, info.bit_depth) {
        (png::ColorType::Rgb, png::BitDepth::Eight) => 3,
        (png::ColorType::Rgba, png::BitDepth::Eight) => 4,
        _ => return Err(unreadable()),
    };
    let (w, h) = (info.width as usize, info.height as usize);
    Ok(boxes
        .iter()
        .map(|b| {
            let (dx, dy) = (b.w * INSET, b.h * INSET);
            let x0 = (b.x + dx).round().max(0.0) as usize;
            let y0 = (b.y + dy).round().max(0.0) as usize;
            let x1 = ((b.x + b.w - dx).round().max(0.0) as usize).min(w);
            let y1 = ((b.y + b.h - dy).round().max(0.0) as usize).min(h);
            let (mut colored, mut all) = (0usize, 0usize);
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = &buf[(y * w + x) * channels..];
                    let spread =
                        i32::from(p[0].max(p[1]).max(p[2])) - i32::from(p[0].min(p[1]).min(p[2]));
                    colored += usize::from(spread > COLORED_SPREAD);
                    all += 1;
                }
            }
            all > 0 && colored as f32 / all as f32 >= COLORED_SHARE
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_box_wholly_on_screen_and_above_the_keyboard_is_read() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 900.0,
        };
        let keyboard = Some(Rect {
            x: 0.0,
            y: 600.0,
            w: 400.0,
            h: 300.0,
        });
        let at = |x: f32, y: f32| Rect {
            x,
            y,
            w: 17.0,
            h: 17.0,
        };
        assert!(readable(at(10.0, 200.0), screen, keyboard));
        assert!(!readable(at(-5.0, 200.0), screen, keyboard));
        assert!(!readable(at(390.0, 200.0), screen, keyboard));
        assert!(!readable(at(10.0, 590.0), screen, keyboard));
        assert!(readable(at(10.0, 590.0), screen, None));
    }

    fn screen(draw: impl Fn(usize, usize) -> [u8; 3]) -> Vec<u8> {
        let (w, h) = (80usize, 40usize);
        let mut rgb = Vec::new();
        for y in 0..h {
            for x in 0..w {
                rgb.extend(draw(x, y));
            }
        }
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w as u32, h as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .and_then(|mut wr| wr.write_image_data(&rgb))
            .unwrap();
        out
    }

    fn at(x: usize, y: usize, left: usize) -> bool {
        (left..left + 17).contains(&x) && (10..27).contains(&y)
    }

    fn edge(x: usize, y: usize, left: usize) -> bool {
        at(x, y, left) && (x == left || x == left + 16 || y == 10 || y == 26)
    }

    const BLUE: [u8; 3] = [0, 117, 255];
    const WHITE: [u8; 3] = [255, 255, 255];
    const GRAY: [u8; 3] = [118, 118, 118];

    fn box_at(left: f32) -> Rect {
        Rect {
            x: left,
            y: 10.0,
            w: 17.0,
            h: 17.0,
        }
    }

    #[test]
    fn a_box_filled_with_the_accent_color_is_ticked_and_an_empty_one_is_not() {
        let png = screen(|x, y| {
            if at(x, y, 5) && (9..18).contains(&x) && (14..23).contains(&y) {
                WHITE
            } else if at(x, y, 5) && !edge(x, y, 5) {
                BLUE
            } else if edge(x, y, 5) || edge(x, y, 40) {
                GRAY
            } else {
                WHITE
            }
        });
        assert_eq!(
            ticked(&png, &[box_at(5.0), box_at(40.0)]).unwrap(),
            [true, false]
        );
    }

    #[test]
    fn a_radio_is_ticked_when_its_dot_is_colored_and_not_when_only_its_ring_is_drawn() {
        let radio = |x: usize, y: usize, left: usize, dot: bool| {
            let (cx, cy) = (left as f32 + 3.5, 13.5f32);
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            if dot && d < 2.4 {
                BLUE
            } else if d > 2.9 && d < 3.6 {
                if dot { BLUE } else { GRAY }
            } else {
                WHITE
            }
        };
        let png = screen(|x, y| {
            if x < 20 {
                radio(x, y, 5, true)
            } else {
                radio(x, y, 40, false)
            }
        });
        let small = |left: f32| Rect {
            x: left,
            y: 10.0,
            w: 7.0,
            h: 7.0,
        };
        assert_eq!(
            ticked(&png, &[small(5.0), small(40.0)]).unwrap(),
            [true, false]
        );
    }

    #[test]
    fn a_dark_empty_box_is_not_ticked_and_a_box_off_the_screen_reads_as_empty() {
        let png = screen(|x, y| if at(x, y, 5) { [40, 40, 40] } else { WHITE });
        let off = Rect {
            x: 500.0,
            y: 500.0,
            w: 17.0,
            h: 17.0,
        };
        assert_eq!(ticked(&png, &[box_at(5.0), off]).unwrap(), [false, false]);
    }

    #[test]
    fn bytes_that_are_not_a_png_are_an_error() {
        assert!(ticked(b"nope", &[box_at(5.0)]).is_err());
    }
}
