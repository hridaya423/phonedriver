//! The control stream: requests with ids, replies matched by id, pushed events.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use phone_use_core::device::DeviceEvent;
use phone_use_core::error::{DeviceId, Error, Op};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::{broadcast, oneshot};
use tokio::task::JoinHandle;

use crate::adb::Result;
use crate::agent::{read_frame, write_frame};
use crate::map;

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>;

pub struct Control {
    device: DeviceId,
    writer: tokio::sync::Mutex<OwnedWriteHalf>,
    pending: Pending,
    next_id: AtomicU64,
    reader: JoinHandle<()>,
}

impl Control {
    /// Takes over a control stream that has already said hello. Pushed events go to `events`; when the
    /// stream ends, `DeviceEvent::Gone` is sent and every waiting request fails with `DeviceGone`.
    pub fn new(
        device: DeviceId,
        stream: TcpStream,
        density: f32,
        events: broadcast::Sender<DeviceEvent>,
    ) -> Control {
        let (mut read, write) = stream.into_split();
        let pending: Pending = Arc::default();
        let waiting = pending.clone();
        let reader = tokio::spawn(async move {
            while let Ok(frame) = read_frame(&mut read).await {
                if let Some(id) = frame["id"].as_u64() {
                    let sender = waiting
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(&id);
                    if let Some(tx) = sender {
                        let _ = tx.send(frame);
                    }
                } else if let Some(event) = map::event(&frame, density) {
                    let _ = events.send(event);
                }
            }
            waiting.lock().unwrap_or_else(|e| e.into_inner()).clear();
            let _ = events.send(DeviceEvent::Gone);
        });
        Control {
            device,
            writer: tokio::sync::Mutex::new(write),
            pending,
            next_id: AtomicU64::new(1),
            reader,
        }
    }

    /// Sends `req` (an object with an `op`) and returns the agent's reply. `op` names the operation in errors.
    pub async fn request(&self, op: Op, mut req: Value) -> Result<Value> {
        let gone = || Error::DeviceGone {
            id: self.device.clone(),
        };
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        req["id"] = id.into();
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, tx);
        if write_frame(&mut *self.writer.lock().await, &req)
            .await
            .is_err()
        {
            self.pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&id);
            return Err(gone());
        }
        let reply = rx.await.map_err(|_| gone())?;
        if reply["ok"] == true {
            Ok(reply)
        } else {
            Err(map::reply_error(
                op,
                reply["error"].as_str().unwrap_or("no error text"),
            ))
        }
    }
}

impl Drop for Control {
    fn drop(&mut self) {
        self.reader.abort();
    }
}
