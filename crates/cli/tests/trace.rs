#[path = "support/fakes.rs"]
mod fakes;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use fakes::{OneDevice, field_tree, pixel_device, tree};
use phone_use::daemon::trace::TraceOptions;
use phone_use::daemon::{Config, Daemon};
use phone_use_core::raw::RawTree;
use proptest::prelude::*;
use serde_json::{Value, json};

fn daemon(trees: Vec<RawTree>, trace: TraceOptions, home: &Path) -> Arc<Daemon> {
    Daemon::with_trace(
        Arc::new(OneDevice(pixel_device(trees))),
        Config {
            home: home.to_path_buf(),
        },
        trace,
    )
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(files(&p));
        } else {
            out.push(p);
        }
    }
    out.sort();
    out
}

fn lines(home: &Path) -> Vec<Value> {
    let traces: Vec<_> = files(&home.join("traces"))
        .into_iter()
        .filter(|f| f.extension().is_some_and(|e| e == "jsonl"))
        .collect();
    assert_eq!(traces.len(), 1, "one trace per session: {traces:?}");
    std::fs::read_to_string(&traces[0])
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn type_secret(text: &str, flag: bool) -> Value {
    json!({"action": {"kind": "type", "text": text, "into": {"ref": "e1"}, "secret": flag}})
}

fn run<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
        .block_on(f)
}

#[test]
fn each_request_is_one_line_with_method_params_the_reply_summary_and_ms() {
    run(async {
        let home = tempfile::tempdir().unwrap();
        let d = daemon(
            vec![field_tree("", false), field_tree("abcdefg", false)],
            TraceOptions::default(),
            home.path(),
        );
        d.call("s", "snapshot", json!({"interactive": true}))
            .await
            .unwrap();
        d.call("s", "act", type_secret("abcdefg", true))
            .await
            .unwrap();
        let e = d
            .call(
                "s",
                "act",
                json!({"action": {"kind": "tap", "target": {"ref": "e99"}}}),
            )
            .await
            .unwrap_err();
        let l = lines(home.path());
        assert_eq!(l.len(), 3, "{l:?}");
        assert_eq!(l[0]["method"], "snapshot");
        assert_eq!(l[0]["params"]["interactive"], true);
        assert_eq!(l[1]["method"], "act");
        assert_eq!(l[1]["params"]["action"]["text"], json!({"secret_len": 7}));
        assert_eq!(l[1]["params"]["action"]["into"], json!({"ref": "e1"}));
        assert_eq!(l[1]["reply"], "typed 7 chars into @e1");
        assert_eq!(l[1]["ok"], true);
        assert_eq!(l[2]["ok"], false);
        assert_eq!(l[2]["reply"], e.message);
        for line in &l {
            assert!(line["ms"].is_u64(), "{line}");
            assert_eq!(line["session"], "s");
        }
    });
}

#[test]
fn typed_text_that_is_not_flagged_secret_is_still_recorded_by_length_only() {
    run(async {
        let home = tempfile::tempdir().unwrap();
        let d = daemon(
            vec![field_tree("", false), field_tree("plain words", false)],
            TraceOptions::default(),
            home.path(),
        );
        d.call("s", "snapshot", json!({})).await.unwrap();
        d.call("s", "act", type_secret("plain words", false))
            .await
            .unwrap();
        let all = std::fs::read_to_string(&files(&home.path().join("traces"))[0]).unwrap();
        assert!(!all.contains("plain words"), "{all}");
        assert!(all.contains("\"text_len\":11"), "{all}");
    });
}

