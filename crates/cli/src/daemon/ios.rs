//! Booted iOS simulators, driven through the PhoneUseRunner (Simulator tier).

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use phone_use_core::device::Device;
use phone_use_core::error::Error;
use phone_use_ios::device::IosDevice;
use phone_use_ios::simulator::Simulator;

use super::{DeviceSource, Listed};

pub struct SimSource {
    home: PathBuf,
}

impl SimSource {
    /// `home` holds the runner build and its logs.
    pub fn new(home: PathBuf) -> SimSource {
        SimSource { home }
    }
}

#[async_trait]
impl DeviceSource for SimSource {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(Simulator::list()
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
