//! The daemon's local socket: a Unix socket under a private directory, or a named pipe on Windows.

use std::io;
use std::path::{Path, PathBuf};

use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName};

pub use interprocess::local_socket::tokio::{Listener, Stream};

/// `PHONEDRIVER_SOCKET`, else under `PHONEDRIVER_HOME`, else `$XDG_RUNTIME_DIR/phonedriver`, else `~/.phonedriver/run`.
pub fn socket_path() -> PathBuf {
    if let Some(p) = std::env::var_os("PHONEDRIVER_SOCKET") {
        return PathBuf::from(p);
    }
    #[cfg(windows)]
    {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".into());
        let scope = std::env::var_os("PHONEDRIVER_HOME")
            .map(|h| {
                format!(
                    "-{:x}",
                    h.to_string_lossy()
                        .bytes()
                        .fold(0u64, |a, b| a.wrapping_mul(131).wrapping_add(u64::from(b)))
                )
            })
            .unwrap_or_default();
        PathBuf::from(format!(r"\\.\pipe\phonedriver-{user}{scope}"))
    }
    #[cfg(not(windows))]
    {
        if std::env::var_os("PHONEDRIVER_HOME").is_none()
            && let Some(x) = std::env::var_os("XDG_RUNTIME_DIR")
        {
            return PathBuf::from(x).join("phonedriver").join("daemon.sock");
        }
        fit_sun_path(home().join("run").join("daemon.sock"))
    }
}

/// `sun_path` holds 104 bytes on macOS and 108 on Linux, terminator included.
#[cfg(unix)]
const SUN_PATH_MAX: usize = 100;

/// A socket path that fits `sun_path`: the path itself, else a hash of it in a private directory under `$TMPDIR`
/// (or `/tmp`), so a long `PHONEDRIVER_HOME` works and the client and the daemon still compute the same name.
#[cfg(unix)]
fn fit_sun_path(path: PathBuf) -> PathBuf {
    let tmp = std::env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    let user = std::env::var("USER").unwrap_or_default();
    short_socket_path(path, &tmp, &user)
}

#[cfg(unix)]
fn short_socket_path(path: PathBuf, tmp: &Path, user: &str) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    if path.as_os_str().len() <= SUN_PATH_MAX {
        return path;
    }
    let hash = user
        .bytes()
        .chain(std::iter::once(0))
        .chain(path.as_os_str().as_bytes().iter().copied())
        .fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
    let name = format!("phonedriver-{hash:016x}/daemon.sock");
    let candidate = tmp.join(&name);
    if candidate.as_os_str().len() <= SUN_PATH_MAX {
        candidate
    } else {
        PathBuf::from("/tmp").join(name)
    }
}

/// `PHONEDRIVER_HOME`, else `~/.phonedriver`.
pub fn home() -> PathBuf {
    if let Some(h) = std::env::var_os("PHONEDRIVER_HOME") {
        return PathBuf::from(h);
    }
    let base = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    base.join(".phonedriver")
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
                    "a phonedriver daemon is already listening",
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_short_path_is_kept_and_a_long_one_moves_to_a_short_private_name() {
        let tmp = Path::new("/var/folders/ab/cdefgh/T");
        let short = PathBuf::from("/tmp/pu-ax2/run/daemon.sock");
        assert_eq!(short_socket_path(short.clone(), tmp, "me"), short);
        let long = PathBuf::from(format!("/Users/me/{}/run/daemon.sock", "deep/".repeat(30)));
        let fitted = short_socket_path(long.clone(), tmp, "me");
        assert!(fitted.as_os_str().len() <= SUN_PATH_MAX, "{fitted:?}");
        assert!(fitted.starts_with(tmp));
        assert_eq!(fitted, short_socket_path(long.clone(), tmp, "me"));
        assert_ne!(fitted, short_socket_path(long, tmp, "someone else"));
    }

    #[test]
    fn a_long_temp_directory_falls_back_to_slash_tmp() {
        let tmp = PathBuf::from(format!("/{}", "t".repeat(120)));
        let long = PathBuf::from(format!("/{}/run/daemon.sock", "h".repeat(120)));
        assert!(short_socket_path(long, &tmp, "me").starts_with("/tmp"));
    }

    #[tokio::test]
    async fn a_daemon_listens_and_a_client_connects_under_a_home_too_long_for_sun_path() {
        let base = std::env::temp_dir().join(format!("pu-long-{}", std::process::id()));
        let home = base.join("h".repeat(110));
        let path = short_socket_path(
            home.join("run").join("daemon.sock"),
            &std::env::temp_dir(),
            "t",
        );
        assert!(path.as_os_str().len() <= SUN_PATH_MAX);
        let listener = listen(&path).await.unwrap();
        let accept = tokio::spawn(async move { listener.accept().await.is_ok() });
        connect(&path).await.unwrap();
        assert!(accept.await.unwrap());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
