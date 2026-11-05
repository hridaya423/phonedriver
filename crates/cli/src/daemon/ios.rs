//! Booted iOS simulators, driven through the PhoneUseRunner (Simulator tier).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use phone_use_core::device::Device;
use phone_use_core::error::Error;
use phone_use_ios::device::IosDevice;
use phone_use_ios::simulator::Simulator;
use tokio::sync::Mutex;
use tokio::time::Instant;

use super::{DeviceSource, Listed};

/// `simctl list` takes about 70 ms on the Mac mini and the daemon lists devices on every request, which added that
/// much to every iOS call; a listing this recent is reused.
const LIST_TTL: Duration = Duration::from_secs(2);

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
        Ok(Arc::new(IosDevice::connect(&sim, &self.home).await?))
    }
}
