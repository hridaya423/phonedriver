//! Several device sources behind one: adb and simulators on a Mac.

use std::sync::Arc;

use async_trait::async_trait;
use phonedriver_core::device::{Device, DisplayMode};
use phonedriver_core::error::{DeviceSummary, Error};

use super::{Booted, DeviceSource, Listed};

/// Each source owns the ids that start with its prefix (`android:`, `sim:`).
pub struct Sources(pub Vec<(&'static str, Arc<dyn DeviceSource>)>);

#[async_trait]
impl DeviceSource for Sources {
    /// The union of every source that answered. A source that fails (no adb server, no Xcode) is left out,
    /// unless every source failed; then the first error is the answer.
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        self.list_except(&[]).await
    }

    async fn list_except(&self, denied: &[String]) -> Result<Vec<Listed>, Error> {
        let mut out = Vec::new();
        let mut first_error = None;
        let mut answered = false;
        for (_, source) in &self.0 {
            match source.list_except(denied).await {
                Ok(list) => {
                    answered = true;
                    out.extend(list);
                }
                Err(e) => {
                    first_error.get_or_insert(e);
                }
            }
        }
        match (answered, first_error) {
            (false, Some(e)) => Err(e),
            _ => Ok(out),
        }
    }

    /// A prefixed id (`android:emulator-5554`, `sim:<udid>`, `ios:<udid>`) asks the one source that owns it. A bare
    /// id (`emulator-5554`) asks the sources in order and stops at the first that has it, so the Android serial a
    /// caller usually means never waits on the iOS sources. The audit measured 2.05 s per call on Windows from
    /// listing physical iOS devices for an Android request. When no source that answered has it and one could not
    /// be asked, the answer says so: with the adb server unreachable and simulators present, a bare
    /// `emulator-5554` was answered with `no device "emulator-5554"; connected: sim:...` and no word about adb.
    async fn list_for(&self, requested: Option<&str>) -> Result<Vec<Listed>, Error> {
        let Some(requested) = requested else {
            return self.list().await;
        };
        if let Some((_, owner)) = self
            .0
            .iter()
            .find(|(prefix, _)| requested.starts_with(prefix))
        {
            return owner.list_for(Some(requested)).await;
        }
        let mut out = Vec::new();
        let mut errors = Vec::new();
        let mut answered = false;
        for (_, source) in &self.0 {
            match source.list_for(Some(requested)).await {
                Ok(list) => {
                    answered = true;
                    let found = list.iter().any(|l| super::names(&l.id, requested));
                    out.extend(list);
                    if found {
                        return Ok(out);
                    }
                }
                Err(e) => errors.push(e),
            }
        }
        if !answered && let Some(e) = errors.first() {
            return Err(e.clone());
        }
        if errors.is_empty() {
            return Ok(out);
        }
        Err(Error::NoDevice {
            requested: Some(requested.to_string()),
            connected: out
                .into_iter()
                .map(|l| DeviceSummary {
                    id: l.id,
                    model: l.model,
                    os: l.os.unwrap_or_default(),
                })
                .collect(),
            unasked: errors.iter().map(Error::to_string).collect(),
            off: vec![],
        })
    }

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        let source = self
            .0
            .iter()
            .find(|(prefix, _)| id.starts_with(prefix))
            .map(|(_, s)| s)
            .ok_or_else(|| Error::DeviceGone { id: id.to_string() })?;
        source.connect(id).await
    }

    async fn connect_on(&self, id: &str, display: DisplayMode) -> Result<Arc<dyn Device>, Error> {
        let source = self
            .0
            .iter()
            .find(|(prefix, _)| id.starts_with(prefix))
            .map(|(_, s)| s)
            .ok_or_else(|| Error::DeviceGone { id: id.to_string() })?;
        source.connect_on(id, display).await
    }

    async fn off(&self) -> Vec<DeviceSummary> {
        let mut out = Vec::new();
        for (_, source) in &self.0 {
            out.extend(source.off().await);
        }
        out
    }

    async fn boot(&self, name: &str) -> Result<Option<Booted>, Error> {
        for (_, source) in &self.0 {
            if let Some(booted) = source.boot(name).await? {
                return Ok(Some(booted));
            }
        }
        Ok(None)
    }

    fn blocker(&self, id: &str) -> Option<&'static str> {
        self.0
            .iter()
            .find(|(prefix, _)| id.starts_with(prefix))
            .and_then(|(_, s)| s.blocker(id))
    }

    async fn hardware_serial(&self, id: &str) -> Result<Option<String>, Error> {
        match self.0.iter().find(|(prefix, _)| id.starts_with(prefix)) {
            Some((_, source)) => source.hardware_serial(id).await,
            None => Ok(None),
        }
    }
}
