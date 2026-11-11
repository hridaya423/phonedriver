import Link from "next/link";
import { JsonLd } from "@/components/Bits";
import { PageTop } from "@/components/PageTop";
import { HARNESSES, PHONE, TASKS, VERSIONS, board, fmt, minutes, ms, op, ratio, short, speed, tokens, type Cell, type Harness, type Op } from "@/lib/bench";
import { JOBS } from "@/lib/data";
import { meta } from "@/lib/meta";
import { AUTHOR, PUBLISHER, REPO, TODAY, raw, url } from "@/lib/site";
import "../charts.css";

const THREE: Harness[] = ["phonedriver", "phone-harness", "mobile-mcp"];

export function generateMetadata() {
  const b = board();
  const fewer = ratio(b.total("mobile-mcp").tokens / b.total("phonedriver").tokens);
  const read = op("tree_list");
  return meta({
    path: "/benchmarks/",
    title: `phonedriver benchmarks: ${fewer} fewer tokens, ${ratio(read.adb / read.phonedriver)} faster than adb`,
    description: `Nine real phone jobs with Claude Sonnet 5.5: phonedriver used ${fewer} fewer tokens than mobile-mcp. Speed against adb with p50, p95 and intervals. Raw data.`,
    og: "/og/benchmarks.png",
    md: true,
  });
}

function Speed({ o }: { o: Op }) {
  const rows: [string, number | null, number | null | undefined][] = [
    ["phonedriver", o.phonedriver, o.phonedriver_p95],
    ["adb", o.adb, o.adb_p95],
    ["phone-harness", o.phone_harness, o.phone_harness_p95],
  ];
  const max = Math.max(...rows.map(([, , p95]) => p95 ?? 0));
  return (
    <div className="mblock">
      <h3>{o.label}</h3>
      {rows.map(([name, p50, p95]) => (
        <div className={name === "phonedriver" ? "mrow us" : "mrow"} key={name}>
          <span>{name}</span>
          {p50 == null ? (
            <span className="na">no equivalent</span>
          ) : (
            <>
              <span className="mbar" aria-hidden="true">
                <i style={{ "--f": p50 / max } as React.CSSProperties} />
                <b style={{ "--f": (p95 ?? 0) / max } as React.CSSProperties} />
              </span>
              <span className="mval">
                {fmt(p50)} ms<small>p95 {fmt(p95)}</small>
              </span>
            </>
          )}
        </div>
      ))}
      <p>
        {ratio(o.adb / o.phonedriver)} faster than adb (95% CI {fmt(o.adb_ratio_ci[0])} to {fmt(o.adb_ratio_ci[1])})
        {o.phone_harness && o.phone_harness_ratio_ci
          ? `, ${ratio(o.phone_harness / o.phonedriver)} faster than phone-harness (${fmt(o.phone_harness_ratio_ci[0])} to ${fmt(o.phone_harness_ratio_ci[1])})`
          : ""}
        .
      </p>
    </div>
  );
}

const TOKENS: [string, string, string][] = [
  ['alarm-gym: 7:30 AM weekday alarm "Gym" in Clock', "110,419 (19.5 calls)", "220,723 (19 calls)"],
  ["wiki-birthdate: Chrome, Wikipedia, Grace Hopper's birth date", "57,753 (7 calls)", "53,403 (7 calls)"],
  ["testbed-web-form: fill and submit a web form", "50,185 (7 calls)", "56,249 (6 calls)"],
  ["Sum of the three medians", "218,357", "330,375"],
];

const list = (xs: string[]) => (xs.length < 2 ? xs.join("") : `${xs.slice(0, -1).join(", ")} and ${xs[xs.length - 1]}`);

