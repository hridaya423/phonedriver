//! Several device sources behind one: adb and simulators on a Mac.

use std::sync::Arc;

use async_trait::async_trait;
use phone_use_core::device::Device;
use phone_use_core::error::Error;

use super::{DeviceSource, Listed};

/// Each source owns the ids that start with its prefix (`android:`, `sim:`).
pub struct Sources(pub Vec<(&'static str, Arc<dyn DeviceSource>)>);

#[async_trait]
impl DeviceSource for Sources {
    /// The union of every source that answered. A source that fails (no adb server, no Xcode) is left out,
    /// unless every source failed; then the first error is the answer.
    async fn list(&self) -> Result<Vec<Listed>, Error> {
        let mut out = Vec::new();
        let mut first_error = None;
        let mut answered = false;
        for (_, source) in &self.0 {
            match source.list().await {
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

    async fn connect(&self, id: &str) -> Result<Arc<dyn Device>, Error> {
        let source = self
            .0
            .iter()
            .find(|(prefix, _)| id.starts_with(prefix))
            .map(|(_, s)| s)
            .ok_or_else(|| Error::DeviceGone { id: id.to_string() })?;
        source.connect(id).await
    }

    fn blocker(&self, id: &str) -> Option<&'static str> {
        self.0
            .iter()
            .find(|(prefix, _)| id.starts_with(prefix))
            .and_then(|(_, s)| s.blocker(id))
    }
}
