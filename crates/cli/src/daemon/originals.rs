//! The value each setting had before phonedriver first wrote it, one file per device in `settings-before/` under the
//! daemon's home (decision D120). A file, so that what an agent or a daemon that died changed can still be put back.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One setting phonedriver changed: what it was before the first write, and what the last write left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Original {
    pub was: String,
    pub set: String,
}

pub type Originals = BTreeMap<String, Original>;

fn path(home: &Path, device: &str) -> PathBuf {
    let name: String = device
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    home.join("settings-before").join(format!("{name}.json"))
}

pub fn read(home: &Path, device: &str) -> Result<Originals, String> {
    let path = path(home, device);
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| {
            format!(
                "{} is not a record phonedriver can read ({e}); fix or delete it",
                path.display()
            )
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Originals::new()),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// Replaces the device's record. An empty one is no file.
pub fn write(home: &Path, device: &str, all: &Originals) -> Result<(), String> {
    let path = path(home, device);
    let failed = |e: std::io::Error| format!("cannot write {}: {e}", path.display());
    if all.is_empty() {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(failed(e)),
            _ => Ok(()),
        };
    }
    let text = serde_json::to_vec(all).map_err(|e| failed(e.into()))?;
    let beside = path.with_extension("json.tmp");
    path.parent()
        .map_or(Ok(()), crate::private::dir)
        .and_then(|()| crate::private::write(&beside, &text))
        .and_then(|()| std::fs::rename(&beside, &path))
        .map_err(failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_is_one_file_per_device_and_an_empty_one_is_no_file() {
        let home = tempfile::tempdir().unwrap();
        let mut all = Originals::new();
        all.insert(
            "a/b".into(),
            Original {
                was: "false".into(),
                set: "true".into(),
            },
        );
        write(home.path(), "android:../R5/x", &all).unwrap();
        let file = home.path().join("settings-before/android_.._R5_x.json");
        assert!(file.is_file(), "a device's name cannot leave the folder");
        assert_eq!(read(home.path(), "android:../R5/x").unwrap(), all);
        assert!(read(home.path(), "android:other").unwrap().is_empty());
        write(home.path(), "android:../R5/x", &Originals::new()).unwrap();
        assert!(!file.exists());
        write(home.path(), "android:../R5/x", &Originals::new()).unwrap();
    }
}
