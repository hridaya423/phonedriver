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

test("the manifest names the plugin and carries the release version", () => {
  const m = json(plugin, ".claude-plugin", "plugin.json");
  assert.strictEqual(m.name, "phonedriver");
  assert.strictEqual(m.version, release);
  assert.strictEqual(m.license, "AGPL-3.0-only");
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
    ".mcp.json",
    "skills/phonedriver/SKILL.md",
  ]);
});

test("the marketplace lists the plugin at its folder", () => {
  const m = json(repo, ".claude-plugin", "marketplace.json");
  assert.strictEqual(m.name, "phonedriver");
  assert.strictEqual(m.plugins.length, 1);
  const p = m.plugins[0];
  assert.strictEqual(p.name, "phonedriver");
  assert.strictEqual(path.resolve(repo, p.source), plugin);
});
