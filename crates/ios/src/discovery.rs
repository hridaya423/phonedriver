//! Choosing the iOS tier for a physical device (architecture 7).

use phone_use_core::error::{Error, Op};

pub const HID_SERVICE: &str = "com.apple.coredevice.hid.universalhidservice";
pub const DISPLAY_SERVICE: &str = "com.apple.coredevice.displayservice";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    CoreDevice,
    Runner,
}

impl Tier {
    pub fn name(self) -> &'static str {
        match self {
            Tier::CoreDevice => "coredevice",
            Tier::Runner => "runner",
        }
    }
}

/// `os_version` is the lockdown `ProductVersion`; `services` are the RSD service names the device lists.
pub fn choose_tier(os_version: &str, services: &[String]) -> Result<Tier, Error> {
    let major: u32 = os_version
        .split('.')
        .next()
        .and_then(|m| m.parse().ok())
        .ok_or_else(|| Error::Transport {
            detail: format!("unreadable iOS version \"{os_version}\""),
        })?;
    let has = |name: &str| services.iter().any(|s| s == name);
    if major >= 27 && has(HID_SERVICE) && has(DISPLAY_SERVICE) {
        Ok(Tier::CoreDevice)
    } else if major >= 17 {
        Ok(Tier::Runner)
    } else {
        Err(Error::Unsupported {
            op: Op::Screenshot,
            reason: "iOS before 17 has no developer services phone-use can reach",
        })
    }
}

#[cfg(test)]
mod tests {
    use phone_use_core::error::Error;

    use super::*;

    fn services(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn coredevice_services() -> Vec<String> {
        services(&[
            HID_SERVICE,
            DISPLAY_SERVICE,
            "com.apple.coredevice.appservice",
        ])
    }

    #[test]
    fn ios_27_with_the_hid_and_display_services_uses_coredevice() {
        assert_eq!(
            choose_tier("27.0", &coredevice_services()),
            Ok(Tier::CoreDevice)
        );
        assert_eq!(
            choose_tier("27.1.2", &coredevice_services()),
            Ok(Tier::CoreDevice)
        );
    }

    #[test]
    fn ios_27_missing_either_service_falls_back_to_the_runner() {
        assert_eq!(
            choose_tier("27.0", &services(&[DISPLAY_SERVICE])),
            Ok(Tier::Runner)
        );
        assert_eq!(
            choose_tier("27.0", &services(&[HID_SERVICE])),
            Ok(Tier::Runner)
        );
        assert_eq!(choose_tier("27.0", &[]), Ok(Tier::Runner));
    }

    #[test]
    fn ios_17_to_26_uses_the_runner_even_if_the_services_are_listed() {
        assert_eq!(
            choose_tier("26.5", &coredevice_services()),
            Ok(Tier::Runner)
        );
        assert_eq!(choose_tier("17.0", &[]), Ok(Tier::Runner));
    }

    #[test]
    fn ios_before_17_is_unsupported() {
        let e = choose_tier("16.7.10", &[]).unwrap_err();
        assert!(matches!(e, Error::Unsupported { .. }), "{e:?}");
    }

    #[test]
    fn a_version_without_a_number_is_a_transport_error() {
        let e = choose_tier("beta", &[]).unwrap_err();
        assert_eq!(
            e,
            Error::Transport {
                detail: "unreadable iOS version \"beta\"".into()
            }
        );
    }

    #[test]
    fn tier_names_are_what_devices_prints() {
        assert_eq!(Tier::CoreDevice.name(), "coredevice");
        assert_eq!(Tier::Runner.name(), "runner");
    }
}
