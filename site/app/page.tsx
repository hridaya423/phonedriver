import Link from "next/link";
import { Arrow, Html, JsonLd, Pane, Star } from "@/components/Bits";
import { DotsEngine, EndEngine, JourneyEngine, PalmEngine, PointEngine, RaceEngine, TypeEngine, ViewsEngine } from "@/components/Engines";
import { Hero } from "@/components/Hero";
import { Route } from "@/components/Route";
import { HARNESSES, PHONE, TASKS, VERSIONS, board, minutes, ms, op, ratio, short, tokens, type Cell, type Harness } from "@/lib/bench";
import { mark, PAINT } from "@/lib/chauffeur";
import { DEMO_CAPTION, FAQ, FAQ_MORE, HOME_DESC, JOBS, MCP_CONFIG, NPX } from "@/lib/data";
import { softwareLd } from "@/lib/ld";
import { inline, plain } from "@/lib/md";
import { meta } from "@/lib/meta";
import run from "@/lib/run.json";
import screen from "@/lib/screen.json";
import { BOOT, PUBLISHED, REPO, raw, url } from "@/lib/site";
import "./charts.css";
import "./home.css";

const CONFIG = MCP_CONFIG;
const CURSOR = `https://cursor.com/en/install-mcp?name=phonedriver&config=${Buffer.from(JSON.stringify({ command: "npx", args: ["-y", "phonedriver", "mcp"] })).toString("base64")}`;
const SLOW = 25;
const UNIT = 10000;
const CLIENTS = [
  { id: "prompt", name: "Ask your agent", say: BOOT },
  { id: "claude", name: "Claude Code", cmd: `claude mcp add phonedriver -- ${NPX}` },
  { id: "codex", name: "Codex", cmd: `codex mcp add phonedriver -- ${NPX}` },
  { id: "cursor", name: "Cursor", file: "~/.cursor/mcp.json" },
  { id: "other", name: "Other", file: "your client's MCP config" },
];

const STOPS = [
  ["real-jobs", "Tokens"],
  ["what-the-model-sees", "The read"],
  ["speed", "Speed"],
  ["asking-before-it-pays", "Approval"],
  ["hidden-display", "Your screen"],
  ["how-it-works", "How it works"],
  ["limits", "Limits"],
  ["install", "Set up"],
];

const ASKS = [
  "Open my app, sign up with a new account, and tell me every bug you find on the way. Give me steps to reproduce each one.",
  "Find the app that drains my battery and tell me how long it ran in the background today.",
  "Go through my app's onboarding in dark mode and in light mode, and show me every screen where the text is hard to read.",
];

const TAPDIFF = [
  "$ phonedriver tap @e24",
  'ok tapped @e24 "Dark theme" at 353,532 · settled in 2034ms',
  '~ @e23 button "Dark theme" "Will turn on when Bedtime starts" → "Will never turn off automatically"',
  '~ @e24 switch "Dark theme" off → on',
];

const RACE: [string, string][] = [
  ["tree_list", "Read the screen"],
  ["screenshot", "Take a screenshot"],
  ["type100", "Type 100 characters"],
  ["tap_diff", "Tap and see what changed"],
];

const TOOLS = ["devices", "apps", "snapshot", "screenshot", "tap", "type", "swipe", "scroll", "press", "launch", "wait", "batch", "clipboard", "shell"];

const DEVICES: [string, React.ReactNode][] = [
  ["Android emulator", "adb"],
  ["Android phone or tablet", "adb and USB debugging"],
  ["iOS Simulator", "a Mac with Xcode"],
  [
    "iPhone or iPad",
    <>
      a Mac with Xcode and an Apple Development certificate. <code>phonedriver ios setup</code> sets up the phone (<Link href="/docs/iphone/">guide</Link>)
    </>,
  ],
];

const ON_HOME = ["What is the best MCP server for controlling a phone?", "Is there an alternative to mobile-mcp?", "Is phonedriver an Appium alternative for AI agents?", "Can Codex or Cursor control a phone?", "Does it install anything on my phone?", "Does phonedriver send telemetry?"];
const HOME_FAQ = [...FAQ, ...FAQ_MORE.filter(([q]) => ON_HOME.includes(q))];

