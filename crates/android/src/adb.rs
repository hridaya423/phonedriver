//! A client for the adb server protocol (architecture 6.1). No `adb` process per call.

use std::time::{SystemTime, UNIX_EPOCH};

use phonedriver_core::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub type Result<T> = std::result::Result<T, Error>;

const SYNC_CHUNK: usize = 64 * 1024;

fn transport(detail: impl Into<String>) -> Error {
    Error::Transport {
        detail: detail.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Server {
    /// `ADB_SERVER_SOCKET` named this address. A dead address is an error, never a reason to start a server.
    Remote,
    /// The default address. If it refuses, `program start-server` runs once.
    Local { program: String },
}

#[derive(Debug, Clone)]
pub struct Adb {
    addr: String,
    server: Server,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    Device,
    Offline,
    Unauthorized,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdbDevice {
    pub serial: String,
    pub state: DeviceState,
    pub model: Option<String>,
    pub transport_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
}

impl ShellOutput {
    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellPacket {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
    Exit(i32),
}

impl Adb {
    /// The server named by `ADB_SERVER_SOCKET` if set, else the default local server.
    pub fn from_env() -> Adb {
        Adb::from_socket_spec(std::env::var("ADB_SERVER_SOCKET").ok().as_deref())
    }

    pub fn from_socket_spec(spec: Option<&str>) -> Adb {
        match spec {
            Some(v) => Adb::with_addr(v.strip_prefix("tcp:").unwrap_or(v)),
            None => Adb::local("127.0.0.1:5037", "adb"),
        }
    }

    /// An explicit server address; a refused connection is reported, not repaired.
    pub fn with_addr(addr: &str) -> Adb {
        Adb {
            addr: addr.to_string(),
            server: Server::Remote,
        }
    }

    pub fn local(addr: &str, adb_program: &str) -> Adb {
        Adb {
            addr: addr.to_string(),
            server: Server::Local {
                program: adb_program.to_string(),
            },
        }
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    fn unreachable(&self, e: std::io::Error) -> Error {
        transport(format!(
            "cannot reach the adb server at {}: {e}. Start it with `adb start-server`, or check the SSH tunnel if ADB_SERVER_SOCKET is set",
            self.addr
        ))
    }

    async fn connect(&self) -> Result<TcpStream> {
        let first = TcpStream::connect(&self.addr).await;
        let stream = match (first, &self.server) {
            (Ok(s), _) => s,
            (Err(e), Server::Local { program })
                if e.kind() == std::io::ErrorKind::ConnectionRefused =>
            {
                tokio::process::Command::new(program)
                    .arg("start-server")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .await
                    .map_err(|e| transport(format!("cannot run `{program} start-server`: {e}")))?;
                TcpStream::connect(&self.addr)
                    .await
                    .map_err(|e| self.unreachable(e))?
            }
            (Err(e), _) => return Err(self.unreachable(e)),
        };
        stream.set_nodelay(true).map_err(|e| self.unreachable(e))?;
        Ok(stream)
    }

    async fn request(&self, s: &mut TcpStream, payload: &str) -> Result<()> {
        let msg = format!("{:04x}{payload}", payload.len());
        s.write_all(msg.as_bytes())
            .await
            .map_err(|e| self.unreachable(e))?;
        let mut status = [0u8; 4];
        s.read_exact(&mut status)
            .await
            .map_err(|e| transport(format!("adb closed the connection during `{payload}`: {e}")))?;
        match &status {
            b"OKAY" => Ok(()),
            b"FAIL" => {
                let message = self.read_hex_prefixed(s).await?;
                Err(transport(format!(
                    "adb refused `{payload}`: {}",
                    String::from_utf8_lossy(&message)
                )))
            }
            other => Err(transport(format!(
                "adb answered `{payload}` with {other:?}"
            ))),
        }
    }

    async fn read_hex_prefixed(&self, s: &mut TcpStream) -> Result<Vec<u8>> {
        let mut len = [0u8; 4];
        s.read_exact(&mut len)
            .await
            .map_err(|e| transport(format!("adb closed the connection: {e}")))?;
        let n = std::str::from_utf8(&len)
            .ok()
            .and_then(|t| usize::from_str_radix(t, 16).ok())
            .ok_or_else(|| transport(format!("adb sent a bad length {len:?}")))?;
        let mut buf = vec![0u8; n];
        s.read_exact(&mut buf)
            .await
            .map_err(|e| transport(format!("adb closed the connection: {e}")))?;
        Ok(buf)
    }

    async fn host_query(&self, query: &str) -> Result<String> {
        let mut s = self.connect().await?;
        self.request(&mut s, query).await?;
        let body = self.read_hex_prefixed(&mut s).await?;
        Ok(String::from_utf8_lossy(&body).into_owned())
    }

    pub async fn version(&self) -> Result<String> {
        self.host_query("host:version").await
    }

    pub async fn devices(&self) -> Result<Vec<AdbDevice>> {
        Ok(parse_devices(&self.host_query("host:devices-l").await?))
    }

    /// `mdns daemon version [...]` when the server resolves mDNS, an error when it does not.
    pub async fn mdns_check(&self) -> Result<String> {
        self.host_query("host:mdns:check").await
    }

    /// The server's mDNS listing, one `<instance>\t<service>\t<ip>:<port>` line per service.
    pub async fn mdns_services(&self) -> Result<String> {
        self.host_query("host:mdns:services").await
    }

    /// `adb pair`: the success line, or an error carrying the server's reason.
    pub async fn pair(&self, addr: &str, code: &str) -> Result<String> {
        let mut s = self.connect().await?;
        self.request(&mut s, &format!("host:pair:{code}:{addr}"))
            .await?;
        let mut status = [0u8; 4];
        s.read_exact(&mut status)
            .await
            .map_err(|e| transport(format!("adb closed the connection during pairing: {e}")))?;
        if &status == b"FAIL" {
            let message = self.read_hex_prefixed(&mut s).await?;
            return Err(transport(format!(
                "adb pair {addr} failed: {}",
                String::from_utf8_lossy(&message)
            )));
        }
        let mut rest =
            vec![
                0u8;
                usize::from_str_radix(std::str::from_utf8(&status).unwrap_or(""), 16)
                    .map_err(|_| transport(format!("adb answered pairing with {status:?}")))?
            ];
        s.read_exact(&mut rest)
            .await
            .map_err(|e| transport(format!("adb closed the connection during pairing: {e}")))?;
        Ok(String::from_utf8_lossy(&rest).into_owned())
    }

    /// `adb connect`: `connected to ...` or `already connected to ...`, or the reason it failed (as an `Ok` line, as adb reports it).
    pub async fn connect_device(&self, addr: &str) -> Result<String> {
        self.host_query(&format!("host:connect:{addr}")).await
    }

    /// `adb tcpip`: the device restarts adbd listening on `port`. The device's own answer is returned.
    pub async fn tcpip(&self, serial: &str, port: u16) -> Result<String> {
        let mut s = self
            .transport_stream(serial, &format!("tcpip:{port}"))
            .await?;
        let mut answer = Vec::new();
        s.read_to_end(&mut answer)
            .await
            .map_err(|e| transport(format!("tcpip on {serial} failed: {e}")))?;
        Ok(String::from_utf8_lossy(&answer).into_owned())
    }

    /// `Android 8.0 (API 26)`, from the device's build properties.
    pub async fn os_label(&self, serial: &str) -> Result<String> {
        let out = self
            .shell(
                serial,
                "getprop ro.build.version.release; getprop ro.build.version.sdk",
            )
            .await?;
        Ok(os_label(&String::from_utf8_lossy(&out.stdout)))
    }

    /// The phone's own serial number, the same over every connection to it.
    pub async fn hardware_serial(&self, serial: &str) -> Result<String> {
        let out = self.shell(serial, "getprop ro.serialno").await?;
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Stops the local server and starts it again as a child of this process, so it runs with this process's
    /// permissions. False for a server named by `ADB_SERVER_SOCKET`, which is not ours to restart.
    pub async fn restart_server(&self) -> Result<bool> {
        let Server::Local { program } = &self.server else {
            return Ok(false);
        };
        for arg in ["kill-server", "start-server"] {
            tokio::process::Command::new(program)
                .arg(arg)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await
                .map_err(|e| transport(format!("cannot run `{program} {arg}`: {e}")))?;
        }
        Ok(true)
    }

    pub async fn track_devices(&self) -> Result<DeviceTracker> {
        let mut s = self.connect().await?;
        self.request(&mut s, "host:track-devices-l").await?;
        Ok(DeviceTracker { s })
    }

    async fn transport_stream(&self, serial: &str, service: &str) -> Result<TcpStream> {
        let mut s = self.connect().await?;
        self.request(&mut s, &format!("host:transport:{serial}"))
            .await?;
        self.request(&mut s, service).await?;
        Ok(s)
    }

    /// Runs a command to completion with shell v2 framing.
    pub async fn shell(&self, serial: &str, cmd: &str) -> Result<ShellOutput> {
        let mut sh = self.shell_stream(serial, cmd).await?;
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        loop {
            match sh.next().await? {
                Some(ShellPacket::Stdout(d)) => stdout.extend(d),
                Some(ShellPacket::Stderr(d)) => stderr.extend(d),
                Some(ShellPacket::Exit(exit_code)) => {
                    return Ok(ShellOutput {
                        stdout,
                        stderr,
                        exit_code,
                    });
                }
                None => return Err(transport(format!("`{cmd}` ended without an exit code"))),
            }
        }
    }

    /// Starts a command and keeps the stream open. Closing the stream kills the command on the device.
    pub async fn shell_stream(&self, serial: &str, cmd: &str) -> Result<ShellStream> {
        let s = self
            .transport_stream(serial, &format!("shell,v2,raw:{cmd}"))
            .await?;
        Ok(ShellStream { s })
    }

    pub async fn push(&self, serial: &str, bytes: &[u8], path: &str, mode: u32) -> Result<()> {
        let mut s = self.transport_stream(serial, "sync:").await?;
        let io = |e: std::io::Error| transport(format!("push to {path} failed: {e}"));
        let spec = format!("{path},{}", 0o100000 | mode);
        let mut head = b"SEND".to_vec();
        head.extend_from_slice(&(spec.len() as u32).to_le_bytes());
        head.extend_from_slice(spec.as_bytes());
        s.write_all(&head).await.map_err(io)?;
        for chunk in bytes.chunks(SYNC_CHUNK) {
            let mut m = b"DATA".to_vec();
            m.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
            m.extend_from_slice(chunk);
            s.write_all(&m).await.map_err(io)?;
        }
        let mtime = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as u32)
            .unwrap_or(0);
        let mut done = b"DONE".to_vec();
        done.extend_from_slice(&mtime.to_le_bytes());
        s.write_all(&done).await.map_err(io)?;
        let mut reply = [0u8; 8];
        s.read_exact(&mut reply).await.map_err(io)?;
        if &reply[..4] != b"OKAY" {
            let n = u32::from_le_bytes(reply[4..8].try_into().unwrap_or([0; 4])) as usize;
            let mut msg = vec![0u8; n.min(4096)];
            let _ = s.read_exact(&mut msg).await;
            return Err(transport(format!(
                "push to {path} failed: {}",
                String::from_utf8_lossy(&msg)
            )));
        }
        let _ = s.write_all(b"QUIT\0\0\0\0").await;
        Ok(())
    }

    /// A duplex stream to the abstract socket `name` on the device.
    pub async fn open_abstract(&self, serial: &str, name: &str) -> Result<TcpStream> {
        self.transport_stream(serial, &format!("localabstract:{name}"))
            .await
    }
}

fn os_label(props: &str) -> String {
    let mut lines = props.lines().map(str::trim).filter(|l| !l.is_empty());
    match (lines.next(), lines.next()) {
        (Some(release), Some(api)) => format!("Android {release} (API {api})"),
        (Some(api), None) if api.chars().all(|c| c.is_ascii_digit()) => {
            format!("Android API {api}")
        }
        _ => "Android".to_string(),
    }
}

fn parse_devices(text: &str) -> Vec<AdbDevice> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let serial = parts.next()?.to_string();
            let state = match parts.next()? {
                "device" => DeviceState::Device,
                "offline" => DeviceState::Offline,
                "unauthorized" => DeviceState::Unauthorized,
                _ => DeviceState::Other,
            };
            let (mut model, mut transport_id) = (None, None);
            for part in parts {
                if let Some(m) = part.strip_prefix("model:") {
                    model = Some(m.to_string());
                } else if let Some(t) = part.strip_prefix("transport_id:") {
                    transport_id = t.parse().ok();
                }
            }
            Some(AdbDevice {
                serial,
                state,
                model,
                transport_id,
            })
        })
        .collect()
}

pub struct DeviceTracker {
    s: TcpStream,
}

impl DeviceTracker {
    /// The next full device list the server pushes; the first call returns the current list.
    pub async fn next(&mut self) -> Result<Vec<AdbDevice>> {
        let mut len = [0u8; 4];
        self.s
            .read_exact(&mut len)
            .await
            .map_err(|e| transport(format!("adb closed the device list stream: {e}")))?;
        let n = std::str::from_utf8(&len)
            .ok()
            .and_then(|t| usize::from_str_radix(t, 16).ok())
            .ok_or_else(|| transport(format!("adb sent a bad length {len:?}")))?;
        let mut buf = vec![0u8; n];
        self.s
            .read_exact(&mut buf)
            .await
            .map_err(|e| transport(format!("adb closed the device list stream: {e}")))?;
        Ok(parse_devices(&String::from_utf8_lossy(&buf)))
    }
}

pub struct ShellStream {
    s: TcpStream,
}

impl ShellStream {
    async fn send(&mut self, id: u8, data: &[u8]) -> Result<()> {
        let mut m = vec![id];
        m.extend_from_slice(&(data.len() as u32).to_le_bytes());
        m.extend_from_slice(data);
        self.s
            .write_all(&m)
            .await
            .map_err(|e| transport(format!("shell stream write failed: {e}")))
    }

    pub async fn write_stdin(&mut self, data: &[u8]) -> Result<()> {
        self.send(0, data).await
    }

    pub async fn close_stdin(&mut self) -> Result<()> {
        self.send(4, &[]).await
    }

    /// The next packet, or `None` once the device closed the stream.
    pub async fn next(&mut self) -> Result<Option<ShellPacket>> {
        let mut head = [0u8; 5];
        match self.s.read_exact(&mut head).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(transport(format!("shell stream read failed: {e}"))),
        }
        let n = u32::from_le_bytes([head[1], head[2], head[3], head[4]]) as usize;
        let mut body = vec![0u8; n];
        self.s
            .read_exact(&mut body)
            .await
            .map_err(|e| transport(format!("shell stream read failed: {e}")))?;
        Ok(Some(match head[0] {
            1 => ShellPacket::Stdout(body),
            2 => ShellPacket::Stderr(body),
            3 => ShellPacket::Exit(body.first().copied().unwrap_or(0) as i32),
            other => return Err(transport(format!("unknown shell packet id {other}"))),
        }))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_os_label_names_the_release_and_the_api_level() {
        assert_eq!(super::os_label("8.0.0\n26\n"), "Android 8.0.0 (API 26)");
        assert_eq!(super::os_label("26\n"), "Android API 26");
        assert_eq!(super::os_label(""), "Android");
    }

    use super::*;

    #[test]
    fn socket_spec_decides_whether_a_server_may_be_started() {
        let remote = Adb::from_socket_spec(Some("tcp:127.0.0.1:5038"));
        assert_eq!(remote.addr(), "127.0.0.1:5038");
        assert_eq!(remote.server, Server::Remote);
        let local = Adb::from_socket_spec(None);
        assert_eq!(local.addr(), "127.0.0.1:5037");
        assert!(matches!(local.server, Server::Local { .. }));
    }
}
