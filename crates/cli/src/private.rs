//! Folders and files that hold what the phone showed (screenshots, recordings, traces): owner-only on Unix, folders
//! 0700 and files 0600 (decision D105). Elsewhere they keep the platform's defaults.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

/// Creates `dir` and any missing parents owner-only, and makes `dir` itself owner-only if it was there already.
pub fn dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(dir)
    }
}

/// Options that create a file owner-only.
pub fn options() -> OpenOptions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut options = OpenOptions::new();
        options.mode(0o600);
        options
    }
    #[cfg(not(unix))]
    {
        OpenOptions::new()
    }
}

/// Writes `bytes` to a new owner-only file at `path`, replacing one that is there.
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut file: File = options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    file.write_all(bytes)
}
