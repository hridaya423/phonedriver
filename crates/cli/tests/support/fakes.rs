#![allow(dead_code)]

use std::sync::Arc;

use async_trait::async_trait;
use phone_use::daemon::{DeviceSource, Listed};
use phone_use_core::device::{
    App, AppId, Button, Capabilities, ClipboardOp, Device, DeviceEvent, DeviceInfo, FrameRef,
    Gesture, Image, ImageFormat, Key, Quality, TextInput, Url,
};
use phone_use_core::error::Error;
use phone_use_core::geometry::Scale;
use phone_use_core::raw::RawTree;
use phone_use_core::testing::FakeDevice;

pub const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52,
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
    async fn open_url(&self, u: &Url) -> Result<(), Error> {
        self.0.open_url(u).await
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
