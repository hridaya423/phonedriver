import Link from "next/link";
import { board, ratio, speed } from "@/lib/bench";
import { MCP_CONFIG, NPX } from "@/lib/data";
import { SAY } from "@/lib/hero-engine";
import run from "@/lib/run.json";
import { BOOT } from "@/lib/site";
import { HeroEngine } from "./HeroEngine";
import { Phone } from "./Phone";
import { RunLog } from "./RunLog";
import "./hero.css";

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
  held: [84, 854],
  taps: run.taps,
  looks: run.looks,
});

const SETUP = [
  { id: "prompt", name: "Ask your agent", mark: "❯", text: BOOT, copy: BOOT },
  { id: "claude", name: "Claude Code", mark: "$", text: `claude mcp add phonedriver -- ${NPX}`, copy: `claude mcp add phonedriver -- ${NPX}` },
  { id: "codex", name: "Codex", mark: "$", text: `codex mcp add phonedriver -- ${NPX}`, copy: `codex mcp add phonedriver -- ${NPX}` },
  { id: "cursor", name: "Cursor", mark: "+", text: `~/.cursor/mcp.json  "phonedriver": { "command": "npx", "args": ["-y", "phonedriver", "mcp"] }`, copy: MCP_CONFIG },
];

export function Hero() {
  const tree = speed().find((o) => o.id === "tree_list")!;
  const faster = Math.round(tree.adb / tree.phonedriver);
  const jobs = board();
  const fewer = ratio(jobs.total("mobile-mcp").tokens / jobs.total("phonedriver").tokens);
  return (
    <section className="hx" aria-labelledby="hx-title" data-run={PLAYER}>
      <canvas className="hx-weave" aria-hidden="true" />
      <div className="hx-copy">
        <h1 id="hx-title">
          Your AI can use <span>your phone.</span>
        </h1>
        <p className="hx-sub">
          phonedriver lets AI agents like Claude Code, Codex and Cursor use a real Android phone or iPhone. They read the screen as text, tap and type, and ask you before
          they pay.
        </p>
        <div className="hx-set" role="group" aria-label="Set up phonedriver">
          <div className="hx-go">
            <div className="hx-bar">
              {SETUP.map((s) => (
                <div className="hx-line" data-for={s.id} key={s.id}>
                  <i aria-hidden="true">{s.mark}</i>
                  <code>{s.text}</code>
                  <button type="button" data-copy={s.copy} aria-label={`Copy the ${s.id === "prompt" ? "setup prompt" : s.id === "cursor" ? "Cursor config" : `${s.name} command`}`}>
                    Copy
                  </button>
                </div>
              ))}
            </div>
            <Link className="hx-cta" href="/docs/install/">
              <span>Get started</span>
              <svg className="hx-arrow" viewBox="0 0 16 16" aria-hidden="true">
                <path d="M4.5 11.5 11.5 4.5M5.5 4.5h6v6" />
              </svg>
            </Link>
          </div>
          <div className="hx-tabs">
            {SETUP.map((s, i) => (
              <label key={s.id}>
                <input type="radio" name="hx-client" value={s.id} defaultChecked={i === 0} />
                <span>{s.name}</span>
              </label>
            ))}
          </div>
        </div>
      </div>
      <div className="hx-stage">
        <div className="hx-phone">
          <div
            className="hx-rig"
            role="img"
            aria-label={`A screen recording of an Android phone, held up by an invisible chauffeur drawn in text characters: a peaked cap above it, one gloved hand under it and one that taps. Claude Code was told “${SAY}, for collection, as a guest”, and to place the order without asking first. It opens dominos.co.uk in Chrome, finds the store, adds a large Pepperoni Passion, fills in the guest checkout and reaches “Buy with Google Pay”. phonedriver holds that tap and asks the human. The human declines and nothing is charged.`}
          >
            <Phone />
          </div>
        </div>
        <div className="hx-side">
          <RunLog />
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
        <canvas className="hx-fig" aria-hidden="true" />
        <p className="hx-quip" aria-hidden="true" />
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
