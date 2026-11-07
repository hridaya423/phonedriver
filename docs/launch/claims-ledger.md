# Claims ledger

**Round 2 status.** Rows below that name the old demo are superseded: the demo is being re-recorded, and no draft now leads with or mentions how the old clip went. Rows about "no shell tool" are superseded by D21 (see Round 2 additions). Paths marked "on `main`" exist at `main` commit `c2297a1`, not in this worktree branch.

Every factual claim in the launch drafts (`show-hn.md`, `x.md`, `reddit.md`, `linkedin.md`, `product-hunt.md`, `devto-hashnode.md`, `short-video.md`, `directories.md`), with the repo file that backs it. Paths are relative to the repo root. "Verify" marks a claim the repo supports only partly or that came through a summarizing fetch. Check each row against the repo at the launch commit before posting. Web claims are in `research.md`, with their URLs.

## Product

| Claim | Where used | Backing file |
|---|---|---|
| phonedriver gives an agent a text view of an Android or iOS screen and the means to tap, type and swipe. Every action returns only the lines that changed. | HN, X | `README.md` (opening paragraph) |
| One Rust binary: CLI, daemon and MCP server | HN, LinkedIn | `docs/decisions.md` D1 |
| Android: pushes a dex jar, starts it with `app_process` as the shell user, as scrcpy does. Nothing installed on the phone, no accessibility service to enable. Tree through UiAutomation, input through InputManager. | HN, X | `docs/decisions.md` D2; `README.md` (Supported devices: "Android needs no install on the phone") |
| Every action waits for settle and returns the diff. Settle uses accessibility events and video frames. The agent does not poll or sleep. | HN, X, LinkedIn | `docs/decisions.md` D12, D10; `README.md` (Try it, step 3); `docs/research/competitors.md` section 3 ("event-driven settle", architecture 4.6) |
| The sample reply `~ @e41 switch "Dark Appearance" off → on`, `+ @e68 button "L4S"`, `settled in 607ms` | HN, X thread 2 | `README.md` (Try it, step 3, iOS Simulator) |
| Refs such as `@e10` are found again at tap time, so a list that shifted by a row still gets the right tap | HN | `README.md` (How it works) |
| iOS uses our own XCTest runner, not WebDriverAgent | HN | `docs/decisions.md` D4 |
| Experimental CoreDevice tier on iOS 27, off by default | HN | `README.md` (`PHONEDRIVER_IOS_TIER`); `docs/decisions.md` D3; `docs/bench/m5.md` (CoreDevice sections: "Default stays Runner") |
| The agent calls typed verbs. No shell tool, no code execution. | HN, answer 5 | `docs/decisions.md` D6; `docs/research/competitors.md` 1b ("shell: none (by design D6)") |
| Thirteen MCP tools | HN answer 10 | `README.md` (Use with other MCP clients) |
| The 13 tool schemas cost about 2.6k tokens | HN answer 10 | `progress.md` (tools/list after gap 4: 9,691 characters plus 345, about 2,573 tokens by a 3.9 characters-per-token ratio, no tokenizer run). Verify: the progress entry for later gaps may have changed the size. |
| A snapshot is 183 to 822 tokens on the measured screens (chars divided by four) | HN answer 10 | `docs/audits/ax-audit.md` (snapshot sizes line) |
| WebView and custom views can have an empty tree, and OCR reads the screen then | HN | `progress.md` (Task 7.2: `tree_is_hollow`, `text "Web form" (ocr)`) |
| A tapped WebView checkbox shows no checked state | HN | `docs/bench/m3.md` (tuning item 4, open items) |
| Text sent with `--secret` never reaches logs, traces or replies | HN answer 5 | `README.md` (Use with other MCP clients, last paragraph); `AGENTS.md` (Secrets) |
| The daemon listens on a Unix socket and nowhere else; the socket sits in a mode-700 directory with mode 600 | HN answer 5 | `README.md` ("Use a device on another machine"; `phonedriver doctor` sample output) |
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
| No telemetry: no install id, no event, no crash upload. phonedriver talks to the device and nothing else. | HN, X, LinkedIn, answer 8 | `docs/decisions.md` D9; `README.md` (Use with other MCP clients, last paragraph). `crates/cli/tests/network.rs` runs the CLI, the daemon and the MCP server under `sandbox-exec` with every non-local connection killed (macOS only, so Linux and Windows are not covered by a test); `docs/bench/m1.md` covers phone-harness. The test uses a fake adb server and no device; a live session on an emulator under the same sandbox is in `progress.md`. |
| Apple is the only outside party, for developer disk image mounting | HN answer 8 | `AGENTS.md` (No telemetry rule). The network test never mounts a disk image, so this half is by code reading (`crates/ios/src/setup.rs`), not by test. |
| phone-harness ships the script, its output and every helper call to PostHog by default | HN answer 8 | `docs/research/competitor-teardown.md` (item 1: "Telemetry ships user content, on by default") |
| The author read the phone-harness source before starting | HN draft slot | `docs/research/competitor-teardown.md` (phone-harness `4bebff1`); `progress.md` (2026-11-05 entry) |
| The repo holds plans, 21 Android and 12 iOS device scenarios, benchmarks with raw data and audits | HN answer 11 | `docs/plans/`; `docs/bench/m2.md`, `m4.md`, `docs/bench/data/`; `docs/audits/` |
| Another project named phone-use exists (`sauravtom/phone-use`, MIT, Android, 6 stars) | HN answer 11, plan | Not in the repo. GitHub API, 2026-11-06 (`research.md`). |
| Coding agents did part of the work | HN decision 1 | `AGENTS.md`, `progress.md` |

