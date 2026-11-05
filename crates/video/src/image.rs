//! Screenshots: the latest frame, scaled and encoded on the host.

use crate::frame_store::Frame;

#[derive(Debug, thiserror::Error)]
#[error("PNG encoding failed: {0}")]
pub struct EncodeError(String);

/// Box-filters one plane from `sw` by `sh` down (or nearest up) to `ow` by `oh`.
fn resample(src: &[u8], sw: usize, sh: usize, ow: usize, oh: usize) -> Vec<u8> {
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
                    .map(|&p| p as u32)
                    .sum::<u32>();
            }
            out[oy * ow + ox] = (sum / ((y1 - y0) * (x1 - x0)) as u32) as u8;
        }
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
