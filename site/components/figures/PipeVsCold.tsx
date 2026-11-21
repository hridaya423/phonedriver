import { op } from "@/lib/bench";
import "./pipe-vs-cold.css";
import { PipeVsColdView } from "./PipeVsColdView";

export function PipeVsCold() {
  const o = op("tree_list");
  return <PipeVsColdView pd={o.phonedriver} adb={o.adb} />;
}
