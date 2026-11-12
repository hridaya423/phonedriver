"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const repo = path.join(__dirname, "..", "..");
const plugin = path.join(repo, "packaging", "plugin");
const read = (...p) => fs.readFileSync(path.join(...p), "utf8");
const json = (...p) => JSON.parse(read(...p));
const release = /^version = "([^"]+)"/m.exec(read(repo, "Cargo.toml"))[1];
const launch = { command: "npx", args: ["-y", "phonedriver", "mcp"] };

test("the bundled skill is the repository's SKILL.md, byte for byte", () => {
  assert.strictEqual(read(plugin, "skills", "phonedriver", "SKILL.md"), read(repo, "SKILL.md"));
});

for (const host of [".claude-plugin", ".codex-plugin", ".cursor-plugin"]) {
  test(`the ${host} manifest names the plugin and carries the release version`, () => {
    const m = json(plugin, host, "plugin.json");
    assert.strictEqual(m.name, "phonedriver");
    assert.strictEqual(m.version, release);
    assert.strictEqual(m.license, "AGPL-3.0-only");
    assert.strictEqual(m.description, json(plugin, ".claude-plugin", "plugin.json").description);
  });
}

test("the plugin's LICENSE is the repository's, byte for byte", () => {
  assert.strictEqual(read(plugin, "LICENSE"), read(repo, "LICENSE"));
});

test("the Codex manifest points at the skill folder and the MCP file", () => {
  const m = json(plugin, ".codex-plugin", "plugin.json");
  assert.strictEqual(m.skills, "./skills/");
  assert.strictEqual(m.mcpServers, "./.mcp.json");
});

test("Cursor's mcp.json is the MCP file, byte for byte", () => {
  assert.strictEqual(json(plugin, ".cursor-plugin", "plugin.json").skills, "./skills/");
  assert.strictEqual(read(plugin, "mcp.json"), read(plugin, ".mcp.json"));
});

test("the icon the Codex and Cursor manifests name is the plugin's icon", () => {
  const icon = fs.readFileSync(path.join(plugin, ".claude-plugin", "icon.png"));
  assert.deepStrictEqual(fs.readFileSync(path.join(plugin, "assets", "icon.png")), icon);
  assert.strictEqual(json(plugin, ".codex-plugin", "plugin.json").interface.logo, "./assets/icon.png");
  assert.strictEqual(json(plugin, ".cursor-plugin", "plugin.json").logo, "assets/icon.png");
});

test("the MCP file starts the published launcher", () => {
  const s = json(plugin, ".mcp.json").mcpServers.phonedriver;
  assert.strictEqual(s.command, launch.command);
  assert.deepStrictEqual(s.args, launch.args);
});

test("the plugin folder holds only what is installed", () => {
  const files = fs
    .readdirSync(plugin, { recursive: true })
    .filter((f) => fs.statSync(path.join(plugin, f)).isFile())
    .map((f) => f.split(path.sep).join("/"));
  assert.deepStrictEqual(files.sort(), [
    ".claude-plugin/icon.png",
    ".claude-plugin/plugin.json",
    ".codex-plugin/plugin.json",
    ".cursor-plugin/plugin.json",
    ".mcp.json",
    "LICENSE",
    "README.md",
    "SECURITY.md",
    "assets/icon.png",
    "mcp.json",
    "skills/phonedriver/SKILL.md",
  ]);
});

for (const host of [".claude-plugin", ".cursor-plugin"]) {
  test(`the ${host} marketplace lists the plugin at its folder`, () => {
    const m = json(repo, host, "marketplace.json");
    assert.strictEqual(m.name, "phonedriver");
    assert.strictEqual(m.plugins.length, 1);
    const p = m.plugins[0];
    assert.strictEqual(p.name, "phonedriver");
    assert.strictEqual(path.resolve(repo, p.source), plugin);
  });
}