export default function Benchmarks() {
  const ops = speed();
  const b = board();
  const us = b.total("phonedriver");
  const mm = b.total("mobile-mcp");
  const adb = b.total("adb");
  const most = Math.max(...THREE.map((h) => b.total(h).tokens));
  const read = op("tree_list");
  const jobs = TASKS.map((task) => ({ task, info: JOBS.find(([id]) => id.split(" ")[0] === task)!, cells: HARNESSES.map((h) => [h, b.cell(h, task)] as [Harness, Cell | null]).filter(([, c]) => c) as [Harness, Cell][] }));
  const slower = jobs.flatMap(({ task, cells }) => cells.filter(([h, c]) => h !== "phonedriver" && c.wall < cells[0][1].wall).map(([h, c]) => `${h} on ${task} (${c.wall} s against ${cells[0][1].wall} s)`));
  const fastest = jobs.filter(({ cells }) => cells.every(([, c]) => c.wall >= cells[0][1].wall)).length;
  const mixed = jobs.filter(({ cells }) => cells.some(([h]) => h === "adb")).reduce((n, j) => n + j.cells[0][1].tokens, 0);
  const ld = [
    {
      "@context": "https://schema.org",
      "@type": "Dataset",
      name: "phonedriver benchmarks: real phone jobs (2026-11-10) and speed (2026-11-07)",
      description:
        "Tokens, tool calls, wall time and pass counts for phonedriver, mobile-mcp 1.0.16, phone-harness 0.3.3 and adb on nine phone jobs run by Claude Sonnet 5.5, and p50 and p95 latency of phonedriver, adb and phone-harness on an Android 17 emulator.",
      url: url("/benchmarks/"),
      license: "https://www.gnu.org/licenses/agpl-3.0.html",
      creator: AUTHOR,
      dateModified: TODAY,
      distribution: [
        { "@type": "DataDownload", encodingFormat: "application/json", contentUrl: url("/benchmarks/data/jobs.json") },
        { "@type": "DataDownload", encodingFormat: "text/csv", contentUrl: url("/benchmarks/data/speed-samples.csv") },
        { "@type": "DataDownload", encodingFormat: "application/json", contentUrl: url("/benchmarks/data/speed.json") },
      ],
    },
    {
      "@context": "https://schema.org",
      "@type": "TechArticle",
      headline: "phonedriver benchmarks: real phone jobs, speed and raw data",
      url: url("/benchmarks/"),
      dateModified: TODAY,
      author: AUTHOR,
      publisher: PUBLISHER,
      inLanguage: "en",
      about: { "@id": url("/#software") },
    },
  ];
  return (
    <>
      <JsonLd data={ld} />
      <PageTop
        say="Prove it."
        crumbs={[["Benchmarks", "/benchmarks/"]]}
        md
        mark={<p className="pt-big or">{ratio(mm.tokens / us.tokens)}</p>}
        cap={`fewer tokens than mobile-mcp on nine real phone jobs, ${b.date}`}
      >
        <h1>Benchmarks</h1>
        <p className="lede">
          On nine real phone jobs, mobile-mcp used {ratio(mm.tokens / us.tokens)} the tokens of phonedriver. Reading a screen takes {fmt(read.phonedriver)} ms where adb
          takes {fmt(read.adb)} ms. This page has every number, what was thrown away, and the raw data.
        </p>
        <ul className="jump">
          <li>
            <a href="#jobs">Real phone jobs</a>
          </li>
          <li>
            <a href="#speed">Speed</a>
          </li>
          <li>
            <a href="#tokens">An earlier token run</a>
          </li>
          <li>
            <a href="#data">Raw data</a>
          </li>
        </ul>
      </PageTop>
      <div className="wrap">
        <article className="prose">
          <div className="body">
            <h2 id="jobs">Nine jobs only a phone can do</h2>
            <p>
              The result of each job cannot be produced by adb, <code>am</code>, <code>cmd</code>, <code>settings</code>, <code>content</code> or a web search. A
              script checks each one from the phone&apos;s own data, never from the agent&apos;s report. All tools ran on the same emulator on {b.date} with Claude
              Sonnet 5.5 through <code>claude -p</code>, one agent at a time: phonedriver three passes, the others two. The phonedriver build is <code>cfa37b2</code>; release 0.1.0 is that build plus Windows build and test fixes. Totals are sums of per-job medians.
            </p>
            <div className="tk-wrap wide">
              <table className="tk full">
                <caption className="vh">Totals over nine jobs: tokens, tool calls, runs passed and time, per tool</caption>
                <thead>
                  <tr>
                    <th scope="col">Tool</th>
                    <th scope="col">Tokens, nine jobs</th>
                    <th scope="col">Tool calls</th>
                    <th scope="col">Runs passed</th>
                    <th scope="col">Time</th>
                  </tr>
                </thead>
                <tbody>
                  {THREE.map((h) => {
                    const t = b.total(h);
                    return (
                      <tr key={h} className={h === "phonedriver" ? "us" : undefined}>
                        <th scope="row">
                          {h} <span>{VERSIONS[h]}</span>
                        </th>
                        <td className="tk-bar">
                          <i style={{ "--f": t.tokens / most } as React.CSSProperties} />
                          <span>{tokens(t.tokens)}</span>
                        </td>
                        <td data-u="calls">{t.calls}</td>
                        <td className={t.passed < t.runs ? "miss" : undefined} data-u="passed">
                          {t.passed} of {t.runs}
                        </td>
                        <td>{minutes(t.wall)}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
            <p>
              adb ran only the two jobs where any route is allowed: {adb.passed} of {adb.runs} runs with {tokens(adb.tokens)} tokens. phonedriver&apos;s sum for the
              same two jobs is {tokens(mixed)}.
            </p>

            <h3>Each job</h3>
            <p>Bars are median tokens, drawn to scale within each job. After the bar: median tokens, runs passed, median tool calls and median wall time.</p>
            <div className="mgrid two wide">
              {jobs.map(({ task, info, cells }) => {
                const max = Math.max(...cells.map(([, c]) => c.tokens));
                return (
                  <div className="mblock" key={task}>
                    <h3>{info[0]}</h3>
                    <p>{info[1]}.</p>
                    {cells.map(([h, c]) => (
                      <div className={h === "phonedriver" ? "mrow us" : "mrow"} key={h}>
                        <span>{h}</span>
                        <span className="mbar" aria-hidden="true">
                          <i style={{ "--f": c.tokens / max } as React.CSSProperties} />
                        </span>
                        <span className="mval">
                          {short(c.tokens)}
                          <small className={c.passed < c.runs ? "miss" : undefined}>
                            {c.passed}/{c.runs}
                          </small>
                          <small>{c.calls} calls</small>
                          <small>{c.wall} s</small>
                        </span>
                      </div>
                    ))}
                    <p>{info[2]}.</p>
                  </div>
                );
              })}
            </div>

            <h3>Where phonedriver does not win</h3>
            <ul>
              <li>
                {slower.length
                  ? `phonedriver had the lowest median wall time on ${fastest} of the nine jobs. Not on these: ${list(slower)}.`
                  : "phonedriver had the lowest median wall time on every job."}{" "}
                Wall time includes the model&apos;s own thinking, so it moves with the day.
              </li>
              <li>
                These are two or three runs per cell on one emulator. They describe what happened and are not rates. Against its own 7 November runs phonedriver used{" "}
                {tokens(us.tokens)} tokens where it had used {tokens(b.before.phonedriver.sum_median_tokens)}, but the emulator was not the same one, so part of that
                is the device.
              </li>
            </ul>
            <p>
              What each tool failed at, the versions, and every run that was thrown away and why are in <a href={raw("docs/bench/jobs.md")}>docs/bench/jobs.md</a>. The
              method is in <Link href="/blog/benchmarking-ai-agents-on-jobs-only-a-phone-can-do/">Benchmarking AI agents on jobs only a phone can do</Link>, and the
              comparison write-up in{" "}
              <Link href="/blog/phonedriver-vs-mobile-mcp-vs-phone-harness-vs-adb-real-phone-jobs/">
                phonedriver vs mobile-mcp vs phone-harness vs adb on real phone jobs
              </Link>
              . If you maintain one of these tools and think I set it up wrong, <a href={`${REPO}/issues`}>open an issue</a> and I will rerun it.
            </p>

            <h2 id="speed">Speed against adb and phone-harness</h2>
            <p>
              p50 of 50 runs after 20 warm-ups, on an Android 17 emulator (API 37, headless, software rendering). Everything ran on the emulator&apos;s own host with the
              three tools interleaved. phonedriver 0.1.1, 11 November 2026. Host CPU load was 31% to 80% around the rows, from other work on the machine. Bars are drawn to scale within each operation; the
              tick is p95.
            </p>
            <div className="mgrid two wide">
              {ops.map((o) => (
                <Speed o={o} key={o.id} />
              ))}
            </div>
            <h3>Every operation, fastest gain first</h3>
            <p>How many times faster phonedriver is, then each tool&apos;s median time with its slowest 1 in 20 (p95) under it.</p>
            <div className="tw">
              <table className="figs">
                <thead>
                  <tr>
                    <th scope="col">Operation</th>
                    <th scope="col">Faster than adb</th>
                    <th scope="col">phonedriver</th>
                    <th scope="col">adb</th>
                    <th scope="col">phone-harness</th>
                    <th scope="col">Faster than phone-harness</th>
                  </tr>
                </thead>
                <tbody>
                  {[...ops]
                    .sort((x, y) => y.adb / y.phonedriver - x.adb / x.phonedriver)
                    .map((o) => (
                      <tr key={o.id}>
                        <th scope="row">{o.label}</th>
                        <td className="x">
                          <b>{ratio(o.adb / o.phonedriver)}</b>
                          <small>
                            {fmt(o.adb_ratio_ci[0])} to {fmt(o.adb_ratio_ci[1])}
                          </small>
                        </td>
                        <td className="us">
                          <b>{fmt(o.phonedriver)} ms</b>
                          <small>p95 {fmt(o.phonedriver_p95)}</small>
                        </td>
                        <td>
                          <b>{fmt(o.adb)} ms</b>
                          <small>p95 {fmt(o.adb_p95)}</small>
                        </td>
                        <td>
                          {o.phone_harness == null ? (
                            <small>not supported</small>
                          ) : (
                            <>
                              <b>{fmt(o.phone_harness)} ms</b>
                              <small>p95 {fmt(o.phone_harness_p95)}</small>
                            </>
                          )}
                        </td>
                        <td className="x">
                          {o.phone_harness == null || !o.phone_harness_ratio_ci ? (
                            <small>n/a</small>
                          ) : (
                            <>
                              <b>{ratio(o.phone_harness / o.phonedriver)}</b>
                              <small>
                                {fmt(o.phone_harness_ratio_ci[0])} to {fmt(o.phone_harness_ratio_ci[1])}
                              </small>
                            </>
                          )}
                        </td>
                      </tr>
                    ))}
                </tbody>
              </table>
            </div>
            <p className="small">The range under each ratio is its 95% confidence interval.</p>
            <h3 id="phone">On a real phone</h3>
            <p>
              A smaller pass on a {PHONE.name} ({PHONE.os}) over USB, on 9 November 2026. 30 samples per phonedriver row and 20 per adb row; the two tools never ran at
              once.
            </p>
            <div className="tw">
              <table>
                <thead>
                  <tr>
                    <th scope="col">Operation</th>
                    <th scope="col">phonedriver p50</th>
                    <th scope="col">adb p50</th>
                    <th scope="col">Ratio</th>
                  </tr>
                </thead>
                <tbody>
                  <tr>
                    <th scope="row">Read the screen</th>
                    <td>{PHONE.read[0]} ms</td>
                    <td>{fmt(PHONE.read[1])} ms</td>
                    <td>{ratio(PHONE.read[1] / PHONE.read[0])}</td>
                  </tr>
                  <tr>
                    <th scope="row">Screenshot</th>
                    <td>{PHONE.shot[0]} ms</td>
                    <td>{fmt(PHONE.shot[1])} ms</td>
                    <td>{ratio(PHONE.shot[1] / PHONE.shot[0])}</td>
                  </tr>
                </tbody>
              </table>
            </div>
            <p>
              The screen read was 461 bytes against 25,334 bytes of XML. On the phone&apos;s main Settings page, <code>uiautomator dump</code> failed with
              &quot;could not get idle state&quot; after 11.5 s, because the page never goes idle, while phonedriver read it in 110 ms. Typing and taps were not
              measured on the phone.
            </p>
            <h3>How to read it</h3>
            <ul>
              <li>
                The bare tap is the closest row. Host to reply, phonedriver takes {fmt(op("tap").phonedriver)} ms when taps come back to back (
                {(op("tap").adb / op("tap").phonedriver).toFixed(1)} times faster than adb) and {fmt(op("tap_paced").phonedriver)} ms when they are 1.5 s apart (
                {(op("tap_paced").adb / op("tap_paced").phonedriver).toFixed(1)} times). The tap misses its own 15 ms budget (decision D14), and typing 100 characters
                misses its 150 ms budget ({fmt(op("type100").phonedriver)} ms).
              </li>
              <li>
                adb returns raw XML and a tap that returns before the app reacts. phonedriver returns a pruned, numbered tree and waits for the app. Different outputs for
                the same job. For &quot;tap, then what changed&quot;, the adb row is a tap, one <code>uiautomator dump</code> and a diff in the caller.
              </li>
              <li>
                <code>adb input text</code> and phone-harness cannot type Unicode. phonedriver typed 100 mixed-script characters (ASCII, accents, CJK, Arabic, emoji) in
                225.0 ms.
              </li>
              <li>
                Fresh daemon to first screen read: phonedriver 1,620.5 ms, phone-harness 2,999.4 ms (new Python process), adb 2,488.0 ms (one dump; adb has no cold
                start, so this is for scale only).
              </li>
              <li>phone-harness&apos;s helpers ran in process, which spares it a Python start per call and favours it.</li>
              <li>
                No iOS device is in this run. On the iOS 26.5 Simulator a tap takes 357 ms against 80 ms for the Simulator&apos;s own tool, because XCTest takes about
                240 ms to confirm the event (D15). On an iPhone 14 Plus and an iPad Pro (M5) it takes 534 and 454 ms.
              </li>
            </ul>
            <p>
              The earlier 153x figure came from a quieter, single-tool run and is not current.{" "}
              <a href={raw("docs/bench/speed.md")}>The full method, machine state and the history of the number</a> are in the repo.
            </p>

            <h2 id="tokens">An earlier token run: three small tasks</h2>
            <p>
              Before the nine jobs, a smaller run: Claude Sonnet 5.5 through <code>claude -p</code>, each tool the only tool, three tasks checked
              from device state (the alarm database, the testbed event log, the answer text). Tokens are input, output, cache creation and cache read. Device: a
              2560x1600 tablet emulator.
            </p>
            <div className="tw">
              <table>
                <thead>
                  <tr>
                    <th scope="col">Task (checked from device state)</th>
                    <th scope="col">phonedriver, 2 to 3 runs, median tokens</th>
                    <th scope="col">phone-harness, 3 runs, median tokens</th>
                  </tr>
                </thead>
                <tbody>
                  {TOKENS.map(([t, a, c]) => (
                    <tr key={t}>
                      <th scope="row">{t}</th>
                      <td>{a}</td>
                      <td>{c}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <p>
              On the sum of medians phonedriver used 34% fewer tokens than phone-harness (218,357 against 330,375). It is ahead on the longest task, level on the form and
              8% behind on the Wikipedia task, where individual runs ranged from 26,147 to 64,522 tokens. Raw adb finished <code>alarm-gym</code> in 2 calls with a{" "}
              <code>SET_ALARM</code> intent (18,607 tokens), a shortcut the task allows, and failed the web form, a WebView with no accessibility nodes. mobile-mcp
              failed every run of this earlier setup for an environment reason (its helper could not reach a remote adb server), so it is not compared here; the nine
              jobs above gave it its own adb server.
            </p>
            <p>
              The fixed cost per turn is also measured: the first turn of a one-word <code>claude -p</code> costs 5,606 tokens with phonedriver&apos;s 14 MCP tools and
              5,214 with phone-harness. <a href={raw("docs/bench/tokens.md")}>All three rounds, the fixes between them and the caveats</a>.
            </p>

            <h2 id="data">Raw data and reproduction</h2>
            <ul>
              <li>
                <a href="/benchmarks/data/jobs.json">jobs.json</a>: every agent run of the nine jobs, with its pass, tokens, tool calls and wall time.
              </li>
              <li>
                <a href="/benchmarks/data/speed-samples.csv">speed-samples.csv</a> and <a href="/benchmarks/data/speed-samples-second-pass.csv">second-pass samples</a>:
                every timed sample.
              </li>
              <li>
                <a href="/benchmarks/data/speed.json">speed.json</a>: p50, p95 and bootstrap intervals per operation.{" "}
                <a href="/benchmarks/data/speed-meta.json">speed-meta.json</a>: machine and versions.
              </li>
              <li>
                <a href="/benchmarks/data/tokens.json">tokens.json</a>: every run of the earlier three-task benchmark.
              </li>
              <li>
                Repeat the speed run: <code>PH_PYTHON=&lt;python with phone-harness 0.3.3&gt; bash bench/speed/reproduce.sh</code> in the <a href={REPO}>repository</a>.
              </li>
            </ul>
          </div>
        </article>
      </div>
    </>
  );
}
