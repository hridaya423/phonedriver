//! The folders of a device's shared storage that files go to and come from (decision D119).

use serde::{Deserialize, Serialize};

/// Where a pushed file goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shelf {
    Photos,
    Downloads,
}

impl Shelf {
    /// The folder on Android's shared storage.
    pub fn folder(self) -> &'static str {
        match self {
            Shelf::Photos => "Pictures",
            Shelf::Downloads => "Download",
        }
    }

    /// Pictures and videos go to the photos; everything else to the downloads.
    pub fn of(name: &str) -> Shelf {
        const SEEN: [&str; 12] = [
            "jpg", "jpeg", "png", "gif", "webp", "heic", "heif", "bmp", "mp4", "mov", "webm", "mkv",
        ];
        match name.rsplit_once('.') {
            Some((_, ext)) if SEEN.contains(&ext.to_ascii_lowercase().as_str()) => Shelf::Photos,
            _ => Shelf::Downloads,
        }
    }
}

/// The folders a file may be pulled from: the downloads and the two photo folders.
pub const PULL_FROM: [&str; 3] = ["Download", "Pictures", "DCIM"];

/// The name a file gets on the device, if it is one: a single path component with no control character, that no
/// shell and no file system reads as anything else.
pub fn name(name: &str) -> Result<&str, String> {
    let bad = name.is_empty()
        || name.len() > 200
        || name.starts_with('.')
        || name.starts_with('-')
        || name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | '\'' | '"' | '`' | '$' | '\0'));
    if bad {
        return Err(format!(
            "{name:?} cannot be a file's name on the device: it must not be empty, start with `.` or `-`, be over 200 bytes, or hold a slash, a quote, `$` or a control character"
        ));
    }
    Ok(name)
}

/// `path` as a path under shared storage that a pull may read: `Download/a.pdf`, `/sdcard/DCIM/Camera/b.jpg`.
/// Anything outside the three folders, or reaching out of them with `..`, is refused.
pub fn shared(path: &str) -> Result<String, String> {
    let inside = ["/sdcard/", "/storage/emulated/0/"]
        .iter()
        .find_map(|root| path.strip_prefix(root))
        .unwrap_or(path);
    let parts: Vec<&str> = inside.split('/').collect();
    let allowed =
        parts.len() >= 2 && PULL_FROM.contains(&parts[0]) && parts.iter().all(|p| name(p).is_ok());
    if !allowed {
        return Err(format!(
            "{path:?} is not a file a pull may read: it takes a path under {}, such as `Download/report.pdf`",
            PULL_FROM.join(", ")
        ));
    }
    Ok(format!("/sdcard/{}", parts.join("/")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pull_reads_under_the_downloads_and_the_photo_folders_only() {
        assert_eq!(shared("Download/a.pdf").unwrap(), "/sdcard/Download/a.pdf");
        assert_eq!(
            shared("/sdcard/DCIM/Camera/b c.jpg").unwrap(),
            "/sdcard/DCIM/Camera/b c.jpg"
        );
        assert_eq!(
            shared("/storage/emulated/0/Pictures/x.png").unwrap(),
            "/sdcard/Pictures/x.png"
        );
        for refused in [
            "Download",
            "Download/",
            "Android/data/com.bank/files/token",
            "Download/../Android/data/x",
            "Download/./a",
            "/data/data/com.bank/databases/a.db",
            "Download/a'; rm -rf x",
            "Pictures/$(id)",
            "Pictures//x.png",
            "download/a.pdf",
            "Download/.hidden",
            "/sdcard/../data/x",
        ] {
            assert!(shared(refused).is_err(), "{refused}");
        }
    }

    #[test]
    fn a_picture_goes_to_the_photos_and_anything_else_to_the_downloads() {
        assert_eq!(Shelf::of("cat.JPG"), Shelf::Photos);
        assert_eq!(Shelf::of("clip.mp4"), Shelf::Photos);
        assert_eq!(Shelf::of("report.pdf"), Shelf::Downloads);
        assert_eq!(Shelf::of("README"), Shelf::Downloads);
    }
}
