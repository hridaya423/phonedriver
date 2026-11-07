mod support;

use std::time::{Duration, Instant};

use phonedriver_android::adb::Adb;
use phonedriver_android::agent::{self, Agent};
use phonedriver_core::error::Error;
use serde_json::json;
use support::agent as raw;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn agent_processes(adb: &Adb, serial: &str) -> Vec<String> {
    let out = adb
        .shell(
            serial,
            "ps -A -o ARGS | grep 'dev.phonedriver.agent.[M]ain'",
        )
        .await
        .unwrap();
    out.stdout_text().lines().map(str::to_string).collect()
}

async fn wait_until_gone(adb: &Adb, serial: &str, within: Duration) -> Duration {
    let start = Instant::now();
    while start.elapsed() < within {
        if agent_processes(adb, serial).await.is_empty() {
            return start.elapsed();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "agent still running after {within:?}: {:?}",
        agent_processes(adb, serial).await
    );
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn deploy_pushes_once_with_mode_0444() {
    let _g = raw::exclusive().await;
    let (adb, serial) = (support::adb(), support::serial());
    let first = agent::deploy(&adb, &serial).await.unwrap();
    adb.shell(&serial, &format!("rm -f {}", first.path))
        .await
        .unwrap();
    let pushed = agent::deploy(&adb, &serial).await.unwrap();
    assert!(pushed.pushed, "{pushed:?}");
    assert!(
        pushed
            .path
            .starts_with("/data/local/tmp/phonedriver-agent-")
    );
    assert!(pushed.path.ends_with(".jar"));
    let mode = adb
        .shell(&serial, &format!("stat -c %a {}", pushed.path))
        .await
        .unwrap();
    assert_eq!(mode.stdout_text().trim(), "444");
    let again = agent::deploy(&adb, &serial).await.unwrap();
    assert!(!again.pushed, "{again:?}");
    assert_eq!(again.path, pushed.path);
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn start_connects_both_streams_and_a_stale_agent_is_killed() {
    let _g = raw::exclusive().await;
    let (adb, serial) = (support::adb(), support::serial());
    let jar = agent::deploy(&adb, &serial).await.unwrap().path;
    let mut stale = adb
        .shell_stream(
            &serial,
            &format!(
                "CLASSPATH={jar} app_process / dev.phonedriver.agent.Main 0.0.0 stale-agent-socket"
            ),
        )
        .await
        .unwrap();
    stale.write_stdin(b"stale-token\n").await.unwrap();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(
        agent_processes(&adb, &serial)
            .await
            .iter()
            .any(|l| l.contains(" 0.0.0 ")),
        "the stale agent should be running first"
    );

    let conn = Agent::start(&adb, &serial).await.unwrap();
    assert!(conn.hello.api >= 26);
    assert!(conn.hello.display.w > 0 && conn.hello.density > 0.5);
    let procs = agent_processes(&adb, &serial).await;
    assert!(!procs.is_empty());
    assert!(
        procs.iter().all(|l| !l.contains(" 0.0.0 ")),
        "stale agent survived: {procs:?}"
    );
    drop(conn);
    wait_until_gone(&adb, &serial, Duration::from_millis(2500)).await;
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn closing_the_shell_stream_ends_the_agent_within_a_second_and_a_half() {
    let _g = raw::exclusive().await;
    let (adb, serial) = (support::adb(), support::serial());
    let conn = Agent::start(&adb, &serial).await.unwrap();
    assert!(!agent_processes(&adb, &serial).await.is_empty());
    drop(conn);
    let gone = wait_until_gone(&adb, &serial, Duration::from_millis(2500)).await;
    assert!(gone < Duration::from_millis(1500), "took {gone:?}");
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn an_agent_nobody_connects_to_exits_after_ten_seconds() {
    let _g = raw::exclusive().await;
    let (adb, serial) = (support::adb(), support::serial());
    let launched = agent::launch(&adb, &serial).await.unwrap();
    let started = Instant::now();
    tokio::time::sleep(Duration::from_secs(8)).await;
    assert!(
        !agent_processes(&adb, &serial).await.is_empty(),
        "exited early"
    );
    wait_until_gone(&adb, &serial, Duration::from_secs(4)).await;
    let lived = started.elapsed();
    assert!(
        lived > Duration::from_secs(9) && lived < Duration::from_secs(13),
        "lived {lived:?}"
    );
    drop(launched);
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn a_connection_without_the_token_is_dropped() {
    let _g = raw::exclusive().await;
    let (adb, serial) = (support::adb(), support::serial());
    let launched = agent::launch(&adb, &serial).await.unwrap();
    let mut s = adb.open_abstract(&serial, &launched.name).await.unwrap();
    s.write_all(b"C").await.unwrap();
    raw::write_frame(
        &mut s,
        &json!({"id":0,"op":"hello","version":agent::VERSION}),
    )
    .await;
    let mut buf = [0u8; 1];
    let n = tokio::time::timeout(Duration::from_secs(2), s.read(&mut buf))
        .await
        .expect("the agent must close the stream")
        .unwrap_or(0);
    assert_eq!(n, 0);
    drop(launched);
}

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn another_ui_automation_client_is_reported_as_a_conflict() {
    let _g = raw::exclusive().await;
    let (adb, serial) = (support::adb(), support::serial());
    let mut holder = adb
        .shell_stream(&serial, "uiautomator events")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let result = Agent::start(&adb, &serial).await;
    drop(holder.close_stdin().await);
    drop(holder);
    match result {
        Err(Error::AutomationConflict { .. }) => {}
        Err(e) => panic!("expected AutomationConflict, got {e:?}"),
        Ok(_) => panic!("expected AutomationConflict, got a connection"),
    }
}
