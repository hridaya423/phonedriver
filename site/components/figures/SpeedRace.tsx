import "./speed-race.css";
import { PHONE, ratio, speed } from "@/lib/bench";
import { SpeedRaceView, type Lane } from "./SpeedRaceView";

const ORDER = ["tree_list", "tree_settings", "screenshot", "type100", "tap_diff", "tap", "tap_paced"];

export function SpeedRace() {
  const ops = speed();
  const emulator: Lane[] = ORDER.map((id) => {
    const o = ops.find((x) => x.id === id)!;
    return { id, label: o.label, us: o.phonedriver, them: o.adb, ratio: ratio(o.adb / o.phonedriver), old: id === "screenshot" };
  });
  const phone: Lane[] = (
    [
      ["read", "UI tree, Display & brightness", PHONE.read],
      ["shot", "Screenshot", PHONE.shot],
    ] as const
  ).map(([id, label, [us, them]]) => ({ id, label, us, them, ratio: ratio(them / us), old: false }));
  return <SpeedRaceView emulator={emulator} phone={phone} phoneName={`${PHONE.name}, ${PHONE.os}`} />;
}
