//! Android devices through the adb server.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use phonedriver_android::adb::{Adb, AdbDevice, DeviceState};
use phonedriver_android::avd::{self, Avd};
use phonedriver_android::device::AndroidDevice;
use phonedriver_android::wireless::{one_per_phone, serial_in_name};
use phonedriver_core::device::{Capabilities, Device, DisplayMode, lane_of};
use phonedriver_core::error::{DeviceSummary, Error};

use super::progress::after_a_second;
use super::{Booted, DeviceSource, Listed, named};

type OsKey = (String, Option<u32>);

/// The emulator sets this property to the name of the AVD it was started with.
const AVD_NAME: &str = "getprop ro.boot.qemu.avd_name";

/// `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`: the emulator keeps running when the daemon that started it exits.
#[cfg(windows)]
const DETACHED: u32 = 0x0000_0008 | 0x0000_0200;

#[derive(Default)]
struct Tracked {
    devices: Option<Vec<AdbDevice>>,
    running: bool,
}

pub struct AdbSource {
    adb: Adb,
    tracked: Arc<Mutex<Tracked>>,
    os: Mutex<HashMap<OsKey, Option<String>>>,
    hardware: Mutex<HashMap<OsKey, Option<String>>>,
    serials: Mutex<HashMap<OsKey, String>>,
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
            hardware: Mutex::default(),
            serials: Mutex::default(),
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
    /// only needs the ids skips it. A device an entry of `denied` names by id is left out before anything is asked of it.
    async fn listed(&self, with_os: bool, denied: &[String]) -> Result<Vec<Listed>, Error> {
        let devices: Vec<AdbDevice> = self
            .devices()
            .await?
            .into_iter()
            .filter(|d| {
                d.state == DeviceState::Device
                    && !super::policy::denies_id(denied, &format!("android:{}", d.serial))
            })
            .collect();
        let key = |d: &AdbDevice| (d.serial.clone(), d.transport_id);
        let devices = if with_os && devices.len() > 1 {
            let mut identified = Vec::new();
            for d in devices {
                let hardware = match serial_in_name(&d.serial) {
                    Some(s) => Some(s.to_string()),
                    None => {
                        let known = self
                            .hardware
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .get(&key(&d))
                            .cloned();
                        match known {
                            Some(h) => h,
                            None => {
                                let h = self.adb.hardware_serial(&d.serial).await.ok();
                                self.hardware
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner())
                                    .insert(key(&d), h.clone());
                                h
                            }
                        }
                    }
                };
                identified.push((d, hardware));
            }
            one_per_phone(identified)
        } else {
            devices
        };
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

fn env(name: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(name)
}

fn summary(avd: &Avd, id: String) -> DeviceSummary {
    DeviceSummary {
        id,
        model: avd.name.clone(),
        os: match &avd.api {
            Some(api) => format!("Android API {}", api.trim_end_matches(".0")),
            None => String::new(),
        },
    }
}

/// The first console port from 5554 that no emulator adb knows has and that nothing listens on. An emulator takes
/// an even port for its console and the next one for adb, and adb names it `emulator-<console port>`.
fn free_port(devices: &[AdbDevice]) -> Option<u16> {
    (5554..=5584u16).step_by(2).find(|port| {
        !devices
            .iter()
            .any(|d| d.serial == format!("emulator-{port}"))
            && [*port, port + 1]
                .iter()
                .all(|p| std::net::TcpListener::bind(("127.0.0.1", *p)).is_ok())
    })
}

impl AdbSource {
    /// The AVDs on this machine, when this machine is where adb's emulators run.
    fn avds(&self) -> Vec<Avd> {
        match avd::home(&env) {
            Some(home) if self.adb.is_local() => avd::list(&home),
            _ => Vec::new(),
        }
    }

    /// The serial and AVD name of each running emulator.
    async fn running(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for d in self.devices().await.unwrap_or_default() {
            if d.state != DeviceState::Device || !d.serial.starts_with("emulator-") {
                continue;
            }
            if let Ok(name) = self.adb.shell(&d.serial, AVD_NAME).await {
                out.push((d.serial, name.stdout_text().trim().to_string()));
            }
        }
        out
    }

