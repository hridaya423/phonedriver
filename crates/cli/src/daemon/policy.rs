//! The device list (D101): which devices the daemon treats as connected. Every request resolves its device through the
//! daemon's source, so one wrapper around that source hides a device from listing, resolving, booting and connecting.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use async_trait::async_trait;
use phonedriver_core::device::{Device, DisplayMode, lane_of};
use phonedriver_core::error::{DeviceSummary, Error};

use super::{Booted, DeviceSource, Listed, named};

const ID_PREFIXES: [&str; 3] = ["android:", "ios:", "sim:"];

#[derive(Debug, Clone, Default, PartialEq)]
struct Rules {
    allow: Vec<Vec<String>>,
    deny: Vec<String>,
}

impl Rules {
    fn is_empty(&self) -> bool {
        self.allow.is_empty() && self.deny.is_empty()
    }
}

struct Loaded {
    stamp: Option<(SystemTime, u64)>,
    rules: Result<Rules, String>,
}

/// The lists the operator wrote: `PHONEDRIVER_ALLOW_DEVICES` and `PHONEDRIVER_DENY_DEVICES` (read once, when the daemon
/// starts) and the file `devices` in the phonedriver home (read again whenever it changes). A device is hidden when any
/// deny entry names it, or when any allow list that is set does not.
#[derive(Default)]
pub struct Policy {
    env: Rules,
    file: Option<PathBuf>,
    loaded: Mutex<Option<Loaded>>,
}

fn comma_list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(str::to_string)
        .collect()
}

/// `allow <id>` and `deny <id>` lines; `#` starts a comment, blank lines are skipped. A line that is neither is an
/// error that names its number, so a typo never silently leaves a phone open.
fn parse_file(text: &str) -> Result<Rules, String> {
    let mut allow = Vec::new();
    let mut rules = Rules::default();
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        match line.split_once(char::is_whitespace) {
            Some(("allow", id)) if !id.trim().is_empty() => allow.push(id.trim().to_string()),
            Some(("deny", id)) if !id.trim().is_empty() => rules.deny.push(id.trim().to_string()),
            _ => {
                return Err(format!(
                    "line {} is `{line}`, not `allow <id>` or `deny <id>`",
                    n + 1
                ));
            }
        }
    }
    if !allow.is_empty() {
        rules.allow.push(allow);
    }
    Ok(rules)
}

impl Policy {
    pub fn new(allow: &str, deny: &str, file: Option<PathBuf>) -> Policy {
        let allow = comma_list(allow);
        Policy {
            env: Rules {
                allow: if allow.is_empty() {
                    Vec::new()
                } else {
                    vec![allow]
                },
                deny: comma_list(deny),
            },
            file,
            loaded: Mutex::new(None),
        }
    }

    pub fn from_env(home: &Path) -> Policy {
        let var = |name| std::env::var(name).unwrap_or_default();
        Policy::new(
            &var("PHONEDRIVER_ALLOW_DEVICES"),
            &var("PHONEDRIVER_DENY_DEVICES"),
            Some(home.join("devices")),
        )
    }

    fn rules(&self) -> Result<Rules, Error> {
        let mut rules = self.env.clone();
        let Some(path) = &self.file else {
            return Ok(rules);
        };
        let refused = |why: String| Error::Refused {
            detail: format!(
                "the device list file {} cannot be used ({why}); fix or remove it, since a device list that cannot be read hides every device",
                path.display()
            ),
        };
        let stamp = match std::fs::metadata(path) {
            Ok(m) => m.modified().ok().map(|t| (t, m.len())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(rules),
            Err(e) => return Err(refused(e.to_string())),
        };
        let mut loaded = self.loaded.lock().unwrap_or_else(|e| e.into_inner());
        if loaded
            .as_ref()
            .is_none_or(|l| l.stamp != stamp || stamp.is_none())
        {
            let parsed = std::fs::read_to_string(path)
                .map_err(|e| e.to_string())
                .and_then(|t| parse_file(&t));
            *loaded = Some(Loaded {
                stamp,
                rules: parsed,
            });
        }
        let file = loaded.as_ref().map(|l| l.rules.clone());
        match file {
            Some(Ok(f)) => {
                rules.allow.extend(f.allow);
                rules.deny.extend(f.deny);
                Ok(rules)
            }
            Some(Err(why)) => Err(refused(why)),
            None => Ok(rules),
        }
    }
}

fn names_id(entry: &str, id: &str) -> bool {
    id == entry || ID_PREFIXES.iter().any(|p| id == format!("{p}{entry}"))
}

/// Whether a deny entry names this device (or its lane) by id, which needs no read of the device.
pub fn denies_id(deny: &[String], id: &str) -> bool {
    let id = lane_of(id).0;
    deny.iter().any(|e| names_id(e, id))
}

/// An entry that could be a phone's hardware serial: not an iOS or Simulator id, an emulator's name or an address.
fn could_be_serial(entry: &str) -> bool {
    let bare = entry.strip_prefix("android:").unwrap_or(entry);
    !entry.starts_with("ios:")
        && !entry.starts_with("sim:")
        && !bare.starts_with("emulator-")
        && !bare.contains(':')
}

fn names_serial(entry: &str, serial: &str) -> bool {
    could_be_serial(entry) && entry.strip_prefix("android:").unwrap_or(entry) == serial
}

/// What `hidden` found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Hidden {
    /// How many devices `devices` would have listed that the list hides.
    pub count: usize,
    pub ids: Vec<String>,
}

