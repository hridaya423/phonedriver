use std::sync::Arc;

use async_trait::async_trait;
use phone_use::daemon::sources::Sources;
use phone_use::daemon::{DeviceSource, Listed};
use phone_use_core::device::{Capabilities, Device};
use phone_use_core::error::Error;
use phone_use_core::testing::FakeDevice;

struct Fixed(Result<Vec<&'static str>, &'static str>);

#[async_trait]
impl DeviceSource for Fixed {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        match &self.0 {
            Ok(ids) => Ok(ids
                .iter()
                .map(|id| Listed {
                    id: id.to_string(),
                    model: "m".into(),
                    os: None,
                    tier: "t".into(),
                    capabilities: Capabilities::empty(),
                })
                .collect()),
            Err(detail) => Err(Error::Transport {
                detail: detail.to_string(),
            }),
        }
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        match &self.0 {
            Ok(ids) if ids.contains(&id) => Ok(Arc::new(FakeDevice::new(vec![]))),
            _ => Err(Error::NoDevice {
                requested: None,
                connected: vec![],
            }),
        }
    }
}

fn sources(adb: Fixed, sim: Fixed) -> Sources {
    Sources(vec![("android:", Arc::new(adb)), ("sim:", Arc::new(sim))])
}

fn ids(list: Vec<Listed>) -> Vec<String> {
    list.into_iter().map(|l| l.id).collect()
}

#[tokio::test]
async fn lists_the_union_of_every_source() {
    let s = sources(Fixed(Ok(vec!["android:a"])), Fixed(Ok(vec!["sim:b"])));
    assert_eq!(ids(s.list().await.unwrap()), ["android:a", "sim:b"]);
}

#[tokio::test]
async fn a_failing_source_is_left_out_when_another_answers() {
    let s = sources(Fixed(Err("no adb server")), Fixed(Ok(vec!["sim:b"])));
    assert_eq!(ids(s.list().await.unwrap()), ["sim:b"]);
    let s = sources(Fixed(Err("no adb server")), Fixed(Ok(vec![])));
    assert_eq!(ids(s.list().await.unwrap()), Vec::<String>::new());
}

#[tokio::test]
async fn when_every_source_fails_the_first_error_is_the_answer() {
    let s = sources(Fixed(Err("no adb server")), Fixed(Err("no xcrun")));
    assert_eq!(
        s.list().await.unwrap_err(),
        Error::Transport {
            detail: "no adb server".into()
        }
    );
}

#[tokio::test]
async fn connect_goes_to_the_source_that_owns_the_prefix() {
    let s = sources(Fixed(Ok(vec!["android:a"])), Fixed(Ok(vec!["sim:b"])));
    assert!(s.connect("sim:b").await.is_ok());
    assert!(s.connect("android:a").await.is_ok());
    assert!(matches!(
        s.connect("ios:c").await,
        Err(Error::DeviceGone { .. })
    ));
}

struct Counting(Fixed, std::sync::atomic::AtomicUsize);

#[async_trait]
impl DeviceSource for Counting {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.0.list().await
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        self.0.connect(id).await
    }
}

fn counted(ids: Vec<&'static str>) -> Arc<Counting> {
    Arc::new(Counting(Fixed(Ok(ids)), Default::default()))
}

fn calls(c: &Counting) -> usize {
    c.1.load(std::sync::atomic::Ordering::SeqCst)
}

#[tokio::test]
async fn a_request_for_an_android_device_never_calls_the_ios_sources() {
    let (adb, sim, phys) = (
        counted(vec!["android:emulator-5554"]),
        counted(vec!["sim:s"]),
        counted(vec!["ios:p"]),
    );
    let s = Sources(vec![
        ("android:", adb.clone()),
        ("sim:", sim.clone()),
        ("ios:", phys.clone()),
    ]);
    let got = s.list_for(Some("android:emulator-5554")).await.unwrap();
    assert_eq!(ids(got), ["android:emulator-5554"]);
    let got = s.list_for(Some("emulator-5554")).await.unwrap();
    assert_eq!(ids(got), ["android:emulator-5554"]);
    assert_eq!((calls(&adb), calls(&sim), calls(&phys)), (2, 0, 0));
}

#[tokio::test]
async fn a_prefixed_request_asks_only_its_owner_and_a_bare_one_falls_through() {
    let (adb, sim, phys) = (
        counted(vec![]),
        counted(vec!["sim:s"]),
        counted(vec!["ios:p"]),
    );
    let s = Sources(vec![
        ("android:", adb.clone()),
        ("sim:", sim.clone()),
        ("ios:", phys.clone()),
    ]);
    assert_eq!(ids(s.list_for(Some("ios:p")).await.unwrap()), ["ios:p"]);
    assert_eq!((calls(&adb), calls(&sim), calls(&phys)), (0, 0, 1));
    assert_eq!(ids(s.list_for(Some("s")).await.unwrap()), ["sim:s"]);
    assert_eq!((calls(&adb), calls(&sim), calls(&phys)), (1, 1, 1));
    assert_eq!(ids(s.list_for(None).await.unwrap()), ["sim:s", "ios:p"]);
}
