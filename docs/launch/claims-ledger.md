# Claims ledger

**Round 2 status.** Rows below that name the old demo are superseded: the demo is being re-recorded, and no draft now leads with or mentions how the old clip went. Rows about "no shell tool" are superseded by D21 (see Round 2 additions). Paths marked "on `main`" exist at `main` commit `c2297a1`, not in this worktree branch.

Every factual claim in `show-hn.md`, `x.md` and `linkedin.md`, with the repo file that backs it. Paths are relative to the repo root. "Verify" marks a claim the repo supports only partly or that came through a summarizing fetch. Check each row against the repo at the launch commit before posting. Web claims are in `research.md`, with their URLs.

## Product

| Claim | Where used | Backing file |
|---|---|---|
| phone-use gives an agent a text view of an Android or iOS screen and the means to tap, type and swipe. Every action returns only the lines that changed. | HN, X | `README.md` (opening paragraph) |
| One Rust binary: CLI, daemon and MCP server | HN, LinkedIn | `docs/decisions.md` D1 |
| Android: pushes a dex jar, starts it with `app_process` as the shell user, as scrcpy does. Nothing installed on the phone, no accessibility service to enable. Tree through UiAutomation, input through InputManager. | HN, X | `docs/decisions.md` D2; `README.md` (Supported devices: "Android needs no install on the phone") |
| Every action waits for settle and returns the diff. Settle uses accessibility events and video frames. The agent does not poll or sleep. | HN, X, LinkedIn | `docs/decisions.md` D12, D10; `README.md` (Try it, step 3); `docs/research/competitors.md` section 3 ("event-driven settle", architecture 4.6) |
| The sample reply `~ @e41 switch "Dark Appearance" off → on`, `+ @e68 button "L4S"`, `settled in 607ms` | HN, X thread 2 | `README.md` (Try it, step 3, iOS Simulator) |
| Refs such as `@e10` are found again at tap time, so a list that shifted by a row still gets the right tap | HN | `README.md` (How it works) |
| iOS uses our own XCTest runner, not WebDriverAgent | HN | `docs/decisions.md` D4 |
| Experimental CoreDevice tier on iOS 27, off by default | HN | `README.md` (`PHONE_USE_IOS_TIER`); `docs/decisions.md` D3; `docs/bench/m5.md` (CoreDevice sections: "Default stays Runner") |
| The agent calls typed verbs. No shell tool, no code execution. | HN, answer 5 | `docs/decisions.md` D6; `docs/research/competitors.md` 1b ("shell: none (by design D6)") |
| Thirteen MCP tools | HN answer 10 | `README.md` (Use with other MCP clients) |
| The 13 tool schemas cost about 2.6k tokens | HN answer 10 | `progress.md` (tools/list after gap 4: 9,691 characters plus 345, about 2,573 tokens by a 3.9 characters-per-token ratio, no tokenizer run). Verify: the progress entry for later gaps may have changed the size. |
| A snapshot is 183 to 822 tokens on the measured screens (chars divided by four) | HN answer 10 | `docs/audits/ax-audit.md` (snapshot sizes line) |
| WebView and custom views can have an empty tree, and OCR reads the screen then | HN | `progress.md` (Task 7.2: `tree_is_hollow`, `text "Web form" (ocr)`) |
| A tapped WebView checkbox shows no checked state | HN | `docs/bench/m3.md` (tuning item 4, open items) |
| Text sent with `--secret` never reaches logs, traces or replies | HN answer 5 | `README.md` (Use with other MCP clients, last paragraph); `AGENTS.md` (Secrets) |
| The daemon listens on a Unix socket and nowhere else; the socket sits in a mode-700 directory with mode 600 | HN answer 5 | `README.md` ("Use a device on another machine"; `phone-use doctor` sample output) |
| The agent can do anything a person can do on screen, including tap Pay | HN answer 5 | Not stated in the repo. It follows from the tool surface (`README.md`, MCP tools). The author must stand behind it. |
| Defence against prompt injection from on-screen text | HN answer 5 | None found in the repo. The draft leaves it as a `[USER: ...]` slot. |
| Remote devices through an `ssh` forward of the daemon socket | HN answer 6 | `README.md` (Use a device on another machine) |
| iOS needs a Mac with Xcode; a physical iPhone needs a signing team; a free Apple ID expires every 7 days | HN, X | `README.md` (Supported devices, iPhone or iPad) |
| Android runs on macOS, Linux and Windows | HN answer 6 | `README.md` (Install); `dist-workspace.toml` (targets); `docs/bench/m1.md` (win-pc). Verify: the Linux binary was not run in any benchmark file read. |
| Wireless debugging works with `adb pair` and `adb connect`, tested on an emulator over TCP, not a real network | HN answer 7 | `docs/bench/wifi.md` |
| AGPL-3.0-only. Applies when you distribute it or run a modified copy as a network service. | HN answer 1 | `README.md` (License); `docs/decisions.md` D13 |
| The reason for AGPL: stop a competing tool taking the code into a closed or permissive project | HN answer 1 | `docs/decisions.md` D13 |
| Every dependency is permissive. The Android agent has scrcpy files under Apache-2.0 in `NOTICE`. | HN answer 1 | `docs/decisions.md` D13; `README.md` (License); `NOTICE` |
| That running the unmodified tool as a tool asks nothing of you | HN answer 1 | A legal reading, not in the repo. The `[USER: ...]` slot asks the author to confirm. |
| No telemetry: no install id, no event, no crash upload. phone-use talks to the device and nothing else. | HN, X, LinkedIn, answer 8 | `docs/decisions.md` D9; `README.md` (Use with other MCP clients, last paragraph). The repo has no network-silence test for phone-use itself; the check in `docs/bench/m1.md` covers phone-harness. |
| Apple is the only outside party, for developer disk image mounting | HN answer 8 | `AGENTS.md` (No telemetry rule) |
| phone-harness ships the script, its output and every helper call to PostHog by default | HN answer 8 | `docs/research/competitor-teardown.md` (item 1: "Telemetry ships user content, on by default") |
| The author read the phone-harness source before starting | HN draft slot | `docs/research/competitor-teardown.md` (phone-harness `4bebff1`); `progress.md` (2026-11-05 entry) |
| The repo holds plans, 21 Android and 12 iOS device scenarios, benchmarks with raw data and audits | HN answer 11 | `docs/plans/`; `docs/bench/m2.md`, `m4.md`, `docs/bench/data/`; `docs/audits/` |
| Another project named phone-use exists (`sauravtom/phone-use`, MIT, Android, 6 stars) | HN answer 11, plan | Not in the repo. GitHub API, 2026-11-06 (`research.md`). |
| Coding agents did part of the work | HN decision 1 | `AGENTS.md`, `progress.md` |

