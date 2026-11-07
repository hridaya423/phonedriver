//! The latest decoded frame, its change against the previous one, and stream health.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use phone_use_core::device::FrameRef;
use phone_use_core::geometry::Rect;
use tokio::sync::watch;
use tokio::time::Instant;

/// Architecture 8: with the agent's repeat-frame setting a live stream never goes quiet for longer.
const STALL_AFTER: Duration = Duration::from_secs(1);
/// Architecture 4.6: a thumbnail sample covers 4 by 4 points and counts as changed above 12 luma levels.
const SAMPLE_POINTS: f32 = 4.0;
const CHANGED_LEVELS: i16 = 12;
/// Limited-range video black is luma 16 (spike A3 measured exactly 16 over a FLAG_SECURE window).
const BLACK_LEVEL: u8 = 20;

/// A decoded picture in planar YUV 4:2:0, rows tightly packed.
#[derive(Clone)]
pub struct Frame {
    /// Assigned by `FrameStore::publish`, starting at 1.
    pub seq: u64,
    pub width: u32,
    pub height: u32,
    pub(crate) y: Vec<u8>,
    pub(crate) u: Vec<u8>,
    pub(crate) v: Vec<u8>,
    pub(crate) turned: bool,
}

impl Frame {
    pub fn from_i420(width: u32, height: u32, y: Vec<u8>, u: Vec<u8>, v: Vec<u8>) -> Frame {
        let (w, h) = (width as usize, height as usize);
        let chroma = w.div_ceil(2) * h.div_ceil(2);
        assert_eq!(y.len(), w * h, "luma plane size");
        assert_eq!(u.len(), chroma, "U plane size");
        assert_eq!(v.len(), chroma, "V plane size");
        Frame {
            seq: 0,
            width,
            height,
            y,
            u,
            v,
            turned: false,
        }
    }

    /// Marks the planes as a picture that is shown a quarter turn clockwise, without moving a pixel: the turn
    /// costs 115 ms for an iPad picture in an unoptimized build, and only a screenshot needs it.
    pub fn turned_cw(mut self) -> Frame {
        self.turned = !self.turned;
        self
    }

    /// Width and height as shown, after any pending quarter turn.
    pub fn display_size(&self) -> (u32, u32) {
        if self.turned {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }

    /// The picture with any pending quarter turn applied.
    pub fn upright(&self) -> std::borrow::Cow<'_, Frame> {
        if self.turned {
            let mut out = self.rotated_cw();
            out.seq = self.seq;
            std::borrow::Cow::Owned(out)
        } else {
            std::borrow::Cow::Borrowed(self)
        }
    }

    /// The picture turned a quarter turn clockwise: the left edge becomes the top. Width and height must be even so
    /// the chroma planes turn with the luma.
    pub fn rotated_cw(&self) -> Frame {
        let turn = |src: &[u8], w: usize, h: usize| {
            let mut out = vec![0; w * h];
            for row in 0..h {
                for col in 0..w {
                    out[col * h + (h - 1 - row)] = src[row * w + col];
                }
            }
            out
        };
        let (w, h) = (self.width as usize, self.height as usize);
        Frame::from_i420(
            self.height,
            self.width,
            turn(&self.y, w, h),
            turn(&self.u, w / 2, h / 2),
            turn(&self.v, w / 2, h / 2),
        )
    }
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Frame(#{} {}x{})", self.seq, self.width, self.height)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    Live,
    Stalled,
}

struct Thumb {
    samples: Vec<u8>,
    cols: usize,
    rows: usize,
    sample_pt: f32,
}

struct Inner {
    seq: u64,
    latest: Option<Arc<Frame>>,
    thumb: Option<Thumb>,
    last_frame: Instant,
}

pub struct FrameStore {
    density: f32,
    screen_long_pt: Option<f32>,
    inner: Mutex<Inner>,
    tx: watch::Sender<FrameRef>,
    decode_errors: AtomicU64,
    publishes_skipped: AtomicU64,
}

impl FrameStore {
    /// `density` is device pixels per point, from the agent's `hello`.
    pub fn new(density: f32) -> FrameStore {
        FrameStore::with_scale(density, None)
    }

