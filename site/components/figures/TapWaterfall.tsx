import "./tap-waterfall.css";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { ROOT } from "@/lib/site";
import { TapWaterfallView, type Phase, type Ab } from "./TapWaterfallView";

const KEYS = ["prepare", "input", "settle", "tree", "stabilize", "render"];

export function TapWaterfall() {
  const md = readFileSync(join(ROOT, "docs/audits/latency-profile-2026-11-07.md"), "utf8");
  const phases: Phase[] = KEYS.map((k) => {
    const m = md.match(new RegExp(`^\\| ${k}[^|]*\\| ([\\d.]+) \\| ([\\d.]+) \\|`, "m"));
    if (!m) throw new Error(`latency audit: no row for ${k}`);
    return { key: k, p50: Number(m[1]), p95: Number(m[2]) };
  });
  const taps = md.match(/(\d+) settled taps, p50 \/ p95 ms, total ([\d,]+) \/ ([\d,]+)/);
  const first = md.match(/base daemon (\d+) ms p50 \/ \d+ p95 \((\d+) samples\), new (\d+) \/ \d+ \((\d+)\)/);
  const second = md.match(/\((\d+) samples each, whole bench\): (\d+) \/ \d+ against (\d+) \/ \d+/);
  if (!taps || !first || !second) throw new Error("latency audit: totals not found");
  const ab: Ab = {
    taps: Number(taps[1]),
    total: Number(taps[2].replace(",", "")),
    first: { n: Number(first[2]), before: Number(first[1]), after: Number(first[3]) },
    second: { n: Number(second[1]), before: Number(second[2]), after: Number(second[3]) },
  };
  return <TapWaterfallView phases={phases} ab={ab} />;
}
