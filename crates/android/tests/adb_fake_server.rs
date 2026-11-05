use std::sync::{Arc, Mutex};

use phone_use_android::adb::{Adb, DeviceState, ShellPacket};
use phone_use_core::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

const DEVICES: &str = "emulator-5554          device product:sdk_gphone64_x86_64 model:sdk_gphone64_x86_64 device:emu64xa transport_id:5\nR5CT1234               unauthorized transport_id:7\n";

#[derive(Default)]
struct Recorded {
    pushes: Vec<(String, Vec<usize>, Vec<u8>)>,
    requests: Vec<String>,
}

struct FakeAdb {
    addr: String,
    recorded: Arc<Mutex<Recorded>>,
    updates: broadcast::Sender<String>,
}

async fn read_request(s: &mut TcpStream) -> Option<String> {
    let mut len = [0u8; 4];
    s.read_exact(&mut len).await.ok()?;
    let n = usize::from_str_radix(std::str::from_utf8(&len).ok()?, 16).ok()?;
    let mut buf = vec![0u8; n];
    s.read_exact(&mut buf).await.ok()?;
    String::from_utf8(buf).ok()
}

async fn send_text(s: &mut TcpStream, text: &str) {
    s.write_all(format!("OKAY{:04x}{text}", text.len()).as_bytes())
        .await
        .unwrap();
}

async fn send_packet(s: &mut TcpStream, id: u8, data: &[u8]) {
    let mut m = vec![id];
    m.extend_from_slice(&(data.len() as u32).to_le_bytes());
    m.extend_from_slice(data);
    s.write_all(&m).await.unwrap();
}

async fn serve_sync(s: &mut TcpStream, recorded: &Mutex<Recorded>) {
    let mut path = String::new();
    let mut chunks = Vec::new();
    let mut data = Vec::new();
    loop {
        let mut head = [0u8; 8];
        if s.read_exact(&mut head).await.is_err() {
            return;
        }
        let n = u32::from_le_bytes(head[4..8].try_into().unwrap()) as usize;
        match &head[..4] {
            b"SEND" => {
                let mut spec = vec![0u8; n];
                s.read_exact(&mut spec).await.unwrap();
                path = String::from_utf8(spec).unwrap();
            }
            b"DATA" => {
                let mut chunk = vec![0u8; n];
                s.read_exact(&mut chunk).await.unwrap();
                chunks.push(n);
                data.extend(chunk);
            }
            b"DONE" => {
                recorded
                    .lock()
                    .unwrap()
                    .pushes
                    .push((path.clone(), chunks.clone(), data.clone()));
                s.write_all(b"OKAY\0\0\0\0").await.unwrap();
            }
            _ => return,
        }
    }
}

async fn serve(
    mut s: TcpStream,
    recorded: Arc<Mutex<Recorded>>,
    updates: broadcast::Sender<String>,
) {
    let mut sub = updates.subscribe();
    while let Some(req) = read_request(&mut s).await {
        recorded.lock().unwrap().requests.push(req.clone());
        match req.as_str() {
            "host:version" => {
                s.write_all(b"OKAY").await.unwrap();
                s.write_all(b"00040029").await.unwrap();
            }
            "host:devices-l" => send_text(&mut s, DEVICES).await,
            "host:track-devices-l" => {
                send_text(&mut s, DEVICES).await;
                while let Ok(next) = sub.recv().await {
                    if s.write_all(format!("{:04x}{next}", next.len()).as_bytes())
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                return;
            }
            "host:transport:nope" => {
                let msg = "device 'nope' not found";
                s.write_all(format!("FAIL{:04x}{msg}", msg.len()).as_bytes())
                    .await
                    .unwrap();
                return;
            }
            "sync:" => {
                s.write_all(b"OKAY").await.unwrap();
                serve_sync(&mut s, &recorded).await;
                return;
            }
            r if r.starts_with("host:transport:") => s.write_all(b"OKAY").await.unwrap(),
            "shell,v2,raw:echo hi" => {
                s.write_all(b"OKAY").await.unwrap();
                send_packet(&mut s, 1, b"hi\n").await;
                send_packet(&mut s, 2, b"warn\n").await;
                send_packet(&mut s, 3, &[7]).await;
                return;
            }
            "shell,v2,raw:cat" => {
                s.write_all(b"OKAY").await.unwrap();
                loop {
                    let mut head = [0u8; 5];
                    if s.read_exact(&mut head).await.is_err() {
                        return;
                    }
                    let n = u32::from_le_bytes(head[1..5].try_into().unwrap()) as usize;
                    let mut body = vec![0u8; n];
                    s.read_exact(&mut body).await.unwrap();
                    if head[0] == 4 {
                        send_packet(&mut s, 3, &[0]).await;
                        return;
                    }
                    send_packet(&mut s, 1, &body).await;
                }
            }
            r if r.starts_with("localabstract:") => {
                s.write_all(b"OKAY").await.unwrap();
                let mut buf = [0u8; 256];
                loop {
                    match s.read(&mut buf).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => s.write_all(&buf[..n]).await.unwrap(),
                    }
                }
            }
            other => panic!("fake adb: unexpected request {other}"),
        }
    }
}

impl FakeAdb {
    async fn start() -> FakeAdb {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let recorded = Arc::new(Mutex::new(Recorded::default()));
        let (updates, _) = broadcast::channel(8);
        let (r, u) = (recorded.clone(), updates.clone());
        tokio::spawn(async move {
            loop {
                let (s, _) = listener.accept().await.unwrap();
                tokio::spawn(serve(s, r.clone(), u.clone()));
            }
        });
        FakeAdb {
            addr,
            recorded,
            updates,
        }
    }

