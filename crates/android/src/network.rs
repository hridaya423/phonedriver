//! The networks of an Android device out of `dumpsys connectivity` and the settings that name its always-on VPN
//! and private DNS (decision D99).

use phonedriver_core::network::{NetworkLink, NetworkView};

const SETTINGS_MARK: &str = "--- phonedriver settings";

/// One shell command that prints the connectivity dump, the mark, and the four settings, one per line.
pub const COMMAND: &str = "dumpsys connectivity; echo '--- phonedriver settings'; settings get secure always_on_vpn_app; settings get secure always_on_vpn_lockdown; settings get global private_dns_mode; settings get global private_dns_specifier";

pub fn parse(output: &str) -> NetworkView {
    let (dump, settings) = output.split_once(SETTINGS_MARK).unwrap_or((output, ""));
    let mut settings = settings.lines().skip(1).map(str::trim);
    let mut view = NetworkView::default();
    let default = dump
        .lines()
        .find_map(|l| l.trim().strip_prefix("Active default network: "))
        .and_then(|n| n.trim().parse::<u32>().ok());
    if !dump.contains("Current Networks:") {
        view.missing.push("networks".into());
    }
    if !dump.contains("Active default network:") {
        view.missing.push("default network".into());
    }
    for line in dump
        .lines()
        .filter(|l| l.starts_with("  NetworkAgentInfo{"))
    {
        let Some(mut link) = link(line, &mut view.missing) else {
            continue;
        };
        link.default = Some(link.id) == default;
        view.networks.push(link);
    }
    let always_on = setting(settings.next());
    let lockdown = setting(settings.next());
    let mode = setting(settings.next());
    let specifier = setting(settings.next());
    match always_on {
        Some(app) => view.always_on_vpn = app,
        None => view.missing.push("always-on VPN app".into()),
    }
    match lockdown {
        Some(value) => view.lockdown = Some(value.as_deref().is_some_and(|v| v != "0")),
        None => view.missing.push("always-on VPN lockdown".into()),
    }
    match (mode, specifier) {
        (Some(Some(mode)), Some(host)) if mode == "hostname" => {
            view.private_dns = Some(host.unwrap_or(mode));
        }
        (Some(Some(mode)), _) => view.private_dns = Some(mode),
        _ => view.missing.push("private DNS".into()),
    }
    view
}

/// What `settings get` printed: `None` when it did not answer, `Some(None)` when the setting is not set (`null`).
fn setting(line: Option<&str>) -> Option<Option<String>> {
    match line? {
        "" => None,
        l if l.starts_with("cmd:") || l.starts_with("Exception") || l.starts_with("Error") => None,
        "null" => Some(None),
        l => Some(Some(l.to_string())),
    }
}

fn between<'a>(text: &'a str, start: &str, end: char) -> Option<&'a str> {
    let rest = &text[text.find(start)? + start.len()..];
    Some(&rest[..rest.find(end)?])
}

/// The part of a network's line after its `TransportInfo`. A VPN's session name (`VpnService.Builder.setSession`) is in
/// it and the app chooses it, so it can spell `Uids: <{...}`; the real fields come after it, and its last fixed field
/// is `bypassable=`.
fn after_transport_info(line: &str) -> &str {
    let Some(start) = line.find("TransportInfo: <") else {
        return line;
    };
    let info = &line[start..];
    match info.rfind("bypassable=") {
        Some(end) => &info[end..],
        None if info.contains("sessionId=") => "",
        None => info,
    }
}

