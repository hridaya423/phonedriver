//! Latency benchmark for phonedriver against architecture section 12 (Android column).
//!
//! `phonedriver-bench --phonedriver <binary> --serial emulator-5554 [--apk testbed.apk] [--runs 50] [--warmup 5] [--csv file]`
//!
//! It starts the daemon itself and calls it over its socket, the path the CLI uses, so the numbers
//! contain no process start.
//!
//! `phonedriver-bench scenario ...` runs scenario files on either platform (see `scenario.rs`);
//! `phonedriver-bench ios ...` is the iOS Simulator benchmark (see `ios.rs`).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use phonedriver::client::Client;
use phonedriver_android::adb::Adb;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};

const PACKAGE: &str = "dev.phonedriver.testbed";

struct Args {
    phonedriver: PathBuf,
    serial: String,
    apk: Option<PathBuf>,
    runs: usize,
    warmup: usize,
    csv: Option<PathBuf>,
}

fn args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let (mut phonedriver, mut serial, mut apk, mut runs, mut warmup, mut csv) =
        (None, None, None, 50, 5, None);
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--phonedriver" => phonedriver = Some(PathBuf::from(value()?)),
            "--serial" => serial = Some(value()?),
            "--apk" => apk = Some(PathBuf::from(value()?)),
            "--runs" => runs = value()?.parse().map_err(|e| format!("--runs: {e}"))?,
            "--warmup" => warmup = value()?.parse().map_err(|e| format!("--warmup: {e}"))?,
            "--csv" => csv = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args {
        phonedriver: phonedriver.ok_or("--phonedriver <binary> is required")?,
        serial: serial.ok_or("--serial <adb serial> is required")?,
        apk,
        runs,
        warmup,
        csv,
    })
}

struct Row {
    op: &'static str,
    budget_ms: Option<f64>,
    ms: Vec<f64>,
    note: &'static str,
}

impl Row {
    fn percentile(&self, p: f64) -> f64 {
        let mut v = self.ms.clone();
        v.sort_by(f64::total_cmp);
        let rank = (p * v.len() as f64).ceil() as usize;
        v[rank.saturating_sub(1).min(v.len() - 1)]
    }
}

struct Rig {
    adb: Adb,
    serial: String,
    client: Client,
    runs: usize,
    warmup: usize,
    rows: Vec<Row>,
}

impl Rig {
    async fn shell(&self, cmd: &str) -> String {
        self.adb
            .shell(&self.serial, cmd)
            .await
            .map(|o| o.stdout_text())
            .unwrap_or_default()
    }

    async fn tab(&self, name: &str) {
        self.shell(&format!(
            "am start -n {PACKAGE}/.MainActivity --es tab {name}"
        ))
        .await;
        tokio::time::sleep(Duration::from_millis(1500)).await;
    }

    async fn call(&mut self, method: &str, params: Value) -> Value {
        let mut params = params;
        params["device"] = json!(self.serial);
        match self.client.call(method, params).await {
            Ok(v) => v,
            Err(e) => panic!("{method} failed: {}", e.message()),
        }
    }

    async fn element(&mut self, role: &str, label: &str) -> Value {
        let r = self
            .call("snapshot", json!({"no_settle": true, "elements": true}))
            .await;
        r["elements"]
            .as_array()
            .and_then(|els| {
                els.iter()
                    .find(|e| e["role"] == role && (e["label"] == label || e["hint"] == label))
            })
            .cloned()
            .unwrap_or_else(|| panic!("no {role} {label:?} on screen"))
    }

    /// Times `op` `runs` times after `warmup` unrecorded calls; `between` runs outside the timer.
    async fn measure<F, B>(
        &mut self,
        name: &'static str,
        budget_ms: Option<f64>,
        note: &'static str,
        mut op: F,
        mut between: B,
    ) where
        F: AsyncFnMut(&mut Rig),
        B: AsyncFnMut(&mut Rig),
    {
        if let Ok(only) = std::env::var("PHONEDRIVER_BENCH_ONLY")
            && !only.split(',').any(|o| name.contains(o))
        {
            return;
        }
        let mut ms = Vec::with_capacity(self.runs);
        for i in 0..self.warmup + self.runs {
            let start = Instant::now();
            op(self).await;
            let took = start.elapsed().as_secs_f64() * 1000.0;
            if i >= self.warmup {
                ms.push(took);
            }
            between(self).await;
        }
        eprintln!("{name}: {} runs done", ms.len());
        self.rows.push(Row {
            op: name,
            budget_ms,
            ms,
            note,
        });
    }
}