    fn adb(&self) -> Adb {
        Adb::with_addr(&self.addr)
    }
}

#[tokio::test]
async fn closed_port_is_a_transport_error_naming_the_address_and_starts_nothing() {
    let dir = std::env::temp_dir().join(format!("pu-adb-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let marker = dir.join("started");
    let script = if cfg!(windows) {
        let script = dir.join("adb.cmd");
        std::fs::write(
            &script,
            format!("@echo off\r\ntype nul > \"{}\"\r\n", marker.display()),
        )
        .unwrap();
        script
    } else {
        let script = dir.join("adb");
        std::fs::write(&script, format!("#!/bin/sh\ntouch {}\n", marker.display())).unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
        script
    };

    let addr = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().to_string()
    };

    let err = Adb::with_addr(&addr).version().await.unwrap_err();
    let Error::Transport { detail } = err else {
        panic!("expected Transport, got {err:?}");
    };
    assert!(detail.contains(&addr), "{detail}");
    assert!(
        !marker.exists(),
        "adb start-server ran for an explicit address"
    );

    let err = Adb::local(&addr, script.to_str().unwrap())
        .version()
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Transport { .. }));
    assert!(marker.exists(), "the default local server is started once");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn version_reads_the_hex_payload() {
    let fake = FakeAdb::start().await;
    assert_eq!(fake.adb().version().await.unwrap(), "0029");
}

#[tokio::test]
async fn devices_parses_devices_l() {
    let fake = FakeAdb::start().await;
    let devices = fake.adb().devices().await.unwrap();
    assert_eq!(devices.len(), 2);
    assert_eq!(devices[0].serial, "emulator-5554");
    assert_eq!(devices[0].state, DeviceState::Device);
    assert_eq!(devices[0].model.as_deref(), Some("sdk_gphone64_x86_64"));
    assert_eq!(devices[0].transport_id, Some(5));
    assert_eq!(devices[1].serial, "R5CT1234");
    assert_eq!(devices[1].state, DeviceState::Unauthorized);
    assert_eq!(devices[1].model, None);
    assert_eq!(devices[1].transport_id, Some(7));
}

#[tokio::test]
async fn track_devices_yields_each_pushed_list() {
    let fake = FakeAdb::start().await;
    let mut tracker = fake.adb().track_devices().await.unwrap();
    assert_eq!(tracker.next().await.unwrap().len(), 2);
    let _keep = fake.updates.subscribe();
    fake.updates
        .send("emulator-5556          device model:Pixel transport_id:9\n".into())
        .unwrap();
    let next = tracker.next().await.unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].serial, "emulator-5556");
}

#[tokio::test]
async fn shell_returns_stdout_stderr_and_exit_code() {
    let fake = FakeAdb::start().await;
    let out = fake.adb().shell("emulator-5554", "echo hi").await.unwrap();
    assert_eq!(out.stdout, b"hi\n");
    assert_eq!(out.stderr, b"warn\n");
    assert_eq!(out.exit_code, 7);
    assert!(
        fake.recorded
            .lock()
            .unwrap()
            .requests
            .contains(&"host:transport:emulator-5554".to_string())
    );
}

#[tokio::test]
async fn shell_stream_writes_stdin_without_closing_it() {
    let fake = FakeAdb::start().await;
    let mut sh = fake
        .adb()
        .shell_stream("emulator-5554", "cat")
        .await
        .unwrap();
    sh.write_stdin(b"token-abc\n").await.unwrap();
    assert_eq!(
        sh.next().await.unwrap(),
        Some(ShellPacket::Stdout(b"token-abc\n".to_vec()))
    );
    sh.close_stdin().await.unwrap();
    assert_eq!(sh.next().await.unwrap(), Some(ShellPacket::Exit(0)));
}

#[tokio::test]
async fn push_sends_chunks_of_at_most_64_kib() {
    let fake = FakeAdb::start().await;
    let bytes: Vec<u8> = (0..150_000u32).map(|i| (i % 251) as u8).collect();
    fake.adb()
        .push("emulator-5554", &bytes, "/data/local/tmp/x.jar", 0o444)
        .await
        .unwrap();
    let rec = fake.recorded.lock().unwrap();
    let (spec, chunks, data) = &rec.pushes[0];
    assert_eq!(spec, "/data/local/tmp/x.jar,33060");
    assert_eq!(chunks, &vec![65536, 65536, 18928]);
    assert_eq!(data, &bytes);
}

#[tokio::test]
async fn open_abstract_returns_a_duplex_stream() {
    let fake = FakeAdb::start().await;
    let mut s = fake
        .adb()
        .open_abstract("emulator-5554", "phone-use-test")
        .await
        .unwrap();
    s.write_all(b"ping").await.unwrap();
    let mut back = [0u8; 4];
    s.read_exact(&mut back).await.unwrap();
    assert_eq!(&back, b"ping");
}

#[tokio::test]
async fn fail_reply_becomes_transport_with_the_servers_message() {
    let fake = FakeAdb::start().await;
    let err = fake.adb().shell("nope", "echo hi").await.unwrap_err();
    let Error::Transport { detail } = err else {
        panic!("expected Transport, got {err:?}");
    };
    assert!(detail.contains("device 'nope' not found"), "{detail}");
}
