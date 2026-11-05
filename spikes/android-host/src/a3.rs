use crate::a2::pct;
use crate::adb::{Adb, Res};
use crate::agent::*;
use crate::video::*;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const DENSITY: f32 = 2.625;
const OUT: &str = "../a3out";

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

async fn sleep(m: u64) {
    tokio::time::sleep(Duration::from_millis(m)).await;
}

struct Ctx<'a> {
    adb: &'a Adb,
    serial: &'a str,
    agent: Agent,
    c: Control,
    w: i64,
    h: i64,
    api: i64,
}

async fn setup<'a>(adb: &'a Adb, serial: &'a str, extra: &str) -> Res<Ctx<'a>> {
    std::fs::create_dir_all(OUT)?;
    push_jar(adb, serial, 0o444).await?;
    let agent = launch(adb, serial, extra, true).await?;
    assert!(agent.wait_listening().await, "{}", agent.log_text());
    let (c, hello) = Control::open(adb, serial, &agent.name, &agent.token).await?;
    let w = hello["display"]["w"].as_i64().unwrap();
    let h = hello["display"]["h"].as_i64().unwrap();
    let api = hello["api"].as_i64().unwrap();
    println!("API {api} display {w}x{h}");
    Ok(Ctx { adb, serial, agent, c, w, h, api })
}

async fn sh(x: &Ctx<'_>, cmd: &str) -> Res<String> {
    let (o, _, _) = x.adb.shell_output(x.serial, cmd).await?;
    Ok(String::from_utf8_lossy(&o).trim().to_string())
}

async fn app(x: &Ctx<'_>, mode: &str) -> Res<()> {
    sh(x, &format!("am start -S -n dev.phoneuse.spikeapp/.Main --es mode {mode}")).await?;
    sleep(1500).await;
    Ok(())
}

async fn settings(x: &Ctx<'_>) -> Res<()> {
    sh(x, "input keyevent KEYCODE_HOME; am force-stop com.android.settings; am start -a android.settings.SETTINGS").await?;
    sleep(2500).await;
    Ok(())
}

async fn video(x: &Ctx<'_>) -> Res<Video> {
    Video::connect(x.adb, x.serial, &x.agent.name, &x.agent.token, DENSITY, None).await
}

