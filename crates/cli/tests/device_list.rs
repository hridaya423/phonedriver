use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use phonedriver::daemon::policy::Policy;
use phonedriver::daemon::rpc::RpcError;
use phonedriver::daemon::{Booted, Config, Daemon, DeviceSource, Listed, named};
use phonedriver_core::device::Device;
use phonedriver_core::error::{DeviceSummary, Error};
use phonedriver_core::geometry::Scale;
use phonedriver_core::raw::RawTree;
use phonedriver_core::testing::FakeDevice;
use serde_json::{Value, json};

fn screen() -> RawTree {
    let json = r#"{"windows":[{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}],"nodes":[
        {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.widget.Button","text":"One","b":[0,100,360,160],"f":"vVe","a":["tap"]}]}"#;
    RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap()
}

struct Bench {
    devices: Vec<(Listed, Arc<FakeDevice>)>,
    serials: HashMap<String, String>,
    connects: Mutex<Vec<String>>,
    reads: Mutex<Vec<String>>,
    off: Mutex<Vec<DeviceSummary>>,
    boots: Mutex<Vec<String>>,
}

impl Bench {
    fn new(ids: &[(&str, Option<&str>)]) -> Arc<Bench> {
        let mut devices = Vec::new();
        let mut serials = HashMap::new();
        for (id, serial) in ids {
            let fake = Arc::new(FakeDevice::new(vec![screen()]).with_id(id));
            devices.push((
                Listed {
                    id: id.to_string(),
                    model: "Pixel".into(),
                    os: None,
                    tier: "fake".into(),
                    capabilities: fake.info().capabilities,
                },
                fake,
            ));
            if let Some(s) = serial {
                serials.insert(id.to_string(), s.to_string());
            }
        }
        Arc::new(Bench {
            devices,
            serials,
            connects: Mutex::default(),
            reads: Mutex::default(),
            off: Mutex::default(),
            boots: Mutex::default(),
        })
    }

    fn fake(&self, id: &str) -> Arc<FakeDevice> {
        self.devices
            .iter()
            .find(|(l, _)| l.id == id)
            .unwrap()
            .1
            .clone()
    }

    fn connects(&self) -> Vec<String> {
        self.connects.lock().unwrap().clone()
    }

    fn reads(&self) -> Vec<String> {
        self.reads.lock().unwrap().clone()
    }
}

#[async_trait]
impl DeviceSource for Bench {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(self.devices.iter().map(|(l, _)| l.clone()).collect())
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        self.connects.lock().unwrap().push(id.to_string());
        self.devices
            .iter()
            .find(|(l, _)| l.id == id)
            .map(|(_, d)| d.clone() as Arc<dyn Device>)
            .ok_or(Error::DeviceGone { id: id.into() })
    }

    async fn off(&self) -> Vec<DeviceSummary> {
        self.off.lock().unwrap().clone()
    }

    async fn boot(&self, name: &str) -> Result<Option<Booted>, Error> {
        self.boots.lock().unwrap().push(name.to_string());
        let off = self.off.lock().unwrap();
        Ok(off
            .iter()
            .find(|d| named(name, &d.id, &d.model))
            .map(|d| Booted {
                device: d.clone(),
                already: false,
            }))
    }

    async fn hardware_serial(&self, id: &str) -> Result<Option<String>, Error> {
        self.reads.lock().unwrap().push(id.to_string());
        Ok(self.serials.get(id).cloned())
    }
}

fn daemon(source: Arc<Bench>, policy: Policy) -> (Arc<Daemon>, tempfile::TempDir) {
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::start(
        source,
        Config {
            home: home.path().to_path_buf(),
        },
        phonedriver::daemon::trace::TraceOptions::default(),
        Vec::new(),
        policy,
        None,
    );
    (d, home)
}

async fn ok(d: &Daemon, method: &str, params: Value) -> Value {
    d.call("default", method, params)
        .await
        .unwrap_or_else(|e| panic!("{method} failed: {e:?}"))
}

async fn err(d: &Daemon, method: &str, params: Value) -> RpcError {
    d.call("default", method, params)
        .await
        .expect_err("expected an error")
}

