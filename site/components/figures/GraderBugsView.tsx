"use client";

import { useState } from "react";
import { Fig, Seg } from "./kit";

type Kind = "pass" | "fail" | "crash";
type Verdict = { kind: Kind; lines: string[] };
type State = { id: string; label: string; should: "pass" | "fail"; shows: string[]; input: string };
type Mark = "-" | "+" | " ";
type Bug = {
  id: string;
  tab: string;
  title: string;
  mis: string;
  job: string;
  fn: string;
  diff: [Mark, string][];
  states: State[];
  old: (s: State) => Verdict;
  now: (s: State) => Verdict;
};

const CHROME_PREFS = "/data/data/com.android.chrome/app_chrome/Default/Preferences";
const verdict = (lines: string[]): Verdict => ({ kind: lines.length ? "fail" : "pass", lines });

const STALE = [/total/i, /decreas|minus|reduc|lower|fewer|down|remov/i, /not (update|chang|recalc|refresh)|stale|unchang|still|wrong|incorrect|stuck|doesn.t|same/i];
const found = (chunks: string[]) => chunks.some((c) => STALE.every((p) => p.test(c)));
const splitOld = (r: string) => r.split(/[\n.!?;]+/).filter((c) => c.trim());
const splitNow = (r: string) => r.split(/(?<=[.!?;])\s+|\n/).filter((c) => c.trim());
const bugReport = (chunks: string[]) => (found(chunks) ? [] : ["planted bug not reported: total-stale-after-decrease"]);

const minutes = (xml: string, key: string) => {
  const m = xml.match(new RegExp(`name="[^"]*(?:${key})[^"]*"[^>]*?(?:value="|>)(\\d+)`, "i"));
  if (!m) return null;
  const v = parseInt(m[1], 10);
  return v <= 60 ? v : v <= 3600 ? Math.floor(v / 60) : Math.floor(v / 60000);
};
const clockTimes = (xml: string) => {
  const bad: string[] = [];
  const snooze = minutes(xml, "snooze");
  const silence = minutes(xml, "silence");
  if (snooze !== 15) bad.push(`snooze length is ${snooze} minutes, wanted 15`);
  if (silence !== 5) bad.push(`silence after is ${silence} minutes, wanted 5`);
  return bad;
};
const places = (xml: string) => [...xml.matchAll(/name="place_timezone_id_\d+">([^<]+)</g)].map((m) => m[1]);

const CLOCK_HEAD = '<string name="home_time_zone">Europe/London</string>';
const clockXml = (extra: string, snooze: number, silence: number) =>
  `<string name="snooze_duration">${snooze}</string><string name="auto_silence">${silence}</string>${CLOCK_HEAD}${extra}`;
const TOKYO = '<string name="place_timezone_id_0">Asia/Tokyo</string>';
const LONDON = '<string name="place_timezone_id_1">Europe/London</string>';
const xmlLines = (xml: string) => xml.replace(/<string name="([^"]+)">([^<]*)<\/string>/g, "$1 = $2\n").trim().split("\n");

const duck = '"default_search_provider_data": {"template_url_data": {"short_name": "DuckDuckGo"}}';
const httpsLine = (key: string | null) => (key ? `, "${key}": true` : "");
const prefsJson = (key: string | null) => `{${duck}${httpsLine(key)}}`;
const searchBad = (j: string) => (/duckduckgo/i.test(j) ? [] : ["default search engine is not DuckDuckGo"]);

const ids: Record<string, string> = { Alice: "Alice Chen", Bob: "Bob Park", Carol: "Carol Diaz" };
type Conv = { name: string; archived: string; pinned: string };
const grid = (rows: Conv[], pinsSeen: boolean) => {
  const want: Record<string, [boolean, boolean]> = { "Alice Chen": [false, false], "Bob Park": [true, false], "Carol Diaz": [false, true] };
  const bad: string[] = [];
  for (const [name, [p, a]] of Object.entries(want)) {
    const mine = rows.find((r) => r.name === name)!;
    const gp = pinsSeen && mine.pinned !== "0";
    const ga = mine.archived !== "0";
    if (gp !== p || ga !== a) bad.push(`${name}: pinned=${cap(gp)} archived=${cap(ga)}, wanted pinned=${cap(p)} archived=${cap(a)}`);
  }
  return bad;
};
const cap = (b: boolean) => (b ? "True" : "False");
const msgRows = (bob: string, carol: string): Conv[] => [
  { name: ids.Alice, archived: "0", pinned: "0" },
  { name: ids.Bob, archived: "0", pinned: bob },
  { name: ids.Carol, archived: carol, pinned: "0" },
];
const msgShows = (r: Conv[]) => [
  "select * from conversations",
  ...r.map((x) => `  ${x.name.padEnd(11)} archive_status=${x.archived}`),
  "select * from conversation_pin",
  ...(r.some((x) => x.pinned !== "0") ? r.filter((x) => x.pinned !== "0").map((x) => `  conversation_id=${x.name}'s _id  pin_status=${x.pinned}`) : ["  (no rows)"]),
];
const msgDone = msgRows("1", "1");
const msgHalf = msgRows("0", "1");

