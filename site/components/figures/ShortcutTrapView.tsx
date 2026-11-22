"use client";

import { useRef, useState, type CSSProperties } from "react";
import { Btn, Fig, Seg, grouped, useClock } from "./kit";

type Stat = { calls: number; tokens: number; wall: number };
export type TrapData = { adb: Stat; found: Stat; screen: [Stat, Stat]; chrome: Stat };
type Mode = "shell" | "screen";
type Line = [cls: "cmd" | "dim" | "bad" | "chg", text: string];
type Kf = { at: number; lines: number; x: string; y: number; dur: number; ease: string; tap: number; hit: boolean; ok: boolean };

const LANE = "14px";
const PHONE = "calc(100% - 62px)";
const END = 144;
const ROW_Y = [48, 76, 104, 132];
const OUT = "cubic-bezier(.23,1,.32,1)";
const IN = "cubic-bezier(.55,0,.9,.4)";
const SPRING = "cubic-bezier(.34,1.56,.64,1)";
const LOOP = 10600;
const DONE = 1e9;

const start: Kf = { at: 0, lines: 0, x: LANE, y: 0, dur: 0, ease: OUT, tap: 0, hit: false, ok: false };
const seq = (patches: Partial<Kf>[]) => patches.reduce<Kf[]>((all, p) => [...all, { ...(all[all.length - 1] ?? start), dur: 0, ...p } as Kf], []);

const taps = (from: number, step: number): Partial<Kf>[] => ROW_Y.map((y, i) => ({ at: from + i * step, tap: i + 1, x: PHONE, y, dur: 380 }));

const PLANS: Record<`${"alarm" | "chrome"}-${Mode}`, Kf[]> = {
  "alarm-shell": seq([{ at: 0 }, { at: 500, lines: 1 }, { at: 900, lines: 2 }, { at: 1300, lines: 3 }, { at: 1900, y: END, dur: 1000, ease: OUT }, { at: 2900, ok: true }]),
  "alarm-screen": seq([{ at: 0 }, { at: 500, lines: 1 }, { at: 1000, lines: 2 }, { at: 1500, x: PHONE, y: 24, dur: 600 }, ...taps(2300, 600), { at: 4900, x: LANE, y: END, dur: 700 }, { at: 5700, ok: true }]),
  "chrome-shell": seq([
    { at: 0 },
    { at: 400, lines: 2 },
    { at: 800, lines: 4 },
    { at: 1200, lines: 6 },
    { at: 1900, y: 80, dur: 650, ease: IN },
    { at: 2550, hit: true },
    { at: 2750, y: 28, dur: 550, ease: SPRING },
    { at: 3500, x: PHONE, y: 24, dur: 600 },
    ...taps(4300, 600),
    { at: 6900, x: LANE, y: END, dur: 700 },
    { at: 7700, ok: true },
  ]),
  "chrome-screen": seq([{ at: 0 }, { at: 500, lines: 1 }, { at: 1000, lines: 2 }, { at: 1500, x: PHONE, y: 24, dur: 600 }, ...taps(2300, 600), { at: 4900, x: LANE, y: END, dur: 700 }, { at: 5700, ok: true }]),
};

const index = (plan: Kf[], t: number) => {
  let i = 0;
  while (i + 1 < plan.length && plan[i + 1].at <= t) i++;
  return i;
};

const ALARM_ROWS = ["Add alarm", "Time 7:30", "Label: Gym", "Weekdays"];
const CHROME_ROWS = ["Settings", "Search engine", "DuckDuckGo", "Secure connections"];

