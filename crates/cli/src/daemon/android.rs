//! Android devices through the adb server.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use phonedriver_android::adb::{Adb, AdbDevice, DeviceState};
use phonedriver_android::device::AndroidDevice;
use phonedriver_core::device::{Capabilities, Device, DisplayMode};
use phonedriver_core::error::Error;

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

impl AdbSource {
    /// `os` is the Android version label, one `getprop` round trip per device the first time it is listed; a caller that
    /// only needs the ids skips it.
    async fn listed(&self, with_os: bool) -> Result<Vec<Listed>, Error> {
        let devices: Vec<AdbDevice> = self
            .devices()
            .await?
            .into_iter()
            .filter(|d| d.state == DeviceState::Device)
            .collect();
        let key = |d: &AdbDevice| (d.serial.clone(), d.transport_id);
        if with_os {
            let unknown: Vec<OsKey> = {
                let known = self.os.lock().unwrap_or_else(|e| e.into_inner());
                devices
                    .iter()
                    .map(key)
                    .filter(|k| !known.contains_key(k))
                    .collect()
            };
            let mut lookups = tokio::task::JoinSet::new();
            for k in unknown {
                let adb = self.adb.clone();
                lookups.spawn(async move {
                    let os = adb.os_label(&k.0).await.ok();
                    (k, os)
                });
            }
            while let Some(Ok((k, os))) = lookups.join_next().await {
                self.os
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(k, os);
            }
        }
        let known = self.os.lock().unwrap_or_else(|e| e.into_inner());
        Ok(devices
            .into_iter()
            .map(|d| Listed {
                os: known.get(&key(&d)).cloned().flatten(),
                id: format!("android:{}", d.serial),
                model: d.model.unwrap_or_else(|| "unknown".into()),
                tier: "agent".into(),
                capabilities: Capabilities::all(),
            })
            .collect())
    }
}

#[async_trait]
impl DeviceSource for AdbSource {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        self.listed(true).await
    }

    async fn list_for(&self, requested: Option<&str>) -> Result<Vec<Listed>, Error> {
        self.listed(requested.is_none()).await
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        self.connect_on(id, DisplayMode::Main).await
    }

    async fn connect_on(&self, id: &str, display: DisplayMode) -> Result<Arc<dyn Device>, Error> {
        let serial = id.strip_prefix("android:").unwrap_or(id);
        let connecting = AndroidDevice::connect(&self.adb, serial, display);
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
