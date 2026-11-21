//! Writes the agent's H.264 stream into an MP4 file as it arrives, without decoding it (decision D59).
//!
//! The file is `ftyp`, one `mdat` that grows with each sample, and a `moov` written by `finish`. A recording that
//! is dropped is finished first, so a device that goes away leaves a file that plays.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::h264::AccessUnit;

const TIMESCALE: u32 = 90_000;
const MDAT_HEADER: u64 = 16;
const NAL_SPS: u8 = 7;
const NAL_PPS: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    pub ms: u64,
}

struct Sample {
    size: u32,
    /// In `TIMESCALE` units; set when the next sample arrives, or by `finish`.
    duration: u32,
    sync: bool,
}

pub struct Recording {
    file: File,
    path: PathBuf,
    width: u32,
    height: u32,
    sps: Vec<u8>,
    pps: Vec<u8>,
    samples: Vec<Sample>,
    bytes: u64,
    mdat_at: u64,
    finished: bool,
}

/// The body of the first `kind` box among the boxes of `boxes`.
fn child<'a>(mut boxes: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    while let (Some(size), Some(name)) = (boxes.get(..4), boxes.get(4..8)) {
        let size = u32::from_be_bytes(size.try_into().ok()?) as usize;
        let body = boxes.get(8..size)?;
        if name == kind {
            return Some(body);
        }
        boxes = &boxes[size..];
    }
    None
}

fn be32(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as u64)
}

/// What an MP4 or QuickTime file says of its first video track: the picture's size, how many frames it holds and
/// how long the movie runs. For a file this module did not write, such as a Simulator's own recording.
pub fn probe(path: &Path) -> io::Result<Recorded> {
    let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidData, what.to_string());
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let mut at = 0;
    let moov = loop {
        if at + 8 > len {
            return Err(bad("the file has no moov box"));
        }
        file.seek(SeekFrom::Start(at))?;
        let mut head = [0u8; 16];
        file.read_exact(&mut head[..8])?;
        let (size, header) = match u32::from_be_bytes([head[0], head[1], head[2], head[3]]) {
            0 => (len - at, 8),
            1 => {
                file.read_exact(&mut head[8..])?;
                let mut wide = [0u8; 8];
                wide.copy_from_slice(&head[8..]);
                (u64::from_be_bytes(wide), 16)
            }
            n => (n as u64, 8),
        };
        if size < header || at + size > len {
            return Err(bad("a box runs past the end of the file"));
        }
        if &head[4..8] == b"moov" {
            let mut body = vec![0; (size - header) as usize];
            file.read_exact(&mut body)?;
            break body;
        }
        at += size;
    };
    let read = || -> Option<Recorded> {
        let mvhd = child(&moov, b"mvhd")?;
        let (timescale, ticks) = match mvhd.first()? {
            0 => (be32(mvhd, 12)?, be32(mvhd, 16)?),
            _ => (be32(mvhd, 20)?, be32(mvhd, 24)? << 32 | be32(mvhd, 28)?),
        };
        let mut boxes = &moov[..];
        loop {
            let size = be32(boxes, 0)? as usize;
            let (name, body) = (boxes.get(4..8)?, boxes.get(8..size)?);
            boxes = &boxes[size..];
            if name != b"trak" {
                continue;
            }
            let tkhd = child(body, b"tkhd")?;
            let (width, height) = (
                be32(tkhd, tkhd.len().checked_sub(8)?)? >> 16,
                be32(tkhd, tkhd.len() - 4)? >> 16,
            );
            if width == 0 || height == 0 {
                continue;
            }
            let stbl = child(child(child(body, b"mdia")?, b"minf")?, b"stbl")?;
            return Some(Recorded {
                path: path.to_path_buf(),
                width: width as u32,
                height: height as u32,
                frames: be32(child(stbl, b"stsz")?, 8)? as u32,
                ms: ticks.checked_mul(1000)?.checked_div(timescale)?,
            });
        }
    };
    read().ok_or_else(|| bad("the file has no video track this reader understands"))
}

/// The NAL units of an Annex-B buffer, without their start codes.
fn nal_units(annex_b: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= annex_b.len() {
        if annex_b[i] == 0 && annex_b[i + 1] == 0 && annex_b[i + 2] == 1 {
            starts.push((i, i + 3));
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::with_capacity(starts.len());
    for (n, (_, begin)) in starts.iter().enumerate() {
        let mut end = starts.get(n + 1).map_or(annex_b.len(), |(code, _)| *code);
        while end > *begin && annex_b[end - 1] == 0 {
            end -= 1;
        }
        if end > *begin {
            out.push(&annex_b[*begin..end]);
        }
    }
    out
}

fn ticks(us: u64) -> u32 {
    (us.saturating_mul(u64::from(TIMESCALE)) / 1_000_000).min(u64::from(u32::MAX)) as u32
}

fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 8);
    out.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out
}

fn full(kind: &[u8; 4], version_flags: u32, body: &[u8]) -> Vec<u8> {
    let mut inner = version_flags.to_be_bytes().to_vec();
    inner.extend_from_slice(body);
    boxed(kind, &inner)
}

