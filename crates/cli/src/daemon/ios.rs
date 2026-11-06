//! Booted iOS simulators, driven through the PhoneUseRunner (Simulator tier).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use phone_use_core::device::Device;
use phone_use_core::error::{Error, UserAction};
use phone_use_ios::coredevice::CoreDevice;
use phone_use_ios::device::IosDevice;
use phone_use_ios::simulator::Simulator;
use phone_use_ios::{discovery, transport};
use tokio::sync::Mutex;
use tokio::time::Instant;

use super::progress::after_a_second;
use super::{DeviceSource, Listed};

/// `simctl list` takes about 70 ms on the Mac mini and the daemon lists devices on every request, which added that
/// much to every iOS call; a listing this recent is reused.
const LIST_TTL: Duration = Duration::from_secs(2);

fn starting_runner(name: &str, home: &std::path::Path) -> String {
    format!(
        "starting the iOS runner on {name} (the first run builds it; logs are in {})",
        home.join("runner").display()
    )
}

pub struct SimSource {
    home: PathBuf,
    listed: Mutex<Option<(Instant, Vec<Simulator>)>>,
}

impl SimSource {
    /// `home` holds the runner build and its logs.
    pub fn new(home: PathBuf) -> SimSource {
        SimSource {
            home,
            listed: Mutex::new(None),
        }
    }

    async fn simulators(&self) -> Result<Vec<Simulator>, Error> {
        let mut listed = self.listed.lock().await;
        if let Some((at, sims)) = listed.as_ref()
            && at.elapsed() < LIST_TTL
        {
            return Ok(sims.clone());
        }
        let sims = Simulator::list().await?;
        *listed = Some((Instant::now(), sims.clone()));
        Ok(sims)
    }
}

#[async_trait]
impl DeviceSource for SimSource {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(self
            .simulators()
            .await?
            .into_iter()
            .map(|s| Listed {
                id: format!("sim:{}", s.udid),
                model: s.name,
                os: Some(s.os),
                tier: "simulator".into(),
                capabilities: IosDevice::CAPABILITIES,
            })
            .collect())
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        let udid = id.strip_prefix("sim:").unwrap_or(id);
        let sim = Simulator::list()
            .await?
            .into_iter()
            .find(|s| s.udid == udid)
            .ok_or_else(|| Error::DeviceGone { id: id.to_string() })?;
        let connecting = IosDevice::connect(&sim, &self.home);
        let device = after_a_second(|| starting_runner(&sim.name, &self.home), connecting).await?;
        Ok(Arc::new(device))
    }
}

/// An iOS 27 device is driven through the runner unless `PHONE_USE_IOS_TIER=coredevice` asks for the CoreDevice
/// services: that tier's screen stream stops showing app changes after the first seconds (progress.md, 2026-11-06).
fn tier_of(found: discovery::Tier) -> discovery::Tier {
    match (found, std::env::var("PHONE_USE_IOS_TIER").as_deref()) {
        (discovery::Tier::CoreDevice, Ok("coredevice")) => discovery::Tier::CoreDevice,
        _ => discovery::Tier::Runner,
    }
}

/// Wired physical devices (ids `ios:<udid>`), driven through the PhoneUseRunner signed with the Apple developer
/// team in `PHONE_USE_TEAM`.
pub struct PhysicalSource {
    home: PathBuf,
    described: Mutex<Vec<discovery::Found>>,
}

impl PhysicalSource {
    pub fn new(home: PathBuf) -> PhysicalSource {
        PhysicalSource {
            home,
            described: Mutex::new(Vec::new()),
        }
    }

    /// Every wired device, described once per udid: describing an iOS 27 device opens a CoreDevice tunnel, and a
    /// tunnel opened while the display stream runs stopped its frames (observed 2026-11-06 on the iPad: the
    /// daemon listed devices on every request and the screenshot went stale).
    async fn found(&self) -> Result<Vec<(transport::Wired, discovery::Found)>, Error> {
        let mut described = self.described.lock().await;
        let mut out = Vec::new();
        for wired in transport::wired()
            .await
            .map_err(|e| transport::classify(&"ios".to_string(), &e))?
        {
            let known = described.iter().find(|f| f.udid == wired.udid).cloned();
            let found = match known {
                Some(f) => f,
                None => {
                    let f = discovery::describe(&wired).await?;
                    described.push(f.clone());
                    f
                }
            };
            out.push((wired, found));
        }
        Ok(out)
    }
}

#[async_trait]
impl DeviceSource for PhysicalSource {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(self
            .found()
            .await?
            .into_iter()
            .map(|(_, d)| Listed {
                id: format!("ios:{}", d.udid),
                model: d.name,
                os: Some(d.os),
                tier: tier_of(d.tier).name().into(),
                capabilities: match tier_of(d.tier) {
                    discovery::Tier::CoreDevice => CoreDevice::CAPABILITIES,
                    discovery::Tier::Runner => IosDevice::CAPABILITIES,
                },
            })
            .collect())
    }

    fn blocker(&self, id: &str) -> Option<&'static str> {
        let udid = id.strip_prefix("ios:").unwrap_or(id);
        let runner_tier = self.described.try_lock().map_or(true, |known| {
            known
                .iter()
                .find(|f| f.udid == udid)
                .is_none_or(|f| tier_of(f.tier) == discovery::Tier::Runner)
        });
        (runner_tier && std::env::var_os("PHONE_USE_TEAM").is_none())
            .then_some("PHONE_USE_TEAM is not set")
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        let udid = id.strip_prefix("ios:").unwrap_or(id);
        let (wired, found) = self
            .found()
            .await?
            .into_iter()
            .find(|(w, _)| w.udid == udid)
            .ok_or_else(|| Error::DeviceGone { id: id.to_string() })?;
        if tier_of(found.tier) == discovery::Tier::CoreDevice {
            return Ok(Arc::new(CoreDevice::connect(&wired, &found.model).await?));
        }
        let team = std::env::var("PHONE_USE_TEAM").map_err(|_| Error::NeedsUser {
            action: UserAction::SetSigningTeam,
        })?;
        let connecting = IosDevice::connect_physical(&wired, &found.model, &team, &self.home);
        let device =
            after_a_second(|| starting_runner(&found.model, &self.home), connecting).await?;
        Ok(Arc::new(device))
    }
}
