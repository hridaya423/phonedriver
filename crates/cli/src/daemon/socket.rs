//! The daemon's local socket: a Unix socket under a private directory, or a named pipe on Windows.

use std::io;
use std::path::{Path, PathBuf};

use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName};

pub use interprocess::local_socket::tokio::{Listener, Stream};

/// `PHONE_USE_SOCKET`, else under `PHONE_USE_HOME`, else `$XDG_RUNTIME_DIR/phone-use`, else `~/.phone-use/run`.
pub fn socket_path() -> PathBuf {
    if let Some(p) = std::env::var_os("PHONE_USE_SOCKET") {
        return PathBuf::from(p);
    }
    #[cfg(windows)]
    {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".into());
        let scope = std::env::var_os("PHONE_USE_HOME")
            .map(|h| {
                format!(
                    "-{:x}",
                    h.to_string_lossy()
                        .bytes()
                        .fold(0u64, |a, b| a.wrapping_mul(131).wrapping_add(u64::from(b)))
                )
            })
            .unwrap_or_default();
        PathBuf::from(format!(r"\\.\pipe\phone-use-{user}{scope}"))
    }
    #[cfg(not(windows))]
    {
        if std::env::var_os("PHONE_USE_HOME").is_none()
            && let Some(x) = std::env::var_os("XDG_RUNTIME_DIR")
        {
            return PathBuf::from(x).join("phone-use").join("daemon.sock");
        }
        home().join("run").join("daemon.sock")
    }
}

/// `PHONE_USE_HOME`, else `~/.phone-use`.
pub fn home() -> PathBuf {
    if let Some(h) = std::env::var_os("PHONE_USE_HOME") {
        return PathBuf::from(h);
    }
    let base = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    base.join(".phone-use")
}

fn name(path: &Path) -> io::Result<interprocess::local_socket::Name<'_>> {
    path.to_fs_name::<GenericFilePath>()
}

pub async fn connect(path: &Path) -> io::Result<Stream> {
    Stream::connect(name(path)?).await
}

/// Binds the socket. A leftover file from a dead daemon is replaced; a live daemon is an error.
pub async fn listen(path: &Path) -> io::Result<Listener> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        if let Some(dir) = path.parent() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)?;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
        if path.exists() {
            if connect(path).await.is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "a phone-use daemon is already listening",
                ));
            }
            std::fs::remove_file(path)?;
        }
    }
    let listener = ListenerOptions::new().name(name(path)?).create_tokio()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(listener)
}
