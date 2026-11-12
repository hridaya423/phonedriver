//! Finding a physical device on the local network, for devices that `phonedriver ios wifi on` remembered
//! (architecture 7.1).

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mdns_sd::{ResolvedService, ScopedIp, ServiceDaemon, ServiceEvent};
use phonedriver_core::error::Error;

use crate::transport::{self, Attached, Link};

const SERVICE: &str = "_apple-mobdev2._tcp.local.";
const FIRST_ANSWERS: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(50);

fn file(home: &Path) -> PathBuf {
    home.join("ios-wifi")
}

/// The udids `phonedriver ios wifi on` turned Wi-Fi connections on for.
pub fn remembered(home: &Path) -> Vec<String> {
    std::fs::read_to_string(file(home))
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

fn write(home: &Path, udids: &[String]) -> Result<(), Error> {
    let path = file(home);
    let io = |e: std::io::Error| Error::Transport {
        detail: format!("cannot write {}: {e}", path.display()),
    };
    std::fs::create_dir_all(home).map_err(io)?;
    std::fs::write(
        &path,
        udids.iter().map(|u| format!("{u}\n")).collect::<String>(),
    )
    .map_err(io)
}

pub fn remember(home: &Path, udid: &str) -> Result<(), Error> {
    let mut udids = remembered(home);
    if !udids.iter().any(|u| u == udid) {
        udids.push(udid.to_string());
    }
    write(home, &udids)
}

pub fn forget(home: &Path, udid: &str) -> Result<(), Error> {
    let udids: Vec<String> = remembered(home).into_iter().filter(|u| u != udid).collect();
    write(home, &udids)
}

/// What a device announces. From iOS 26.4: an identifier and tags that only a computer it trusts can recompute.
/// Before that: no tags, and its Wi-Fi hardware address at the start of the service name.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Advert {
    mac: String,
    identifier: Vec<u8>,
    tags: Vec<Vec<u8>>,
    addrs: Vec<(IpAddr, Option<u32>)>,
}

fn advert(service: &ResolvedService) -> Option<Advert> {
    let mac = service
        .fullname
        .split('@')
        .next()
        .unwrap_or_default()
        .to_string();
    let identifier = service
        .get_property_val("identifier")
        .flatten()
        .unwrap_or_default()
        .to_vec();
    let tags = service
        .txt_properties
        .iter()
        .filter(|p| p.key().starts_with("authTag"))
        .filter_map(|p| p.val().map(<[u8]>::to_vec))
        .collect();
    let addrs = service
        .addresses
        .iter()
        .map(|a| match a {
            ScopedIp::V6(v6) => (a.to_ip_addr(), Some(v6.scope_id().index)),
            _ => (a.to_ip_addr(), None),
        })
        .collect();
    Some(Advert {
        mac,
        identifier,
        tags,
        addrs,
    })
}

/// What the cable pairing left on this computer that tells its device apart on the network.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Known {
    udid: String,
    host_id: String,
    mac: String,
}

fn is_from(advert: &Advert, known: &Known) -> bool {
    if advert.tags.is_empty() {
        return !known.mac.is_empty() && advert.mac.eq_ignore_ascii_case(&known.mac);
    }
    let tags: Vec<&[u8]> = advert.tags.iter().map(Vec::as_slice).collect();
    idevice::mdns::txt_record_matches(known.host_id.as_bytes(), &advert.identifier, &tags)
}

/// IPv4 first: it needs no interface to be named.
fn address(advert: &Advert) -> Option<(IpAddr, Option<u32>)> {
    advert
        .addrs
        .iter()
        .find(|(a, _)| a.is_ipv4())
        .or(advert.addrs.first())
        .copied()
}

fn find(adverts: &[Advert], known: &Known) -> Option<Attached> {
    let (addr, scope) = adverts
        .iter()
        .filter(|a| is_from(a, known))
        .find_map(address)?;
    Some(Attached {
        udid: known.udid.clone(),
        link: Link::Network { addr, scope },
    })
}

/// A running browse for the devices in `udids`. Dropping it stops the browse.
pub struct Lan {
    udids: Vec<String>,
    started: Instant,
    seen: Arc<Mutex<HashMap<String, Advert>>>,
    daemon: ServiceDaemon,
}