    /// For a stream whose frames may be smaller than the screen: an encoder that rejects the display size
    /// encodes a downscaled picture, so pixels per point come from each frame's long side.
    pub fn fitted(screen_long_pt: f32) -> FrameStore {
        FrameStore::with_scale(1.0, Some(screen_long_pt))
    }

    fn with_scale(density: f32, screen_long_pt: Option<f32>) -> FrameStore {
        FrameStore {
            density,
            screen_long_pt,
            inner: Mutex::new(Inner {
                seq: 0,
                latest: None,
                thumb: None,
                last_frame: Instant::now(),
            }),
            tx: watch::Sender::new(FrameRef::default()),
            decode_errors: AtomicU64::new(0),
            publishes_skipped: AtomicU64::new(0),
        }
    }

    /// Frame pixels per screen point.
    pub fn pixels_per_point(&self, frame: &Frame) -> f32 {
        match self.screen_long_pt {
            Some(long) => frame.width.max(frame.height) as f32 / long,
            None => self.density,
        }
    }

    fn block(&self, frame: &Frame) -> usize {
        ((SAMPLE_POINTS * self.pixels_per_point(frame)).round() as usize).max(1)
    }

    fn thumb(&self, frame: &Frame) -> Thumb {
        let (w, h) = (frame.width as usize, frame.height as usize);
        let b = self.block(frame);
        let (shown_w, shown_h) = if frame.turned { (h, w) } else { (w, h) };
        let (cols, rows) = (shown_w / b, shown_h / b);
        let mut samples = vec![0u8; cols * rows];
        let area = (b * b) as u32;
        let row_sum = |row: usize, col: usize| {
            let start = row * w + col;
            frame.y[start..start + b]
                .iter()
                .map(|&p| p as u32)
                .sum::<u32>()
        };
        for ty in 0..rows {
            for tx in 0..cols {
                let mut sum = 0u32;
                if frame.turned {
                    for dx in 0..b {
                        sum += row_sum(h - 1 - (tx * b + dx), ty * b);
                    }
                } else {
                    for dy in 0..b {
                        sum += row_sum(ty * b + dy, tx * b);
                    }
                }
                samples[ty * cols + tx] = (sum / area) as u8;
            }
        }
        Thumb {
            samples,
            cols,
            rows,
            sample_pt: b as f32 / self.pixels_per_point(frame),
        }
    }

    fn change(&self, previous: &Thumb, now: &Thumb, frame: &Frame) -> (f32, Option<Rect>) {
        if previous.cols != now.cols || previous.rows != now.rows {
            let (shown_w, shown_h) = frame.display_size();
            let ppp = self.pixels_per_point(frame);
            let (w, h) = (shown_w as f32 / ppp, shown_h as f32 / ppp);
            return (
                w * h,
                Some(Rect {
                    x: 0.0,
                    y: 0.0,
                    w,
                    h,
                }),
            );
        }
        let (mut count, mut min_c, mut max_c, mut min_r, mut max_r) =
            (0usize, usize::MAX, 0usize, usize::MAX, 0usize);
        for r in 0..now.rows {
            for c in 0..now.cols {
                let i = r * now.cols + c;
                if (previous.samples[i] as i16 - now.samples[i] as i16).abs() > CHANGED_LEVELS {
                    count += 1;
                    min_c = min_c.min(c);
                    max_c = max_c.max(c);
                    min_r = min_r.min(r);
                    max_r = max_r.max(r);
                }
            }
        }
        if count == 0 {
            return (0.0, None);
        }
        let s = now.sample_pt;
        (
            count as f32 * s * s,
            Some(Rect {
                x: min_c as f32 * s,
                y: min_r as f32 * s,
                w: (max_c - min_c + 1) as f32 * s,
                h: (max_r - min_r + 1) as f32 * s,
            }),
        )
    }

