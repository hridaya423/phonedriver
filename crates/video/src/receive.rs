//! RTP datagrams to pictures for the iOS display stream (architecture 7.2).

use std::time::{Duration, Instant};

use tokio::sync::mpsc::{self, error::TryRecvError};

use crate::frame_store::Frame;
use crate::h264::Decoder;
use crate::rtp::{Depacketizer, Event};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Keyframe,
    Decoded(u32),
    DecodeFailed,
    /// A picture was decoded but not shown, because `queued` datagrams were waiting (more than `Pacing::give_up_above`).
    PublishSkipped {
        queued: usize,
    },
}

/// When a picture is shown while datagrams are still waiting. Publishing is cheap (the quarter turn is deferred
/// to the screenshot), so the newest picture is shown every `at_least_every` even during a backlog, which keeps
/// what a screenshot shows from lagging by the whole backlog; beyond `give_up_above` waiting datagrams the host
/// is too starved to do even that and only decoding continues.
#[derive(Debug, Clone, Copy)]
pub struct Pacing {
    pub at_least_every: Duration,
    pub give_up_above: usize,
}

impl Default for Pacing {
    fn default() -> Pacing {
        Pacing {
            at_least_every: Duration::from_millis(100),
            give_up_above: 400,
        }
    }
}

/// Decodes every access unit, because each one is a reference for the next, but hands `publish` a picture only
/// when no datagram is waiting or `pacing` allows. Publishing every picture once cost more than decoding
/// (rotation, thumbnail), so the receiver fell behind the device's 20 pictures a second, and the screen it showed
/// lagged further each second (observed on the iPad, 2026-11-06: 34 s behind after 40 s in an unoptimized build).
pub fn decode_stream(
    mut queue: mpsc::Receiver<Vec<u8>>,
    mut publish: impl FnMut(Frame),
    mut signal: impl FnMut(Signal),
    pacing: Pacing,
) {
    let Ok(mut decoder) = Decoder::new() else {
        return;
    };
    let mut depacketizer = Depacketizer::new();
    let mut newest = None;
    let mut last_publish = Instant::now();
    loop {
        let datagram = match queue.try_recv() {
            Ok(datagram) => datagram,
            Err(TryRecvError::Empty) => {
                if let Some(frame) = newest.take() {
                    publish(frame);
                    last_publish = Instant::now();
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
                        if newest.is_some() && last_publish.elapsed() >= pacing.at_least_every {
                            let queued = queue.len();
                            if queued > pacing.give_up_above {
                                signal(Signal::PublishSkipped { queued });
                            } else if let Some(frame) = newest.take() {
                                publish(frame);
                                last_publish = Instant::now();
                            }
                        }
                    }
                    Err(_) => signal(Signal::DecodeFailed),
                },
            }
        }
    }
}
