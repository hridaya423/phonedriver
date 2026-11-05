mod a2;
mod a3;
mod a4;
mod video;
mod a5;
mod adb;
mod agent;
use adb::{Adb, Res};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn pct(v: &mut Vec<f64>) -> (f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = v[v.len() / 2];
    let p95 = v[((v.len() as f64 * 0.95).ceil() as usize - 1).min(v.len() - 1)];
    (p50, p95)
}

#[tokio::main]
async fn main() -> Res<()> {
    let args: Vec<String> = std::env::args().collect();
    let adb = Adb::from_env();
    let serial = args.get(2).cloned().unwrap_or_default();
    match args.get(1).map(String::as_str) {
        Some("a0") => a0(&adb, &serial).await,
        Some("a2") => a2::run(&adb, &serial).await,
        Some("a2maps") => a2::maps(&adb, &serial, &args[3], &args[4]).await,
        Some("a4") => a4::run(&adb, &serial).await,
        Some("a4chrome") => a4::chrome(&adb, &serial).await,
        Some("a3") => a3::run(&adb, &serial, args.get(3).map(String::as_str).unwrap_or("all")).await,
        Some("fixture") => a3::check_fixture(&args[2]),
        Some("a5") => a5::run(&adb, &serial).await,
        other => Err(format!("unknown command {other:?}").into()),
    }
}

async fn a0(adb: &Adb, serial: &str) -> Res<()> {
    println!("host:version = {}", adb.version().await?);
    println!("host:track-devices-l first reply:\n{}", adb.track_devices_first().await?);

    let mut vv = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        adb.version().await?;
        vv.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let (p50, p95) = pct(&mut vv);
    println!("host:version (new TCP connection + 1 request; tunnel floor): 20 runs p50={p50:.1} ms p95={p95:.1} ms");

    let mut v = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        let (out, _, code) = adb.shell_output(serial, "echo hi").await?;
        v.push(t.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(out, b"hi\n");
        assert_eq!(code, 0);
    }
    let (p50, p95) = pct(&mut v);
    println!("shell,v2,raw echo hi (new connection each): 20 runs p50={p50:.1} ms p95={p95:.1} ms");

    let mut sh = adb.shell(serial, "cat").await?;
    sh.write_stdin(b"token-abc\n").await?;
    let (id, d) = sh.read_packet().await?;
    println!("stdin round trip without closing stdin: id={id} data={:?}", String::from_utf8_lossy(&d));
    sh.close_stdin().await?;
    let (_, _, code) = sh.run_to_end().await?;
    println!("after close_stdin cat exit code {code}");

    let data = vec![0x5au8; 1024 * 1024];
    let mut tp = Vec::new();
    for i in 0..5 {
        let t = Instant::now();
        adb.push(serial, "/data/local/tmp/a0_1mb.bin", 0o444, &data).await?;
        let (o, _, _) = adb.shell_output(serial, "ls -l /data/local/tmp/a0_1mb.bin").await?;
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        tp.push(1.0 / (ms / 1000.0));
        if i == 0 {
            println!("push listing: {}", String::from_utf8_lossy(&o).trim());
        }
    }
    let (p50, p95) = pct(&mut tp);
    println!("push 1 MB x5 (includes verifying ls): throughput p50={p50:.2} MB/s p95={p95:.2} MB/s");

    let name = "phoneuse_a0_echo";
    let mut agent = adb
        .shell(serial, &format!("CLASSPATH=/data/local/tmp/spike.jar app_process / dev.phoneuse.spike.Main echo {name}"))
        .await?;
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    let mut rt = Vec::new();
    let mut s = adb.open_abstract(serial, name).await?;
    for _ in 0..50 {
        let t = Instant::now();
        s.write_all(b"ping").await?;
        let mut b = [0u8; 4];
        s.read_exact(&mut b).await?;
        rt.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let (p50, p95) = pct(&mut rt);
    println!("localabstract:{name} echo through tunnel (50 pings): p50={p50:.1} ms p95={p95:.1} ms");
    let big = vec![7u8; 1 << 20];
    let t = Instant::now();
    let (mut r, mut w) = s.split();
    let wr = async { w.write_all(&big).await };
    let rd = async {
        let mut got = vec![0u8; big.len()];
        r.read_exact(&mut got).await.map(|_| got)
    };
    let (a, b) = tokio::join!(wr, rd);
    a?;
    assert_eq!(b?, big);
    println!("localabstract 1 MB echo round trip: {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
    drop(s);
    agent.s.shutdown().await.ok();
    let _ = adb.shell_output(serial, "pkill -f dev.phoneuse.spike.Main").await;
    Ok(())
}
