//! The agent's video wire format (protocol section 2) and the H.264 decoder.

use std::io;

use openh264::formats::YUVSource;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::frame_store::Frame;

const FOURCC_H264: &[u8; 4] = b"h264";
const MAX_PACKET: usize = 64 << 20;
const FLAG_BARRIER: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecHeader {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessUnit {
    pub pts_us: u64,
    pub config: bool,
    pub keyframe: bool,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WirePacket {
    Access(AccessUnit),
    /// The display changed size or rotation; a new config packet and keyframe follow.
    Reconfigure(CodecHeader),
    /// Written by the agent between packets when the host asks (`video_barrier`): every packet before it is already read.
    Barrier(u64),
}

pub async fn read_header<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<CodecHeader> {
    let mut h = [0u8; 12];
    r.read_exact(&mut h).await?;
    if &h[..4] != FOURCC_H264 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported codec {:?}", &h[..4]),
        ));
    }
    Ok(CodecHeader {
        width: u32::from_be_bytes([h[4], h[5], h[6], h[7]]),
        height: u32::from_be_bytes([h[8], h[9], h[10], h[11]]),
    })
}

pub async fn read_packet<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<WirePacket> {
    let mut h = [0u8; 13];
    r.read_exact(&mut h).await?;
    let pts_us = u64::from_be_bytes([h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]]);
    let flags = h[8];
    let len = u32::from_be_bytes([h[9], h[10], h[11], h[12]]) as usize;
    if pts_us == u64::MAX && len == 0 {
        return Ok(WirePacket::Reconfigure(read_header(r).await?));
    }
    if flags & FLAG_BARRIER != 0 && len == 8 {
        let mut id = [0u8; 8];
        r.read_exact(&mut id).await?;
        return Ok(WirePacket::Barrier(u64::from_be_bytes(id)));
    }
    if len > MAX_PACKET {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("a {len} byte video packet"),
        ));
    }
    let mut data = vec![0u8; len];
    r.read_exact(&mut data).await?;
    Ok(WirePacket::Access(AccessUnit {
        pts_us,
        config: flags & 1 != 0,
        keyframe: flags & 2 != 0,
        data,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("H.264 decode failed: {0}")]
pub struct DecodeError(String);

pub struct Decoder(openh264::decoder::Decoder);

impl Decoder {
    pub fn new() -> Result<Decoder, DecodeError> {
        openh264::decoder::Decoder::new()
            .map(Decoder)
            .map_err(|e| DecodeError(e.to_string()))
    }

    /// Feeds one Annex-B packet. Config packets and packets that complete no picture return `None`.
    pub fn decode(&mut self, annex_b: &[u8]) -> Result<Option<Frame>, DecodeError> {
        let yuv = self
            .0
            .decode(annex_b)
            .map_err(|e| DecodeError(e.to_string()))?;
        Ok(yuv.map(|yuv| {
            let (w, h) = yuv.dimensions();
            let (ys, us, vs) = yuv.strides();
            let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
            Frame::from_i420(
                w as u32,
                h as u32,
                tight(yuv.y(), ys, w, h),
                tight(yuv.u(), us, cw, ch),
                tight(yuv.v(), vs, cw, ch),
            )
        }))
    }
}

fn tight(plane: &[u8], stride: usize, w: usize, h: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(w * h);
    for row in 0..h {
        out.extend_from_slice(&plane[row * stride..row * stride + w]);
    }
    out
}
