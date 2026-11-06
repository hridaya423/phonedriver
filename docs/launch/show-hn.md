# Show HN plan (round 2)

Nothing here has been posted. This file is a fact sheet and a structure. It is not text to paste. HN's guidelines say "Don't post generated text or AI-edited text", and dang's Show HN tips say not to use an LLM for any of it, "not even a tiny bit". Moderators ban accounts for this (`research.md`, round 2). Write the title, the first comment and every reply yourself, from the facts below.

Slots: `[TBD: ...]` waits on a measurement or a build. `[USER: ...]` needs your own words or a decision. `claims-ledger.md` names the repo file behind each fact. Repo facts below are from `main` at `c2297a1` (shell tool, token benchmark) unless noted.

## What the ranking rules mean for this post

| Mechanic (source in `research.md`) | What to do |
|---|---|
| Rank is roughly `(points-1)^0.8 / (age_hours+2)^1.8` in the old code. The current constants are secret. Age decays fast. | The first two hours decide the result. Be at the keyboard for them. |
| A thread with more than 20 comments and more comments than points is penalized (old code, 2013 crawl, dang's "flamewar detector"). | Answer in short, factual replies. Do not argue. Do not post long back-and-forth chains. |
| Voting-ring detector and "Our software ignores most promo-votes". | Do not ask anyone to vote. Do not post the HN link anywhere. Tell no one to "check it out on HN". |
| Show HN is restricted for new accounts (`news.ycombinator.com/showlim`). | Open your HN account profile today. If the account is new or has no comment history, spend a week commenting honestly before launch. `[USER: account age and karma]` |
| Second-chance pool and repost invites go to authors with an email in their profile. | Put an email in your profile before launch. |
| Moderators rewrite baity titles and strip "Show HN" from posts that cannot be tried. | Keep the title plain. Make the install work first. |
| The fastest hit rate in three datasets and in a 90-day own count is Monday 00:00 UTC (Sunday 7 pm ET, 4 pm PT). Midday weekdays have the most readers and the most competition. | Submit Monday 00:00 UTC. Fallback: Tuesday 15:00 UTC. Convert to your time zone and check that you can stay for two hours. |

## Decisions for the author

1. **AI-assisted code.** `AGENTS.md` and `progress.md` are public and show agents did part of the work. HN says "Don't post quickly-generated one-offs." Say it in one plain sentence in the first comment. `[USER: how much you wrote, and how you reviewed the rest]`
2. **The name.** `phone-use` is taken on npm and as the GitHub organization `phone-use`. `sauravtom/phone-use` has the same name and 6 stars. `plan.md` lists three free alternatives. The drafts keep `phone-use` until you decide.
3. **Account.** Use your personal account, not a project account, with an email in the profile.
4. **Decision D6 is out of date.** `docs/decisions.md` D6 says "never execute agent-written code". D21 shipped a `shell` tool that runs agent-written commands. Fix D6 before launch, because HN readers will quote it against you.

## Title

HN titles are limited to 80 characters. Use "Show HN: phone-use – " then what it does. Moderators call a title "baity" when it uses superlatives or metaphors.

| # | Title | Length |
|---|---|---|
| 1 | Show HN: phone-use – Agents read a phone as text and get only what changed | 74 |
| 2 | Show HN: phone-use – Android and iOS control for agents, with text diffs | 72 |
| 3 | Show HN: phone-use – A Rust CLI and MCP server for agents to drive phones | 73 |

**Recommended: option 1.** It states what the tool does and the one idea that separates it. Evidence from 11,542 Show HNs (correlation only): titles with "AI", "agent" or "Claude" reached 20+ points less often (3.5% against 5.1%), and titles with "Rust" or "open source" more often (6.7% and 7.0%). The effects are weak and confounded, so option 1 stays and option 3 is the fallback if you want "Rust" in the title. Do not add a number to the title. The adb speed figures come from an emulator on a busy machine, and a title claim starts the fairness argument before readers see the caveats.

## URL

`https://github.com/hridaya423/phone-use`

Submit the URL and your own text together. HN shows the text as your first comment. If it does not appear, post it yourself within a minute.

## First comment: structure

Write 350 to 450 words, five blocks, in this order. dang's tip: "a clear statement of what your project is or does" first, then "the backstory of how you came to work on this, and explaining what's different about it". Do not praise the project.

1. **What it is** (one or two sentences). Facts: a text view of an Android or iOS screen, and the means to tap, type, swipe. One Rust binary: CLI, daemon, MCP server. Every action returns only the lines that changed.
2. **Why you built it** (your words). `[USER: what you tried first and what failed.]` Usable facts: you read the phone-harness source before starting; `adb shell input` starts a process per call; `uiautomator dump` took 2.8 s on the test emulator.
3. **How it works** (3 to 5 technical points; pick the ones you can defend).
4. **Numbers and limits** (what you measured, what you did not).
5. **What feedback you want** (two specific asks).

### Facts for block 3

- Android: a dex jar pushed and started with `app_process` as the shell user, as scrcpy does. Nothing installed on the phone. No accessibility service to enable. Tree through UiAutomation, input through InputManager. (D2)
- Settle: every action waits for accessibility events and video frames to go quiet, then returns the diff, such as `~ @e41 switch "Dark Appearance" off → on`. No polling. (D12, D10)
- Refs like `@e10` are found again at tap time, so a list that moved one row still gets the right tap. (README)
- iOS: own XCTest runner, not WebDriverAgent (D4). The iOS 27 CoreDevice and hybrid tiers are experimental and off by default. [TBD: hybrid tier status; `progress.md` on this branch calls it unfinished and unproven on device]
- `shell` verb and MCP tool: on Android it runs in the agent already on the device, over the open control stream. After a command that can change state it settles and returns the screen diff. On a Simulator it runs through `simctl spawn`. A physical iPhone has no shell. On a physical Android device it refuses wipes and removing apps phone-use did not install unless `--force`. The guard is "a guard against slips, not a sandbox". (D21, `docs/bench/shell.md`)

### Facts for block 4

Speed, `docs/bench/adb.md`: Android 17 emulator, busy machine, median of 50 after 5 warm-ups, phone-use and adb measured back to back.

| Operation | phone-use | adb |
|---|---|---|
| screenshot | 45.3 ms | 692.4 ms |
| Settings screen read | 237.0 ms | 2,800.5 ms |
| tap, then know what changed | 666.9 ms | 3,111.7 ms |
| bare tap | 144.1 ms | 223.7 ms |

- Tap after the latest change (`progress.md` on `main`, "Speedup 4"): 33.7 to 35.9 ms p50 on the API 37 emulator, three interleaved runs, 100 runs each. The budget is 15 ms and is missed. [TBD: a paired quiet-machine re-run against adb for the post]
- iOS tap: 357 ms on the Simulator, 534 ms on an iPhone 14 Plus, 454 ms on an iPad Pro, because XCTest takes about 240 ms to confirm an event. (D15, `docs/bench/m5.md`)
- Agent evaluation (`docs/bench/m3.md`): 15 of 15 held-out runs on five Android tasks with Claude Sonnet 5.5; median 6 tool calls, 18 s. The first pass scored 9 of 15. All six failures were bugs in the checks or the device reset.
- Token benchmark (`docs/bench/tokens.md`, one run per cell, tablet emulator, shell tool not yet in the run): phone-use passed 3 of 3 tasks with 413,534 tokens in 45 calls. phone-harness passed 3 of 3 with 337,059. Raw adb passed 2 of 3 with 240,389. mobile-mcp failed 0 of 3 for an environment reason (its device server did not start), so its tokens are not comparable. **phone-use did not use fewer tokens.** Do not claim it did. [TBD: rerun with the shell tool; rerun mobile-mcp on a local emulator]
- Limits to state: no physical Android phone measured; iOS needs a Mac with Xcode, and a physical iPhone needs a signing team (a free Apple ID expires every 7 days); WebViews and custom views can have an empty tree, then OCR fills in without a checked state; one iOS scenario (`settings-dark-appearance`) failed twice in six full runs and the cause is not found.
- License: AGPL-3.0-only. No telemetry (D9).

### Block 5, feedback asks (pick two)

Results on a real Android phone. Apps where the snapshot comes back empty or wrong. Whether the diff format reads well to your agent. Whether `shell` should be on by default.

## Prepared answers: facts, not prose

Reply in your own words, in one to four sentences. Answer within 15 minutes in the first two hours. Concede what is true.

| # | Objection | Facts to use | Do not say |
|---|---|---|---|
| 1 | AGPL | Applies when you distribute it or run a modified copy as a network service (README, License). Chosen so a competitor cannot take the code into a closed project (D13). Dependencies are permissive. scrcpy files under Apache-2.0 are in `NOTICE`. | That the license "does not affect" a company's use. That is a legal reading. `[USER: confirm your position]` |
| 2 | Why not adb | `adb shell input` starts a process per call. `uiautomator dump` waits for idle: 2.8 s per screen. adb cannot say what a tap changed; the benchmark builds it from tap, dump and diff at 3.1 s. `input text` fails on Unicode. Method and caveats in `docs/bench/adb.md`. Raw adb used fewer tokens in the token benchmark. | That adb is unusable. |
| 3 | Why not Appium or Maestro | They are test frameworks: a script written once, run many times. phone-use is one action chosen after seeing the screen, with the diff in each reply. Appium needs the UiAutomator2 server installed. The Maestro MCP page says physical iOS devices are not supported. | Quotes you have not re-read. `docs/research/competitors.md` came from a summarizing fetch. |
| 4 | Why not mobile-mcp | It is the most used (10.4k stars), covers real iOS devices and cloud. From its `server.ts`: no wait after actions, one line per element with coordinates and size. phone-use settles and returns the diff. | Anything about its token use. Your run did not work in your environment. |
| 5 | Security: agent with a shell and a phone | There is a `shell` tool (D21). Android runs it in the on-device agent as the shell user. A physical Android device refuses wipes and unknown-app removal without `--force`, and the guard "is not a sandbox". The daemon listens on a Unix socket in a mode-700 directory and nowhere else. `--secret` text never reaches logs, traces or replies. The agent can do what a person can do on screen, including tap Pay. Emulators and Simulators have no guard. | That it is safe. That the guard stops a determined command. Any claim about prompt injection from screen text; the repo has no defence. `[USER: confirm]` |
| 6 | iOS needs a Mac | XCTest, the Simulator and the developer disk image need Xcode on a Mac. A device on another machine works through an `ssh` forward of the daemon socket (README). | |
| 7 | Real phones | iOS: tested on an iPhone 14 Plus and an iPad Pro through CLI and MCP; the 12-scenario suite was not run on devices. Android: same code path as the emulator, no hardware measured. Wireless debugging was tested on an emulator over TCP only (`docs/bench/wifi.md`). | That Android hardware works. |
| 8 | Telemetry | None: no install id, no event, no crash upload (D9). phone-harness ships the script, output and helper calls to PostHog by default (`docs/research/competitor-teardown.md`). Apple is the only outside party, for developer disk image mounting. | "Nothing else leaves the machine" until a network-silence test for phone-use exists. `[USER: add the test or drop the claim]` |
| 9 | Benchmark fairness | Busy emulator host. phone-use tap was slower there than in the quiet run. phone-harness numbers come from an earlier quiet run, in process, which favors phone-harness. adb's tap returns before the app reacts. Raw XML against pruned text. No physical Android. Raw samples in `docs/bench/data/`. | "Rigged" arguments. Offer the method. |
| 10 | Tokens | See the token benchmark above. Snapshots are 183 to 822 tokens on measured screens (characters divided by four). The tool schemas cost about 2.6k tokens. One run per cell. phone-use was not cheapest. | A token advantage. |
| 11 | "Vibe-coded" / same name as another repo | `[USER: your answer.]` Repo facts: plans, decision log, 21 Android and 12 iOS device scenarios, benchmarks with raw data, audits. Another project named phone-use exists (`sauravtom/phone-use`, Android only, MIT). | |

## If the post stalls

Do not delete and repost. Do not ask for votes. Email hn@ycombinator.com and ask for the second-chance pool (`item?id=26998308`). Check that the title and first comment state what the tool does, because dang names "I can't tell what this is" as the failure.
