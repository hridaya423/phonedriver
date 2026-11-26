//! `phonedriver init`: adds the MCP server to every agent found on this machine, then runs the doctor checks.
//!
//! Claude Code and Codex own files that they rewrite themselves, so their own commands add the server. Cursor,
//! VS Code and Gemini CLI read a JSON file, which is edited here when it is plain JSON and left alone otherwise.
//!
//! With `--skill` each agent found also gets `SKILL.md` in the folder it reads a person's own skills from
//! (decision D137). Plain `init` writes none: beside the MCP server the skill cost tokens and saved no turn (D136).

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::{Value, json};

const NAME: &str = "phonedriver";

/// The skill `phonedriver skill` prints and `init --skill` installs.
pub const SKILL: &str = include_str!("../../../SKILL.md");

/// Where each agent reads a person's own skills, under the home folder, as its documentation said on 2026-11-26:
/// Claude Code `~/.claude/skills`, VS Code's Copilot `~/.copilot/skills`, and Codex, Cursor and Gemini CLI
/// `~/.agents/skills`, one folder the three share.
const CLAUDE_SKILLS: &str = ".claude/skills";
const COPILOT_SKILLS: &str = ".copilot/skills";
const SHARED_SKILLS: &str = ".agents/skills";

/// How an agent starts the server.
#[derive(Debug, Clone, PartialEq)]
pub struct Server {
    pub command: String,
    pub args: Vec<String>,
}

impl Server {
    /// Through `npx` when that is how this process was started (the npm launcher sets `PHONEDRIVER_NPX`), so the
    /// agent follows new releases; otherwise this binary, by name when that name on `PATH` is this file.
    fn here() -> Server {
        if std::env::var_os("PHONEDRIVER_NPX").is_some() {
            let npx = ["npx", "-y", NAME, "mcp"].map(String::from);
            return if cfg!(windows) {
                Server {
                    command: "cmd".into(),
                    args: std::iter::once("/c".to_string()).chain(npx).collect(),
                }
            } else {
                Server {
                    command: "npx".into(),
                    args: npx[1..].to_vec(),
                }
            };
        }
        let exe = std::env::current_exe().and_then(|p| p.canonicalize());
        let by_name = on_path(NAME).and_then(|p| p.canonicalize().ok());
        let command = match (&exe, by_name) {
            (Ok(exe), Some(named)) if *exe == named => NAME.to_string(),
            (Ok(exe), _) => exe.display().to_string(),
            (Err(_), _) => NAME.to_string(),
        };
        Server {
            command,
            args: vec!["mcp".into()],
        }
    }

    fn words(&self) -> Vec<&str> {
        std::iter::once(self.command.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect()
    }

    fn entry(&self, key: &str) -> Value {
        let mut entry = json!({"command": self.command, "args": self.args});
        if key == "servers" {
            entry["type"] = json!("stdio");
        }
        entry
    }

    fn to_paste(&self, key: &str) -> String {
        let args = serde_json::to_string(&self.args)
            .unwrap_or_default()
            .replace(",", ", ");
        let kind = if key == "servers" {
            "\"type\": \"stdio\", "
        } else {
            ""
        };
        format!(
            "{{\n  \"{key}\": {{\n    \"{NAME}\": {{ {kind}\"command\": {}, \"args\": {args} }}\n  }}\n}}",
            json!(self.command)
        )
    }
}

fn on_path(program: &str) -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ""]
    } else {
        &[""]
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .flat_map(|dir| {
            names
                .iter()
                .map(move |ext| dir.join(format!("{program}{ext}")))
        })
        .find(|p| p.is_file())
}

/// What one agent's line says.
#[derive(Debug, PartialEq)]
enum Outcome {
    NotFound,
    Did(String),
    Already,
    NotChanged(String),
    Failed(String),
}

