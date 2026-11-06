//! Latency benchmark for the iOS Simulator tier against architecture section 12 (Runner / Simulator column) and the
//! runner-against-WebDriverAgent check of decision D4.
//!
//! `phone-use-bench ios --phone-use <binary> --device sim:<udid> [--runs 50] [--warmup 5] [--csv file]`
//!
//! The runner rows talk to the runner's socket directly, before the daemon starts; the other rows go through the
//! daemon socket, as the Android benchmark does. The testbed must be installed on the simulator.

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use phone_use::client::Client;
use phone_use_core::error::Op;
use phone_use_ios::runner::Runner;
use serde_json::{Value, json};

use super::{Row, mixed_text, start_daemon};

const TESTBED: &str = "dev.phoneuse.testbed";
const SETTINGS: &str = "com.apple.Preferences";

struct Args {
    phone_use: PathBuf,
    udid: String,
    physical: bool,
    runs: usize,
    warmup: usize,
    csv: Option<PathBuf>,
}

fn args() -> Result<Args, String> {
    let mut it = std::env::args().skip(2);
    let (mut phone_use, mut udid, mut runs, mut warmup, mut csv) = (None, None, 50, 5, None);
    let mut physical = false;
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--phone-use" => phone_use = Some(PathBuf::from(value()?)),
            "--device" => {
                let v = value()?;
                physical = v.starts_with("ios:");
                udid = Some(
                    v.strip_prefix("sim:")
                        .or_else(|| v.strip_prefix("ios:"))
                        .unwrap_or(&v)
                        .to_string(),
                );
            }
            "--runs" => runs = value()?.parse().map_err(|e| format!("--runs: {e}"))?,
            "--warmup" => warmup = value()?.parse().map_err(|e| format!("--warmup: {e}"))?,
            "--csv" => csv = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args {
        phone_use: phone_use.ok_or("--phone-use <binary> is required")?,
        udid: udid.ok_or("--device sim:<udid> or ios:<udid> is required")?,
        physical,
        runs,
        warmup,
        csv,
    })
}

