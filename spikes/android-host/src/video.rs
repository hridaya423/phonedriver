use crate::adb::{Adb, Res};
use openh264::decoder::Decoder;
use openh264::formats::YUVSource;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone, Debug)]
pub struct FrameEvent {
    pub arrival: Instant,
    pub pts_us: u64,
    pub flags: u8,
    pub bytes: usize,
    pub decode_ms: f64,
    pub w: usize,
    pub h: usize,
    pub changed_samples: usize,
    pub changed_area_pt2: usize,
    pub decoded: bool,
}

pub struct Latest {
    pub w: usize,
    pub h: usize,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
    pub ys: usize,
    pub us: usize,
}

impl Latest {
    pub fn rgb(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.w * self.h * 3];
        for j in 0..self.h {
            for i in 0..self.w {
                let y = self.y[j * self.ys + i] as f32 - 16.0;
                let u = self.u[(j / 2) * self.us + i / 2] as f32 - 128.0;
                let v = self.v[(j / 2) * self.us + i / 2] as f32 - 128.0;
                let r = 1.164 * y + 1.596 * v;
                let g = 1.164 * y - 0.392 * u - 0.813 * v;
                let b = 1.164 * y + 2.017 * u;
                let o = (j * self.w + i) * 3;
                out[o] = r.clamp(0.0, 255.0) as u8;
                out[o + 1] = g.clamp(0.0, 255.0) as u8;
                out[o + 2] = b.clamp(0.0, 255.0) as u8;
            }
        }
        out
    }
    pub fn luma_at(&self, x: usize, y: usize) -> u8 {
        self.y[y * self.ys + x]
    }
}

pub fn write_png(path: &str, w: usize, h: usize, rgb: &[u8]) -> Res<()> {
    let f = std::fs::File::create(path)?;
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()?.write_image_data(rgb)?;
    Ok(())
}

pub struct Video {
    pub events: Arc<Mutex<Vec<FrameEvent>>>,
    pub latest: Arc<Mutex<Option<Latest>>>,
    pub raw: Arc<Mutex<Vec<u8>>>,
    pub headers: Arc<Mutex<Vec<(Instant, usize, usize)>>>,
    pub errors: Arc<AtomicUsize>,
    pub sps: Arc<Mutex<Vec<Vec<u8>>>>,
    pub connected_at: Instant,
    pub task: tokio::task::JoinHandle<()>,
}

struct Packet {
    arrival: Instant,
    pts: u64,
    flags: u8,
    data: Vec<u8>,
}

impl Video {
    pub async fn connect(adb: &Adb, serial: &str, name: &str, token: &str, density: f32, max_size: Option<i64>) -> Res<Video> {
        let connected_at = Instant::now();
        let mut s = adb.open_abstract(serial, name).await?;
        s.write_all(b"V").await?;
        let mut tok = serde_json::json!({"token":token});
        if let Some(m) = max_size {
            tok["max_size"] = m.into();
        }
        crate::agent::write_frame(&mut s, &tok).await?;
        let events = Arc::new(Mutex::new(Vec::new()));
        let latest = Arc::new(Mutex::new(None));
        let raw = Arc::new(Mutex::new(Vec::new()));
        let headers = Arc::new(Mutex::new(Vec::new()));
        let errors = Arc::new(AtomicUsize::new(0));
        let sps = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = mpsc::channel::<Packet>();
        {
            let (events, latest, errors) = (events.clone(), latest.clone(), errors.clone());
            std::thread::spawn(move || decode_loop(rx, events, latest, errors, density));
        }
        let (raw2, headers2, sps2) = (raw.clone(), headers.clone(), sps.clone());
        let task = tokio::spawn(async move {
            let mut hdr = [0u8; 12];
            if s.read_exact(&mut hdr).await.is_err() {
                return;
            }
            raw2.lock().unwrap().extend_from_slice(&hdr);
            let w = u32::from_be_bytes(hdr[4..8].try_into().unwrap()) as usize;
            let h = u32::from_be_bytes(hdr[8..12].try_into().unwrap()) as usize;
            headers2.lock().unwrap().push((Instant::now(), w, h));
            loop {
                let mut fh = [0u8; 13];
                if s.read_exact(&mut fh).await.is_err() {
                    return;
                }
                let pts = u64::from_be_bytes(fh[0..8].try_into().unwrap());
                let flags = fh[8];
                let len = u32::from_be_bytes(fh[9..13].try_into().unwrap()) as usize;
                let mut data = vec![0u8; len];
                if s.read_exact(&mut data).await.is_err() {
                    return;
                }
                let arrival = Instant::now();
                {
                    let mut r = raw2.lock().unwrap();
                    r.extend_from_slice(&fh);
                    r.extend_from_slice(&data);
                }
                if pts == u64::MAX && len == 0 {
                    let mut hdr = [0u8; 12];
                    if s.read_exact(&mut hdr).await.is_err() {
                        return;
                    }
                    raw2.lock().unwrap().extend_from_slice(&hdr);
                    let w = u32::from_be_bytes(hdr[4..8].try_into().unwrap()) as usize;
                    let h = u32::from_be_bytes(hdr[8..12].try_into().unwrap()) as usize;
                    headers2.lock().unwrap().push((Instant::now(), w, h));
                    continue;
                }
                if flags & 1 != 0 {
                    sps2.lock().unwrap().push(data.clone());
                }
                if tx.send(Packet { arrival, pts, flags, data }).is_err() {
                    return;
                }
            }
        });
        Ok(Video { events, latest, raw, headers, errors, sps, connected_at, task })
    }

