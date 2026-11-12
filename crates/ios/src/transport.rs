//! Reaching physical devices through usbmuxd, lockdown and the CoreDevice tunnel; mapping idevice failures to
//! `core::Error` and the single reconnect (architecture 7.1).

use idevice::core_device_proxy::CoreDeviceProxy;
use std::net::IpAddr;
use std::sync::Arc;

use idevice::pairing_file::PairingFile;
use idevice::provider::{IdeviceProvider, TcpProvider, UsbmuxdProvider};
use idevice::rsd::RsdHandshake;
use idevice::services::lockdown::LockdownClient;
use idevice::tcp::handle::AdapterHandle;
use idevice::usbmuxd::{Connection, UsbmuxdAddr, UsbmuxdDevice};
use idevice::{IdeviceError, IdeviceService};
use phonedriver_core::error::{DeviceId, Error, UserAction};

use crate::lan::Lan;

const LABEL: &str = "phonedriver";

/// How this computer reaches a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    Usb {
        mux_id: u32,
    },
    /// Straight to the device's lockdown port. macOS usbmuxd also lists Wi-Fi devices, but it dropped an iPhone
    /// that restarted on the cable and never listed it again (observed 2026-11-12, iOS 27.0.1), so it is not used.
    Network {
        addr: IpAddr,
        scope: Option<u32>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attached {
    pub udid: String,
    pub link: Link,
}

impl Attached {
    pub async fn provider(&self) -> Result<Arc<dyn IdeviceProvider>, IdeviceError> {
        Ok(match self.link {
            Link::Usb { mux_id } => Arc::new(UsbmuxdProvider {
                addr: usbmuxd_addr(),
                tag: mux_id,
                udid: self.udid.clone(),
                device_id: mux_id,
                label: LABEL.to_string(),
            }),
            Link::Network { addr, scope } => Arc::new(TcpProvider {
                addr,
                scope_id: scope,
                pairing_file: pair_record(&self.udid).await?,
                label: LABEL.to_string(),
            }),
        })
    }
}

fn usbmuxd_addr() -> UsbmuxdAddr {
    UsbmuxdAddr::from_env_var().unwrap_or_default()
}

/// The record the cable pairing left with usbmuxd; it also opens a session over the network.
pub async fn pair_record(udid: &str) -> Result<PairingFile, IdeviceError> {
    usbmuxd_addr().connect(0).await?.get_pair_record(udid).await
}

fn cabled_of(listed: Vec<UsbmuxdDevice>) -> Vec<Attached> {
    let mut out: Vec<Attached> = Vec::new();
    for d in listed {
        if d.connection_type == Connection::Usb && !out.iter().any(|a| a.udid == d.udid) {
            out.push(Attached {
                udid: d.udid,
                link: Link::Usb {
                    mux_id: d.device_id,
                },
            });
        }
    }
    out
}

/// One entry per device; the cable wins over the network.
fn merge(mut cabled: Vec<Attached>, lan: Vec<Attached>) -> Vec<Attached> {
    for device in lan {
        if !cabled.iter().any(|a| a.udid == device.udid) {
            cabled.push(device);
        }
    }
    cabled
}

pub async fn cabled() -> Result<Vec<Attached>, IdeviceError> {
    let mut mux = usbmuxd_addr().connect(0).await?;
    Ok(cabled_of(mux.get_devices().await?))
}

/// `cabled` and, with a browse, the remembered devices that answer on the network.
pub async fn with_network(cabled: Vec<Attached>, lan: Option<&Lan>) -> Vec<Attached> {
    match lan {
        Some(lan) => merge(cabled, lan.attached().await),
        None => cabled,
    }
}

pub async fn attached(lan: Option<&Lan>) -> Result<Vec<Attached>, IdeviceError> {
    Ok(with_network(cabled().await?, lan).await)
}

const WIRELESS_DOMAIN: &str = "com.apple.mobile.wireless_lockdown";
const WIFI_KEY: &str = "EnableWifiConnections";

/// Whether the device accepts this computer over Wi-Fi.
pub async fn network_allowed(device: &Attached) -> Result<bool, IdeviceError> {
    let mut lockdown = session(device.provider().await?.as_ref()).await?;
    Ok(lockdown
        .get_value(Some(WIFI_KEY), Some(WIRELESS_DOMAIN))
        .await
        .ok()
        .and_then(|v| v.as_boolean())
        .unwrap_or(false))
}

/// Sets the switch Finder shows as "Show this iPhone when on Wi-Fi". The device then answers computers it
/// already trusts on the networks it joins.
pub async fn allow_network(device: &Attached, on: bool) -> Result<(), IdeviceError> {
    let mut lockdown = session(device.provider().await?.as_ref()).await?;
    lockdown
        .set_value(WIFI_KEY, on.into(), Some(WIRELESS_DOMAIN))
        .await
}

async fn session(provider: &dyn IdeviceProvider) -> Result<LockdownClient, IdeviceError> {
    let mut lockdown = LockdownClient::connect(provider).await?;
    let pairing = provider.get_pairing_file().await?;
    lockdown.start_session(&pairing).await?;
    Ok(lockdown)
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

pub async fn identity(device: &Attached) -> Result<Identity, IdeviceError> {
    let mut lockdown = LockdownClient::connect(device.provider().await?.as_ref()).await?;
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
    pub async fn open(device: &Attached) -> Result<Tunnel, IdeviceError> {
        let provider = device.provider().await?;
        let socket = lockdown_service(provider.as_ref(), CoreDeviceProxy::service_name()).await?;
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

    fn listed(udid: &str, mux_id: u32, connection_type: Connection) -> UsbmuxdDevice {
        UsbmuxdDevice {
            connection_type,
            udid: udid.to_string(),
            device_id: mux_id,
        }
    }

    fn on_cable(udid: &str, mux_id: u32) -> Attached {
        Attached {
            udid: udid.to_string(),
            link: Link::Usb { mux_id },
        }
    }

    fn on_wifi(udid: &str) -> Attached {
        Attached {
            udid: udid.to_string(),
            link: Link::Network {
                addr: "192.168.1.20".parse().unwrap(),
                scope: None,
            },
        }
    }

    #[test]
    fn only_the_cable_listings_of_usbmuxd_count_and_each_device_once() {
        let wifi = Connection::Network("192.168.1.20".parse().unwrap());
        assert_eq!(
            cabled_of(vec![
                listed("A", 7, wifi),
                listed("B", 3, Connection::Usb),
                listed("B", 4, Connection::Usb),
            ]),
            vec![on_cable("B", 3)]
        );
    }

    #[test]
    fn a_device_found_only_on_the_network_is_attached_and_the_cable_wins_when_both_see_it() {
        assert_eq!(
            merge(vec![on_cable("A", 3)], vec![on_wifi("A"), on_wifi("B")]),
            vec![on_cable("A", 3), on_wifi("B")]
        );
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
