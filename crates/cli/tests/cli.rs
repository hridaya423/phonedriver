#[path = "support/fakes.rs"]
mod fakes;

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
        .stdout("phonedriver 0.1.1\n");
}

#[test]
fn the_version_flag_prints_the_same_line_as_the_version_verb() {
    for flag in ["--version", "-V"] {
        Command::cargo_bin("phonedriver")
            .unwrap()
            .arg(flag)
            .assert()
            .success()
            .stdout("phonedriver 0.1.1\n");
    }
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
        Env { home, socket, adb }
    }

    fn cmd(&self) -> Command {
        let mut c = Command::cargo_bin("phonedriver").unwrap();
        c.env("PHONEDRIVER_HOME", self.home.path())
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
    let first = env.rpc("hello", json!({"version": "0.1.1"})).await;
    env.cmd().arg("devices").assert().success();
    let second = env.rpc("hello", json!({"version": "0.1.1"})).await;
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
    assert_eq!(
        lines.next(),
        Some("error: no device is connected"),
        "{stderr}"
    );
    assert!(
        lines.next().unwrap().contains("phonedriver doctor"),
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
    let pid = env.rpc("hello", json!({"version": "0.1.1"})).await["result"]["pid"]
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
    let home = tempfile::tempdir().unwrap();
    let daemon = phonedriver::daemon::Daemon::new(
        Arc::new(fakes::OneDevice(fakes::pixel_device(trees))),
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
