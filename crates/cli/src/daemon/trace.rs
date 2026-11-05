//! Per-session trace files (architecture 5). Typed text is never written, only its length.

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::rpc::RpcError;

pub struct Traces {
    dir: PathBuf,
    files: Mutex<HashMap<String, File>>,
}

impl Traces {
    pub fn new(dir: PathBuf) -> Traces {
        Traces {
            dir,
            files: Mutex::default(),
        }
    }

    pub fn record(
        &self,
        session: &str,
        method: &str,
        request: &str,
        result: &Result<Value, RpcError>,
        took: Duration,
    ) {
        let (ok, reply, settle_ms) = match result {
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
        let line = json!({
            "t": at, "session": session, "method": method, "request": request,
            "ok": ok, "reply": reply, "ms": took.as_millis() as u64, "settle_ms": settle_ms,
        });
        let mut files = self.files.lock().unwrap_or_else(|e| e.into_inner());
        if !files.contains_key(session) {
            let opened = match std::fs::create_dir_all(&self.dir) {
                Ok(()) => File::create(self.dir.join(format!("{}.jsonl", ulid::Ulid::generate()))),
                Err(e) => Err(e),
            };
            match opened {
                Ok(f) => {
                    files.insert(session.to_string(), f);
                }
                Err(_) => return,
            }
        }
        if let Some(f) = files.get_mut(session) {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// A one-line description of a request that never contains typed text or text targets.
pub fn describe(method: &str, params: &Value) -> String {
    if method != "act" {
        return method.to_string();
    }
    let action = &params["action"];
    let target = |t: &Value| {
        if let Some(r) = t.get("ref").and_then(Value::as_str) {
            format!("ref {r}")
        } else if t.get("point").is_some() {
            format!("point {},{}", t["point"]["x"], t["point"]["y"])
        } else if let Some(s) = t.get("text").and_then(Value::as_str) {
            format!("text of {} chars", s.chars().count())
        } else {
            "target".into()
        }
    };
    match action["kind"].as_str() {
        Some("tap") => format!("act tap {}", target(&action["target"])),
        Some("type") => format!(
            "act type {} chars{} mode {}",
            action["text"].as_str().map_or(0, |s| s.chars().count()),
            action
                .get("into")
                .filter(|t| !t.is_null())
                .map(|t| format!(" into {}", target(t)))
                .unwrap_or_default(),
            action["mode"].as_str().unwrap_or("paste")
        ),
        Some("press") => format!("act press {}", action["key"].as_str().unwrap_or("?")),
        _ => "act".into(),
    }
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
