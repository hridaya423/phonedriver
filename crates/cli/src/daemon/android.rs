//! Android devices through the adb server.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use phone_use_android::adb::{Adb, AdbDevice, DeviceState};
use phone_use_android::device::AndroidDevice;
use phone_use_core::device::{Capabilities, Device};
use phone_use_core::error::Error;

use super::progress::after_a_second;
use super::{DeviceSource, Listed};

type OsKey = (String, Option<u32>);

#[derive(Default)]
struct Tracked {
    devices: Option<Vec<AdbDevice>>,
    running: bool,
}

pub struct AdbSource {
    adb: Adb,
    tracked: Arc<Mutex<Tracked>>,
    os: Mutex<HashMap<OsKey, Option<String>>>,
}

impl AdbSource {
    pub fn from_env() -> AdbSource {
        AdbSource::new(Adb::from_env())
    }

    pub fn new(adb: Adb) -> AdbSource {
        AdbSource {
            adb,
            tracked: Arc::default(),
            os: Mutex::default(),
        }
    }

    /// The daemon resolves a device on every request, and a `host:devices-l` round trip each time is part of the
    /// bare tap's latency, so the list is kept current from adb's `track-devices` stream; until the stream is up
    /// (and after it drops) a request asks the server directly.
    async fn devices(&self) -> Result<Vec<AdbDevice>, Error> {
        let start = {
            let mut t = self.tracked.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(d) = &t.devices {
                return Ok(d.clone());
            }
            !std::mem::replace(&mut t.running, true)
        };
        if start {
            let (adb, tracked) = (self.adb.clone(), self.tracked.clone());
            tokio::spawn(async move {
                if let Ok(mut tracker) = adb.track_devices().await {
                    while let Ok(list) = tracker.next().await {
                        tracked.lock().unwrap_or_else(|e| e.into_inner()).devices = Some(list);
                    }
                }
                let mut t = tracked.lock().unwrap_or_else(|e| e.into_inner());
                t.devices = None;
                t.running = false;
            });
        }
        self.adb.devices().await
    }
}

#[async_trait]
impl DeviceSource for AdbSource {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        let mut listed = Vec::new();
        for d in self
            .devices()
            .await?
            .into_iter()
            .filter(|d| d.state == DeviceState::Device)
        {
            let key = (d.serial.clone(), d.transport_id);
            let known = self
                .os
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&key)
                .cloned();
            let os = match known {
                Some(os) => os,
                None => {
                    let os = self.adb.os_label(&d.serial).await.ok();
                    self.os
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(key, os.clone());
                    os
                }
            };
            listed.push(Listed {
                id: format!("android:{}", d.serial),
                model: d.model.unwrap_or_else(|| "unknown".into()),
                os,
                tier: "agent".into(),
                capabilities: Capabilities::all(),
            });
        }
        Ok(listed)
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        let serial = id.strip_prefix("android:").unwrap_or(id);
        let connecting = AndroidDevice::connect(&self.adb, serial);
        let device = after_a_second(
            || {
                format!(
                    "starting the Android agent on {serial} (the first run on a device pushes it)"
                )
            },
            connecting,
        )
        .await?;
        Ok(Arc::new(device))
    }
}