## Speed (Android 17 emulator, win-pc, busy machine, median of 50 after 5 warm-ups)

| Claim | Where used | Backing file |
|---|---|---|
| Screenshot 45 ms (45.3) against 692 ms (692.4) with `adb exec-out screencap` | HN, X thread 4 | `docs/bench/adb.md` (results table; run B for phone-use) |
| Settings screen read 237 ms (237.0) against 2,800 ms (2,800.5) with `uiautomator dump` | HN, X, LinkedIn | `docs/bench/adb.md` (results table; run B is the slower of two; phone-use figure is `--no-settle`) |
| Tap then know what changed 667 ms (666.9) against 3,112 ms (3,111.7) | HN, X thread 4 | `docs/bench/adb.md` |
| A bare tap is 1.6 times faster than adb | HN, X thread 4 | `docs/bench/adb.md` (ratios paragraph: tap 1.6x) |
| adb screen read took 2.8 s | HN, LinkedIn, X | `docs/bench/adb.md` |
| phone-use and adb were measured back to back; phone-use A and B bracket the adb run | HN, X thread 4, answer 9 | `docs/bench/adb.md` (Conditions) |
| The machine was busy; phone-use's tap was 130 to 144 ms, against 30 to 37 ms in the quiet M1 run | HN | `docs/bench/adb.md` (Conditions, fairness caveats); `docs/bench/m1.md` |
| Tap budget is 15 ms and is missed | HN | `docs/bench/m1.md` (verdict); `docs/decisions.md` D14 |
| adb's tap returns before the app reacts; phone-use's waits for the app | HN answer 9 | `docs/bench/adb.md` (fairness caveats); `docs/decisions.md` D14 |
| adb tree is raw XML and phone-use's is pruned text | HN answer 9 | `docs/bench/adb.md` (fairness caveats) |
| The phone-harness numbers are from an earlier quiet run, in process, which favors phone-harness | HN answer 9 | `README.md` (Speed); `docs/bench/m1.md` |
| `input text` fails on Unicode; phone-use typed mixed-script text | HN answer 2 | `docs/bench/adb.md` (fairness caveats); `docs/bench/m1.md` (Unicode) |
| No physical Android phone has been measured | HN, X, LinkedIn | `README.md` (Speed note; Supported devices); `docs/bench/m1.md` (Not done) |
| Raw samples are in `docs/bench/data/`; `phone-use-bench` reproduces them | HN answer 9 | `docs/bench/adb.md`; `docs/bench/m1.md`; `bench/` |
| Every fact above is for an emulator; `adb.md` has no quiet-machine re-run | HN, X | `docs/bench/adb.md` (Not done) |

