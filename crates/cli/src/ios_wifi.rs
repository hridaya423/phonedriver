//! `phonedriver ios wifi` and `phonedriver ios register`: one-time steps for a physical iOS device.

use std::path::Path;

use phonedriver_ios::lan::{self, Lan};
use phonedriver_ios::transport::{self, Attached, Link};

/// The devices on a cable and the remembered ones on the network, for a command that runs once.
pub(crate) async fn attached(home: &Path) -> Result<Vec<Attached>, String> {
    let udids = lan::remembered(home);
    let browse = match udids.is_empty() {
        true => None,
        false => Some(Lan::watch(udids).map_err(|e| e.to_string())?),
    };
    transport::attached(browse.as_ref())
        .await
        .map_err(|e| format!("cannot list devices: {e}"))
}

fn name(device: &Attached) -> String {
    format!("ios:{}", device.udid)
}

fn pick<'a>(attached: &'a [Attached], wanted: Option<&str>) -> Result<&'a Attached, String> {
    if let Some(wanted) = wanted {
        let udid = wanted.strip_prefix("ios:").unwrap_or(wanted);
        return attached.iter().find(|d| d.udid == udid).ok_or_else(|| {
            format!("no iOS device {wanted} is attached; `phonedriver devices` lists them")
        });
    }
    match attached {
        [one] => Ok(one),
        [] => Err("no iOS device is attached. Plug it in with the cable, unlock it and tap Trust, then run this again".into()),
        many => Err(format!(
            "more than one iOS device is attached ({}); pass --device with one of them",
            many.iter().map(name).collect::<Vec<_>>().join(", ")
        )),
    }
}

fn done(device: &Attached, on: bool) -> String {
    let id = name(device);
    match (on, matches!(device.link, Link::Usb { .. })) {
        (true, true) => format!(
            "{id} now answers this computer over Wi-Fi. Unplug the cable; the device stays in `phonedriver devices` while both are on the same network.\nThe device answers only computers it already trusts. `phonedriver ios wifi off` turns this off."
        ),
        (true, false) => format!("{id} already answers this computer over Wi-Fi."),
        (false, true) => {
            format!("{id} no longer answers over Wi-Fi. It needs the cable from now on.")
        }
        (false, false) => format!(
            "{id} no longer answers over Wi-Fi. It leaves `phonedriver devices` until you plug it in."
        ),
    }
}

/// The setting is read back only on the cable: turned off over Wi-Fi, the device drops the connection that would
/// read it (observed 2026-11-12, "device socket io failed", with the setting off).
async fn flow(home: &Path, on: bool, wanted: Option<&str>) -> Result<String, String> {
    let attached = attached(home).await?;
    let device = pick(&attached, wanted)?;
    transport::allow_network(device, on).await.map_err(|e| {
        let e = transport::classify(&name(device), &e);
        format!("{e}. {}", e.hint())
    })?;
    if matches!(device.link, Link::Usb { .. }) {
        let is = transport::network_allowed(device)
            .await
            .map_err(|e| format!("cannot read the setting back: {e}"))?;
        if is != on {
            return Err(format!(
                "{} did not keep the setting. Open Finder, select the device and set \"Show this iPhone when on Wi-Fi\" there.",
                name(device)
            ));
        }
    }
    match on {
        true => lan::remember(home, &device.udid),
        false => lan::forget(home, &device.udid),
    }
    .map_err(|e| e.to_string())?;
    Ok(done(device, on))
}

async fn register_flow(home: &Path, wanted: Option<&str>) -> Result<String, String> {
    let team = phonedriver_ios::team::signing_team().map_err(|e| format!("{e}. {}", e.hint()))?;
    let attached = attached(home).await?;
    let device = pick(&attached, wanted)?;
    println!(
        "registering {} with team {team}: Xcode adds the device to that Apple developer account and builds the runner for it",
        name(device)
    );
    phonedriver_ios::runner::register(home, &team, &device.udid)
        .await
        .map_err(|e| format!("{e}. {}", e.hint()))?;
    Ok(format!(
        "{} is registered. Retry the command that asked for this.",
        name(device)
    ))
}

/// `phonedriver ios register`.
pub fn register(home: &Path, wanted: Option<&str>) -> bool {
    report(|rt| rt.block_on(register_flow(home, wanted)))
}

pub fn run(home: &Path, on: bool, wanted: Option<&str>) -> bool {
    report(|rt| rt.block_on(flow(home, on, wanted)))
}

fn report(run: impl FnOnce(&tokio::runtime::Runtime) -> Result<String, String>) -> bool {
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        eprintln!("error: cannot start the async runtime");
        return false;
    };
    match run(&rt) {
        Ok(message) => {
            println!("{message}");
            true
        }
        Err(message) => {
            eprintln!("error: {message}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const USB: Link = Link::Usb { mux_id: 1 };

    fn wifi() -> Link {
        Link::Network {
            addr: "192.168.1.20".parse().unwrap(),
            scope: None,
        }
    }

    fn device(udid: &str, link: Link) -> Attached {
        Attached {
            udid: udid.to_string(),
            link,
        }
    }

    #[test]
    fn the_only_attached_device_is_picked_and_a_named_one_is_found_with_or_without_the_prefix() {
        let both = [device("A", USB), device("B", wifi())];
        assert_eq!(pick(&both[..1], None).unwrap().udid, "A");
        assert_eq!(pick(&both, Some("ios:B")).unwrap().udid, "B");
        assert_eq!(pick(&both, Some("B")).unwrap().udid, "B");
    }

    #[test]
    fn no_device_asks_for_the_cable_and_two_devices_ask_for_a_choice() {
        assert!(pick(&[], None).unwrap_err().contains("Plug it in"));
        let both = [device("A", USB), device("B", USB)];
        assert_eq!(
            pick(&both, None).unwrap_err(),
            "more than one iOS device is attached (ios:A, ios:B); pass --device with one of them"
        );
        assert!(pick(&both, Some("ios:C")).unwrap_err().contains("ios:C"));
    }

    #[test]
    fn turning_it_on_over_the_cable_says_to_unplug_and_off_over_wifi_says_the_device_leaves() {
        assert!(done(&device("A", USB), true).contains("Unplug the cable"));
        assert!(done(&device("A", wifi()), false).contains("until you plug it in"));
    }
}