const BUGS: Bug[] = [
  {
    id: "decimal",
    tab: "$36.00",
    title: "A decimal point ended the sentence",
    mis: "Flunks a correct bug report, so a tool that found the bug loses the run.",
    job: "qa-shop",
    fn: "found_bugs",
    diff: [
      [" ", "def found_bugs(report, bugs):"],
      ["-", '    chunks = [c for c in re.split(r"[\\n.!?;]+", report) if c.strip()]'],
      ["+", '    chunks = [c for c in re.split(r"(?<=[.!?;])\\s+|\\n", report) if c.strip()]'],
    ],
    states: [
      { id: "a", label: "Names the $36.00", should: "pass", input: "Quantity 3 to 2: the total stays $36.00 and is not updated after Decrease.", shows: [] },
      { id: "b", label: "No price", should: "pass", input: "The total does not change after Decrease.", shows: [] },
      { id: "c", label: "Misses the bug", should: "fail", input: "SAVE10 looks fine and the total is right.", shows: [] },
    ],
    old: (s) => verdict(bugReport(splitOld(s.input))),
    now: (s) => verdict(bugReport(splitNow(s.input))),
  },
  {
    id: "zone",
    tab: "Clock zone",
    title: "The host's own timezone passed for London",
    mis: "A fresh Clock already says Europe/London, so a run that never added London can pass.",
    job: "clock-settings",
    fn: "check_clock_settings",
    diff: [
      [" ", '    files = su(f"cat {CLOCK_DIR}/shared_prefs/*.xml")'],
      ["-", "    bad = []"],
      ["-", `    tokyo = su(f"grep -rIlE 'Tokyo' {CLOCK_DIR}").strip()`],
      ["-", `    london = su(f"grep -rIlE 'London' {CLOCK_DIR}").strip()`],
      ["-", "    if not tokyo:"],
      ["-", `        bad.append("no Tokyo world clock found in the Clock app's files")`],
      ["-", "    if not london:"],
      ["-", `        bad.append("no London world clock found in the Clock app's files")`],
      ["+", `    places = set(re.findall(r'name="place_timezone_id_\\d+">([^<]+)<', files))`],
      ["+", "    bad = ["],
      ["+", '        f"no {city} world clock; clocks are {sorted(places)}"'],
      ["+", '        for city, zone in (("Tokyo", "Asia/Tokyo"), ("London", "Europe/London"))'],
      ["+", "        if zone not in places"],
      ["+", "    ]"],
    ],
    states: [
      { id: "a", label: "Fresh Clock", should: "fail", input: clockXml("", 10, 10), shows: [] },
      { id: "b", label: "Tokyo, 15, 5", should: "fail", input: clockXml(TOKYO, 15, 5), shows: [] },
      { id: "c", label: "Tokyo and London", should: "pass", input: clockXml(TOKYO + LONDON, 15, 5), shows: [] },
    ],
    old: (s) => verdict([...(s.input.includes("Tokyo") ? [] : ["no Tokyo world clock found in the Clock app's files"]), ...(s.input.includes("London") ? [] : ["no London world clock found in the Clock app's files"]), ...clockTimes(s.input)]),
    now: (s) => {
      const have = places(s.input);
      return verdict([...(["Asia/Tokyo", "Europe/London"] as const).filter((z) => !have.includes(z)).map((z) => `no ${z === "Asia/Tokyo" ? "Tokyo" : "London"} world clock; clocks are [${have.map((h) => `'${h}'`).join(", ")}]`), ...clockTimes(s.input)]);
    },
  },
  {
    id: "pins",
    tab: "Pins",
    title: "Pins live in another table",
    mis: "Flunks every run that pinned Bob Park: the old check looked for a pin column that does not exist.",
    job: "messages-pin-archive",
    fn: "conversations",
    diff: [
      ["-", `    out = su(f'sqlite3 -header -separator "|" {BUGLE_DB} "select * from conversations"').splitlines()`],
      ["-", "def flag(row, word):"],
      ["-", "    return any("],
      ["-", '        word in k.lower() and v not in ("", "0", "NULL", "null") for k, v in row.items()'],
      ["-", "    )"],
      ["-", '        pinned, archived = flag(mine[0], "pin"), flag(mine[0], "archiv")'],
      ["+", "    out = su("],
      ["+", `        f'sqlite3 -separator "|" {BUGLE_DB} "select c._id, c.name, c.archive_status, coalesce(p.pin_status, 0) '`],
      ["+", `        'from conversations c left join conversation_pin p on p.conversation_id = c._id"'`],
      ["+", "    )"],
    ],
    states: [
      { id: "a", label: "Bob pinned, Carol archived", should: "pass", input: "done", shows: msgShows(msgDone) },
      { id: "b", label: "Carol archived only", should: "fail", input: "half", shows: msgShows(msgHalf) },
    ],
    old: (s) => verdict(grid(s.input === "done" ? msgDone : msgHalf, false)),
    now: (s) => verdict(grid(s.input === "done" ? msgDone : msgHalf, true)),
  },
  {
    id: "missing",
    tab: "No file",
    title: "A missing file crashed the grader",
    mis: "A traceback exits 1, the same as a failed check, so a run that skipped Chrome looks graded and no reason is printed.",
    job: "chrome-prefs",
    fn: "check_chrome_prefs",
    diff: [
      ["-", '    prefs = json.loads(su(f"cat {CHROME_PREFS}"))'],
      ["+", '    raw = su(f"cat {CHROME_PREFS}")'],
      ["+", '    if not raw.startswith("{"):'],
      ["+", `        return [f"Chrome has no Preferences file: {raw.strip()[:80]!r}"]`],
      ["+", "    prefs = json.loads(raw)"],
    ],
    states: [{ id: "a", label: "Chrome never wrote Preferences", should: "fail", input: `cat: ${CHROME_PREFS}: No such file or directory`, shows: [`su 0 cat ${CHROME_PREFS}`, "  (cat: …/Preferences: No such file or directory)"] }],
    old: () => ({ kind: "crash", lines: ["Traceback (most recent call last):", "  …", "json.decoder.JSONDecodeError: Expecting value: line 1 column 1 (char 0)", "exit 1, no reason"] }),
    now: (s) => verdict([`Chrome has no Preferences file: '${s.input.slice(0, 80)}'`]),
  },
  {
    id: "https",
    tab: "HTTPS key",
    title: "Chrome renamed the switch",
    mis: "Flunks a run that turned Always use secure connections on, because Chrome writes a different key now.",
    job: "chrome-prefs",
    fn: "check_chrome_prefs",
    diff: [
      ["-", '    if prefs.get("https_only_mode_enabled") is not True:'],
      ["-", `        bad.append(f"https_only_mode_enabled is {prefs.get('https_only_mode_enabled')!r}")`],
      ["+", '    if not (prefs.get("https_first_balanced_mode_enabled") or prefs.get("https_only_mode_enabled")):'],
      ["+", '        bad.append("Always use secure connections is off (https_first_balanced_mode_enabled and https_only_mode_enabled are not true)")'],
    ],
    states: [
      { id: "a", label: "Secure connections on", should: "pass", input: prefsJson("https_first_balanced_mode_enabled"), shows: [] },
      { id: "b", label: "Left off", should: "fail", input: prefsJson(null), shows: [] },
    ],
    old: (s) => verdict([...searchBad(s.input), ...(s.input.includes('"https_only_mode_enabled": true') ? [] : ["https_only_mode_enabled is None"])]),
    now: (s) =>
      verdict([...searchBad(s.input), ...(/"https_(first_balanced|only)_mode_enabled": true/.test(s.input) ? [] : ["Always use secure connections is off (https_first_balanced_mode_enabled and https_only_mode_enabled are not true)"])]),
  },
];

