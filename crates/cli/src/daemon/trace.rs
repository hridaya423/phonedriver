//! Per-session trace files (architecture 5). Typed text is never written, only its length.

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::rpc::RpcError;

/// What a trace may hold beyond the defaults: observation text and text targets (`full`), and screenshots (`screens`).
#[derive(Debug, Clone, Copy, Default)]
pub struct TraceOptions {
    pub full: bool,
    pub screens: bool,
}

struct Open {
    file: File,
    stem: String,
    shots: u32,
}

pub struct Traces {
    dir: PathBuf,
    options: TraceOptions,
    files: Mutex<HashMap<String, Open>>,
}

impl Traces {
    pub fn new(dir: PathBuf, options: TraceOptions) -> Traces {
        Traces {
            dir,
            options,
            files: Mutex::default(),
        }
    }

    pub fn record(
        &self,
        session: &str,
        method: &str,
        params: &Value,
        result: &Result<Value, RpcError>,
        took: Duration,
    ) {
        let (ok, reply, settle_ms) = match result {
            Ok(v) if method == "shell" => (
                true,
                format!(
                    "exit {}, {} output chars",
                    v["exit"],
                    v["output"].as_str().map_or(0, |o| o.chars().count())
                ),
                v["action"]["settle_ms"].as_u64(),
            ),
            Ok(v) => (
                true,
                v["summary"].as_str().unwrap_or(method).to_string(),
                v["settle_ms"].as_u64(),
            ),
            Err(e) => (false, e.message.clone(), None),
        };
        let at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let mut line = json!({
            "t": at, "session": session, "method": method,
            "params": sanitize(method, params, self.options.full),
            "ok": ok, "reply": reply, "ms": took.as_millis() as u64, "settle_ms": settle_ms,
        });
        if self.options.full
            && let Ok(v) = result
            && let Some(text) = v["text"]
                .as_str()
                .or_else(|| v["observation"]["text"].as_str())
                .filter(|t| !t.is_empty())
        {
            line["observation"] = json!(text);
        }
        let mut files = self.files.lock().unwrap_or_else(|e| e.into_inner());
        if !files.contains_key(session) {
            let stem = ulid::Ulid::generate().to_string();
            match self.create(&format!("{stem}.jsonl")) {
                Ok(file) => {
                    files.insert(
                        session.to_string(),
                        Open {
                            file,
                            stem,
                            shots: 0,
                        },
                    );
                }
                Err(_) => return,
            }
        }
        let Some(open) = files.get_mut(session) else {
            return;
        };
        if self.options.screens
            && let Ok(v) = result
            && let Some(path) = v["path"].as_str().filter(|_| method == "screenshot")
        {
            open.shots += 1;
            let name = format!("{}-{}.png", open.stem, open.shots);
            if std::fs::read(path)
                .and_then(|bytes| self.create(&name)?.write_all(&bytes))
                .is_ok()
            {
                line["screen"] = json!(name);
            }
        }
        let _ = writeln!(open.file, "{line}");
    }

    fn create(&self, name: &str) -> std::io::Result<File> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&self.dir)?;
            std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700))?;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(self.dir.join(name))
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir_all(&self.dir)?;
            File::create(self.dir.join(name))
        }
    }
}

/// The request as the trace keeps it. Typed text and the clipboard text become their lengths whatever the flags say,
/// because a `SecureField` is only known to the actor and not on every error path. Text targets are copies of what
/// the screen shows, so they follow `full`. A URL loses its query and fragment, where tokens live.
fn length(key: &str, v: &Value) -> Value {
    json!({ key: v.as_str().map_or(0, |s| s.chars().count()) })
}

fn hide_text(t: &mut Value, full: bool) {
    if !full && let Some(s) = t.get("text") {
        t["text"] = length("text_len", s);
    }
}

fn hide_condition(condition: &mut Value, full: bool) {
    for key in ["appears", "gone"] {
        hide_text(&mut condition[key], full);
    }
}

fn hide_action(action: &mut Value, full: bool) {
    let secret = action["secret"].as_bool().unwrap_or(false);
    if action["kind"] == "type"
        && let Some(text) = action.get("text").cloned()
    {
        action["text"] = length(if secret { "secret_len" } else { "text_len" }, &text);
    }
    if let Some(url) = action["url"].as_str() {
        let end = url.find(['?', '#']).unwrap_or(url.len());
        action["url"] = json!(&url[..end]);
    }
    for key in ["target", "into", "from", "to", "at", "in", "until"] {
        hide_text(&mut action[key], full);
    }
}

fn sanitize(method: &str, params: &Value, full: bool) -> Value {
    let mut p = params.clone();
    if let Some(o) = p.as_object_mut() {
        o.remove("session");
    }
    match method {
        "act" => hide_action(&mut p["action"], full),
        "wait" => hide_condition(&mut p["condition"], full),
        "batch" => {
            for step in p["steps"].as_array_mut().into_iter().flatten() {
                if step["kind"] == "wait" {
                    hide_condition(&mut step["condition"], full);
                } else {
                    hide_action(step, full);
                }
            }
        }
        "find" if !full => {
            if let Some(s) = p.get("text").cloned() {
                p["text"] = length("text_len", &s);
            }
        }
        "shell" => {
            if let Some(s) = p.get("command").cloned()
                && p["secret"].as_bool().unwrap_or(false)
            {
                p["command"] = length("secret_len", &s);
            }
        }
        "clipboard" => {
            if let Some(s) = p.get("set").cloned() {
                p["set"] = length("text_len", &s);
            }
        }
        _ => {}
    }
    p
}

pub fn clean_older_than(dir: &std::path::Path, age: Duration) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            clean_older_than(&path, age);
        } else if entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|m| m.elapsed().ok())
            .is_some_and(|e| e > age)
        {
            let _ = std::fs::remove_file(path);
        }
    }
}
