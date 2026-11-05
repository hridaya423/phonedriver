use crate::adb::{Adb, Res};
use crate::agent::*;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn agent_pids(adb: &Adb, serial: &str) -> Res<String> {
    let (o, _, _) = adb.shell_output(serial, "ps -A -o PID,ARGS | grep '[d]ev.phoneuse.agent'").await?;
    Ok(String::from_utf8_lossy(&o).trim().to_string())
}

pub async fn run(adb: &Adb, serial: &str) -> Res<()> {
    let (o, _, _) = adb.shell_output(serial, "getprop ro.build.version.sdk").await?;
    println!("API {}", String::from_utf8_lossy(&o).trim());

    println!("\n(c) jar mode");
    for mode in [0o444u32, 0o644] {
        push_jar(adb, serial, mode).await?;
        let (o, _, _) = adb.shell_output(serial, &format!("ls -l {JAR}")).await?;
        let a = launch(adb, serial, "", true).await?;
        let ok = a.wait_listening().await;
        println!("  mode {:o}: {} -> loaded={} ; log: {}", mode, String::from_utf8_lossy(&o).trim(), ok, a.log_text().lines().take(3).collect::<Vec<_>>().join(" | ").chars().take(260).collect::<String>());
        drop(a);
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    push_jar(adb, serial, 0o444).await?;

    println!("\n(a) stream close kills the agent");
    let a = launch(adb, serial, "", true).await?;
    assert!(a.wait_listening().await, "{}", a.log_text());
    println!("  while running: {}", agent_pids(adb, serial).await?);
    let t = Instant::now();
    let Agent { stdin, reader, .. } = a;
    drop(stdin);
    reader.abort();
    let _ = reader.await;
    let mut gone_ms = None;
    for _ in 0..200 {
        if agent_pids(adb, serial).await?.is_empty() {
            gone_ms = Some(t.elapsed().as_millis());
            break;
        }
    }
    println!("  gone after {:?} ms (polling includes ~60 ms per ps round trip)", gone_ms);

    println!("\n(b) pkill patterns");
    for pat in ["phone-use-agent", "dev.phoneuse.agent.Main", "'dev.phoneuse.agent.[M]ain'"] {
        let a = launch(adb, serial, "", true).await?;
        assert!(a.wait_listening().await);
        let (o, e, _) = adb.shell_output(serial, &format!("pkill -f {pat}; echo rc=$?")).await?;
        tokio::time::sleep(Duration::from_millis(400)).await;
        println!("  pkill -f {pat}: out={:?} err={:?}; agent alive afterwards: {}", String::from_utf8_lossy(&o).trim(), String::from_utf8_lossy(&e).trim(), !agent_pids(adb, serial).await?.is_empty());
        drop(a);
        tokio::time::sleep(Duration::from_millis(1500)).await;
    }

    println!("\n(d) token");
    let a = launch(adb, serial, "", true).await?;
    assert!(a.wait_listening().await);
    let mut s = adb.open_abstract(serial, &a.name).await?;
    s.write_all(b"C").await?;
    let bad = serde_json::json!({"id":0,"op":"hello","version":"0.1"});
    write_frame(&mut s, &bad).await?;
    let mut b = [0u8; 1];
    let r = tokio::time::timeout(Duration::from_secs(2), s.read(&mut b)).await;
    println!("  no token: {:?}", match r { Ok(Ok(0)) => "closed with no reply".to_string(), Ok(Ok(n)) => format!("got {n} bytes"), Ok(Err(e)) => format!("error {e}"), Err(_) => "no close within 2 s".into() });
    let mut s = adb.open_abstract(serial, &a.name).await?;
    s.write_all(b"C").await?;
    write_frame(&mut s, &serde_json::json!({"id":0,"op":"hello","version":"0.1","token":"00".repeat(32)})).await?;
    let r = tokio::time::timeout(Duration::from_secs(2), s.read(&mut b)).await;
    println!("  wrong token: {:?}", match r { Ok(Ok(0)) => "closed with no reply".to_string(), Ok(Ok(n)) => format!("got {n} bytes"), Ok(Err(e)) => format!("error {e}"), Err(_) => "no close within 2 s".into() });
    let (_c, hello) = Control::open(adb, serial, &a.name, &a.token).await?;
    println!("  right token: {hello}");
    println!("  agent log:\n{}", a.log_text());
    drop(_c);
    drop(a);
    tokio::time::sleep(Duration::from_millis(2000)).await;

    println!("\n(extra) agent exits on its own when nothing connects (10 s)");
    let a = launch(adb, serial, "", true).await?;
    assert!(a.wait_listening().await);
    let t = Instant::now();
    loop {
        if agent_pids(adb, serial).await?.is_empty() || t.elapsed() > Duration::from_secs(20) {
            break;
        }
    }
    println!("  gone after {} ms; log tail: {:?}", t.elapsed().as_millis(), a.log_text().lines().last());
    Ok(())
}
