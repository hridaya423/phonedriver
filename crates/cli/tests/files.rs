//! `files`: a file goes from one host folder to the device's Pictures or Download, or comes back from there, and
//! the reply is a receipt (decision D119).

use crate::fakes;

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use fakes::tree;
use phonedriver::daemon::rpc::RpcError;
use phonedriver::daemon::{Config, Daemon, DeviceSource, Listed};
use phonedriver_core::device::Device;
use phonedriver_core::error::Error;
use phonedriver_core::testing::{Call, FakeDevice};
use serde_json::{Value, json};

struct One {
    id: &'static str,
    fake: Arc<FakeDevice>,
}

#[async_trait]
impl DeviceSource for One {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(vec![Listed {
            id: self.id.into(),
            model: "Fake".into(),
            os: None,
            tier: "fake".into(),
            capabilities: self.fake.info().capabilities,
        }])
    }
    async fn connect(&self, _id: &str) -> Result<Arc<dyn Device>, Error> {
        Ok(self.fake.clone())
    }
}

const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

struct Rig {
    d: Arc<Daemon>,
    fake: Arc<FakeDevice>,
    home: tempfile::TempDir,
    folder: tempfile::TempDir,
}

fn setup(id: &'static str) -> Rig {
    let fake = Arc::new(FakeDevice::new(vec![tree(&["One"])]));
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::new(
        Arc::new(One {
            id,
            fake: fake.clone(),
        }),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    Rig {
        d,
        fake,
        home,
        folder: tempfile::tempdir().unwrap(),
    }
}

impl Rig {
    fn with(&self, mut params: Value) -> Value {
        params["folder"] = json!(self.folder.path());
        params
    }
    async fn ok(&self, params: Value) -> Value {
        self.d
            .call("default", "files", self.with(params))
            .await
            .unwrap_or_else(|e| panic!("files failed: {e:?}"))
    }
    async fn err(&self, params: Value) -> RpcError {
        self.d
            .call("default", "files", self.with(params))
            .await
            .expect_err("expected an error")
    }
    fn transfers(&self) -> Vec<Call> {
        self.fake
            .calls()
            .into_iter()
            .filter(|c| matches!(c, Call::FilePush(..) | Call::FilePull(..)))
            .collect()
    }
}

fn everything_under(dir: &Path) -> String {
    let mut out = String::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.push_str(&everything_under(&path));
        } else {
            out.push_str(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
        }
    }
    out
}

#[tokio::test(start_paused = true)]
async fn a_push_writes_the_file_to_the_photos_or_the_downloads_and_answers_a_receipt_only() {
    let rig = setup("fake:1");
    std::fs::write(rig.folder.path().join("cat.png"), b"abc").unwrap();
    std::fs::write(rig.folder.path().join("notes.txt"), b"PLAINBYTESOFTHEFILE").unwrap();
    let r = rig.ok(json!({"push": "cat.png"})).await;
    assert_eq!(
        r,
        json!({"pushed": "cat.png", "to": "photos", "device_path": "/sdcard/Pictures/cat.png", "bytes": 3, "sha256": ABC})
    );
    assert_eq!(rig.fake.file("/sdcard/Pictures/cat.png").unwrap(), b"abc");
    let r = rig.ok(json!({"push": "notes.txt"})).await;
    assert_eq!(r["device_path"], "/sdcard/Download/notes.txt");
    assert!(!r.to_string().contains("PLAINBYTES"), "{r}");
    let r = rig.ok(json!({"push": "cat.png", "to": "downloads"})).await;
    assert_eq!(r["device_path"], "/sdcard/Download/cat.png");
    let e = rig.err(json!({"push": "cat.png"})).await;
    assert_eq!(e.code, -32025, "{e:?}");
    assert!(e.message.contains("already on the device"), "{}", e.message);
    assert_eq!(rig.transfers().len(), 3);
    rig.d.call("default", "release", json!({})).await.unwrap();
    assert!(
        !everything_under(rig.home.path()).contains("PLAINBYTES"),
        "a trace keeps the receipt, not the bytes"
    );
}