## iOS

| Claim | Where used | Backing file |
|---|---|---|
| Tap takes 357 ms on the iOS 26.5 Simulator | HN, X, LinkedIn | `README.md` (Speed note); `docs/bench/m4.md` |
| Tap takes 534 ms (iPhone 14 Plus, iOS 26.1) and 454 ms (iPad Pro M5, iOS 27.0) | HN, X, LinkedIn | `docs/bench/m5.md` |
| XCTest takes about 240 ms to confirm an event | HN, X, LinkedIn | `docs/decisions.md` D15; `docs/bench/m4.md` |
| Tested on an iPhone 14 Plus and an iPad Pro through the CLI and MCP; the 12-scenario suite was not run on devices | HN answer 7 | `README.md` (Supported devices); `docs/bench/m5.md` (Verdict) |
| 12 of 12 iOS Simulator scenarios, three runs in a row | HN answer 11 | `docs/bench/m4.md` (Verdict) |
| One iOS scenario, `settings-dark-appearance`, failed twice in six full runs and the cause is not found | HN | `docs/bench/m4.md` (Verdict, "The flake") |
| The demo: Claude Code's own terminal, Sonnet 5.5, iPhone 17 Simulator, iOS 26.5, real recording, 24 s from prompt to answer | HN, X, LinkedIn | `README.md` (demo caption) |
| Cut only at the start and end; nothing trimmed between tool calls | X, LinkedIn | `README.md` (demo caption) |
| First take: the typed search did not register, Safari landed on a donation page, the agent loaded the article by URL, every call succeeded | X, LinkedIn | `README.md` (demo caption) |
| The answer in the clip is December 9, 1906 | X alt text | `README.md` (demo `alt`) |
| `demo.mp4` is 960x660, 30 fps, 32 s, 1 MB | X, LinkedIn, plan | `.github/assets/demo.mp4` (ffprobe, 2026-11-06). The README's "24 s" is prompt to answer; the file is 32 s with the start and end. |

## Agent evaluation

| Claim | Where used | Backing file |
|---|---|---|
| Claude Sonnet 5.5 completed 15 of 15 held-out runs on five Android tasks | HN, LinkedIn | `docs/bench/m3.md` (Held-out gate) |
| Median 6 tool calls, 42,817 tokens (including cache reads), 18 s per run | HN, LinkedIn, answer 10 | `docs/bench/m3.md` |
| The first pass scored 9 of 15; all six failures were bugs in the check or the device reset, not the tool | HN, LinkedIn | `docs/bench/m3.md` (first-pass table and "No failure was caused by phone-use") |
| The agent saw ten tools and a one-paragraph instruction in that eval, with no SKILL.md | (not in drafts; context for answer 10) | `docs/bench/m3.md` (runner paragraph) |
| 21 Android scenarios pass on API 26 and 37 emulators | HN answer 11 | `docs/bench/m2.md` |

## Competitors (verify before quoting)

| Claim | Where used | Backing file |
|---|---|---|
| mobile-mcp: no wait after actions in `server.ts`, so the agent re-lists elements; one line per element with position and size | HN answer 4 | `docs/research/competitors.md` section 3. Verify: the file says the fetches went through a summarizing tool, "re-open the link before quoting". |
| mobile-mcp supports real iOS devices and cloud | HN answer 4 | `docs/research/competitors.md` section 1a |
| Maestro MCP says physical iOS devices are not yet supported | HN answer 3 | `docs/research/competitors.md` section 1a. Verify. |
| Appium needs an installed UiAutomator2 server and a driver stack | HN answer 3 | `docs/research/competitors.md` section 3 ("Appium installs UiAutomator2 server APKs (not read in detail)"). Verify. |
| No competitor published a latency figure the research found | (not used in drafts) | `docs/research/competitors.md` section 3 |

## Number slots