fn every_file_is_free_of(home: &Path, secret: &str) {
    for f in files(home) {
        if f.extension().is_some_and(|e| e == "sock" || e == "png") {
            continue;
        }
        let text = std::fs::read(&f).unwrap();
        assert!(
            !String::from_utf8_lossy(&text).contains(secret),
            "{} holds {secret}",
            f.display()
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn a_secret_never_reaches_a_trace_flagged_or_typed_into_a_secure_field(
        secret in "[A-Za-z0-9]{12}", via_flag in any::<bool>(), full in any::<bool>(),
    ) {
        run(async {
            let home = tempfile::tempdir().unwrap();
            let masked = "•".repeat(12);
            let (before, after) = if via_flag {
                (field_tree("", false), field_tree(&secret, false))
            } else {
                (field_tree("", true), field_tree(&masked, true))
            };
            let d = daemon(
                vec![before, after],
                TraceOptions { full, screens: false },
                home.path(),
            );
            d.call("s", "snapshot", json!({})).await.unwrap();
            let _ = d.call("s", "act", type_secret(&secret, via_flag)).await;
            let _ = d.call("s", "act", type_secret(&secret, via_flag)).await;
            let _ = d.call("s", "clipboard", json!({"set": secret})).await;
            every_file_is_free_of(home.path(), &secret);
        });
    }
}

#[test]
fn a_shell_command_is_traced_but_a_secret_one_is_only_a_length_and_output_is_only_a_length() {
    run(async {
        let home = tempfile::tempdir().unwrap();
        let d = daemon(vec![tree(&["One"])], TraceOptions::default(), home.path());
        d.call(
            "s",
            "shell",
            json!({"command": "settings get system font_scale"}),
        )
        .await
        .unwrap();
        d.call(
            "s",
            "shell",
            json!({"command": "cmd wifi connect-network x wpa2 hunter2hunter2", "secret": true}),
        )
        .await
        .unwrap();
        let all = lines(home.path());
        assert_eq!(
            all[0]["params"]["command"],
            "settings get system font_scale"
        );
        assert_eq!(all[1]["params"]["command"], json!({"secret_len": 46}));
        assert!(all[0]["reply"].as_str().unwrap().starts_with("exit 0, "));
        every_file_is_free_of(home.path(), "hunter2hunter2");
    });
}

#[test]
fn observation_text_and_text_targets_appear_only_with_trace_full() {
    run(async {
        for full in [false, true] {
            let home = tempfile::tempdir().unwrap();
            let d = daemon(
                vec![tree(&["Open", "Balance 4,512"])],
                TraceOptions {
                    full,
                    screens: false,
                },
                home.path(),
            );
            d.call("s", "snapshot", json!({})).await.unwrap();
            d.call(
                "s",
                "act",
                json!({"action": {"kind": "tap", "target": {"text": "Open"}}}),
            )
            .await
            .unwrap();
            let all = std::fs::read_to_string(&files(&home.path().join("traces"))[0]).unwrap();
            assert_eq!(all.contains("Balance 4,512"), full, "full={full}: {all}");
            assert_eq!(all.contains("\"text\":\"Open\""), full, "{all}");
            if !full {
                assert!(all.contains("\"text_len\":4"), "{all}");
            }
        }
    });
}

#[test]
fn trace_screens_copies_each_screenshot_next_to_the_trace_and_names_it() {
    run(async {
        for screens in [false, true] {
            let home = tempfile::tempdir().unwrap();
            let d = daemon(
                vec![tree(&["One"])],
                TraceOptions {
                    full: false,
                    screens,
                },
                home.path(),
            );
            d.call("s", "screenshot", json!({})).await.unwrap();
            let pngs: Vec<_> = files(&home.path().join("traces"))
                .into_iter()
                .filter(|f| f.extension().is_some_and(|e| e == "png"))
                .collect();
            if screens {
                assert_eq!(pngs.len(), 1);
                let l = lines(home.path());
                assert_eq!(
                    l[0]["screen"],
                    pngs[0].file_name().unwrap().to_str().unwrap()
                );
                assert_eq!(std::fs::read(&pngs[0]).unwrap(), fakes::PNG);
            } else {
                assert!(pngs.is_empty());
                assert!(lines(home.path())[0].get("screen").is_none());
            }
        }
    });
}

#[cfg(unix)]
#[test]
fn the_trace_directory_is_private_to_its_owner() {
    use std::os::unix::fs::PermissionsExt;
    run(async {
        let home = tempfile::tempdir().unwrap();
        let d = daemon(vec![tree(&["One"])], TraceOptions::default(), home.path());
        d.call("s", "snapshot", json!({})).await.unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        let dir = home.path().join("traces");
        assert_eq!(mode(&dir), 0o700);
        assert_eq!(mode(&files(&dir)[0]), 0o600);
    });
}

#[test]
fn a_trace_file_eight_days_old_is_deleted_on_daemon_start_and_one_of_six_days_stays() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("traces");
    std::fs::create_dir_all(&dir).unwrap();
    let age = |name: &str, days: u64| {
        let f = std::fs::File::create(dir.join(name)).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(days * 24 * 3600))
            .unwrap();
    };
    age("old.jsonl", 8);
    age("old-1.png", 8);
    age("recent.jsonl", 6);
    run(async {
        let _ = daemon(vec![tree(&["One"])], TraceOptions::default(), home.path());
    });
    let left: Vec<_> = files(&dir)
        .iter()
        .map(|f| f.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left, vec!["recent.jsonl"]);
}
