"use strict";
const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn, spawnSync } = require("node:child_process");

const pkg = require("../package.json");

const TARGETS = {
  "darwin-arm64": "aarch64-apple-darwin",
  "darwin-x64": "x86_64-apple-darwin",
  "linux-arm64": "aarch64-unknown-linux-gnu",
  "linux-x64": "x86_64-unknown-linux-gnu",
  "win32-x64": "x86_64-pc-windows-msvc",
};

const DEFAULT_BASE = `https://github.com/hridaya423/phonedriver/releases/download/v${pkg.version}`;

function targetFor(platform, arch) {
  const target = TARGETS[`${platform}-${arch}`];
  if (!target) {
    throw new Error(
      `no prebuilt binary for ${platform} ${arch}. Supported: ${Object.keys(TARGETS).join(", ")}. ` +
        "Build from source: https://github.com/hridaya423/phonedriver#install",
    );
  }
  return target;
}

function cacheRoot(env, platform) {
  if (env.PHONEDRIVER_NPM_CACHE) return env.PHONEDRIVER_NPM_CACHE;
  if (platform === "win32") {
    return path.join(env.LOCALAPPDATA || path.join(os.homedir(), "AppData", "Local"), "phonedriver", "npm");
  }
  if (platform === "darwin") return path.join(os.homedir(), "Library", "Caches", "phonedriver", "npm");
  return path.join(env.XDG_CACHE_HOME || path.join(os.homedir(), ".cache"), "phonedriver", "npm");
}

function archiveName(target) {
  return `phonedriver-${target}.${target.includes("windows") ? "zip" : "tar.gz"}`;
}

async function fetchBytes(url) {
  if (url.startsWith("file:")) return fs.readFileSync(new URL(url));
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok) throw new Error(`GET ${url} answered ${res.status}`);
  return Buffer.from(await res.arrayBuffer());
}

function findBinary(dir, exe) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      const found = findBinary(full, exe);
      if (found) return found;
    } else if (entry.name === exe) {
      return full;
    }
  }
  return null;
}

async function ensureBinary({ env = process.env, platform = process.platform, arch = process.arch, log } = {}) {
  const target = targetFor(platform, arch);
  const exe = platform === "win32" ? "phonedriver.exe" : "phonedriver";
  const dir = path.join(cacheRoot(env, platform), pkg.version, target);
  const cached = path.join(dir, exe);
  if (fs.existsSync(cached)) return cached;

  const base = (env.PHONEDRIVER_RELEASE_BASE_URL || DEFAULT_BASE).replace(/\/+$/, "");
  const name = archiveName(target);
  log(`phonedriver ${pkg.version}: first run, downloading ${name} from ${base} (cached for next time)`);

  let archive;
  let sumText;
  try {
    [archive, sumText] = await Promise.all([fetchBytes(`${base}/${name}`), fetchBytes(`${base}/${name}.sha256`)]);
  } catch (err) {
    throw new Error(
      `download failed: ${err.message}. Check your connection, or install another way: ` +
        "https://github.com/hridaya423/phonedriver#install",
    );
  }

  const expected = sumText.toString("utf8").trim().split(/\s+/)[0].toLowerCase();
  const actual = crypto.createHash("sha256").update(archive).digest("hex");
  if (expected !== actual) {
    throw new Error(`checksum mismatch for ${name}: expected ${expected}, got ${actual}. Nothing was installed.`);
  }

  fs.mkdirSync(path.dirname(dir), { recursive: true });
  const work = fs.mkdtempSync(path.join(path.dirname(dir), `.dl-${process.pid}-`));
  try {
    const archivePath = path.join(work, name);
    fs.writeFileSync(archivePath, archive);
    const out = path.join(work, "out");
    fs.mkdirSync(out);
    const tar = spawnSync("tar", ["-xf", archivePath, "-C", out], { encoding: "utf8" });
    if (tar.error || tar.status !== 0) {
      throw new Error(`could not unpack ${name} with tar: ${tar.error ? tar.error.message : tar.stderr.trim()}`);
    }
    const found = findBinary(out, exe);
    if (!found) throw new Error(`${name} does not contain ${exe}`);
    fs.chmodSync(found, 0o755);
    fs.mkdirSync(dir, { recursive: true });
    fs.renameSync(found, cached);
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
  return cached;
}

async function main(argv, opts = {}) {
  const env = opts.env || process.env;
  const log = opts.log || ((line) => process.stderr.write(`${line}\n`));
  const binary = await ensureBinary({ env, platform: opts.platform, arch: opts.arch, log });
  return new Promise((resolve, reject) => {
    const child = spawn(binary, argv, { stdio: "inherit", env });
    const forward = (sig) => () => child.kill(sig);
    const handlers = ["SIGINT", "SIGTERM", "SIGHUP"].map((sig) => [sig, forward(sig)]);
    for (const [sig, fn] of handlers) process.on(sig, fn);
    child.on("error", reject);
    child.on("exit", (code, signal) => {
      for (const [sig, fn] of handlers) process.off(sig, fn);
      resolve(code === null ? 128 + (os.constants.signals[signal] || 0) : code);
    });
  });
}

module.exports = { targetFor, cacheRoot, archiveName, ensureBinary, main, TARGETS };
