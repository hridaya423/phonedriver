"use client";

import { useRef, useState } from "react";
import { Btn, Fig, Seg, ease, useClock } from "./kit";

const ROW = 34;
const POOL = 8;
const VIEW = 13 % POOL;
const TOP0 = 10;
const FLINGS = 4;
const PER = 4;
const HOLD = 1100;
const GAP = 1050;
const FLING = 700;
const TAP = HOLD + FLINGS * GAP + 250;
const LINES = [TAP, TAP + 450, TAP + 900, TAP + 1350, TAP + 1800, TAP + 2250];
const LOOP = TAP + 5600;
const STILL = TAP + 4000;

type Mode = "pd" | "id";

const scrollAt = (t: number) => {
  let s = TOP0;
  for (let k = 0; k < FLINGS; k++) s += PER * ease((t - HOLD - k * GAP) / FLING);
  return s;
};

const viewRow = (top: number) => {
  for (let r = Math.floor(top) - 1; r <= top + 8; r++) if (r % POOL === VIEW && r + 1 > top && r < top + 7.5) return r;
  return null;
};

export function RecycledRow({ node }: { node: string }) {
  const root = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const touch = useRef<HTMLElement>(null);
  const [mode, setMode] = useState<Mode>("pd");
  const [t, setT] = useState(STILL);
  const [tag, setTag] = useState<number | null>(29);
  const hop = useRef(0);
  const last = useRef<number | null>(29);
  const clock = useClock(
    root,
    (now) => {
      const lt = now % LOOP;
      const top = scrollAt(lt);
      if (list.current) list.current.style.transform = `translate3d(0,${-(top - 1) * ROW}px,0)`;
      if (touch.current) {
        const k = Math.floor((lt - HOLD) / GAP);
        const p = k >= 0 && k < FLINGS ? (lt - HOLD - k * GAP) / FLING : -1;
        touch.current.style.opacity = p >= 0 && p <= 1 ? String(Math.min(1, p * 6, (1 - p) * 4)) : "0";
        touch.current.style.transform = `translate3d(0,${(0.78 - 0.5 * ease(p)) * 100}%,0)`;
      }
      const r = viewRow(top);
      if (r !== last.current) {
        if (r !== null && last.current !== null) hop.current++;
        last.current = r;
        setTag(r);
      }
      setT(Math.floor(lt / 50) * 50);
    },
    { still: STILL, key: mode },
  );

  const top = t === STILL && !clock.seen ? scrollAt(STILL) : scrollAt(t);
  const refRow = mode === "pd" ? 13 : (tag ?? 29);
  const on = (i: number) => t >= LINES[i];
  const done = t >= LINES[5];
  const pick = (m: Mode) => {
    setMode(m);
    clock.seek(TAP - 200);
  };

  return (
    <Fig
      id="recycled-row"
      title="One view, many rows"
      controls={
        <>
          <Seg
            label="Ref rule"
            value={mode}
            onChange={pick}
            options={[
              ["id", "Follow the node ID"],
              ["pd", "phonedriver"],
            ]}
          />
          <Btn onClick={() => clock.seek(0)}>Replay</Btn>
        </>
      }
      caption={
        <>
          Android&apos;s <code>ListView</code> draws a long list with a few recycled views, so one platform node ID names a new row after every scroll. In a 1,000-row list on
          API 37, node <code>{node}</code> was &quot;Row 13&quot;, then &quot;Row 29&quot;. phonedriver keeps a ref on a relabeled node only outside a list, and only if it stayed
          within 8 points.
        </>
      }
    >
      <div className="rr" ref={root}>
        <div className="rr-phone" aria-hidden="true">
          <div className="rr-bar">
            <b>ListView</b>
            <span>1,000 rows</span>
          </div>
          <div className="rr-view">
            <div className="rr-list" ref={list} style={{ transform: `translate3d(0,${-(top - 1) * ROW}px,0)` }}>
              {Array.from({ length: 36 }, (_, i) => i + 1).map((r) => (
                <div className="rr-row" key={r} style={{ top: (r - 1) * ROW }}>
                  Row {r}
                </div>
              ))}
              <div className="rr-ref" style={{ top: (refRow - 1) * ROW + 2, opacity: mode === "id" && tag === null ? 0 : 1 }}>
                <b>@e12</b>
              </div>
              {tag !== null && (
                <div className="rr-tag" key={hop.current} data-hop={hop.current > 0 ? "" : undefined} style={{ top: (tag - 1) * ROW }}>
                  <span>node {node}</span>
                </div>
              )}
              {mode === "id" && done && <i className="rr-tap" data-go="" style={{ top: (29 - 1) * ROW + ROW / 2 }} />}
            </div>
            <i className="rr-touch" ref={touch} />
          </div>
        </div>
        <div className="rr-side">
          <div className="fx-term" role="img" aria-label={mode === "pd" ? "phonedriver refuses the stale ref" : "a tool that follows the node ID taps Row 29"}>
            <p className="cmd">$ phonedriver snapshot</p>
            <p>
              @e12 text &quot;Row 13&quot; <span className="dim">· node {node}</span>
            </p>
            <p className="dim" data-in={t >= HOLD ? "" : undefined} data-out={t < HOLD ? "" : undefined}>
              · scrolled {Math.round(top - TOP0)} rows
            </p>
            <p className="cmd" data-in={on(0) ? "" : undefined} data-out={on(0) ? undefined : ""}>
              $ phonedriver tap @e12
            </p>
            {mode === "pd" ? (
              <>
                <p className="dim" data-in={on(1) ? "" : undefined} data-out={on(1) ? undefined : ""}>
                  node {node}: still there
                </p>
                <p className="chg" data-in={on(2) ? "" : undefined} data-out={on(2) ? undefined : ""}>
                  label &quot;Row 13&quot; → &quot;Row 29&quot;, inside a list
                </p>
                <p className="bad" data-in={on(4) ? "" : undefined} data-out={on(4) ? undefined : ""}>
                  error: @e12 is no longer on screen
                </p>
                <p className="dim" data-in={on(5) ? "" : undefined} data-out={on(5) ? undefined : ""}>
                  hint: Run `phonedriver snapshot` for fresh refs.
                </p>
              </>
            ) : (
              <>
                <p className="dim" data-in={on(1) ? "" : undefined} data-out={on(1) ? undefined : ""}>
                  node {node}: still there
                </p>
                <p className="ok" data-in={on(4) ? "" : undefined} data-out={on(4) ? undefined : ""}>
                  ok tapped @e12
                </p>
              </>
            )}
          </div>
          <p className={mode === "pd" ? "fx-verdict good" : "fx-verdict bad"} data-wait={done ? undefined : ""}>
            {mode === "pd" ? (
              <>
                <b>Refused.</b> Row 13 left the screen, so @e12 did too.
                <span>Same node ID, same place, new label, in a list: another row.</span>
              </>
            ) : (
              <>
                <b>Tapped Row 29.</b> The reply still says ok.
                <span>The node ID stayed; the row it named did not.</span>
              </>
            )}
          </p>
        </div>
      </div>
    </Fig>
  );
}
