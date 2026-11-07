//! Reaching physical devices through usbmuxd, lockdown and the CoreDevice tunnel; mapping idevice failures to
//! `core::Error` and the single reconnect (architecture 7.1).

use idevice::core_device_proxy::CoreDeviceProxy;
use idevice::provider::{IdeviceProvider, UsbmuxdProvider};
use idevice::rsd::RsdHandshake;
use idevice::services::lockdown::LockdownClient;
use idevice::tcp::handle::AdapterHandle;
use idevice::usbmuxd::{Connection, UsbmuxdAddr};
use idevice::{IdeviceError, IdeviceService};
use phonedriver_core::error::{DeviceId, Error, UserAction};

const LABEL: &str = "phonedriver";

/// A device usbmuxd sees over USB. Network-only devices are left out: their pairing needs a separate approval
/// on the device, which v1 does not ask for (`docs/spikes/ios.md`, "Device access probe").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wired {
    pub udid: String,
    pub mux_id: u32,
}

impl Wired {
    pub fn provider(&self) -> UsbmuxdProvider {
        idevice::usbmuxd::UsbmuxdDevice {
            connection_type: Connection::Usb,
            udid: self.udid.clone(),
            device_id: self.mux_id,
        }
        .to_provider(usbmuxd_addr(), LABEL)
    }
}

fn usbmuxd_addr() -> UsbmuxdAddr {
    UsbmuxdAddr::from_env_var().unwrap_or_default()
}

pub async fn wired() -> Result<Vec<Wired>, IdeviceError> {
    let mut mux = usbmuxd_addr().connect(0).await?;
    let mut out: Vec<Wired> = Vec::new();
    for d in mux.get_devices().await? {
        if d.connection_type == Connection::Usb && !out.iter().any(|w| w.udid == d.udid) {
            out.push(Wired {
                udid: d.udid,
                mux_id: d.device_id,
            });
        }
    }
    Ok(out)
}

/// What lockdown says about a device before any session: these keys need no pairing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    pub model: String,
    pub os: String,
}

async fn text(lockdown: &mut LockdownClient, key: &str) -> Result<String, IdeviceError> {
    Ok(lockdown
        .get_value(Some(key), None)
        .await?
        .as_string()
        .unwrap_or_default()
        .to_string())
}

pub async fn identity(device: &Wired) -> Result<Identity, IdeviceError> {
    let mut lockdown = LockdownClient::connect(&device.provider()).await?;
    Ok(Identity {
        name: text(&mut lockdown, "DeviceName").await?,
        model: text(&mut lockdown, "ProductType").await?,
        os: text(&mut lockdown, "ProductVersion").await?,
    })
}

/// What `IdeviceService::connect` does, spelled out because blaze's frontend cannot lay out that trait's default
/// method for any service type.
pub async fn lockdown_service(
    provider: &dyn IdeviceProvider,
    name: impl Into<String>,
) -> Result<idevice::Idevice, IdeviceError> {
    let mut lockdown = LockdownClient::connect(provider).await?;
    let pairing = provider.get_pairing_file().await?;
    let legacy = lockdown.start_session(&pairing).await?;
    let (port, ssl) = lockdown.start_service(name).await?;
    let mut socket = provider.connect(port).await?;
    if ssl {
        socket.start_session(&pairing, legacy).await?;
    }
    Ok(socket)
}

/// The CoreDevice tunnel over the userspace TCP stack, with its RSD service list.
pub struct Tunnel {
    pub adapter: AdapterHandle,
    pub rsd: RsdHandshake,
}

impl Tunnel {
    pub async fn open(device: &Wired) -> Result<Tunnel, IdeviceError> {
        let provider = device.provider();
        let socket = lockdown_service(&provider, CoreDeviceProxy::service_name()).await?;
        let proxy = CoreDeviceProxy::new(socket).await?;
        let rsd_port = proxy.tunnel_info().server_rsd_port;
        let mut adapter = proxy.create_software_tunnel()?.to_async_handle();
        let stream = adapter.connect(rsd_port).await?;
        let rsd = RsdHandshake::new(stream).await?;
        Ok(Tunnel { adapter, rsd })
    }

    pub fn services(&self) -> Vec<String> {
        let mut names: Vec<String> = self.rsd.services.keys().cloned().collect();
        names.sort();
        names
    }
}

fn is_lost(err: &IdeviceError) -> bool {
    matches!(
        err,
        IdeviceError::Socket(_)
            | IdeviceError::NoEstablishedConnection
            | IdeviceError::DeviceNotFound
            | IdeviceError::Heartbeat(_)
    )
}

pub fn classify(id: &DeviceId, err: &IdeviceError) -> Error {
    let needs = |action| Error::NeedsUser { action };
    match err {
        IdeviceError::InvalidHostID
        | IdeviceError::PairingDialogResponsePending
        | IdeviceError::UserDeniedPairing => needs(UserAction::TrustComputer),
        IdeviceError::DeveloperModeNotEnabled => needs(UserAction::EnableDeveloperMode),
        IdeviceError::ImageNotMounted => needs(UserAction::MountDeveloperImage),
        IdeviceError::DeviceLocked | IdeviceError::PasswordProtected => {
            needs(UserAction::UnlockDevice)
        }
        e if is_lost(e) => Error::DeviceGone { id: id.clone() },
        e => Error::Transport {
            detail: e.to_string(),
        },
    }
}