fn simctl(args: &[&str]) -> String {
    let out = Command::new("xcrun")
        .arg("simctl")
        .args(args)
        .output()
        .expect("run xcrun simctl");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Starts `app` fresh, with `extra` launch arguments, and gives it time to draw its first screen.
async fn fresh((udid, physical): (&str, bool), app: &str, extra: &[&str]) {
    if physical {
        let mut launch = vec!["devicectl", "device", "process", "launch", "--device", udid];
        launch.extend(["--terminate-existing", app]);
        if !extra.is_empty() {
            launch.push("--");
            launch.extend_from_slice(extra);
        }
        Command::new("xcrun")
            .args(launch)
            .output()
            .expect("run xcrun devicectl");
        tokio::time::sleep(Duration::from_millis(2500)).await;
        return;
    }
    simctl(&["terminate", udid, app]);
    let mut args = vec!["launch", udid, app];
    args.extend_from_slice(extra);
    simctl(&args);
    tokio::time::sleep(Duration::from_millis(2500)).await;
}

/// Times `op` `runs` times after `warmup` unrecorded calls; `between` runs outside the timer.
async fn measure<S, F, B>(
    state: &mut S,
    rows: &mut Vec<Row>,
    (runs, warmup): (usize, usize),
    (name, budget_ms, note): (&'static str, Option<f64>, &'static str),
    mut op: F,
    mut between: B,
) where
    F: AsyncFnMut(&mut S),
    B: AsyncFnMut(&mut S),
{
    let mut ms = Vec::with_capacity(runs);
    for i in 0..warmup + runs {
        let start = Instant::now();
        op(state).await;
        let took = start.elapsed().as_secs_f64() * 1000.0;
        if i >= warmup {
            ms.push(took);
        }
        between(state).await;
    }
    eprintln!("{name}: {} runs done", ms.len());
    rows.push(Row {
        op: name,
        budget_ms,
        ms,
        note,
    });
}

async fn runner_rows(args: &Args, home: &std::path::Path, rows: &mut Vec<Row>) -> String {
    let mut runner = Runner::start(&args.udid, home)
        .await
        .unwrap_or_else(|e| panic!("start the runner: {e:?}"));
    let h = &runner.hello;
    let facts = format!(
        "{} · iOS {} · {}x{} pt @{}x · runner {} build {}",
        h.model,
        h.ios,
        h.screen.w,
        h.screen.h,
        h.screen.scale,
        h.version,
        h.build.clone().unwrap_or_default()
    );
    let counts = (args.runs, args.warmup);
    let snapshot = async |r: &mut Runner| {
        let reply = r
            .client
            .request(Op::Tree, json!({"op": "snapshot", "max_depth": 30}))
            .await
            .expect("runner snapshot");
        assert!(reply["nodes"].as_array().is_some_and(|n| !n.is_empty()));
    };
    let none = async |_: &mut Runner| {};
    fresh((&args.udid, args.physical), TESTBED, &["-tab", "list"]).await;
    measure(
        &mut runner,
        rows,
        counts,
        (
            "runner snapshot, List tab (D4)",
            None,
            "runner socket, depth 30; tuned WDA p50 44.8 ms (task 5.3)",
        ),
        snapshot,
        none,
    )
    .await;
    fresh((&args.udid, args.physical), TESTBED, &["-tab", "taps"]).await;
    measure(
        &mut runner,
        rows,
        counts,
        (
            "runner snapshot, Taps tab (D4)",
            None,
            "runner socket, depth 30; tuned WDA p50 26.1 ms (task 5.3)",
        ),
        snapshot,
        none,
    )
    .await;
    fresh((&args.udid, args.physical), SETTINGS, &[]).await;
    measure(
        &mut runner,
        rows,
        counts,
        (
            "runner snapshot, Settings main (D4)",
            None,
            "runner socket, depth 30; tuned WDA p50 49.7 ms (task 5.3)",
        ),
        snapshot,
        none,
    )
    .await;
    measure(
        &mut runner,
        rows,
        counts,
        (
            "runner screenshot, luma at 0.25 px/pt (settle poll)",
            Some(300.0),
            "plan 5.6 threshold; sets the settle poll rate",
        ),
        async |r: &mut Runner| {
            r.client
                .request(
                    Op::Screenshot,
                    json!({"op": "screenshot", "format": "luma", "scale": 0.25}),
                )
                .await
                .expect("runner luma");
        },
        none,
    )
    .await;
    measure(
        &mut runner,
        rows,
        counts,
        (
            "runner screenshot, PNG at 1 px/pt",
            None,
            "XCTest capture is 61 ms of it",
        ),
        async |r: &mut Runner| {
            r.client
                .request(
                    Op::Screenshot,
                    json!({"op": "screenshot", "format": "png", "scale": 1}),
                )
                .await
                .expect("runner png");
        },
        none,
    )
    .await;
    facts
}

struct Rig {
    client: Client,
    device: String,
    udid: String,
}

impl Rig {
    async fn call(&mut self, method: &str, params: Value) -> Value {
        let mut params = params;
        params["device"] = json!(self.device);
        match self.client.call(method, params).await {
            Ok(v) => v,
            Err(e) => panic!("{method} failed: {}", e.message()),
        }
    }

    async fn element(&mut self, id: &str) -> Value {
        let r = self
            .call("snapshot", json!({"no_settle": true, "elements": true}))
            .await;
        r["elements"]
            .as_array()
            .and_then(|els| els.iter().find(|e| e["id"] == id))
            .cloned()
            .unwrap_or_else(|| panic!("no element {id} on screen"))
    }

    async fn center(&mut self, id: &str) -> Value {
        let b = self.element(id).await["bounds"].clone();
        let f = |k: &str| b[k].as_f64().unwrap_or(0.0);
        json!({"x": f("x") + f("w") / 2.0, "y": f("y") + f("h") / 2.0})
    }

    async fn tap(&mut self, at: Value, settle: bool) -> Value {
        self.call(
            "act",
            json!({"action": {"kind": "tap", "target": {"point": at}}, "settle": settle}),
        )
        .await
    }

    async fn plain_field(&mut self) {
        fresh(
            (&self.udid, self.device.starts_with("ios:")),
            TESTBED,
            &["-tab", "text"],
        )
        .await;
        let at = self.center("text.plain").await;
        self.tap(at, true).await;
    }
}

async fn daemon_rows(args: &Args, home: &std::path::Path, rows: &mut Vec<Row>) {
    let socket = home.join("d.sock");
    let _daemon = start_daemon(&args.phone_use, home, &socket)
        .await
        .expect("start the daemon");
    let client = Client::connect_to(&socket, "bench")
        .await
        .unwrap_or_else(|e| panic!("{}", e.message()));
    let mut rig = Rig {
        client,
        device: format!(
            "{}:{}",
            if args.physical { "ios" } else { "sim" },
            args.udid
        ),
        udid: args.udid.clone(),
    };
    let counts = (args.runs, args.warmup);
    let none = async |_: &mut Rig| {};

    fresh((&args.udid, args.physical), TESTBED, &["-tab", "taps"]).await;
    rig.call("snapshot", json!({})).await;
    let point = rig.center("taps.b5").await;
    let b17 = rig.element("taps.b17").await["ref"].clone();
    let toggle = rig.center("taps.dark").await;
    measure(&mut rig, rows, counts,
        ("tap at a point, settle off (Taps tab)", Some(80.0), "`act tap`, settle off; XCTest confirms each event about 230 ms after the app has it (D15)"),
        async |r: &mut Rig| {
            let p = point.clone();
            r.tap(p, false).await;
        },
        none).await;
    rig.call("snapshot", json!({})).await;
    let b17 = rig.element("taps.b17").await["ref"].clone();
    measure(
        &mut rig,
        rows,
        counts,
        (
            "tap on a ref, settle off",
            None,
            "adds one tree read to resolve the ref",
        ),
        async |r: &mut Rig| {
            let t = b17.clone();
            r.call(
                "act",
                json!({"action": {"kind": "tap", "target": {"ref": t}}, "settle": false}),
            )
            .await;
        },
        none,
    )
    .await;
    measure(
        &mut rig,
        rows,
        counts,
        (
            "screenshot, settled Taps tab",
            Some(400.0),
            "runner PNG at 1 px per pt, written to disk",
        ),
        async |r: &mut Rig| {
            r.call("screenshot", json!({})).await;
        },
        none,
    )
    .await;
    measure(
        &mut rig,
        rows,
        counts,
        (
            "tap to settled diff (Dark theme toggle)",
            Some(1200.0),
            "`act tap` on the testbed toggle, waits for settle, returns the diff",
        ),
        async |r: &mut Rig| {
            let p = toggle.clone();
            let out = r.tap(p, true).await;
            if out["changed"] != true {
                eprintln!("no diff after the toggle tap: {}", out["settle_ms"]);
            }
        },
        async |_: &mut Rig| tokio::time::sleep(Duration::from_millis(300)).await,
    )
    .await;

    fresh((&args.udid, args.physical), SETTINGS, &[]).await;
    measure(
        &mut rig,
        rows,
        counts,
        (
            "snapshot, Settings main, --no-settle",
            Some(300.0),
            "runner tree, prune, refs, render",
        ),
        async |r: &mut Rig| {
            r.call("snapshot", json!({"no_settle": true})).await;
        },
        none,
    )
    .await;
    measure(
        &mut rig,
        rows,
        counts,
        (
            "snapshot, Settings main, settled",
            None,
            "as an agent calls it: waits for quiet first",
        ),
        async |r: &mut Rig| {
            r.call("snapshot", json!({})).await;
        },
        none,
    )
    .await;

    fresh((&args.udid, args.physical), TESTBED, &["-tab", "list"]).await;
    measure(
        &mut rig,
        rows,
        counts,
        (
            "snapshot, List tab (1,000 rows), --no-settle",
            None,
            "visible rows only",
        ),
        async |r: &mut Rig| {
            r.call("snapshot", json!({"no_settle": true})).await;
        },
        none,
    )
    .await;

    rig.plain_field().await;
    let text = mixed_text();
    measure(
        &mut rig,
        rows,
        counts,
        (
            "type 100 mixed characters, settle off",
            Some(400.0),
            "`act type` paste mode: runner typing at 5,000 chars/s (D15)",
        ),
        async |r: &mut Rig| {
            let t = text.clone();
            r.call(
                "act",
                json!({"action": {"kind": "type", "text": t}, "settle": false}),
            )
            .await;
        },
        async |r: &mut Rig| r.plain_field().await,
    )
    .await;
    rig.call("release", json!({})).await;
}

pub async fn main() {
    let args = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };
    let home = std::env::var_os("PHONE_USE_BENCH_HOME").map_or_else(
        || std::env::temp_dir().join(format!("phone-use-bench-{}", std::process::id())),
        std::path::PathBuf::from,
    );
    std::fs::create_dir_all(home.join("runner")).expect("create the bench home");
    let mut rows = Vec::new();
    let facts = if args.physical {
        "physical device, Runner tier".to_string()
    } else {
        runner_rows(&args, &home, &mut rows).await
    };
    daemon_rows(&args, &home, &mut rows).await;

    println!(
        "# phone-use latency, iOS {}\n",
        if args.physical { "device" } else { "Simulator" }
    );
    println!("{} · {facts}\n", args.udid);
    println!(
        "{} runs per op after {} warm-ups.\n",
        args.runs, args.warmup
    );
    println!("| op | runs | p50 ms | p95 ms | max ms | budget p50 ms | met | notes |");
    println!("|---|---|---|---|---|---|---|---|");
    for r in &rows {
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
        for r in &rows {
            for v in &r.ms {
                out.push_str(&format!("\"{}\",{v:.3}\n", r.op));
            }
        }
        std::fs::write(csv, out).expect("write the csv");
    }
}
