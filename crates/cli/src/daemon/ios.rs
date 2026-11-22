//! Booted iOS simulators, driven through the PhonedriverRunner (Simulator tier).

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use phonedriver_core::device::Device;
use phonedriver_core::error::{DeviceSummary, Error};
use phonedriver_ios::coredevice::CoreDevice;
use phonedriver_ios::device::IosDevice;
use phonedriver_ios::lan::Lan;
use phonedriver_ios::setup::XCODE;
use phonedriver_ios::simulator::Simulator;
use phonedriver_ios::{discovery, transport};
use tokio::sync::Mutex;
use tokio::time::Instant;

use super::progress::after_a_second;
use super::{Booted, DeviceSource, Listed, named};

/// `simctl list` takes about 70 ms on the Mac mini and the daemon lists devices on every request, which added that
/// much to every iOS call; a listing this recent is reused.
const LIST_TTL: Duration = Duration::from_secs(2);
/// A request that names a Simulator the last listing (younger than this) holds is answered from it while a new one is
/// taken in the background: with the 2 s reuse alone, every other call of a benchmark run waited 110 ms for `simctl`.
/// A Simulator the listing does not hold, and every request that names none, still waits for a new listing.
const LIST_STALE: Duration = Duration::from_secs(30);

#[derive(Debug, PartialEq)]
enum Use {
    Kept,
    KeptAndRefresh,
    Taken,
}

fn use_listing(age: Option<Duration>, holds_requested: bool, refreshing: bool) -> Use {
    match age {
        Some(a) if a < LIST_TTL => Use::Kept,
        Some(a) if a < LIST_STALE && holds_requested && refreshing => Use::Kept,
        Some(a) if a < LIST_STALE && holds_requested => Use::KeptAndRefresh,
        _ => Use::Taken,
    }
}

/// The last `simctl` listing; `generation` moves when a boot drops it, so a refresh begun before cannot put it back.
#[derive(Default)]
struct SimListing {
    generation: u64,
    taken: Option<(Instant, Vec<Simulator>)>,
    refreshing: bool,
}

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

fn starting_installed_runner(name: &str, home: &std::path::Path) -> String {
    format!(
        "starting the iOS runner installed on {name} (logs are in {})",
        home.join("runner").display()
    )
}

pub struct SimSource {
    home: PathBuf,
    listed: Arc<Mutex<SimListing>>,
}

impl SimSource {
    /// `home` holds the runner build and its logs.
    pub fn new(home: PathBuf) -> SimSource {
        SimSource {
            home,
            listed: Arc::default(),
        }
    }

    /// The Simulators `simctl` lists; `requested`, a `sim:` id, may be answered from a listing up to `LIST_STALE` old.
    async fn simulators(&self, requested: Option<&str>) -> Result<Vec<Simulator>, Error> {
        let mut listed = self.listed.lock().await;
        let age = listed.taken.as_ref().map(|(at, _)| at.elapsed());
        let holds = |sims: &[Simulator]| {
            requested.is_some_and(|r| {
                sims.iter()
                    .any(|s| r.strip_prefix("sim:") == Some(s.udid.as_str()))
            })
        };
        let holds_requested = listed.taken.as_ref().is_some_and(|(_, sims)| holds(sims));
        match (
            use_listing(age, holds_requested, listed.refreshing),
            &listed.taken,
        ) {
            (Use::Kept, Some((_, sims))) => return Ok(sims.clone()),
            (Use::KeptAndRefresh, Some((_, sims))) => {
                let sims = sims.clone();
                listed.refreshing = true;
                let (shared, generation) = (self.listed.clone(), listed.generation);
                tokio::spawn(async move {
                    let fresh = Simulator::list().await;
                    let mut listed = shared.lock().await;
                    listed.refreshing = false;
                    if let Ok(fresh) = fresh
                        && listed.generation == generation
                    {
                        listed.taken = Some((Instant::now(), fresh));
                    }
                });
                return Ok(sims);
            }
            _ => {}
        }
        let sims = Simulator::list().await?;
        listed.taken = Some((Instant::now(), sims.clone()));
        Ok(sims)
    }

    fn listed(sims: Vec<Simulator>) -> Vec<Listed> {
        sims.into_iter()
            .map(|s| Listed {
                id: format!("sim:{}", s.udid),
                model: s.name,
                os: Some(s.os),
                tier: "simulator".into(),
                capabilities: IosDevice::CAPABILITIES,
            })
            .collect()
    }
}

