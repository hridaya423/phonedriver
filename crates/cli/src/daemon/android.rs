//! Android devices through the adb server.

use std::sync::Arc;

use async_trait::async_trait;
use phone_use_android::adb::{Adb, DeviceState};
use phone_use_android::device::AndroidDevice;
use phone_use_core::device::{Capabilities, Device};
use phone_use_core::error::Error;

use super::{DeviceSource, Listed};

pub struct AdbSource {
    adb: Adb,
}

impl AdbSource {
    pub fn from_env() -> AdbSource {
        AdbSource {
            adb: Adb::from_env(),
        }
    }
}

#[async_trait]
impl DeviceSource for AdbSource {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(self
            .adb
            .devices()
            .await?
            .into_iter()
            .filter(|d| d.state == DeviceState::Device)
            .map(|d| Listed {
                id: format!("android:{}", d.serial),
                model: d.model.unwrap_or_else(|| "unknown".into()),
                os: None,
                tier: "agent".into(),
                capabilities: Capabilities::all(),
            })
            .collect())
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        let serial = id.strip_prefix("android:").unwrap_or(id);
        Ok(Arc::new(AndroidDevice::connect(&self.adb, serial).await?))
    }
}
