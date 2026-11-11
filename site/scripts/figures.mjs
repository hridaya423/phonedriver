import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dirname, "../..");
const fixtures = join(root, "crates/core/tests/fixtures/android");
const tree = JSON.parse(readFileSync(join(fixtures, "real_settings_main.json"), "utf8"));
const snap = readFileSync(join(root, "crates/core/src/snapshots/phonedriver_core__render__tests__real_settings_main.snap"), "utf8");
const text = snap.split("\n---\n")[1].trimEnd().split("\n");
const [, , width, height] = tree.windows[0].bounds;
const targets = tree.nodes.filter((n) => n.f.includes("v") && n.a?.some((a) => a === "tap" || a.startsWith("scroll")));
const lines = text.map((line) => {
  const ref = /^\s*(@e\d+)/.exec(line)?.[1];
  if (!ref) return { text: line };
  const [l, t, r, b] = targets[Number(ref.slice(2)) - 1].b;
  return { text: line, ref, box: [l / width, t / height, (r - l) / width, (b - t) / height].map((v) => Math.round(v * 1e4) / 1e4) };
});
if (lines.filter((l) => l.ref).length !== targets.length) throw new Error("refs and tappable nodes differ");
writeFileSync(join(import.meta.dirname, "../lib/screen.json"), JSON.stringify({ width, height, nodes: tree.nodes.length, bytes: Buffer.byteLength(text.join("\n")), lines }, null, 1) + "\n");
const jpeg = (from, name, ...ops) =>
  execFileSync("sips", [...ops, "-s", "format", "jpeg", "-s", "formatOptions", "82", from, "--out", join(import.meta.dirname, "../public/assets", name)]);
const hidden = join(root, "docs/bench/data/hidden-display-2026-11-08");
jpeg(join(fixtures, "real_settings_main.png"), "screen-settings.jpg", "--resampleWidth", "720");
jpeg(join(root, ".github/assets/demo-poster.png"), "demo-poster.jpg");
jpeg(join(hidden, "main-during.png"), "hidden-yours.jpg");
jpeg(join(hidden, "hidden-screen.png"), "hidden-agents.jpg", "-c", "300", "800", "--cropOffset", "1", "1");
