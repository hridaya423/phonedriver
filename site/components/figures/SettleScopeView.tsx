"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { Btn, Fig, Seg, clamp, grouped, useClock } from "./kit";

const W = 90;
const H = 200;
const SPAN = 1400;
const PACE = 0.6;
const FADE = 140;
const HOLD = 900;
const LO = 1;
const HI = Math.log10(400000);

type Case = "caret" | "toggle" | "spinner" | "page";
type Rule = "one" | "two";
type Frame = [ms: number, area: number];

const series = (from: number, step: number, areas: number[]): Frame[] => areas.map((a, i) => [from + i * step, a]);

const FRAMES: Record<Case, Frame[]> = {
  caret: [[100, 96], [600, 96], [1100, 96]],
  toggle: [[290, 176], [322, 312], [355, 432], [388, 396], [421, 288], [454, 208], [487, 136], [520, 72], [553, 40]],
  spinner: series(330, 50, [480, 448, 416, 480, 432, 464, 400, 480, 448, 416, 480, 432, 464, 400, 480, 448, 416, 480, 432, 464, 400, 480]),
  page: [[260, 13000], [292, 78640], [325, 152000], [358, 253952], [391, 196000], [424, 98000], [457, 41000], [490, 15000], [523, 6200], [556, 2400], [589, 900], [622, 300]],
};

const NAMES: Record<Case, string> = { caret: "a blinking caret", toggle: "a Material switch toggle", spinner: "a 48 dp spinner", page: "a screen transition" };
const TAP: Record<Case, [number, number]> = { caret: [24, 104], toggle: [76, 61], spinner: [45, 62], page: [70, 61] };

const CMD: Record<Case, string> = { caret: "@e2", toggle: "@e4", spinner: "@e3", page: "@e5" };
const NOTHING = "hint: nothing changed; the tap may have missed, or changed only what the element list does not show (a web checkbox): `phonedriver screenshot` shows the picture";

type Props = { first: number; sig: number; expect: number; quiet: number };

function run(frames: Frame[], first: number, sig: number, expect: number, quiet: number) {
  let last: number | null = null;
  let start: number | null = null;
  const counted = new Set<number>();
  for (let i = 0; i < frames.length; i++) {
    const [t, a] = frames[i];
    if (t >= (last === null ? expect : last + quiet)) break;
    if (a > sig || (last === null && a > first)) {
      last = t;
      start ??= t;
      counted.add(i);
    }
  }
  return last === null ? { changed: false, at: expect, start, last, counted } : { changed: true, at: last + quiet, start, last, counted };
}

const cellsIn = (x0: number, y0: number, w: number, h: number) => {
  const out: number[] = [];
  for (let y = y0; y < y0 + h; y++) for (let x = x0; x < x0 + w; x++) out.push(y * W + x);
  return out;
};

const byDistance = (cells: number[], cx: number, cy: number) =>
  [...cells].sort((a, b) => Math.hypot((a % W) - cx, ((a / W) | 0) - cy) - Math.hypot((b % W) - cx, ((b / W) | 0) - cy));

function useCells() {
  return useMemo(() => {
    const ring = cellsIn(38, 93, 14, 14)
      .filter((c) => {
        const r = Math.hypot((c % W) + 0.5 - 45, ((c / W) | 0) + 0.5 - 100);
        return r >= 4 && r <= 6.4;
      })
      .sort((a, b) => Math.atan2(((a / W) | 0) - 100, (a % W) - 45) - Math.atan2(((b / W) | 0) - 100, (b % W) - 45));
    const sw = cellsIn(70, 57, 13, 8);
    const all = cellsIn(0, 0, W, H);
    const page = byDistance(all, 70, 61);
    return (kind: Case, idx: number, count: number, n: number): number[] => {
      if (kind === "caret") return cellsIn(12, 56, 1, 6).slice(0, n);
      if (kind === "toggle") return byDistance(sw, 72 + (idx / (count - 1)) * 9, 61).slice(0, n);
      if (kind === "spinner") return Array.from({ length: Math.min(n, ring.length) }, (_, i) => ring[(idx * 7 + i) % ring.length]);
      return page.slice(0, n);
    };
  }, []);
}

