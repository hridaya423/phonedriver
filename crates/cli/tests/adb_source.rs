use std::sync::{Arc, Mutex};

use phone_use::daemon::DeviceSource;
use phone_use::daemon::android::AdbSource;
use phone_use_android::adb::Adb;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const DEVICES: &str =
    "emulator-5554          device product:sdk model:sdk_gphone64 device:emu64xa transport_id:5\n";

async fn serve(mut s: TcpStream, requests: Arc<Mutex<Vec<String>>>) {
    loop {
        let mut len = [0u8; 4];
        if s.read_exact(&mut len).await.is_err() {
            return;
        }
        let n = usize::from_str_radix(std::str::from_utf8(&len).unwrap(), 16).unwrap();
        let mut buf = vec![0u8; n];
        s.read_exact(&mut buf).await.unwrap();
        let req = String::from_utf8(buf).unwrap();
        requests.lock().unwrap().push(req.clone());
        match req.as_str() {
            "host:devices-l" => s
                .write_all(format!("OKAY{:04x}{DEVICES}", DEVICES.len()).as_bytes())
                .await
                .unwrap(),
            "host:track-devices-l" => {
                s.write_all(b"OKAY").await.unwrap();
                s.write_all(format!("{:04x}{DEVICES}", DEVICES.len()).as_bytes())
                    .await
                    .unwrap();
                let _ = s.read_u8().await;
                return;
            }
            _ => return,
        }
    }
}

#[tokio::test]
async fn once_tracking_listing_devices_does_not_ask_the_adb_server_again() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let log = requests.clone();
    tokio::spawn(async move {
        loop {
            let (s, _) = listener.accept().await.unwrap();
            tokio::spawn(serve(s, log.clone()));
        }
    });
    let source = AdbSource::new(Adb::from_socket_spec(Some(&addr)));
    assert_eq!(source.list().await.unwrap().len(), 1);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let before = requests.lock().unwrap().len();
    for _ in 0..5 {
        let listed = source.list().await.unwrap();
        assert_eq!(listed[0].id, "android:emulator-5554");
    }
    let seen = requests.lock().unwrap().clone();
    assert_eq!(seen.len(), before, "{seen:?}");
    assert_eq!(
        seen.iter().filter(|r| *r == "host:track-devices-l").count(),
        1,
        "{seen:?}"
    );
}