async fn listed(d: &Daemon) -> Vec<String> {
    ok(d, "devices", json!({}))
        .await
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect()
}

const TWO: [(&str, Option<&str>); 2] = [
    ("android:emulator-5554", None),
    ("android:emulator-5558", None),
];

#[tokio::test]
async fn without_a_list_every_device_shows_and_no_serial_is_read() {
    let bench = Bench::new(&TWO);
    let (d, _home) = daemon(bench.clone(), Policy::default());
    assert_eq!(listed(&d).await.len(), 2);
    assert_eq!(ok(&d, "hidden", json!({})).await["count"], 0);
    assert!(bench.reads().is_empty());
}

#[tokio::test]
async fn an_allow_list_hides_every_device_it_does_not_name() {
    let bench = Bench::new(&TWO);
    let (d, _home) = daemon(
        bench.clone(),
        Policy::new("android:emulator-5554", "", None),
    );
    assert_eq!(listed(&d).await, vec!["android:emulator-5554"]);
    let hidden = ok(&d, "hidden", json!({})).await;
    assert_eq!(hidden["count"], 1);
    assert_eq!(hidden["ids"], json!(["android:emulator-5558"]));
}

#[tokio::test]
async fn a_deny_list_hides_the_device_it_names_and_the_daemon_never_connects_to_it() {
    let bench = Bench::new(&TWO);
    let (d, _home) = daemon(
        bench.clone(),
        Policy::new("", "android:emulator-5558", None),
    );
    assert_eq!(listed(&d).await, vec!["android:emulator-5554"]);
    let e = err(&d, "snapshot", json!({"device": "android:emulator-5558"})).await;
    assert_eq!(e.code, -32001, "{}", e.message);
    assert!(e.message.contains("emulator-5558"), "{}", e.message);
    let e = err(&d, "snapshot", json!({"device": "emulator-5558"})).await;
    assert_eq!(e.code, -32001);
    ok(&d, "snapshot", json!({})).await;
    assert_eq!(bench.connects(), vec!["android:emulator-5554"]);
    assert!(bench.reads().is_empty(), "an id rule needs no serial");
}

#[tokio::test]
async fn deny_wins_over_allow() {
    let bench = Bench::new(&TWO);
    let policy = Policy::new(
        "android:emulator-5554,android:emulator-5558",
        "android:emulator-5558",
        None,
    );
    let (d, _home) = daemon(bench, policy);
    assert_eq!(listed(&d).await, vec!["android:emulator-5554"]);
}

#[tokio::test]
async fn a_hardware_serial_rule_hides_the_same_phone_under_both_of_its_ids() {
    let bench = Bench::new(&[
        ("android:ZZ1", Some("ABC999")),
        ("android:192.168.1.20:5555", Some("ABC999")),
        ("android:OTHER", Some("XYZ111")),
    ]);
    let (d, _home) = daemon(bench.clone(), Policy::new("", "android:ABC999", None));
    assert_eq!(listed(&d).await, vec!["android:OTHER"]);
    for id in ["android:ZZ1", "android:192.168.1.20:5555", "ZZ1"] {
        let e = err(&d, "snapshot", json!({"device": id})).await;
        assert_eq!(e.code, -32001, "{id}: {}", e.message);
    }
    assert_eq!(bench.connects(), Vec::<String>::new());

    let (d, _home) = daemon(bench.clone(), Policy::new("ABC999", "", None));
    let mut shown = listed(&d).await;
    shown.sort();
    assert_eq!(shown, vec!["android:192.168.1.20:5555", "android:ZZ1"]);
}

#[tokio::test]
async fn a_device_the_ids_decide_is_never_asked_for_its_serial() {
    let bench = Bench::new(&[
        ("android:ZZ1", Some("ABC999")),
        ("android:emulator-5554", None),
    ]);
    let (d, _home) = daemon(bench.clone(), Policy::new("android:ZZ1", "", None));
    listed(&d).await;
    assert!(!bench.reads().contains(&"android:ZZ1".to_string()));
}