    /// Starts the AVD on a port of its own, so that its serial is known, and waits until Android has booted.
    async fn start(&self, avd: &Avd) -> Result<String, Error> {
        let refused = |detail: String| Error::Refused { detail };
        let name = &avd.name;
        let emulator = avd::emulator(&env).ok_or_else(|| {
            refused(format!(
                "the `emulator` program that starts {name} was not found; set ANDROID_HOME to the Android SDK"
            ))
        })?;
        let port = free_port(&self.devices().await?)
            .ok_or_else(|| refused("no emulator port is free (5554 to 5584)".into()))?;
        let log = std::env::temp_dir().join(format!("phonedriver-emulator-{port}.log"));
        let out = std::fs::File::create(&log)
            .map_err(|e| refused(format!("cannot write {}: {e}", log.display())))?;
        let mut cmd = std::process::Command::new(&emulator);
        cmd.args(["-avd", name, "-port", &port.to_string()])
            .stdin(std::process::Stdio::null())
            .stderr(out.try_clone().map_err(|e| refused(e.to_string()))?)
            .stdout(out);
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut cmd, DETACHED);
        let mut child = cmd
            .spawn()
            .map_err(|e| refused(format!("cannot run {}: {e}", emulator.display())))?;
        let serial = format!("emulator-{port}");
        loop {
            if let Ok(Some(status)) = child.try_wait() {
                let said = std::fs::read_to_string(&log).unwrap_or_default();
                let last = said.lines().rev().find(|l| !l.trim().is_empty());
                return Err(refused(format!(
                    "the emulator for {name} stopped ({status}): {}",
                    last.unwrap_or("it printed nothing")
                )));
            }
            let up = self
                .devices()
                .await
                .unwrap_or_default()
                .iter()
                .any(|d| d.serial == serial && d.state == DeviceState::Device);
            if up
                && let Ok(done) = self.adb.shell(&serial, "getprop sys.boot_completed").await
                && done.stdout_text().trim() == "1"
            {
                return Ok(serial);
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
}

#[async_trait]
impl DeviceSource for AdbSource {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        self.listed(true, &[]).await
    }

    async fn list_except(&self, denied: &[String]) -> Result<Vec<Listed>, Error> {
        self.listed(true, denied).await
    }

    async fn off(&self) -> Vec<DeviceSummary> {
        let avds = self.avds();
        if avds.is_empty() {
            return Vec::new();
        }
        let running = self.running().await;
        avds.iter()
            .filter(|a| !running.iter().any(|(_, name)| *name == a.name))
            .map(|a| summary(a, format!("avd:{}", a.name)))
            .collect()
    }

    async fn boot(&self, name: &str) -> Result<Option<Booted>, Error> {
        let avds = self.avds();
        let Some(avd) = avds
            .iter()
            .find(|a| named(name, &format!("avd:{}", a.name), &a.name))
        else {
            return Ok(None);
        };
        let running = self.running().await;
        let on = running.iter().find(|(_, name)| *name == avd.name);
        let serial = match on {
            Some((serial, _)) => serial.clone(),
            None => self.start(avd).await?,
        };
        Ok(Some(Booted {
            device: summary(avd, format!("android:{serial}")),
            already: on.is_some(),
        }))
    }

    /// A request that names a device sees every connection, so an id that the unnamed listing folded into
    /// another connection of the same phone still resolves.
    async fn list_for(&self, requested: Option<&str>) -> Result<Vec<Listed>, Error> {
        self.listed(requested.is_none(), &[]).await
    }

    /// Always the phone's `ro.serialno`, read once per connection: an emulator's adb name (`emulator-5554`) and a
    /// phone's adb name over Wi-Fi (`192.168.1.20:5555`) are not it, and `adb devices` over USB shows it only usually.
    async fn hardware_serial(&self, id: &str) -> Result<Option<String>, Error> {
        let serial = id.strip_prefix("android:").unwrap_or(id);
        let device = self
            .devices()
            .await?
            .into_iter()
            .find(|d| d.serial == serial)
            .ok_or_else(|| Error::DeviceGone { id: id.to_string() })?;
        let key = (device.serial.clone(), device.transport_id);
        let known = self
            .serials
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned();
        if let Some(s) = known {
            return Ok(Some(s));
        }
        let read = self.adb.hardware_serial(&device.serial).await?;
        self.serials
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, read.clone());
        Ok(Some(read))
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        self.connect_on(id, DisplayMode::Main).await
    }

    async fn connect_on(&self, id: &str, display: DisplayMode) -> Result<Arc<dyn Device>, Error> {
        let (device, lane) = lane_of(id);
        let serial = device.strip_prefix("android:").unwrap_or(device);
        let connecting = AndroidDevice::connect(&self.adb, serial, display, lane);
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