impl Lan {
    pub fn watch(udids: Vec<String>) -> Result<Lan, Error> {
        let failed = |e: mdns_sd::Error| Error::Transport {
            detail: format!("cannot browse the local network for iOS devices: {e}"),
        };
        let daemon = ServiceDaemon::new().map_err(failed)?;
        let events = daemon.browse(SERVICE).map_err(failed)?;
        let seen: Arc<Mutex<HashMap<String, Advert>>> = Arc::default();
        let into = seen.clone();
        std::thread::spawn(move || {
            while let Ok(event) = events.recv() {
                let Ok(mut seen) = into.lock() else { return };
                match event {
                    ServiceEvent::ServiceResolved(service) => {
                        if let Some(advert) = advert(&service) {
                            seen.insert(service.fullname.clone(), advert);
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        seen.remove(&fullname);
                    }
                    _ => {}
                }
            }
        });
        Ok(Lan {
            udids,
            started: Instant::now(),
            seen,
            daemon,
        })
    }

    pub fn udids(&self) -> &[String] {
        &self.udids
    }

    fn now(&self, known: &[Known]) -> Vec<Attached> {
        let adverts: Vec<Advert> = self
            .seen
            .lock()
            .map(|s| s.values().cloned().collect())
            .unwrap_or_default();
        known.iter().filter_map(|k| find(&adverts, k)).collect()
    }

    /// The remembered devices that answer on the network now. A browse younger than 2 s waits for its first
    /// answers, so a one-shot command sees a device that is there.
    pub async fn attached(&self) -> Vec<Attached> {
        let mut known = Vec::new();
        for udid in &self.udids {
            if let Ok(pairing) = transport::pair_record(udid).await {
                known.push(Known {
                    udid: udid.clone(),
                    host_id: pairing.host_id,
                    mac: pairing.wifi_mac_address,
                });
            }
        }
        loop {
            let found = self.now(&known);
            if found.len() == known.len() || self.started.elapsed() >= FIRST_ANSWERS {
                return found;
            }
            tokio::time::sleep(POLL).await;
        }
    }
}

impl Drop for Lan {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;

    use super::*;

    const HOST: &str = "5A3C1D2E-0000-4000-8000-1234567890AB";

    fn advert_for(host_id: &str, identifier: &str, addrs: &[(&str, Option<u32>)]) -> Advert {
        let tag = idevice::mdns::derive_auth_tag(host_id.as_bytes(), identifier.as_bytes());
        Advert {
            mac: "aa:bb:cc:dd:ee:ff".into(),
            identifier: identifier.as_bytes().to_vec(),
            tags: vec![
                STANDARD.encode([0u8; 8]).into_bytes(),
                STANDARD.encode(tag).into_bytes(),
            ],
            addrs: addrs
                .iter()
                .map(|(a, scope)| (a.parse().unwrap(), *scope))
                .collect(),
        }
    }

    fn known(host_id: &str, mac: &str) -> Known {
        Known {
            udid: "UDID".into(),
            host_id: host_id.into(),
            mac: mac.into(),
        }
    }

    #[test]
    fn an_older_ios_with_no_tags_is_found_by_its_wifi_address_and_never_a_tagged_one() {
        let mut old = advert_for(HOST, "", &[("192.168.0.50", None)]);
        old.tags.clear();
        old.mac = "A6:F0:1F:00:00:01".into();
        assert!(find(&[old.clone()], &known("x", "a6:f0:1f:00:00:01")).is_some());
        assert!(find(&[old.clone()], &known("x", "a6:f0:1f:00:00:02")).is_none());
        assert!(find(&[old], &known("x", "")).is_none());
        let tagged = advert_for("someone-else", "ID", &[("192.168.0.9", None)]);
        assert!(find(&[tagged], &known(HOST, "aa:bb:cc:dd:ee:ff")).is_none());
    }

    #[test]
    fn a_device_is_found_by_the_tag_only_its_trusted_computer_can_compute() {
        let adverts = [
            advert_for("someone-else", "ID-1", &[("192.168.0.9", None)]),
            advert_for(HOST, "ID-2", &[("192.168.0.136", None)]),
        ];
        assert_eq!(
            find(&adverts, &known(HOST, "")),
            Some(Attached {
                udid: "UDID".into(),
                link: Link::Network {
                    addr: "192.168.0.136".parse().unwrap(),
                    scope: None
                },
            })
        );
        assert_eq!(find(&adverts[..1], &known(HOST, "")), None);
    }

    #[test]
    fn ipv4_is_preferred_and_a_link_local_address_keeps_its_interface() {
        let both = advert_for(
            HOST,
            "ID",
            &[("fe80::1", Some(14)), ("192.168.0.136", None)],
        );
        assert_eq!(
            address(&both),
            Some(("192.168.0.136".parse().unwrap(), None))
        );
        let v6 = advert_for(HOST, "ID", &[("fe80::1", Some(14))]);
        assert_eq!(address(&v6), Some(("fe80::1".parse().unwrap(), Some(14))));
    }

    #[test]
    fn remembering_and_forgetting_a_device_round_trips_through_the_file() {
        let home = std::env::temp_dir().join(format!("pd-lan-{}", std::process::id()));
        assert_eq!(remembered(&home), Vec::<String>::new());
        remember(&home, "A").unwrap();
        remember(&home, "B").unwrap();
        remember(&home, "A").unwrap();
        assert_eq!(remembered(&home), ["A", "B"]);
        forget(&home, "A").unwrap();
        assert_eq!(remembered(&home), ["B"]);
        std::fs::remove_dir_all(&home).unwrap();
    }
}