/// The file's text with the server added under `key`. `Ok(None)` when it is there already.
fn with_server(
    existing: Option<&str>,
    key: &str,
    server: &Server,
) -> Result<Option<String>, String> {
    let mut root: Value = match existing.map(str::trim) {
        None | Some("") => json!({}),
        Some(text) => serde_json::from_str(text).map_err(|_| "is not plain JSON".to_string())?,
    };
    let servers = root
        .as_object_mut()
        .ok_or("is not a JSON object")?
        .entry(key)
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or(format!("has a `{key}` that is not an object"))?;
    let entry = server.entry(key);
    match servers.get(NAME) {
        Some(theirs)
            if theirs["command"] == entry["command"] && theirs["args"] == entry["args"] =>
        {
            return Ok(None);
        }
        Some(theirs) => {
            return Err(format!(
                "already has another {NAME} entry ({}); remove it to have this one added",
                theirs["command"].as_str().unwrap_or("no command")
            ));
        }
        None => servers.insert(NAME.into(), entry),
    };
    let mut text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    text.push('\n');
    Ok(Some(text))
}

fn in_file(file: &Path, key: &str, server: &Server, dry_run: bool) -> Outcome {
    let existing = std::fs::read_to_string(file).ok();
    let text = match with_server(existing.as_deref(), key, server) {
        Ok(None) => return Outcome::Already,
        Ok(Some(text)) => text,
        Err(why) => {
            return Outcome::NotChanged(format!(
                "{} {why}. The entry:\n{}",
                file.display(),
                server.to_paste(key)
            ));
        }
    };
    if dry_run {
        return Outcome::Did(format!("would add to {}", file.display()));
    }
    let partial = file.with_extension("json.phonedriver-partial");
    match std::fs::write(&partial, text).and_then(|()| std::fs::rename(&partial, file)) {
        Ok(()) => Outcome::Did(format!("added to {}", file.display())),
        Err(e) => Outcome::Failed(format!("cannot write {}: {e}", file.display())),
    }
}

fn said(out: &std::process::Output) -> String {
    let text = [out.stdout.as_slice(), out.stderr.as_slice()].concat();
    String::from_utf8_lossy(&text).into_owned()
}

fn first_line(text: &str) -> &str {
    text.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("it gave no reason")
}

fn claude(server: &Server, dry_run: bool) -> Outcome {
    let Some(program) = on_path("claude") else {
        return Outcome::NotFound;
    };
    let mut args = vec!["mcp", "add", "--scope", "user", NAME, "--"];
    args.extend(server.words());
    if dry_run {
        return Outcome::Did(format!("would run: claude {}", args.join(" ")));
    }
    match Command::new(program).args(&args).output() {
        Ok(out) if out.status.success() => Outcome::Did("added, for every project".into()),
        Ok(out) if said(&out).contains("already exists") => Outcome::Already,
        Ok(out) => Outcome::Failed(first_line(&said(&out)).to_string()),
        Err(e) => Outcome::Failed(format!("cannot run claude: {e}")),
    }
}

fn codex(server: &Server, dry_run: bool) -> Outcome {
    let Some(program) = on_path("codex") else {
        return Outcome::NotFound;
    };
    let mut args = vec!["mcp", "add", NAME, "--"];
    args.extend(server.words());
    if dry_run {
        return Outcome::Did(format!("would run: codex {}", args.join(" ")));
    }
    let has = Command::new(&program).args(["mcp", "get", NAME]).output();
    if has.is_ok_and(|out| out.status.success()) {
        return Outcome::Already;
    }
    match Command::new(program).args(&args).output() {
        Ok(out) if out.status.success() => Outcome::Did("added".into()),
        Ok(out) => Outcome::Failed(first_line(&said(&out)).to_string()),
        Err(e) => Outcome::Failed(format!("cannot run codex: {e}")),
    }
}

fn vs_code_user(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Code/User")
    } else if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map_or_else(|| home.join("AppData/Roaming"), PathBuf::from)
            .join("Code/User")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map_or_else(|| home.join(".config"), PathBuf::from)
            .join("Code/User")
    }
}

fn file_agent(dir: PathBuf, file: &str, key: &str, server: &Server, dry_run: bool) -> Outcome {
    if !dir.is_dir() {
        return Outcome::NotFound;
    }
    in_file(&dir.join(file), key, server, dry_run)
}