## Speed (Android 17 emulator, win-ts, all tools local and interleaved, p50 of 50 after 20 warm-ups)

Figures below are from `docs/bench/speed-2026-11-07.md` (raw samples in `docs/bench/data/speed-2026-11-07/`, chart data in `docs/bench/speed.json`). The older figures in `docs/bench/adb.md` (busy win-pc run) and `docs/bench/m1.md` are superseded for every public claim. Launch drafts that still quote 45.3, 237.0, 666.9, 144.1 or 153 are stale (`docs/launch/linkedin.md`, `show-hn.md`, `x.md`, `plan.md`).

| Claim | Where used | Backing file |
|---|---|---|
| Up to 98 times faster than adb at p50: the 1,000-row list read, 24.0 ms against 2,352.4 ms, 95% CI on the ratio 92.2 to 106.0 (banner, social preview, chart headline; computed from `docs/bench/speed.json`) | README, banner, social preview | `docs/bench/speed-2026-11-07.md` (Results, Headline rule) |
| Screenshot 11.6 ms against 476.3 ms with `adb exec-out screencap` (41x), against phone-harness 570.5 ms (49x) | HN, X thread 4 | `docs/bench/speed-2026-11-07.md` |
| Settings screen read 89.3 ms against 2,433.5 ms with `uiautomator dump` (27x), against phone-harness 2,700.0 ms (30x); the settled snapshot is 92.5 ms | HN, X, LinkedIn | `docs/bench/speed-2026-11-07.md` |
| Type 100 ASCII characters 251.1 ms against 4,840.9 ms with `input text` (19x), against phone-harness 5,523.3 ms (22x) | HN, X | `docs/bench/speed-2026-11-07.md` (second pass, host load 20%) |
| Tap then know what changed 586.9 ms against 2,725.8 ms for tap, one dump and a diff (4.6x) | HN, X thread 4 | `docs/bench/speed-2026-11-07.md` |
| A bare tap is 2.1 times faster than adb when taps are back to back (117.6 ms against 250.5 ms; interval 1.2 to 3.0), 1.6 times when 1.5 s apart (72.4 ms against 114.5 ms) | HN, X thread 4 | `docs/bench/speed-2026-11-07.md` (Results, What this does not show) |
| On the device, from the host sending a tap to the app's touch listener, phonedriver takes 27.5 ms, adb 71.2 ms, phone-harness 76.6 ms | HN, X | `docs/bench/speed-2026-11-07.md` (Method, Results: tap, device side; clock offset good to about 8 ms) |
| Fresh daemon to first screen read 1,620.5 ms, phone-harness fresh process 2,999.4 ms, adb one dump 2,488.0 ms | HN | `docs/bench/speed-2026-11-07.md` (Cold start; adb has no cold start, so for scale only) |
| Everything ran on the emulator's host with the three tools interleaved; mean host CPU load 13% to 39% per op from other work | HN, X thread 4, answer 9 | `docs/bench/speed-2026-11-07.md` (Machine and state, Method) |
| The 153x of the first M1 run is not a current number: the list read is 111x phone-harness and 98x adb now | HN answer 9 | `docs/bench/speed-2026-11-07.md` (What happened to the 153x) |
| Tap budget is 15 ms and is missed | HN | `docs/bench/m1.md` (verdict); `docs/decisions.md` D14 |
| adb's tap returns before the app reacts; phonedriver's waits for the app | HN answer 9 | `docs/bench/adb.md` (fairness caveats); `docs/decisions.md` D14 |
| adb tree is raw XML and phonedriver's is pruned text | HN answer 9 | `docs/bench/speed-2026-11-07.md` (What this does not show) |
| phone-harness helpers were called in process (no Python start per call), which favors phone-harness | HN answer 9 | `docs/bench/speed-2026-11-07.md` (Machine and state) |
| `input text` fails on Unicode; phonedriver typed mixed-script text in 225.0 ms | HN answer 2 | `docs/bench/speed-2026-11-07.md`; `docs/bench/m1.md` (Unicode) |
| No physical Android phone has been measured | HN, X, LinkedIn | `README.md` (Speed note; Supported devices); `docs/bench/m1.md` (Not done) |
| Raw samples are in `docs/bench/data/`; `phonedriver-bench` reproduces them | HN answer 9 | `docs/bench/adb.md`; `docs/bench/m1.md`; `bench/` |
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
| The first pass scored 9 of 15; all six failures were bugs in the check or the device reset, not the tool | HN, LinkedIn | `docs/bench/m3.md` (first-pass table and "No failure was caused by phonedriver") |
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
| A physical Android device refuses wipes and removing apps phonedriver did not install unless `--force`. The guard is "a guard against slips, not a sandbox". Emulators and Simulators have no guard. | HN answer 5, Reddit variant 1 | `docs/decisions.md` D21 ("Safety") |
| D6 says "never execute agent-written code" and D21 ships `shell` | HN decision 4, plan | `docs/decisions.md` D6 against D21 |
| Fourteen MCP tools with `shell` | (not used in drafts) | `README.md` (on `main`) |
| Token benchmark: phonedriver 3 of 3 passed, 413,534 tokens, 45 calls; mobile-mcp 0 of 3 (environment failure), 537,300; phone-harness 3 of 3, 337,059; raw adb 2 of 3, 240,389. One run per cell, tablet emulator, shell tool not in the run. | HN, X reply 3, Reddit, LinkedIn | `docs/bench/tokens.md` (on `main`) |
| Tap p50 33.7 to 35.9 ms on the API 37 emulator after the track-devices change, three interleaved runs of 100; the previous commit was 50.6 to 52.7 ms | HN, X, LinkedIn | `progress.md` ("Speedup 4", on `main`) |
| Tap budget is 15 ms and is missed | HN, X | `docs/bench/m1.md`; `docs/decisions.md` D14 |
| Hybrid iOS tier (CoreDevice input and frames, runner tree) is opt-in, unfinished and unproven on device | HN block 3 | `progress.md` on this branch (2026-11-06 hybrid entries); `README.md` (`PHONEDRIVER_IOS_TIER` row) |
| `screenshot` can draw `@eN` marks for vision models | Reddit variant 2 | commit `3d52a9a` ("feat(screenshot): draw @eN marks on the screenshot for vision models") |
| A `getChild` prefetch is cancelled unless `FLAG_PREFETCH_UNINTERRUPTIBLE` is also set; the fix cut a tree read from 78 to 103 ms to 41 to 46 ms | Reddit variant 3 | `docs/bench/m1.md` ("What limits each phonedriver number") |
| One `unsafe` module for a Windows handle list | Reddit variant 3 | `docs/bench/m1.md` ("Windows fixes", item 1) |
| Internal wires are length-prefixed JSON; adb protocol spoken directly; resident daemon | Reddit variant 3 | `docs/decisions.md` D7, D11, D1 |
| `claude mcp add phonedriver -- <path> mcp` | Reddit variant 1 | `README.md` (Try it) |
| Any client that starts a stdio MCP server can use it | Reddit variant 2 | `README.md` ("Use with other MCP clients"). No local-model run is in the repo. |
| Snapshots are 183 to 822 tokens on measured screens; schemas about 2.6k tokens | HN, Reddit | `docs/audits/ax-audit.md`; `progress.md` (tools/list entries) |
| mobile-mcp has 10.4k stars | HN answer 4 | `docs/research/competitors.md` section 1a (stars as displayed 2026-11-06) |
| Name check: `phone-use` is taken on npm and as the GitHub organization `phone-use`; `tapdiff`, `phonewright`, `diffphone` are free on crates.io, npm, PyPI and as GitHub accounts | plan | Not in the repo. Registry and GitHub API checks of 2026-11-06 (`research.md`). |