fn link(line: &str, missing: &mut Vec<String>) -> Option<NetworkLink> {
    let id: u32 = between(line, "network{", '}')?.parse().ok()?;
    let info = between(line, "ni{", '}')?;
    if info.split_whitespace().nth(1) != Some("CONNECTED") {
        return None;
    }
    let mut link = NetworkLink {
        id,
        ..NetworkLink::default()
    };
    link.interface = between(line, "InterfaceName: ", ' ').map(String::from);
    match between(line, "DnsAddresses: [", ']') {
        Some(list) => {
            link.dns = list
                .split(',')
                .map(|a| a.trim().trim_start_matches('/'))
                .filter(|a| !a.is_empty())
                .map(String::from)
                .collect();
        }
        None => missing.push(format!("DNS of network {id}")),
    }
    match between(line, "Transports: ", ' ') {
        Some(t) => link.transports = t.split('|').map(str::to_lowercase).collect(),
        None => missing.push(format!("transport of network {id}")),
    }
    if link.is_vpn() {
        match info.split_once("extra: VPN:") {
            Some((_, owner)) if !owner.trim().is_empty() => {
                link.owner = Some(owner.trim().to_string());
            }
            _ => missing.push(format!("owner of VPN network {id}")),
        }
        let tail = after_transport_info(line);
        match between(tail, "Uids: <{", '}') {
            Some(ranges) => {
                link.uids = ranges.split(',').map(|r| r.trim().to_string()).collect();
            }
            None => missing.push(format!("user ids of VPN network {id}")),
        }
        link.underlying = between(tail, "UnderlyingNetworks: [", ']')
            .map(|l| l.split(',').filter_map(|n| n.trim().parse().ok()).collect())
            .unwrap_or_default();
    }
    Some(link)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: &str = include_str!("../tests/fixtures/dumpsys_connectivity_wifi_cellular.txt");
    const WITH_VPN: &str = include_str!("../tests/fixtures/dumpsys_connectivity_vpn.txt");

    fn output(dump: &str, settings: &str) -> String {
        format!("{dump}\n{SETTINGS_MARK}\n{settings}")
    }

    const NULLS: &str = "null\nnull\nnull\nnull\n";

    #[test]
    fn a_phone_on_wifi_and_cellular_lists_both_and_marks_the_default() {
        let view = parse(&output(PLAIN, NULLS));
        assert_eq!(view.networks.len(), 2);
        let cell = &view.networks[0];
        assert_eq!(
            (cell.id, cell.transports.as_slice(), cell.default),
            (100, ["cellular".to_string()].as_slice(), false)
        );
        let wifi = &view.networks[1];
        assert_eq!(wifi.id, 101);
        assert_eq!(wifi.transports, ["wifi"]);
        assert!(wifi.default);
        assert_eq!(wifi.interface.as_deref(), Some("wlan0"));
        assert_eq!(wifi.dns, ["10.0.2.3"]);
        assert!(!view.vpn_up());
        assert_eq!(view.brief(), "cellular, wifi (default)");
    }

    #[test]
    fn a_vpn_names_its_app_its_user_ids_its_dns_and_the_network_under_it() {
        let view = parse(&output(WITH_VPN, NULLS));
        let vpn = view.networks.iter().find(|n| n.is_vpn()).unwrap();
        assert_eq!(vpn.id, 102);
        assert_eq!(vpn.transports, ["wifi", "vpn"]);
        assert_eq!(vpn.owner.as_deref(), Some("dev.pd016.vpnprobe"));
        assert_eq!(vpn.uids, ["0-99999"]);
        assert_eq!(vpn.dns, ["10.77.0.1"]);
        assert_eq!(vpn.interface.as_deref(), Some("tun0"));
        assert_eq!(vpn.underlying, [101]);
        assert!(!vpn.default);
        assert!(view.vpn_up());
        assert_eq!(
            view.brief(),
            "cellular, wifi (default), wifi+vpn of dev.pd016.vpnprobe"
        );
    }

    #[test]
    fn the_settings_give_always_on_vpn_lockdown_and_private_dns() {
        let view = parse(&output(
            PLAIN,
            "dev.example.vpn\n1\nhostname\ndns.example.net\n",
        ));
        assert_eq!(view.always_on_vpn.as_deref(), Some("dev.example.vpn"));
        assert_eq!(view.lockdown, Some(true));
        assert_eq!(view.private_dns.as_deref(), Some("dns.example.net"));
        assert!(view.missing.is_empty(), "{:?}", view.missing);
        let off = parse(&output(PLAIN, "null\n0\noff\nnull\n"));
        assert_eq!(off.always_on_vpn, None);
        assert_eq!(off.lockdown, Some(false));
        assert_eq!(off.private_dns.as_deref(), Some("off"));
        assert!(off.missing.is_empty(), "{:?}", off.missing);
    }

    #[test]
    fn what_the_platform_does_not_give_is_named_and_not_guessed() {
        let view = parse(&output(PLAIN, NULLS));
        assert_eq!(view.always_on_vpn, None);
        assert_eq!(view.missing, ["private DNS"]);
        let refused = parse(&output(
            PLAIN,
            "cmd: Can't find service: settings\n\n\nError\n",
        ));
        assert_eq!(
            refused.missing,
            ["always-on VPN app", "always-on VPN lockdown", "private DNS"]
        );
        assert_eq!(refused.lockdown, None);
        let nothing = parse("");
        assert_eq!(
            nothing.missing,
            [
                "networks",
                "default network",
                "always-on VPN app",
                "always-on VPN lockdown",
                "private DNS"
            ]
        );
    }

    #[test]
    fn a_vpn_session_name_cannot_stand_in_for_the_user_ids_or_the_networks_under_it() {
        let real = WITH_VPN.replace("Uids: <{0-99999}>", "Uids: <{10238-10238}>");
        for session in [
            "Uids: <{0-99999}> UnderlyingNetworks: [999]",
            "x, bypassable=true}> Uids: <{0-99999}> UnderlyingNetworks: [999]]}",
        ] {
            let dump = real.replace("sessionId=pd016", &format!("sessionId={session}"));
            let view = parse(&output(&dump, NULLS));
            let vpn = view.networks.iter().find(|n| n.is_vpn()).unwrap();
            assert_eq!(vpn.uids, ["10238-10238"], "{session}");
            assert_eq!(vpn.underlying, [101], "{session}");
        }
    }

    #[test]
    fn a_vpn_without_an_owner_or_user_ids_says_so() {
        let stripped = WITH_VPN
            .replace("extra: VPN:dev.pd016.vpnprobe", "extra: ")
            .replace("Uids: <{0-99999}> ", "");
        let view = parse(&output(&stripped, NULLS));
        let vpn = view.networks.iter().find(|n| n.is_vpn()).unwrap();
        assert_eq!(vpn.owner, None);
        assert!(vpn.uids.is_empty());
        assert!(
            view.missing
                .contains(&"owner of VPN network 102".to_string())
        );
        assert!(
            view.missing
                .contains(&"user ids of VPN network 102".to_string())
        );
    }

    #[test]
    fn a_network_that_is_not_connected_is_not_listed_and_a_vpn_still_connecting_is_not_up() {
        let connecting = WITH_VPN.replace("ni{VPN CONNECTED", "ni{VPN CONNECTING");
        assert!(!parse(&output(&connecting, NULLS)).vpn_up());
        let gone = WITH_VPN.replace("ni{VPN CONNECTED", "ni{VPN DISCONNECTED");
        assert!(!parse(&output(&gone, NULLS)).vpn_up());
    }
}