#[tokio::test]
async fn force_does_not_get_a_command_through_to_a_hidden_device() {
    let bench = Bench::new(&TWO);
    let (d, _home) = daemon(
        bench.clone(),
        Policy::new("", "android:emulator-5558", None),
    );
    let e = err(
        &d,
        "shell",
        json!({"device": "android:emulator-5558", "command": "ls /sdcard", "force": true}),
    )
    .await;
    assert_eq!(e.code, -32001, "{}", e.message);
    assert!(bench.fake("android:emulator-5558").calls().is_empty());
    assert!(bench.connects().is_empty());
}

#[tokio::test]
async fn a_name_that_only_a_hidden_device_has_is_not_found_by_the_default_either() {
    let bench = Bench::new(&TWO);
    let (d, _home) = daemon(
        bench.clone(),
        Policy::new("", "android:emulator-5558", None),
    );
    ok(&d, "snapshot", json!({})).await;
    let (d, _home) = daemon(
        bench.clone(),
        Policy::new("", "android:emulator-5554,android:emulator-5558", None),
    );
    let e = err(&d, "snapshot", json!({})).await;
    assert_eq!(e.code, -32001, "{}", e.message);
}

#[tokio::test]
async fn a_hidden_emulator_is_not_booted_or_reported_as_already_on() {
    let bench = Bench::new(&TWO);
    let (d, _home) = daemon(
        bench.clone(),
        Policy::new("", "android:emulator-5558", None),
    );
    bench.off.lock().unwrap().push(DeviceSummary {
        id: "android:emulator-5558".into(),
        model: "Pixel Tablet".into(),
        os: "Android 15".into(),
    });
    let e = err(&d, "boot", json!({"name": "Pixel Tablet"})).await;
    assert!(e.message.contains("no device named"), "{}", e.message);
}

fn write(home: &Path, text: &str) -> Policy {
    let file = home.join("devices");
    std::fs::write(&file, text).unwrap();
    Policy::new("", "", Some(file))
}

#[tokio::test]
async fn the_file_hides_devices_and_is_read_again_when_it_changes() {
    let dir = tempfile::tempdir().unwrap();
    let bench = Bench::new(&TWO);
    let (d, _home) = daemon(
        bench,
        write(
            dir.path(),
            "# test phones only\nallow android:emulator-5554 # the real one\n",
        ),
    );
    assert_eq!(listed(&d).await, vec!["android:emulator-5554"]);
    std::fs::write(
        dir.path().join("devices"),
        "allow android:emulator-5554\nallow android:emulator-5558\ndeny android:emulator-5554\n",
    )
    .unwrap();
    assert_eq!(listed(&d).await, vec!["android:emulator-5558"]);
}

#[tokio::test]
async fn the_environment_and_the_file_both_bind_and_deny_from_either_wins() {
    let dir = tempfile::tempdir().unwrap();
    let ids: Vec<(String, Option<&str>)> = (1..=3)
        .map(|n| (format!("android:emulator-{n}"), None))
        .collect();
    let refs: Vec<(&str, Option<&str>)> = ids.iter().map(|(i, s)| (i.as_str(), *s)).collect();
    let bench = Bench::new(&refs);
    let file = dir.path().join("devices");
    std::fs::write(
        &file,
        "allow android:emulator-1\nallow android:emulator-2\n",
    )
    .unwrap();
    let policy = Policy::new(
        "android:emulator-2,android:emulator-3",
        "",
        Some(file.clone()),
    );
    let (d, _home) = daemon(bench.clone(), policy);
    assert_eq!(listed(&d).await, vec!["android:emulator-2"]);
    let policy = Policy::new("", "android:emulator-2", Some(file));
    let (d, _home) = daemon(bench, policy);
    assert_eq!(listed(&d).await, vec!["android:emulator-1"]);
}

