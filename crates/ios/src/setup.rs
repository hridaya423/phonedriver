//! What a physical device needs before phone-use can drive it, and `phone-use ios setup` (architecture 7.1, 7.4).

use idevice::IdeviceService;
use idevice::services::amfi::AmfiClient;
use idevice::services::mobile_image_mounter::ImageMounter;
use phone_use_core::error::{Error, UserAction};

use crate::transport::{self, Wired, lockdown_service};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub developer_mode: bool,
    pub image_mounted: bool,
}

/// Reads the state; a device that does not trust this Mac fails with `NeedsUser(TrustComputer)`.
pub async fn status(device: &Wired) -> Result<Status, Error> {
    let id = format!("ios:{}", device.udid);
    let lost = |e| transport::classify(&id, &e);
    let provider = device.provider();
    let socket = lockdown_service(&provider, AmfiClient::service_name())
        .await
        .map_err(lost)?;
    let developer_mode = AmfiClient::new(socket)
        .get_developer_mode_status()
        .await
        .map_err(lost)?;
    let socket = lockdown_service(&provider, ImageMounter::service_name())
        .await
        .map_err(lost)?;
    let image_mounted = !ImageMounter::new(socket)
        .copy_devices()
        .await
        .map_err(lost)?
        .is_empty();
    Ok(Status {
        developer_mode,
        image_mounted,
    })
}

/// Shows the Developer Mode switch in Settings; the person turns it on and restarts the device.
pub async fn reveal_developer_mode(device: &Wired) -> Result<(), Error> {
    let id = format!("ios:{}", device.udid);
    let lost = |e| transport::classify(&id, &e);
    let socket = lockdown_service(&device.provider(), AmfiClient::service_name())
        .await
        .map_err(lost)?;
    AmfiClient::new(socket)
        .reveal_developer_mode_option_in_ui()
        .await
        .map_err(lost)
}

/// Mounts the developer disk image with Xcode's own tool, which holds the personalized files and talks to Apple's
/// signing server; this is the one step that sends anything to Apple (decision D9).
pub async fn mount_image(udid: &str) -> Result<(), Error> {
    let out = tokio::process::Command::new("xcrun")
        .args(["devicectl", "manage", "ddis", "update", "--device", udid])
        .output()
        .await
        .map_err(|e| Error::Transport {
            detail: format!("cannot run xcrun devicectl: {e}"),
        })?;
    if out.status.success() {
        Ok(())
    } else {
        Err(Error::Transport {
            detail: format!(
                "devicectl could not mount the developer disk image: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        })
    }
}

pub fn developer_mode_action() -> UserAction {
    UserAction::EnableDeveloperMode
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turning_developer_mode_on_is_a_user_action_with_the_settings_path() {
        assert!(developer_mode_action().hint().contains("Developer Mode"));
    }
}