    /// Stores `frame` as the latest, numbers it, and tells subscribers how much of the screen changed.
    /// The first frame, having nothing to compare with, reports no change.
    pub fn publish(&self, mut frame: Frame) -> FrameRef {
        let thumb = self.thumb(&frame);
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let (changed_area_pt2, changed_box) = match &inner.thumb {
            Some(previous) => self.change(previous, &thumb, &frame),
            None => (0.0, None),
        };
        inner.seq += 1;
        frame.seq = inner.seq;
        let r = FrameRef {
            seq: inner.seq,
            changed_area_pt2,
            changed_box,
        };
        inner.latest = Some(Arc::new(frame));
        inner.thumb = Some(thumb);
        inner.last_frame = Instant::now();
        drop(inner);
        self.tx.send_replace(r);
        r
    }

    /// A receiver that always holds the newest `FrameRef`; intermediate ones are skipped.
    pub fn subscribe(&self) -> watch::Receiver<FrameRef> {
        self.tx.subscribe()
    }

    pub fn latest(&self) -> Option<Arc<Frame>> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .latest
            .clone()
    }

    pub fn state(&self) -> StreamState {
        let last = self
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last_frame;
        if last.elapsed() > STALL_AFTER {
            StreamState::Stalled
        } else {
            StreamState::Live
        }
    }

    /// True when the newest frame is black everywhere inside `region` (points): what a `FLAG_SECURE` window looks like.
    /// False when there is no frame or the region covers no thumbnail sample.
    pub fn region_is_black(&self, region: Rect) -> bool {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(thumb) = &inner.thumb else {
            return false;
        };
        let s = thumb.sample_pt;
        let col = |pt: f32| ((pt / s).max(0.0) as usize).min(thumb.cols);
        let row = |pt: f32| ((pt / s).max(0.0) as usize).min(thumb.rows);
        let (c0, c1) = (col(region.x), col(region.x + region.w));
        let (r0, r1) = (row(region.y), row(region.y + region.h));
        if c0 >= c1 || r0 >= r1 {
            return false;
        }
        (r0..r1).all(|r| (c0..c1).all(|c| thumb.samples[r * thumb.cols + c] <= BLACK_LEVEL))
    }

    pub fn record_skipped_publish(&self) {
        self.publishes_skipped.fetch_add(1, Ordering::Relaxed);
    }

    /// Pictures decoded but not shown because the receiver was far behind (see `receive::Pacing`).
    pub fn publishes_skipped(&self) -> u64 {
        self.publishes_skipped.load(Ordering::Relaxed)
    }

    pub fn record_decode_error(&self) {
        self.decode_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn decode_errors(&self) -> u64 {
        self.decode_errors.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quarter_turn_clockwise_moves_the_left_edge_to_the_top() {
        let frame = Frame::from_i420(
            4,
            2,
            vec![1, 2, 3, 4, 5, 6, 7, 8],
            vec![10, 20],
            vec![30, 40],
        );
        let turned = frame.rotated_cw();
        assert_eq!((turned.width, turned.height), (2, 4));
        assert_eq!(turned.y, [5, 1, 6, 2, 7, 3, 8, 4]);
        assert_eq!(turned.u, [10, 20]);
        assert_eq!(turned.v, [30, 40]);
    }

    #[test]
    fn a_turned_frame_has_the_thumbnail_and_size_of_its_upright_picture() {
        let (w, h) = (24u32, 16u32);
        let y: Vec<u8> = (0..w * h).map(|i| ((i * 7) % 251) as u8).collect();
        let (u, v) = (
            vec![90; (w * h / 4) as usize],
            vec![140; (w * h / 4) as usize],
        );
        let stored = Frame::from_i420(w, h, y, u, v);
        let lazy = stored.clone().turned_cw();
        let eager = stored.rotated_cw();
        assert_eq!(lazy.display_size(), (h, w));
        let store = FrameStore::new(1.0);
        let (a, b) = (store.thumb(&lazy), store.thumb(&eager));
        assert_eq!((a.cols, a.rows), (b.cols, b.rows));
        assert_eq!(a.samples, b.samples);
        let up = lazy.upright();
        assert_eq!(
            (up.width, up.height, &up.y),
            (eager.width, eager.height, &eager.y)
        );
    }
}
