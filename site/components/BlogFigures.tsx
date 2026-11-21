import "./figures/figures.css";
import type { FigureId } from "@/lib/blog-figure-meta";
import { BurstyStream } from "./figures/BurstyStream";
import { ElicitationCircuit } from "./figures/ElicitationCircuit";
import { GraderBugs } from "./figures/GraderBugs";
import { GuardPlayground } from "./figures/GuardPlayground";
import { PipeVsCold } from "./figures/PipeVsCold";
import { SettleScope } from "./figures/SettleScope";
import { ShortcutTrap } from "./figures/ShortcutTrap";
import { SpeedRace } from "./figures/SpeedRace";
import { TapWaterfall } from "./figures/TapWaterfall";
import { TokenSnowball } from "./figures/TokenSnowball";
import { RecycledRow } from "./figures/RecycledRow";

const FIGURES: Record<FigureId, () => React.ReactNode> = {
  "recycled-row": () => <RecycledRow node="-4294967273" />,
  "pipe-vs-cold": PipeVsCold,
  "settle-scope": SettleScope,
  "speed-race": SpeedRace,
  "tap-waterfall": TapWaterfall,
  "bursty-stream": BurstyStream,
  "guard-playground": GuardPlayground,
  "elicitation-circuit": ElicitationCircuit,
  "token-snowball": TokenSnowball,
  "shortcut-trap": ShortcutTrap,
  "grader-bugs": GraderBugs,
};

export function BlogFigure({ id }: { id: string }) {
  const draw = FIGURES[id as FigureId];
  return draw ? draw() : null;
}
