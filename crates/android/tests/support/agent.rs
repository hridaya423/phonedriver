use std::time::Duration;

use phone_use_android::adb::{Adb, ShellPacket, ShellStream};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub const VERSION: &str = "0.1.0";
const JAR: &str = "/data/local/tmp/phone-use-agent-test.jar";

static DEVICE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Only one agent can hold the accessibility bridge, so live tests take turns.
pub async fn exclusive() -> tokio::sync::MutexGuard<'static, ()> {
    DEVICE_LOCK.lock().await
}

pub struct RawAgent {
    pub adb: Adb,
    pub serial: String,
    pub name: String,
    pub token: String,
    pub shell: ShellStream,
    pub stderr: String,
}

fn random_hex(n: usize) -> String {
    let mut b = vec![0u8; n];
    getrandom::fill(&mut b).unwrap();
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub async fn launch(adb: &Adb, serial: &str) -> RawAgent {
    adb.shell(serial, "pkill -f 'dev.phoneuse.agent.[M]ain'")
        .await
        .unwrap();
    let jar = std::fs::read(env!("PHONE_USE_AGENT_JAR")).unwrap();
    adb.shell(serial, &format!("rm -f {JAR}")).await.unwrap();
    adb.push(serial, &jar, JAR, 0o444).await.unwrap();
    let name = format!("phone-use-{VERSION}-{}", random_hex(4));
    let token = random_hex(32);
    let mut shell = adb
        .shell_stream(
            serial,
            &format!("CLASSPATH={JAR} app_process / dev.phoneuse.agent.Main {VERSION} {name}"),
        )
        .await
        .unwrap();
    shell
        .write_stdin(format!("{token}\n").as_bytes())
        .await
        .unwrap();
    let mut stderr = String::new();
    let mut stdout = String::new();
    let ready = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(p) = shell.next().await.unwrap() {
            match p {
                ShellPacket::Stdout(d) => {
                    stdout.push_str(&String::from_utf8_lossy(&d));
                    if stdout.lines().any(|l| l == "ready") {
                        return true;
                    }
                }
                ShellPacket::Stderr(d) => stderr.push_str(&String::from_utf8_lossy(&d)),
                ShellPacket::Exit(_) => return false,
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(ready, "agent did not print ready; stderr: {stderr}");
    RawAgent {
        adb: adb.clone(),
        serial: serial.to_string(),
        name,
        token,
        shell,
        stderr,
    }
}

pub async fn write_frame(s: &mut TcpStream, v: &Value) {
    let b = serde_json::to_vec(v).unwrap();
    let mut m = (b.len() as u32).to_be_bytes().to_vec();
    m.extend(b);
    s.write_all(&m).await.unwrap();
}

pub async fn read_frame(s: &mut TcpStream) -> std::io::Result<Value> {
    let mut l = [0u8; 4];
    s.read_exact(&mut l).await?;
    let mut b = vec![0u8; u32::from_be_bytes(l) as usize];
    s.read_exact(&mut b).await?;
    Ok(serde_json::from_slice(&b).unwrap())
}

pub struct RawControl {
    pub stream: TcpStream,
    pub hello: Value,
    next_id: u64,
}

impl RawAgent {
    pub async fn control(&mut self) -> RawControl {
        let mut s = self
            .adb
            .open_abstract(&self.serial, &self.name)
            .await
            .unwrap();
        s.write_all(b"C").await.unwrap();
        write_frame(
            &mut s,
            &json!({"id":0,"op":"hello","version":VERSION,"token":self.token}),
        )
        .await;
        let hello = match read_frame(&mut s).await {
            Ok(h) => h,
            Err(e) => panic!(
                "no hello reply ({e}); agent stderr: {}",
                self.drain_stderr().await
            ),
        };
        RawControl {
            stream: s,
            hello,
            next_id: 1,
        }
    }

    pub async fn drain_stderr(&mut self) -> String {
        let mut text = std::mem::take(&mut self.stderr);
        while let Ok(Ok(Some(p))) =
            tokio::time::timeout(Duration::from_millis(500), self.shell.next()).await
        {
            match p {
                ShellPacket::Stderr(d) | ShellPacket::Stdout(d) => {
                    text.push_str(&String::from_utf8_lossy(&d))
                }
                ShellPacket::Exit(c) => text.push_str(&format!("[exit {c}]")),
            }
        }
        text
    }

    pub async fn video(&self) -> TcpStream {
        let mut s = self
            .adb
            .open_abstract(&self.serial, &self.name)
            .await
            .unwrap();
        s.write_all(b"V").await.unwrap();
        write_frame(&mut s, &json!({"token":self.token})).await;
        s
    }
}

impl RawControl {
    /// Sends a request and returns the reply with the matching id, skipping pushed events.
    pub async fn request(&mut self, mut req: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        req["id"] = id.into();
        write_frame(&mut self.stream, &req).await;
        loop {
            let v = read_frame(&mut self.stream).await.unwrap();
            if v["id"] == id {
                return v;
            }
        }
    }

    pub async fn ok(&mut self, req: Value) -> Value {
        let r = self.request(req).await;
        assert_eq!(r["ok"], true, "{r}");
        r
    }
}