#[tokio::test(start_paused = true)]
async fn a_push_reads_inside_the_one_folder_only() {
    let rig = setup("fake:1");
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.txt"), b"abc").unwrap();
    std::fs::create_dir(rig.folder.path().join("sub")).unwrap();
    std::fs::write(rig.folder.path().join("sub/in.txt"), b"abc").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        outside.path().join("secret.txt"),
        rig.folder.path().join("link.txt"),
    )
    .unwrap();
    let big = std::fs::File::create(rig.folder.path().join("big.bin")).unwrap();
    big.set_len(64 * 1024 * 1024 + 1).unwrap();
    let above = format!(
        "../{}/secret.txt",
        outside.path().file_name().unwrap().to_string_lossy()
    );
    let absolute = outside.path().join("secret.txt").display().to_string();
    for (name, why) in [
        (above.as_str(), "is not inside"),
        (absolute.as_str(), "is not inside"),
        #[cfg(unix)]
        ("link.txt", "is not inside"),
        ("sub", "is not a file"),
        ("nothing.txt", "cannot read"),
        ("big.bin", "over the limit"),
    ] {
        let e = rig.err(json!({"push": name})).await;
        assert_eq!(e.code, -32025, "{name}: {e:?}");
        assert!(e.message.contains(why), "{name}: {}", e.message);
    }
    assert!(rig.transfers().is_empty(), "nothing reached the device");
    let r = rig.ok(json!({"push": "sub/in.txt"})).await;
    assert_eq!(r["device_path"], "/sdcard/Download/in.txt");

    let e = rig
        .d
        .call("default", "files", json!({"push": "sub/in.txt"}))
        .await
        .expect_err("no folder");
    assert!(e.message.contains("PHONEDRIVER_FILES"), "{}", e.message);
    let e = rig
        .d
        .call(
            "default",
            "files",
            json!({"push": "in.txt", "folder": "sub"}),
        )
        .await
        .expect_err("a relative folder");
    assert!(e.message.contains("PHONEDRIVER_FILES"), "{}", e.message);
}

#[tokio::test(start_paused = true)]
async fn a_pull_takes_from_the_downloads_or_the_photos_and_replaces_nothing() {
    let rig = setup("fake:1");
    rig.fake.with_file("/sdcard/Download/report.pdf", b"abc");
    rig.fake.with_file("/sdcard/DCIM/Camera/shot.jpg", b"abc");
    rig.fake
        .with_file("/sdcard/Android/data/com.bank/token", b"abc");
    let r = rig.ok(json!({"pull": "Download/report.pdf"})).await;
    assert_eq!(
        r,
        json!({"pulled": "Download/report.pdf", "file": "report.pdf", "bytes": 3, "sha256": ABC})
    );
    assert_eq!(
        std::fs::read(rig.folder.path().join("report.pdf")).unwrap(),
        b"abc"
    );
    rig.ok(json!({"pull": "/sdcard/DCIM/Camera/shot.jpg"}))
        .await;
    let before = rig.transfers().len();
    for (path, why) in [
        (
            "Android/data/com.bank/token",
            "under Download, Pictures, DCIM",
        ),
        ("/sdcard/Android/data/com.bank/token", "under Download"),
        ("Download/../Android/data/com.bank/token", "under Download"),
        ("/data/data/com.bank/databases/a.db", "under Download"),
        ("Download/report.pdf", "is already in"),
    ] {
        let e = rig.err(json!({"pull": path})).await;
        assert_eq!(e.code, -32025, "{path}: {e:?}");
        assert!(e.message.contains(why), "{path}: {}", e.message);
    }
    assert_eq!(
        rig.transfers().len(),
        before,
        "none of those reached the device"
    );
    let e = rig.err(json!({"pull": "Download/missing.pdf"})).await;
    assert!(e.message.contains("is not on the device"), "{}", e.message);
    assert!(!rig.folder.path().join("missing.pdf").exists());

    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        rig.fake.with_file("/sdcard/Download/link.pdf", b"abc");
        std::os::unix::fs::symlink(
            outside.path().join("made.pdf"),
            rig.folder.path().join("link.pdf"),
        )
        .unwrap();
        let e = rig.err(json!({"pull": "Download/link.pdf"})).await;
        assert!(e.message.contains("is already in"), "{}", e.message);
        assert!(
            !outside.path().join("made.pdf").exists(),
            "a link is not followed out"
        );
    }
}

