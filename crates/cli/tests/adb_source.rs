use std::sync::{Arc, Mutex};

use phonedriver::daemon::DeviceSource;
use phonedriver::daemon::android::AdbSource;
use phonedriver::daemon::policy::{Gated, Policy};
use phonedriver_android::adb::Adb;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const DEVICES: &str =
    "emulator-5554          device product:sdk model:sdk_gphone64 device:emu64xa transport_id:5\n";

async fn serve(s: TcpStream, requests: Arc<Mutex<Vec<String>>>) {
    serve_devices(s, requests, DEVICES).await
}

async fn send_packet(s: &mut TcpStream, id: u8, data: &[u8]) {
    let mut m = vec![id];
    m.extend_from_slice(&(data.len() as u32).to_le_bytes());
    m.extend_from_slice(data);
    s.write_all(&m).await.unwrap();
}

/// Answers `getprop ro.serialno` with a serial for an emulator, and with nothing and exit code 1 for any other device.
async fn serve_devices(mut s: TcpStream, requests: Arc<Mutex<Vec<String>>>, devices: &str) {
    let mut target = String::new();
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
                .write_all(format!("OKAY{:04x}{devices}", devices.len()).as_bytes())
                .await
                .unwrap(),
            "host:track-devices-l" => {
                s.write_all(b"OKAY").await.unwrap();
                s.write_all(format!("{:04x}{devices}", devices.len()).as_bytes())
                    .await
                    .unwrap();
                let _ = s.read_u8().await;
                return;
            }
            r if r.starts_with("host:transport:") => {
                target = r.trim_start_matches("host:transport:").to_string();
                s.write_all(b"OKAY").await.unwrap();
            }
            "shell,v2,raw:getprop ro.serialno" => {
                s.write_all(b"OKAY").await.unwrap();
                if target.starts_with("emulator-") {
                    send_packet(&mut s, 1, b"EMULATOR37X2X13X0\n").await;
                    send_packet(&mut s, 3, &[0]).await;
                } else {
                    send_packet(&mut s, 3, &[1]).await;
                }
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

#[tokio::test]
async fn an_empty_serial_answer_is_a_failed_read_that_a_deny_list_hides_and_that_is_asked_again() {
    const WIFI: &str =
        "192.168.1.20:5555      device product:p model:Phone device:d transport_id:9\n";
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let log = requests.clone();
    tokio::spawn(async move {
        loop {
            let (s, _) = listener.accept().await.unwrap();
            tokio::spawn(serve_devices(s, log.clone(), WIFI));
        }
    });
    let source = Arc::new(AdbSource::new(Adb::from_socket_spec(Some(&addr))));
    let read = source.hardware_serial("android:192.168.1.20:5555").await;
    assert!(read.is_err(), "an empty answer with exit 1: {read:?}");
    let gated = Gated::new(source.clone(), Policy::new("", "R5CT123", None));
    let listed = gated.list().await.unwrap();
    assert!(
        listed.is_empty(),
        "{:?}",
        listed.iter().map(|l| &l.id).collect::<Vec<_>>()
    );
    let asked = requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| *r == "shell,v2,raw:getprop ro.serialno")
        .count();
    assert_eq!(asked, 2, "a failed read is not kept");
}

#[tokio::test]
async fn listing_through_a_deny_list_never_asks_a_device_it_names_by_id() {
    const BOTH: &str = "emulator-5554          device product:sdk model:sdk_gphone64 device:emu64xa transport_id:5\nR5CTDENIED             device product:p model:Phone device:d transport_id:6\n";
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let log = requests.clone();
    tokio::spawn(async move {
        loop {
            let (s, _) = listener.accept().await.unwrap();
            tokio::spawn(serve_devices(s, log.clone(), BOTH));
        }
    });
    let source = Arc::new(AdbSource::new(Adb::from_socket_spec(Some(&addr))));
    let gated = Gated::new(source, Policy::new("", "android:R5CTDENIED", None));
    let listed: Vec<String> = gated
        .list()
        .await
        .unwrap()
        .into_iter()
        .map(|l| l.id)
        .collect();
    assert_eq!(listed, ["android:emulator-5554"]);
    let seen = requests.lock().unwrap().clone();
    assert!(!seen.iter().any(|r| r.contains("R5CTDENIED")), "{seen:?}");
}

/// A read that failed (a phone still booting, a dropped shell) was kept as "no serial" for as long as the phone stayed
/// connected, so the same phone over USB and Wi-Fi was listed twice from then on. A failure is asked again next time.
#[tokio::test]
async fn a_failed_serial_read_while_listing_is_asked_again_on_the_next_list() {
    const TWO: &str = "emulator-5554          device product:sdk model:sdk_gphone64 device:emu64xa transport_id:5\n192.168.1.20:5555      device product:p model:Phone device:d transport_id:9\n";
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let log = requests.clone();
    tokio::spawn(async move {
        loop {
            let (s, _) = listener.accept().await.unwrap();
            tokio::spawn(serve_devices(s, log.clone(), TWO));
        }
    });
    let source = AdbSource::new(Adb::from_socket_spec(Some(&addr)));
    let asked = || {
        requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| *r == "shell,v2,raw:getprop ro.serialno")
            .count()
    };
    source.list().await.unwrap();
    let first = asked();
    assert!(first >= 1, "{first}");
    source.list().await.unwrap();
    assert_eq!(asked(), first, "a list right after asks nothing again");
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    source.list().await.unwrap();
    assert!(
        asked() > first,
        "the failed read was kept: {first} then {}",
        asked()
    );
}
