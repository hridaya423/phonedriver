//! `phonedriver pair` and `phonedriver untether`: cable-free Android setup.

use std::io::IsTerminal;
use std::process::ExitCode;
use std::time::Duration;

use phonedriver_android::adb::{Adb, AdbDevice, DeviceState};
use phonedriver_android::wireless::{
    MdnsService, NetAddr, ServiceKind, iface_label, is_mobile_data, parse_ip_addrs,
    parse_mdns_services, qr_payload,
};
use qrcode::QrCode;
use qrcode::render::unicode::Dense1x2;

const POLL: Duration = Duration::from_secs(1);
const SCAN_WINDOW: Duration = Duration::from_secs(120);
const CONNECT_WINDOW: Duration = Duration::from_secs(30);
const TCPIP_PORT: u16 = 5555;
const CONNECT_ATTEMPT: Duration = Duration::from_secs(8);

fn find_pairing<'a>(services: &'a [MdnsService], name: &str) -> Option<&'a MdnsService> {
    services
        .iter()
        .find(|s| s.kind == ServiceKind::Pairing && s.instance == name)
}

fn host_of(addr: &str) -> &str {
    addr.rsplit_once(':').map_or(addr, |(host, _)| host)
}

fn find_connect<'a>(services: &'a [MdnsService], pairing_addr: &str) -> Option<&'a MdnsService> {
    services
        .iter()
        .find(|s| s.kind == ServiceKind::Connect && host_of(&s.addr) == host_of(pairing_addr))
}

fn connected_serial(devices: &[AdbDevice], connect: &MdnsService) -> Option<String> {
    devices
        .iter()
        .find(|d| {
            d.state == DeviceState::Device
                && (d.serial == connect.addr || d.serial.contains(&connect.instance))
        })
        .map(|d| d.serial.clone())
}

fn render_qr(payload: &str, color: bool) -> String {
    let Ok(code) = QrCode::new(payload.as_bytes()) else {
        return String::new();
    };
    let text = code.render::<Dense1x2>().build();
    if !color {
        return text;
    }
    let mut out = String::new();
    for line in text.lines() {
        out.push_str(&format!("\x1b[30;47m{line}\x1b[0m\n"));
    }
    out
}