| Slot | Why it is open | Where used |
|---|---|---|
| [TBD: token benchmark] | `docs/research/competitors.md` section 4.7 plans it. `bench/agent-eval/run-harness.sh` and the edit to `tasks.yaml` are uncommitted. No `docs/bench/tokens.md` existed on 2026-11-06. | HN comment, answers 4 and 10, LinkedIn, plan |
| [TBD: final tap latency] | The latest commits changed the Android path (paste, power-state reads). The published numbers predate them. | HN, X, LinkedIn, plan |
| [TBD: re-run on a quiet machine] | `docs/bench/adb.md` "Not done". | HN, X |
| [TBD: iOS hybrid tier] | CoreDevice is experimental and off by default. | HN, plan |
| [TBD: shell tool] | D6 says none. The user named a shell tool as open. | HN, answer 5, plan |

## Round 2 additions (paths on `main` at `c2297a1` unless noted)

| Claim | Where used | Backing file |
|---|---|---|
| A `shell` verb and MCP tool exist. On Android it runs in the on-device agent over the open control stream. After a state-changing command it settles and returns the screen diff. A Simulator runs it through `simctl spawn`. A physical iPhone has no shell. | HN, X, Reddit | `docs/decisions.md` D21; `README.md` ("Shell"); `docs/bench/shell.md` |
| A physical Android device refuses wipes and removing apps phone-use did not install unless `--force`. The guard is "a guard against slips, not a sandbox". Emulators and Simulators have no guard. | HN answer 5, Reddit variant 1 | `docs/decisions.md` D21 ("Safety") |
| D6 says "never execute agent-written code" and D21 ships `shell` | HN decision 4, plan | `docs/decisions.md` D6 against D21 |
| Fourteen MCP tools with `shell` | (not used in drafts) | `README.md` (on `main`) |
| Token benchmark: phone-use 3 of 3 passed, 413,534 tokens, 45 calls; mobile-mcp 0 of 3 (environment failure), 537,300; phone-harness 3 of 3, 337,059; raw adb 2 of 3, 240,389. One run per cell, tablet emulator, shell tool not in the run. | HN, X reply 3, Reddit, LinkedIn | `docs/bench/tokens.md` (on `main`) |
| Tap p50 33.7 to 35.9 ms on the API 37 emulator after the track-devices change, three interleaved runs of 100; the previous commit was 50.6 to 52.7 ms | HN, X, LinkedIn | `progress.md` ("Speedup 4", on `main`) |
| Tap budget is 15 ms and is missed | HN, X | `docs/bench/m1.md`; `docs/decisions.md` D14 |
| Hybrid iOS tier (CoreDevice input and frames, runner tree) is opt-in, unfinished and unproven on device | HN block 3 | `progress.md` on this branch (2026-11-06 hybrid entries); `README.md` (`PHONE_USE_IOS_TIER` row) |
| `screenshot` can draw `@eN` marks for vision models | Reddit variant 2 | commit `3d52a9a` ("feat(screenshot): draw @eN marks on the screenshot for vision models") |
| A `getChild` prefetch is cancelled unless `FLAG_PREFETCH_UNINTERRUPTIBLE` is also set; the fix cut a tree read from 78 to 103 ms to 41 to 46 ms | Reddit variant 3 | `docs/bench/m1.md` ("What limits each phone-use number") |
| One `unsafe` module for a Windows handle list | Reddit variant 3 | `docs/bench/m1.md` ("Windows fixes", item 1) |
| Internal wires are length-prefixed JSON; adb protocol spoken directly; resident daemon | Reddit variant 3 | `docs/decisions.md` D7, D11, D1 |
| `claude mcp add phone-use -- <path> mcp` | Reddit variant 1 | `README.md` (Try it) |
| Any client that starts a stdio MCP server can use it | Reddit variant 2 | `README.md` ("Use with other MCP clients"). No local-model run is in the repo. |
| Snapshots are 183 to 822 tokens on measured screens; schemas about 2.6k tokens | HN, Reddit | `docs/audits/ax-audit.md`; `progress.md` (tools/list entries) |
| mobile-mcp has 10.4k stars | HN answer 4 | `docs/research/competitors.md` section 1a (stars as displayed 2026-11-06) |
| Name check: `phone-use` is taken on npm and as the GitHub organization `phone-use`; `tapdiff`, `phonewright`, `diffphone` are free on crates.io, npm, PyPI and as GitHub accounts | plan | Not in the repo. Registry and GitHub API checks of 2026-11-06 (`research.md`). |

New number slots in round 2: [TBD: paired quiet-machine re-run including tap]; [TBD: token rerun with the shell tool and mobile-mcp on a local emulator]; [TBD: re-recorded demo]; [TBD: local-model result]; [TBD: iOS hybrid tier]. The old [TBD: shell tool] slot is closed: it shipped.