const WORD: Record<Kind, string> = { pass: "pass", fail: "fail", crash: "crash" };

function evidence(bug: Bug, s: State): { head: string; lines: string[] } {
  if (bug.id === "decimal")
    return { head: "The report, as each splitter cuts it", lines: [] };
  if (bug.id === "zone") return { head: "Clock's shared_prefs after the run", lines: xmlLines(s.input) };
  if (bug.id === "https") return { head: "Chrome's Preferences, trimmed to the two settings", lines: s.input.slice(1, -1).split(/, (?=")/).map((l) => `  ${l}`) };
  return { head: bug.id === "pins" ? "Messages' bugle_db after the run" : "What the check read", lines: s.shows };
}

export function GraderBugsView() {
  const [id, setId] = useState(BUGS[0].id);
  const [sid, setSid] = useState<Record<string, string>>({});
  const bug = BUGS.find((b) => b.id === id)!;
  const s = bug.states.find((x) => x.id === sid[bug.id]) ?? bug.states[0];
  const was = bug.old(s);
  const is = bug.now(s);
  const ev = evidence(bug, s);
  const wrong = was.kind === "crash" || was.kind !== s.should;
  const key = `${bug.id}-${s.id}`;

  const cut = (chunks: string[]) =>
    chunks.map((c, i) => (
      <span key={i} className="gb-chunk">
        {c}
      </span>
    ));

  const box = (name: string, v: Verdict, old: boolean) => (
    <div className="gb-v" data-kind={v.kind} data-wrong={old && wrong ? "" : undefined} key={`${key}${name}`}>
      <p className="gb-vh">
        <span>{name}</span>
        <b>{WORD[v.kind]}</b>
      </p>
      <div className="gb-vl">
        {v.lines.length ? v.lines.map((l, i) => <p key={i}>{l}</p>) : <p className="dim">no failure lines</p>}
      </div>
      {old && <p className="gb-m">{wrong ? "Wrong." : "Same verdict as the fixed check on this state."}</p>}
      {!old && <p className="gb-m ok">{is.kind === s.should ? `Right: this state should ${s.should}.` : ""}</p>}
    </div>
  );

  return (
    <Fig
      id="grader-bugs"
      title="Five grader bugs, caught before any model ran"
      controls={<Seg label="Grader bug" value={id} onChange={setId} options={BUGS.map((b, i) => [b.id, `${i + 1} ${b.tab}`] as const)} />}
      caption={
        <>
          Each diff is the fix to <code>bench/agent-eval/jobs/jobs.py</code> from 7 November 2026, found by driving every check by hand through a pass state and its failure states
          (<code>docs/bench/jobs.md</code>, &quot;checks proven by hand&quot;). The verdicts are the old and the fixed checks run on the states you pick. The states are fixtures built from
          the test cases in <code>bench/agent-eval/jobs/test_jobs.py</code> and the keys the check reads.
        </>
      }
    >
      <div className="gb" key={bug.id}>
        <div className="gb-top">
          <h3>{bug.title}</h3>
          <p>
            <b>{bug.job}</b> <span>in</span> <code>{bug.fn}</code>
          </p>
        </div>
        <div className="gb-diff" role="img" aria-label={`Diff of ${bug.fn}`}>
          {bug.diff.map(([m, t], i) => (
            <p key={i} data-m={m} style={{ animationDelay: `${i * 35}ms` }}>
              <i>{m === " " ? "" : m}</i>
              <span>{t}</span>
            </p>
          ))}
        </div>
        <div className="gb-grade">
          <div className="gb-in">
            <p className="gb-lbl">Grade this state</p>
            {bug.states.length > 1 ? (
              <Seg label="State" value={s.id} onChange={(v) => setSid({ ...sid, [bug.id]: v })} options={bug.states.map((x) => [x.id, x.label] as const)} />
            ) : (
              <p className="gb-one">{s.label}</p>
            )}
            <div className="gb-ev" key={key}>
              <p className="gb-lbl">{ev.head}</p>
              {bug.id === "decimal" ? (
                <>
                  <p className="gb-cut">
                    <small>old</small>
                    {cut(splitOld(s.input))}
                  </p>
                  <p className="gb-cut">
                    <small>fixed</small>
                    {cut(splitNow(s.input))}
                  </p>
                </>
              ) : (
                <pre>{ev.lines.join("\n")}</pre>
              )}
            </div>
          </div>
          <div className="gb-vs">
            {box("Old check", was, true)}
            {box("Fixed check", is, false)}
          </div>
        </div>
        <p className="fx-verdict bad" data-wait={wrong ? undefined : ""} key={`${key}v`}>
          <b>{was.kind === "crash" ? "Crash, not a grade." : s.should === "pass" ? "Flunked correct work." : "Free pass."}</b>
          <span>{bug.mis}</span>
        </p>
      </div>
    </Fig>
  );
}
