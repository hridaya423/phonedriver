"use client";

import { useRef, useState } from "react";
import { Btn, Fig, Seg, grouped, useClock } from "./kit";

type Cell = { passed: number; runs: number; tokens: number; calls: number; wall: number };
type Harness = "phonedriver" | "mobile-mcp" | "phone-harness" | "adb";
type Cells = Record<Harness, Cell | null>;
export type SnowJob = { id: string; cells: Cells };
type Mode = "tokens" | "calls" | "wall";

const TOOLS: Harness[] = ["phonedriver", "phone-harness", "mobile-mcp", "adb"];
const MODES = [
  ["tokens", "Tokens"],
  ["calls", "Calls"],
  ["wall", "Wall time"],
] as const;
const ALL = "all";
const TURN = 120;
const REST = 2600;
const DONE = 1e9;

const metric = (c: Cell, m: Mode) => (m === "tokens" ? c.tokens : m === "calls" ? c.calls : c.wall);
const show = (v: number, m: Mode) => (m === "wall" ? `${grouped(v)} s` : grouped(v, m === "calls" && v % 1 ? 1 : 0));
const times = (v: number) => `${v < 10 ? v.toFixed(1) : v.toFixed(0)}x`;

type Steps = { n: number; mean: number; tokens: number; bars: { w: number; h: number }[] };

function steps(calls: number, tokens: number): Steps {
  const mean = tokens / calls;
  const count = Math.ceil(calls);
  const bars = Array.from({ length: count }, (_, i) => {
    const w = Math.min(1, calls - i);
    const x = (i + w / 2) / calls;
    return { w, h: mean * (0.4 + 1.2 * x) };
  });
  return { n: calls, mean, tokens, bars };
}

const tally = (s: Steps, k: number) => (k >= s.bars.length ? s.tokens : s.bars.slice(0, k).reduce((a, b) => a + b.w * b.h, 0));