#[tokio::test]
async fn a_file_that_cannot_be_read_hides_everything_and_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let bench = Bench::new(&TWO);
    let (d, _home) = daemon(
        bench.clone(),
        write(dir.path(), "deney android:emulator-5558\n"),
    );
    for method in ["devices", "snapshot", "hidden"] {
        let e = err(&d, method, json!({})).await;
        assert!(
            e.message.contains("line 1") && e.message.contains("hides every device"),
            "{method}: {}",
            e.message
        );
    }
    assert!(bench.connects().is_empty());
}

#[tokio::test]
async fn devices_that_are_not_ready_are_hidden_by_id_when_an_allow_list_does_not_name_them() {
    let bench = Bench::new(&TWO);
    let (d, _home) = daemon(bench, Policy::new("android:emulator-5554", "", None));
    let hidden = ok(
        &d,
        "hidden",
        json!({"ids": ["android:OFFLINE1", "android:emulator-5554"]}),
    )
    .await;
    assert_eq!(
        hidden["ids"],
        json!(["android:OFFLINE1", "android:emulator-5558"])
    );
    assert_eq!(hidden["count"], 2);
}

#[tokio::test]
async fn a_live_lease_is_marked_so_doctor_can_leave_the_device_alone() {
    let bench = Bench::new(&[("android:emulator-5554", None)]);
    let (d, _home) = daemon(bench, Policy::default());
    ok(&d, "snapshot", json!({})).await;
    let entries = ok(&d, "devices", json!({})).await;
    assert_eq!(entries[0]["leased_by"], "default");
    assert_eq!(entries[0]["lease_live"], true);
}

#[tokio::test]
async fn a_denied_simulator_is_neither_booted_nor_listed_as_off() {
    let bench = Bench::new(&TWO);
    bench.off.lock().unwrap().extend([
        DeviceSummary {
            id: "sim:DENIED-UDID".into(),
            model: "Staff iPhone".into(),
            os: "iOS 26".into(),
        },
        DeviceSummary {
            id: "sim:OPEN-UDID".into(),
            model: "Test iPhone".into(),
            os: "iOS 26".into(),
        },
    ]);
    let (d, _home) = daemon(bench.clone(), Policy::new("", "sim:DENIED-UDID", None));
    for name in ["sim:DENIED-UDID", "DENIED-UDID", "Staff iPhone"] {
        let e = err(&d, "boot", json!({"name": name})).await;
        assert!(
            e.message.contains("no device named"),
            "{name}: {}",
            e.message
        );
        assert!(
            !e.to_json().to_string().contains("Staff iPhone (iOS"),
            "{name}: {e:?}"
        );
    }
    let off = ok(&d, "boot", json!({})).await;
    assert_eq!(off["off"].as_array().unwrap().len(), 1, "{off}");
    assert!(!off.to_string().contains("DENIED"), "{off}");
    assert!(
        !bench
            .boots
            .lock()
            .unwrap()
            .iter()
            .any(|b| b.contains("DENIED") || b == "Staff iPhone"),
        "{:?}",
        bench.boots.lock().unwrap()
    );
}

struct Broken;

#[async_trait]
impl DeviceSource for Broken {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Err(Error::Transport {
            detail: "xcrun is not installed".into(),
        })
    }
    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        Err(Error::DeviceGone { id: id.into() })
    }
}

#[tokio::test]
async fn a_device_that_is_not_found_while_a_source_fails_names_no_hidden_device() {
    let bench = Bench::new(&[
        ("android:STAFF-PHONE-1", None),
        ("android:emulator-5554", None),
    ]);
    let sources = phonedriver::daemon::sources::Sources(vec![
        ("android:", bench.clone() as Arc<dyn DeviceSource>),
        ("sim:", Arc::new(Broken)),
    ]);
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::start(
        Arc::new(sources),
        Config {
            home: home.path().to_path_buf(),
        },
        phonedriver::daemon::trace::TraceOptions::default(),
        Vec::new(),
        Policy::new("", "android:STAFF-PHONE-1", None),
        None,
    );
    let e = err(&d, "snapshot", json!({"device": "nosuchdevice"})).await;
    assert!(e.message.contains("emulator-5554"), "{}", e.message);
    assert!(!e.to_json().to_string().contains("STAFF"), "{e:?}");
}