export function ShortcutTrapView({ data }: { data: TrapData }) {
  const root = useRef<HTMLDivElement>(null);
  const [mode, setMode] = useState<Mode>("shell");
  const [at, setAt] = useState<[number, number]>([99, 99]);
  const planA = PLANS[`alarm-${mode}`];
  const planC = PLANS[`chrome-${mode}`];
  const [s1, s2] = data.screen;
  const x1 = s1.tokens / data.adb.tokens;
  const x2 = s2.tokens / data.adb.tokens;
  const times = (v: number) => `${v < 10 ? v.toFixed(1) : v.toFixed(0)}x`;

  const clock = useClock(
    root,
    (now) => {
      const t = now >= DONE ? DONE : now % LOOP;
      const next: [number, number] = [index(planA, t), index(planC, t)];
      setAt((p) => (p[0] === next[0] && p[1] === next[1] ? p : next));
    },
    { still: DONE, key: mode },
  );
  const pick = (m: Mode) => {
    setMode(m);
    clock.seek(0);
  };

  const a = planA[Math.min(at[0], planA.length - 1)];
  const c = planC[Math.min(at[1], planC.length - 1)];

  const alarmLines: Line[] =
    mode === "shell"
      ? [
          ["cmd", "$ adb shell am start \\"],
          ["cmd", "    -a android.intent.action.SET_ALARM \\"],
          ["dim", "    --ei …alarm.HOUR 7 --ei …alarm.MINUTES 30 …"],
        ]
      : [
          ["cmd", "$ phonedriver, through the Clock screen"],
          ["dim", "Add alarm, hour, minute, label, weekday chips"],
        ];
  const chromeLines: Line[] =
    mode === "shell"
      ? [
          ["cmd", "$ adb shell am start -a …"],
          ["bad", "# no intent sets Chrome's search engine"],
          ["cmd", "$ adb shell cmd …"],
          ["bad", "# no cmd reaches it"],
          ["cmd", "$ adb shell settings put …"],
          ["bad", "# no settings key holds it"],
        ]
      : [
          ["cmd", "$ phonedriver, through Chrome's settings"],
          ["dim", "Search engine, DuckDuckGo, Secure connections"],
        ];

  const stat = (s: Stat, extra?: Stat) => (
    <p className="sx-stat">
      <b>{grouped(s.calls)}</b> calls <b>{grouped(s.tokens)}</b> tokens
      {extra && (
        <span>
          run 3: {grouped(extra.calls)} calls, {grouped(extra.tokens)} tokens
        </span>
      )}
    </p>
  );

  const col = (id: "alarm" | "chrome", kf: Kf, lines: Line[], rows: string[]) => {
    const alarm = id === "alarm";
    const dead = alarm && mode === "shell";
    const shellWall = !alarm;
    return (
      <div className="sx-col" data-id={id}>
        <p className="sx-tag">
          <span data-kept={alarm ? undefined : ""}>{alarm ? "Dropped" : "Kept"}</span>
          {alarm ? "Set a 7:30 alarm, labelled Gym, on weekdays" : "DuckDuckGo as Chrome's search engine, secure connections on"}
        </p>
        <div className="fx-term" aria-label="Commands" data-compact={mode === "screen" ? "" : undefined}>
          {lines.map(([cls, text], i) => (
            <p key={`${mode}${i}`} className={cls} data-in={i < kf.lines ? "" : undefined} data-out={i < kf.lines ? undefined : ""}>
              {text}
            </p>
          ))}
        </div>
        <div className="sx-lane" aria-hidden="true" data-hit={kf.hit ? "" : undefined}>
          <i className="sx-rail" />
          {shellWall && (
            <div className="sx-wall" data-live={mode === "shell" ? "" : undefined}>
              <b>Chrome&apos;s private Preferences file</b>
              <span>no intent, cmd or settings key</span>
            </div>
          )}
          <div className="sx-phone" data-dead={dead ? "" : undefined}>
            <span className="sx-ph-h">{alarm ? (dead ? "Clock, unused" : "Clock") : "Chrome"}</span>
            {rows.map((r, i) => (
              <span key={r} className="sx-ph-r" style={{ top: ROW_Y[i] - 14 } as CSSProperties} data-hit={kf.tap === i + 1 ? "" : undefined} data-done={kf.tap > i + 1 ? "" : undefined}>
                {r}
              </span>
            ))}
          </div>
          <i className="sx-ball" style={{ left: kf.x, top: kf.y, transitionDuration: `${kf.dur}ms`, transitionTimingFunction: kf.ease }} />
        </div>
        <div className="sx-check" data-ok={kf.ok ? "" : undefined}>
          <span>The check reads the phone, not the reply</span>
          <code>{alarm ? "alarm_templates  7|30|Gym|31|1" : "default_search_provider_data  DuckDuckGo"}</code>
          <b>{kf.ok ? "pass" : "waiting"}</b>
        </div>
        {alarm ? (mode === "shell" ? stat(data.adb) : stat(s1, s2)) : mode === "shell" ? <p className="sx-stat">The shell route has no way in. The screen is the only door.</p> : stat(data.chrome)}
      </div>
    );
  };

  const both = a.ok && c.ok;

  return (
    <Fig
      id="shortcut-trap"
      title="Two jobs, one benchmark"
      controls={
        <>
          <Seg
            label="Route the agent may take"
            value={mode}
            onChange={pick}
            options={[
              ["shell", "Shell allowed"],
              ["screen", "Screen only"],
            ]}
          />
          <Btn onClick={() => clock.seek(0)}>Replay</Btn>
        </>
      }
      caption={
        <>
          Left: the early token benchmark&apos;s <code>alarm-gym</code> job; raw adb finished it in {data.adb.calls} calls and {grouped(data.adb.tokens)} tokens in {data.adb.wall} s. phonedriver on the same
          task took {s1.calls} and {s2.calls} calls through the screen, and {data.found.calls} when it found the intent itself. The extras shown are Android&apos;s alarm intent extras; the run kept its counts, not its
          command line. Right: the <code>chrome-prefs</code> job, with phonedriver&apos;s median of three passes. Timings inside the animation are illustrative. From{" "}
          <code>docs/bench/tokens.json</code> (round 1, 7 November 2026), <code>docs/bench/jobs.json</code> (10 November) and <code>docs/bench/jobs.md</code>.
        </>
      }
    >
      <div className="sx" ref={root} role="group" aria-label={mode === "shell" ? "With the shell allowed, one intent sets the alarm, and no shell route reaches Chrome's search engine setting" : "With the screen as the only route, both jobs are done through the phone's UI"}>
        {col("alarm", a, alarmLines, ALARM_ROWS)}
        {col("chrome", c, chromeLines, CHROME_ROWS)}
      </div>
      <p className="fx-verdict good" data-wait={both ? undefined : ""}>
        {mode === "shell" ? (
          <>
            <b>One job is a command. The other has none.</b>
            <span>The alarm tests whether the model finds the intent, so it was cut. DuckDuckGo only goes through Chrome&apos;s screen, so it stays.</span>
          </>
        ) : (
          <>
            <b>
              Forced through the screen, the alarm cost {times(x1)} to {times(x2)} what adb paid.
            </b>
            <span>That gap measures the shortcut, not the tool. Run 1 found the intent and cost {grouped(data.found.tokens)} tokens.</span>
          </>
        )}
      </p>
    </Fig>
  );
}
