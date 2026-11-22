"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { Btn, Fig, Seg, easeInOut, useClock, useReduced } from "./kit";

type Node = "m" | "s" | "d" | "p" | "y";
type Tone = "cmd" | "dim" | "ok" | "bad" | "chg";
type Step = { from: Node; to: Node; ms: number; line?: [Tone, string] };
type Run = "ask" | "allow" | "session" | "decline" | "auto";

const LABEL = "Buy with GPay";
const ASKED = `"${LABEL}" spends money or cannot be undone: it needs the human's approval`;
const DONE = `ok tapped @e31 "${LABEL}"`;

const RUNS: Record<Run, Step[]> = {
  ask: [
    { from: "m", to: "s", ms: 750, line: ["cmd", `tools/call tap {"target":"${LABEL}"}`] },
    { from: "s", to: "d", ms: 500, line: ["dim", "daemon: the label is a payment"] },
    { from: "d", to: "s", ms: 500, line: ["bad", `needs_approval: ${ASKED}`] },
    { from: "s", to: "y", ms: 700, line: ["chg", "elicitation/create → your client, not the model"] },
  ],
  allow: [
    { from: "y", to: "s", ms: 650, line: ["ok", "you: Allow"] },
    { from: "s", to: "d", ms: 500, line: ["dim", `daemon: finds "${LABEL}" again on a fresh screen`] },
    { from: "d", to: "p", ms: 450, line: ["dim", "phone: tap"] },
    { from: "p", to: "d", ms: 300 },
    { from: "d", to: "s", ms: 400 },
    { from: "s", to: "m", ms: 750, line: ["ok", `${DONE}; approved by the human`] },
  ],
  session: [
    { from: "y", to: "s", ms: 650, line: ["ok", "you: Allow for this session"] },
    { from: "s", to: "d", ms: 500, line: ["dim", "daemon: payments allowed until it stops (kept in memory only)"] },
    { from: "d", to: "p", ms: 450, line: ["dim", "phone: tap"] },
    { from: "p", to: "d", ms: 300 },
    { from: "d", to: "s", ms: 400 },
    { from: "s", to: "m", ms: 750, line: ["ok", `${DONE}; approved by the human for this session`] },
  ],
  decline: [
    { from: "y", to: "s", ms: 650, line: ["bad", "you: Decline"] },
    { from: "s", to: "m", ms: 750, line: ["bad", "The human declined; nothing was tapped."] },
  ],
  auto: [
    { from: "m", to: "s", ms: 750, line: ["cmd", `tools/call tap {"target":"${LABEL}","autonomous":true}`] },
    { from: "s", to: "d", ms: 500, line: ["dim", "daemon: autonomous, so nobody is asked"] },
    { from: "d", to: "p", ms: 450, line: ["dim", "phone: tap"] },
    { from: "p", to: "d", ms: 300 },
    { from: "d", to: "s", ms: 400 },
    { from: "s", to: "m", ms: 750, line: ["ok", DONE] },
  ],
};

const NODES: [Node, string, string][] = [
  ["m", "Claude", "the model"],
  ["s", "phonedriver", "MCP server"],
  ["d", "daemon", "the guard"],
  ["p", "phone", LABEL],
];

const edge = (a: Node, b: Node) => [a, b].sort().join("");

