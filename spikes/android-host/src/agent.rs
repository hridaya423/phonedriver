use crate::adb::{Adb, Res};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;

pub const JAR: &str = "/data/local/tmp/spike-agent.jar";

pub struct Agent {
    pub stdin: OwnedWriteHalf,
    pub name: String,
    pub token: String,
    pub log: Arc<Mutex<Vec<String>>>,
    pub reader: tokio::task::JoinHandle<()>,
}

pub fn rand_hex(n: usize) -> String {
    let mut b = vec![0u8; n];
    let mut f = std::fs::File::open("/dev/urandom").unwrap();
    std::io::Read::read_exact(&mut f, &mut b).unwrap();
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub async fn push_jar(adb: &Adb, serial: &str, mode: u32) -> Res<()> {
    let data = std::fs::read("../android-agent/build/spike.jar")?;
    adb.push(serial, JAR, mode, &data).await
}

pub async fn launch(adb: &Adb, serial: &str, extra: &str, send_token: bool) -> Res<Agent> {
    let name = format!("phoneuse-spike-{}", rand_hex(4));
    let token = rand_hex(32);
    let sh = adb
        .shell(serial, &format!("CLASSPATH={JAR} app_process / dev.phoneuse.agent.Main 0.1 {name} {extra}"))
        .await?;
    let (mut r, mut w) = sh.s.into_split();
    if send_token {
        w.write_all(&{
            let mut m = vec![0u8];
            let line = format!("{token}\n");
            m.extend_from_slice(&(line.len() as u32).to_le_bytes());
            m.extend_from_slice(line.as_bytes());
            m
        })
        .await?;
    }
    let log = Arc::new(Mutex::new(Vec::new()));
    let l2 = log.clone();
    let reader = tokio::spawn(async move {
        loop {
            let mut h = [0u8; 5];
            if r.read_exact(&mut h).await.is_err() {
                break;
            }
            let n = u32::from_le_bytes(h[1..5].try_into().unwrap()) as usize;
            let mut d = vec![0u8; n];
            if r.read_exact(&mut d).await.is_err() {
                break;
            }
            if h[0] == 3 {
                l2.lock().unwrap().push(format!("[exit {}]", d.first().copied().unwrap_or(0)));
                break;
            }
            for line in String::from_utf8_lossy(&d).lines() {
                l2.lock().unwrap().push(line.to_string());
            }
        }
    });
    Ok(Agent { stdin: w, name, token, log, reader })
}

impl Agent {
    pub async fn wait_listening(&self) -> bool {
        for _ in 0..200 {
            if self.log.lock().unwrap().iter().any(|l| l.contains("agent listening")) {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        false
    }

    pub fn log_text(&self) -> String {
        self.log.lock().unwrap().join("\n")
    }
}

pub struct Control {
    pub s: TcpStream,
    next: u64,
}

impl Control {
    pub async fn open(adb: &Adb, serial: &str, name: &str, token: &str) -> Res<(Control, serde_json::Value)> {
        let mut s = adb.open_abstract(serial, name).await?;
        s.write_all(b"C").await?;
        let hello = serde_json::json!({"id":0,"op":"hello","version":"0.1","token":token});
        write_frame(&mut s, &hello).await?;
        let r = read_frame(&mut s).await?;
        Ok((Control { s, next: 1 }, r))
    }

    pub async fn req(&mut self, mut v: serde_json::Value) -> Res<serde_json::Value> {
        v["id"] = self.next.into();
        self.next += 1;
        write_frame(&mut self.s, &v).await?;
        read_frame(&mut self.s).await
    }
}

pub async fn write_frame(s: &mut TcpStream, v: &serde_json::Value) -> Res<()> {
    let b = serde_json::to_vec(v)?;
    let mut m = (b.len() as u32).to_be_bytes().to_vec();
    m.extend(b);
    s.write_all(&m).await?;
    Ok(())
}

pub async fn read_frame(s: &mut TcpStream) -> Res<serde_json::Value> {
    let mut l = [0u8; 4];
    s.read_exact(&mut l).await?;
    let mut b = vec![0u8; u32::from_be_bytes(l) as usize];
    s.read_exact(&mut b).await?;
    Ok(serde_json::from_slice(&b)?)
}
