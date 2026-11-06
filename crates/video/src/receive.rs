//! RTP datagrams to pictures for the iOS display stream (architecture 7.2).

use tokio::sync::mpsc::{self, error::TryRecvError};

use crate::frame_store::Frame;
use crate::h264::Decoder;
use crate::rtp::{Depacketizer, Event};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Keyframe,
    Decoded(u32),
    DecodeFailed,
}

/// Decodes every access unit, because each one is a reference for the next, but hands `publish` a picture only
/// when no datagram is waiting. Publishing costs more than decoding (copy, rotation, thumbnail), so a receiver
/// that published every picture fell behind the device's 20 pictures a second, and the screen it showed lagged
/// further each second (observed on the iPad, 2026-11-06: 34 s behind after 40 s in an unoptimized build).
pub fn decode_stream(
    mut queue: mpsc::Receiver<Vec<u8>>,
    mut publish: impl FnMut(Frame),
    mut signal: impl FnMut(Signal),
) {
    let Ok(mut decoder) = Decoder::new() else {
        return;
    };
    let mut depacketizer = Depacketizer::new();
    let mut newest = None;
    loop {
        let datagram = match queue.try_recv() {
            Ok(datagram) => datagram,
            Err(TryRecvError::Empty) => {
                if let Some(frame) = newest.take() {
                    publish(frame);
                }
                match queue.blocking_recv() {
                    Some(datagram) => datagram,
                    None => return,
                }
            }
            Err(TryRecvError::Disconnected) => {
                if let Some(frame) = newest.take() {
                    publish(frame);
                }
                return;
            }
        };
        for event in depacketizer.push(&datagram) {
            match event {
                Event::KeyframeNeeded => signal(Signal::Keyframe),
                Event::Unit(unit) => match decoder.decode(&unit.data) {
                    Ok(frame) => {
                        if frame.is_some() {
                            newest = frame;
                        }
                        signal(Signal::Decoded(unit.timestamp));
                    }
                    Err(_) => signal(Signal::DecodeFailed),
                },
            }
        }
    }
}
