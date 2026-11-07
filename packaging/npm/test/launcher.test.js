"use strict";
const test = require("node:test");
const assert = require("node:assert");
const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const pkg = require("../package.json");
const { targetFor, archiveName } = require("../lib/launcher");

const BIN = path.join(__dirname, "..", "bin", "phonedriver.js");
const posix = process.platform !== "win32";

function tmp() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "pd-npm-test-"));
}

function fakeRelease(script, { badSum = false } = {}) {
  const root = tmp();
  const target = targetFor(process.platform, process.arch);
  const stage = path.join(root, "stage", `phonedriver-${target}`);
  fs.mkdirSync(stage, { recursive: true });
  fs.writeFileSync(path.join(stage, "phonedriver"), script, { mode: 0o755 });
  const release = path.join(root, "release");
  fs.mkdirSync(release);
  const name = archiveName(target);
  const tar = spawnSync("tar", ["-cJf", path.join(release, name), "-C", path.join(root, "stage"), `phonedriver-${target}`]);
  assert.strictEqual(tar.status, 0, String(tar.stderr));
  const hash = crypto.createHash("sha256").update(fs.readFileSync(path.join(release, name))).digest("hex");
  fs.writeFileSync(path.join(release, `${name}.sha256`), `${badSum ? "0".repeat(64) : hash}  ${name}\n`);
  return { release, root };
}

function run(args, release, cache, input) {
  return spawnSync(process.execPath, [BIN, ...args], {
    encoding: "utf8",
    input,
    env: {
      ...process.env,
      PHONEDRIVER_RELEASE_BASE_URL: `file://${release}`,
      PHONEDRIVER_NPM_CACHE: cache,
    },
  });
}

test("target map covers the five cargo-dist targets", () => {
  assert.strictEqual(targetFor("darwin", "arm64"), "aarch64-apple-darwin");
  assert.strictEqual(targetFor("darwin", "x64"), "x86_64-apple-darwin");
  assert.strictEqual(targetFor("linux", "x64"), "x86_64-unknown-linux-gnu");
  assert.strictEqual(targetFor("linux", "arm64"), "aarch64-unknown-linux-gnu");
  assert.strictEqual(targetFor("win32", "x64"), "x86_64-pc-windows-msvc");
  assert.strictEqual(archiveName("x86_64-pc-windows-msvc"), "phonedriver-x86_64-pc-windows-msvc.zip");
  assert.throws(() => targetFor("freebsd", "x64"), /no prebuilt binary for freebsd x64/);
});

test("package, Cargo and server.json versions agree", () => {
  const root = path.join(__dirname, "..", "..", "..");
  const cargo = fs.readFileSync(path.join(root, "Cargo.toml"), "utf8").match(/^version = "([^"]+)"/m)[1];
  const server = JSON.parse(fs.readFileSync(path.join(root, "server.json"), "utf8"));
  assert.strictEqual(pkg.version, cargo);
  assert.strictEqual(server.version, cargo);
  assert.strictEqual(server.packages[0].version, cargo);
  assert.strictEqual(server.packages[0].identifier, pkg.name);
  assert.strictEqual(server.name, pkg.mcpName);
});

test("first run downloads, verifies, caches and passes args, stdin and exit code through", { skip: !posix }, () => {
  const { release } = fakeRelease('#!/bin/sh\necho "args:$*"\nread line\necho "stdin:$line"\nexit 7\n');
  const cache = tmp();
  const first = run(["mcp", "--x"], release, cache, "hello\n");
  assert.strictEqual(first.status, 7);
  assert.strictEqual(first.stdout, "args:mcp --x\nstdin:hello\n");
  assert.match(first.stderr, /first run, downloading phonedriver-.* from file:/);
  assert.ok(fs.existsSync(path.join(cache, pkg.version, targetFor(process.platform, process.arch), "phonedriver")));

  fs.rmSync(release, { recursive: true });
  const second = run(["--version"], release, cache);
  assert.strictEqual(second.status, 7);
  assert.strictEqual(second.stdout, "args:--version\nstdin:\n");
  assert.strictEqual(second.stderr, "");
});

test("a bad checksum installs nothing and says so", { skip: !posix }, () => {
  const { release } = fakeRelease("#!/bin/sh\necho hi\n", { badSum: true });
  const cache = tmp();
  const r = run(["--version"], release, cache);
  assert.strictEqual(r.status, 1);
  assert.match(r.stderr, /checksum mismatch/);
  assert.strictEqual(r.stdout, "");
  assert.ok(!fs.existsSync(path.join(cache, pkg.version)));
});

test("a missing release explains the failure and points at the other installers", () => {
  const empty = tmp();
  const r = run(["--version"], empty, tmp());
  assert.strictEqual(r.status, 1);
  assert.match(r.stderr, /download failed/);
  assert.match(r.stderr, /#install/);
});
