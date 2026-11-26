use crate::fakes;

use std::path::PathBuf;
use std::sync::Arc;

use assert_cmd::Command;
use phonedriver::client::Failure;
use phonedriver::daemon::rpc::RpcError;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

const DEVICES: &str = "emulator-5554          device product:sdk_gphone64_x86_64 model:Pixel_9 device:emu64xa transport_id:5\n";

#[test]
fn version_prints_name_and_version() {
    Command::cargo_bin("phonedriver")
        .unwrap()
        .arg("version")
        .assert()
        .success()
        .stdout("phonedriver 0.1.7\n");
}

#[test]
fn the_version_flag_prints_the_same_line_as_the_version_verb() {
    for flag in ["--version", "-V"] {
        Command::cargo_bin("phonedriver")
            .unwrap()
            .arg(flag)
            .assert()
            .success()
            .stdout("phonedriver 0.1.7\n");
    }
}

#[test]
fn compare_of_two_files_needs_no_device_and_says_why_a_pair_cannot_be_compared() {
    let dir = tempfile::tempdir().unwrap();
    let small = dir.path().join("small.png");
    std::fs::write(&small, fakes::PNG).unwrap();
    let wide = dir.path().join("wide.png");
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, 2, 1);
    encoder.set_color(png::ColorType::Grayscale);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[0, 0])
        .unwrap();
    std::fs::write(&wide, png).unwrap();
    let text = dir.path().join("notes.txt");
    std::fs::write(&text, "not a picture").unwrap();
    let boxed = dir.path().join("boxed.png");
    let compare = |a: &PathBuf, b: &PathBuf| {
        let mut c = Command::cargo_bin("phonedriver").unwrap();
        c.env("PHONEDRIVER_HOME", dir.path())
            .arg("compare")
            .arg(a)
            .arg(b)
            .arg("--image")
            .arg(&boxed);
        c.assert()
    };
    compare(&small, &small).success().stdout("same\n");
    assert!(!boxed.exists(), "nothing changed, so nothing is drawn");
    compare(&small, &wide)
        .code(1)
        .stderr("error: the pictures are 1x1 and 2x1; compare two screenshots of one device\n");
    let out = compare(&small, &text).code(1);
    let err = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(
        err.starts_with("error: the second picture cannot be read: "),
        "{err}"
    );
}

#[test]
fn no_args_prints_usage_and_exits_2() {
    let out = Command::cargo_bin("phonedriver").unwrap().assert().code(2);
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(stderr.starts_with("usage: phonedriver"), "{stderr}");
}

#[test]
fn a_secret_on_the_command_line_is_a_usage_error_and_nothing_is_typed() {
    let out = Command::cargo_bin("phonedriver")
        .unwrap()
        .args(["type", "--secret", "hunter2"])
        .assert()
        .code(2);
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("stdin") && stderr.contains("PHONEDRIVER_SECRET"),
        "{stderr}"
    );
}

#[test]
fn exit_codes_follow_the_error_kind() {
    let code = |c: i32| Failure::Rpc(RpcError::new(c, "x", Value::Null)).exit_code();
    assert_eq!(code(-32005), 3);
    assert_eq!(code(-32003), 4);
    assert_eq!(code(-32001), 4);
    for other in [
        -32002, -32004, -32006, -32007, -32011, -32012, -32013, -32014, -32015, -32016,
    ] {
        assert_eq!(code(other), 1, "{other}");
    }
    assert_eq!(Failure::Local("x".into()).exit_code(), 1);
}

struct Env {
    home: tempfile::TempDir,
    socket: PathBuf,
    adb: String,
}