New number slots in round 2: [TBD: paired quiet-machine re-run including tap]; [TBD: token rerun with the shell tool and mobile-mcp on a local emulator]; [TBD: re-recorded demo]; [TBD: local-model result]; [TBD: iOS hybrid tier]. The old [TBD: shell tool] slot is closed: it shipped.

## README restructure, hero chart, FAQ, compare page (2026-11-07)

The product is named phonedriver in this copy; the code still says phonedriver until the rename pass.

| Claim | Where used | Backing file |
|---|---|---|
| phonedriver is an open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones. (canonical sentence) | README top, `Cargo.toml`, `llms.txt`, `docs/README.md`, `docs/compare.md`, `docs/launch/github-metadata.md` | MCP: `phonedriver mcp`, README "Use with other MCP clients". CLI: README "Try it". Claude Code: `claude mcp add` in README. Android and iOS: README "Supported devices". Open source: `LICENSE` (AGPL-3.0-only), `Cargo.toml` |
| Up to 98x faster than adb | hero chart title, README, FAQ | `docs/bench/speed.json` (`headline_adb` of `tree_list`), `docs/bench/speed-2026-11-07.md` "Verdict" |
| Read 1,000-row tree 24.0 vs 2,352.4 ms (98x); screenshot 11.6 vs 476.3 (41x); type 100 chars 251.1 vs 4,840.9 (19x); tap and see what changed 586.9 vs 2,725.8 (4.6x) | hero chart, README Numbers, compare.md | `docs/bench/speed.json`; `docs/bench/speed-2026-11-07.md` "Results" |
| phone-harness is 111x, 49x and 22x slower on the first three rows; "tap, then what changed" not supported | hero chart, README Numbers, compare.md | `docs/bench/speed-2026-11-07.md` ratio table |
| Hero bars are drawn to scale: adb full width, phonedriver at its p50 fraction of adb | hero chart | `design/build/build.py` `hero()`, reads `speed.json` |
| p50 of 50 runs on an Android 17 emulator; no physical phone measured | hero subtitle, README, FAQ | `docs/bench/speed-2026-11-07.md` "Method", "What this does not show" |
| mobile-mcp has no speed figure here | README Numbers, compare.md, FAQ | Not in `docs/bench/speed-2026-11-07.md`; `docs/research/competitors.md` "Nothing here was benchmarked" |
| 218,357 tokens against 330,375, 34% fewer; all runs passed for both | README Numbers, compare.md, FAQ | `docs/bench/tokens.md` "Round 3" (sum of medians; 218,357 / 330,375 = 0.661) |
| phonedriver used more on `wiki-birthdate`, 57,753 against 53,403 | FAQ, compare.md | `docs/bench/tokens.md` "Round 3" |
| mobile-mcp passed 0 of 3, an environment failure; adb failed the web form | README Numbers, compare.md | `docs/bench/tokens.md` "Caveats" |
| mobile-mcp: no wait or diff after actions; Android, iOS, cloud; Apache-2.0; batch, recording, logs | README Numbers, FAQ, compare.md | `docs/research/competitors.md` sections 1a, 1b, "Act and observe in one call". Verify: fetched through a summarizing tool on 2026-11-06 |
| phone-harness: Python CLI, OCR and `uiautomator dump`, iPhone via Mac mirroring, MIT | README Numbers, compare.md | `docs/research/competitor-teardown.md`; `docs/research/competitors.md` 1a |
| adb and phone-harness cannot type Unicode; phonedriver typed mixed-script text in 225.0 ms | compare.md | `docs/bench/speed-2026-11-07.md` "Results" |
| Approval guard stops `Place order`, `Pay`, `Delete account`; exit code 5; fixed English labels | README Numbers, FAQ, compare.md | `docs/decisions.md` D27; README "Debugging" (now `docs/reference.md`) |
| Shell on the device | README Numbers, compare.md | `docs/decisions.md` D21 |
| iPhone 14 Plus and iPad Pro tested, tap 454 to 534 ms | FAQ, compare.md | README "Supported devices"; `docs/bench/m5.md` |
| AGPL-3.0-only; network-service source clause; no telemetry | FAQ | `LICENSE`; README "License" and "Use with other MCP clients"; decision D9 |
| GitHub description (106 characters) and 11 topics, each topic non-empty on GitHub | `docs/launch/github-metadata.md` | `gh search repos --topic <t> --limit 1`, 2026-11-07 |

