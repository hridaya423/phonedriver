//! Set-of-marks screenshots: boxes and `eN` labels drawn on a PNG.

/// A box on the picture, in image pixels, with the text to print at its corner.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub label: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

const COLORS: [[u8; 3]; 6] = [
    [230, 25, 75],
    [0, 130, 200],
    [60, 180, 75],
    [245, 130, 48],
    [145, 30, 180],
    [0, 128, 128],
];
const SCALE: usize = 2;
const GLYPH_W: usize = 3;
const GLYPH_H: usize = 5;

fn glyph(c: char) -> [u8; 5] {
    match c {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b001, 0b001],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        'e' => [0b111, 0b101, 0b111, 0b100, 0b011],
        _ => [0; 5],
    }
}

struct Canvas {
    w: usize,
    h: usize,
    rgb: Vec<u8>,
}

impl Canvas {
    fn put(&mut self, x: i64, y: i64, c: [u8; 3]) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            let i = (y as usize * self.w + x as usize) * 3;
            self.rgb[i..i + 3].copy_from_slice(&c);
        }
    }

    fn fill(&mut self, x: i64, y: i64, w: usize, h: usize, c: [u8; 3]) {
        for dy in 0..h as i64 {
            for dx in 0..w as i64 {
                self.put(x + dx, y + dy, c);
            }
        }
    }

    fn outline(&mut self, x: i64, y: i64, w: usize, h: usize, c: [u8; 3]) {
        self.fill(x, y, w, 2, c);
        self.fill(x, y + h as i64 - 2, w, 2, c);
        self.fill(x, y, 2, h, c);
        self.fill(x + w as i64 - 2, y, 2, h, c);
    }

    fn label(&mut self, x: i64, y: i64, text: &str, c: [u8; 3]) {
        let n = text.chars().count();
        let (bw, bh) = (n * (GLYPH_W + 1) * SCALE + SCALE, (GLYPH_H + 2) * SCALE);
        self.fill(x, y, bw, bh, c);
        for (i, ch) in text.chars().enumerate() {
            let rows = glyph(ch);
            for (r, bits) in rows.iter().enumerate() {
                for col in 0..GLYPH_W {
                    if bits >> (GLYPH_W - 1 - col) & 1 == 1 {
                        let gx = x + (SCALE + i * (GLYPH_W + 1) * SCALE + col * SCALE) as i64;
                        let gy = y + ((r + 1) * SCALE) as i64;
                        self.fill(gx, gy, SCALE, SCALE, [255, 255, 255]);
                    }
                }
            }
        }
    }
}

/// Draws each mark's outline and label on the PNG `png` and returns the new PNG. Marks are clipped to the image.
pub fn draw_marks(png: &[u8], marks: &[Mark]) -> Result<Vec<u8>, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("the PNG is too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err("only 8-bit PNGs can be marked".into());
    }
    let channels = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Indexed => return Err("indexed PNGs cannot be marked".into()),
    };
    let (w, h) = (info.width as usize, info.height as usize);
    let mut rgb = Vec::with_capacity(w * h * 3);
    for px in buf[..w * h * channels].chunks(channels) {
        match channels {
            1 | 2 => rgb.extend_from_slice(&[px[0]; 3]),
            _ => rgb.extend_from_slice(&px[..3]),
        }
    }
    let mut canvas = Canvas { w, h, rgb };
    for (i, m) in marks.iter().enumerate() {
        let c = COLORS[i % COLORS.len()];
        let (x, y) = (m.x.round() as i64, m.y.round() as i64);
        let (mw, mh) = ((m.w.round() as usize).max(4), (m.h.round() as usize).max(4));
        canvas.outline(x, y, mw, mh, c);
        canvas.label(x, y, &m.label, c);
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut wr| wr.write_image_data(&canvas.rgb))
        .map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgb);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .unwrap()
            .write_image_data(&vec![200; (w * h * 3) as usize])
            .unwrap();
        out
    }

    fn decode(png: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut r = png::Decoder::new(std::io::Cursor::new(png))
            .read_info()
            .unwrap();
        let mut buf = vec![0; r.output_buffer_size().unwrap()];
        let i = r.next_frame(&mut buf).unwrap();
        (i.width, i.height, buf)
    }

    #[test]
    fn a_mark_draws_its_outline_and_label_and_leaves_the_inside_alone() {
        let marks = [Mark {
            label: "e12".into(),
            x: 20.0,
            y: 30.0,
            w: 100.0,
            h: 60.0,
        }];
        let out = draw_marks(&gray(200, 200), &marks).unwrap();
        let (w, h, px) = decode(&out);
        assert_eq!((w, h), (200, 200));
        let at = |x: usize, y: usize| px[(y * 200 + x) * 3..(y * 200 + x) * 3 + 3].to_vec();
        assert_eq!(at(70, 31), COLORS[0], "top edge");
        assert_eq!(at(119, 60), COLORS[0], "right edge");
        assert_eq!(at(70, 70), [200, 200, 200], "inside");
        assert_eq!(at(5, 5), [200, 200, 200], "outside");
        assert!(
            (22..50).any(|x| at(x, 35) == [255, 255, 255]),
            "label glyph pixels are white"
        );
    }

    #[test]
    fn marks_outside_the_picture_are_clipped_and_bad_input_is_an_error() {
        let marks = [Mark {
            label: "e1".into(),
            x: 190.0,
            y: 190.0,
            w: 100.0,
            h: 100.0,
        }];
        assert!(draw_marks(&gray(200, 200), &marks).is_ok());
        assert!(draw_marks(b"not a png", &marks).is_err());
    }
}
