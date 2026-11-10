#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use crate::fakes::{OneDevice, Pixels};
use phonedriver::client::{Client, Failure};
use phonedriver::daemon::{Config, Daemon, socket};
use phonedriver_core::device::{Capabilities, Device, Gesture};
use phonedriver_core::geometry::Scale;
use phonedriver_core::raw::RawTree;
use phonedriver_core::testing::{Call, FakeDevice};
use serde_json::{Value, json};

pub const PLACE_ORDER: &str = "Place order · $34.10";

pub fn checkout() -> RawTree {
    let node = |cls: &str, text: &str, top: i32, flags: &str, acts: &str| {
        format!(
            r#"{{"p":0,"w":1,"cls":"{cls}","text":"{text}","b":[0,{top},360,{}],"f":"{flags}","a":[{acts}]}}"#,
            top + 50
        )
    };
    let nodes = [
        r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}"#
            .to_string(),
        node("android.widget.TextView", "2 x Pad thai", 100, "vVe", ""),
        node(
            "android.widget.TextView",
            "Deliver to 12 Elm St",
            150,
            "vVe",
            "",
        ),
        node("android.widget.TextView", "Total $34.10", 200, "vVe", ""),
        node("android.widget.Button", "Back", 300, "vVe", r#""tap""#),
        node("android.widget.Button", PLACE_ORDER, 400, "vVe", r#""tap""#),
        node("android.widget.Button", "Send", 0, "vVe", r#""tap""#),
        node(
            "android.widget.Button",
            "Delete photo",
            50,
            "vVe",
            r#""tap""#,
        ),
        node("android.widget.Button", "Install", 550, "vVe", r#""tap""#),
        node(
            "android.widget.EditText",
            "",
            500,
            "vVeEF",
            r#""tap","set_text""#,
        ),
    ];
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example.shop","bounds":[0,0,360,800]}}],"nodes":[{}]}}"#,
        nodes.join(",")
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

pub fn composer() -> RawTree {
    let json = r#"{"windows":[{"id":1,"type":"application","package":"com.example.chat","bounds":[0,0,360,800]}],"nodes":[
        {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.widget.EditText","text":"","b":[0,500,300,550],"f":"vVeEF","a":["tap","set_text"]},
        {"p":0,"w":1,"cls":"android.widget.Button","text":"Send","b":[300,500,360,550],"f":"vVe","a":["tap"]}]}"#;
    RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap()
}

pub struct Rig {
    pub fake: Arc<FakeDevice>,
    pub socket: PathBuf,
    pub _home: tempfile::TempDir,
}

pub async fn rig() -> Rig {
    rig_showing(checkout()).await
}

pub async fn rig_showing(tree: RawTree) -> Rig {
    let fake = Arc::new(FakeDevice::new(vec![tree]));
    let mut info = fake.info().clone();
    info.capabilities |= Capabilities::SCREENSHOT;
    let device: Arc<dyn Device> = Arc::new(Pixels(fake.clone(), info));
    let home = tempfile::tempdir().unwrap();
    let daemon = Daemon::new(
        Arc::new(OneDevice(device)),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    let socket = test_socket(home.path(), "daemon.sock");
    let listener = socket::listen(&socket).await.unwrap();
    tokio::spawn(daemon.serve_on(listener));
    Rig {
        fake,
        socket,
        _home: home,
    }
}

impl Rig {
    pub async fn act(&self, action: Value, approval: Option<&str>) -> Result<Value, Failure> {
        self.act_asking(action, approval, &[]).await
    }

    pub async fn act_asking(
        &self,
        action: Value,
        approval: Option<&str>,
        ask: &[&str],
    ) -> Result<Value, Failure> {
        let mut c = Client::connect_to(&self.socket, "guard-test")
            .await
            .unwrap();
        let mut p = json!({"action": action, "settle": false, "ask": ask});
        if let Some(a) = approval {
            p["approval"] = json!(a);
        }
        c.call("act", p).await
    }

    pub fn gestures(&self) -> usize {
        self.fake
            .calls()
            .iter()
            .filter(|c| matches!(c, Call::Gesture(Gesture { .. })))
            .count()
    }
}

fn test_socket(home: &std::path::Path, name: &str) -> std::path::PathBuf {
    if cfg!(windows) {
        let scope = home.file_name().unwrap().to_string_lossy();
        std::path::PathBuf::from(format!(r"\\.\pipe\phonedriver-test-{scope}-{name}"))
    } else {
        home.join("run").join(name)
    }
}