#[async_trait]
impl DeviceSource for SimSource {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(SimSource::listed(self.simulators(None).await?))
    }

    async fn list_for(&self, requested: Option<&str>) -> Result<Vec<Listed>, Error> {
        Ok(SimSource::listed(self.simulators(requested).await?))
    }

    async fn off(&self) -> Vec<DeviceSummary> {
        Simulator::off()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|s| DeviceSummary {
                id: format!("sim:{}", s.udid),
                model: s.name,
                os: s.os,
            })
            .collect()
    }

    async fn boot(&self, name: &str) -> Result<Option<Booted>, Error> {
        let off = self.off().await;
        let Some(device) = off.into_iter().find(|d| named(name, &d.id, &d.model)) else {
            return Ok(None);
        };
        Simulator::boot(device.id.trim_start_matches("sim:")).await?;
        let mut listed = self.listed.lock().await;
        listed.generation += 1;
        listed.taken = None;
        Ok(Some(Booted {
            device,
            already: false,
        }))
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
                false => Some(Lan::watch(&self.home, remembered)?),
            };
        }
        browse.cabled = udids;
        Ok(transport::with_network(cabled, browse.lan.as_ref()).await)
    }

    /// Every attached device, described once per udid: describing an iOS 27 device opens a CoreDevice tunnel, and a
    /// tunnel opened while the display stream runs stopped its frames (observed 2026-11-06 on the iPad: the
    /// daemon listed devices on every request and the screenshot went stale).
    async fn found(&self) -> Result<Vec<(transport::Attached, discovery::Found)>, Error> {
        described_only(self.each_found().await?)
    }

    async fn each_found(
        &self,
    ) -> Result<Vec<(transport::Attached, Result<discovery::Found, Error>)>, Error> {
        let mut described = self.described.lock().await;
        let mut out = Vec::new();
        for attached in self.attached().await? {
            let known = described.iter().find(|f| f.udid == attached.udid).cloned();
            let found = match known {
                Some(f) => Ok(f),
                None => discovery::describe(&attached).await.inspect(|f| {
                    described.push(f.clone());
                }),
            };
            out.push((attached, found));
        }
        Ok(out)
    }
}

/// The devices that could be described. One that could not (locked, not trusted yet) is left out while others list,
/// and its error is the answer only when it is the only device: it made the whole list fail for every device.
fn described_only<A, F>(each: Vec<(A, Result<F, Error>)>) -> Result<Vec<(A, F)>, Error> {
    let mut out = Vec::new();
    let mut first_error = None;
    for (attached, found) in each {
        match found {
            Ok(f) => out.push((attached, f)),
            Err(e) => {
                first_error.get_or_insert(e);
            }
        }
    }
    match first_error {
        Some(e) if out.is_empty() => Err(e),
        _ => Ok(out),
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
        (XCODE && runner_tier && self.team().is_err()).then_some("no signing team")
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        let udid = id.strip_prefix("ios:").unwrap_or(id);
        let (attached, found) = self
            .each_found()
            .await?
            .into_iter()
            .find(|(w, _)| w.udid == udid)
            .ok_or_else(|| Error::DeviceGone { id: id.to_string() })?;
        let found = found?;
        if tier_of(found.tier) == discovery::Tier::CoreDevice {
            return Ok(Arc::new(
                CoreDevice::connect(&attached, &found.model).await?,
            ));
        }
        if !XCODE {
            phonedriver_ios::setup::mount_cached(&attached, &self.home).await?;
            let connecting = IosDevice::connect_installed(&attached, &found.model, &self.home);
            let device = after_a_second(
                || starting_installed_runner(&found.model, &self.home),
                connecting,
            )
            .await?;
            return Ok(Arc::new(device));
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

    #[test]
    fn a_device_that_cannot_be_described_leaves_the_others_listed() {
        let gone = || Error::DeviceGone { id: "ios:b".into() };
        let listed = described_only(vec![("a", Ok(1)), ("b", Err(gone())), ("c", Ok(3))]).unwrap();
        assert_eq!(listed, vec![("a", 1), ("c", 3)]);
        let alone = described_only::<&str, i32>(vec![("b", Err(gone()))]).unwrap_err();
        assert!(matches!(alone, Error::DeviceGone { .. }), "{alone:?}");
        assert!(described_only::<&str, i32>(vec![]).unwrap().is_empty());
    }

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

    #[test]
    fn a_listing_is_kept_two_seconds_and_answers_for_a_simulator_it_holds_while_a_new_one_is_taken_until_thirty()
     {
        let s = Duration::from_secs;
        assert_eq!(use_listing(None, true, false), Use::Taken);
        assert_eq!(use_listing(Some(s(1)), false, false), Use::Kept);
        assert_eq!(use_listing(Some(s(5)), true, false), Use::KeptAndRefresh);
        assert_eq!(use_listing(Some(s(5)), true, true), Use::Kept);
        assert_eq!(use_listing(Some(s(5)), false, false), Use::Taken);
        assert_eq!(use_listing(Some(s(40)), true, true), Use::Taken);
    }
}
