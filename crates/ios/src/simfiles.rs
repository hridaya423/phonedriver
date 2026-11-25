//! A Simulator's photo library as files of this Mac (decision D129): the library's originals are files under the
//! device's data folder, which is where a listing and a pull read them.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use phonedriver_core::files::{Entry, Listing};
use serde_json::Value;

/// Where the photo library keeps its originals, from what `simctl list devices -j` printed.
pub fn dcim(devices: &str, udid: &str) -> Option<PathBuf> {
    let devices: Value = serde_json::from_str(devices).ok()?;
    let data = devices["devices"]
        .as_object()?
        .values()
        .filter_map(|list| list.as_array())
        .flatten()
        .find(|d| d["udid"] == udid)?["dataPath"]
        .as_str()?;
    Some(Path::new(data).join("Media").join("DCIM"))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        match entry.file_type() {
            Ok(t) if t.is_dir() => walk(&entry.path(), out),
            Ok(t) if t.is_file() => out.push(entry.path()),
            _ => {}
        }
    }
}

/// The originals under `dcim`, as `DCIM/100APPLE/IMG_0001.JPG`, or those under `folder` (`DCIM/100APPLE`). The
/// Simulator has no Download or Pictures folder, so those list nothing.
pub fn list(dcim: &Path, folder: Option<&str>, now: u64) -> Listing {
    let root = match folder {
        None | Some("DCIM") => dcim.to_path_buf(),
        Some(f) => match f.strip_prefix("DCIM/") {
            Some(rest) => dcim.join(rest),
            None => return Listing { now, files: vec![] },
        },
    };
    let mut found = Vec::new();
    walk(&root, &mut found);
    let files = found
        .into_iter()
        .filter_map(|path| {
            let about = std::fs::metadata(&path).ok()?;
            let rel: Vec<&str> = path
                .strip_prefix(dcim)
                .ok()?
                .components()
                .map(|c| c.as_os_str().to_str())
                .collect::<Option<_>>()?;
            Some(Entry {
                path: format!("DCIM/{}", rel.join("/")),
                bytes: about.len(),
                modified: about
                    .modified()
                    .ok()?
                    .duration_since(UNIX_EPOCH)
                    .ok()?
                    .as_secs(),
            })
        })
        .collect();
    Listing { now, files }
}

/// The file of this Mac behind `path`, which `files::shared` made: an original under `dcim` that no link takes
/// out of it.
pub fn original(dcim: &Path, path: &str) -> Result<PathBuf, String> {
    let not_there = || {
        format!(
            "{} is not in the Simulator's photo library: a pull takes a path under DCIM that a listing shows",
            path.strip_prefix("/sdcard/").unwrap_or(path)
        )
    };
    let rest = path.strip_prefix("/sdcard/DCIM/").ok_or_else(not_there)?;
    let real = dcim.join(rest).canonicalize().map_err(|_| not_there())?;
    let inside = dcim.canonicalize().map_err(|_| not_there())?;
    if !real.starts_with(&inside) || !real.is_file() {
        return Err(not_there());
    }
    Ok(real)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_library_is_under_the_data_path_simctl_names_for_this_device() {
        let devices = r#"{"devices":{"com.apple.CoreSimulator.SimRuntime.iOS-26-5":[
            {"udid":"AAA","dataPath":"/x/Devices/AAA/data","state":"Booted"},
            {"udid":"BBB","dataPath":"/x/Devices/BBB/data","state":"Shutdown"}]}}"#;
        assert_eq!(
            dcim(devices, "BBB").unwrap(),
            Path::new("/x/Devices/BBB/data").join("Media").join("DCIM")
        );
        assert!(dcim(devices, "CCC").is_none());
    }

    #[test]
    fn a_listing_is_the_originals_and_a_pull_reads_one_of_them_only() {
        let data = tempfile::tempdir().unwrap();
        let dcim = data.path().join("Media").join("DCIM");
        std::fs::create_dir_all(dcim.join("100APPLE")).unwrap();
        std::fs::create_dir_all(dcim.join(".MISC")).unwrap();
        std::fs::write(dcim.join("100APPLE").join("IMG_0001.JPG"), b"abc").unwrap();
        std::fs::write(dcim.join(".MISC").join("x.plist"), b"abc").unwrap();
        std::fs::write(data.path().join("secret.txt"), b"abc").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            data.path().join("secret.txt"),
            dcim.join("100APPLE").join("link.jpg"),
        )
        .unwrap();

        let before = list(&dcim, None, 7);
        assert_eq!(before.now, 7);
        assert_eq!(before.files.len(), 1, "{before:?}");
        assert_eq!(before.files[0].path, "DCIM/100APPLE/IMG_0001.JPG");
        assert_eq!(before.files[0].bytes, 3);
        assert_eq!(list(&dcim, Some("DCIM/100APPLE"), 7).files.len(), 1);
        assert!(list(&dcim, Some("Download"), 7).files.is_empty());
        assert!(list(&dcim, Some("DCIM/101APPLE"), 7).files.is_empty());

        assert!(original(&dcim, "/sdcard/DCIM/100APPLE/IMG_0001.JPG").is_ok());
        for refused in [
            "/sdcard/DCIM/100APPLE/link.jpg",
            "/sdcard/DCIM/100APPLE/missing.jpg",
            "/sdcard/Download/a.pdf",
            "/sdcard/DCIM/100APPLE",
        ] {
            assert!(original(&dcim, refused).is_err(), "{refused}");
        }
    }
}
