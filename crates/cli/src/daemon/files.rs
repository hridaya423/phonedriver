//! The host side of `files` (decision D119): the one folder a person allowed, what may be read from it and what
//! may be written to it. The device side is `Device::file_push` and `file_pull`.

use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

/// The largest file a transfer takes. The daemon holds the whole file in memory to hash it and hand it to the
/// device, and no other call reaches that device until the transfer ends.
pub const LIMIT: u64 = 64 * 1024 * 1024;

const HOW: &str = "set PHONEDRIVER_FILES to the one folder files may go to and come from (an absolute path; for the MCP server, in its `env`)";

/// The folder the client's environment named, with every link in it resolved.
pub fn folder(given: Option<&Path>) -> Result<PathBuf, String> {
    match given {
        Some(folder) if folder.is_absolute() => folder.canonicalize().map_err(|e| {
            format!(
                "the folder PHONEDRIVER_FILES names, {}, cannot be used: {e}",
                folder.display()
            )
        }),
        Some(folder) => Err(format!(
            "PHONEDRIVER_FILES is {}, which is not an absolute path: {HOW}",
            folder.display()
        )),
        None => Err(format!("no folder is allowed for files: {HOW}")),
    }
}

/// The file `name` in `folder`, which `folder` made: a relative path with no `..`, that no link takes out of the
/// folder, of a file within the limit.
pub fn source(folder: &Path, name: &str) -> Result<PathBuf, String> {
    let outside = || {
        format!(
            "{name} is not inside {}: a push takes a file in that folder",
            folder.display()
        )
    };
    let relative = Path::new(name);
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(outside());
    }
    let path = folder.join(relative);
    let real = path
        .canonicalize()
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if !real.starts_with(folder) {
        return Err(outside());
    }
    let about =
        std::fs::metadata(&real).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if !about.is_file() {
        return Err(format!("{name} is not a file"));
    }
    too_large(name, about.len())?;
    Ok(real)
}

pub fn too_large(name: &str, bytes: u64) -> Result<(), String> {
    if bytes > LIMIT {
        return Err(format!(
            "{name} is {bytes} bytes, over the limit of {LIMIT} (64 MiB): the daemon holds a file in memory whole"
        ));
    }
    Ok(())
}

/// Where a pulled file named `base` goes in `folder`. Nothing is replaced, and a link that is there is not followed.
pub fn destination(folder: &Path, base: &str) -> Result<PathBuf, String> {
    let path = folder.join(base);
    if std::fs::symlink_metadata(&path).is_ok() {
        return Err(format!(
            "{base} is already in {}: a pull replaces nothing, so move that file away first",
            folder.display()
        ));
    }
    Ok(path)
}

/// Writes a new file; one that appeared since `destination` looked is left alone.
pub fn keep(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
