use crate::adb::{Adb, Res};
use crate::agent::*;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const S: &str = "héllo 世界 مرحبا 👋🏽";

async fn start_edit(adb: &Adb, serial: &str) -> Res<()> {
    adb.shell_output(serial, "am start -S -n dev.phoneuse.spikeapp/.Main --es mode edit").await?;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    Ok(())
}

async fn text(c: &mut Control, mode: &str, t: &str, extra: Value) -> Res<(Value, f64)> {
    let mut q = json!({"op":"text","mode":mode,"text":t,"readback":true});
    for (k, v) in extra.as_object().unwrap() {
        q[k] = v.clone();
    }
    let s = Instant::now();
    let r = c.req(q).await?;
    Ok((r, s.elapsed().as_secs_f64() * 1000.0))
}

fn show(label: &str, want: &str, r: &Value, rtt: f64) {
    let got = r["readback"].as_str().unwrap_or("<null>");
    println!("  {label}: ok={} exact={} agent_ms={:.1} host_rtt_ms={:.0} readback={:?} err={}", r["ok"], got == want, r["elapsed_ms"].as_f64().unwrap_or(-1.0), rtt, got, r["error"]);
}

pub async fn run(adb: &Adb, serial: &str) -> Res<()> {
    push_jar(adb, serial, 0o444).await?;
    let a = launch(adb, serial, "", true).await?;
    assert!(a.wait_listening().await, "{}", a.log_text());
    let (mut c, hello) = Control::open(adb, serial, &a.name, &a.token).await?;
    let api = hello["api"].as_i64().unwrap();
    println!("API {api}; test string {S:?} ({} UTF-16 units)", S.encode_utf16().count());

    println!("\nEditText in test app");
    start_edit(adb, serial).await?;
    c.req(json!({"op":"clipboard_set","text":"OLD-CLIP"})).await?;
    let (r, rtt) = text(&mut c, "paste", S, json!({})).await?;
    show("paste", S, &r, rtt);
    let g = c.req(json!({"op":"clipboard_get"})).await?;
    println!("  clipboard after paste (restored): {}", g["text"]);

    start_edit(adb, serial).await?;
    let (r, rtt) = text(&mut c, "set", S, json!({})).await?;
    show("set_text", S, &r, rtt);

    start_edit(adb, serial).await?;
    let (r, rtt) = text(&mut c, "keys", "héllo", json!({})).await?;
    show("keys héllo", "héllo", &r, rtt);
    start_edit(adb, serial).await?;
    let (r, rtt) = text(&mut c, "keys", "hi 👋", json!({})).await?;
    show("keys with emoji (expect getEvents null -> error, nothing typed)", "", &r, rtt);
    start_edit(adb, serial).await?;
    let (r, rtt) = text(&mut c, "keys", "hello world", json!({})).await?;
    show("keys ASCII", "hello world", &r, rtt);

    let hundred: String = "a世界b😀".repeat(100).chars().scan(0usize, |n, ch| { *n += ch.len_utf16(); if *n <= 100 { Some(ch) } else { None } }).collect();
    println!("  100-unit string: {} units", hundred.encode_utf16().count());
    for mode in ["paste", "set", "keys"] {
        let s: String = if mode == "keys" { "abcdefghij".repeat(10) } else { hundred.clone() };
        let mut times = Vec::new();
        let mut rtts = Vec::new();
        let mut exact = 0;
        for _ in 0..10 {
            start_edit(adb, serial).await?;
            let (r, rtt) = text(&mut c, mode, &s, json!({})).await?;
            times.push(r["elapsed_ms"].as_f64().unwrap_or(-1.0));
            rtts.push(rtt);
            if r["readback"].as_str() == Some(&s) {
                exact += 1;
            }
        }
        let (a50, a95) = crate::a2::pct(&mut times);
        let (h50, h95) = crate::a2::pct(&mut rtts);
        println!("  100 chars via {mode}: exact readback {exact}/10; agent time p50={a50:.1} p95={a95:.1} ms; host rtt (includes 150 ms readback delay) p50={h50:.0} p95={h95:.0} ms");
    }

    println!("\nclipboard restore of an image-URI clip");
    start_edit(adb, serial).await?;
    let ru = c.req(json!({"op":"clipboard_set_uri","uri":"content://settings/system/screen_off_timeout"})).await?;
    println!("  set uri clip: {ru}");
    let before = c.req(json!({"op":"clipboard_get"})).await?;
    let (r, rtt) = text(&mut c, "paste", "XYZ", json!({})).await?;
    show("paste while an image URI is on the clipboard", "XYZ", &r, rtt);
    let after = c.req(json!({"op":"clipboard_get"})).await?;
    println!("  before: {}\n  after : {}", before["desc"], after["desc"]);

    if api >= 33 {
        println!("\nclipboard overlay on API {api}");
        adb.shell_output(serial, "input keyevent KEYCODE_HOME").await?;
        tokio::time::sleep(Duration::from_millis(1000)).await;
        for sensitive in [false, true] {
            c.req(json!({"op":"clipboard_set","text":format!("overlay test sensitive={sensitive}"),"sensitive":sensitive})).await?;
            for (i, ms) in [300u64, 1200, 4000].iter().enumerate() {
                tokio::time::sleep(Duration::from_millis(if i == 0 { *ms } else { *ms - [300u64, 1200, 4000][i - 1] })).await;
                let o = std::process::Command::new("adb").args(["-s", serial, "exec-out", "screencap", "-p"]).output()?;
                let name = format!("../overlay_{api}_sens{}_{}ms.png", sensitive as u8, ms);
                std::fs::write(&name, o.stdout)?;
                println!("  saved {name}");
            }
            tokio::time::sleep(Duration::from_millis(4000)).await;
        }
    }
    Ok(())
}

pub async fn chrome(adb: &Adb, serial: &str) -> Res<()> {
    push_jar(adb, serial, 0o444).await?;
    let a = launch(adb, serial, "", true).await?;
    assert!(a.wait_listening().await, "{}", a.log_text());
    let (mut c, _) = Control::open(adb, serial, &a.name, &a.token).await?;
    for restore in [0i64, 50, 150, 500] {
        for _ in 0..16 {
            c.req(json!({"op":"key","keycode":67})).await?;
        }
        c.req(json!({"op":"clipboard_set","text":"OLD-CLIP"})).await?;
        let (r, rtt) = text(&mut c, "paste", "PASTED-NEW", json!({"restore_ms":restore})).await?;
        tokio::time::sleep(Duration::from_millis(400)).await;
        let g = c.req(json!({"op":"clipboard_get"})).await?;
        let o = std::process::Command::new("adb").args(["-s", serial, "exec-out", "screencap", "-p"]).output()?;
        let name = format!("../chrome_restore{restore}.png");
        std::fs::write(&name, o.stdout)?;
        println!("restore_ms={restore}: ok={} readback={} clipboard_after={} shot={name} rtt={rtt:.0}", r["ok"], r["readback"], g["text"]);
    }
    Ok(())
}
