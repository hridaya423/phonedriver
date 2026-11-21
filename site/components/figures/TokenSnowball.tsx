import { HARNESSES, TASKS, board, type Cell, type Harness } from "@/lib/bench";
import { TokenSnowballView, type SnowJob } from "./TokenSnowballView";
import "./token-snowball.css";

export function TokenSnowball() {
  const b = board();
  const cells = (get: (h: Harness) => Cell | null) => Object.fromEntries(HARNESSES.map((h) => [h, get(h)])) as Record<Harness, Cell | null>;
  const jobs: SnowJob[] = TASKS.map((id) => ({ id, cells: cells((h) => b.cell(h, id)) }));
  const total = cells((h) => (h === "adb" ? null : b.total(h)));
  return <TokenSnowballView jobs={jobs} total={total} />;
}