/// A physical phone is a person's: every transfer is held for their word, and their word covers the one transfer
/// they were shown.
#[tokio::test(start_paused = true)]
async fn on_a_physical_device_every_transfer_asks_and_an_approval_covers_the_one_it_was_shown() {
    let rig = setup("android:R5CT123");
    std::fs::write(rig.folder.path().join("cat.png"), b"abc").unwrap();
    std::fs::write(rig.folder.path().join("dog.png"), b"abcd").unwrap();
    rig.fake.with_file("/sdcard/Download/report.pdf", b"abc");

    let e = rig.err(json!({"push": "cat.png"})).await;
    assert_eq!(e.code, -32019, "{e:?}");
    assert_eq!(e.data["category"], "files");
    let shown = json!({"label": e.data["label"], "details": e.data["details"]});
    assert!(
        e.data["label"].as_str().unwrap().contains("cat.png"),
        "{}",
        e.data
    );
    assert!(e.data["details"].to_string().contains(ABC), "{}", e.data);
    assert!(rig.transfers().is_empty());

    let e = rig
        .err(json!({"push": "dog.png", "approval": "once", "approved": shown}))
        .await;
    assert_eq!(e.code, -32019, "another file than the one approved: {e:?}");
    let e = rig
        .err(json!({"push": "cat.png", "approval": "once"}))
        .await;
    assert_eq!(e.code, -32019, "an approval that shows nothing: {e:?}");
    assert!(rig.transfers().is_empty());

    rig.ok(json!({"push": "cat.png", "approval": "session", "approved": shown}))
        .await;
    assert_eq!(rig.transfers().len(), 1);
    let e = rig.err(json!({"push": "dog.png"})).await;
    assert_eq!(
        e.code, -32019,
        "a session approval is not for the next file: {e:?}"
    );

    let e = rig.err(json!({"pull": "Download/report.pdf"})).await;
    assert_eq!(e.code, -32019, "{e:?}");
    assert!(
        e.data["label"]
            .as_str()
            .unwrap()
            .contains("copy Download/report.pdf from the phone"),
        "{}",
        e.data
    );
    assert!(!rig.folder.path().join("report.pdf").exists());
    let shown = json!({"label": e.data["label"], "details": e.data["details"]});
    rig.ok(json!({"pull": "Download/report.pdf", "approval": "once", "approved": shown}))
        .await;
    rig.ok(json!({"push": "dog.png", "approval": "autonomous"}))
        .await;
    assert_eq!(rig.transfers().len(), 3);
}