const MATRIX: [u32; 9] = [0x0001_0000, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000];

impl Recording {
    pub fn create(path: &Path, width: u32, height: u32) -> io::Result<Recording> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        let mut ftyp = b"isom".to_vec();
        ftyp.extend_from_slice(&0x200u32.to_be_bytes());
        ftyp.extend_from_slice(b"isomiso2avc1mp41");
        file.write_all(&boxed(b"ftyp", &ftyp))?;
        let mdat_at = file.stream_position()?;
        file.write_all(&1u32.to_be_bytes())?;
        file.write_all(b"mdat")?;
        file.write_all(&MDAT_HEADER.to_be_bytes())?;
        Ok(Recording {
            file,
            path: path.to_path_buf(),
            width,
            height,
            sps: Vec::new(),
            pps: Vec::new(),
            samples: Vec::new(),
            bytes: 0,
            mdat_at,
            finished: false,
        })
    }

    /// Adds one unit of the stream. A config unit sets the parameter sets; any other becomes a sample that lasts
    /// `lasts_us`, which the caller can correct with `stretch_last` once it knows when the next one came.
    pub fn push(&mut self, unit: &AccessUnit, lasts_us: u64) -> io::Result<()> {
        let mut body = Vec::with_capacity(unit.data.len() + 8);
        for nal in nal_units(&unit.data) {
            match nal[0] & 0x1f {
                NAL_SPS => self.sps = nal.to_vec(),
                NAL_PPS => self.pps = nal.to_vec(),
                _ => {
                    body.extend_from_slice(&(nal.len() as u32).to_be_bytes());
                    body.extend_from_slice(nal);
                }
            }
        }
        if unit.config || body.is_empty() {
            return Ok(());
        }
        self.file.write_all(&body)?;
        self.bytes += body.len() as u64;
        self.samples.push(Sample {
            size: body.len() as u32,
            duration: ticks(lasts_us).max(1),
            sync: unit.keyframe,
        });
        Ok(())
    }

    /// Sets how long the newest sample lasts.
    pub fn stretch_last(&mut self, lasts_us: u64) {
        if let Some(last) = self.samples.last_mut() {
            last.duration = ticks(lasts_us).max(1);
        }
    }

    pub fn frames(&self) -> u32 {
        self.samples.len() as u32
    }

    pub fn finish(mut self) -> io::Result<Recorded> {
        self.close()
    }

    fn close(&mut self) -> io::Result<Recorded> {
        self.finished = true;
        let total: u64 = self.samples.iter().map(|s| u64::from(s.duration)).sum();
        let moov = self.moov(total);
        self.file.write_all(&moov)?;
        self.file.seek(SeekFrom::Start(self.mdat_at + 8))?;
        self.file
            .write_all(&(MDAT_HEADER + self.bytes).to_be_bytes())?;
        self.file.flush()?;
        Ok(Recorded {
            path: self.path.clone(),
            width: self.width,
            height: self.height,
            frames: self.frames(),
            ms: total * 1000 / u64::from(TIMESCALE),
        })
    }

    fn moov(&self, total: u64) -> Vec<u8> {
        let duration = total.min(u64::from(u32::MAX)) as u32;
        let mut mvhd = vec![0u8; 8];
        mvhd.extend_from_slice(&TIMESCALE.to_be_bytes());
        mvhd.extend_from_slice(&duration.to_be_bytes());
        mvhd.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        mvhd.extend_from_slice(&0x0100u16.to_be_bytes());
        mvhd.extend_from_slice(&[0u8; 10]);
        for m in MATRIX {
            mvhd.extend_from_slice(&m.to_be_bytes());
        }
        mvhd.extend_from_slice(&[0u8; 24]);
        mvhd.extend_from_slice(&2u32.to_be_bytes());

        let mut tkhd = vec![0u8; 8];
        tkhd.extend_from_slice(&1u32.to_be_bytes());
        tkhd.extend_from_slice(&[0u8; 4]);
        tkhd.extend_from_slice(&duration.to_be_bytes());
        tkhd.extend_from_slice(&[0u8; 16]);
        for m in MATRIX {
            tkhd.extend_from_slice(&m.to_be_bytes());
        }
        tkhd.extend_from_slice(&(self.width << 16).to_be_bytes());
        tkhd.extend_from_slice(&(self.height << 16).to_be_bytes());

        let mut mdhd = vec![0u8; 8];
        mdhd.extend_from_slice(&TIMESCALE.to_be_bytes());
        mdhd.extend_from_slice(&duration.to_be_bytes());
        mdhd.extend_from_slice(&0x55c4u16.to_be_bytes());
        mdhd.extend_from_slice(&[0u8; 2]);

        let mut hdlr = vec![0u8; 4];
        hdlr.extend_from_slice(b"vide");
        hdlr.extend_from_slice(&[0u8; 12]);
        hdlr.extend_from_slice(b"phonedriver\0");

        let mut vmhd = Vec::new();
        vmhd.extend_from_slice(&[0u8; 8]);
        let url = full(b"url ", 1, &[]);
        let mut dref = 1u32.to_be_bytes().to_vec();
        dref.extend_from_slice(&url);
        let dinf = boxed(b"dinf", &full(b"dref", 0, &dref));

        let mut minf = full(b"vmhd", 1, &vmhd);
        minf.extend_from_slice(&dinf);
        minf.extend_from_slice(&self.stbl());

        let mut mdia = full(b"mdhd", 0, &mdhd);
        mdia.extend_from_slice(&full(b"hdlr", 0, &hdlr));
        mdia.extend_from_slice(&boxed(b"minf", &minf));

        let mut trak = full(b"tkhd", 3, &tkhd);
        trak.extend_from_slice(&boxed(b"mdia", &mdia));

        let mut moov = full(b"mvhd", 0, &mvhd);
        moov.extend_from_slice(&boxed(b"trak", &trak));
        boxed(b"moov", &moov)
    }

    fn stbl(&self) -> Vec<u8> {
        let mut avcc = vec![
            1,
            self.sps.get(1).copied().unwrap_or(66),
            self.sps.get(2).copied().unwrap_or(0),
            self.sps.get(3).copied().unwrap_or(31),
            0xff,
            0xe1,
        ];
        avcc.extend_from_slice(&(self.sps.len() as u16).to_be_bytes());
        avcc.extend_from_slice(&self.sps);
        avcc.push(1);
        avcc.extend_from_slice(&(self.pps.len() as u16).to_be_bytes());
        avcc.extend_from_slice(&self.pps);

        let mut avc1 = vec![0u8; 6];
        avc1.extend_from_slice(&1u16.to_be_bytes());
        avc1.extend_from_slice(&[0u8; 16]);
        avc1.extend_from_slice(&(self.width as u16).to_be_bytes());
        avc1.extend_from_slice(&(self.height as u16).to_be_bytes());
        avc1.extend_from_slice(&0x0048_0000u32.to_be_bytes());
        avc1.extend_from_slice(&0x0048_0000u32.to_be_bytes());
        avc1.extend_from_slice(&[0u8; 4]);
        avc1.extend_from_slice(&1u16.to_be_bytes());
        avc1.extend_from_slice(&[0u8; 32]);
        avc1.extend_from_slice(&0x0018u16.to_be_bytes());
        avc1.extend_from_slice(&0xffffu16.to_be_bytes());
        avc1.extend_from_slice(&boxed(b"avcC", &avcc));
        let mut stsd = 1u32.to_be_bytes().to_vec();
        stsd.extend_from_slice(&boxed(b"avc1", &avc1));

        let mut runs: Vec<(u32, u32)> = Vec::new();
        for s in &self.samples {
            match runs.last_mut() {
                Some((count, duration)) if *duration == s.duration => *count += 1,
                _ => runs.push((1, s.duration)),
            }
        }
        let mut stts = (runs.len() as u32).to_be_bytes().to_vec();
        for (count, duration) in runs {
            stts.extend_from_slice(&count.to_be_bytes());
            stts.extend_from_slice(&duration.to_be_bytes());
        }

        let sync: Vec<u32> = self
            .samples
            .iter()
            .enumerate()
            .filter(|(_, s)| s.sync)
            .map(|(i, _)| i as u32 + 1)
            .collect();
        let mut stss = (sync.len() as u32).to_be_bytes().to_vec();
        for n in sync {
            stss.extend_from_slice(&n.to_be_bytes());
        }

        let count = self.samples.len() as u32;
        let mut stsc = 1u32.to_be_bytes().to_vec();
        stsc.extend_from_slice(&1u32.to_be_bytes());
        stsc.extend_from_slice(&count.to_be_bytes());
        stsc.extend_from_slice(&1u32.to_be_bytes());

        let mut stsz = 0u32.to_be_bytes().to_vec();
        stsz.extend_from_slice(&count.to_be_bytes());
        for s in &self.samples {
            stsz.extend_from_slice(&s.size.to_be_bytes());
        }

        let mut co64 = 1u32.to_be_bytes().to_vec();
        co64.extend_from_slice(&(self.mdat_at + MDAT_HEADER).to_be_bytes());

        let mut stbl = full(b"stsd", 0, &stsd);
        stbl.extend_from_slice(&full(b"stts", 0, &stts));
        stbl.extend_from_slice(&full(b"stss", 0, &stss));
        stbl.extend_from_slice(&full(b"stsc", 0, &stsc));
        stbl.extend_from_slice(&full(b"stsz", 0, &stsz));
        stbl.extend_from_slice(&full(b"co64", 0, &co64));
        boxed(b"stbl", &stbl)
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_annex_b_buffer_splits_at_three_and_four_byte_start_codes() {
        let data = [
            0, 0, 0, 1, 0x67, 1, 2, 0, 0, 1, 0x68, 3, 0, 0, 0, 1, 0x65, 9, 9,
        ];
        let nals = nal_units(&data);
        assert_eq!(nals, [&[0x67, 1, 2][..], &[0x68, 3][..], &[0x65, 9, 9][..]]);
        assert!(nal_units(&[1, 2, 3]).is_empty());
    }
}