/// What became of the skill in one agent's folder of skills. A file that is there with other text is someone's
/// own or an older version's, and is left; a new one is made only where nothing is, links included.
fn with_skill(skills: &Path, dry_run: bool) -> Result<String, String> {
    use std::io::Write;
    let file = skills.join(NAME).join("SKILL.md");
    match std::fs::read_to_string(&file) {
        Ok(has) if has == SKILL => return Ok(format!("skill already at {}", file.display())),
        Ok(_) => {
            return Ok(format!(
                "skill not changed: {} is not this version's; delete it and run `init --skill` again to replace it",
                file.display()
            ));
        }
        Err(_) => {}
    }
    if dry_run {
        return Ok(format!("would write the skill to {}", file.display()));
    }
    std::fs::create_dir_all(skills.join(NAME))
        .and_then(|()| {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&file)
        })
        .and_then(|mut new| new.write_all(SKILL.as_bytes()))
        .map(|()| format!("skill written to {}", file.display()))
        .map_err(|e| format!("skill failed: cannot write {}: {e}", file.display()))
}

/// Adds the server to each agent found and prints one line for each; with `skill`, the line also says what became
/// of the skill in that agent's folder. Unless `register_only`, the doctor checks follow. False when an agent that
/// was found could not be given the server, or the skill it was to get.
pub fn run(home: &Path, dry_run: bool, register_only: bool, skill: bool) -> ExitCode {
    let server = Server::here();
    let user = std::env::home_dir().unwrap_or_default();
    let agents = [
        ("Claude Code", claude(&server, dry_run), CLAUDE_SKILLS),
        ("Codex", codex(&server, dry_run), SHARED_SKILLS),
        (
            "Cursor",
            file_agent(
                user.join(".cursor"),
                "mcp.json",
                "mcpServers",
                &server,
                dry_run,
            ),
            SHARED_SKILLS,
        ),
        (
            "VS Code",
            file_agent(vs_code_user(&user), "mcp.json", "servers", &server, dry_run),
            COPILOT_SKILLS,
        ),
        (
            "Gemini CLI",
            file_agent(
                user.join(".gemini"),
                "settings.json",
                "mcpServers",
                &server,
                dry_run,
            ),
            SHARED_SKILLS,
        ),
    ];
    let mut skill_failed = false;
    for (name, outcome, skills) in &agents {
        let mut line = match outcome {
            Outcome::NotFound => "not found".to_string(),
            Outcome::Did(what) => what.clone(),
            Outcome::Already => "already there".to_string(),
            Outcome::NotChanged(why) => format!("not changed: {why}"),
            Outcome::Failed(why) => format!("failed: {why}"),
        };
        if skill && *outcome != Outcome::NotFound {
            let said = with_skill(&user.join(skills), dry_run).unwrap_or_else(|why| {
                skill_failed = true;
                why
            });
            line = format!("{line} · {said}");
        }
        println!("{name:<12} {line}");
    }
    if agents.iter().all(|(_, o, _)| *o == Outcome::NotFound) {
        println!(
            "\nNo agent found on this machine. Add this to your agent's MCP servers:\n{}",
            server.to_paste("mcpServers")
        );
    } else if !dry_run {
        println!(
            "\nRestart your agent so that it loads phonedriver, then ask it: Open Settings and turn on dark mode."
        );
    }
    let failed = skill_failed
        || agents
            .iter()
            .any(|(_, o, _)| matches!(o, Outcome::Failed(_)));
    if !register_only && !dry_run {
        println!();
        crate::doctor::run(home, None, false);
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn npx() -> Server {
        Server {
            command: "npx".into(),
            args: vec!["-y".into(), NAME.into(), "mcp".into()],
        }
    }

    #[test]
    fn the_server_is_added_beside_what_the_file_already_holds() {
        let text = with_server(Some(r#"{"a": 1, "servers": {"x": {}}}"#), "servers", &npx())
            .unwrap()
            .unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["servers"]["x"], json!({}));
        assert_eq!(
            v["servers"][NAME],
            json!({"type": "stdio", "command": "npx", "args": ["-y", NAME, "mcp"]})
        );
        assert_eq!(with_server(Some(&text), "servers", &npx()), Ok(None));
    }

    #[test]
    fn a_file_that_is_no_json_object_is_refused_with_the_reason() {
        assert_eq!(
            with_server(Some("// note\n{}"), "mcpServers", &npx()),
            Err("is not plain JSON".into())
        );
        assert_eq!(
            with_server(Some("[]"), "mcpServers", &npx()),
            Err("is not a JSON object".into())
        );
    }
}
