//! The remote pairing a computer without Xcode keeps for a device on Wi-Fi (architecture 7.1, decision D103). Made
//! once over the cable, it opens a tunnel to the developer services that lockdown closes over the network.

use std::net::{IpAddr, SocketAddr, SocketAddrV6};
use std::path::{Path, PathBuf};
use std::time::Duration;

use idevice::remote_pairing::errors::RemotePairingError;
use idevice::remote_pairing::{
    PeerDevice, RemotePairingClient, RemotePairingLockdownService, RpPairingFile, RpPairingSocket,
    connect_tls_psk_tunnel_native,
};
use idevice::rsd::RsdHandshake;
use idevice::{IdeviceError, IdeviceService};
use phonedriver_core::error::{Error, UserAction};
use tokio::net::TcpStream;

use crate::transport::{self, Attached, Tunnel};

/// What a device announces on the network once a computer has paired with it this way.
pub const SERVICE: &str = "_remotepairing._tcp.local.";

/// Over the cable the device asks for a pairing code and shows none. It accepts this one, the code idevice sends
/// when a device asks for consent instead (iPhone17,5, iOS 26.5, 2026-11-20).
const CODE: &str = "000000";

fn path(home: &Path, udid: &str) -> Result<PathBuf, Error> {
    Ok(home
        .join("ios-wifi-pairing")
        .join(format!("{}.plist", transport::file_udid(udid)?)))
}

/// The record `pair` kept for `udid`.
pub fn read(home: &Path, udid: &str) -> Option<RpPairingFile> {
    RpPairingFile::from_bytes(&std::fs::read(path(home, udid).ok()?).ok()?).ok()
}

/// The key that tells the device's announcements apart, from the record `pair` kept for `udid`.
pub fn key(home: &Path, udid: &str) -> Option<Vec<u8>> {
    read(home, udid)?.alt_irk().map(<[u8]>::to_vec)
}

fn write(home: &Path, udid: &str, record: &RpPairingFile) -> Result<(), Error> {
    transport::write_private(&path(home, udid)?, &record.to_bytes())
}

/// Deletes the record kept for `udid`, if there is one.
pub fn forget(home: &Path, udid: &str) -> Result<(), Error> {
    let path = path(home, udid)?;
    match std::fs::remove_file(&path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(Error::Transport {
            detail: format!("cannot delete {}: {e}", path.display()),
        }),
        _ => Ok(()),
    }
}

/// Whether a `SERVICE` announcement with this `identifier` and `auth_tag` comes from the device whose record holds
/// `key`.
pub fn announced_by(key: &[u8], identifier: &str, auth_tag: &str) -> bool {
    PeerDevice::validate_auth_tag(key, identifier, auth_tag)
}

pub fn socket_addr(addr: IpAddr, scope: Option<u32>, port: u16) -> SocketAddr {
    match (addr, scope) {
        (IpAddr::V6(v6), Some(scope)) => SocketAddr::V6(SocketAddrV6::new(v6, port, 0, scope)),
        _ => SocketAddr::new(addr, port),
    }
}

/// Pairs with the device's `remotepairingdeviced` over the cable and keeps the record in `home`. A record that still
/// verifies is kept; one without the device's public key (made before 0.1.7) cannot verify the device, so the cable
/// pairs again and the new record holds the key. The cable pairing already trusts this computer, so nothing shows on
/// the device.
pub async fn pair(device: &Attached, home: &Path) -> Result<(), Error> {
    let id = format!("ios:{}", device.udid);
    let lost = |e| transport::classify(&id, &e);
    let provider = device.provider().await.map_err(lost)?;
    let socket = transport::lockdown_service(
        provider.as_ref(),
        RemotePairingLockdownService::service_name(),
    )
    .await
    .map_err(lost)?;
    let mut client = RemotePairingLockdownService::new(socket)
        .into_client(transport::LABEL)
        .map_err(lost)?;
    let mut record =
        read(home, &device.udid).unwrap_or_else(|| RpPairingFile::generate(transport::LABEL));
    client
        .connect(&mut record, || async { CODE.to_string() })
        .await
        .map_err(lost)?;
    write(home, &device.udid, &record)
}

/// Opens the tunnel through the remote pairing service the device `udid` announces at `addr`, and its RSD service
/// list, or fails after `transport::ANSWER_WITHIN`. It only verifies the record: a new pairing needs the cable.
/// Pair-verify checks that the peer signs with the device's key from the record before this computer signs anything
/// (decision D106), so a record without that key asks for the cable before connecting.
pub async fn open(
    udid: &str,
    addr: SocketAddr,
    record: &mut RpPairingFile,
) -> Result<Tunnel, Error> {
    open_within(transport::ANSWER_WITHIN, udid, addr, record).await
}