#[tokio::test(start_paused = true)]
async fn a_saved_push_is_a_step_with_the_name_and_no_folder_and_a_pull_is_not_a_step() {
    let rig = setup("fake:1");
    std::fs::write(rig.folder.path().join("cat.png"), b"abc").unwrap();
    rig.fake.with_file("/sdcard/Download/report.pdf", b"abc");
    rig.ok(json!({"push": "cat.png"})).await;
    rig.ok(json!({"pull": "Download/report.pdf"})).await;
    let j = rig.d.call("default", "journal", json!({})).await.unwrap();
    let steps = j["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 1, "{j}");
    assert_eq!(steps[0]["method"], "files");
    assert_eq!(
        steps[0]["params"],
        json!({"push": "cat.png", "to": "photos"})
    );
}

#[tokio::test(start_paused = true)]
async fn a_call_names_one_transfer_or_one_listing() {
    let rig = setup("fake:1");
    for params in [
        json!({"push": "a", "pull": "Download/b"}),
        json!({"push": "a", "list": "Download"}),
        json!({"pull": "Download/b", "list": "Download"}),
    ] {
        let e = rig.err(params).await;
        assert_eq!(e.code, -32602, "{e:?}");
    }
}

/// Decision D129: a pull names a path, so an agent has to be able to see what is there. A call with no file lists
/// the three shelves, the newest files of each, with a size and a time and never a byte of a file.
#[tokio::test(start_paused = true)]
async fn a_call_with_no_file_lists_the_newest_files_of_each_shelf_and_no_contents() {
    let rig = setup("fake:1");
    rig.fake
        .with_file_aged("/sdcard/Download/report.pdf", b"BYTESOFTHEREPORT", 120);
    rig.fake
        .with_file_aged("/sdcard/Pictures/cat.png", b"BYTESOFTHECAT", 30);
    for n in 0..14 {
        rig.fake.with_file_aged(
            &format!("/sdcard/DCIM/Camera/IMG_{n:02}.jpg"),
            b"BYTESOFAPHOTO",
            1000 + n,
        );
    }
    rig.fake
        .with_file_aged("/sdcard/Android/data/com.bank/token", b"BYTESOFATOKEN", 1);
    rig.fake.with_file_aged("/sdcard/Music/song.mp3", b"abc", 1);

    let r = rig.ok(json!({})).await;
    let paths: Vec<&str> = r["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths.len(), 12, "{r}");
    assert_eq!(
        paths[..4],
        [
            "Pictures/cat.png",
            "Download/report.pdf",
            "DCIM/Camera/IMG_00.jpg",
            "DCIM/Camera/IMG_01.jpg"
        ],
        "newest first: {r}"
    );
    assert_eq!(paths[11], "DCIM/Camera/IMG_09.jpg", "{r}");
    assert_eq!(r["more"], 4, "{r}");
    assert_eq!(r["now"], FakeDevice::FILES_NOW, "{r}");
    assert_eq!(
        r["files"][1],
        json!({"path": "Download/report.pdf", "bytes": 16, "modified": FakeDevice::FILES_NOW - 120})
    );
    assert!(!r.to_string().contains("BYTESOF"), "{r}");
    assert!(!r.to_string().contains("token"), "{r}");
    assert!(rig.transfers().is_empty());
    assert_eq!(everything_under(rig.folder.path()), "");
}

#[tokio::test(start_paused = true)]
async fn a_listing_of_one_folder_shows_more_of_it_and_only_a_shelf_or_a_folder_under_one() {
    let rig = setup("fake:1");
    rig.fake.with_file_aged("/sdcard/Download/a.pdf", b"abc", 5);
    for n in 0..60 {
        rig.fake.with_file_aged(
            &format!("/sdcard/DCIM/Camera/IMG_{n:02}.jpg"),
            b"abc",
            1000 + n,
        );
    }
    let r = rig.ok(json!({"list": "DCIM/Camera"})).await;
    let files = r["files"].as_array().unwrap();
    assert_eq!(files.len(), 50, "{r}");
    assert_eq!(files[0]["path"], "DCIM/Camera/IMG_00.jpg");
    assert_eq!(r["more"], 10, "{r}");
    assert!(
        rig.fake
            .calls()
            .contains(&Call::FileList(Some("DCIM/Camera".into())))
    );

    for refused in ["Android/data", "Download/../Android", "/data/data", ""] {
        let e = rig.err(json!({"list": refused})).await;
        assert_eq!(e.code, -32025, "{refused}: {e:?}");
    }
    let r = rig.ok(json!({"list": "Pictures"})).await;
    assert_eq!(r["files"], json!([]), "{r}");
    assert_eq!(r["more"], 0, "{r}");
}

/// The switch for files is the folder the person named: with none, nothing of the phone's files is told either.
#[tokio::test(start_paused = true)]
async fn a_listing_is_refused_until_the_person_names_a_folder() {
    let rig = setup("fake:1");
    rig.fake.with_file_aged("/sdcard/Download/a.pdf", b"abc", 5);
    let e = rig
        .d
        .call("default", "files", json!({}))
        .await
        .expect_err("no folder");
    assert_eq!(e.code, -32025, "{e:?}");
    assert!(e.message.contains("PHONEDRIVER_FILES"), "{e:?}");
    assert!(
        !rig.fake
            .calls()
            .iter()
            .any(|c| matches!(c, Call::FileList(_)))
    );
}

/// The names of a person's photos and downloads are theirs. On their phone a listing waits for their word; a word
/// for the session covers later listings and no transfer.
#[tokio::test(start_paused = true)]
async fn on_a_physical_device_a_listing_asks_and_a_session_approval_covers_listings_only() {
    let rig = setup("android:R5CT123");
    std::fs::write(rig.folder.path().join("cat.png"), b"abc").unwrap();
    rig.fake.with_file_aged("/sdcard/Download/a.pdf", b"abc", 5);
    let listed = || {
        rig.fake
            .calls()
            .iter()
            .filter(|c| matches!(c, Call::FileList(_)))
            .count()
    };

    let e = rig.err(json!({})).await;
    assert_eq!(e.code, -32019, "{e:?}");
    assert_eq!(e.data["category"], "files");
    assert!(
        e.data["label"].as_str().unwrap().contains("list the names"),
        "{}",
        e.data
    );
    assert_eq!(listed(), 0);
    let shown = json!({"label": e.data["label"], "details": e.data["details"]});

    let e = rig.err(json!({"approval": "once"})).await;
    assert_eq!(e.code, -32019, "an approval that shows nothing: {e:?}");
    rig.ok(json!({"approval": "once", "approved": shown})).await;
    assert_eq!(listed(), 1);
    let e = rig.err(json!({})).await;
    assert_eq!(e.code, -32019, "once is once: {e:?}");

    rig.ok(json!({"approval": "session", "approved": shown}))
        .await;
    rig.ok(json!({})).await;
    rig.ok(json!({"list": "Download"})).await;
    assert_eq!(listed(), 4);

    let e = rig.err(json!({"push": "cat.png"})).await;
    assert_eq!(e.code, -32019, "a listing's word moves no file: {e:?}");
    let e = rig
        .err(json!({"push": "cat.png", "approval": "session", "approved": shown}))
        .await;
    assert_eq!(
        e.code, -32019,
        "nor when it is sent again with a transfer: {e:?}"
    );
    let e = rig.err(json!({"pull": "Download/a.pdf"})).await;
    assert_eq!(e.code, -32019, "{e:?}");
    assert!(rig.transfers().is_empty());
}
