use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub type Res<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub struct Adb {
    addr: String,
}

impl Adb {
    pub fn from_env() -> Adb {
        let addr = match std::env::var("ADB_SERVER_SOCKET") {
            Ok(v) => v.strip_prefix("tcp:").unwrap_or(&v).to_string(),
            Err(_) => "127.0.0.1:5037".to_string(),
        };
        Adb { addr }
    }

    async fn connect(&self) -> Res<TcpStream> {
        let s = TcpStream::connect(&self.addr).await?;
        s.set_nodelay(true)?;
        Ok(s)
    }

    pub async fn request(s: &mut TcpStream, payload: &str) -> Res<()> {
        let msg = format!("{:04x}{}", payload.len(), payload);
        s.write_all(msg.as_bytes()).await?;
        let mut status = [0u8; 4];
        s.read_exact(&mut status).await?;
        if &status == b"OKAY" {
            return Ok(());
        }
        if &status == b"FAIL" {
            let mut len = [0u8; 4];
            s.read_exact(&mut len).await?;
            let n = usize::from_str_radix(std::str::from_utf8(&len)?, 16)?;
            let mut m = vec![0u8; n];
            s.read_exact(&mut m).await?;
            return Err(format!("adb FAIL: {}", String::from_utf8_lossy(&m)).into());
        }
        Err(format!("bad status {:?}", status).into())
    }

    async fn read_hex_prefixed(s: &mut TcpStream) -> Res<Vec<u8>> {
        let mut len = [0u8; 4];
        s.read_exact(&mut len).await?;
        let n = usize::from_str_radix(std::str::from_utf8(&len)?, 16)?;
        let mut m = vec![0u8; n];
        s.read_exact(&mut m).await?;
        Ok(m)
    }

    pub async fn version(&self) -> Res<String> {
        let mut s = self.connect().await?;
        Self::request(&mut s, "host:version").await?;
        Ok(String::from_utf8(Self::read_hex_prefixed(&mut s).await?)?)
    }

    pub async fn track_devices_first(&self) -> Res<String> {
        let mut s = self.connect().await?;
        Self::request(&mut s, "host:track-devices-l").await?;
        Ok(String::from_utf8(Self::read_hex_prefixed(&mut s).await?)?)
    }

    pub async fn transport(&self, serial: &str, service: &str) -> Res<TcpStream> {
        let mut s = self.connect().await?;
        Self::request(&mut s, &format!("host:transport:{serial}")).await?;
        Self::request(&mut s, service).await?;
        Ok(s)
    }

    pub async fn shell(&self, serial: &str, cmd: &str) -> Res<Shell> {
        let s = self.transport(serial, &format!("shell,v2,raw:{cmd}")).await?;
        Ok(Shell { s })
    }

    pub async fn shell_output(&self, serial: &str, cmd: &str) -> Res<(Vec<u8>, Vec<u8>, i32)> {
        let mut sh = self.shell(serial, cmd).await?;
        sh.run_to_end().await
    }

    pub async fn open_abstract(&self, serial: &str, name: &str) -> Res<TcpStream> {
        self.transport(serial, &format!("localabstract:{name}")).await
    }

    pub async fn push(&self, serial: &str, remote: &str, mode: u32, data: &[u8]) -> Res<()> {
        let mut s = self.transport(serial, "sync:").await?;
        let spec = format!("{remote},{mode}");
        let mut m = Vec::new();
        m.extend_from_slice(b"SEND");
        m.extend_from_slice(&(spec.len() as u32).to_le_bytes());
        m.extend_from_slice(spec.as_bytes());
        for chunk in data.chunks(64 * 1024) {
            m.extend_from_slice(b"DATA");
            m.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
            m.extend_from_slice(chunk);
        }
        m.extend_from_slice(b"DONE");
        m.extend_from_slice(&0u32.to_le_bytes());
        s.write_all(&m).await?;
        let mut head = [0u8; 8];
        s.read_exact(&mut head).await?;
        if &head[..4] != b"OKAY" {
            let n = u32::from_le_bytes(head[4..8].try_into()?) as usize;
            let mut e = vec![0u8; n];
            s.read_exact(&mut e).await?;
            return Err(format!("sync: {}{}", String::from_utf8_lossy(&head[..4]), String::from_utf8_lossy(&e)).into());
        }
        s.write_all(b"QUIT\0\0\0\0").await?;
        Ok(())
    }
}

pub struct Shell {
    pub s: TcpStream,
}

impl Shell {
    pub async fn write_stdin(&mut self, data: &[u8]) -> Res<()> {
        let mut m = vec![0u8];
        m.extend_from_slice(&(data.len() as u32).to_le_bytes());
        m.extend_from_slice(data);
        self.s.write_all(&m).await?;
        Ok(())
    }

    pub async fn close_stdin(&mut self) -> Res<()> {
        let mut m = vec![4u8];
        m.extend_from_slice(&0u32.to_le_bytes());
        self.s.write_all(&m).await?;
        Ok(())
    }

    pub async fn read_packet(&mut self) -> Res<(u8, Vec<u8>)> {
        let mut h = [0u8; 5];
        self.s.read_exact(&mut h).await?;
        let n = u32::from_le_bytes(h[1..5].try_into()?) as usize;
        let mut d = vec![0u8; n];
        self.s.read_exact(&mut d).await?;
        Ok((h[0], d))
    }

    pub async fn run_to_end(&mut self) -> Res<(Vec<u8>, Vec<u8>, i32)> {
        let (mut out, mut err, mut code) = (Vec::new(), Vec::new(), -1);
        loop {
            let (id, d) = match self.read_packet().await {
                Ok(p) => p,
                Err(_) => break,
            };
            match id {
                1 => out.extend(d),
                2 => err.extend(d),
                3 => {
                    code = *d.first().unwrap_or(&0) as i32;
                    break;
                }
                _ => {}
            }
        }
        Ok((out, err, code))
    }
}