fn mixed_text() -> String {
    "héllo 世界 مرحبا 👋🏽 abc 123 "
        .chars()
        .cycle()
        .take(100)
        .collect()
}

async fn start_daemon(
    binary: &Path,
    home: &Path,
    socket: &Path,
) -> Result<tokio::process::Child, String> {
    let mut child = tokio::process::Command::new(binary)
        .arg("daemon")
        .env("PHONEDRIVER_HOME", home)
        .env("PHONEDRIVER_SOCKET", socket)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("cannot start {}: {e}", binary.display()))?;
    let out = child.stdout.take().ok_or("no stdout")?;
    let mut line = String::new();
    BufReader::new(out)
        .read_line(&mut line)
        .await
        .map_err(|e| e.to_string())?;
    if !line.starts_with("ready") {
        return Err(format!("the daemon did not say ready: {line:?}"));
    }
    Ok(child)
}

mod ios;
mod scenario;

#[tokio::main]
async fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.first().map(String::as_str) == Some("scenario") {
        std::process::exit(scenario::main(&raw[1..]).await);
    }
    if raw.first().map(String::as_str) == Some("eval") {
        std::process::exit(scenario::eval_main(&raw[1..]).await);
    }
    if let (Some("stats"), Some(file)) = (raw.first().map(String::as_str), raw.get(1)) {
        let text = std::fs::read_to_string(file).unwrap_or_default();
        println!("{}", scenario::transcript_stats(&text));
        return;
    }
    if raw.first().map(String::as_str) == Some("ios") {
        return ios::main().await;
    }
    let args = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };
    let home = std::env::temp_dir().join(format!("phonedriver-bench-{}", std::process::id()));
    std::fs::create_dir_all(&home).expect("create the bench home");
    let socket = if cfg!(windows) {
        PathBuf::from(format!(
            r"\\.\pipe\phonedriver-bench-{}",
            std::process::id()
        ))
    } else {
        home.join("d.sock")
    };
    let _daemon = start_daemon(&args.phonedriver, &home, &socket)
        .await
        .expect("start the daemon");
    let client = Client::connect_to(&socket, "bench")
        .await
        .unwrap_or_else(|e| panic!("{}", e.message()));
    let mut rig = Rig {
        adb: Adb::from_env(),
        serial: args.serial.clone(),
        client,
        runs: args.runs,
        warmup: args.warmup,
        rows: Vec::new(),
    };
    if let Some(apk) = &args.apk {
        let bytes = std::fs::read(apk).expect("read the apk");
        rig.adb
            .push(
                &rig.serial,
                &bytes,
                "/data/local/tmp/phonedriver-testbed.apk",
                0o644,
            )
            .await
            .expect("push the apk");
        let out = rig
            .shell("pm install -r /data/local/tmp/phonedriver-testbed.apk")
            .await;
        assert!(out.contains("Success"), "{out}");
    }
    let facts = format!(
        "device {} · {} · Android API {} · adb server {}",
        rig.serial,
        rig.shell("getprop ro.product.model").await.trim(),
        rig.shell("getprop ro.build.version.sdk").await.trim(),
        rig.adb.addr()
    );

    rig.tab("taps").await;
    let first = rig.call("snapshot", json!({})).await;
    eprintln!("connected: {}", first["header"]["device"]["model"]);
    let toggle = rig.element("switch", "Dark theme").await;
    let b = &toggle["bounds"];
    let toggle_at = json!({"x": b["x"].as_f64().unwrap_or(0.0) + b["w"].as_f64().unwrap_or(0.0) / 2.0,
                           "y": b["y"].as_f64().unwrap_or(0.0) + b["h"].as_f64().unwrap_or(0.0) / 2.0});
    let b17 = rig.element("button", "b17").await["ref"].clone();

    rig.measure(
        "tap (point, request to delivered)",
        Some(15.0),
        "`act tap` with settle off, Taps tab",
        async |r: &mut Rig| {
            r.call(
                "act",
                json!({"action": {"kind": "tap", "target": {"point": {"x": 120, "y": 300}}}, "settle": false}),
            )
            .await;
        },
        async |_: &mut Rig| {},
    )
    .await;
    rig.measure(
        "tap (ref, request to delivered)",
        None,
        "adds one tree read to resolve the ref",
        async |r: &mut Rig| {
            let t = b17.clone();
            r.call(
                "act",
                json!({"action": {"kind": "tap", "target": {"ref": t}}, "settle": false}),
            )
            .await;
        },
        async |_: &mut Rig| {},
    )
    .await;
    rig.measure(
        "screenshot (settled Taps tab)",
        Some(30.0),
        "latest frame to PNG at 1 px per point, written to disk",
        async |r: &mut Rig| {
            r.call("screenshot", json!({})).await;
        },
        async |_: &mut Rig| {},
    )
    .await;
    rig.measure(
        "tap to settled diff (toggle, point)",
        Some(600.0),
        "`act tap` on the Dark theme switch, waits for settle, returns the diff",
        async |r: &mut Rig| {
            let p = toggle_at.clone();
            let out = r
                .call(
                    "act",
                    json!({"action": {"kind": "tap", "target": {"point": p}}, "settle": true}),
                )
                .await;
            assert_eq!(out["changed"], true, "{out}");
        },
        async |_: &mut Rig| {
            tokio::time::sleep(Duration::from_millis(300)).await;
        },
    )
    .await;

    rig.shell("am force-stop com.android.settings").await;
    rig.shell("am start -a android.settings.SETTINGS").await;
    tokio::time::sleep(Duration::from_millis(2500)).await;
    rig.measure(
        "snapshot (Settings main, no settle)",
        Some(80.0),
        "cold tree read, prune, refs, render",
        async |r: &mut Rig| {
            r.call("snapshot", json!({"no_settle": true})).await;
        },
        async |_: &mut Rig| {},
    )
    .await;
    rig.measure(
        "snapshot (Settings main, settled)",
        None,
        "as an agent calls it: waits for quiet first",
        async |r: &mut Rig| {
            r.call("snapshot", json!({})).await;
        },
        async |_: &mut Rig| {},
    )
    .await;

    rig.tab("list").await;
    rig.measure(
        "snapshot (List tab, 1000 rows, no settle)",
        None,
        "visible rows only; the tree holds what the RecyclerView has bound",
        async |r: &mut Rig| {
            r.call("snapshot", json!({"no_settle": true})).await;
        },
        async |_: &mut Rig| {},
    )
    .await;

    rig.tab("text").await;
    let field = rig.element("text_field", "plain").await;
    let at = &field["bounds"];
    let (x, y) = (
        at["x"].as_f64().unwrap_or(0.0) + at["w"].as_f64().unwrap_or(0.0) / 2.0,
        at["y"].as_f64().unwrap_or(0.0) + at["h"].as_f64().unwrap_or(0.0) / 2.0,
    );
    rig.call(
        "act",
        json!({"action": {"kind": "tap", "target": {"point": {"x": x, "y": y}}}}),
    )
    .await;
    rig.call("snapshot", json!({})).await;
    let text = mixed_text();
    rig.measure(
        "type 100 chars (ASCII, accents, CJK, Arabic, emoji)",
        Some(150.0),
        "`act type` paste into the focused field, settle off",
        async |r: &mut Rig| {
            let t = text.clone();
            r.call(
                "act",
                json!({"action": {"kind": "type", "text": t}, "settle": false}),
            )
            .await;
        },
        async |r: &mut Rig| {
            r.call(
                "act",
                json!({"action": {"kind": "type", "text": "", "mode": "set"}, "settle": false}),
            )
            .await;
        },
    )
    .await;

    println!("# phonedriver latency\n");
    println!("{facts}\n");
    println!(
        "{} runs per op after {} warm-ups, through the daemon socket.\n",
        args.runs, args.warmup
    );
    println!("| op | runs | p50 ms | p95 ms | max ms | budget p50 ms | met | what limits it |");
    println!("|---|---|---|---|---|---|---|---|");
    for r in &rig.rows {
        let (budget, met) = match r.budget_ms {
            Some(b) => (
                format!("{b}"),
                if r.percentile(0.5) <= b { "yes" } else { "NO" },
            ),
            None => ("-".into(), "-"),
        };
        println!(
            "| {} | {} | {:.1} | {:.1} | {:.1} | {budget} | {met} | {} |",
            r.op,
            r.ms.len(),
            r.percentile(0.5),
            r.percentile(0.95),
            r.percentile(1.0),
            r.note
        );
    }
    if let Some(csv) = &args.csv {
        let mut out = String::from("op,ms\n");
        for r in &rig.rows {
            for v in &r.ms {
                out.push_str(&format!("\"{}\",{v:.3}\n", r.op));
            }
        }
        std::fs::write(csv, out).expect("write the csv");
    }
    rig.call("release", json!({})).await;
}
