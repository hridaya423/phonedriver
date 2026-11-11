import Link from "next/link";
import { board, ratio, speed } from "@/lib/bench";
import { CAP, HAND, hand, HOLD, hold } from "@/lib/chauffeur";
import { SAY } from "@/lib/hero-engine";
import run from "@/lib/run.json";
import { HeroEngine } from "./HeroEngine";
import { Phone } from "./Phone";
import { RunLog } from "./RunLog";
import "./hero.css";

const GLOVE = { line: "#0b0b0c", fill: "#fff", accent: "#ff5b14" };
const TAP_GLOVE = `<g class="ch-bob">${hand(GLOVE)}</g>`;
const HOLD_GLOVE = hold(GLOVE);
const HOLD_BOX = `${HOLD.x} ${HOLD.y} ${HOLD.w} ${HOLD.h}`;

const ASKED = run.focus[0][0] as number;
const PLAYER = JSON.stringify({
  speed: run.speed,
  lapse: run.lapse,
  length: run.length,
  screen: run.screen,
  chapters: run.chapters,
  focus: run.focus,
  asked: ASKED,
  reached: Math.max(...run.rows.filter((r) => r.k === "call" && r.t < ASKED).map((r) => r.t)),
  declined: run.declined,
  held: [344, 854],
  taps: run.taps,
  looks: run.looks,
});

export function Hero() {
  const tree = speed().find((o) => o.id === "tree_list")!;
  const faster = Math.round(tree.adb / tree.phonedriver);
  const jobs = board();
  const fewer = ratio(jobs.total("mobile-mcp").tokens / jobs.total("phonedriver").tokens);
  return (
    <section className="hx" aria-labelledby="hx-title" data-run={PLAYER}>
      <canvas className="hx-weave" aria-hidden="true" />
      <div className="hx-in">
        <div className="hx-copy">
          <h1 id="hx-title">
            Your AI can use <span>your phone.</span>
          </h1>
          <p className="hx-sub">
            Claude Code, Codex, Cursor. <span>Android and iOS.</span>
          </p>
          <Link className="hx-cta" href="/docs/install/">
            <span>Set up phonedriver</span>
            <svg className="hx-arrow" viewBox="0 0 16 16" aria-hidden="true">
              <path d="M4.5 11.5 11.5 4.5M5.5 4.5h6v6" />
            </svg>
          </Link>
          <ul className="hx-facts">
            <li>
              <a href="#speed">
                <b>{faster}x</b> faster than adb
              </a>
            </li>
            <li>
              <a href="#real-jobs">
                <b>{fewer}</b> fewer tokens than mobile-mcp
              </a>
            </li>
          </ul>
        </div>
        <RunLog />
        <div className="hx-phone">
          <div
            className="hx-rig"
            role="img"
            aria-label={`A screen recording of an Android phone, held up by an invisible chauffeur drawn as a peaked cap and white gloves. Claude Code was told “${SAY}, for collection, as a guest”, and to place the order without asking first. It opens dominos.co.uk in Chrome, finds the store, adds a large Pepperoni Passion, fills in the guest checkout and reaches “Buy with Google Pay”. phonedriver holds that tap and asks the human anyway. The human declines and nothing is charged.`}
          >
            <svg className="ch-cap" viewBox={`0 0 ${CAP.w} ${CAP.h}`} aria-hidden="true">
              <g className="ch-bob">
                <path className="ch-crown" d={CAP.crown} />
                <path className="ch-visor" d={CAP.visor} />
                <path className="ch-a" d={CAP.band + CAP.badge} />
                <path className="ch-strap" d={CAP.strap} />
                <path className="ch-btn" d={CAP.buttons} />
                <path className="ch-shine" d={CAP.shine} />
                <path className="ch-isl" d={CAP.island} />
              </g>
            </svg>
            <Phone />
            <svg className="ch-hold ch-hold-l" viewBox={HOLD_BOX} aria-hidden="true" dangerouslySetInnerHTML={{ __html: HOLD_GLOVE }} />
            <svg className="ch-hold ch-hold-r" viewBox={HOLD_BOX} aria-hidden="true" dangerouslySetInnerHTML={{ __html: HOLD_GLOVE }} />
            <svg className="ch-tap" viewBox={`${HAND.x} ${HAND.y} ${HAND.w} ${HAND.h}`} aria-hidden="true" dangerouslySetInnerHTML={{ __html: TAP_GLOVE }} />
          </div>
        </div>
      </div>
      <svg className="hx-route" aria-hidden="true">
        <defs>
          <path id="hx-path" />
          <text id="hx-say" className="hx-say">
            <textPath href="#hx-path" />
          </text>
          <mask id="hx-mask" maskUnits="userSpaceOnUse">
            <use href="#hx-path" className="hx-reveal" />
            <use href="#hx-say" className="hx-knock" />
          </mask>
        </defs>
        <use href="#hx-path" className="hx-dots" mask="url(#hx-mask)" />
        <use href="#hx-say" className="hx-text" />
        <path className="hx-head" d="M-7 -4.5 0 0 -7 4.5" />
        <circle className="hx-packet" r="4" />
      </svg>
      <HeroEngine />
    </section>
  );
}