const yOf = (a: number) => clamp((Math.log10(Math.max(a, 10)) - LO) / (HI - LO));

export function SettleScopeView({ first, sig, expect, quiet }: Props) {
  const root = useRef<HTMLDivElement>(null);
  const clip = useRef<HTMLDivElement>(null);
  const head = useRef<HTMLDivElement>(null);
  const cvs = useRef<HTMLCanvasElement>(null);
  const [kind, setKind] = useState<Case>("toggle");
  const [rule, setRule] = useState<Rule>("two");
  const [done, setDone] = useState(true);
  const [flip, setFlip] = useState(true);
  const [running, setRunning] = useState(true);
  const [round, setRound] = useState(0);
  const cells = useCells();

  const frames = FRAMES[kind];
  const firstArea = rule === "one" ? sig : first;
  const r = useMemo(() => run(frames, firstArea, sig, expect, quiet), [frames, firstArea, sig, expect, quiet]);
  const peak = frames.reduce((m, f) => Math.max(m, f[1]), 0);
  const peakIdx = frames.findIndex((f) => f[1] === peak);
  const end = r.at;

  const paint = (ms: number) => {
    const c = cvs.current;
    const g = c?.getContext("2d");
    if (!c || !g) return;
    const buf = new Uint32Array(W * H);
    frames.forEach(([t, a], i) => {
      const age = ms - t;
      if (age < 0 || age > FADE) return;
      const alpha = Math.round(165 * (1 - age / FADE));
      const px = ((alpha << 24) | (20 << 16) | (91 << 8) | 255) >>> 0;
      for (const c2 of cells(kind, i, frames.length, Math.round(a / 16))) buf[c2] = px;
    });
    g.putImageData(new ImageData(new Uint8ClampedArray(buf.buffer), W, H), 0, 0);
  };

  const draw = (ms: number, still: boolean) => {
    const p = still ? 1 : clamp(ms / end);
    const x = clamp(Math.min(ms, end) / SPAN) * 100;
    if (clip.current) clip.current.style.clipPath = `inset(0 ${100 - x}% 0 0)`;
    if (head.current) head.current.style.left = `${x}%`;
    setFlip(ms >= frames[0][0]);
    paint(still ? frames[peakIdx][0] + 8 : ms);
    return p;
  };

  const clock = useClock(
    root,
    (t) => {
      const ms = clock.reduced ? SPAN : t * PACE;
      draw(clock.reduced ? SPAN : ms, clock.reduced);
      if (clock.reduced) {
        setDone(true);
        return;
      }
      const fin = ms >= end;
      setDone(fin);
      if (ms > end + HOLD) setRunning(false);
    },
    { running, still: 0, key: `${kind}${rule}` },
  );

  useEffect(() => {
    draw(SPAN, true);
  }, []);

  const restart = (k: Case, rl: Rule) => {
    if (clip.current) clip.current.style.clipPath = "inset(0 100% 0 0)";
    if (head.current) head.current.style.left = "0";
    setFlip(false);
    setKind(k);
    setRule(rl);
    setDone(false);
    setRunning(true);
    setRound((n) => n + 1);
    clock.seek(0);
  };

  const area = Math.round(peak);
  const lines = reply(kind, rule, r.changed);
  const ok = kind === "caret" ? true : r.changed;
  const bad = kind !== "caret" && !r.changed;
  const xs = (ms: number) => `${(ms / SPAN) * 100}%`;

  return (
    <Fig
      id="settle-scope"
      title="What counts as a change"
      controls={
        <>
          <Seg
            label="Case"
            value={kind}
            onChange={(k) => restart(k, rule)}
            options={[
              ["caret", "Caret"],
              ["toggle", "Toggle"],
              ["spinner", "Spinner"],
              ["page", "Page change"],
            ]}
          />
          <Seg
            label="Rule"
            value={rule}
            onChange={(v) => restart(kind, v)}
            options={[
              ["one", `One: ${grouped(sig)}`],
              ["two", `Two phases`],
            ]}
          />
          <Btn onClick={() => restart(kind, rule)}>Tap again</Btn>
        </>
      }
      caption={
        <>
          Peak changed area per case from spike A3, 20 runs per case on the API 26 and 37 emulators: caret 96 pt², toggle 432, spinner 480, screen transition 13,000 to 254,000
          (<code>docs/spikes/android.md</code>, <code>docs/architecture.md</code> 4.6). Thresholds and windows are read from <code>crates/core/src/settle.rs</code>. The frame-by-frame
          series and their times are illustrative; only the peaks are measured. Zero of 20 toggle taps cleared {grouped(sig)} pt².
        </>
      }
    >
      <div className="ss" ref={root}>
        <span className="vh">
          {NAMES[kind]} peaks at {grouped(peak)} square points. With {rule === "one" ? `one threshold of ${grouped(sig)}` : `a ${first} first-change floor and a ${grouped(sig)} quiet ceiling`}, the reply is{" "}
          {r.changed ? "changed" : "no change"}.
        </span>
        <div className="ss-phone" aria-hidden="true">
          <div className="ss-screen" data-on={flip ? "" : undefined}>
            <i className="ss-bar" style={at(6, 8, 26, 4)} />
            {[28, 118, 140, 162, 184].map((y) => (
              <span key={y}>
                <i className="ss-bar" style={at(6, y, 34, 3)} />
                <i className="ss-rule" style={at(0, y + 12, W, 0)} />
              </span>
            ))}
            {kind === "caret" && (
              <>
                <i className="ss-box" style={at(6, 52, 78, 14)} />
                <i className="ss-caret" style={at(12, 56, 1, 6)} />
                <i className="ss-bar" style={at(6, 102, 22, 3)} />
              </>
            )}
            {kind === "toggle" && (
              <>
                <i className="ss-bar" style={at(6, 59, 36, 3)} />
                <i className="ss-pill" style={at(70, 57, 13, 8)}>
                  <b />
                </i>
              </>
            )}
            {kind === "spinner" && (
              <>
                <i className="ss-bar" style={at(6, 58, 34, 3)} />
                <i className="ss-btn" style={at(58, 54, 26, 11)} />
                <i className="ss-ring" style={at(39, 94, 12, 12)} />
              </>
            )}
            {kind === "page" && (
              <>
                <i className="ss-bar" style={at(6, 59, 36, 3)} />
                <i className="ss-chev" style={at(76, 58, 5, 5)} />
              </>
            )}
            <i className="ss-tap" key={round} style={at(TAP[kind][0] - 3, TAP[kind][1] - 3, 6, 6)} />
          </div>
          <canvas ref={cvs} width={W} height={H} className="ss-lit" />
        </div>
        <div className="ss-main">
          <div className="ss-plot" aria-hidden="true">
            {[10, 100, 1000, 10000, 100000].map((v) => (
              <span key={v} className="ss-y" style={{ bottom: `calc(30px + (100% - 30px) * ${yOf(v)})` }}>
                {v >= 1000 ? `${v / 1000}k` : v}
              </span>
            ))}
            {rule === "two" && <Line y={yOf(first)} text={`first change ${first} pt²`} />}
            <Line y={yOf(sig)} text={rule === "one" ? `one threshold ${grouped(sig)} pt²` : `quiet ceiling ${grouped(sig)} pt²`} up />
            <div className="ss-clip" ref={clip} style={{ clipPath: `inset(0 ${100 - (end / SPAN) * 100}% 0 0)` }}>
              {frames.map(([t, a], i) => (
                <i key={i} className="ss-b" data-n={r.counted.has(i) ? "" : undefined} style={{ left: xs(t), height: `calc((100% - 30px) * ${yOf(a)})` }} />
              ))}
              <span className="ss-ph a" style={{ left: 0, width: xs(r.start ?? expect) }}>
                {r.start === null ? `waited ${expect} ms` : "1 first"}
              </span>
              {r.changed && (
                <span className="ss-ph b" style={{ left: xs(r.last!), width: xs(quiet) }}>
                  2 quiet
                </span>
              )}
            </div>
            <div className="ss-head" ref={head} style={{ left: xs(end) }} />
            <span className="ss-peak">
              peak <b>{grouped(area)}</b> pt² · {grouped(area / 16)} cells
            </span>
          </div>
          <div className="fx-term" role="img" aria-label={`phonedriver reply: ${lines.map((l) => l[1]).join(" ")}`}>
            <p className="cmd">$ phonedriver tap {CMD[kind]}</p>
            {lines.map(([cls, text], i) => (
              <p key={`${kind}${rule}${i}`} className={cls} data-in={done ? "" : undefined} data-out={done ? undefined : ""}>
                {text}
              </p>
            ))}
          </div>
          <p className={ok ? "fx-verdict good" : "fx-verdict bad"} data-wait={done ? undefined : ""}>
            {verdict(kind, rule, r.changed, first, sig)}
          </p>
        </div>
      </div>
    </Fig>
  );
}

