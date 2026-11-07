#![cfg(target_os = "macos")]

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const DEVICES: &str = "emulator-5554          device product:sdk_gphone64_x86_64 model:Pixel_9 device:emu64xa transport_id:5\n";

const PROFILE: &str = r#"(version 1)
(allow default)
(deny network-outbound (with send-signal SIGKILL))
(deny network-inbound (with send-signal SIGKILL))
(allow network-outbound (remote ip "localhost:*"))
(allow network-outbound (remote unix-socket))
(allow network-inbound (local unix-socket))"#;

struct Rig {
    home: tempfile::TempDir,
    adb: String,
}

impl Rig {
    async fn new() -> Rig {
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
                            "host:devices-l" => format!("OKAY{:04x}{DEVICES}", DEVICES.len()),
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
        Rig {
            home: tempfile::tempdir().unwrap(),
            adb,
        }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new("sandbox-exec");
        c.args(["-p", PROFILE])
            .arg(assert_cmd::cargo::cargo_bin("phonedriver"))
            .args(args)
            .env("PHONEDRIVER_HOME", self.home.path())
            .env(
                "PHONEDRIVER_SOCKET",
                self.home.path().join("run").join("d.sock"),
            )
            .env("ADB_SERVER_SOCKET", &self.adb)
            .env("USBMUXD_SOCKET_ADDRESS", "127.0.0.1:1")
            .env_remove("PHONEDRIVER_SESSION")
            .env_remove("PHONEDRIVER_DEVICE");
        c
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let socket: PathBuf = self.home.path().join("run").join("d.sock");
        if let Ok(mut s) = std::os::unix::net::UnixStream::connect(socket) {
            let _ = s.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"shutdown\"}\n");
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

/// The sandbox kills the process that breaks the rule. When that is the daemon, the client reports that it closed the
/// connection, so a clean run is an exit code and no such message.
fn killed_by_the_sandbox(o: &Output) -> bool {
    o.status.signal().is_some()
        || String::from_utf8_lossy(&o.stderr).contains("closed the connection")
}

#[test]
fn the_sandbox_kills_a_connection_to_a_remote_address() {
    let o = Command::new("sandbox-exec")
        .args([
            "-p",
            PROFILE,
            "/usr/bin/nc",
            "-z",
            "-w",
            "2",
            "192.0.2.1",
            "80",
        ])
        .output()
        .unwrap();
    assert_eq!(o.status.signal(), Some(9), "{o:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_talks_only_to_the_daemon_socket_adb_and_the_local_usbmuxd_address() {
    let rig = Rig::new().await;
    let devices = rig.run(&["devices"]);
    assert!(devices.status.success(), "{devices:?}");
    let table = String::from_utf8_lossy(&devices.stdout);
    assert!(table.contains("android:emulator-5554"), "{table}");
    for args in [
        vec!["devices", "--json"],
        vec!["snapshot"],
        vec!["apps"],
        vec!["doctor"],
        vec!["version"],
        vec!["devices"],
    ] {
        let o = rig.run(&args);
        assert!(
            !killed_by_the_sandbox(&o),
            "{args:?} opened a connection the sandbox forbids: {o:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_mcp_server_opens_no_remote_connection() {
    let rig = Rig::new().await;
    let mut child = rig
        .command(&["mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut send = |line: &str| writeln!(stdin, "{line}").unwrap();
    send(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"net-test","version":"0"}}}"#,
    );
    let mut line = String::new();
    out.read_line(&mut line).unwrap();
    assert!(line.contains("\"id\":1"), "{line}");
    send(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    send(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
    line.clear();
    out.read_line(&mut line).unwrap();
    assert!(line.contains("\"id\":2"), "{line}");
    send(
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"devices","arguments":{}}}"#,
    );
    line.clear();
    out.read_line(&mut line).unwrap();
    assert!(line.contains("emulator-5554"), "{line}");
    drop(stdin);
    let mut rest = String::new();
    let _ = out.read_to_string(&mut rest);
    let status = child.wait().unwrap();
    assert!(status.signal().is_none(), "{status:?}");
}