export function TokenSnowballView({ jobs, total }: { jobs: SnowJob[]; total: Cells }) {
  const root = useRef<HTMLDivElement>(null);
  const [mode, setMode] = useState<Mode>("tokens");
  const [sel, setSel] = useState(ALL);
  const [k, setK] = useState(DONE);

  const pick = (id: string) => {
    setSel(id);
    clock.seek(0);
  };
  const subject = (h: Harness) => {
    if (sel === ALL) {
      const t = total[h]!;
      return { calls: t.calls / jobs.length, tokens: t.tokens / jobs.length };
    }
    const c = jobs.find((j) => j.id === sel)!.cells[h]!;
    return { calls: c.calls, tokens: c.tokens };
  };
  const us = subject("phonedriver");
  const them = subject("mobile-mcp");
  const A = steps(us.calls, us.tokens);
  const B = steps(them.calls, them.tokens);
  const nmax = Math.max(A.n, B.n);
  const hmax = Math.max(...A.bars.map((x) => x.h), ...B.bars.map((x) => x.h));
  const loop = Math.ceil(nmax) * TURN + REST;

  const clock = useClock(
    root,
    (now) => {
      const next = now >= DONE ? DONE : Math.min(Math.floor((now % loop) / TURN), Math.ceil(nmax));
      setK((p) => (p === next ? p : next));
    },
    { still: DONE, key: `${sel}` },
  );

  const callsX = them.calls / us.calls;
  const ctxX = them.tokens / them.calls / (us.tokens / us.calls);
  const tokX = them.tokens / us.tokens;
  const label = sel === ALL ? "an average job of the nine" : sel;
  const scope = sel === ALL ? "Nine jobs" : sel;

  const lane = (h: Harness, s: Steps) => {
    const mine = h === "phonedriver";
    return (
      <div className={mine ? "ts-row us" : "ts-row"}>
        <p className="ts-hd">
          <span className="ts-who">{h}</span>
          <span className="ts-num">
            <b>{grouped(tally(s, k))}</b> tokens
          </span>
        </p>
        <div className="ts-plot" aria-hidden="true">
          <div className="ts-steps" style={{ width: `${(s.n / nmax) * 100}%` }}>
            {s.bars.map((b, i) => (
              <i key={i} data-on={i < k ? "" : undefined} style={{ flexGrow: b.w, height: `${(b.h / hmax) * 100}%` }} />
            ))}
          </div>
        </div>
        <p className="ts-sub">
          {grouped(s.n, s.n % 1 ? 1 : 0)} calls, {grouped(Math.round(s.mean))} tokens a call on average
        </p>
      </div>
    );
  };

  const stat = (h: Harness, j: SnowJob | null) => (j ? j.cells[h] : total[h]);
  const rowOf = (j: SnowJob | null) => {
    const live = TOOLS.map((h) => ({ h, c: stat(h, j) })).filter((x): x is { h: Harness; c: Cell } => !!x.c);
    const best = Math.min(...live.map((x) => metric(x.c, mode)));
    const top = Math.max(...live.map((x) => metric(x.c, mode)));
    const lowest = live.filter((x) => metric(x.c, mode) === best).map((x) => x.h);
    return { live, best, top, lowest };
  };
  const losses = (m: Mode) =>
    jobs
      .map((j) => {
        const live = TOOLS.map((h) => ({ h, v: j.cells[h] ? metric(j.cells[h]!, m) : null })).filter((x): x is { h: Harness; v: number } => x.v !== null);
        const best = Math.min(...live.map((x) => x.v));
        const mine = live.find((x) => x.h === "phonedriver")!.v;
        return mine > best ? { id: j.id, by: live.filter((x) => x.v === best).map((x) => x.h) } : null;
      })
      .filter((x): x is { id: string; by: Harness[] } => !!x);
  const lost = losses(mode);
  const noun = mode === "tokens" ? "tokens" : mode === "calls" ? "calls" : "wall time";

  const body = (j: SnowJob | null) => {
    const id = j ? j.id : ALL;
    const { live, best, top, lowest } = rowOf(j);
    const lose = j ? lost.find((l) => l.id === j.id) : undefined;
    return (
      <li key={id}>
        <button type="button" className="ts-job" aria-pressed={sel === id} onClick={() => pick(id)} data-total={j ? undefined : ""}>
          <span className="ts-id">
            {j ? j.id : "Nine jobs, summed"}
            {lose && <em>{lose.by.join(" and ")} lower</em>}
          </span>
          {TOOLS.map((h) => {
            const c = stat(h, j);
            if (!c) return <span key={h} className="ts-c none" data-h={h} data-label={h} />;
            const v = metric(c, mode);
            return (
              <span key={h} className={h === "phonedriver" ? "ts-c us" : "ts-c"} data-h={h} data-label={h} data-best={lowest.includes(h) ? "" : undefined}>
                <span className="ts-v">
                  <b>{show(v, mode)}</b>
                  <span className={c.passed < c.runs ? "miss" : undefined}>
                    {c.passed}/{c.runs}
                  </span>
                </span>
                <i style={{ width: `${(v / top) * 100}%` }} />
              </span>
            );
          })}
        </button>
      </li>
    );
  };

  const tokensLine = () => `${grouped(us.tokens)} against ${grouped(them.tokens)}`;

  return (
    <Fig
      id="token-snowball"
      title="Why tokens compound"
      controls={
        <>
          <Seg label="Measure" value={mode} onChange={setMode} options={MODES} />
          <Btn onClick={() => clock.seek(0)}>Replay</Btn>
        </>
      }
      caption={
        <>
          Every turn re-reads the conversation so far, so a job costs about its calls times the tokens each call carries. The runs logged a total and a call count per run, not
          tokens per turn: the staircase is drawn as a steady climb whose average is the measured tokens per call, and the totals, calls and ratios are measured. Per-job figures are
          medians of the same-device passes of 10 November 2026 (phonedriver three passes, the others two) on a Pixel Tablet API 37 emulator, from{" "}
          <code>docs/bench/jobs.json</code>. Tokens add input, output, cache creation and cache read.
        </>
      }
    >
      <div className="ts" ref={root}>
        <p className="ts-sel">
          One turn per step, same scale for both. Showing <b>{label}</b>
          {sel !== ALL && (
            <>
              {" "}
              <button type="button" className="ts-back" onClick={() => pick(ALL)}>
                back to the nine-job average
              </button>
            </>
          )}
        </p>
        {lane("phonedriver", A)}
        {lane("mobile-mcp", B)}
        <div className="ts-x" role="img" aria-label={`${scope}: mobile-mcp made ${times(callsX)} the calls, each carrying ${times(ctxX)} the tokens, so ${times(tokX)} the tokens: ${tokensLine()}`}>
          <p>
            <b>{times(callsX)}</b>
            <span>the calls</span>
          </p>
          <i aria-hidden="true">×</i>
          <p>
            <b>{times(ctxX)}</b>
            <span>tokens a call</span>
          </p>
          <i aria-hidden="true">=</i>
          <p className="out">
            <b>{times(tokX)}</b>
            <span>the tokens</span>
          </p>
        </div>
        <p className="ts-tot">
          {sel === ALL
            ? `An average job: ${tokensLine()} tokens. All nine: ${grouped(total.phonedriver!.tokens)} against ${grouped(Math.round(total["mobile-mcp"]!.tokens))}.`
            : `${sel}: ${tokensLine()} tokens.`}
        </p>

        <div className="ts-tbl">
          <p className="ts-th" aria-hidden="true">
            <span>Pick a job to redraw the staircase</span>
            {TOOLS.map((h) => (
              <span key={h}>{h}</span>
            ))}
          </p>
          <ul className="ts-list">
            {body(null)}
            {jobs.map((j) => body(j))}
          </ul>
        </div>
        <p className="fx-verdict good">
          <b>
            phonedriver is lowest or level on {noun} in {jobs.length - lost.length} of {jobs.length} jobs.
          </b>
          <span>
            {lost.length === 0
              ? "No job goes to another tool."
              : lost.map((l, i) => `${i ? "; " : ""}${l.by.join(" and ")} ${l.by.length > 1 ? "are" : "is"} lower on ${l.id}`).join("") + ". Orange square: lowest in the row. Red: runs that failed."}
          </span>
        </p>
      </div>
    </Fig>
  );
}
