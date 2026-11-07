use phonedriver_android::adb::Adb;
use phonedriver_android::agent::{self, Launched};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub use phonedriver_android::agent::VERSION;

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
    pub launched: Launched,
}

pub async fn launch(adb: &Adb, serial: &str) -> RawAgent {
    let launched = agent::launch(adb, serial).await.unwrap();
    RawAgent {
        adb: adb.clone(),
        serial: serial.to_string(),
        name: launched.name.clone(),
        token: launched.token.clone(),
        launched,
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
    pub async fn control(&self) -> RawControl {
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
                "no hello reply ({e}); agent log: {}",
                self.launched.log.recent().join("\n")
            ),
        };
        RawControl {
            stream: s,
            hello,
            next_id: 1,
        }
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