## Round 3: launch kit rebuild (2026-11-07)

The drafts in `docs/launch/` were rewritten on 2026-11-07 and now quote only the rows below and the "README restructure" rows above. The stale figures in earlier sections (45.3, 237.0, 666.9, 144.1, 153, 175.6, 61.6, the 413,534-token run) are gone from the drafts. "Verify" marks a row to check at the launch commit. Slots: `[DEMO]` waits on the re-recording, `[JOBS]` on the real-jobs runs.

| Claim | Where used | Backing file |
|---|---|---|
| Up to 98x faster than adb, 111x phone-harness, at reading the screen: 24.0 ms against 2,352.4 ms (adb) and 2,653.7 ms (phone-harness); 2,653.7 / 24.0 = 110.6, rounded to 111 | all channels | `docs/bench/speed.json` (`tree_list`), `docs/bench/speed-2026-11-07.md` |
| The 98x is for one op, a 1,000-row list read. The Settings screen read is 27x; the screenshot 41x; type 100 characters 19x; tap then what changed 4.6x | HN, X, Reddit, LinkedIn | `docs/bench/speed-2026-11-07.md` Results |
| Back-to-back tap 2.1x faster than adb (117.6 against 250.5 ms, CI on the ratio 1.22 to 2.95, p50 ratio 2.13); 1.5 s apart 1.6x (72.4 against 114.5 ms, CI 1.47 to 1.73) | HN, X follow-up 1 | `docs/bench/speed.json` (`tap`, `tap_paced`) |
| phone-harness cannot do "tap, then what changed" | README, compare | `docs/bench/speed-2026-11-07.md` ratio table |
| 34% fewer tokens than phone-harness across three agent tasks, 218,357 against 330,375, every run passed for both; phonedriver behind on one task (57,753 against 53,403); phone-harness figures are from round 2, not rerun in round 3, same runner and model; one to three runs per cell | all channels | `docs/bench/tokens.md` "Round 3" |
| The tokens chart `.github/assets/tokens-light.png` does not show these sums (it shows five series and the old name); do not post it | X follow-up 3, LinkedIn slide 6 | Opened 2026-11-07; rebuild first |
| Demo (today): Claude Code 2.1.319, Sonnet 5.5, Android 17 emulator, Uber Eats, 10x sped up, the run took 6 min 7 s, last 3 s at real speed, file 960x660, 38.9 s, ends at Uber's sign-in page, nothing ordered; the terminal says `phone-use` | X hook, Reddit, LinkedIn, short video | README demo caption, `progress.md` "README demo retake", ffprobe 2026-11-07, frames looked at |
| Demo content: rejected the cookie banner, set the Times Square address, searched Indian, a mis-tap landed on an Uber One page and the agent went back, picked Patiala Indian Grill and Bar (4.8 stars, 900+ reviews), Butter Chicken $20.95, Garlic Naan $7.95, subtotal $28.90 | Reddit r/ClaudeCode, short video | `progress.md` "README demo retake"; frames at 8, 16, 24, 32 s |
| Signed out, Uber Eats never shows Place order or Pay, so the approval guard did not fire in today's clip | short video, X hook | `progress.md` "README demo retake" (Found) |
| The new demo (Opus 5.5, approval-prompt scene) | X hook variant, LinkedIn, short video, PH | [DEMO]: not recorded. Replace every "[DEMO]" line, and every claim about what the prompt looks like, after looking at the new file |
| Approval guard: phonedriver asks the human through MCP elicitation before a tap on Pay or Place order; choices Allow, Allow for this session, Decline; the model cannot answer it; autonomous mode when the user grants it (`autonomous: true`, `--autonomous`, `PHONEDRIVER_GUARD=off`) | all channels | `docs/decisions.md` D27; `site/content/docs/approval-guard.md`. Verify: that Claude Code 2.1.319 renders the form as described is by D27 and the protocol notes, and the new recording must show it |
| Guard limits: English labels only, fixed list; icon-only buttons not guarded; `shell` `input tap` not seen; a model set on bypassing it can pass `autonomous`; headless `claude -p` answers cancel so the tap is not made; a change smaller than a quarter of the screen adding a guarded button inside 1 s is missed; no screenshot in the prompt | HN answer 4, Reddit, LinkedIn slide 7, X post 5 | D27 "Limits"; blog post "Letting Claude order dinner" |
| Shell on the device | README, compare | D21 |
| iPhone 14 Plus (iOS 26.1) and iPad Pro (iOS 27.0) tested through the CLI and MCP; tap 454 to 534 ms; the 12-scenario suite not run on devices; Simulator tap 357 ms | all channels | `docs/bench/m5.md`; README Supported devices |
| No physical Android phone measured; Android hardware untested | all channels | README Speed note; Supported devices |
| 14 MCP tools, about 51 tokens each in Claude Code's wrapper; first turn 5,606 tokens against 5,214 for phone-harness. The older "13 tools, about 2.6k tokens" row above is superseded | Reddit r/mcp | `docs/bench/tokens.md` "Round 3" |
| Snapshots are 183 to 822 tokens on measured screens | Reddit r/mcp | `docs/audits/ax-audit.md` |
| A test runs the CLI, the daemon and the MCP server under a macOS `sandbox-exec` that kills any non-local connection; it uses a fake adb server and no device, runs on macOS only | HN, Reddit, X | `crates/cli/tests/network.rs`; README. The README names the test `cargo test -p phonedriver --test network`; after the rename it is `-p phonedriver` (verify) |
| The Cursor config is documented and untested | Reddit r/mcp, directories | `site/content/docs/cursor-and-mcp-clients.md` |
| Claude Code is the tested client | Reddit | `site/content/docs/claude-code.md` |
| Names `phonedriver` on npm, crates.io and PyPI return 404 (free) on 2026-11-07; GitHub user `phonedriver` 404; the repo `hridaya423/phonedriver` does not exist yet (404) | plan, directories | curl against the registries and the GitHub API, 2026-11-07 |
| `phonedriver.dev` is registered (whois status ACTIVE) and has no DNS address; nothing deployed | plan, github-metadata | whois and dig, 2026-11-07. Who owns it: [USER] |
| Jobs benchmark: phonedriver passed 18/18 runs (9 jobs, 2 runs each), 977,780 tokens, 123.5 calls, 466.5 s (sums of per-job medians) | HN, blog post 4, PH, benchmarks page | `docs/bench/jobs.json` `totals`; `docs/bench/jobs.md` "Totals". Claude Sonnet 5.5 on a tablet emulator, subscription |
| Jobs benchmark: mobile-mcp 18/18, 4,892,241 tokens (about 5x phonedriver), 910 s | blog post 4, benchmarks page | `docs/bench/jobs.json` `totals` |
| Jobs benchmark: phone-harness 14/18, 1,272,617 tokens, 1,946 s; relay-code 0/2 (uiautomator dump fails on the Contacts list), qa-signup and qa-shop 1/2 each (pass 1 hit the 10-minute limit; why was not diagnosed) | blog post 4, benchmarks page | `docs/bench/jobs.md` "What each harness failed at" |
| Jobs benchmark: adb 4/4 on the two mixed jobs only (log-code 45.5 s, wakelock-restrict 49 s), 144,669 tokens | blog post 4 | `docs/bench/jobs.json` `totals` |
| Where phonedriver does not win: phone-harness is cheaper on photos-crop (104k against 221k), clock-settings (77k against 95k) and log-code (41k against 59k); mobile-mcp is 0.5 s faster on wakelock-restrict (28.5 s against 29 s) | HN, blog post 4, benchmarks page | `docs/bench/jobs.md` "Where phonedriver does not win" |
| The 23% token lead over phone-harness (977,780 against 1,272,617) comes mostly from phone-harness's failed runs; say it with the number. Two passes per cell, one model, one emulator: medians describe what happened, not rates | HN, blog post 4 | `docs/bench/jobs.json`; the two timed-out phone-harness runs spent 265k and 623k tokens |
| One phonedriver qa-shop run was scored a fail by rubric v1 and passes under v2; the original verdict is kept (`success_as_run`) | blog post 4 | `docs/bench/jobs.md`, `bench/agent-eval/jobs/summarize.py` |
| A field that refuses `set_text` is selected with Ctrl+A and pasted over (a backspace version failed on an unmappable key) | blog post 4 | `android-agent/src/dev/phonedriver/agent/TextInput.java`, `Input.java`; `docs/bench/jobs.md` fixes table |
| First agent-eval pass scored 9 of 15 and all six failures were bugs in the checks or device reset | HN answer 11, Reddit r/SideProject | `docs/bench/m3.md` |
| Directory facts (Smithery dead, the registry's package types, awesome-mcp-servers category and line format, awesome-claude-code 14-day or 100-star rule, awesome-ios rules, Docker licence line) | directories.md | Fetched 2026-11-07; URLs in `directories.md`. Rules change: re-open each page on the day |
| Platform mechanics (HN, X, Reddit, LinkedIn, Product Hunt, Lobsters, dev.to, short video, star conversion) | research and plan | `research-2026-11.md`, with a tag per claim. Reddit rules were secondhand (403) |
| The Show HN title lengths (71, 76, 77, 67 characters) and the tweet lengths | show-hn.md, x.md | Counted with Python 2026-11-07 |