/// The source the daemon uses: `inner` without the devices the policy hides.
pub struct Gated {
    inner: Arc<dyn DeviceSource>,
    policy: Policy,
}

impl Gated {
    pub fn new(inner: Arc<dyn DeviceSource>, policy: Policy) -> Gated {
        Gated { inner, policy }
    }

    /// Reads the phone's hardware serial (one read-only `getprop`, nothing pushed) only when a rule that the id did not
    /// settle could name it. A read that fails counts as a match for a deny entry and as no match for an allow entry; a source with no serial to read (iOS, the Simulator) is no match for either.
    async fn permits(&self, id: &str) -> Result<bool, Error> {
        let rules = self.policy.rules()?;
        if rules.is_empty() {
            return Ok(true);
        }
        let id = lane_of(id).0;
        if rules.deny.iter().any(|e| names_id(e, id)) {
            return Ok(false);
        }
        let android = id.starts_with("android:");
        let mut serial: Option<Result<Option<String>, Error>> = None;
        if android && rules.deny.iter().any(|e| could_be_serial(e)) {
            let s = self.inner.hardware_serial(id).await;
            let denied = match &s {
                Ok(s) => s
                    .iter()
                    .any(|s| rules.deny.iter().any(|e| names_serial(e, s))),
                Err(_) => true,
            };
            if denied {
                return Ok(false);
            }
            serial = Some(s);
        }
        for list in &rules.allow {
            if list.iter().any(|e| names_id(e, id)) {
                continue;
            }
            if android && list.iter().any(|e| could_be_serial(e)) {
                if serial.is_none() {
                    serial = Some(self.inner.hardware_serial(id).await);
                }
                if let Some(Ok(Some(s))) = &serial
                    && list.iter().any(|e| names_serial(e, s))
                {
                    continue;
                }
            }
            return Ok(false);
        }
        Ok(true)
    }

    /// The devices that are off and may be started. An AVD is listed by its name (`avd:<name>`) and gets its adb id
    /// only once it runs, so before a boot only a deny entry that names `avd:<name>` hides it; the started device is
    /// judged by its id again.
    async fn startable(&self, off: Vec<DeviceSummary>) -> Result<Vec<DeviceSummary>, Error> {
        let rules = self.policy.rules()?;
        if rules.is_empty() {
            return Ok(off);
        }
        let mut out = Vec::new();
        for d in off {
            let shown = if d.id.starts_with("avd:") {
                !rules.deny.contains(&d.id)
            } else {
                self.permits(&d.id).await?
            };
            if shown {
                out.push(d);
            }
        }
        Ok(out)
    }

    async fn visible(&self, list: Vec<Listed>) -> Result<Vec<Listed>, Error> {
        if self.policy.rules()?.is_empty() {
            return Ok(list);
        }
        let mut out = Vec::new();
        for l in list {
            if self.permits(&l.id).await? {
                out.push(l);
            }
        }
        Ok(out)
    }

    /// The ids the list hides among the devices that are connected, and `extra`: ids of devices that are not ready, which
    /// cannot be read for a serial and so are judged by id alone (hidden when an allow list does not name them).
    pub async fn hidden(&self, extra: &[String]) -> Result<Hidden, Error> {
        let rules = self.policy.rules()?;
        if rules.is_empty() {
            return Ok(Hidden::default());
        }
        let shown = self.inner.list().await.unwrap_or_default();
        let mut every = shown.clone();
        for l in self
            .inner
            .list_for(Some("android:"))
            .await
            .unwrap_or_default()
        {
            if !every.iter().any(|e| e.id == l.id) {
                every.push(l);
            }
        }
        let mut hidden = Hidden::default();
        for l in &every {
            if !self.permits(&l.id).await? {
                hidden.ids.push(l.id.clone());
                if shown.iter().any(|s| s.id == l.id) {
                    hidden.count += 1;
                }
            }
        }
        for id in extra {
            let by_id = |e: &String| names_id(e, id);
            let refused =
                rules.deny.iter().any(by_id) || rules.allow.iter().any(|l| !l.iter().any(by_id));
            if refused && !hidden.ids.contains(id) {
                hidden.ids.push(id.clone());
                hidden.count += 1;
            }
        }
        hidden.ids.sort();
        Ok(hidden)
    }
}