impl Env {
    async fn new(devices: &'static str) -> Env {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let adb = format!("tcp:{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = listener.accept().await else {
                    return;
                };
                tokio::spawn(async move {
                    let mut len = [0u8; 4];
                    while s.read_exact(&mut len).await.is_ok() {
                        let n =
                            usize::from_str_radix(std::str::from_utf8(&len).unwrap(), 16).unwrap();
                        let mut cmd = vec![0u8; n];
                        s.read_exact(&mut cmd).await.unwrap();
                        let reply = match String::from_utf8(cmd).unwrap().as_str() {
                            "host:version" => "OKAY00040029".to_string(),
                            "host:devices-l" => format!("OKAY{:04x}{devices}", devices.len()),
                            other if other.starts_with("host:transport:") => {
                                let msg = "no fake transport";
                                format!("FAIL{:04x}{msg}", msg.len())
                            }
                            _ => "FAIL0000".into(),
                        };
                        if s.write_all(reply.as_bytes()).await.is_err() {
                            return;
                        }
                    }
                });
            }
        });
        let home = tempfile::tempdir().unwrap();
        let socket = if cfg!(windows) {
            PathBuf::from(format!(
                r"\\.\pipe\phonedriver-test-{}",
                home.path().file_name().unwrap().to_string_lossy()
            ))
        } else {
            home.path().join("run").join("d.sock")
        };
        let bin = home.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let xcrun = bin.join("xcrun");
            std::fs::write(&xcrun, "#!/bin/sh\necho '{\"devices\":{}}'\n").unwrap();
            std::fs::set_permissions(&xcrun, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        Env { home, socket, adb }
    }

    fn cmd(&self) -> Command {
        let mut c = Command::cargo_bin("phonedriver").unwrap();
        let path = std::env::join_paths(std::iter::once(self.home.path().join("bin")).chain(
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
        ))
        .unwrap();
        c.env("PHONEDRIVER_HOME", self.home.path())
            .env("PATH", path)
            .env("PHONEDRIVER_SOCKET", &self.socket)
            .env("ADB_SERVER_SOCKET", &self.adb)
            .env("USBMUXD_SOCKET_ADDRESS", "127.0.0.1:1")
            .env_remove("PHONEDRIVER_SESSION")
            .env_remove("PHONEDRIVER_DEVICE");
        c
    }

    async fn rpc(&self, method: &str, params: Value) -> Value {
        let stream = phonedriver::daemon::socket::connect(&self.socket)
            .await
            .unwrap();
        let (read, mut write) = tokio::io::split(stream);
        let req = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
        write
            .write_all(format!("{req}\n").as_bytes())
            .await
            .unwrap();
        let mut line = String::new();
        BufReader::new(read).read_line(&mut line).await.unwrap();
        serde_json::from_str(&line).unwrap()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let socket = self.socket.clone();
        let _ = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                if let Ok(stream) = phonedriver::daemon::socket::connect(&socket).await {
                    let (_r, mut w) = tokio::io::split(stream);
                    let _ = w
                        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"shutdown\"}\n")
                        .await;
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            });
        })
        .join();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn devices_starts_the_daemon_and_the_next_call_reuses_it() {
    let env = Env::new(DEVICES).await;
    let out = env.cmd().arg("devices").assert().success();
    let table = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(table.contains("android:emulator-5554"), "{table}");
    assert!(table.contains("Pixel_9"), "{table}");
    assert!(table.lines().next().unwrap().contains("TIER"), "{table}");
    let first = env.rpc("hello", json!({"version": "0.1.7"})).await;
    env.cmd().arg("devices").assert().success();
    let second = env.rpc("hello", json!({"version": "0.1.7"})).await;
    let pid = first["result"]["pid"].as_u64().unwrap();
    assert!(pid > 0);
    assert_eq!(second["result"]["pid"].as_u64().unwrap(), pid);
}

#[tokio::test(flavor = "multi_thread")]
async fn json_output_parses_and_matches_the_rpc_result() {
    let env = Env::new(DEVICES).await;
    let out = env.cmd().args(["devices", "--json"]).assert().success();
    let shown: Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    let rpc = env.rpc("devices", json!({})).await;
    assert_eq!(shown, rpc["result"]);
    assert_eq!(shown[0]["id"], "android:emulator-5554");
}

