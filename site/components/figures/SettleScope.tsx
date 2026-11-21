import { readFileSync } from "node:fs";
import { join } from "node:path";
import { ROOT } from "@/lib/site";
import "./settle-scope.css";
import { SettleScopeView } from "./SettleScopeView";

const num = (src: string, name: string) => +new RegExp(`${name}: (\\d+)`).exec(src)![1];

export function SettleScope() {
  const src = readFileSync(join(ROOT, "crates/core/src/settle.rs"), "utf8");
  const action = /pub fn action\(\) -> Config \{([\s\S]*?)\n    \}/.exec(src)![1];
  const first = num(action, "first_change_area");
  const sig = num(action, "significant_area");
  const expect = +/expect_change: Some\(Duration::from_millis\((\d+)\)\)/.exec(action)![1];
  const quiet = +/quiet: Duration::from_millis\((\d+)\)/.exec(action)![1];
  return <SettleScopeView first={first} sig={sig} expect={expect} quiet={quiet} />;
}
