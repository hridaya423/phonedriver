use std::path::PathBuf;

use assert_cmd::Command;
use phone_use::client::Failure;
use phone_use::daemon::rpc::RpcError;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

const DEVICES: &str = "emulator-5554          device product:sdk_gphone64_x86_64 model:Pixel_9 device:emu64xa transport_id:5\n";

#[test]
fn version_prints_name_and_version() {
    Command::cargo_bin("phone-use")
        .unwrap()
        .arg("version")
        .assert()
        .success()
        .stdout("phone-use 0.1.0\n");
}

#[test]
fn no_args_prints_usage_and_exits_2() {
    let out = Command::cargo_bin("phone-use").unwrap().assert().code(2);
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(stderr.starts_with("usage: phone-use"), "{stderr}");
}

#[test]
fn a_secret_on_the_command_line_is_a_usage_error_and_nothing_is_typed() {
    let out = Command::cargo_bin("phone-use")
        .unwrap()
        .args(["type", "--secret", "hunter2"])
        .assert()
        .code(2);
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("stdin") && stderr.contains("PHONE_USE_SECRET"),
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
        -32002, -32004, -32006, -32007, -32011, -32012, -32013, -32014, -32015,
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
                r"\\.\pipe\phone-use-test-{}",
                home.path().file_name().unwrap().to_string_lossy()
            ))
        } else {
            home.path().join("run").join("d.sock")
        };
        Env { home, socket, adb }
    }

    fn cmd(&self) -> Command {
        let mut c = Command::cargo_bin("phone-use").unwrap();
        c.env("PHONE_USE_HOME", self.home.path())
            .env("PHONE_USE_SOCKET", &self.socket)
            .env("ADB_SERVER_SOCKET", &self.adb)
            .env_remove("PHONE_USE_SESSION")
            .env_remove("PHONE_USE_DEVICE");
        c
    }

    async fn rpc(&self, method: &str, params: Value) -> Value {
        let stream = phone_use::daemon::socket::connect(&self.socket)
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
                if let Ok(stream) = phone_use::daemon::socket::connect(&socket).await {
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
    let first = env.rpc("hello", json!({"version": "0.1.0"})).await;
    env.cmd().arg("devices").assert().success();
    let second = env.rpc("hello", json!({"version": "0.1.0"})).await;
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
        lines.next().unwrap().contains("phone-use devices"),
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
    let listener = phone_use::daemon::socket::listen(&env.socket)
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
    let pid = env.rpc("hello", json!({"version": "0.1.0"})).await["result"]["pid"]
        .as_u64()
        .unwrap();
    assert_ne!(pid, u64::from(std::process::id()));
}

#[test]
fn skill_prints_skill_md_exactly() {
    let want =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../SKILL.md")).unwrap();
    let out = Command::cargo_bin("phone-use")
        .unwrap()
        .arg("skill")
        .assert()
        .success();
    assert_eq!(
        String::from_utf8(out.get_output().stdout.clone()).unwrap(),
        want
    );
    assert!(want.starts_with("---\nname: phone-use\ndescription: "));
    assert!(want.lines().count() < 150);
}