#[tokio::test(flavor = "multi_thread")]
async fn no_device_exits_4_with_the_message_then_the_hint() {
    let env = Env::new("").await;
    let out = env.cmd().arg("snapshot").assert().code(4);
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    let mut lines = stderr.lines();
    let message = lines.next().unwrap();
    let hint = lines.next().unwrap();
    assert!(
        (message == "error: no device is connected" && hint.contains("phonedriver doctor"))
            || (message.starts_with("error: no device is connected; can be started: ")
                && hint.contains("phonedriver boot")),
        "{stderr}"
    );
    assert!(out.get_output().stdout.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn any_other_failure_exits_1() {
    let env = Env::new(DEVICES).await;
    let out = env.cmd().arg("snapshot").assert().code(1);
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(stderr.starts_with("error: "), "{stderr}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_daemon_of_another_version_is_shut_down_and_replaced() {
    use interprocess::local_socket::traits::tokio::Listener as _;
    let env = Env::new(DEVICES).await;
    let listener = phonedriver::daemon::socket::listen(&env.socket)
        .await
        .unwrap();
    let (told_to_stop, stopped) = tokio::sync::oneshot::channel();
    let old = tokio::spawn(async move {
        let stream = listener.accept().await.unwrap();
        let (read, mut write) = tokio::io::split(stream);
        let mut lines = BufReader::new(read).lines();
        let mut stop = Some(told_to_stop);
        while let Ok(Some(line)) = lines.next_line().await {
            let req: Value = serde_json::from_str(&line).unwrap();
            let reply = match req["method"].as_str().unwrap() {
                "hello" => {
                    json!({"jsonrpc":"2.0","id":req["id"],"error":{"code":-32010,"message":"old","data":{"daemon_version":"0.0.1"}}})
                }
                _ => {
                    if let Some(s) = stop.take() {
                        let _ = s.send(());
                    }
                    json!({"jsonrpc":"2.0","id":req["id"],"result":{}})
                }
            };
            write
                .write_all(format!("{reply}\n").as_bytes())
                .await
                .unwrap();
            if stop.is_none() {
                break;
            }
        }
    });
    let cmd = env.cmd().arg("devices").output().unwrap();
    assert!(
        cmd.status.success(),
        "{}",
        String::from_utf8_lossy(&cmd.stderr)
    );
    stopped.await.unwrap();
    old.await.unwrap();
    let pid = env.rpc("hello", json!({"version": "0.1.7"})).await["result"]["pid"]
        .as_u64()
        .unwrap();
    assert_ne!(pid, u64::from(std::process::id()));
}

#[test]
fn skill_prints_skill_md_exactly() {
    let want =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../SKILL.md")).unwrap();
    let out = Command::cargo_bin("phonedriver")
        .unwrap()
        .arg("skill")
        .assert()
        .success();
    assert_eq!(
        String::from_utf8(out.get_output().stdout.clone()).unwrap(),
        want
    );
    assert!(want.starts_with("---\nname: phonedriver\ndescription: "));
    assert!(want.lines().count() < 150);
}

fn doctor_output(env: &Env) -> (String, Option<i32>) {
    let out = env.cmd().arg("doctor").output().unwrap();
    (String::from_utf8(out.stdout).unwrap(), out.status.code())
}

#[tokio::test(flavor = "multi_thread")]
async fn doctor_fails_the_adb_line_and_names_the_address_it_tried() {
    let mut env = Env::new("").await;
    env.adb = "tcp:127.0.0.1:1".into();
    let (out, code) = doctor_output(&env);
    let line = out.lines().find(|l| l.contains("adb server")).expect(&out);
    assert!(line.starts_with("fail"), "{out}");
    assert!(line.contains("ADB_SERVER_SOCKET=tcp:127.0.0.1:1"), "{out}");
    assert!(out.contains("fix:"), "{out}");
    assert_eq!(code, Some(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn doctor_tells_the_user_to_accept_the_usb_debugging_prompt() {
    let env = Env::new("R5CT123 unauthorized usb:1-1 transport_id:3\n").await;
    let (out, code) = doctor_output(&env);
    let adb = out.lines().find(|l| l.contains("adb server")).expect(&out);
    assert!(adb.starts_with("ok"), "{out}");
    assert!(
        adb.contains(&format!("0029 at {}", env.adb.trim_start_matches("tcp:"))),
        "{out}"
    );
    let line = out
        .lines()
        .find(|l| l.contains("android:R5CT123"))
        .expect(&out);
    assert!(
        line.starts_with("fail") && line.contains("unauthorized"),
        "{out}"
    );
    assert!(
        out.contains("accept the USB debugging prompt on the phone"),
        "{out}"
    );
    assert_eq!(code, Some(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn doctor_says_which_step_failed_when_the_agent_cannot_start() {
    let env = Env::new(DEVICES).await;
    let (out, code) = doctor_output(&env);
    assert!(out.contains("ok    android:emulator-5554"), "{out}");
    let line = out
        .lines()
        .find(|l| l.contains("agent android:emulator-5554"))
        .expect(&out);
    assert!(line.starts_with("fail"), "{out}");
    assert!(!out.contains("fix: Run `phonedriver doctor`"), "{out}");
    assert!(out.contains("daemon.log"), "{out}");
    assert!(!out.contains("video android:emulator-5554"), "{out}");
    assert_eq!(code, Some(1));
}

const TWO_DEVICES: &str = "emulator-5554          device product:sdk_gphone64_x86_64 model:Pixel_9 device:emu64xa transport_id:5\nemulator-5556          device product:sdk_gphone64_x86_64 model:Pixel_8 device:emu64xa transport_id:6\n";

#[tokio::test(flavor = "multi_thread")]
async fn doctor_probes_only_the_device_it_is_given() {
    let env = Env::new(TWO_DEVICES).await;
    for given in ["android:emulator-5556", "emulator-5556"] {
        let out = env
            .cmd()
            .args(["--device", given, "doctor"])
            .output()
            .unwrap();
        let out = String::from_utf8(out.stdout).unwrap();
        assert!(
            out.contains("agent android:emulator-5556"),
            "{given}: {out}"
        );
        assert!(!out.contains("emulator-5554"), "{given}: {out}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn doctor_names_the_devices_it_will_start_the_agent_on_before_it_does() {
    let env = Env::new(TWO_DEVICES).await;
    let out = env.cmd().arg("doctor").output().unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("android:emulator-5554, android:emulator-5556")
            && stderr.contains("--device"),
        "{stderr}"
    );
    let out = env
        .cmd()
        .args(["--device", "emulator-5556", "doctor"])
        .output()
        .unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(!stderr.contains("--device"), "{stderr}");
}

#[tokio::test(flavor = "multi_thread")]
async fn doctor_names_a_device_it_was_given_that_adb_does_not_list() {
    let env = Env::new(TWO_DEVICES).await;
    let out = env
        .cmd()
        .args(["--device", "android:emulator-9999", "doctor"])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let line = text
        .lines()
        .find(|l| l.contains("android:emulator-9999"))
        .expect(&text);
    assert!(line.starts_with("fail"), "{text}");
    assert!(text.contains("emulator-5554, emulator-5556"), "{text}");
    assert!(!text.contains("agent android:emulator-5554"), "{text}");
    assert_eq!(out.status.code(), Some(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_trace_options_can_come_from_the_environment_as_1() {
    let env = Env::new(DEVICES).await;
    env.cmd()
        .env("PHONEDRIVER_TRACE_FULL", "1")
        .env("PHONEDRIVER_TRACE_SCREENS", "1")
        .arg("devices")
        .assert()
        .success();
}

#[cfg(unix)]
#[test]
fn a_socket_that_nothing_serves_names_the_variable_in_the_error() {
    let home = tempfile::tempdir().unwrap();
    let out = Command::cargo_bin("phonedriver")
        .unwrap()
        .env("PHONEDRIVER_HOME", home.path())
        .env("PHONEDRIVER_SOCKET", "/dev/null/phonedriver.sock")
        .arg("devices")
        .assert()
        .code(1);
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("PHONEDRIVER_SOCKET=/dev/null/phonedriver.sock") && stderr.contains("ssh"),
        "{stderr}"
    );
}

async fn fake_daemon(trees: Vec<phonedriver_core::raw::RawTree>) -> (tempfile::TempDir, PathBuf) {
    fake_daemon_on(fakes::pixel_device(trees)).await
}

async fn fake_daemon_on(
    device: Arc<dyn phonedriver_core::device::Device>,
) -> (tempfile::TempDir, PathBuf) {
    let home = tempfile::tempdir().unwrap();
    let daemon = phonedriver::daemon::Daemon::new(
        Arc::new(fakes::OneDevice(device)),
        phonedriver::daemon::Config {
            home: home.path().to_path_buf(),
        },
    );
    let socket = if cfg!(windows) {
        PathBuf::from(format!(
            r"\\.\pipe\phonedriver-test-{}-served",
            home.path().file_name().unwrap().to_string_lossy()
        ))
    } else {
        home.path().join("run").join("d.sock")
    };
    let listener = phonedriver::daemon::socket::listen(&socket).await.unwrap();
    tokio::spawn(daemon.serve_on(listener));
    (home, socket)
}

async fn cli(home: &tempfile::TempDir, socket: &PathBuf, args: &[&str]) -> String {
    let mut c = Command::cargo_bin("phonedriver").unwrap();
    c.env("PHONEDRIVER_HOME", home.path())
        .env("PHONEDRIVER_SOCKET", socket)
        .env_remove("PHONEDRIVER_SESSION")
        .env_remove("PHONEDRIVER_DEVICE")
        .args(args);
    let out = tokio::task::spawn_blocking(move || c.assert().success().get_output().stdout.clone())
        .await
        .unwrap();
    String::from_utf8(out).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_cli_prints_the_device_line_once_per_session_and_again_when_it_changes() {
    let (home, socket) = fake_daemon(vec![fakes::tree(&["One", "Two"])]).await;
    let first = cli(&home, &socket, &["snapshot"]).await;
    assert!(first.starts_with("device fake:1 · "), "{first}");
    let second = cli(&home, &socket, &["snapshot"]).await;
    assert!(!second.contains("device fake:1"), "{second}");
    assert!(second.starts_with("app "), "{second}");
    assert!(second.contains("@e1 button \"One\""), "{second}");
    let other = cli(
        &home,
        &socket,
        &["--session", "other", "--share", "snapshot"],
    )
    .await;
    assert!(other.starts_with("device fake:1 · "), "{other}");
    let tapped = cli(&home, &socket, &["tap", "One"]).await;
    assert!(!tapped.contains("device fake:1"), "{tapped}");
}

#[tokio::test(flavor = "multi_thread")]
async fn json_output_keeps_the_device_line_in_every_reply() {
    let (home, socket) = fake_daemon(vec![fakes::tree(&["One", "Two"])]).await;
    for _ in 0..2 {
        let out = cli(&home, &socket, &["snapshot", "--json"]).await;
        let shown: Value = serde_json::from_str(&out).unwrap();
        assert!(
            shown["text"]
                .as_str()
                .unwrap()
                .starts_with("device fake:1 · "),
            "{out}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn replay_with_screens_keeps_each_steps_screen_then_compares_with_it() {
    let (home, socket) = fake_daemon(vec![fakes::tree(&["One", "Two"])]).await;
    let steps = home.path().join("run.steps");
    let tap = |text: &str| json!({"method": "act", "params": {"action": {"kind": "tap", "target": {"text": text}}}});
    std::fs::write(&steps, format!("{}\n{}\n", tap("One"), tap("Two"))).unwrap();
    let shots = home.path().join("shots");
    let (file, dir) = (steps.to_str().unwrap(), shots.to_str().unwrap());
    let replay = ["replay", file, "--screens", dir];

    let first = cli(&home, &socket, &replay).await;
    assert!(
        first.contains(&format!(
            "\nkept 2 screens in {dir}; a replay with --screens {dir} compares each step's screen with them\n"
        )),
        "{first}"
    );
    for n in ["01.png", "02.png"] {
        assert_eq!(std::fs::read(shots.join(n)).unwrap(), fakes::PNG, "{n}");
    }

    let again = cli(&home, &socket, &replay).await;
    assert!(
        again.contains(&format!("\n2 screens matched those in {dir}\n")),
        "{again}"
    );
    let traces: String = std::fs::read_dir(home.path().join("traces"))
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect();
    assert!(
        traces.contains("\"method\":\"act\"") && !traces.contains("\"next\""),
        "a step whose screen is compared waits for the whole screen: {traces}"
    );

    std::fs::remove_file(shots.join("01.png")).unwrap();
    let mut wide = Vec::new();
    let mut encoder = png::Encoder::new(&mut wide, 2, 1);
    encoder.set_color(png::ColorType::Grayscale);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[0, 0])
        .unwrap();
    std::fs::write(shots.join("02.png"), wide).unwrap();
    let mut c = Command::cargo_bin("phonedriver").unwrap();
    c.env("PHONEDRIVER_HOME", home.path())
        .env("PHONEDRIVER_SOCKET", &socket)
        .env_remove("PHONEDRIVER_SESSION")
        .env_remove("PHONEDRIVER_DEVICE")
        .args(replay);
    let out = tokio::task::spawn_blocking(move || c.assert().failure().get_output().clone())
        .await
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    let png2 = shots.join("02.png");
    assert!(
        err.contains(&format!(
            "step 2 of 2 (tap \"Two\") failed: its screen cannot be compared with {}: the pictures are 2x1 and 1x1",
            png2.display()
        )),
        "{err}"
    );
    assert!(err.contains("\n1. tapped @e1 \"One\""), "{err}");
    assert!(
        shots.join("01.png").exists(),
        "step 1 kept its screen again"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_audit_adds_the_screens_accessibility_problems() {
    let (home, socket) = fake_daemon(vec![fakes::tree(&["One", ""])]).await;
    let out = cli(&home, &socket, &["snapshot", "--audit"]).await;
    assert!(
        out.ends_with("\naudit: 1 finding\n@e2 button: no name a screen reader can say\n"),
        "{out}"
    );
    let (home, socket) = fake_daemon(vec![fakes::tree(&["One"])]).await;
    let out = cli(&home, &socket, &["snapshot", "--audit"]).await;
    assert!(
        out.ends_with("\naudit: no unnamed controls or small tap targets\n"),
        "{out}"
    );
}

/// Issue #43: a React Native app enlarges a touch area with `hitSlop`, which the bounds accessibility reports do not
/// show, so a small-target finding there may be wrong; the audit says so once, and only beside such a finding.
#[tokio::test(flavor = "multi_thread")]
async fn a_small_target_finding_says_its_size_is_the_reported_bounds() {
    let small = phonedriver_core::raw::RawTree::from_android_json(
        r#"{"windows":[{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}],"nodes":[
        {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.view.ViewGroup","desc":"Language","b":[100,100,269,140],"f":"vVe","a":["tap"]}]}"#,
        phonedriver_core::geometry::Scale { density: 1.0 },
    )
    .unwrap();
    let (home, socket) = fake_daemon(vec![small]).await;
    let out = cli(&home, &socket, &["snapshot", "--audit"]).await;
    assert!(
        out.ends_with(
            "\naudit: 1 finding\n@e1 button \"Language\": 169x40pt, under 44x44pt\nsizes are the bounds accessibility reports; a touch area the app enlarges (React Native `hitSlop`) is not seen\n"
        ),
        "{out}"
    );
}

async fn cli_with(
    home: &tempfile::TempDir,
    socket: &PathBuf,
    args: &[&str],
    env: &[(&str, &str)],
) -> std::process::Output {
    let mut c = Command::cargo_bin("phonedriver").unwrap();
    c.env("PHONEDRIVER_HOME", home.path())
        .env("PHONEDRIVER_SOCKET", socket)
        .env_remove("PHONEDRIVER_SESSION")
        .env_remove("PHONEDRIVER_DEVICE")
        .env_remove("PHONEDRIVER_SECRET")
        .envs(env.iter().copied())
        .args(args);
    tokio::task::spawn_blocking(move || c.output().unwrap())
        .await
        .unwrap()
}

fn everything_under(dir: &std::path::Path) -> String {
    let mut all = String::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            all += &everything_under(&path);
        } else if let Ok(text) = std::fs::read_to_string(&path) {
            all += &text;
        }
    }
    all
}

fn typing_trees(value: &str) -> Vec<phonedriver_core::raw::RawTree> {
    let mut trees = vec![fakes::field_tree("", false), fakes::field_tree("", false)];
    trees.extend(std::iter::repeat_n(fakes::field_tree(value, false), 20));
    trees
}

fn password_trees(secret: &str) -> Vec<phonedriver_core::raw::RawTree> {
    let mut trees = vec![fakes::field_tree("", true), fakes::field_tree("", true)];
    let dots = "•".repeat(secret.chars().count());
    trees.extend(std::iter::repeat_n(fakes::field_tree(&dots, true), 20));
    trees
}

fn output(out: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn a_replayed_secret_goes_only_into_a_password_or_code_field_unless_the_environment_allows_any()
 {
    let (home, socket) = fake_daemon(typing_trees("hunter4-test")).await;
    let steps = home.path().join("login.steps");
    let step = json!({"method": "act", "params": {"action": {"kind": "type", "secret": true, "secret_env": "PHONEDRIVER_SECRET_1", "into": {"text": "Field"}}}});
    std::fs::write(&steps, format!("{step}\n")).unwrap();
    let file = steps.to_str().unwrap();
    let refused = cli_with(
        &home,
        &socket,
        &["replay", file],
        &[("PHONEDRIVER_SECRET_1", "hunter4-test")],
    )
    .await;
    let shown = output(&refused);
    assert!(!refused.status.success(), "{shown}");
    assert!(
        shown.contains("only into a password or one-time code field"),
        "{shown}"
    );
    assert!(
        shown.contains("PHONEDRIVER_REPLAY_SECRET_ANY_FIELD=1"),
        "{shown}"
    );
    assert!(!shown.contains("hunter4-test"), "{shown}");
    let allowed = cli_with(
        &home,
        &socket,
        &["replay", file],
        &[
            ("PHONEDRIVER_SECRET_1", "hunter4-test"),
            ("PHONEDRIVER_REPLAY_SECRET_ANY_FIELD", "1"),
        ],
    )
    .await;
    let shown = output(&allowed);
    assert!(allowed.status.success(), "{shown}");
    assert!(!shown.contains("hunter4-test"), "{shown}");
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread")]
async fn replay_screens_masks_the_text_it_read_from_a_screen_that_shows_the_secret() {
    use std::os::unix::fs::PermissionsExt;
    let (home, socket) = fake_daemon(password_trees("hunter2-test-secret")).await;
    let steps = home.path().join("login.steps");
    let step = json!({"method": "act", "params": {"action": {"kind": "type", "secret": true, "secret_env": "PHONEDRIVER_SECRET_1", "into": {"text": "Field"}}}});
    std::fs::write(&steps, format!("{step}\n")).unwrap();
    let shots = home.path().join("shots");
    std::fs::create_dir(&shots).unwrap();
    let mut black = Vec::new();
    let mut encoder = png::Encoder::new(&mut black, 1, 1);
    encoder.set_color(png::ColorType::Rgb);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[0, 0, 0])
        .unwrap();
    std::fs::write(shots.join("01.png"), black).unwrap();
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let count = home.path().join("ocr-count");
    let tesseract = bin.join("tesseract");
    std::fs::write(
        &tesseract,
        format!(
            "#!/bin/sh\ncat > /dev/null\nn=$(cat {c} 2>/dev/null || echo 0)\nn=$((n+1))\necho $n > {c}\n[ $n -ge 2 ] && printf '5\\t1\\t1\\t1\\t1\\t1\\t0\\t0\\t3\\t3\\t96\\thunter2-test-secret\\n'\nexit 0\n",
            c = count.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&tesseract, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let out = cli_with(
        &home,
        &socket,
        &[
            "replay",
            steps.to_str().unwrap(),
            "--screens",
            shots.to_str().unwrap(),
        ],
        &[
            ("PHONEDRIVER_SECRET_1", "hunter2-test-secret"),
            ("PATH", path.to_str().unwrap()),
        ],
    )
    .await;
    let shown = output(&out);
    assert!(!out.status.success(), "{shown}");
    assert!(shown.contains("its screen is not the one in"), "{shown}");
    assert!(shown.contains("+ \"•••\""), "{shown}");
    assert!(!shown.contains("hunter2-test-secret"), "{shown}");
    let now = shots.join("01.now.png");
    assert_eq!(
        std::fs::read(&now).unwrap(),
        fakes::PNG,
        "the screen it showed is kept"
    );
    assert!(
        shown.contains(&format!(
            "hint: The screen it showed is in {}. If that one is right, put it in place of {}.",
            now.display(),
            shots.join("01.png").display()
        )),
        "{shown}"
    );
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn save_stages_its_file_under_a_fresh_name_and_follows_no_link_beside_it() {
    let (home, socket) = fake_daemon(vec![fakes::tree(&["One"])]).await;
    let dir = home.path();
    let victim = dir.join("victim.txt");
    std::fs::write(&victim, "keep me").unwrap();
    std::os::unix::fs::symlink(&victim, dir.join("run.steps.tmp")).unwrap();
    std::fs::write(dir.join("other.steps.tmp"), "someone else's").unwrap();
    for file in ["run.steps", "other.steps"] {
        cli(&home, &socket, &["snapshot"]).await;
        cli(&home, &socket, &["tap", "One"]).await;
        let path = dir.join(file);
        let saved = cli(&home, &socket, &["save", path.to_str().unwrap()]).await;
        assert!(saved.starts_with("saved 1 step to "), "{saved}");
        let meta = std::fs::symlink_metadata(&path).unwrap();
        assert!(meta.file_type().is_file(), "{file} is not a plain file");
    }
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep me");
    assert_eq!(
        std::fs::read_to_string(dir.join("other.steps.tmp")).unwrap(),
        "someone else's"
    );
    assert!(
        std::fs::symlink_metadata(dir.join("run.steps.tmp"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_replayed_step_runs_as_the_replay_and_not_with_the_share_or_device_in_the_file() {
    let (home, socket) = fake_daemon(vec![fakes::tree(&["One"])]).await;
    cli(&home, &socket, &["--session", "someone-else", "snapshot"]).await;
    let steps = home.path().join("shared.steps");
    let shell = json!({"method": "shell", "params": {"command": "echo shared", "share": true, "device": "fake:1"}});
    std::fs::write(&steps, format!("{shell}\n")).unwrap();
    let out = cli_with(&home, &socket, &["replay", steps.to_str().unwrap()], &[]).await;
    let shown = output(&out);
    assert!(!out.status.success(), "{shown}");
    assert!(
        shown.contains("is leased by session someone-else"),
        "{shown}"
    );
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread")]
async fn the_daemon_a_cli_starts_inherits_no_phonedriver_secret_variable() {
    let env = Env::new(DEVICES).await;
    env.cmd()
        .env("PHONEDRIVER_SECRET", "s-plain")
        .env("PHONEDRIVER_SECRET_1", "s-one")
        .env("PHONEDRIVER_SECRET_APP_PASSWORD", "s-named")
        .env("PHONEDRIVER_SECRETS_KEPT", "kept")
        .arg("devices")
        .assert()
        .success();
    let hello = env
        .rpc("hello", json!({"version": env!("CARGO_PKG_VERSION")}))
        .await;
    let pid = hello["result"]["pid"].as_u64().unwrap();
    let environ = std::fs::read(format!("/proc/{pid}/environ")).unwrap();
    let vars: Vec<String> = environ
        .split(|b| *b == 0)
        .map(|v| String::from_utf8_lossy(v).to_string())
        .collect();
    assert!(
        vars.iter().any(|v| v == "PHONEDRIVER_SECRETS_KEPT=kept"),
        "{vars:?}"
    );
    for v in &vars {
        assert!(
            !v.starts_with("PHONEDRIVER_SECRET=") && !v.starts_with("PHONEDRIVER_SECRET_"),
            "the daemon holds {v}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_saved_secret_names_a_variable_and_a_replay_types_what_the_environment_holds() {
    let (home, socket) = fake_daemon(typing_trees("hunter2-test")).await;
    cli(&home, &socket, &["snapshot"]).await;
    let typed = cli_with(
        &home,
        &socket,
        &["type", "--secret", "--into", "Field"],
        &[("PHONEDRIVER_SECRET", "hunter2-test")],
    )
    .await;
    assert!(typed.status.success(), "{typed:?}");
    let steps = home.path().join("login.steps");
    let file = steps.to_str().unwrap();
    let saved = cli(&home, &socket, &["save", file]).await;
    assert!(
        saved.contains("1. type secret $PHONEDRIVER_SECRET_1 into \"Field\""),
        "{saved}"
    );
    assert!(
        saved.contains("set PHONEDRIVER_SECRET_1 before replaying"),
        "{saved}"
    );
    let kept = std::fs::read_to_string(&steps).unwrap();
    assert!(
        kept.contains(r#""secret_env":"PHONEDRIVER_SECRET_1""#),
        "{kept}"
    );
    assert!(!kept.contains("hunter2-test"), "{kept}");

    let (home, socket) = fake_daemon(password_trees("hunter3-test")).await;
    let replayed = cli_with(
        &home,
        &socket,
        &["replay", file],
        &[("PHONEDRIVER_SECRET_1", "hunter3-test")],
    )
    .await;
    let shown = format!(
        "{}{}",
        String::from_utf8_lossy(&replayed.stdout),
        String::from_utf8_lossy(&replayed.stderr)
    );
    assert!(replayed.status.success(), "{shown}");
    assert!(shown.starts_with("replayed 1 step in "), "{shown}");
    assert!(!shown.contains("hunter3-test"), "{shown}");
    let traces = everything_under(&home.path().join("traces"));
    assert!(traces.contains("\"secret_len\":12"), "{traces}");
    assert!(!traces.contains("hunter3-test"), "{traces}");
    assert!(!traces.contains("hunter2-test"), "{traces}");
    assert!(!std::fs::read_to_string(&steps).unwrap().contains("hunter3"));
    assert!(!everything_under(home.path()).contains("hunter3-test"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_replay_stops_at_a_secret_whose_variable_is_not_set_and_names_it() {
    let (home, socket) = fake_daemon(typing_trees("x")).await;
    let steps = home.path().join("login.steps");
    let step = json!({"method": "act", "params": {"action": {"kind": "type", "secret": true, "secret_env": "PHONEDRIVER_SECRET_APP_PASSWORD", "into": {"text": "Field"}, "submit": false, "append": false}}});
    let tap = json!({"method": "act", "params": {"action": {"kind": "tap", "target": {"text": "Field"}}}});
    std::fs::write(&steps, format!("{tap}\n{step}\n{tap}\n")).unwrap();
    let file = steps.to_str().unwrap();
    let out = cli_with(&home, &socket, &["replay", file], &[]).await;
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success(), "{err}");
    assert!(
        err.contains("step 2 of 3 (type secret $PHONEDRIVER_SECRET_APP_PASSWORD into \"Field\") failed: PHONEDRIVER_SECRET_APP_PASSWORD is not set"),
        "{err}"
    );
    assert!(err.contains("\n1. tapped @e1 \"Field\""), "{err}");
    assert!(
        err.contains("hint: Set PHONEDRIVER_SECRET_APP_PASSWORD and replay again, or type the secret yourself with `type --secret`, then replay the same file from step 3 for the 1 step left."),
        "{err}"
    );
    let empty = cli_with(
        &home,
        &socket,
        &["replay", file],
        &[("PHONEDRIVER_SECRET_APP_PASSWORD", "")],
    )
    .await;
    assert!(String::from_utf8_lossy(&empty.stderr).contains("is not set"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_secret_may_only_come_from_a_variable_that_begins_phonedriver_secret() {
    let (home, socket) = fake_daemon(typing_trees("x")).await;
    let steps = home.path().join("login.steps");
    let step = json!({"method": "act", "params": {"action": {"kind": "type", "secret": true, "secret_env": "HOME", "into": {"text": "Field"}}}});
    std::fs::write(&steps, format!("{step}\n")).unwrap();
    let out = cli_with(
        &home,
        &socket,
        &["replay", steps.to_str().unwrap()],
        &[("HOME", "/somewhere/private")],
    )
    .await;
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success(), "{err}");
    assert!(err.contains("must begin PHONEDRIVER_SECRET_"), "{err}");
    assert!(!err.contains("/somewhere/private"), "{err}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_old_secret_step_still_stops_the_replay_for_a_person() {
    let (home, socket) = fake_daemon(typing_trees("x")).await;
    let steps = home.path().join("old.steps");
    let old = json!({"manual": "a secret was typed here; type it with `type --secret`"});
    std::fs::write(&steps, format!("{old}\n")).unwrap();
    let out = cli_with(
        &home,
        &socket,
        &["replay", steps.to_str().unwrap()],
        &[("PHONEDRIVER_SECRET_1", "hunter2-test")],
    )
    .await;
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success(), "{err}");
    assert!(
        err.contains("step 1 of 1 (a secret was typed here; type it with `type --secret`) failed"),
        "{err}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn var_fills_double_brace_names_and_run_is_new_in_every_replay() {
    let (home, socket) = fake_daemon(vec![fakes::tree(&["One", "Two"])]).await;
    let steps = home.path().join("signup.steps");
    let tap = |text: &str| json!({"method": "act", "params": {"action": {"kind": "tap", "target": {"text": text}}}});
    std::fs::write(
        &steps,
        format!("{}\n{}\n", tap("{{which}}"), tap("test+{{run}}@{{domain}}")),
    )
    .unwrap();
    let file = steps.to_str().unwrap();
    let mut runs = Vec::new();
    for _ in 0..2 {
        let out = cli_with(
            &home,
            &socket,
            &[
                "replay",
                file,
                "--var",
                "domain=example.com",
                "--var",
                "which=Two",
            ],
            &[],
        )
        .await;
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(!out.status.success(), "{err}");
        assert!(
            err.contains("step 2 of 2 (tap \"test+") && err.contains("@example.com\") failed"),
            "{err}"
        );
        assert!(err.contains("\n1. tapped @e2 \"Two\""), "{err}");
        let run = err
            .split("test+")
            .nth(1)
            .unwrap()
            .split('@')
            .next()
            .unwrap();
        assert!(
            run.len() >= 6 && run.chars().all(|c| c.is_ascii_alphanumeric()),
            "{run}"
        );
        runs.push(run.to_string());
    }
    assert_ne!(runs[0], runs[1]);
    assert!(
        std::fs::read_to_string(&steps)
            .unwrap()
            .contains("{{run}}@{{domain}}")
    );

    let typed = home.path().join("typed.steps");
    let step = json!({"method": "act", "params": {"action": {"kind": "type", "text": "u-{{run}}", "into": {"text": "Field"}}}});
    std::fs::write(&typed, format!("{step}\n")).unwrap();
    // The field shows what was typed, `{{run}}` filled in: a replay stops at a type the field did not take.
    let fake = Arc::new(phonedriver_core::testing::FakeDevice::new(vec![
        fakes::field_tree("", false),
    ]));
    fake.on_text(|t| {
        use phonedriver_core::device::TextInput;
        match t {
            TextInput::Paste(v) | TextInput::SetValue(v) | TextInput::Keystrokes(v) => {
                Some(vec![fakes::field_tree(v, false)])
            }
            _ => None,
        }
    });
    let mut info = phonedriver_core::device::Device::info(fake.as_ref()).clone();
    info.capabilities |= phonedriver_core::device::Capabilities::SCREENSHOT;
    let (home, socket) = fake_daemon_on(Arc::new(fakes::Pixels(fake, info))).await;
    let out = cli(&home, &socket, &["replay", typed.to_str().unwrap()]).await;
    let first = out.lines().next().unwrap();
    assert!(
        first.starts_with("replayed 1 step in ")
            && first.ends_with(')')
            && first.contains(" (run "),
        "{out}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_variable_nobody_gave_is_refused_before_the_first_step_and_every_one_is_named() {
    let (home, socket) = fake_daemon(typing_trees("x")).await;
    let steps = home.path().join("signup.steps");
    let tap = json!({"method": "act", "params": {"action": {"kind": "tap", "target": {"text": "Field"}}}});
    let typed = json!({"method": "act", "params": {"action": {"kind": "type", "text": "{{user}}", "into": {"text": "Field"}}}});
    let open = json!({"method": "act", "params": {"action": {"kind": "open", "url": "https://{{host}}/{{user}}/{{run}}"}}});
    let shell = json!({"method": "shell", "params": {"command": "echo {{token}}"}});
    std::fs::write(&steps, format!("{tap}\n{typed}\n{open}\n{shell}\n")).unwrap();
    let file = steps.to_str().unwrap();
    let out = cli_with(&home, &socket, &["replay", file, "--var", "token=t"], &[]).await;
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success(), "{err}");
    assert!(
        err.starts_with(&format!(
            "error: {file} needs 2 variables nobody gave: host, user\n"
        )),
        "{err}"
    );
    assert!(
        err.contains("hint: Add --var host=VALUE --var user=VALUE (MCP: vars). {{run}} is filled in by phonedriver; variables are not secrets, so they may show in replies."),
        "{err}"
    );
    assert!(!err.contains("tapped"), "{err}");
    let bad = cli_with(&home, &socket, &["replay", file, "--var", "run=1"], &[]).await;
    assert!(String::from_utf8_lossy(&bad.stderr).contains("`run` is filled in by phonedriver"));
}