async fn open_within(
    limit: Duration,
    udid: &str,
    addr: SocketAddr,
    record: &mut RpPairingFile,
) -> Result<Tunnel, Error> {
    if record.device_public_key.is_none() {
        return Err(Error::NeedsUser {
            action: UserAction::PairForWifi,
        });
    }
    transport::answered_within(limit, udid, async {
        tunnel(addr, record)
            .await
            .map_err(|e| refused(udid, addr, e))
    })
    .await
}

fn refused(udid: &str, addr: SocketAddr, e: IdeviceError) -> Error {
    match e {
        IdeviceError::RemotePairing(RemotePairingError::NoDeviceKey) => Error::NeedsUser {
            action: UserAction::PairForWifi,
        },
        IdeviceError::RemotePairing(RemotePairingError::DeviceNotVerified) => Error::Transport {
            detail: format!(
                "ios:{udid}: what answers at {addr} did not prove it is the device this computer paired with, so nothing was sent to it; if it is that device, plug it in, unlock it and run `phonedriver ios wifi on` again"
            ),
        },
        e => transport::classify(&format!("ios:{udid}"), &e),
    }
}

/// The software TCP stack is set up the way idevice's `CoreDeviceProxy::create_software_tunnel` sets it up for the
/// cable.
async fn tunnel(addr: SocketAddr, record: &mut RpPairingFile) -> Result<Tunnel, IdeviceError> {
    let stream = TcpStream::connect(addr).await?;
    let mut client = RemotePairingClient::new(RpPairingSocket::new(stream), transport::LABEL);
    client.attempt_pair_verify().await?;
    client.validate_pairing(record).await?;
    let mut listener = addr;
    listener.set_port(client.create_tcp_listener().await?);
    let tunnel =
        connect_tls_psk_tunnel_native(TcpStream::connect(listener).await?, client.encryption_key())
            .await?;
    let info = tunnel.info.clone();
    let mut adapter = idevice::tcp::adapter::Adapter::new(
        Box::new(tunnel.into_inner()),
        info.client_address.parse()?,
        info.server_address.parse()?,
    );
    adapter.set_mss((info.mtu as usize).saturating_sub(60));
    let mut adapter = adapter.to_async_handle();
    let stream = adapter.connect(info.server_rsd_port).await?;
    let rsd = RsdHandshake::new(stream).await?;
    Ok(Tunnel { adapter, rsd })
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;
    use idevice::remote_pairing::compute_auth_tag;

    use super::*;

    const KEY: [u8; 16] = [7; 16];

    fn tag(key: &[u8; 16], identifier: &str) -> String {
        STANDARD.encode(compute_auth_tag(key, identifier))
    }

    #[test]
    fn an_announcement_matches_only_the_device_the_record_was_made_with() {
        let ours = tag(&KEY, "ID-2");
        assert!(announced_by(&KEY, "ID-2", &ours));
        assert!(!announced_by(&[8; 16], "ID-2", &ours));
        assert!(!announced_by(&KEY, "ID-3", &ours));
        assert!(!announced_by(&KEY, "ID-2", "not a tag"));
        assert!(!announced_by(&KEY[..8], "ID-2", &ours));
    }

    #[test]
    fn a_record_is_kept_per_device_in_the_home_and_forgetting_it_twice_is_fine() {
        let home = tempfile::tempdir().unwrap();
        assert!(read(home.path(), "A").is_none());
        let mut record = RpPairingFile::generate("phonedriver");
        record.alt_irk = Some(KEY.to_vec());
        write(home.path(), "A", &record).unwrap();
        let back = read(home.path(), "A").unwrap();
        assert_eq!(back.identifier(), record.identifier());
        assert_eq!(back.alt_irk(), Some(&KEY[..]));
        assert_eq!(key(home.path(), "A"), Some(KEY.to_vec()));
        assert!(read(home.path(), "B").is_none());
        assert_eq!(key(home.path(), "B"), None);
        forget(home.path(), "A").unwrap();
        assert!(read(home.path(), "A").is_none());
        forget(home.path(), "A").unwrap();
    }

    #[tokio::test]
    async fn a_device_that_takes_the_connection_and_never_answers_fails_the_tunnel_instead_of_waiting_for_good()
     {
        let silent = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = silent.local_addr().unwrap();
        let holding = tokio::spawn(async move {
            let mut held = vec![];
            while let Ok((stream, _)) = silent.accept().await {
                held.push(stream);
            }
        });
        let mut record = RpPairingFile::generate("phonedriver");
        record.device_public_key = Some([1; 32]);
        let opened = open_within(Duration::from_millis(200), "A", addr, &mut record).await;
        holding.abort();
        let detail = match opened {
            Err(Error::Transport { detail }) => detail,
            Err(e) => panic!("{e:?}"),
            Ok(_) => panic!("a tunnel opened to a socket that never answers"),
        };
        assert!(detail.starts_with("ios:A did not answer"), "{detail}");
    }

    #[tokio::test]
    async fn a_record_made_before_the_device_key_was_kept_asks_for_the_cable_before_connecting() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut record = RpPairingFile::generate("phonedriver");
        let opened = open_within(Duration::from_secs(2), "A", addr, &mut record).await;
        assert!(
            matches!(
                opened,
                Err(Error::NeedsUser {
                    action: UserAction::PairForWifi
                })
            ),
            "{:?}",
            opened.err()
        );
        let reached = tokio::time::timeout(Duration::from_millis(100), listener.accept()).await;
        assert!(
            reached.is_err(),
            "it connected with a record it cannot verify the device with"
        );
    }

    #[test]
    fn a_peer_that_does_not_prove_it_is_the_paired_device_is_named_as_such() {
        let addr: SocketAddr = "192.168.1.20:49152".parse().unwrap();
        let e = refused(
            "A",
            addr,
            IdeviceError::RemotePairing(RemotePairingError::DeviceNotVerified),
        );
        let Error::Transport { detail } = &e else {
            panic!("{e:?}")
        };
        assert!(detail.starts_with("ios:A: "), "{detail}");
        assert!(detail.contains("192.168.1.20:49152"), "{detail}");
        assert!(detail.contains("phonedriver ios wifi on"), "{detail}");
        assert_eq!(
            refused(
                "A",
                addr,
                IdeviceError::RemotePairing(RemotePairingError::NoDeviceKey)
            ),
            Error::NeedsUser {
                action: UserAction::PairForWifi
            }
        );
    }

    #[test]
    fn a_udid_with_other_characters_never_becomes_a_path() {
        let home = tempfile::tempdir().unwrap();
        let record = RpPairingFile::generate("phonedriver");
        for udid in ["../A", "A/B", "", "A.plist", "A\\B"] {
            assert!(write(home.path(), udid, &record).is_err(), "{udid}");
            assert!(read(home.path(), udid).is_none(), "{udid}");
            assert!(forget(home.path(), udid).is_err(), "{udid}");
        }
        assert!(write(home.path(), "00008110-001E04900239801E", &record).is_ok());
        assert!(!home.path().join("A.plist").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_record_is_replaced_whole_only_when_it_changed_and_is_left_for_its_owner_only() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let home = tempfile::tempdir().unwrap();
        let mut record = RpPairingFile::generate("phonedriver");
        record.alt_irk = Some(KEY.to_vec());
        write(home.path(), "A", &record).unwrap();
        let file = path(home.path(), "A").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        let before = std::fs::metadata(&file).unwrap().ino();
        write(home.path(), "A", &record).unwrap();
        let same = std::fs::metadata(&file).unwrap();
        assert_eq!(same.ino(), before, "an unchanged record was written again");
        assert_eq!(same.permissions().mode() & 0o777, 0o600);
        record.device_public_key = Some([3; 32]);
        write(home.path(), "A", &record).unwrap();
        let changed = std::fs::metadata(&file).unwrap();
        assert_ne!(
            changed.ino(),
            before,
            "a changed record was rewritten in place"
        );
        assert_eq!(changed.permissions().mode() & 0o777, 0o600);
        assert_eq!(
            read(home.path(), "A").unwrap().device_public_key,
            Some([3; 32])
        );
        let names: Vec<_> = std::fs::read_dir(file.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["A.plist"]);
    }

    #[test]
    fn a_link_local_address_keeps_its_interface_in_the_socket_address() {
        assert_eq!(
            socket_addr("192.168.1.164".parse().unwrap(), None, 49152).to_string(),
            "192.168.1.164:49152"
        );
        assert_eq!(
            socket_addr("fe80::1".parse().unwrap(), Some(14), 49152).to_string(),
            "[fe80::1%14]:49152"
        );
    }
}