type Row = { k: string; text?: string; lines?: string[] };
const ROWS = run.rows as Row[];
const ASKED = ROWS.find((r) => r.k === "ask")!.lines!;
const PICK = 1100 + ASKED.length * 140 + 300;
const at = (ms: number) => ({ "--k": `${ms}ms` }) as React.CSSProperties;
const DECLINED = ROWS.find((r) => r.k === "declined")!.lines![0];

const REFS = screen.lines.filter((l) => l.ref);
const REST = REFS.findIndex((l) => l.text.includes("Display & touch")) + 1;

export const metadata = meta({ path: "/", title: "phonedriver: the MCP server that lets AI agents use a phone", description: HOME_DESC, og: "/assets/og-home.png" });

function Tokens({ c }: { c: Cell | null }) {
  if (!c) return <span className="jb-na">not run</span>;
  return (
    <>
      <span className="jb-n">{short(c.tokens)}</span>
      <span className={c.passed < c.runs ? "jb-p miss" : "jb-p"}>
        {c.passed}/{c.runs}
      </span>
    </>
  );
}

export default function Home() {
  const b = board();
  const totals = (["phonedriver", "phone-harness", "mobile-mcp"] as Harness[]).map((h) => ({ h, t: b.total(h) }));
  const most = Math.max(...totals.map((x) => x.t.tokens));
  const runs = totals.reduce((n, x) => n + x.t.runs, 0);
  const fewer = ratio(b.total("mobile-mcp").tokens / b.total("phonedriver").tokens);
  const race = RACE.map(([id, label]) => ({ label, o: op(id) }));
  const read = race[0].o;
  const ld = [
    ...softwareLd(),
    { "@context": "https://schema.org", "@type": "WebSite", name: "phonedriver", url: url("/"), inLanguage: "en" },
    {
      "@context": "https://schema.org",
      "@type": "VideoObject",
      name: "Claude Code orders a pizza on an Android phone with phonedriver and asks before it pays",
      description: DEMO_CAPTION,
      thumbnailUrl: url("/assets/demo-poster.jpg"),
      uploadDate: `${PUBLISHED}T00:00:00Z`,
      contentUrl: url("/assets/demo.mp4"),
      duration: "PT43S",
      width: 1280,
      height: 720,
    },
    {
      "@context": "https://schema.org",
      "@type": "FAQPage",
      mainEntity: HOME_FAQ.map(([q, a]) => ({ "@type": "Question", name: q, acceptedAnswer: { "@type": "Answer", text: plain(a) } })),
    },
  ];
  return (
    <>
      <JsonLd data={ld} />
      <Hero />
      <canvas className="weave" aria-hidden="true" />
      <nav className="dock" inert aria-label="On this page">
        <a className="dock-top" href="#main" aria-label="Back to the top" dangerouslySetInnerHTML={{ __html: mark("text", PAINT.icon, 30) }} />
        <ol className="dock-map">
          {STOPS.map(([id, name]) => (
            <li key={id}>
              <a href={`#${id}`}>
                <i aria-hidden="true" />
                <span>{name}</span>
              </a>
            </li>
          ))}
        </ol>
        <button type="button" className="btn dock-cp" data-copy={BOOT}>
          <span data-label>Copy the setup prompt</span>
        </button>
      </nav>

      <section className="stop" id="real-jobs">
        <Route n={1} say="Doesn't that eat my tokens?" short="And my tokens?" />
        <div className="wrap">
          <div className="hm-stat">
            <div>
              <h2>{fewer} fewer tokens than mobile&#8209;mcp on nine real phone jobs.</h2>
              <p className="lede">
                Tokens are what an agent&apos;s usage limit is spent on. I ran nine jobs that only a phone can do with Claude Sonnet 5.5 and three MCP servers, all on one
                Android emulator on {b.date}. A script checked the phone&apos;s own data afterwards, not the model&apos;s report.
              </p>
            </div>
            <p className="hm-big" aria-hidden="true" data-land="" data-at=".07">
              {fewer}
            </p>
          </div>
          <div className="df" data-life="" data-unit={UNIT} data-point=".df-row.us .df-field" data-point-x=".df-row.us .df-field i:last-child" data-point-at="760" data-point-edge="page">
            {totals.map(({ h, t }) => {
              const n = Math.round(t.tokens / UNIT);
              return (
                <div className={h === "phonedriver" ? "df-row us" : "df-row"} key={h}>
                  <p className="df-head">
                    <span className="df-who">
                      {h} <span>{VERSIONS[h]}</span>
                    </span>
                    <span className="df-num">
                      <b title={`${tokens(t.tokens)} tokens`} data-tokens={t.tokens}>
                        {short(t.tokens)}
                      </b>
                      <span>tokens</span>
                      <span className={t.passed < t.runs ? "miss" : undefined}>
                        {t.passed} of {t.runs} passed
                      </span>
                      <span>{minutes(t.wall)}</span>
                    </span>
                  </p>
                  <div className="df-field" aria-hidden="true" style={{ "--c6": Math.ceil(most / UNIT / 6), "--c12": Math.ceil(most / UNIT / 12) } as React.CSSProperties}>
                    {Array.from({ length: n }, (_, i) => (
                      <i key={i} />
                    ))}
                  </div>
                </div>
              );
            })}
            <p className="df-key">One dot is {tokens(UNIT)} tokens. Totals are sums of per-job medians over the nine jobs.</p>
            <canvas className="pf" aria-hidden="true" />
            <i className="pf-ring" aria-hidden="true" />
            <DotsEngine />
            <PointEngine host=".df" />
          </div>
          <details className="jl">
            <summary>
              <span>The nine jobs, one by one</span>
              <span>
                {TASKS.length} jobs, {runs} runs
              </span>
            </summary>
            <div className="jd">
              <p className="jd-head" aria-hidden="true">
                <span />
                <span>What Claude was asked to do</span>
                {totals.map(({ h }) => (
                  <span key={h}>{h}</span>
                ))}
                <span />
              </p>
              {TASKS.map((task, n) => {
                const info = JOBS.find(([id]) => id.split(" ")[0] === task)!;
                const all = HARNESSES.map((h) => [h, b.cell(h, task)] as const).filter((x): x is readonly [Harness, Cell] => !!x[1]);
                const top = Math.max(...all.map(([, c]) => c.tokens));
                const cells = totals.map(({ h }) => b.cell(h, task));
                const best = Math.min(...cells.filter((c): c is Cell => !!c && c.passed === c.runs).map((c) => c.tokens));
                return (
                  <details key={task} name="job" open={n === 0}>
                    <summary>
                      <span className="jd-n">{String(n + 1).padStart(2, "0")}</span>
                      <span className="jd-t">{info[1]}</span>
                      <span className="jd-cs">
                        {cells.map((c, i) => (
                          <span key={i} className={c?.tokens === best ? "jd-c best" : "jd-c"} data-label={totals[i].h}>
                            <span className="vh">{totals[i].h}: </span>
                            <Tokens c={c} />
                          </span>
                        ))}
                      </span>
                    </summary>
                    <div className="jd-card">
                      <div className="jd-bars">
                        {all.map(([h, c]) => (
                          <p key={h} className={h === "phonedriver" ? "jd-bar us" : "jd-bar"}>
                            <span>{h}</span>
                            <span className="jd-tr">
                              <i style={{ "--f": c.tokens / top } as React.CSSProperties} />
                            </span>
                            <b>{short(c.tokens)}</b>
                            <span className={c.passed < c.runs ? "miss" : undefined}>
                              {c.passed}/{c.runs}
                            </span>
                            <span>{c.calls} calls</span>
                            <span>{c.wall} s</span>
                          </p>
                        ))}
                      </div>
                      <p className="jd-why">
                        <b>Why it needs the phone.</b> {info[2]}.
                      </p>
                    </div>
                  </details>
                );
              })}
            </div>
            <p className="hm-foot">
              Each row shows median tokens, then runs passed. The orange square marks the fewest tokens among tools that passed every run: phone-harness wins two jobs.
              Open a row for tool calls, wall time and why the job cannot be done from a shell. This is my own test set. Every run, the versions, the checks and what
              was thrown away are <Link href="/benchmarks/#jobs">published</Link>. If you maintain one of these tools and think I set it up wrong,{" "}
              <a href={`${REPO}/issues`}>open an issue</a> and I will rerun it.
            </p>
          </details>
        </div>
      </section>

      <section className="stop" id="what-the-model-sees">
        <Route n={2} say="How is it that cheap?" />
        <div className="wrap">
          <div className="hm-stat">
            <div>
              <h2>It reads the screen as text.</h2>
              <p className="lede">Each screen arrives as a short numbered list, and the model taps by number. It takes a screenshot only when it needs to see a picture.</p>
            </div>
            <p className="hm-pair" aria-hidden="true">
              <span>
                {screen.nodes}
                <small>nodes on the screen</small>
              </span>
              <i>→</i>
              <b>
                {screen.lines.length}
                <small>lines for the model</small>
              </b>
            </p>
          </div>
          <div className="tv" data-on={REST} data-life="">
            <figure className="tv-shot">
              <div className="tv-screen">
                <img
                  src="/assets/screen-settings.jpg"
                  width={720}
                  height={1616}
                  alt="The Settings app on a Pixel 9 emulator: a search field, then rows for Google, Network & internet, Connected devices, Apps, Notifications, Sound & vibration, Modes, Display & touch, Wallpaper & style and Storage."
                  decoding="async"
                  loading="lazy"
                />
                {REFS.map((l, i) => (
                  <i
                    key={l.ref}
                    className="tv-box"
                    data-i={i + 1}
                    data-on={i + 1 === REST ? "" : undefined}
                    style={{ left: `${l.box![0] * 100}%`, top: `${l.box![1] * 100}%`, width: `${l.box![2] * 100}%`, height: `${l.box![3] * 100}%` }}
                  >
                    <b>{l.ref}</b>
                  </i>
                ))}
              </div>
              <figcaption>
                What you see <span>{screen.nodes} nodes in Android&apos;s accessibility tree</span>
              </figcaption>
            </figure>
            <div className="tv-text">
              <div data-land="" data-at=".5">
                <Pane title="phonedriver snapshot" note={`${screen.lines.length} lines`} label="phonedriver snapshot output">
                  <ol className="tv-lines" tabIndex={0} aria-label="The same screen as phonedriver's text. Up and down arrows move between elements.">
                    {screen.lines.map((l, i) => {
                      const n = l.text.length - l.text.trimStart().length;
                      const at = l.ref ? REFS.indexOf(l) + 1 : 0;
                      return l.ref ? (
                        <li key={i} data-i={at} data-on={at === REST ? "" : undefined} style={{ "--i": n } as React.CSSProperties}>
                          <b>{l.ref}</b>
                          {l.text.trimStart().slice(l.ref.length)}
                        </li>
                      ) : (
                        <li key={i} className="tv-meta">
                          {l.text}
                        </li>
                      );
                    })}
                  </ol>
                </Pane>
              </div>
              <p className="tv-cap">
                What the model sees. Android&apos;s accessibility tree for this screen has <b>{screen.nodes} nodes</b>; phonedriver sends these{" "}
                <b>{screen.lines.length} lines</b>. Point at a line to see what it is on the screen.
              </p>
              <Pane title="then, one screen later" note="only what changed" lines={TAPDIFF} label="phonedriver tap output" />
              <p className="tv-cap">
                After every tap, phonedriver waits until the screen stops changing and replies with the difference: <code>+</code> a line that appeared, <code>-</code>{" "}
                one that left, <code>~</code> one that changed. The model doesn&apos;t poll or guess how long a screen takes to load.
              </p>
            </div>
            <svg className="tv-link" aria-hidden="true">
              <path />
            </svg>
            <canvas className="tv-fx" aria-hidden="true" />
            <ViewsEngine />
          </div>
        </div>
      </section>

      <section className="stop" id="speed">
        <Route n={3} say="And is it quick?" />
        <div className="wrap">
          <div className="hm-head">
            <h2>{ratio(read.adb / read.phonedriver)} faster than adb at reading the screen.</h2>
            <p className="lede">
              A small helper process on the phone keeps the accessibility connection open, where adb starts a new process and writes XML on every call. Each ball below
              is one call, out to the phone and back.
            </p>
          </div>
          <figure className="race pane" data-slow={SLOW} data-land="" data-at=".72" data-life="" data-point="li:first-child .race-lane.us .race-ms" data-point-at="900">
            <figcaption className="pane-head">
              <span>four calls, slowed down {SLOW} times</span>
              <button type="button" className="race-btn" hidden>
                Play
              </button>
            </figcaption>
            <ol>
              {race.map(({ label, o }) => (
                <li key={o.id}>
                  <p className="race-name">
                    {label}
                    <b>{ratio(o.adb / o.phonedriver)}</b>
                  </p>
                  {(
                    [
                      ["phonedriver", o.phonedriver],
                      ["adb", o.adb],
                    ] as const
                  ).map(([who, t]) => (
                    <p className={who === "adb" ? "race-lane" : "race-lane us"} key={who}>
                      <span className="race-who">{who}</span>
                      <span className="race-track" aria-hidden="true">
                        <i data-ms={t} />
                      </span>
                      <span className="race-ms">{ms(t)} ms</span>
                    </p>
                  ))}
                </li>
              ))}
            </ol>
            <p className="race-cap">
              One second here is {1000 / SLOW} ms. Each ball leaves your agent on the left, reaches the phone on the right and comes back in the time one call takes: the
              median of 50 runs on an Android 17 emulator. adb is <code>adb shell uiautomator dump</code>, <code>screencap</code> and <code>input</code>.{" "}
              <Link href="/benchmarks/#speed">Method and raw data</Link>
            </p>
            <canvas className="pf" aria-hidden="true" />
            <i className="pf-ring" aria-hidden="true" />
            <RaceEngine />
            <PointEngine host=".race" />
          </figure>
          <dl className="hm-phone">
            <dt>
              On a real phone{" "}
              <span>
                {PHONE.name}, {PHONE.os}
              </span>
            </dt>
            <dd>
              <b>{ratio(PHONE.read[1] / PHONE.read[0])}</b> Reading the screen took {PHONE.read[0]} ms against {ms(PHONE.read[1])} ms.
            </dd>
            <dd>
              <b>{ratio(PHONE.shot[1] / PHONE.shot[0])}</b> A screenshot took {Math.round(PHONE.shot[0])} ms against {ms(PHONE.shot[1])} ms.
            </dd>
          </dl>
        </div>
      </section>

      <section className="stop" id="asking-before-it-pays">
        <Route n={4} say="Can it spend my money?" />
        <div className="wrap">
          <div className="hm-split rev">
          <div>
            <h2>It asks you before it taps Pay.</h2>
            <p className="lede">
              When a tap lands on a button labeled Pay, Buy now, Buy with GPay, Place order, Start subscription, Transfer or Delete account, phonedriver holds the tap and
              asks you in Claude Code&apos;s own prompt. The model never sees that question and can&apos;t answer it.
            </p>
          </div>
          <div className="hm-sticky">
            <div className="ask-stage" data-land="">
              <Pane title="Claude Code · Opus 5.5" note={`recorded ${run.date}`} label="The approval question from the recorded run">
                <div className="ask" data-life="">
                  <p className="ask-you" style={at(0)}>
                    <i aria-hidden="true">❯</i>
                    <span>
                      {ROWS[0].text!.split(". ")[0]}. <em>…</em>
                    </span>
                  </p>
                  <p className="ask-gap" style={at(500)}>
                    {run.calls} calls and {run.worked.replace("Worked for ", "")} later
                  </p>
                  <div className="ask-q" style={at(1100)}>
                    {ASKED.map((l, i) => (
                      <p key={i} style={at(1100 + i * 140)}>
                        {l}
                      </p>
                    ))}
                    <p className="ask-pick" style={at(PICK)}>
                      <i aria-hidden="true"> </i>✔ Your answer: Allow
                    </p>
                    <p className="ask-pick" style={at(PICK + 200)}>
                      <i className="ask-ca" aria-hidden="true" style={at(PICK + 500)}>
                        ❯
                      </i>
                      Accept
                      <i className="ask-cd" aria-hidden="true" style={at(PICK + 1100)}>
                        ❯
                      </i>
                      <b style={at(PICK + 1100)}>Decline</b>
                    </p>
                  </div>
                  <p className="ask-no" style={at(PICK + 1500)}>
                    {DECLINED}
                  </p>
                </div>
              </Pane>
              <canvas className="ask-fig" aria-hidden="true" />
              <PalmEngine />
            </div>
            <p className="tv-cap">
              From the run at the top of this page. phonedriver held the tap on Buy with GPay, the human said no, and nothing was charged.{" "}
              <a href="/assets/demo.mp4">Watch the whole run (MP4, 43 s)</a>
            </p>
          </div>
          </div>
            <p className="hm-say">
              The human said no. <b>Nothing was charged.</b>
            </p>
            <dl className="hm-facts">
              <div>
                <dt>You answer</dt>
                <dd>
                  Allow, Allow for this session, or Decline. In a client that can&apos;t show such a prompt, the tap waits and the model has to ask you in chat.
                </dd>
              </div>
              <div>
                <dt>Eleven languages</dt>
                <dd>
                  It reads button labels in English, Spanish, Portuguese, French, German, Italian, Dutch, Japanese, Chinese (Simplified), Korean and Hindi. It does not
                  catch a payment button that is only an icon, or one labeled in another language.
                </dd>
              </div>
              <div>
                <dt>More to ask about</dt>
                <dd>
                  Set <code>PHONEDRIVER_ASK=send,delete,install</code> to be asked before it sends a message, deletes something or installs an app.
                </dd>
              </div>
              <div>
                <dt>Passwords</dt>
                <dd>
                  The model can&apos;t type plain text into a password or one-time code field. It gets an error that tells it to use <code>secret</code>, which keeps
                  the text out of logs and traces.
                </dd>
              </div>
              <div>
                <dt>Your call</dt>
                <dd>
                  An agent you told to run without asking passes <code>autonomous: true</code>. <code>PHONEDRIVER_GUARD=off</code> turns the check off.{" "}
                  <Link href="/docs/approval-guard/">How the guard decides</Link>
                </dd>
              </div>
            </dl>
        </div>
      </section>

      <section className="stop" id="hidden-display" data-weave="thin">
        <Route n={5} say="Do I have to put my phone down?" short="And my own phone?" />
        <div className="wrap">
          <div className="hm-head">
            <h2>Your screen stays yours.</h2>
            <p className="lede">
              On Android 13 and later, the agent can work on a hidden second screen while you keep using the phone. Set <code>PHONEDRIVER_DISPLAY=hidden</code> in the MCP
              config, or pass <code className="nb">--display hidden</code> to the CLI.
            </p>
          </div>
          <div className="hd" data-life="">
            <figure>
              <img
                src="/assets/hidden-yours.jpg"
                width={1280}
                height={800}
                alt="An Android tablet's home screen with a clock, a calendar widget and the app dock, unchanged while the agent works."
                decoding="async"
                loading="lazy"
              />
              <figcaption>Your screen</figcaption>
            </figure>
            <figure data-land="" data-at=".62">
              <img
                src="/assets/hidden-agents.jpg"
                width={800}
                height={300}
                alt="A test app with the text “typed on the hidden screen” in its first field, on a screen that is not shown on the tablet."
                decoding="async"
                loading="lazy"
              />
              <figcaption>The hidden screen, same minute (its top left corner)</figcaption>
            </figure>
            <p className="hm-note">
              Tested on an emulator and on a {PHONE.name} ({PHONE.os}). Typing into an app&apos;s own fields leaves no trace on your screen. Typing into a web page shows
              the system&apos;s clipboard note, and your clipboard is put back after. The two captures are from one run on a tablet emulator on 8 November 2026.
            </p>
          </div>
        </div>
      </section>

      <section className="stop" id="how-it-works">
        <Route n={6} say="What is it made of?" />
        <div className="wrap">
          <div className="hm-head">
            <h2>How it works.</h2>
            <p className="lede">
              On Android there is no app to install. phonedriver pushes its helper over adb when it connects. On iOS it builds a small XCTest runner on your Mac.
            </p>
          </div>
          <div>
            <ol className="flow" data-land="" data-at=".705">
              <li>
                <h3>Your agent</h3>
                <p>Claude Code, Codex, Cursor or a script. It calls the tools over MCP, or runs the same commands in a shell.</p>
                <span className="flow-via">MCP or CLI</span>
                <span className="flow-tr" aria-hidden="true">
                  <i />
                </span>
              </li>
              <li>
                <h3>A daemon on your computer</h3>
                <p>
                  It keeps the connection open and remembers which <code>@e</code> number is which element, so a tap still lands after a list has moved.
                </p>
                <span className="flow-via">adb or Xcode</span>
                <span className="flow-tr" aria-hidden="true">
                  <i />
                </span>
              </li>
              <li>
                <h3>A helper on the phone</h3>
                <p>
                  It reads the accessibility tree and streams the screen as H.264 video, which is how phonedriver knows when the screen has stopped changing. On iOS, an
                  XCTest runner does the same job.
                </p>
              </li>
            </ol>
            <div className="works" id="works-with">
              <div className="tw">
                <table>
                  <thead>
                    <tr>
                      <th scope="col">Works with</th>
                      <th scope="col">You need</th>
                    </tr>
                  </thead>
                  <tbody>
                    {DEVICES.map(([d, need]) => (
                      <tr key={d}>
                        <th scope="row">{d}</th>
                        <td>{need}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              <div>
                <h3 className="hm-count">
                  <b>{TOOLS.length}</b> tools, as an MCP server and a CLI
                </h3>
                <ul className="tools">
                  {TOOLS.map((t) => (
                    <li key={t}>{t}</li>
                  ))}
                </ul>
                <p className="hm-note">
                  phonedriver sends no telemetry. After the first download from GitHub it connects only to your device, and a test runs the whole tool in a sandbox that
                  blocks every other connection. <a href={raw("docs/architecture.md")}>Architecture</a> and <a href={raw("docs/decisions.md")}>design decisions</a> are in
                  the repository.
                </p>
              </div>
            </div>
            <dl className="hm-facts">
              <div>
                <dt>No cable</dt>
                <dd>
                  The cable can come off after setup. <code>phonedriver ios wifi on</code> keeps an iPhone in reach while it shares a Wi-Fi network with the Mac.{" "}
                  <code>phonedriver pair</code> or <code>phonedriver untether</code> does the same for an <Link href="/docs/android-phone/">Android phone</Link>.
                </dd>
              </div>
              <div>
                <dt>Slow links</dt>
                <dd>
                  When the phone is far away, on a tunnel or a mobile network, phonedriver sends a smaller screen stream and compressed screen trees. On a link shaped to
                  3G, the median tap on an emulator took 1.4 s, down from 2.8 s.
                </dd>
              </div>
              <div>
                <dt>30-second hand-over</dt>
                <dd>
                  A device you used from a terminal goes to your agent after 30 seconds idle. Your own <code>phonedriver snapshot</code> no longer blocks the agent for 10
                  minutes.
                </dd>
              </div>
            </dl>
          </div>
        </div>
      </section>

      <section className="stop" id="limits" data-weave="thin">
        <Route n={7} say="Anything I should know?" />
        <div className="wrap hm-rail">
          <h2>Limits and questions.</h2>
          <div>
            <ul className="limits" data-land="">
              <li>
                Tested on Android emulators (API 26 and 37), a {PHONE.name} ({PHONE.os}), the iOS Simulator, an iPhone 13 Pro Max, an iPhone 14 Plus and an iPad Pro. Other
                Android phones use the same path. Bug reports from them are welcome.
              </li>
              <li>
                A tap takes {Math.round(op("tap_paced").phonedriver)} to {Math.round(op("tap").phonedriver)} ms on an Android emulator and 450 to 530 ms on a real iPhone.
              </li>
              <li>The model can do anything on the phone you could do with your finger. For unattended runs, use an emulator or a spare phone.</li>
              <li>
                phonedriver never enters a passcode and can&apos;t pass Face ID or a fingerprint check. On a locked phone a command stops and says the device is locked, so
                a phone that locks mid-task stops the task. Before a long task on an iPhone, set Auto-Lock to Never. Screens that block screenshots, as some banking
                apps do, come out black in a screenshot, though their text can still be read.
              </li>
            </ul>
            <div className="faq" id="faq">
              {HOME_FAQ.map(([q, a], i) => (
                <details className="qa" name="faq" key={q} open={i === 0}>
                  <summary>
                    <h3>{q}</h3>
                  </summary>
                  <Html html={`<p>${inline(a)}</p>`} />
                </details>
              ))}
            </div>
          </div>
        </div>
      </section>

      <section className="stop hm-end" id="install">
        <Route n={8} say="Fine. Set it up." />
        <div className="wrap end">
          <div className="end-copy">
            <h2>Give your agent a phone.</h2>
            <p className="lede">
              Paste one prompt. Your agent checks your machine, adds the server and tells you what to plug in: an Android phone with USB debugging on, an emulator, or the
              iOS Simulator.
            </p>
            <p className="hm-end-links">
              <Link href="/docs/">
                Read the docs
                <Arrow />
              </Link>
              <a href={REPO}>
                <Star />
                Star it on GitHub
              </a>
              <span>Free and open source, AGPL-3.0</span>
            </p>
          </div>
          <div className="end-pane">
            <canvas className="end-fig" aria-hidden="true" />
            <EndEngine />
            <div className="ins" role="group" aria-label="Your client" data-land="">
              <div className="ins-tabs">
                {CLIENTS.map((c, i) => (
                  <label key={c.id}>
                    <input type="radio" name="client" value={c.id} defaultChecked={i === 0} />
                    <span>{c.name}</span>
                  </label>
                ))}
              </div>
              {CLIENTS.map((c) =>
                c.cmd ? (
                  <div className="ins-panel" data-for={c.id} key={c.id}>
                    <code>
                      <i aria-hidden="true">$ </i>
                      {c.cmd}
                    </code>
                    <button type="button" className="btn" data-copy={c.cmd}>
                      Copy
                    </button>
                  </div>
                ) : c.say ? (
                  <div className="ins-panel json" data-for={c.id} key={c.id}>
                    <blockquote className="ins-say" data-life="">
                      {c.say}
                      <TypeEngine />
                    </blockquote>
                    <p>
                      <span>Paste it into Claude Code, Codex, Cursor or any agent that can run commands.</span>
                      <button type="button" className="btn" data-copy={c.say}>
                        Copy the prompt
                      </button>
                    </p>
                  </div>
                ) : (
                  <div className="ins-panel json" data-for={c.id} key={c.id}>
                    <pre tabIndex={0} aria-label={`MCP server entry for ${c.file}`}>
                      <code>{CONFIG}</code>
                    </pre>
                    <p>
                      <span>Goes in {c.file}</span>
                      {c.id === "cursor" && (
                        <a className="btn ghost" href={CURSOR}>
                          Add to Cursor
                          <Arrow />
                        </a>
                      )}
                      <button type="button" className="btn" data-copy={CONFIG}>
                        Copy
                      </button>
                    </p>
                  </div>
                ),
              )}
            </div>
            <p className="hm-note">
              The first run downloads one binary (macOS, Linux or Windows) and caches it. To add the server by hand, pick your client above. Homebrew, the shell installer
              and real iPhones are in the <Link href="/docs/install/">install guide</Link>.
            </p>
          </div>
          <div className="end-asks">
            <h3 className="hm-kick">Then give it a job only a phone can do</h3>
            <ul className="asks">
              {ASKS.map((a) => (
                <li key={a}>
                  <p>{a}</p>
                  <button type="button" className="ask-cp" data-copy={a} aria-label="Copy this prompt">
                    <span data-label>Copy</span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        </div>
      </section>
      <JourneyEngine />
    </>
  );
}
