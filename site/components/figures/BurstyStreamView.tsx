"use client";

import { useRef, useState } from "react";
import { Btn, Fig, Seg, clamp, useClock } from "./kit";

type Rule = "alarm" | "last" | "barrier";
type Result = "ok" | "fail" | "stale";

const SPAN = 6000;
const SLOW = 1.4;
const REST = 2400;
const TAP = 1200;
const STALL = 1000;
const BEFORE = Array.from({ length: 10 }, (_, i) => i * 100);
const AFTER = [1900, 2000, 2100, 2200];
const FRAMES = [...BEFORE, ...AFTER];
const STEADY = Array.from({ length: SPAN / 100 }, (_, i) => i * 100);
const SHOTS = [500, 1300, 3000, 4000, 5000];
const LABEL: Record<Result, string> = { ok: "ok", fail: "fail", stale: "stale" };

const pct = (ms: number) => `${(ms / SPAN) * 100}%`;
const lastBefore = (t: number) => Math.max(...FRAMES.filter((f) => f <= t));
const nextAfter = (t: number) => FRAMES.find((f) => f > t) ?? SPAN;

function judge(rule: Rule, t: number): { result: Result; at: number } {
  const last = lastBefore(t);
  const old = t > TAP && last < TAP;
  if (rule === "alarm") return { result: t - last > STALL ? "fail" : old ? "stale" : "ok", at: t };
  if (rule === "last") return { result: old ? "stale" : "ok", at: t };
  return { result: "ok", at: old ? nextAfter(t) : t };
}

export function BurstyStreamView() {
  const root = useRef<HTMLDivElement>(null);
  const head = useRef<HTMLElement>(null);
  const [rule, setRule] = useState<Rule>("alarm");
  const [tm, setTm] = useState(SPAN);

  const clock = useClock(
    root,
    (now) => {
      const t = Math.min(SPAN, (now % (SPAN * SLOW + REST)) / SLOW);
      if (head.current) head.current.style.left = pct(t);
      setTm(Math.floor(t / 40) * 40);
    },
    { still: SPAN * SLOW, key: rule },
  );

  const shots = SHOTS.map((t) => ({ t, ...judge(rule, t) }));
  const bad = shots.filter((s) => s.result !== "ok");
  const failed = shots.filter((s) => s.result === "fail").length;
  const stale = shots.filter((s) => s.result === "stale").length;
  const summary = `Rule: ${rule === "alarm" ? "no frame for 1 s means the stream is dead" : rule === "last" ? "serve the last frame while the video socket is open" : "serve the last frame, but after an action wait for the frames still in the encoder"}. Of five screenshots in the illustration, ${failed} fail and ${stale} show the screen from before the tap.`;
  const seek = (e: React.MouseEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    clock.seek(clamp((e.clientX - r.left) / r.width) * SPAN * SLOW);
  };

  return (
    <Fig
      id="bursty-stream"
      title="Steady frames, or bursts and silence"
      controls={
        <>
          <Seg
            label="Screenshot rule"
            value={rule}
            onChange={(r) => {
              setRule(r);
              clock.seek(0);
            }}
            options={[
              ["alarm", "1 s alarm"],
              ["last", "Last frame"],
              ["barrier", "Barrier"],
            ]}
          />
          <Btn onClick={() => clock.seek(0)}>Replay</Btn>
        </>
      }
      caption={
        <>
          Over an SSH tunnel on API 37 the emulator&apos;s software encoder sent bursts of about 10 frames per second, then nothing for up to 5 s while the screen did not change; the
          1 s rule failed 18 of 40 idle screenshots. The timeline is an illustration: frame and screenshot times are drawn to show the mechanism, not measured. The fix of 0.1.5 is
          decision D74: after an action a screenshot waits for the frames the encoder still holds. Sources: <code>docs/decisions.md</code> (the task 2.7 correction under D10, and D74) and <code>docs/architecture.md</code>.
        </>
      }
    >
      <div className="bs" ref={root} role="group" aria-label={summary}>
        <p className="vh">{summary}</p>
        <div className="bs-plot" aria-hidden="true" onClick={seek}>
          <div className="bs-axis">
            {[0, 1, 2, 3, 4, 5, 6].map((s) => (
              <em key={s} style={s === 6 ? { right: 0 } : { left: pct(s * 1000) }} data-end={s === 6 ? "" : undefined}>
                {s}
                {s === 6 ? " s" : ""}
              </em>
            ))}
          </div>
          <div className="bs-grid">
            {[1, 2, 3, 4, 5].map((s) => (
              <i key={s} style={{ left: pct(s * 1000) }} />
            ))}
            <i className="bs-tap" style={{ left: pct(TAP) }}>
              <b>tap</b>
            </i>
          </div>
          <div className="bs-name">What I assumed: a steady stream</div>
          <div className="bs-track">
            {STEADY.map((f) => (
              <i key={f} style={{ left: pct(f) }} />
            ))}
          </div>
          <div className="bs-name">What arrived</div>
          <div className="bs-track" data-arrived="">
            {rule === "alarm" && (
              <span className="bs-dead" style={{ left: pct(lastBefore(SPAN) + STALL), width: pct(SPAN - lastBefore(SPAN) - STALL) }} data-on={tm >= lastBefore(SPAN) + STALL ? "" : undefined}>
                <em>stream dead?</em>
              </span>
            )}
            <span className="bs-enc" style={{ left: pct(TAP), width: pct(AFTER[0] - TAP) }}>
              <em>in encoder</em>
            </span>
            {FRAMES.map((f) => (
              <i key={f} style={{ left: pct(f) }} data-new={f > TAP ? "" : undefined} data-on={tm >= f ? "" : undefined} />
            ))}
          </div>
          <div className="bs-name">Screenshots</div>
          <div className="bs-track bs-shots">
            {shots.map((s) => s.at > s.t && <span key={s.t} className="bs-wait" style={{ left: pct(s.t), width: pct(clamp(tm, s.t, s.at) - s.t) }} />)}
            {shots.map((s, i) => {
              const done = tm >= s.at;
              const waiting = s.at > s.t && tm >= s.t && !done;
              return (
                <span key={s.t} className="bs-shot" style={{ left: pct(s.t) }} data-on={tm >= s.t ? "" : undefined}>
                  <i />
                  <b data-r={done ? s.result : waiting ? "wait" : undefined}>{done ? LABEL[s.result] : waiting ? "wait" : i + 1}</b>
                </span>
              );
            })}
          </div>
          <div className="bs-ph">
            <i ref={head} style={{ left: pct(SPAN) }} />
          </div>
        </div>
        <p className={bad.length ? "fx-verdict bad" : "fx-verdict good"} aria-hidden="true">
          {rule === "alarm" ? (
            <>
              <b>Silence read as a dead stream.</b>
              <span>The screen was idle and the stream was open, yet 18 of 40 idle screenshots failed. The one after the tap shows the old screen.</span>
            </>
          ) : rule === "last" ? (
            <>
              <b>Idle screenshots pass. The one after the tap shows the old screen.</b>
              <span>The frames the tap caused were still inside the encoder; the frame on hand predated it.</span>
            </>
          ) : (
            <>
              <b>Every screenshot shows the screen as it is.</b>
              <span>The one after the tap waits for the encoder. Measured after D74: 5 of 5 right after taps, 0.61 to 1.11 s each; before, 1 of 5, 1 of 5 and 0 of 5, 0.21 to 0.40 s.</span>
            </>
          )}
        </p>
      </div>
    </Fig>
  );
}