    pub fn count(&self) -> usize {
        self.events.lock().unwrap().len()
    }

    pub fn since(&self, n: usize) -> Vec<FrameEvent> {
        self.events.lock().unwrap()[n..].to_vec()
    }

    pub async fn wait_frames(&self, n: usize, timeout_ms: u64) -> bool {
        let t = Instant::now();
        while self.count() < n {
            if t.elapsed().as_millis() as u64 > timeout_ms {
                return false;
            }
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        true
    }
}

fn thumb(y: &[u8], ys: usize, w: usize, h: usize, block: usize) -> Vec<u8> {
    let (tw, th) = (w / block, h / block);
    let mut t = vec![0u8; tw * th];
    for ty in 0..th {
        for tx in 0..tw {
            let mut sum = 0u32;
            for dy in 0..block {
                let row = (ty * block + dy) * ys + tx * block;
                for dx in 0..block {
                    sum += y[row + dx] as u32;
                }
            }
            t[ty * tw + tx] = (sum / (block * block) as u32) as u8;
        }
    }
    t
}

fn decode_loop(rx: mpsc::Receiver<Packet>, events: Arc<Mutex<Vec<FrameEvent>>>, latest: Arc<Mutex<Option<Latest>>>, errors: Arc<AtomicUsize>, density: f32) {
    let mut dec = Decoder::new().unwrap();
    let block = (4.0 * density).round() as usize;
    let mut prev: Option<(Vec<u8>, usize, usize)> = None;
    for p in rx {
        let t0 = Instant::now();
        let r = dec.decode(&p.data);
        let decode_ms = t0.elapsed().as_secs_f64() * 1000.0;
        match r {
            Ok(Some(yuv)) => {
                let (w, h) = yuv.dimensions();
                let (ys, us, _) = yuv.strides();
                let th = thumb(yuv.y(), ys, w, h, block);
                let mut changed = 0;
                if let Some((pt, pw, ph)) = &prev {
                    if *pw == w && *ph == h && pt.len() == th.len() {
                        changed = pt.iter().zip(th.iter()).filter(|(a, b)| (**a as i32 - **b as i32).abs() > 12).count();
                    }
                }
                prev = Some((th, w, h));
                *latest.lock().unwrap() = Some(Latest { w, h, y: yuv.y().to_vec(), u: yuv.u().to_vec(), v: yuv.v().to_vec(), ys, us });
                events.lock().unwrap().push(FrameEvent { arrival: p.arrival, pts_us: p.pts, flags: p.flags, bytes: p.data.len(), decode_ms, w, h, changed_samples: changed, changed_area_pt2: changed * 16, decoded: true });
            }
            Ok(None) => {
                events.lock().unwrap().push(FrameEvent { arrival: p.arrival, pts_us: p.pts, flags: p.flags, bytes: p.data.len(), decode_ms, w: 0, h: 0, changed_samples: 0, changed_area_pt2: 0, decoded: false });
            }
            Err(_) => {
                errors.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

pub struct Sps {
    pub profile_idc: u8,
    pub constraint: u8,
    pub level_idc: u8,
}

pub fn parse_sps(annexb: &[u8]) -> Option<Sps> {
    let mut i = 0;
    while i + 4 < annexb.len() {
        if annexb[i] == 0 && annexb[i + 1] == 0 && (annexb[i + 2] == 1 || (annexb[i + 2] == 0 && annexb[i + 3] == 1)) {
            let start = if annexb[i + 2] == 1 { i + 3 } else { i + 4 };
            if annexb[start] & 0x1f == 7 {
                return Some(Sps { profile_idc: annexb[start + 1], constraint: annexb[start + 2], level_idc: annexb[start + 3] });
            }
            i = start;
        } else {
            i += 1;
        }
    }
    None
}