/// Runs `op`; if the connection was lost, calls `reconnect` once and runs `op` once more.
pub async fn with_reconnect<T>(
    id: &DeviceId,
    mut op: impl AsyncFnMut() -> Result<T, IdeviceError>,
    mut reconnect: impl AsyncFnMut() -> Result<(), IdeviceError>,
) -> Result<T, Error> {
    let gone = || Error::DeviceGone { id: id.clone() };
    match op().await {
        Ok(v) => Ok(v),
        Err(e) if is_lost(&e) => {
            reconnect().await.map_err(|_| gone())?;
            op().await.map_err(|e| classify(id, &e))
        }
        Err(e) => Err(classify(id, &e)),
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use idevice::IdeviceError;
    use phonedriver_core::error::{Error, UserAction};

    use super::*;

    fn id() -> String {
        "ios:00008110-000A1B2C3D4E".to_string()
    }

    fn needs(action: UserAction) -> Error {
        Error::NeedsUser { action }
    }

    #[test]
    fn a_device_that_does_not_know_this_host_needs_trust() {
        assert_eq!(
            classify(&id(), &IdeviceError::InvalidHostID),
            needs(UserAction::TrustComputer)
        );
        assert_eq!(
            classify(&id(), &IdeviceError::PairingDialogResponsePending),
            needs(UserAction::TrustComputer)
        );
        assert_eq!(
            classify(&id(), &IdeviceError::UserDeniedPairing),
            needs(UserAction::TrustComputer)
        );
    }

    #[test]
    fn developer_mode_off_needs_the_toggle() {
        assert_eq!(
            classify(&id(), &IdeviceError::DeveloperModeNotEnabled),
            needs(UserAction::EnableDeveloperMode)
        );
    }

    #[test]
    fn a_missing_image_needs_mounting_and_names_the_setup_command() {
        let e = classify(&id(), &IdeviceError::ImageNotMounted);
        assert_eq!(e, needs(UserAction::MountDeveloperImage));
        assert!(
            UserAction::MountDeveloperImage
                .hint()
                .contains("phonedriver ios setup")
        );
    }

    #[test]
    fn a_locked_device_needs_unlocking() {
        assert_eq!(
            classify(&id(), &IdeviceError::DeviceLocked),
            needs(UserAction::UnlockDevice)
        );
        assert_eq!(
            classify(&id(), &IdeviceError::PasswordProtected),
            needs(UserAction::UnlockDevice)
        );
    }

    #[test]
    fn a_dead_socket_is_a_gone_device() {
        let e = classify(
            &id(),
            &IdeviceError::Socket(io::Error::from(io::ErrorKind::BrokenPipe)),
        );
        assert_eq!(e, Error::DeviceGone { id: id() });
        assert_eq!(
            classify(&id(), &IdeviceError::NoEstablishedConnection),
            Error::DeviceGone { id: id() }
        );
    }

    #[test]
    fn an_unclassified_failure_is_a_transport_error_with_its_text() {
        let e = classify(&id(), &IdeviceError::GetProhibited);
        assert_eq!(
            e,
            Error::Transport {
                detail: "this request was prohibited".into()
            }
        );
    }

    fn lost() -> IdeviceError {
        IdeviceError::Socket(io::Error::from(io::ErrorKind::ConnectionReset))
    }

    #[tokio::test]
    async fn a_lost_tunnel_reconnects_once_and_the_retry_wins() {
        let mut ops = 0;
        let mut reconnects = 0;
        let out = with_reconnect(
            &id(),
            async || {
                ops += 1;
                if ops == 1 { Err(lost()) } else { Ok(7) }
            },
            async || {
                reconnects += 1;
                Ok(())
            },
        )
        .await;
        assert_eq!(out, Ok(7));
        assert_eq!((ops, reconnects), (2, 1));
    }

    #[tokio::test]
    async fn a_second_loss_is_device_gone_without_a_second_reconnect() {
        let mut ops = 0;
        let mut reconnects = 0;
        let out: Result<(), Error> = with_reconnect(
            &id(),
            async || {
                ops += 1;
                Err(lost())
            },
            async || {
                reconnects += 1;
                Ok(())
            },
        )
        .await;
        assert_eq!(out, Err(Error::DeviceGone { id: id() }));
        assert_eq!((ops, reconnects), (2, 1));
    }

    #[tokio::test]
    async fn a_failed_reconnect_is_device_gone_and_the_op_is_not_retried() {
        let mut ops = 0;
        let out: Result<(), Error> = with_reconnect(
            &id(),
            async || {
                ops += 1;
                Err(lost())
            },
            async || Err(lost()),
        )
        .await;
        assert_eq!(out, Err(Error::DeviceGone { id: id() }));
        assert_eq!(ops, 1);
    }

    #[tokio::test]
    async fn an_error_that_is_not_a_loss_does_not_reconnect() {
        let mut reconnects = 0;
        let out: Result<(), Error> = with_reconnect(
            &id(),
            async || Err(IdeviceError::DeveloperModeNotEnabled),
            async || {
                reconnects += 1;
                Ok(())
            },
        )
        .await;
        assert_eq!(out, Err(needs(UserAction::EnableDeveloperMode)));
        assert_eq!(reconnects, 0);
    }
}
