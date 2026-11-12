//! Booted iOS simulators, driven through the PhonedriverRunner (Simulator tier).

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use phonedriver_core::device::Device;
use phonedriver_core::error::Error;
use phonedriver_ios::coredevice::CoreDevice;
use phonedriver_ios::device::IosDevice;
use phonedriver_ios::lan::Lan;
use phonedriver_ios::simulator::Simulator;
use phonedriver_ios::{discovery, transport};
use tokio::sync::Mutex;
use tokio::time::Instant;

use super::progress::after_a_second;
use super::{DeviceSource, Listed};

/// `simctl list` takes about 70 ms on the Mac mini and the daemon lists devices on every request, which added that
/// much to every iOS call; a listing this recent is reused.
const LIST_TTL: Duration = Duration::from_secs(2);

/// The usbmuxd listing of attached devices is reused for this long, failures included: on Windows a missing usbmuxd
/// cost 2 s per request. usbmuxd attach and detach events are not exposed by the `idevice` client we use, so a
/// device plugged in is seen within this time rather than at once.
const ATTACHED_TTL: Duration = Duration::from_secs(5);

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

/// An iOS 27 device is driven through the runner unless `PHONEDRIVER_IOS_TIER=coredevice` asks for the CoreDevice
/// services: that tier's screen can fall behind when the host is starved of CPU (architecture 7.2, spike I6).
fn tier_of(found: discovery::Tier) -> discovery::Tier {
    match (found, std::env::var("PHONEDRIVER_IOS_TIER").as_deref()) {
        (discovery::Tier::CoreDevice, Ok("coredevice")) => discovery::Tier::CoreDevice,
        _ => discovery::Tier::Runner,
    }
}

/// Physical devices usbmuxd lists (ids `ios:<udid>`), driven through the PhonedriverRunner signed with the Apple developer
/// team `phonedriver_ios::team::signing_team` finds.
pub struct PhysicalSource {
    home: PathBuf,
    described: Mutex<Vec<discovery::Found>>,
    attached: Mutex<Option<Listing>>,
    browse: Mutex<Browse>,
    team: OnceLock<String>,
}

#[derive(Default)]
struct Browse {
    lan: Option<Lan>,
    cabled: Vec<String>,
}

/// A browse is kept while it serves the remembered devices, and replaced when a cable comes off: the device then
/// withdraws the announcement it made on the cable's own network interface, the browse drops the device, and it
/// took up to 45 s to find it again on Wi-Fi (observed 2026-11-12). A new browse asks at once.
fn keeps_browse(
    browsing: Option<&[String]>,
    remembered: &[String],
    was_cabled: &[String],
    cabled: &[String],
) -> bool {
    browsing == Some(remembered) && was_cabled.iter().all(|u| cabled.contains(u))
}

type Listing = (Instant, Result<Vec<transport::Attached>, Error>);

impl PhysicalSource {
    pub fn new(home: PathBuf) -> PhysicalSource {
        PhysicalSource {
            home,
            described: Mutex::new(Vec::new()),
            attached: Mutex::new(None),
            browse: Mutex::new(Browse::default()),
            team: OnceLock::new(),
        }
    }

    /// A team once found is kept. A failure is not, so a certificate made later is seen without a restart.
    fn team(&self) -> Result<String, Error> {
        if let Some(team) = self.team.get() {
            return Ok(team.clone());
        }
        let team = phonedriver_ios::team::signing_team()?;
        Ok(self.team.get_or_init(|| team).clone())
    }

    async fn attached(&self) -> Result<Vec<transport::Attached>, Error> {
        let mut cached = self.attached.lock().await;
        if let Some((at, answer)) = cached.as_ref()
            && at.elapsed() < ATTACHED_TTL
        {
            return answer.clone();
        }
        let answer = self.list_attached().await;
        *cached = Some((Instant::now(), answer.clone()));
        answer
    }

    async fn list_attached(&self) -> Result<Vec<transport::Attached>, Error> {
        let cabled = transport::cabled()
            .await
            .map_err(|e| transport::classify(&"ios".to_string(), &e))?;
        let udids: Vec<String> = cabled.iter().map(|d| d.udid.clone()).collect();
        let remembered = phonedriver_ios::lan::remembered(&self.home);
        let mut browse = self.browse.lock().await;
        if !keeps_browse(
            browse.lan.as_ref().map(Lan::udids),
            &remembered,
            &browse.cabled,
            &udids,
        ) {
            browse.lan = match remembered.is_empty() {
                true => None,
                false => Some(Lan::watch(remembered)?),
            };
        }
        browse.cabled = udids;
        Ok(transport::with_network(cabled, browse.lan.as_ref()).await)
    }

    /// Every attached device, described once per udid: describing an iOS 27 device opens a CoreDevice tunnel, and a
    /// tunnel opened while the display stream runs stopped its frames (observed 2026-11-06 on the iPad: the
    /// daemon listed devices on every request and the screenshot went stale).
    async fn found(&self) -> Result<Vec<(transport::Attached, discovery::Found)>, Error> {
        let mut described = self.described.lock().await;
        let mut out = Vec::new();
        for attached in self.attached().await? {
            let known = described.iter().find(|f| f.udid == attached.udid).cloned();
            let found = match known {
                Some(f) => f,
                None => {
                    let f = discovery::describe(&attached).await?;
                    described.push(f.clone());
                    f
                }
            };
            out.push((attached, found));
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
        (runner_tier && self.team().is_err()).then_some("no signing team")
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        let udid = id.strip_prefix("ios:").unwrap_or(id);
        let (attached, found) = self
            .found()
            .await?
            .into_iter()
            .find(|(w, _)| w.udid == udid)
            .ok_or_else(|| Error::DeviceGone { id: id.to_string() })?;
        if tier_of(found.tier) == discovery::Tier::CoreDevice {
            return Ok(Arc::new(
                CoreDevice::connect(&attached, &found.model).await?,
            ));
        }
        let team = self.team()?;
        let connecting = IosDevice::connect_physical(&attached, &found.model, &team, &self.home);
        let device =
            after_a_second(|| starting_runner(&found.model, &self.home), connecting).await?;
        Ok(Arc::new(device))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(udids: &[&str]) -> Vec<String> {
        udids.iter().map(|u| u.to_string()).collect()
    }

    #[test]
    fn the_browse_is_replaced_when_a_cable_comes_off_or_the_remembered_devices_change() {
        let remembered = ids(&["A"]);
        let browsing = Some(remembered.as_slice());
        assert!(keeps_browse(
            browsing,
            &remembered,
            &ids(&["A"]),
            &ids(&["A"])
        ));
        assert!(keeps_browse(browsing, &remembered, &[], &ids(&["A"])));
        assert!(!keeps_browse(browsing, &remembered, &ids(&["A"]), &[]));
        assert!(!keeps_browse(browsing, &ids(&["A", "B"]), &[], &[]));
        assert!(!keeps_browse(None, &remembered, &[], &[]));
    }
}