function Line({ y, text, up }: { y: number; text: string; up?: boolean }) {
  return (
    <i className="ss-line" data-up={up ? "" : undefined} style={{ bottom: `calc(30px + (100% - 30px) * ${y})` }}>
      <span>{text}</span>
    </i>
  );
}

const at = (x: number, y: number, w: number, h: number) => ({ left: `${(x / W) * 100}%`, top: `${(y / H) * 100}%`, width: `${(w / W) * 100}%`, height: `${(h / H) * 100}%` });

function reply(kind: Case, rule: Rule, changed: boolean): [string, string][] {
  if (!changed) return [["dim", `ok tapped ${CMD[kind]} · no change after 500ms`], ["dim", NOTHING]];
  if (kind === "toggle") return [["ok", `ok tapped ${CMD[kind]}`], ["chg", '~ @e4 switch "Dark theme" off → on']];
  if (kind === "spinner") return [["ok", `ok tapped ${CMD[kind]}`], ["chg", "+ @e8 progress"]];
  return [["ok", `ok tapped ${CMD[kind]} · new screen`], ["dim", "app com.android.settings (Settings)"]];
}

function verdict(kind: Case, rule: Rule, changed: boolean, first: number, sig: number) {
  if (kind === "caret")
    return (
      <>
        <b>Nothing changed.</b> The caret peaks at 96 pt².
        <span>It stays under the {first} pt² floor, so a blink never counts as the screen moving.</span>
      </>
    );
  if (!changed && kind === "toggle")
    return (
      <>
        <b>changed: false.</b> The switch flipped.
        <span>Its 432 pt² never passed {grouped(sig)}, in all 20 taps, so the first phase waited out its window and gave up.</span>
      </>
    );
  if (!changed)
    return (
      <>
        <b>changed: false.</b> A spinner started.
        <span>Its 480 pt² frames are under {grouped(sig)}, so one threshold sees nothing happen.</span>
      </>
    );
  if (kind === "toggle")
    return (
      <>
        <b>Changed.</b> 432 pt² clears the {first} floor.
        <span>The quiet phase then ignores everything under {grouped(sig)} and ends after 250 ms.</span>
      </>
    );
  if (kind === "spinner")
    return (
      <>
        <b>Changed, and not held by the spinner.</b>
        <span>Its frames never pass {grouped(sig)}, so the pixels settle after 250 ms. The tree read then waits up to 4 s for it to clear and says loading if it has not.</span>
      </>
    );
  return (
    <>
      <b>New screen.</b> Both rules agree.
      <span>13,000 to 254,000 pt² is far above either threshold; settle waits until the motion drops under {grouped(sig)} for 250 ms.</span>
    </>
  );
}