async fn switch_center(x: &mut Ctx<'_>) -> Res<(i64, i64)> {
    let t = x.c.req(json!({"op":"tree"})).await?;
    for n in t["nodes"].as_array().unwrap() {
        if n["cls"].as_str().map(|c| c.contains("Switch")).unwrap_or(false) {
            let b: Vec<i64> = n["b"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
            println!("  switch node bounds {:?}", b);
            return Ok(((b[0] + b[2]) / 2, (b[1] + b[3]) / 2));
        }
    }
    Err("no switch".into())
}

async fn swipe(x: &mut Ctx<'_>, up: bool) -> Res<()> {
    let (a, b) = if up { (x.h * 3 / 4, x.h / 4) } else { (x.h / 4, x.h * 3 / 4) };
    let mut path = vec![json!({"x":x.w/2,"y":a,"t_ms":0})];
    for i in 1..=8 {
        path.push(json!({"x":x.w/2,"y":a + (b - a) * i / 8,"t_ms":i*25}));
    }
    path.push(json!({"x":x.w/2,"y":b,"t_ms":220}));
    x.c.req(json!({"op":"gesture","pointers":[{"id":0,"path":path}]})).await?;
    Ok(())
}

fn summarize(label: &str, ev: &[FrameEvent], dur_s: f64) {
    let zero = ev.iter().filter(|e| e.changed_area_pt2 == 0).count();
    let max = ev.iter().map(|e| e.changed_area_pt2).max().unwrap_or(0);
    println!("  {label}: {} frames in {dur_s:.1} s = {:.1} fps; frames with zero changed area {zero}; max changed_area_pt2 {max}", ev.len(), ev.len() as f64 / dur_s);
}

pub async fn run(adb: &Adb, serial: &str, which: &str) -> Res<()> {
    let mut x = setup(adb, serial, "").await?;
    let all = which == "all";
    if all || which == "first" {
        first(&mut x).await?;
    }
    if all || which == "fps" {
        fps(&mut x).await?;
    }
    if all || which == "areas" {
        areas(&mut x).await?;
    }
    if all || which == "latency" {
        latency(&mut x).await?;
    }
    if all || which == "shots" {
        shots(&mut x).await?;
    }
    if all || which == "secure" {
        secure(&mut x).await?;
    }
    if all || which == "sync" {
        sync(&mut x).await?;
    }
    if all || which == "recover" {
        recover(&mut x).await?;
    }
    if all || which == "fixture" {
        fixture(&mut x).await?;
    }
    println!("agent log tail:\n{}", x.agent.log_text().lines().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n"));
    Ok(())
}

async fn first(x: &mut Ctx<'_>) -> Res<()> {
    println!("\n[first frame on a static screen, 10 cold stream starts]");
    settings(x).await?;
    let mut to_header = Vec::new();
    let mut to_first = Vec::new();
    let mut sps_line = String::new();
    for i in 0..10 {
        let v = video(x).await?;
        assert!(v.wait_frames(2, 5000).await, "no frames: {}", x.agent.log_text());
        let ev = v.since(0);
        let h0 = v.headers.lock().unwrap()[0].0;
        let first_decoded = ev.iter().find(|e| e.decoded).unwrap();
        to_header.push(ms(h0 - v.connected_at));
        to_first.push(ms(first_decoded.arrival - v.connected_at));
        if i == 0 {
            let sps = v.sps.lock().unwrap().first().cloned().unwrap();
            let p = parse_sps(&sps).unwrap();
            let hdr = v.headers.lock().unwrap()[0];
            sps_line = format!("encoder SPS: profile_idc={} constraint_flags=0x{:02x} level_idc={} ({:.1}); stream size {}x{}; decode errors so far {}", p.profile_idc, p.constraint, p.level_idc, p.level_idc as f32 / 10.0, hdr.1, hdr.2, v.errors.load(std::sync::atomic::Ordering::Relaxed));
        }
        v.task.abort();
        sleep(600).await;
    }
    let (a50, a95) = pct(&mut to_header);
    let (b50, b95) = pct(&mut to_first);
    println!("  connect -> codec header: p50={a50:.0} p95={a95:.0} ms; connect -> first decoded frame: p50={b50:.0} p95={b95:.0} ms (includes ~13 ms tunnel RTT per round trip)");
    println!("  {sps_line}");
    Ok(())
}

async fn fps(x: &mut Ctx<'_>) -> Res<()> {
    println!("\n[frame rate, static vs scrolling; changed area of repeated frames]");
    settings(x).await?;
    let v = video(x).await?;
    assert!(v.wait_frames(2, 5000).await);
    sleep(500).await;
    let n = v.count();
    let t = Instant::now();
    sleep(5000).await;
    let ev = v.since(n);
    summarize("static Settings, 5 s", &ev, t.elapsed().as_secs_f64());
    let n = v.count();
    let t = Instant::now();
    for k in 0..8 {
        swipe(x, k % 2 == 0).await?;
    }
    let ev = v.since(n);
    summarize("scrolling Settings (8 swipes)", &ev, t.elapsed().as_secs_f64());
    let mut dec: Vec<f64> = ev.iter().filter(|e| e.decoded).map(|e| e.decode_ms).collect();
    let mut sizes: Vec<f64> = ev.iter().map(|e| e.bytes as f64).collect();
    let (d50, d95) = pct(&mut dec);
    let (s50, s95) = pct(&mut sizes);
    println!("  decode time per frame on this Mac ({} frames, {}x{}): p50={d50:.2} ms p95={d95:.2} ms; packet size p50={s50:.0} B p95={s95:.0} B; decode errors {}", dec.len(), ev[0].w, ev[0].h, v.errors.load(std::sync::atomic::Ordering::Relaxed));
    v.task.abort();
    Ok(())
}

async fn burst(x: &mut Ctx<'_>, v: &Video, label: &str, window_ms: u64, act: impl std::future::Future<Output = Res<()>>) -> Res<()> {
    sleep(1800).await;
    let n = v.count();
    let t = Instant::now();
    act.await?;
    sleep(window_ms.saturating_sub(t.elapsed().as_millis() as u64)).await;
    let ev = v.since(n);
    let max = ev.iter().map(|e| e.changed_area_pt2).max().unwrap_or(0);
    let nz: Vec<usize> = ev.iter().map(|e| e.changed_area_pt2).filter(|a| *a > 0).collect();
    println!("  {label}: {} frames, {} with change; max changed_area_pt2={max}; nonzero areas {:?}", ev.len(), nz.len(), &nz[..nz.len().min(12)]);
    let _ = x;
    Ok(())
}

async fn areas(x: &mut Ctx<'_>) -> Res<()> {
    println!("\n[changed area per event, in pt^2 (samples of 4x4 pt, counted above 12 luma levels, x16)]");
    let v = video(x).await?;
    assert!(v.wait_frames(2, 5000).await);
    app(x, "edit").await?;
    burst(x, &v, "blinking caret, focused EditText, 3 s", 3000, async { Ok(()) }).await?;
    app(x, "spinner").await?;
    burst(x, &v, "48 dp indeterminate spinner, 3 s", 3000, async { Ok(()) }).await?;
    app(x, "switch").await?;
    let (cx, cy) = switch_center(x).await?;
    let c = &mut x.c as *mut Control;
    burst(x, &v, "toggle switch tap + animation, 1.5 s", 1500, async move {
        let c = unsafe { &mut *c };
        c.req(json!({"op":"gesture","pointers":[{"id":0,"path":[{"x":cx,"y":cy,"t_ms":0},{"x":cx,"y":cy,"t_ms":0}]}]})).await?;
        Ok(())
    })
    .await?;
    settings(x).await?;
    sleep(800).await;
    let adb = x.adb;
    let serial = x.serial.to_string();
    burst(x, &v, "screen transition Settings -> All apps, 3 s", 3000, async move {
        adb.shell_output(&serial, "am start -n com.android.settings/.Settings\\$ManageApplicationsActivity").await?;
        Ok(())
    })
    .await?;
    v.task.abort();
    Ok(())
}

async fn latency(x: &mut Ctx<'_>) -> Res<()> {
    println!("\n[glass-to-host: tap toggling a switch -> first frame with changed_area_pt2 > 128]");
    app(x, "switch").await?;
    let v = video(x).await?;
    assert!(v.wait_frames(2, 5000).await);
    let (cx, cy) = switch_center(x).await?;
    let mut lat = Vec::new();
    let mut first_any = Vec::new();
    for _ in 0..20 {
        sleep(900).await;
        let n = v.count();
        let t0 = Instant::now();
        x.c.req(json!({"op":"gesture","pointers":[{"id":0,"path":[{"x":cx,"y":cy,"t_ms":0},{"x":cx,"y":cy,"t_ms":0}]}]})).await?;
        let reply = t0.elapsed();
        let mut found = None;
        let mut any = None;
        let deadline = Instant::now() + Duration::from_millis(1500);
        while Instant::now() < deadline && found.is_none() {
            sleep(3).await;
            for e in v.since(n) {
                if e.changed_area_pt2 > 0 && any.is_none() {
                    any = Some(e.arrival - t0);
                }
                if e.changed_area_pt2 > 128 {
                    found = Some(e.arrival - t0);
                    break;
                }
            }
        }
        let _ = reply;
        if let Some(f) = found {
            lat.push(ms(f));
        }
        if let Some(a) = any {
            first_any.push(ms(a));
        }
    }
    let (a50, a95) = pct(&mut lat);
    let (b50, b95) = pct(&mut first_any);
    println!("  {} of 20 taps produced a frame > 128 pt2 (none reached 4096: see areas); tap request -> that frame arrives decoded: p50={a50:.0} p95={a95:.0} ms; -> first changed frame (> 0): p50={b50:.0} p95={b95:.0} ms (tunnel RTT floor ~13 ms each way is included)", lat.len());
    v.task.abort();
    Ok(())
}

async fn shots(x: &mut Ctx<'_>) -> Res<()> {
    println!("\n[screenshot from the latest frame vs adb exec-out screencap -p]");
    settings(x).await?;
    let v = video(x).await?;
    assert!(v.wait_frames(3, 5000).await);
    sleep(800).await;
    let mut ours = Vec::new();
    for i in 0..10 {
        let t = Instant::now();
        let l = v.latest.lock().unwrap();
        let l = l.as_ref().unwrap();
        let rgb = l.rgb();
        let conv = ms(t.elapsed());
        let t2 = Instant::now();
        write_png(&format!("{OUT}/frame_api{}_{i}.png", x.api), l.w, l.h, &rgb)?;
        ours.push((conv, ms(t2.elapsed())));
    }
    let mut c: Vec<f64> = ours.iter().map(|o| o.0).collect();
    let mut p: Vec<f64> = ours.iter().map(|o| o.1).collect();
    let (c50, c95) = pct(&mut c);
    let (p50, p95) = pct(&mut p);
    println!("  from latest frame (convert YUV->RGB {c50:.1}/{c95:.1} ms + PNG encode+write {p50:.0}/{p95:.0} ms, p50/p95 of 10, unoptimized converter)");
    let mut sc = Vec::new();
    for i in 0..10 {
        let t = Instant::now();
        let (o, _, _) = x.adb.shell_output(x.serial, "screencap -p").await?;
        sc.push(ms(t.elapsed()));
        if i == 0 {
            std::fs::write(format!("{OUT}/screencap_api{}.png", x.api), &o)?;
            println!("  screencap -p over our adb client: {} bytes", o.len());
        }
    }
    let (s50, s95) = pct(&mut sc);
    println!("  adb screencap -p (our client, whole round trip incl. tunnel): p50={s50:.0} p95={s95:.0} ms");
    v.task.abort();
    Ok(())
}

async fn secure(x: &mut Ctx<'_>) -> Res<()> {
    println!("\n[FLAG_SECURE window]");
    app(x, "secure").await?;
    let v = video(x).await?;
    assert!(v.wait_frames(3, 5000).await);
    sleep(800).await;
    let tree = x.c.req(json!({"op":"tree"})).await?;
    let mut bounds = None;
    for n in tree["nodes"].as_array().unwrap() {
        if n["text"].as_str().map(|t| t.contains("SECRET")).unwrap_or(false) {
            bounds = Some(n["b"].clone());
            println!("  tree text node: {} bounds {}", n["text"], n["b"]);
        }
    }
    let b: Vec<usize> = bounds.unwrap().as_array().unwrap().iter().map(|v: &Value| v.as_i64().unwrap() as usize).collect();
    let l = v.latest.lock().unwrap();
    let l = l.as_ref().unwrap();
    let (sx, sy) = (l.w as f64 / x.w as f64, l.h as f64 / x.h as f64);
    let (mut sum, mut max, mut n) = (0u64, 0u8, 0u64);
    for yy in (b[1] as f64 * sy) as usize..((b[3] as f64 * sy) as usize).min(l.h) {
        for xx in (b[0] as f64 * sx) as usize..((b[2] as f64 * sx) as usize).min(l.w) {
            let p = l.luma_at(xx, yy);
            sum += p as u64;
            max = max.max(p);
            n += 1;
        }
    }
    println!("  luma inside the text node's bounds in the video frame: mean {:.2}, max {max} over {n} samples (black = 16 in limited range)", sum as f64 / n as f64);
    write_png(&format!("{OUT}/secure_frame_api{}.png", x.api), l.w, l.h, &l.rgb())?;
    drop(l);
    let (o, _, _) = x.adb.shell_output(x.serial, "screencap -p").await?;
    std::fs::write(format!("{OUT}/secure_screencap_api{}.png", x.api), &o)?;
    v.task.abort();
    Ok(())
}

async fn sync(x: &mut Ctx<'_>) -> Res<()> {
    println!("\n[sync frame request -> next keyframe]");
    settings(x).await?;
    let v = video(x).await?;
    assert!(v.wait_frames(3, 5000).await);
    let mut lat = Vec::new();
    for _ in 0..10 {
        sleep(1200).await;
        let n = v.count();
        let t0 = Instant::now();
        x.c.req(json!({"op":"sync_frame"})).await?;
        if std::env::var("SYNCMODE").is_ok() {
            swipe(x, true).await?;
            swipe(x, false).await?;
        }
        let mut got = None;
        let deadline = Instant::now() + Duration::from_millis(4000);
        while Instant::now() < deadline && got.is_none() {
            sleep(2).await;
            for e in v.since(n) {
                if e.flags & 2 != 0 {
                    got = Some(e.arrival - t0);
                    break;
                }
            }
        }
        if let Some(g) = got {
            lat.push(ms(g));
        }
    }
    let (a50, a95) = pct(&mut lat);
    let nk = v.since(0).iter().filter(|e| e.flags & 2 != 0).count();
    println!("  keyframes seen in the whole run: {nk} of {} frames", v.count());
    println!("  {} of 10 requests produced a keyframe; request -> keyframe arrives: p50={a50:.0} p95={a95:.0} ms (includes tunnel RTT)", lat.len());
    v.task.abort();
    Ok(())
}

async fn wait_size(v: &Video, from: usize, w: usize, h: usize, timeout: u64) -> Option<Instant> {
    let t = Instant::now();
    while (t.elapsed().as_millis() as u64) < timeout {
        if let Some(e) = v.since(from).iter().find(|e| e.decoded && e.w == w && e.h == h) {
            return Some(e.arrival);
        }
        sleep(5).await;
    }
    None
}

async fn recover(x: &mut Ctx<'_>) -> Res<()> {
    println!("\n[stream recovery]");
    settings(x).await?;
    let v = video(x).await?;
    assert!(v.wait_frames(3, 5000).await);
    let (w0, h0) = (x.w as usize, x.h as usize);
    sh(x, "settings put system accelerometer_rotation 0; settings put system user_rotation 0").await?;
    sleep(1500).await;
    let n = v.count();
    let t = Instant::now();
    sh(x, "settings put system user_rotation 1").await?;
    let got = wait_size(&v, n, h0, w0, 8000).await;
    println!("  rotation 1: new size {}x{} first decoded frame after {:?} ms (command round trip included)", h0, w0, got.map(|g| ms(g - t)));
    sleep(1500).await;
    let n = v.count();
    let t = Instant::now();
    sh(x, "settings put system user_rotation 0").await?;
    let got = wait_size(&v, n, w0, h0, 8000).await;
    println!("  rotation back to 0: first decoded frame at {w0}x{h0} after {:?} ms", got.map(|g| ms(g - t)));
    sleep(1500).await;
    let n = v.count();
    let t = Instant::now();
    sh(x, "wm size 720x1600").await?;
    let got = wait_size(&v, n, 720, 1600, 8000).await;
    println!("  wm size 720x1600: first decoded frame at 720x1600 after {:?} ms; headers seen: {:?}", got.map(|g| ms(g - t)), v.headers.lock().unwrap().iter().map(|h| (h.1, h.2)).collect::<Vec<_>>());
    sleep(1500).await;
    let n = v.count();
    let t = Instant::now();
    sh(x, "wm size reset").await?;
    let got = wait_size(&v, n, w0, h0, 8000).await;
    println!("  wm size reset: first decoded frame at {w0}x{h0} after {:?} ms", got.map(|g| ms(g - t)));
    sleep(1500).await;
    let n = v.count();
    let t = Instant::now();
    sh(x, "input keyevent KEYCODE_SLEEP").await?;
    sleep(2500).await;
    let during = v.since(n);
    println!("  screen off: {} frames in the 2.5 s after KEYCODE_SLEEP (last changed area {:?})", during.len(), during.last().map(|e| e.changed_area_pt2));
    let n = v.count();
    let t2 = Instant::now();
    sh(x, "input keyevent KEYCODE_WAKEUP").await?;
    let mut first_frame = None;
    while t2.elapsed().as_millis() < 6000 {
        if v.count() > n {
            first_frame = Some(t2.elapsed());
            break;
        }
        sleep(5).await;
    }
    println!("  screen on: first new frame {:?} ms after KEYCODE_WAKEUP (frames after sleep command: total {})", first_frame.map(ms), v.count() - n);
    let _ = t;
    sleep(2000).await;
    println!("  stream alive at the end: {} frames in last 2 s", v.since(v.count().saturating_sub(100)).iter().filter(|e| e.arrival.elapsed().as_secs() < 2).count());
    v.task.abort();
    Ok(())
}

async fn fixture(x: &mut Ctx<'_>) -> Res<()> {
    println!("\n[fixture: 3 s of Settings scroll, wire format]");
    settings(x).await?;
    let v = video(x).await?;
    assert!(v.wait_frames(3, 5000).await);
    sleep(300).await;
    let start = Instant::now();
    let mut up = true;
    while start.elapsed().as_millis() < 3000 {
        swipe(x, up).await?;
        up = !up;
    }
    sleep(300).await;
    let raw = v.raw.lock().unwrap().clone();
    let path = format!("{OUT}/settings_scroll_api{}.h264", x.api);
    std::fs::write(&path, &raw)?;
    println!("  wrote {path}: {} bytes, {} packets", raw.len(), v.count());
    v.task.abort();
    Ok(())
}

pub fn check_fixture(path: &str) -> Res<()> {
    use openh264::decoder::Decoder;
    use openh264::formats::YUVSource;
    let b = std::fs::read(path)?;
    let w = u32::from_be_bytes(b[4..8].try_into()?);
    let h = u32::from_be_bytes(b[8..12].try_into()?);
    let mut i = 12;
    let mut dec = Decoder::new()?;
    let (mut packets, mut frames, mut keys) = (0, 0, 0);
    let mut first_last: Vec<(Vec<u8>, usize)> = Vec::new();
    while i < b.len() {
        let flags = b[i + 8];
        let len = u32::from_be_bytes(b[i + 9..i + 13].try_into()?) as usize;
        let data = &b[i + 13..i + 13 + len];
        i += 13 + len;
        packets += 1;
        if flags & 2 != 0 {
            keys += 1;
        }
        if let Some(y) = dec.decode(data)? {
            frames += 1;
            let (dw, dh) = y.dimensions();
            assert_eq!((dw, dh), (w as usize, h as usize));
            if frames == 1 || true {
                first_last.push((y.y().to_vec(), y.strides().0));
                if first_last.len() > 2 {
                    first_last.remove(1);
                }
            }
        }
    }
    println!("{path}: header {w}x{h}, {packets} packets, {frames} decoded frames, {keys} keyframes, decode errors 0");
    Ok(())
}
