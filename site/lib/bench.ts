import { readFileSync } from "node:fs";
import { join } from "node:path";
import { ROOT } from "./site";

export type Op = {
  id: string;
  label: string;
  phonedriver: number;
  phonedriver_p95: number;
  adb: number;
  adb_p95: number;
  phone_harness: number | null;
  phone_harness_p95?: number | null;
  adb_ratio_ci: [number, number];
  phone_harness_ratio_ci?: [number, number];
};

type Run = { pass: string; harness: string; task: string; success: boolean; total_tokens: number; tool_calls: number; wall_s: number };
type Total = { passed: number; runs: number; sum_median_tokens: number; sum_median_tool_calls: number; sum_median_wall_s: number };
type Passes = { passes: string[] };
type Jobs = {
  runs: Run[];
  totals: Record<string, Total>;
  totals_2026_11_07: Record<string, Total>;
  same_device_2026_11_10: { device: string; phonedriver_cfa37b2: Passes; "mobile-mcp_1.0.16": Passes; "phone-harness_0.3.3": Passes; adb: Passes };
};

export type Cell = { passed: number; runs: number; tokens: number; calls: number; wall: number };

const json = <T>(rel: string): T => JSON.parse(readFileSync(join(ROOT, rel), "utf8"));

export const speed = () => json<{ ops: Op[] }>("docs/bench/speed.json").ops;
export const op = (id: string) => speed().find((o) => o.id === id)!;

export const HARNESSES = ["phonedriver", "mobile-mcp", "phone-harness", "adb"] as const;
export type Harness = (typeof HARNESSES)[number];

export const VERSIONS: Record<Harness, string> = { phonedriver: "0.1.0", "mobile-mcp": "1.0.16", "phone-harness": "0.3.3", adb: "" };

export const TASKS = ["qa-signup", "qa-shop", "relay-code", "chrome-prefs", "clock-settings", "messages-pin-archive", "photos-crop", "wakelock-restrict", "log-code"];

const median = (xs: number[]) => {
  const s = [...xs].sort((a, b) => a - b);
  const m = s.length >> 1;
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};

export function board() {
  const j = json<Jobs>("docs/bench/jobs.json");
  const d = j.same_device_2026_11_10;
  const passes: Record<Harness, string[]> = {
    phonedriver: d.phonedriver_cfa37b2.passes,
    "mobile-mcp": d["mobile-mcp_1.0.16"].passes,
    "phone-harness": d["phone-harness_0.3.3"].passes,
    adb: d.adb.passes,
  };
  const cell = (h: Harness, task: string): Cell | null => {
    const runs = j.runs.filter((r) => r.harness === h && r.task === task && passes[h].includes(r.pass));
    if (!runs.length) return null;
    return {
      passed: runs.filter((r) => r.success).length,
      runs: runs.length,
      tokens: median(runs.map((r) => r.total_tokens)),
      calls: median(runs.map((r) => r.tool_calls)),
      wall: median(runs.map((r) => r.wall_s)),
    };
  };
  const total = (h: Harness): Cell => {
    const t = j.totals[h];
    return { passed: t.passed, runs: t.runs, tokens: t.sum_median_tokens, calls: t.sum_median_tool_calls, wall: t.sum_median_wall_s };
  };
  for (const h of HARNESSES) {
    const sum = TASKS.reduce((n, t) => n + (cell(h, t)?.tokens ?? 0), 0);
    if (Math.abs(sum - total(h).tokens) > 1) throw new Error(`jobs.json: ${h} per-job medians sum to ${sum}, totals say ${total(h).tokens}`);
  }
  return { cell, total, before: j.totals_2026_11_07, device: d.device, date: "10 November 2026" };
}

export const DATA_FILES: Record<string, string> = {
  "speed.json": "docs/bench/speed.json",
  "jobs.json": "docs/bench/jobs.json",
  "tokens.json": "docs/bench/tokens.json",
  "speed-samples.csv": "docs/bench/data/speed-2026-11-11/samples.csv",
  "speed-samples-second-pass.csv": "docs/bench/data/speed-2026-11-11/second-pass/samples.csv",
  "speed-meta.json": "docs/bench/data/speed-2026-11-11/meta.json",
};

const grouped = (v: number, digits: number) => v.toLocaleString("en-US", { minimumFractionDigits: digits, maximumFractionDigits: digits });

export const fmt = (v: number | null | undefined) => (v == null ? "n/a" : v >= 1000 ? grouped(v, 1) : v.toFixed(1));
export const ms = (v: number) => grouped(v, 0);
export const ratio = (v: number) => (v < 10 ? v.toFixed(1) : v.toFixed(0)) + "x";
export const tokens = (v: number) => grouped(v, 0);
export const short = (v: number) => (v >= 1e6 ? `${(v / 1e6).toFixed(1)}M` : `${Math.round(v / 1e3)}k`);
export const minutes = (s: number) => `${(s / 60).toFixed(1)} min`;

export const PHONE = { name: "HONOR X8", os: "Android 13", read: [78.5, 2506.6], shot: [16.4, 1005.7] } as const;

export function benchmarksMd() {
  const b = board();
  let s =
    "# phonedriver benchmarks\n\nReal phone jobs (nine jobs checked from device state, Claude Sonnet 5.5, one Android emulator, 10 November 2026; sums of per-job medians):\n\n| Tool | Passed | Tokens | Tool calls | Wall time |\n|---|---|---|---|---|\n";
  for (const h of HARNESSES) {
    const t = b.total(h);
    s += `| ${h}${VERSIONS[h] ? ` ${VERSIONS[h]}` : " (2 mixed jobs)"} | ${t.passed}/${t.runs} | ${tokens(t.tokens)} | ${t.calls} | ${t.wall} s |\n`;
  }
  s += `\nmobile-mcp used ${ratio(b.total("mobile-mcp").tokens / b.total("phonedriver").tokens)} the tokens of phonedriver. phone-harness used ${ratio(b.total("phone-harness").tokens / b.total("phonedriver").tokens)} the tokens and passed ${b.total("phone-harness").passed} of ${b.total("phone-harness").runs} runs.\n\nSpeed, p50 and p95 in ms, Android 17 emulator, 50 runs after 20 warm-ups, tools interleaved on one host.\n\n| Operation | phonedriver p50 | p95 | adb p50 | p95 | phone-harness p50 | p95 | adb / phonedriver (95% CI) |\n|---|---|---|---|---|---|---|---|\n`;
  for (const o of speed())
    s += `| ${o.label} | ${fmt(o.phonedriver)} | ${fmt(o.phonedriver_p95)} | ${fmt(o.adb)} | ${fmt(o.adb_p95)} | ${fmt(o.phone_harness)} | ${fmt(o.phone_harness_p95)} | ${ratio(o.adb / o.phonedriver)} (${fmt(o.adb_ratio_ci[0])} to ${fmt(o.adb_ratio_ci[1])}) |\n`;
  s += `\nOn a ${PHONE.name} phone (${PHONE.os}): reading the screen ${PHONE.read[0]} ms against ${ms(PHONE.read[1])} ms for adb, a screenshot ${PHONE.shot[0]} ms against ${ms(PHONE.shot[1])} ms.\n`;
  return s;
}
