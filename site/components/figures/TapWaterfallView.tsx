"use client";

import { useRef, useState } from "react";
import { Btn, Fig, Seg, clamp, useClock } from "./kit";

export type Phase = { key: string; p50: number; p95: number };
export type Ab = { taps: number; total: number; first: { n: number; before: number; after: number }; second: { n: number; before: number; after: number } };

type Mode = "before" | "agree" | "differ";

const SLOW = 3;
const REST = 2200;
const AXIS = 1000;
const QUIET = 250;
const PROBE_AT = 120;
const RTT = 100;

const pct = (ms: number) => `${(ms / AXIS) * 100}%`;
const fmt = (v: number) => (v < 1 ? v.toFixed(1) : String(v));

export function TapWaterfallView({ phases, ab }: { phases: Phase[]; ab: Ab }) {
  const root = useRef<HTMLDivElement>(null);
  const fills = useRef<(HTMLElement | null)[]>([]);
  const probe = useRef<HTMLElement>(null);
  const head = useRef<HTMLElement>(null);
  const read = useRef<HTMLElement>(null);
  const [mode, setMode] = useState<Mode>("before");
  const [done, setDone] = useState(true);

  const starts: number[] = [];
  phases.reduce((a, p) => (starts.push(a), a + p.p50), 0);
  const end = ab.total;
  const settle = phases.findIndex((p) => p.key === "settle");
  const quietAt = starts[settle] + phases[settle].p50 - QUIET;
  const probeAt = quietAt + PROBE_AT;

  const clock = useClock(
    root,
    (now) => {
      const lt = now % (end * SLOW + REST);
      const tm = Math.min(end, lt / SLOW);
      phases.forEach((p, i) => {
        const el = fills.current[i];
        if (el) el.style.transform = `scaleX(${clamp((tm - starts[i]) / Math.max(p.p50, 0.0001))})`;
      });
      if (probe.current) probe.current.style.transform = `scaleX(${clamp((tm - probeAt) / RTT)})`;
      if (head.current) head.current.style.left = pct(tm);
      if (read.current) read.current.textContent = String(Math.round(tm));
      setDone((d) => (d === tm >= end ? d : tm >= end));
    },
    { still: end * SLOW, key: mode },
  );

  const after = mode !== "before";
  const gone = (k: string) => after && (mode === "agree" ? k === "tree" || k === "stabilize" : false);
  const note = (k: string) => (mode === "agree" && k === "tree" ? "no read" : mode === "agree" && k === "stabilize" ? "skipped" : mode === "differ" && k === "stabilize" ? "no 100 ms sleep" : null);

  const summary = after
    ? `After the change, p50 ${ab.first.before} to ${ab.first.after} ms. Over a ${RTT} ms tunnel one probe read fits the quiet window and ${mode === "agree" ? "the reads agree, so stabilize is skipped" : "the reads disagree, so stabilize still runs"}.`
    : `A settled tap took ${ab.total} ms at p50. Host render took ${fmt(phases[phases.length - 1].p50)} ms.`;

  return (
    <Fig
      id="tap-waterfall"
      title={`Anatomy of a ${ab.total} ms settled tap`}
      controls={
        <>
          <Seg
            label="Before or after the change"
            value={mode}
            onChange={(m) => {
              setMode(m);
              clock.seek(0);
            }}
            options={[
              ["before", "Before"],
              ["agree", "After: agree"],
              ["differ", "After: differ"],
            ]}
          />
          <Btn onClick={() => clock.seek(0)}>Replay</Btn>
        </>
      }
      caption={
        <>
          Median phase times of {ab.taps} settled taps through an SSH tunnel with about {RTT} ms per round trip, Pixel_API_37 emulator, from the 2026-11-07 latency audit. The
          audit has no phase breakdown after its change, so the After views draw the mechanism on the before timeline: a tree read starting {PROBE_AT} ms into the {QUIET} ms quiet
          window. If two reads agree there is no read and no <code>stabilize</code> afterwards; <code>stabilize</code> also skips its 100 ms sleep when its starting tree is at least
          100 ms old. Only the totals are measured: interleaved A/B, {ab.first.n} taps each, then {ab.second.n} each. Source: <code>docs/audits/latency-profile-2026-11-07.md</code>.
        </>
      }
    >
      <div className="tw" ref={root} role="group" aria-label={summary}>
        <p className="vh">{summary}</p>
        <div className="tw-top" aria-hidden="true">
          {after ? (
            <p className="tw-big">
              <s>{ab.first.before}</s> <u>→</u> <b>{ab.first.after}</b>
              <i>ms p50, interleaved</i>
            </p>
          ) : (
            <p className="tw-big">
              <b ref={read}>{ab.total}</b>
              <i>ms since the tap was sent</i>
            </p>
          )}
        </div>
        <div className="tw-plot" aria-hidden="true">
          <div className="tw-axis">
            <span />
            <div>
              {[0, 250, 500, 750, 1000].map((v) => (
                <em key={v} style={{ left: pct(v) }} data-end={v === 1000 ? "" : undefined}>
                  {v}
                </em>
              ))}
            </div>
          </div>
          <div className="tw-grid">
            {[250, 500, 750, 1000].map((v) => (
              <i key={v} style={{ left: pct(v) }} />
            ))}
          </div>
          {phases.map((p, i) => {
            const sliver = p.p50 < 1;
            const ghost = gone(p.key);
            const n = note(p.key);
            return (
              <div className="tw-row" key={p.key} data-k={p.key}>
                <div className="tw-lab">
                  <b>{p.key}</b>
                  <span>
                    {fmt(p.p50)} ms <u>p95 {fmt(p.p95)}</u>
                  </span>
                </div>
                <div className="tw-track">
                  <span className="tw-bar" style={{ left: pct(starts[i]), width: sliver ? "1px" : pct(p.p50) }} data-ghost={ghost ? "" : undefined} data-us={sliver ? "" : undefined}>
                    <i ref={(el) => void (fills.current[i] = el)} />
                    {ghost && n && <em>{n}</em>}
                  </span>
                  {sliver && (
                    <span className="tw-call" style={{ right: "9px" }}>
                      host render: {fmt(p.p50)} ms
                    </span>
                  )}
                  {after && p.key === "settle" && (
                    <>
                      <span className="tw-quiet" style={{ left: pct(quietAt), width: pct(QUIET) }} />
                      <span className="tw-probe" style={{ left: pct(probeAt), width: pct(RTT) }}>
                        <i ref={probe} />
                      </span>
                      <span className="tw-probe" data-no="" style={{ left: pct(probeAt + RTT), width: pct(RTT) }} />
                    </>
                  )}
                </div>
              </div>
            );
          })}
          <div className="tw-ph">
            <i ref={head} style={{ left: pct(end) }} />
          </div>
        </div>
        {after && (
          <p className="tw-key" aria-hidden="true">
            <span data-k="q">quiet window, {QUIET} ms</span>
            <span data-k="p">probe read, {PROBE_AT} ms in</span>
            <span data-k="n">a second read would end after the window</span>
            {mode === "differ" && <span data-k="s">stabilize runs, without its 100 ms sleep</span>}
          </p>
        )}
        <p className="fx-verdict good" data-wait={clock.reduced || done || after ? undefined : ""} aria-hidden="true">
          {after ? (
            <>
              <b>{ab.first.before} to {ab.first.after} ms.</b> {ab.second.before} to {ab.second.after} on the second set.
              <span>
                Only one {RTT} ms read fits the quiet window; the dashed read would end after it closes. The gain is 40 to 75 ms, inside the noise of a bare tap.
              </span>
            </>
          ) : (
            <>
              <b>{fmt(phases[phases.length - 1].p50)} ms of {ab.total} is ours.</b>
              <span>The rest is the tunnel and waiting for the screen to stop moving.</span>
            </>
          )}
        </p>
      </div>
    </Fig>
  );
}
