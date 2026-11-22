"use client";

import { useRef, useState } from "react";
import { Btn, Fig, Seg, clamp, grouped, useClock } from "./kit";

const FLOOR = 1000;
const HOLD = 700;
const STAGES = [
  ["fork adb", "a new client process, every call"],
  ["start app_process", "a fresh JVM"],
  ["connect UiAutomation", "a new connection"],
  ["waitForIdle(1000, 10000)", "insists on 1 s with no accessibility events"],
  ["dump XML", "written to disk"],
  ["exit", "all of it thrown away"],
] as const;

type Speed = "slow" | "real";

export function PipeVsColdView({ pd, adb }: { pd: number; adb: number }) {
  const root = useRef<HTMLDivElement>(null);
  const fill = useRef<HTMLDivElement>(null);
  const ticks = useRef<HTMLDivElement>(null);
  const pdN = useRef<HTMLElement>(null);
  const adbN = useRef<HTMLElement>(null);
  const clk = useRef<HTMLElement>(null);
  const [speed, setSpeed] = useState<Speed>("slow");
  const [running, setRunning] = useState(true);
  const [stage, setStage] = useState<number>(STAGES.length);
  const [scrub, setScrub] = useState(adb);

  const reads = Math.floor(adb / pd);
  const idle = FLOOR / adb;
  const other = (1 - idle) / 5;
  const bounds = [other, other * 2, other * 3, other * 3 + idle, other * 4 + idle, 1];
  const pace = speed === "slow" ? 1 / 3 : 1;

  const draw = (ms: number) => {
    const p = clamp(ms / adb);
    const n = Math.min(reads, Math.floor(ms / pd));
    if (fill.current) fill.current.style.transform = `scaleX(${p})`;
    if (ticks.current) ticks.current.style.clipPath = `inset(0 ${100 - (n / reads) * 100}% 0 0)`;
    if (pdN.current) pdN.current.textContent = String(n);
    if (adbN.current) adbN.current.textContent = p >= 1 ? "1" : "0";
    if (clk.current) clk.current.textContent = grouped(Math.min(ms, adb), 1);
    setStage(p >= 1 ? STAGES.length : bounds.findIndex((b) => p < b));
  };

  const clock = useClock(
    root,
    (t) => {
      const ms = clock.reduced ? t : t * pace;
      draw(ms);
      if (ms > adb + HOLD * pace) setRunning(false);
    },
    { running, still: scrub, key: speed },
  );

  const done = stage === STAGES.length;
  const send = () => {
    setRunning(true);
    clock.seek(0);
  };

  return (
    <Fig
      id="pipe-vs-cold"
      title="One read, two ways"
      controls={
        <>
          <Seg
            label="Playback speed"
            value={speed}
            onChange={(s) => {
              setSpeed(s);
              send();
            }}
            options={[
              ["slow", "Slow, ÷3"],
              ["real", "Real time"],
            ]}
          />
          {clock.reduced ? (
            <label className="pv-scrub">
              <span>Time</span>
              <input type="range" min={0} max={Math.round(adb)} step={1} value={Math.round(scrub)} onChange={(e) => setScrub(+e.target.value)} />
            </label>
          ) : (
            <Btn primary onClick={send}>
              Send read
            </Btn>
          )}
        </>
      }
      caption={
        <>
          Reading a 1,000-row list on the Android 17 emulator, p50 of 50: <code>adb shell uiautomator dump</code> {grouped(adb, 1)} ms, <code>phonedriver snapshot --no-settle</code>{" "}
          {grouped(pd, 1)} ms (<code>docs/bench/speed.json</code>, <code>tree_list</code>). The time axis is to scale. Only the 1,000 ms floor of the idle wait is known; the other
          stages are drawn equal and their durations were not measured. Playback ÷3 is slowed down.
        </>
      }
    >
      <div className="pv" ref={root}>
        <span className="vh">
          One adb read takes {grouped(adb, 1)} milliseconds and starts a new process, a JVM and a UiAutomation connection every time. phonedriver keeps one helper and one socket open and reads
          in {grouped(pd, 1)} milliseconds, so it completes {reads} reads in the time adb completes one.
        </span>
        <div className="pv-lane" aria-hidden="true">
          <div className="pv-lh">
            <span className="pv-nm">
              <i className="pv-sq them" />
              adb <code>uiautomator dump</code>
            </span>
            <span className="pv-ct">
              <b ref={adbN}>1</b> read
            </span>
          </div>
          <div className="pv-track">
            <div className="pv-fill" ref={fill} style={{ transform: "scaleX(1)" }} />
            {STAGES.map(([name], i) => (
              <span key={name} className="pv-seg" data-idle={i === 3 ? "" : undefined} data-on={stage === i ? "" : undefined} style={{ left: `${(i === 0 ? 0 : bounds[i - 1]) * 100}%`, width: `${(bounds[i] - (i === 0 ? 0 : bounds[i - 1])) * 100}%` }}>
                {i + 1}
              </span>
            ))}
          </div>
        </div>
        <div className="pv-lane" aria-hidden="true">
          <div className="pv-lh">
            <span className="pv-nm">
              <i className="pv-sq" />
              phonedriver <code>snapshot</code>
            </span>
            <span className="pv-ct us">
              <b ref={pdN}>{reads}</b> reads
            </span>
          </div>
          <div className="pv-track pv-pipe">
            <div className="pv-ticks" ref={ticks}>
              <svg viewBox={`0 0 ${reads} 1`} preserveAspectRatio="none" shapeRendering="crispEdges">
                <path d={Array.from({ length: reads }, (_, k) => `M${k} 0h.5v1h-.5z`).join("")} />
              </svg>
            </div>
          </div>
          <div className="pv-axis">
            {[0, 500, 1000, 1500].map((ms) => (
              <span key={ms} style={{ left: `${(ms / adb) * 100}%` }}>
                {grouped(ms)}
              </span>
            ))}
            <span className="end">
              <b ref={clk}>{grouped(adb, 1)}</b> ms
            </span>
          </div>
        </div>
        <ol className="pv-steps">
          {STAGES.map(([name, note], i) => (
            <li key={name} data-on={stage === i ? "" : undefined} data-past={stage > i ? "" : undefined}>
              <span>{i + 1}</span>
              <b>{name}</b> <em>{note}</em>
            </li>
          ))}
        </ol>
        <p className="fx-verdict good" data-wait={done ? undefined : ""}>
          <b>{reads} reads</b> in the time adb takes for 1.
          <span>
            The helper and its socket were already open, so a read is one JSON request and its reply. adb pays the whole sequence again each time.
          </span>
        </p>
      </div>
    </Fig>
  );
}
