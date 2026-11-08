//! The pure parts of cable-free setup: the QR payload Android's "Pair device with QR code" reads, the lines of
//! `host:mdns:services`, and the addresses in `ip -o -4 addr show`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceKind {
    Pairing,
    Connect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MdnsService {
    pub instance: String,
    pub kind: ServiceKind,
    pub addr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetAddr {
    pub iface: String,
    pub ip: String,
}

/// The text of the QR code that Developer options > Wireless debugging > Pair device with QR code scans.
pub fn qr_payload(name: &str, password: &str) -> String {
    format!("WIFI:T:ADB;S:{name};P:{password};;")
}

/// Lines of `<instance>\t<service type>\t<ip>:<port>` as `adb`'s `host:mdns:services` returns them. Only the
/// pairing and connect services are kept.
pub fn parse_mdns_services(text: &str) -> Vec<MdnsService> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let (instance, service, addr) = (parts.next()?, parts.next()?, parts.next()?);
            let service = service.trim_end_matches('.');
            let service = service.strip_suffix("._tcp").unwrap_or(service);
            let kind = match service {
                "_adb-tls-pairing" => ServiceKind::Pairing,
                "_adb-tls-connect" => ServiceKind::Connect,
                _ => return None,
            };
            Some(MdnsService {
                instance: instance.to_string(),
                kind,
                addr: addr.trim().to_string(),
            })
        })
        .collect()
}

/// The non-loopback IPv4 addresses in `ip -o -4 addr show`, one per line.
pub fn parse_ip_addrs(text: &str) -> Vec<NetAddr> {
    text.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let iface = words.nth(1)?.trim_end_matches(':');
            let cidr = words.skip_while(|w| *w != "inet").nth(1)?;
            let ip = cidr.split('/').next()?;
            if ip.starts_with("127.") {
                return None;
            }
            Some(NetAddr {
                iface: iface.to_string(),
                ip: ip.to_string(),
            })
        })
        .collect()
}

fn is_tailscale_range(ip: &str) -> bool {
    let mut octets = ip.split('.').filter_map(|o| o.parse::<u8>().ok());
    matches!((octets.next(), octets.next()), (Some(100), Some(64..=127)))
}

pub fn iface_label(addr: &NetAddr) -> String {
    let iface = &addr.iface;
    let kind = if iface.starts_with("wlan") {
        "Wi-Fi"
    } else if iface.starts_with("tun") || iface.starts_with("tailscale") {
        if is_tailscale_range(&addr.ip) {
            "Tailscale"
        } else {
            "VPN"
        }
    } else if iface.starts_with("rmnet") || iface.starts_with("ccmni") {
        "mobile data"
    } else {
        return iface.clone();
    };
    format!("{iface} ({kind})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_qr_payload_is_the_wifi_adb_string_android_studio_shows() {
        assert_eq!(
            qr_payload("phonedriver-4F9K2M", "7QX3NB0HT5WA"),
            "WIFI:T:ADB;S:phonedriver-4F9K2M;P:7QX3NB0HT5WA;;"
        );
    }

    const OPENSCREEN: &str = "adb-R5CW30ABCDE-xYz12a\t_adb-tls-connect\t192.168.1.20:41233\nphonedriver-4F9K2M\t_adb-tls-pairing\t192.168.1.20:37099\n";
    const BONJOUR: &str = "adb-R5CW30ABCDE-xYz12a\t_adb-tls-connect._tcp.\t192.168.1.20:41233\nphonedriver-4F9K2M\t_adb-tls-pairing._tcp.\t192.168.1.20:37099\nold-phone\t_adb._tcp.\t192.168.1.30:5555\n";

    #[test]
    fn mdns_lines_from_the_openscreen_resolver_in_adb_transport_mdns_cpp_parse() {
        assert_eq!(
            parse_mdns_services(OPENSCREEN),
            vec![
                MdnsService {
                    instance: "adb-R5CW30ABCDE-xYz12a".into(),
                    kind: ServiceKind::Connect,
                    addr: "192.168.1.20:41233".into()
                },
                MdnsService {
                    instance: "phonedriver-4F9K2M".into(),
                    kind: ServiceKind::Pairing,
                    addr: "192.168.1.20:37099".into()
                },
            ]
        );
    }

    #[test]
    fn mdns_lines_from_the_bonjour_resolver_keep_pairing_and_connect_and_drop_plain_adb() {
        let kinds: Vec<_> = parse_mdns_services(BONJOUR)
            .into_iter()
            .map(|s| (s.instance, s.kind))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("adb-R5CW30ABCDE-xYz12a".to_string(), ServiceKind::Connect),
                ("phonedriver-4F9K2M".to_string(), ServiceKind::Pairing),
            ]
        );
    }

    const ADB_37_0_1_MACOS_DNS_SD_REGISTERED: &str =
        "phonedriver-TEST\t_adb-tls-pairing._tcp\t10.31.234.154:37099\n";

    #[test]
    fn mdns_output_observed_from_adb_37_0_1_on_macos_parses() {
        assert_eq!(
            parse_mdns_services(ADB_37_0_1_MACOS_DNS_SD_REGISTERED),
            vec![MdnsService {
                instance: "phonedriver-TEST".into(),
                kind: ServiceKind::Pairing,
                addr: "10.31.234.154:37099".into()
            }]
        );
    }

    #[test]
    fn mdns_text_that_is_empty_or_has_the_client_header_has_no_services() {
        assert!(parse_mdns_services("").is_empty());
        assert!(parse_mdns_services("List of discovered mdns services\n").is_empty());
    }

    const IP_EMULATOR_API_37: &str = "1: lo    inet 127.0.0.1/8 scope host lo\\       valid_lft forever preferred_lft forever\n15: eth0    inet 10.0.2.15/24 brd 10.0.2.255 scope global eth0\\       valid_lft forever preferred_lft forever\n16: wlan0    inet 10.0.2.18/24 brd 10.0.2.255 scope global dynamic wlan0\\       valid_lft 43772sec preferred_lft 43772sec\n";

    #[test]
    fn ip_addresses_from_an_api_37_emulator_skip_loopback() {
        assert_eq!(
            parse_ip_addrs(IP_EMULATOR_API_37),
            vec![
                NetAddr {
                    iface: "eth0".into(),
                    ip: "10.0.2.15".into()
                },
                NetAddr {
                    iface: "wlan0".into(),
                    ip: "10.0.2.18".into()
                },
            ]
        );
    }

    #[test]
    fn interfaces_are_labelled_by_what_they_are() {
        let label = |iface: &str, ip: &str| {
            iface_label(&NetAddr {
                iface: iface.into(),
                ip: ip.into(),
            })
        };
        assert_eq!(label("wlan0", "192.168.1.20"), "wlan0 (Wi-Fi)");
        assert_eq!(label("tun0", "100.101.102.103"), "tun0 (Tailscale)");
        assert_eq!(label("tun0", "10.8.0.2"), "tun0 (VPN)");
        assert_eq!(
            label("rmnet_data1", "10.20.30.40"),
            "rmnet_data1 (mobile data)"
        );
        assert_eq!(label("eth0", "10.0.2.15"), "eth0");
    }
}