fn not_connected(id: &str) -> Error {
    Error::DeviceGone { id: id.to_string() }
}

#[async_trait]
impl DeviceSource for Gated {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        let rules = self.policy.rules()?;
        if rules.is_empty() {
            return self.inner.list().await;
        }
        self.visible(self.inner.list_except(&rules.deny).await?).await
    }

    async fn list_for(&self, requested: Option<&str>) -> Result<Vec<Listed>, Error> {
        if requested.is_none() {
            return self.list().await;
        }
        match self.inner.list_for(requested).await {
            Ok(list) => self.visible(list).await,
            Err(Error::NoDevice {
                requested,
                connected,
                unasked,
                off,
            }) => {
                let mut shown = Vec::new();
                for d in connected {
                    if self.permits(&d.id).await? {
                        shown.push(d);
                    }
                }
                Err(Error::NoDevice {
                    requested,
                    connected: shown,
                    unasked,
                    off: self.startable(off).await?,
                })
            }
            Err(e) => Err(e),
        }
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        if !self.permits(id).await? {
            return Err(not_connected(id));
        }
        self.inner.connect(id).await
    }

    async fn connect_on(&self, id: &str, display: DisplayMode) -> Result<Arc<dyn Device>, Error> {
        if !self.permits(id).await? {
            return Err(not_connected(id));
        }
        self.inner.connect_on(id, display).await
    }

    async fn off(&self) -> Vec<DeviceSummary> {
        self.startable(self.inner.off().await)
            .await
            .unwrap_or_default()
    }

    /// The name is resolved against the devices that may be started before anything is started, so a hidden
    /// Simulator is never booted to be refused.
    async fn boot(&self, name: &str) -> Result<Option<Booted>, Error> {
        let mut target = name.to_string();
        if !self.policy.rules()?.is_empty() {
            let every = self.inner.off().await;
            let shown = self.startable(every.clone()).await?;
            match shown.iter().find(|d| named(name, &d.id, &d.model)) {
                Some(d) => target = d.id.clone(),
                None if every.iter().any(|d| named(name, &d.id, &d.model)) => return Ok(None),
                None => {}
            }
        }
        match self.inner.boot(&target).await? {
            Some(booted) if !self.permits(&booted.device.id).await? => Ok(None),
            booted => Ok(booted),
        }
    }

    fn blocker(&self, id: &str) -> Option<&'static str> {
        self.inner.blocker(id)
    }

    async fn hardware_serial(&self, id: &str) -> Result<Option<String>, Error> {
        self.inner.hardware_serial(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_comma_list_drops_blanks_and_spaces() {
        assert_eq!(comma_list(" a, ,b,"), vec!["a", "b"]);
        assert!(comma_list("").is_empty());
    }

    #[test]
    fn the_file_has_allow_and_deny_lines_and_comments() {
        let rules = parse_file(
            "# staff phones\n\ndeny android:R5CT123  # Maria's\nallow android:emulator-5554\nallow ios:abc\n",
        )
        .unwrap();
        assert_eq!(rules.deny, vec!["android:R5CT123"]);
        assert_eq!(rules.allow, vec![vec!["android:emulator-5554", "ios:abc"]]);
    }

    #[test]
    fn a_line_that_is_not_allow_or_deny_names_its_number() {
        let e = parse_file("deny a\ndeney b\n").unwrap_err();
        assert!(e.contains("line 2") && e.contains("deney b"), "{e}");
        assert!(parse_file("allow\n").is_err());
    }

    #[test]
    fn an_entry_names_an_id_with_or_without_its_prefix_and_a_serial_only_off_ios() {
        assert!(names_id("android:emulator-5554", "android:emulator-5554"));
        assert!(names_id("emulator-5554", "android:emulator-5554"));
        assert!(!names_id("5555", "android:192.168.1.20:5555"));
        assert!(names_serial("android:R5CT123", "R5CT123"));
        assert!(names_serial("R5CT123", "R5CT123"));
        assert!(!names_serial("ios:R5CT123", "R5CT123"));
        assert!(!names_serial("sim:R5CT123", "R5CT123"));
        assert!(!could_be_serial("android:emulator-5558"));
        assert!(!could_be_serial("android:192.168.1.20:5555"));
    }
}
