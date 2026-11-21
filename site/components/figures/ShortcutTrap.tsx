import { readFileSync } from "node:fs";
import { join } from "node:path";
import { ROOT } from "@/lib/site";
import { board } from "@/lib/bench";
import { ShortcutTrapView, type TrapData } from "./ShortcutTrapView";
import "./shortcut-trap.css";

type Run = { harness: string; task: string; total_tokens: number; tool_calls: number; wall_s: number };

export function ShortcutTrap() {
  const runs: Run[] = JSON.parse(readFileSync(join(ROOT, "docs/bench/tokens.json"), "utf8"));
  const run = (harness: string) => runs.find((r) => r.task === "alarm-gym" && r.harness === harness)!;
  const pick = (r: Run) => ({ calls: r.tool_calls, tokens: r.total_tokens, wall: r.wall_s });
  const chrome = board().cell("phonedriver", "chrome-prefs")!;
  const data: TrapData = {
    adb: pick(run("adb")),
    found: pick(run("phonedriver run 1")),
    screen: [pick(run("phonedriver run 2")), pick(run("phonedriver run 3"))],
    chrome: { calls: chrome.calls, tokens: chrome.tokens, wall: chrome.wall },
  };
  return <ShortcutTrapView data={data} />;
}