export function ElicitationCircuitView() {
  const root = useRef<HTMLDivElement>(null);
  const map = useRef<HTMLDivElement>(null);
  const ball = useRef<HTMLElement>(null);
  const reduced = useReduced();
  const [mode, setMode] = useState<"ask" | "auto">("ask");
  const [runs, setRuns] = useState<Run[]>(["ask"]);
  const [done, setDone] = useState(RUNS.ask.length);
  const [going, setGoing] = useState(false);
  const started = useRef(false);

  const run = runs[runs.length - 1];
  const steps = runs.flatMap((r) => RUNS[r]);
  const before = runs.slice(0, -1).reduce((n, r) => n + RUNS[r].length, 0);
  const shown = steps.slice(0, before + done);
  const lit = new Set(shown.map((s) => edge(s.from, s.to)));
  const waiting = run === "ask" && done === RUNS.ask.length;
  const answer = runs.find((r) => r !== "ask" && r !== "auto");
  const asked = run !== "auto" && (run !== "ask" || done >= RUNS.ask.length);

  const at = useCallback((n: Node) => {
    const el = map.current?.querySelector<HTMLElement>(`[data-node=${n}]`);
    const box = map.current?.getBoundingClientRect();
    if (!el || !box) return [0, 0];
    const r = el.getBoundingClientRect();
    return [r.left - box.left + r.width / 2, r.top - box.top + r.height / 2];
  }, []);

  const clock = useClock(
    root,
    (t) => {
      const list = RUNS[run];
      let acc = 0;
      let i = 0;
      while (i < list.length && t >= acc + list[i].ms) acc += list[i++].ms;
      if (i !== done) setDone(i);
      if (i >= list.length) {
        setGoing(false);
        if (ball.current) ball.current.style.opacity = "0";
        return;
      }
      const s = list[i];
      const [x0, y0] = at(s.from);
      const [x1, y1] = at(s.to);
      const p = easeInOut((t - acc) / s.ms);
      if (ball.current) {
        ball.current.style.opacity = "1";
        ball.current.style.transform = `translate3d(${x0 + (x1 - x0) * p}px,${y0 + (y1 - y0) * p}px,0)`;
      }
    },
    { running: going, still: 0, key: runs.join() },
  );

  const play = (next: Run[]) => {
    setRuns(next);
    if (reduced) {
      setDone(RUNS[next[next.length - 1]].length);
      return;
    }
    setDone(0);
    setGoing(true);
    clock.seek(0);
  };

  useEffect(() => {
    if (clock.seen && !started.current && !reduced) {
      started.current = true;
      play(["ask"]);
    }
  });

  const pick = (m: "ask" | "auto") => {
    setMode(m);
    play([m]);
  };

  const verdict = waiting
    ? ["good", "Waiting for you.", "The tool call is still open. The model cannot see this prompt."]
    : run === "auto"
      ? ["bad", "Tapped. Nobody was asked.", "autonomous: true skips the guard, and the agent can pass it on its own."]
      : answer === "decline"
        ? ["good", "Declined.", "Nothing was tapped, and the model is told so."]
        : answer
          ? ["good", "Tapped, in the same call.", "One tool call: asked, answered, tapped."]
          : ["", "", ""];
  const settled = !going && (waiting || done === RUNS[run].length);

  return (
    <Fig
      id="elicitation-circuit"
      title="Who answers a held tap"
      controls={
        <>
          <Seg
            label="Call"
            value={mode}
            onChange={pick}
            options={[
              ["ask", "Ask the human"],
              ["auto", "autonomous: true"],
            ]}
          />
          <Btn onClick={() => play([mode])}>Replay</Btn>
        </>
      }
      caption={
        <>
          The daemon only refuses; the MCP server asks you through the client&apos;s own prompt (MCP elicitation), so the question goes around the model. Your answer finishes the
          tap inside the same tool call. The texts are phonedriver&apos;s own, from <code>crates/cli/src/mcp.rs</code> and <code>crates/core/src/error.rs</code>.
        </>
      }
    >
      <div className="ec" ref={root} data-mode={run === "auto" ? "auto" : "ask"}>
        <div className="ec-map" ref={map}>
          <div className="ec-node ec-you" data-node="y" data-on={lit.has(edge("s", "y")) ? "" : undefined}>
            <b>You</b>
            <span>your client&apos;s prompt</span>
          </div>
          {NODES.map(([n, name, sub], k) => (
            <div className={`ec-node ec-${n}`} data-node={n} key={n}>
              <b>{name}</b>
              <span>{sub}</span>
              {k < 3 && <i className={`ec-h ec-h${k}`} data-lit={lit.has(edge(n, NODES[k + 1][0])) ? "" : undefined} aria-hidden="true" />}
            </div>
          ))}
          <i className="ec-ball" ref={ball} aria-hidden="true" />
          <p className="ec-ghost" data-on={asked ? undefined : ""} data-auto={run === "auto" ? "" : undefined}>
            {run === "auto" ? "No prompt. The call carried autonomous: true, so nobody was asked." : "Your client's own prompt opens here, outside the model's view."}
          </p>
          <div className="ec-ask" data-on={asked ? "" : undefined}>
            <p className="ec-ask-t">phonedriver needs your approval</p>
            <p>{ASKED}</p>
            <div className="ec-ask-b">
              {(
                [
                  ["allow", "Allow"],
                  ["session", "Allow for this session"],
                  ["decline", "Decline"],
                ] as const
              ).map(([r, text]) => (
                <button type="button" key={r} disabled={!waiting} aria-pressed={answer === r} onClick={() => play(["ask", r])}>
                  {text}
                </button>
              ))}
            </div>
          </div>
        </div>
        <div className="fx-term ec-log" aria-live="polite">
          {shown.flatMap((s, i) =>
            s.line ? [
              <p className={s.line[0]} key={i} data-in="">
                {s.line[0] === "cmd" ? "$ " : ""}
                {s.line[1]}
              </p>,
            ] : [],
          )}
        </div>
        <p className={`fx-verdict ${verdict[0]}`} data-wait={settled ? undefined : ""}>
          <b>{verdict[1]}</b>
          <span>{verdict[2]}</span>
        </p>
      </div>
    </Fig>
  );
}
