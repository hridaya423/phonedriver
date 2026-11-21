import "./guard-playground.css";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { ROOT } from "@/lib/site";
import type { Lists } from "./guard-rules";
import { GuardPlaygroundView } from "./GuardPlaygroundView";

const LABELS = "crates/cli/src/daemon/guard/labels.rs";

export function GuardPlayground() {
  const src = readFileSync(join(ROOT, LABELS), "utf8");
  const lists = Object.fromEntries([...src.matchAll(/pub const (\w+): &\[&str\] = &\[([\s\S]*?)\];/g)].map(([, name, body]) => [name, [...body.matchAll(/"([^"]*)"/g)].map((m) => m[1])])) as Lists;
  return <GuardPlaygroundView lists={lists} />;
}
