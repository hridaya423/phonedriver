//! The pipeline from an agent video stream to the frame store.

use std::sync::Arc;

use tokio::io::AsyncRead;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::frame_store::FrameStore;
use crate::h264::{self, Decoder, WirePacket};

/// Packets waiting for the decoder. When it is full the socket reader waits, which pushes back on the
/// agent instead of dropping inter-predicted frames the decoder needs.
const QUEUE: usize = 8;

pub struct VideoStream {
    pub store: Arc<FrameStore>,
    reader: JoinHandle<()>,
    done: watch::Receiver<bool>,
}

impl VideoStream {
    /// Reads the agent's video wire format from `reader` and decodes it into a new `FrameStore`.
    pub fn spawn<R: AsyncRead + Unpin + Send + 'static>(
        mut reader: R,
        store: FrameStore,
    ) -> VideoStream {
        let store = Arc::new(store);
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(QUEUE);
        let (done_tx, done) = watch::channel(false);

        let decoder_store = store.clone();
        std::thread::spawn(move || {
            if let Ok(mut decoder) = Decoder::new() {
                while let Some(packet) = rx.blocking_recv() {
                    match decoder.decode(&packet) {
                        Ok(Some(frame)) => {
                            decoder_store.publish(frame);
                        }
                        Ok(None) => {}
                        Err(_) => decoder_store.record_decode_error(),
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
                if let WirePacket::Access(unit) = packet
                    && tx.send(unit.data).await.is_err()
                {
                    return;
                }
            }
        });

        VideoStream {
            store,
            reader,
            done,
        }
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
