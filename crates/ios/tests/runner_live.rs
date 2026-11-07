//! Runner lifecycle on booted simulators. Run on the Mac that hosts them:
//! `PHONEDRIVER_TEST_DEVICE=sim:<udid> cargo test -p phonedriver-ios --test runner_live -- --ignored --test-threads=1`.
//! The two-simulator test also needs `PHONEDRIVER_TEST_DEVICE2=sim:<udid>`.

#![cfg(unix)]

use std::io::BufRead;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use phonedriver_ios::runner::{self, Client, Runner};
use phonedriver_ios::simulator::Simulator;

fn udid(var: &str) -> String {
    let v = std::env::var(var).unwrap_or_else(|_| panic!("set {var}=sim:<udid>"));
    v.strip_prefix("sim:").unwrap_or(&v).to_string()
}

fn home() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("phonedriver-home");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn answers(port: u16) -> bool {
    match Client::connect(port, "probe").await {
        Ok(c) => tokio::time::timeout(Duration::from_secs(2), c.hello())
            .await
            .is_ok_and(|h| h.is_ok()),
        Err(_) => false,
    }
}

#[tokio::test]
#[ignore]
async fn list_returns_the_booted_test_simulator() {
    let udid = udid("PHONEDRIVER_TEST_DEVICE");
    let sims = Simulator::list().await.unwrap();
    let me = sims.iter().find(|s| s.udid == udid).expect("not booted");
    assert!(me.os.starts_with("iOS "), "{me:?}");
    assert!(!me.name.is_empty());
}

#[tokio::test]
#[ignore]
async fn the_runner_builds_once_per_source_hash() {
    let first = runner::build(&home()).await.unwrap();
    let started = Instant::now();
    let second = runner::build(&home()).await.unwrap();
    assert!(!second.built);
    assert_eq!(first.xctestrun, second.xctestrun);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(second.xctestrun.is_file());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn two_simulators_get_two_runners_on_two_ports() {
    let (a, b) = (
        udid("PHONEDRIVER_TEST_DEVICE"),
        udid("PHONEDRIVER_TEST_DEVICE2"),
    );
    let home = home();
    let (ra, rb) = tokio::join!(Runner::start(&a, &home), Runner::start(&b, &home));
    let (ra, rb) = (ra.unwrap(), rb.unwrap());
    assert_ne!(ra.port, rb.port);
    assert!(ra.port >= runner::PORT_BASE && rb.port >= runner::PORT_BASE);
    assert_eq!(
        ra.client.hello().await.unwrap().udid.as_deref(),
        Some(a.as_str())
    );
    assert_eq!(
        rb.client.hello().await.unwrap().udid.as_deref(),
        Some(b.as_str())
    );
    assert_eq!(
        ra.hello.build.as_deref(),
        Some(runner::build_hash().as_str())
    );
    let (pa, pb) = (ra.port, rb.port);
    drop(ra);
    drop(rb);
    let deadline = Instant::now() + Duration::from_secs(15);
    while (answers(pa).await || answers(pb).await) && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(!answers(pa).await, "runner on {pa} outlived its owner");
    assert!(!answers(pb).await, "runner on {pb} outlived its owner");
}

/// Started by the next test in a process group of its own: starts a runner, prints its port, and waits to be killed.
#[tokio::test]
#[ignore]
async fn hold_runner() {
    let Ok(home) = std::env::var("HOLD_RUNNER_HOME") else {
        return;
    };
    let r = Runner::start(&udid("PHONEDRIVER_TEST_DEVICE"), home.as_ref())
        .await
        .unwrap();
    println!("port {}", r.port);
    std::future::pending::<()>().await;
}

#[tokio::test]
#[ignore]
async fn a_runner_left_by_a_killed_owner_is_reused_and_dies_with_its_process_group() {
    let udid = udid("PHONEDRIVER_TEST_DEVICE");
    let home = home();
    let mut owner = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "hold_runner", "--ignored", "--nocapture"])
        .env("HOLD_RUNNER_HOME", &home)
        .stdout(Stdio::piped())
        .process_group(0)
        .spawn()
        .unwrap();
    let pgid = owner.id();
    let port: u16 = std::io::BufReader::new(owner.stdout.take().unwrap())
        .lines()
        .map_while(Result::ok)
        .find_map(|l| l.strip_prefix("port ").map(|p| p.trim().parse().unwrap()))
        .expect("the owner printed no port");

    Command::new("kill")
        .args(["-KILL", &pgid.to_string()])
        .status()
        .unwrap();
    owner.wait().unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;

    let started = Instant::now();
    let again = Runner::start(&udid, &home).await.unwrap();
    assert!(again.reused());
    assert_eq!(again.port, port);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    drop(again);
    assert!(
        answers(port).await,
        "dropping a reused runner must not stop it"
    );

    Command::new("kill")
        .args(["-TERM", &format!("-{pgid}")])
        .status()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while answers(port).await && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(
        !answers(port).await,
        "the runner outlived its process group"
    );
}
