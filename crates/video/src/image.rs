//! Screenshots: the latest frame, scaled and encoded on the host.

use crate::frame_store::Frame;

#[derive(Debug, thiserror::Error)]
#[error("PNG encoding failed: {0}")]
pub struct EncodeError(String);

/// Source columns `[x0, x1)` that make output pixel `o` of `out` pixels over `src` pixels.
fn spans(src: usize, out: usize) -> Vec<(usize, usize)> {
    (0..out)
        .map(|o| {
            let a = o * src / out;
            (a, ((o + 1) * src).div_ceil(out).max(a + 1).min(src))
        })
        .collect()
}

/// Box-filters one plane from `sw` by `sh` down (or nearest up) to `ow` by `oh`.
fn resample(src: &[u8], sw: usize, sh: usize, ow: usize, oh: usize) -> Vec<u8> {
    let columns = spans(sw, ow);
    let mut out = Vec::with_capacity(ow * oh);
    let mut rows = vec![0u32; sw];
    for (y0, y1) in spans(sh, oh) {
        rows.fill(0);
        for row in y0..y1 {
            for (sum, &p) in rows.iter_mut().zip(&src[row * sw..(row + 1) * sw]) {
                *sum += u32::from(p);
            }
        }
        let height = y1 - y0;
        out.extend(columns.iter().map(|&(x0, x1)| {
            let sum: u32 = rows[x0..x1].iter().sum();
            (sum / (height * (x1 - x0)) as u32) as u8
        }));
    }
    out
}

/// The size in pixels of a `width` by `height` image scaled by `scale`.
pub fn scaled_size(width: u32, height: u32, scale: f32) -> (u32, u32) {
    (
        ((width as f32 * scale).round() as u32).max(1),
        ((height as f32 * scale).round() as u32).max(1),
    )
}

/// PNG of `frame` at `scale` times its pixel size (BT.601 limited range, as the agent's encoder writes it).
pub fn encode_png(frame: &Frame, scale: f32) -> Result<Vec<u8>, EncodeError> {
    let frame = &*frame.upright();
    let (w, h) = (frame.width as usize, frame.height as usize);
    let (ow, oh) = scaled_size(frame.width, frame.height, scale);
    let (ow, oh) = (ow as usize, oh as usize);
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let y = resample(&frame.y, w, h, ow, oh);
    let u = resample(&frame.u, cw, ch, ow, oh);
    let v = resample(&frame.v, cw, ch, ow, oh);
    let mut rgb = Vec::with_capacity(ow * oh * 3);
    for i in 0..ow * oh {
        let c = 298 * (y[i] as i32 - 16);
        let (d, e) = (u[i] as i32 - 128, v[i] as i32 - 128);
        rgb.push(((c + 409 * e + 128) >> 8).clamp(0, 255) as u8);
        rgb.push(((c - 100 * d - 208 * e + 128) >> 8).clamp(0, 255) as u8);
        rgb.push(((c + 516 * d + 128) >> 8).clamp(0, 255) as u8);
    }
    let mut png = Vec::new();
    let mut enc = png::Encoder::new(&mut png, ow as u32, oh as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    enc.write_header()
        .and_then(|mut wr| wr.write_image_data(&rgb))
        .map_err(|e| EncodeError(e.to_string()))?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_filter_per_pixel(src: &[u8], sw: usize, sh: usize, ow: usize, oh: usize) -> Vec<u8> {
        let mut out = vec![0u8; ow * oh];
        for oy in 0..oh {
            let y0 = oy * sh / oh;
            let y1 = ((oy + 1) * sh).div_ceil(oh).max(y0 + 1).min(sh);
            for ox in 0..ow {
                let x0 = ox * sw / ow;
                let x1 = ((ox + 1) * sw).div_ceil(ow).max(x0 + 1).min(sw);
                let mut sum = 0u32;
                for row in y0..y1 {
                    sum += src[row * sw + x0..row * sw + x1]
                        .iter()
                        .map(|&p| u32::from(p))
                        .sum::<u32>();
                }
                out[oy * ow + ox] = (sum / ((y1 - y0) * (x1 - x0)) as u32) as u8;
            }
        }
        out
    }

    #[test]
    fn resample_matches_the_per_pixel_box_filter_down_and_up() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut noise = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 32) as u8
        };
        for (sw, sh, ow, oh) in [
            (1080, 2424, 411, 923),
            (541, 1212, 206, 462),
            (7, 5, 3, 2),
            (4, 4, 4, 4),
            (3, 3, 10, 9),
            (100, 1, 7, 1),
            (1, 100, 1, 7),
        ] {
            let src: Vec<u8> = (0..sw * sh).map(|_| noise()).collect();
            assert_eq!(
                resample(&src, sw, sh, ow, oh),
                box_filter_per_pixel(&src, sw, sh, ow, oh),
                "{sw}x{sh} to {ow}x{oh}"
            );
        }
    }
}
