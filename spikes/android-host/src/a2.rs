use crate::adb::{Adb, Res};
use crate::agent::*;
use serde_json::json;
use std::time::Instant;

pub fn pct(v: &mut Vec<f64>) -> (f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[((v.len() as f64 * 0.95).ceil() as usize - 1).min(v.len() - 1)])
}

pub async fn touch_log(c: &mut Control) -> Res<String> {
    let r = c.req(json!({"op":"tree"})).await?;
    for n in r["nodes"].as_array().unwrap() {
        if n["desc"] == "touchlog" {
            return Ok(n["text"].as_str().unwrap_or("").to_string());
        }
    }
    Ok("<touchlog not found>".into())
}

pub async fn run(adb: &Adb, serial: &str) -> Res<()> {
    push_jar(adb, serial, 0o444).await?;
    let a = launch(adb, serial, "", true).await?;
    assert!(a.wait_listening().await, "{}", a.log_text());
    let (mut c, hello) = Control::open(adb, serial, &a.name, &a.token).await?;
    let (w, h) = (hello["display"]["w"].as_i64().unwrap(), hello["display"]["h"].as_i64().unwrap());
    println!("display {w}x{h}");
    adb.shell_output(serial, "am start -S -n dev.phoneuse.spikeapp/.Main --es mode touch").await?;
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    let (x, y) = (w / 2, h * 2 / 3);

    println!("log before: {}", touch_log(&mut c).await?);
    let mut ours = Vec::new();
    let mut agent_side = Vec::new();
    for _ in 0..50 {
        let t = Instant::now();
        let r = c.req(json!({"op":"gesture","pointers":[{"id":0,"path":[{"x":x,"y":y,"t_ms":0},{"x":x,"y":y,"t_ms":0}]}]})).await?;
        ours.push(t.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(r["ok"], true, "{r}");
        agent_side.push(r["agent_us"].as_f64().unwrap() / 1000.0);
    }
    println!("log after 50 agent taps: {}", touch_log(&mut c).await?);
    let (p50, p95) = pct(&mut ours);
    println!("agent gesture tap, host write -> reply (WAIT_FOR_FINISH): 50 taps p50={p50:.1} ms p95={p95:.1} ms");

    let (a50, a95) = pct(&mut agent_side);
    println!("agent-side time inside the agent for the same taps (injectInputEvent DOWN+UP, WAIT_FOR_FINISH): p50={a50:.1} ms p95={a95:.1} ms");
    let mut sh = Vec::new();
    for _ in 0..50 {
        let t = Instant::now();
        adb.shell_output(serial, &format!("input tap {x} {y}")).await?;
        sh.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    println!("log after 50 `input tap`: {}", touch_log(&mut c).await?);
    let (p50, p95) = pct(&mut sh);
    println!("adb shell input tap via our adb client (new shell,v2 stream each): 50 taps p50={p50:.1} ms p95={p95:.1} ms");

    let mut cli = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        std::process::Command::new("adb").args(["-s", serial, "shell", "input", "tap", &x.to_string(), &y.to_string()]).output()?;
        cli.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let (p50, p95) = pct(&mut cli);
    println!("adb CLI shell input tap: 20 taps p50={p50:.1} ms p95={p95:.1} ms");

    let mut path = vec![json!({"x":x,"y":h*3/4,"t_ms":0})];
    for i in 1..=10 {
        path.push(json!({"x":x,"y":h*3/4 - i*(h/2)/10,"t_ms":i*20}));
    }
    path.push(json!({"x":x,"y":h/4,"t_ms":400}));
    path.push(json!({"x":x,"y":h/4,"t_ms":400}));
    let t = Instant::now();
    let r = c.req(json!({"op":"gesture","pointers":[{"id":0,"path":path}]})).await?;
    println!("swipe (down, 10 moves, hold 200 ms, up) reply ok={} in {:.0} ms; log: {}", r["ok"], t.elapsed().as_secs_f64() * 1000.0, touch_log(&mut c).await?);

    adb.shell_output(serial, "am start -S -n dev.phoneuse.spikeapp/.Main --es mode touch").await?;
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    let (cy, d0, d1) = (h / 2, 80, 360);
    let mut p0 = Vec::new();
    let mut p1 = Vec::new();
    for i in 0..=10 {
        let d = d0 + (d1 - d0) * i / 10;
        p0.push(json!({"x":x - d,"y":cy,"t_ms":i*30}));
        p1.push(json!({"x":x + d,"y":cy,"t_ms":i*30}));
    }
    p0.push(json!({"x":x-d1,"y":cy,"t_ms":330}));
    p1.push(json!({"x":x+d1,"y":cy,"t_ms":330}));
    let t = Instant::now();
    let r = c.req(json!({"op":"gesture","pointers":[{"id":0,"path":p0},{"id":1,"path":p1}]})).await?;
    println!("pinch spread reply ok={} err={} in {:.0} ms; log: {}", r["ok"], r["error"], t.elapsed().as_secs_f64() * 1000.0, touch_log(&mut c).await?);
    println!("agent log tail: {}", a.log_text().lines().rev().take(3).collect::<Vec<_>>().join(" | "));
    Ok(())
}

pub async fn maps(adb: &Adb, serial: &str, pkg: &str, out: &str) -> Res<()> {
    push_jar(adb, serial, 0o444).await?;
    let a = launch(adb, serial, "", true).await?;
    assert!(a.wait_listening().await);
    let (mut c, hello) = Control::open(adb, serial, &a.name, &a.token).await?;
    let (w, h) = (hello["display"]["w"].as_i64().unwrap(), hello["display"]["h"].as_i64().unwrap());
    adb.shell_output(serial, &format!("monkey -p {pkg} -c android.intent.category.LAUNCHER 1")).await?;
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;
    let shot = |name: String| {
        let serial = serial.to_string();
        async move {
            let o = std::process::Command::new("adb").args(["-s", &serial, "exec-out", "screencap", "-p"]).output().unwrap();
            std::fs::write(name, o.stdout).unwrap();
        }
    };
    shot(format!("{out}_before.png")).await;
    let (x, cy) = (w / 2, h / 2);
    let mut p0 = Vec::new();
    let mut p1 = Vec::new();
    for i in 0..=10 {
        let d = 60 + (400 - 60) * i / 10;
        p0.push(json!({"x":x - d,"y":cy - d,"t_ms":i*30}));
        p1.push(json!({"x":x + d,"y":cy + d,"t_ms":i*30}));
    }
    p0.push(json!({"x":x-400,"y":cy-400,"t_ms":330}));
    p1.push(json!({"x":x+400,"y":cy+400,"t_ms":330}));
    let r = c.req(json!({"op":"gesture","pointers":[{"id":0,"path":p0},{"id":1,"path":p1}]})).await?;
    println!("pinch ok={}", r["ok"]);
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    shot(format!("{out}_after.png")).await;
    Ok(())
}
