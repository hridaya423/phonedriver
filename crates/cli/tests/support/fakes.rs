#![allow(dead_code)]

use std::sync::Arc;

use async_trait::async_trait;
use phonedriver::daemon::{DeviceSource, Listed};
use phonedriver_core::device::{
    App, AppId, Button, Capabilities, ClipboardOp, Device, DeviceEvent, DeviceInfo, FrameRef,
    Gesture, Image, ImageFormat, Key, Quality, TextInput, Url,
};
use phonedriver_core::error::Error;
use phonedriver_core::geometry::Scale;
use phonedriver_core::raw::RawTree;
use phonedriver_core::testing::FakeDevice;

pub const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x38, 0x71, 0xe2, 0x04,
    0x00, 0x04, 0xb4, 0x02, 0x59, 0x16, 0x2e, 0x81, 0x40, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
    0x44, 0xae, 0x42, 0x60, 0x82,
];

pub fn tree(labels: &[&str]) -> RawTree {
    let mut nodes = vec![
        r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}"#
            .to_string(),
    ];
    for (i, l) in labels.iter().enumerate() {
        let top = 100 + 100 * i as i32;
        nodes.push(format!(
            r#"{{"p":0,"w":1,"cls":"android.widget.Button","text":"{l}","b":[0,{top},360,{}],"f":"vVe","a":["tap"]}}"#,
            top + 60
        ));
    }
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[{}]}}"#,
        nodes.join(",")
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

pub fn field_tree(value: &str, password: bool) -> RawTree {
    let flags = if password { "vVeEFP" } else { "vVeEF" };
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[
        {{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},
        {{"p":0,"w":1,"cls":"android.widget.EditText","text":"{value}","desc":"Field","b":[0,100,360,160],"f":"{flags}","a":["tap","set_text"]}}]}}"#
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

pub struct Pixels(pub Arc<FakeDevice>, pub DeviceInfo);

#[async_trait]
impl Device for Pixels {
    fn info(&self) -> &DeviceInfo {
        &self.1
    }
    async fn tree(&self) -> Result<RawTree, Error> {
        self.0.tree().await
    }
    fn frames(&self) -> Option<tokio::sync::watch::Receiver<FrameRef>> {
        self.0.frames()
    }
    async fn screenshot(&self, _q: Quality) -> Result<Image, Error> {
        Ok(Image {
            width: 360,
            height: 800,
            format: ImageFormat::Png,
            bytes: PNG.to_vec(),
        })
    }
    fn events(&self) -> tokio::sync::broadcast::Receiver<DeviceEvent> {
        self.0.events()
    }
    async fn gesture(&self, g: Gesture) -> Result<(), Error> {
        self.0.gesture(g).await
    }
    async fn key(&self, k: Key) -> Result<(), Error> {
        self.0.key(k).await
    }
    async fn button(&self, b: Button) -> Result<(), Error> {
        self.0.button(b).await
    }
    async fn open_url(&self, u: &Url, app: Option<&AppId>) -> Result<Option<AppId>, Error> {
        self.0.open_url(u, app).await
    }
    async fn text(&self, t: TextInput) -> Result<(), Error> {
        self.0.text(t).await
    }
    async fn clipboard(&self, op: ClipboardOp) -> Result<Option<String>, Error> {
        self.0.clipboard(op).await
    }
    async fn apps(&self) -> Result<Vec<App>, Error> {
        self.0.apps().await
    }
    async fn launch(&self, app: &AppId) -> Result<(), Error> {
        self.0.launch(app).await
    }
    async fn foreground(&self) -> Result<Option<AppId>, Error> {
        self.0.foreground().await
    }
    async fn shell(
        &self,
        req: &phonedriver_core::device::ShellRequest,
    ) -> Result<phonedriver_core::device::ShellReply, Error> {
        self.0.shell(req).await
    }
}

pub struct OneDevice(pub Arc<dyn Device>);

#[async_trait]
impl DeviceSource for OneDevice {
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        Ok(vec![Listed {
            id: "fake:1".into(),
            model: "Fake".into(),
            os: Some("FakeOS 1".into()),
            tier: "fake".into(),
            capabilities: self.0.info().capabilities,
        }])
    }
    async fn connect(&self, _id: &str) -> Result<Arc<dyn Device>, Error> {
        Ok(self.0.clone())
    }
}

pub fn pixel_device(trees: Vec<RawTree>) -> Arc<dyn Device> {
    let fake = Arc::new(FakeDevice::new(trees));
    let mut info = fake.info().clone();
    info.capabilities |= Capabilities::SCREENSHOT;
    Arc::new(Pixels(fake, info))
}
