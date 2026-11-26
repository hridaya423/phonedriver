#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use assert_cmd::Command;
use serde_json::Value;

const FAKE_CLAUDE: &str = r#"#!/bin/sh
echo "$@" >> "$HOME/claude.log"
if [ -e "$HOME/claude.has" ]; then
  echo "MCP server phonedriver already exists in user config"
  exit 1
fi
: > "$HOME/claude.has"
echo "Added stdio MCP server phonedriver"
"#;

const FAKE_CODEX: &str = r#"#!/bin/sh
echo "$@" >> "$HOME/codex.log"
if [ "$2" = get ]; then
  [ -e "$HOME/codex.has" ] && exit 0
  echo "Error: No MCP server named 'phonedriver' found." >&2
  exit 1
fi
: > "$HOME/codex.has"
"#;

struct Machine(tempfile::TempDir);

impl Machine {
    fn new() -> Machine {
        let m = Machine(tempfile::tempdir().unwrap());
        fs::create_dir(m.path("bin")).unwrap();
        m
    }

    fn path(&self, rel: &str) -> std::path::PathBuf {
        self.0.path().join(rel)
    }

    fn program(&self, name: &str, script: &str) {
        let p = self.path("bin").join(name);
        fs::write(&p, script).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn with_every_agent(self) -> Machine {
        self.program("claude", FAKE_CLAUDE);
        self.program("codex", FAKE_CODEX);
        fs::create_dir(self.path(".cursor")).unwrap();
        fs::create_dir(self.path(".gemini")).unwrap();
        fs::write(
            self.path(".gemini/settings.json"),
            r#"{"theme": "dark", "mcpServers": {"other": {"command": "x", "args": []}}}"#,
        )
        .unwrap();
        self
    }

    fn init(&self, args: &[&str]) -> Command {
        let mut c = Command::cargo_bin("phonedriver").unwrap();
        c.arg("init")
            .args(args)
            .env_clear()
            .env("HOME", self.0.path())
            .env("PATH", self.path("bin"))
            .env("PHONEDRIVER_HOME", self.path("pd"))
            .env("ADB_SERVER_SOCKET", "tcp:127.0.0.1:1");
        c
    }

    fn out(&self, args: &[&str]) -> String {
        let out = self
            .init(args)
            .env("PHONEDRIVER_NPX", "1")
            .assert()
            .success();
        String::from_utf8(out.get_output().stdout.clone()).unwrap()
    }
}

fn json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn init_puts_the_server_into_every_agent_it_finds_and_says_where() {
    let m = Machine::new().with_every_agent();
    let out = m.out(&["--register-only"]);
    assert_eq!(
        fs::read_to_string(m.path("claude.log")).unwrap(),
        "mcp add --scope user phonedriver -- npx -y phonedriver mcp\n"
    );
    assert_eq!(
        fs::read_to_string(m.path("codex.log")).unwrap(),
        "mcp get phonedriver\nmcp add phonedriver -- npx -y phonedriver mcp\n"
    );
    let cursor = json(&m.path(".cursor/mcp.json"));
    assert_eq!(
        cursor["mcpServers"]["phonedriver"],
        serde_json::json!({"command": "npx", "args": ["-y", "phonedriver", "mcp"]})
    );
    let gemini = json(&m.path(".gemini/settings.json"));
    assert_eq!(gemini["theme"], "dark");
    assert_eq!(gemini["mcpServers"]["other"]["command"], "x");
    assert_eq!(gemini["mcpServers"]["phonedriver"]["command"], "npx");
    for line in [
        "Claude Code  added",
        "Codex        added",
        "Cursor       added to ",
        "Gemini CLI   added to ",
        "VS Code      not found",
    ] {
        assert!(out.contains(line), "no {line:?} in:\n{out}");
    }
    assert!(out.contains("Restart your agent"), "{out}");
}

#[test]
fn a_second_init_changes_nothing_and_says_each_agent_already_has_it() {
    let m = Machine::new().with_every_agent();
    m.out(&["--register-only"]);
    let cursor = fs::read(m.path(".cursor/mcp.json")).unwrap();
    let gemini = fs::read(m.path(".gemini/settings.json")).unwrap();
    let out = m.out(&["--register-only"]);
    assert_eq!(fs::read(m.path(".cursor/mcp.json")).unwrap(), cursor);
    assert_eq!(fs::read(m.path(".gemini/settings.json")).unwrap(), gemini);
    assert_eq!(out.matches("already there").count(), 4, "{out}");
    assert_eq!(
        fs::read_to_string(m.path("codex.log"))
            .unwrap()
            .lines()
            .count(),
        3,
        "codex was asked to add it twice"
    );
}

#[test]
fn a_config_file_that_is_not_plain_json_is_left_as_it_is_and_the_entry_is_printed() {
    let m = Machine::new();
    fs::create_dir(m.path(".cursor")).unwrap();
    let theirs = "{\n  // my servers\n  \"mcpServers\": {}\n}\n";
    fs::write(m.path(".cursor/mcp.json"), theirs).unwrap();
    let out = m.out(&["--register-only"]);
    assert_eq!(
        fs::read_to_string(m.path(".cursor/mcp.json")).unwrap(),
        theirs
    );
    assert!(out.contains("Cursor       not changed"), "{out}");
    assert!(out.contains(r#""command": "npx""#), "{out}");
}

#[test]
fn another_entry_named_phonedriver_is_not_replaced() {
    let m = Machine::new();
    fs::create_dir(m.path(".cursor")).unwrap();
    let theirs =
        r#"{"mcpServers": {"phonedriver": {"command": "/old/phonedriver", "args": ["mcp"]}}}"#;
    fs::write(m.path(".cursor/mcp.json"), theirs).unwrap();
    let out = m.out(&["--register-only"]);
    assert_eq!(
        fs::read_to_string(m.path(".cursor/mcp.json")).unwrap(),
        theirs
    );
    assert!(out.contains("/old/phonedriver"), "{out}");
}

#[test]
fn started_as_its_own_binary_it_registers_that_binary_by_its_path() {
    let m = Machine::new();
    fs::create_dir(m.path(".cursor")).unwrap();
    m.init(&["--register-only"]).assert().success();
    let entry = &json(&m.path(".cursor/mcp.json"))["mcpServers"]["phonedriver"];
    let command = entry["command"].as_str().unwrap();
    assert!(Path::new(command).is_absolute(), "{command}");
    assert!(command.ends_with("phonedriver"), "{command}");
    assert_eq!(entry["args"], serde_json::json!(["mcp"]));
}

#[test]
fn a_dry_run_writes_nothing_and_runs_nothing() {
    let m = Machine::new().with_every_agent();
    let out = m.out(&["--register-only", "--dry-run"]);
    assert!(!m.path(".cursor/mcp.json").exists());
    assert!(!m.path("claude.log").exists());
    assert!(out.contains("Claude Code  would run: claude mcp add --scope user phonedriver -- npx -y phonedriver mcp"), "{out}");
    assert!(out.contains("Cursor       would add to "), "{out}");
}

#[test]
fn with_no_agent_found_it_prints_the_entry_to_paste() {
    let m = Machine::new();
    let out = m.out(&["--register-only"]);
    assert!(out.contains("No agent found"), "{out}");
    assert!(
        out.contains(r#""args": ["-y", "phonedriver", "mcp"]"#),
        "{out}"
    );
}

#[test]
fn init_ends_with_the_doctor_checks() {
    let m = Machine::new().with_every_agent();
    let out = m.out(&[]);
    assert!(out.contains("adb server"), "{out}");
}

fn skill() -> String {
    fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../SKILL.md")).unwrap()
}

const CLAUDE_SKILL: &str = ".claude/skills/phonedriver/SKILL.md";
const SHARED_SKILL: &str = ".agents/skills/phonedriver/SKILL.md";
const COPILOT_SKILL: &str = ".copilot/skills/phonedriver/SKILL.md";

#[test]
fn init_with_skill_writes_the_skill_where_each_agent_found_reads_a_persons_own() {
    let m = Machine::new().with_every_agent();
    let out = m.out(&["--register-only", "--skill"]);
    assert_eq!(fs::read_to_string(m.path(CLAUDE_SKILL)).unwrap(), skill());
    assert_eq!(fs::read_to_string(m.path(SHARED_SKILL)).unwrap(), skill());
    assert!(!m.path(".copilot").exists(), "VS Code was not found");
    let line = |agent: &str| {
        out.lines()
            .find(|l| l.starts_with(agent))
            .unwrap_or_else(|| panic!("no line for {agent} in:\n{out}"))
            .to_string()
    };
    assert!(line("Claude Code").contains("added"), "{out}");
    assert!(
        line("Claude Code").ends_with(&format!(
            "· skill written to {}",
            m.path(CLAUDE_SKILL).display()
        )),
        "{out}"
    );
    assert!(
        line("Codex").ends_with(&format!(
            "· skill written to {}",
            m.path(SHARED_SKILL).display()
        )),
        "{out}"
    );
    for agent in ["Cursor", "Gemini CLI"] {
        assert!(
            line(agent).ends_with(&format!(
                "· skill already at {}",
                m.path(SHARED_SKILL).display()
            )),
            "Codex, Cursor and Gemini CLI read one folder: {out}"
        );
    }
    assert_eq!(line("VS Code"), "VS Code      not found");
}

#[test]
fn vs_code_gets_the_skill_in_the_folder_copilot_reads() {
    let m = Machine::new();
    for user in ["Library/Application Support/Code/User", ".config/Code/User"] {
        fs::create_dir_all(m.path(user)).unwrap();
    }
    let out = m.out(&["--register-only", "--skill"]);
    assert_eq!(
        fs::read_to_string(m.path(COPILOT_SKILL)).unwrap(),
        skill(),
        "{out}"
    );
    assert!(!m.path(".claude").exists() && !m.path(".agents").exists());
}

#[test]
fn init_without_skill_writes_no_skill() {
    let m = Machine::new().with_every_agent();
    let out = m.out(&["--register-only"]);
    assert!(!m.path(".claude").exists() && !m.path(".agents").exists());
    assert!(!out.contains("skill"), "{out}");
}

#[test]
fn a_skill_file_that_differs_is_left_as_it_is_and_the_line_says_so() {
    let m = Machine::new().with_every_agent();
    fs::create_dir_all(m.path(CLAUDE_SKILL).parent().unwrap()).unwrap();
    fs::write(m.path(CLAUDE_SKILL), "mine\n").unwrap();
    let out = m.out(&["--register-only", "--skill"]);
    assert_eq!(fs::read_to_string(m.path(CLAUDE_SKILL)).unwrap(), "mine\n");
    assert!(
        out.contains(&format!(
            "· skill not changed: {} is not this version's; delete it and run `init --skill` again to replace it",
            m.path(CLAUDE_SKILL).display()
        )),
        "{out}"
    );
    assert_eq!(fs::read_to_string(m.path(SHARED_SKILL)).unwrap(), skill());
}

#[test]
fn a_dry_run_with_skill_writes_nothing_and_a_second_run_finds_the_skill_there() {
    let m = Machine::new().with_every_agent();
    let out = m.out(&["--register-only", "--skill", "--dry-run"]);
    assert!(!m.path(".claude").exists() && !m.path(".agents").exists());
    assert!(
        out.contains(&format!(
            "· would write the skill to {}",
            m.path(CLAUDE_SKILL).display()
        )),
        "{out}"
    );
    m.out(&["--register-only", "--skill"]);
    let out = m.out(&["--register-only", "--skill"]);
    assert_eq!(out.matches("· skill already at ").count(), 4, "{out}");
}

#[test]
fn a_skill_that_cannot_be_written_fails_the_run_and_says_why() {
    let m = Machine::new();
    m.program("claude", FAKE_CLAUDE);
    fs::write(m.path(".claude"), "a file where the folder would be").unwrap();
    let out = m
        .init(&["--register-only", "--skill"])
        .env("PHONEDRIVER_NPX", "1")
        .assert()
        .failure();
    let out = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(out.contains("· skill failed: cannot write "), "{out}");
}
