//! The pipeline from an agent video stream to the frame store.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::io::AsyncRead;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::frame_store::FrameStore;
use crate::h264::{self, Decoder, WirePacket};

/// Packets waiting for the decoder. When it is full the socket reader waits, which pushes back on the
/// agent instead of dropping inter-predicted frames the decoder needs.
const QUEUE: usize = 8;

enum Item {
    Unit(Vec<u8>),
    Barrier(u64),
}

pub struct VideoStream {
    pub store: Arc<FrameStore>,
    reader: JoinHandle<()>,
    done: watch::Receiver<bool>,
    passed: watch::Receiver<u64>,
    barriers: AtomicU64,
}

impl VideoStream {
    /// Reads the agent's video wire format from `reader` and decodes it into a new `FrameStore`.
    pub fn spawn<R: AsyncRead + Unpin + Send + 'static>(
        mut reader: R,
        store: FrameStore,
    ) -> VideoStream {
        let store = Arc::new(store);
        let (tx, mut rx) = mpsc::channel::<Item>(QUEUE);
        let (done_tx, done) = watch::channel(false);
        let (passed_tx, passed) = watch::channel(0u64);

        let decoder_store = store.clone();
        std::thread::spawn(move || {
            if let Ok(mut decoder) = Decoder::new() {
                while let Some(item) = rx.blocking_recv() {
                    match item {
                        Item::Unit(packet) => match decoder.decode(&packet) {
                            Ok(Some(frame)) => {
                                decoder_store.publish(frame);
                            }
                            Ok(None) => {}
                            Err(_) => decoder_store.record_decode_error(),
                        },
                        Item::Barrier(id) => {
                            passed_tx.send_replace(id);
                        }
                    }
                }
            }
            done_tx.send_replace(true);
        });

        let reader = tokio::spawn(async move {
            if h264::read_header(&mut reader).await.is_err() {
                return;
            }
            while let Ok(packet) = h264::read_packet(&mut reader).await {
                let item = match packet {
                    WirePacket::Access(unit) => Item::Unit(unit.data),
                    WirePacket::Barrier(id) => Item::Barrier(id),
                    WirePacket::Reconfigure(_) => continue,
                };
                if tx.send(item).await.is_err() {
                    return;
                }
            }
        });

        VideoStream {
            store,
            reader,
            done,
            passed,
            barriers: AtomicU64::new(0),
        }
    }

    /// A number to put in a `video_barrier` request to the agent.
    pub fn next_barrier(&self) -> u64 {
        self.barriers.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// True once barrier `id` has come back through the stream, which means every packet the agent wrote before it has
    /// been decoded and shown. False when the stream ended or `limit` passed without it: a stream that still looks
    /// open but delivers nothing (a dead agent no socket has reported yet) is never caught up.
    pub async fn wait_barrier(&self, id: u64, limit: Duration) -> bool {
        let mut passed = self.passed.clone();
        tokio::time::timeout(limit, passed.wait_for(|p| *p >= id))
            .await
            .is_ok_and(|r| r.is_ok())
    }

    /// True once the stream has ended. A quiet stream is not an ended one: a static screen sends no frames.
    pub fn ended(&self) -> bool {
        *self.done.borrow()
    }

    /// Resolves once the stream has ended and every packet read has been decoded.
    pub async fn finished(&self) {
        let mut done = self.done.clone();
        let _ = done.wait_for(|d| *d).await;
    }
}

impl Drop for VideoStream {
    fn drop(&mut self) {
        self.reader.abort();
    }
}