fn pick_device(devices: &[AdbDevice], wanted: Option<&str>) -> Result<String, String> {
    if let Some(wanted) = wanted {
        let serial = wanted.strip_prefix("android:").unwrap_or(wanted);
        return match devices.iter().find(|d| d.serial == serial) {
            Some(d) => Ok(d.serial.clone()),
            None => Err(format!(
                "no Android device {wanted} is known to adb; `phonedriver devices` lists them"
            )),
        };
    }
    let ready: Vec<&AdbDevice> = devices
        .iter()
        .filter(|d| d.state == DeviceState::Device)
        .collect();
    match ready.as_slice() {
        [one] => Ok(one.serial.clone()),
        [] => Err("no Android device is ready. Plug the phone in with USB debugging on, or run `phonedriver pair` first".into()),
        many => Err(format!(
            "more than one Android device is connected ({}); pass --device with one of them",
            many.iter()
                .map(|d| format!("android:{}", d.serial))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn is_connected(answer: &str) -> bool {
    answer.starts_with("connected to") || answer.starts_with("already connected to")
}

fn random_text(len: usize) -> String {
    ulid::Ulid::generate().to_string()[26 - len..].to_string()
}

async fn mdns_services(adb: &Adb) -> Vec<MdnsService> {
    match adb.mdns_services().await {
        Ok(text) => parse_mdns_services(&text),
        Err(_) => Vec::new(),
    }
}

async fn pair_flow(adb: &Adb) -> Result<String, (u8, String)> {
    let fail = |message: String| (1u8, message);
    let check = adb.mdns_check().await;
    if !check
        .as_deref()
        .is_ok_and(|v| v.starts_with("mdns daemon version"))
    {
        let why = match check {
            Ok(text) => text,
            Err(e) => e.to_string(),
        };
        return Err(fail(format!(
            "adb's mDNS is not available ({why}), so the phone cannot be found by QR code. Pair with the code instead: on the phone choose Pair device with pairing code, then run `adb pair <ip:port>` and `adb connect <ip:port>` (docs/reference.md, Android over Wi-Fi)."
        )));
    }
    let name = format!("phonedriver-{}", random_text(6));
    let password = random_text(12);
    let color = std::io::stdout().is_terminal();
    print!("{}", render_qr(&qr_payload(&name, &password), color));
    println!(
        "On the phone: Settings > Developer options > Wireless debugging > Pair device with QR code. Scan this."
    );
    println!("The phone and this computer must be on the same Wi-Fi. Waiting up to 2 minutes.");

    let deadline = tokio::time::Instant::now() + SCAN_WINDOW;
    let pairing = loop {
        if let Some(found) = find_pairing(&mdns_services(adb).await, &name) {
            break found.clone();
        }
        if tokio::time::Instant::now() >= deadline {
            return Err((
                3,
                "no phone scanned the code within 2 minutes. Check that the phone is on the same Wi-Fi as this computer (guest networks and routers with client isolation block it; some routers block mDNS). If it still fails, pair with the code: choose Pair device with pairing code on the phone, then run `adb pair <ip:port>` and `adb connect <ip:port>` (docs/reference.md, Android over Wi-Fi).".into(),
            ));
        }
        tokio::time::sleep(POLL).await;
    };
    adb.pair(&pairing.addr, &password)
        .await
        .map_err(|e| fail(format!("{e}. Run `phonedriver pair` again for a new code.")))?;
    println!("paired with {}", host_of(&pairing.addr));

    let deadline = tokio::time::Instant::now() + CONNECT_WINDOW;
    loop {
        let services = mdns_services(adb).await;
        if let Some(connect) = find_connect(&services, &pairing.addr) {
            let devices = adb.devices().await.unwrap_or_default();
            if let Some(serial) = connected_serial(&devices, connect) {
                return Ok(serial);
            }
            if let Ok(answer) = adb.connect_device(&connect.addr).await
                && is_connected(&answer)
            {
                return Ok(connect.addr.clone());
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(fail(
                "paired, but the phone did not announce its debugging port within 30 seconds. Check that Wireless debugging is still on, then run `adb connect <ip:port>` with the address on the Wireless debugging screen.".into(),
            ));
        }
        tokio::time::sleep(POLL).await;
    }
}

/// `phonedriver pair`: shows a QR code for Android 11+ wireless debugging and connects the phone that scans it.
pub fn pair() -> ExitCode {
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        eprintln!("error: cannot start the async runtime");
        return ExitCode::from(1);
    };
    match rt.block_on(pair_flow(&Adb::from_env())) {
        Ok(serial) => {
            println!("connected: android:{serial}");
            println!("Try `phonedriver devices`, then `phonedriver snapshot`.");
            ExitCode::SUCCESS
        }
        Err((code, message)) => {
            eprintln!("error: {message}");
            ExitCode::from(code)
        }
    }
}

/// The addresses worth trying. A phone on mobile data only is stopped before adb is switched to TCP: on a HONOR X8
/// with Wi-Fi off, both of its addresses were mobile data and each cost an 8 s connect that could not succeed
/// (observed 2026-11-12).
fn reachable(addrs: &[NetAddr]) -> Result<Vec<&NetAddr>, String> {
    let reachable: Vec<&NetAddr> = addrs.iter().filter(|a| !is_mobile_data(a)).collect();
    if reachable.is_empty() {
        return Err("the phone is on mobile data only, and this computer cannot reach it there. Turn on Wi-Fi or Tailscale on the phone, then run this again. Nothing was changed on the phone".into());
    }
    Ok(reachable)
}

async fn untether_flow(adb: &Adb, device: Option<&str>) -> Result<String, String> {
    let devices = adb.devices().await.map_err(|e| e.to_string())?;
    let serial = pick_device(&devices, device)?;
    let out = adb
        .shell(&serial, "ip -o -4 addr show")
        .await
        .map_err(|e| e.to_string())?;
    let addrs = parse_ip_addrs(&out.stdout_text());
    if addrs.is_empty() {
        return Err("the phone has no IPv4 address on any network. Join a Wi-Fi network or Tailscale on the phone, then run this again".into());
    }
    println!("addresses on the phone:");
    for a in &addrs {
        println!("  {}  {}", a.ip, iface_label(a));
    }
    let reachable = reachable(&addrs)?;
    adb.tcpip(&serial, TCPIP_PORT)
        .await
        .map_err(|e| e.to_string())?;
    let mut answers = Vec::new();
    for a in reachable {
        let target = format!("{}:{TCPIP_PORT}", a.ip);
        println!("trying {target}");
        let answer = match tokio::time::timeout(CONNECT_ATTEMPT, adb.connect_device(&target)).await
        {
            Ok(answer) => answer.map_err(|e| e.to_string())?,
            Err(_) => format!("no answer from {target} in {} s", CONNECT_ATTEMPT.as_secs()),
        };
        if is_connected(&answer) {
            return Ok(target);
        }
        answers.push(answer);
    }
    Err(format!(
        "adb on the phone now listens on port {TCPIP_PORT}, but this computer reached none of its addresses ({}). Put this computer on the same network as the phone, or on the same Tailscale tailnet, then run `adb connect <ip>:{TCPIP_PORT}`.",
        answers.join("; ")
    ))
}

/// `phonedriver untether`: makes adb on the phone listen on TCP port 5555 and connects to it.
pub fn untether(device: Option<&str>) -> ExitCode {
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        eprintln!("error: cannot start the async runtime");
        return ExitCode::from(1);
    };
    match rt.block_on(untether_flow(&Adb::from_env(), device)) {
        Ok(target) => {
            println!("connected: android:{target}");
            println!(
                "Warning: adb on this phone now listens on every network it joins, until it reboots. Each new computer still has to be allowed on the phone's screen, but only do this on networks you trust, or with Tailscale. `adb -s {target} usb` or a reboot turns it off."
            );
            println!(
                "A phone still on the cable now shows up twice in `phonedriver devices`; unplug it or pass --device android:{target}."
            );
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc(instance: &str, kind: ServiceKind, addr: &str) -> MdnsService {
        MdnsService {
            instance: instance.into(),
            kind,
            addr: addr.into(),
        }
    }

    fn dev(serial: &str, state: DeviceState) -> AdbDevice {
        AdbDevice {
            serial: serial.into(),
            state,
            model: None,
            transport_id: None,
        }
    }

    #[test]
    fn only_the_pairing_service_with_our_generated_name_is_ours() {
        let services = vec![
            svc("someone-elses", ServiceKind::Pairing, "192.168.1.9:40000"),
            svc(
                "phonedriver-4F9K2M",
                ServiceKind::Connect,
                "192.168.1.20:41233",
            ),
            svc(
                "phonedriver-4F9K2M",
                ServiceKind::Pairing,
                "192.168.1.20:37099",
            ),
        ];
        assert_eq!(
            find_pairing(&services, "phonedriver-4F9K2M").unwrap().addr,
            "192.168.1.20:37099"
        );
        assert!(find_pairing(&services, "phonedriver-ZZZZZZ").is_none());
    }

    #[test]
    fn the_connect_service_must_come_from_the_phone_that_paired() {
        let services = vec![
            svc("adb-OTHER", ServiceKind::Connect, "192.168.1.9:44000"),
            svc(
                "adb-R5CW30ABCDE-xYz12a",
                ServiceKind::Connect,
                "192.168.1.20:41233",
            ),
        ];
        assert_eq!(
            find_connect(&services, "192.168.1.20:37099")
                .unwrap()
                .instance,
            "adb-R5CW30ABCDE-xYz12a"
        );
        assert!(find_connect(&services, "192.168.1.77:37099").is_none());
    }

    #[test]
    fn a_phone_adb_already_connected_is_found_by_address_or_by_mdns_name() {
        let connect = svc(
            "adb-R5CW30ABCDE-xYz12a",
            ServiceKind::Connect,
            "192.168.1.20:41233",
        );
        let by_addr = [dev("192.168.1.20:41233", DeviceState::Device)];
        assert_eq!(
            connected_serial(&by_addr, &connect).unwrap(),
            "192.168.1.20:41233"
        );
        let by_name = [dev(
            "adb-R5CW30ABCDE-xYz12a._adb-tls-connect._tcp",
            DeviceState::Device,
        )];
        assert_eq!(
            connected_serial(&by_name, &connect).unwrap(),
            "adb-R5CW30ABCDE-xYz12a._adb-tls-connect._tcp"
        );
        let offline = [dev("192.168.1.20:41233", DeviceState::Offline)];
        assert!(connected_serial(&offline, &connect).is_none());
        assert!(connected_serial(&[], &connect).is_none());
    }

    #[test]
    fn the_qr_has_a_light_quiet_zone_and_a_dark_finder_corner() {
        let text = render_qr("WIFI:T:ADB;S:phonedriver-4F9K2M;P:7QX3NB0HT5WA;;", false);
        let lines: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
        assert!(lines[0].iter().all(|c| *c == ' '));
        assert_eq!(lines[2][4], '█');
        assert!(lines.iter().all(|l| l.len() == lines[0].len()));
    }

    #[test]
    fn on_a_terminal_the_qr_is_black_on_white_whatever_the_theme() {
        let text = render_qr("WIFI:T:ADB;S:a;P:b;;", true);
        assert!(
            text.lines()
                .all(|l| l.starts_with("\x1b[30;47m") && l.ends_with("\x1b[0m"))
        );
    }

    #[test]
    fn a_phone_on_mobile_data_only_is_stopped_and_mobile_addresses_are_not_tried() {
        let addr = |iface: &str, ip: &str| NetAddr {
            iface: iface.into(),
            ip: ip.into(),
        };
        let mobile = [
            addr("rmnet_data2", "10.202.58.11"),
            addr("rmnet_data3", "10.155.228.231"),
        ];
        assert!(reachable(&mobile).unwrap_err().contains("mobile data only"));
        let mixed = [
            addr("rmnet_data2", "10.202.58.11"),
            addr("wlan0", "192.168.1.20"),
        ];
        assert_eq!(reachable(&mixed).unwrap(), vec![&mixed[1]]);
    }

    #[test]
    fn untether_picks_the_named_device_or_the_only_one() {
        let one = [
            dev("R5CW30ABCDE", DeviceState::Device),
            dev("ghost", DeviceState::Offline),
        ];
        assert_eq!(pick_device(&one, None).unwrap(), "R5CW30ABCDE");
        assert_eq!(pick_device(&one, Some("android:ghost")).unwrap(), "ghost");
        assert_eq!(
            pick_device(&one, Some("R5CW30ABCDE")).unwrap(),
            "R5CW30ABCDE"
        );
        let two = [dev("a", DeviceState::Device), dev("b", DeviceState::Device)];
        let err = pick_device(&two, None).unwrap_err();
        assert!(err.contains("android:a") && err.contains("android:b") && err.contains("--device"));
        assert!(
            pick_device(&[], None)
                .unwrap_err()
                .contains("phonedriver pair")
        );
    }

    #[test]
    fn adb_connect_answers_are_told_apart() {
        assert!(is_connected("connected to 192.168.1.20:5555"));
        assert!(is_connected("already connected to 192.168.1.20:5555"));
        assert!(!is_connected(
            "failed to connect to '10.0.2.18:5555': Connection timed out"
        ));
    }
}
