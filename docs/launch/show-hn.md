# Show HN: phonedriver (round 3)

Nothing here is posted or submitted. This file is **fact notes and a structure. It is not text to paste.**

> **Hacker News bans generated text and AI-edited text.** The guidelines say "Don't post generated text or AI-edited text", and moderators have said they ban accounts for it (`research-2026-11.md`, Hacker News). An agent wrote this file. So the title, the first comment and every reply must be typed by you, in your own words, from the facts below. Do not paste any sentence from this file into HN. Do not run your draft through a model "to tidy it". The README and the site you link also get read for this, so read both once more yourself before launch.

Slots: `[JOBS: ...]` waits on the real-jobs benchmark. `[DEMO: ...]` waits on the re-recorded demo. `[USER: ...]` needs your own words or a decision. Every number below has a row in `claims-ledger.md`.

## Gate: do not submit until all of these are true

A Show HN has to be something people can run. A reader who clicks and cannot install in two minutes leaves, and the post is flagged or ignored.

1. The repo is public at `github.com/hridaya423/phonedriver`, and the README install lines work on a clean machine (today the README says nothing is published).
2. `claude mcp add phonedriver -- phonedriver mcp` then `claude mcp list` shows Connected, from the released binary.
3. phonedriver.dev answers 200 and its links work (`python3 site/check.py` is green).
4. The registry entries in `directories.md` that take minutes (the official MCP Registry, Glama, Smithery) are live, so a visitor who searches "android mcp" finds it.
5. Your HN account is not new and has comment history, and has an email in the profile (`news.ycombinator.com/showlim` restricts Show HN from new accounts). [USER: check age and karma today.]

## What the ranking rules mean here

| Mechanic | Do |
|---|---|
| Rank decays with age (old code: `(points-1)^0.8 / (age_hours+2)^1.8`; current constants secret). | Be at the keyboard for the first two hours. |
| Flamewar and voting-ring software penalties exist and are secret. | Short, factual replies. Never ask anyone to vote. Never post or DM the HN link. Do not tell anyone "it is on HN". |
| A moderator can rewrite a baity title and remove "Show HN" from a post people cannot try. | Plain title, working install. |
| Best hit-rate window in three datasets and an own count: Monday 00:00 UTC (Sunday 7 pm ET, 4 pm PT). | See `plan.md` for the slot chosen and the fallback. |

## Title

80 characters maximum. Format `Show HN: phonedriver – <what it does>`. No superlatives, no "revolutionary", no all-caps, no exclamation. Lengths below are measured, prefix included.

| # | Title | Length |
|---|---|---|
| 1 | Show HN: phonedriver – Let AI agents see and tap Android and iOS phones | 71 |
| 2 | Show HN: phonedriver – Agents read a phone as text and get only what changed | 76 |
| 3 | Show HN: phonedriver – MCP server and CLI for agents to drive phones, in Rust | 77 |
| 4 | Show HN: phonedriver – Claude Code reads an Android screen in 24 ms | 67 |

**Recommended: 1.** It says what the tool is in the words a stranger searches for. Option 2 states the one idea that separates it (the diff) and is the pick if you want the title to carry the "how". Option 4 puts a number in the title, which starts the benchmark argument before readers have seen the caveats; keep the number for the comment. The own cut of 11,542 Show HNs found weak correlations (titles with AI, agent or Claude reach 20+ points 3.5% of the time against 5.1% without; "Rust" and "open source" do better), confounded and small. Do not put the name of a model or "AI-powered" in the title.

## URL: repo or the technical post

Recommended: **the repo** (`https://github.com/hridaya423/phonedriver`). Show HN is for something to try, and the README has the install lines, the speed chart and the demo. Put the technical post in the first comment as "the method" (`https://phonedriver.dev/blog/how-phonedriver-reads-a-screen-98x-faster-than-adb/`). A blog post as the submitted URL reads as an article, not a project, and invites "where is the code" comments.

Do not use a URL shortener. Do not add tracking parameters. Submit the URL with no text in the text box, then post your first comment at once as a normal comment (a Show HN with both a URL and text is allowed, but the first comment as a separate comment lets you edit it for two hours).

## First comment: fact notes (retype in your own words)

Length target: 250 to 400 words. dang's structure: what it is, the backstory of how you came to work on it, what is different, then limits. Do not praise the project ("blazing", "seamless"). State limits before others do.

1. **What it is.** An open-source MCP server and CLI that lets an agent such as Claude Code see and control an Android or iOS phone. One Rust binary: CLI, daemon, MCP server. The screen is numbered text, and every tap returns only the lines that changed (README opening; D1).
2. **Why you built it.** `[USER: what you tried first and why it did not work. This is the part only you can write. Facts available: you read the phone-harness source before starting; `uiautomator dump` costs 2.35 s on a 1,000-row list; an agent that polls and sleeps wastes calls.]`
3. **How it works** (pick two or three you can defend):
   - Android: a dex jar pushed and started with `app_process` as the shell user, the way scrcpy does it. Nothing installed on the phone, no accessibility service to turn on. Tree through UiAutomation, input through InputManager (D2).
   - Every action waits for the screen to settle (accessibility events plus video frames) and returns the diff, so the agent never polls (D12, D10).
   - Refs such as `@e10` are found again at tap time, so a list that moved a row still gets the right tap.
   - iOS: our own XCTest runner, not WebDriverAgent (D4).
   - A `shell` on the device, and an approval guard: before an agent taps `Pay` or `Place order`, phonedriver asks the human through an MCP elicitation, with Allow, Allow for this session and Decline. Autonomous mode when you grant it (D27).
4. **Numbers, with the setting** (Android 17 emulator, p50 of 50 runs after 20 warm-ups, tools interleaved on the same host, `docs/bench/speed-2026-11-07.md`):
   - Read a 1,000-row list: 24.0 ms against 2,352.4 ms for `adb shell uiautomator dump`. 98x, 95% CI on the ratio 92.2 to 106.0. phone-harness 0.3.3: 2,653.7 ms (111x).
   - Screenshot 41x. Type 100 characters 19x. Tap then know what changed 4.6x (586.9 ms against 2,725.8 ms for tap, dump and diff done by the caller). A bare tap is 2.1x back to back and 1.6x 1.5 s apart: the closest row.
   - Tokens, three agent tasks with `claude-sonnet-5-5`, sum of medians: 218,357 against 330,375 for phone-harness, 34% fewer, every run passed for both. phonedriver was behind on one task (57,753 against 53,403). mobile-mcp passed 0 of 3 for an environment reason, so it is not in the comparison.
5. **Limits, plainly:** emulator only for speed, no physical Android phone measured. iOS needs a Mac with Xcode; an iPhone 14 Plus and an iPad Pro were tested through the CLI and MCP, a tap there is 454 to 534 ms, and the 12-scenario suite was not run on devices. The guard matches a fixed list of English labels: a button with no label is not guarded. Coding agents wrote part of the code (`AGENTS.md`, `progress.md`). [USER: say how much you wrote and how you reviewed the rest.] [JOBS: one sentence from `docs/bench/jobs.md` once the runs finish.]
6. **What you want back (two asks):** results on a real Android phone; apps where the snapshot is empty or wrong.

Do not include the demo claim "real time". The demo is sped up 10x and says so.

## Likely skeptic questions and true answers

Answer within 15 minutes for the first two hours, one to four sentences, in your words. Concede what is true. Do not argue the same point twice (the flamewar penalty counts comments).

| # | Question | True answer (source) | Do not say |
|---|---|---|---|
| 1 | "98x against adb is apples to oranges." | Partly. adb returns raw XML; phonedriver returns a pruned text tree, and `uiautomator dump` waits for idle. The ratio is for the same job (read the screen). Emulator only, one host. The tap row, where the outputs match best, is 1.1 to 2.1x. Raw samples and method: `docs/bench/speed-2026-11-07.md`, "What this does not show". | That adb is unusable. That the number holds on hardware. |
| 2 | "Why not Appium or Maestro?" | They are test frameworks for scripts written once and run many times. phonedriver is one action chosen after seeing the screen, with the diff in the reply. Appium needs its UiAutomator2 server installed. Re-read their docs before quoting (`competitors.md` came through a summarizing fetch). | Quotes you have not reopened. |
| 3 | "mobile-mcp already does this, 10k stars." | It does Android, iOS and cloud devices, and has more users. From its `server.ts` it has no wait after actions, so the agent lists the elements again. I did not benchmark its speed, and my token run of it failed for an environment reason. | Anything about its speed or tokens. |
| 4 | "An agent with a phone can spend my money." | Yes, and this is the main reason D27 exists. phonedriver holds a tap on `Pay`, `Place order`, `Delete account` and asks the human through MCP elicitation. The agent cannot answer for itself over MCP. Limits: fixed English labels; icon-only buttons are missed; `shell` `input tap` is not seen; a model set on passing `autonomous: true` can; headless `claude -p` has nobody to ask, so it declines. A seat belt, not a lock (D27, blog post "Letting Claude order dinner"). | That it is safe. That it stops prompt injection in general. |
| 5 | "Prompt injection from the screen." | The guard stops a model that follows its rules or is steered by screen text from tapping Pay. Nothing else in the repo defends against injected text. [USER: your position.] | Any claim of a general defence. |
| 6 | "The demo is sped up." | Yes, 10x, and the clip says so. The run took 6 min 7 s. It stopped at Uber's sign-in page; nothing was ordered. [DEMO: update for the new recording.] | "Real time". |
| 7 | "AGPL kills it for my company." | AGPL-3.0-only. It applies when you distribute it or run a modified copy as a network service. Chosen so a competitor cannot lift the code into a closed project (D13). Dependencies are permissive; scrcpy files under Apache-2.0 are in `NOTICE`. | A legal opinion on a company's use. [USER: your position.] |
| 8 | "Telemetry?" | None. No install id, no event, no crash upload (D9). A test runs the CLI, the daemon and the MCP server under macOS `sandbox-exec` with every non-local connection killed (`cargo test -p phonedriver --test network`), and it passes. Limits: it runs on macOS only, with a fake adb server and no device; Apple is contacted only to mount an iOS developer disk image, which that test does not cover. | "Provably private" on Linux or Windows. |
| 9 | "Does it work on a real phone?" | iOS: yes, on an iPhone 14 Plus (iOS 26.1) and an iPad Pro (iOS 27.0), through the CLI and MCP, tap 454 to 534 ms. Android hardware: same code path as the emulator, not tested on one. | That Android hardware is verified. |
| 10 | "Tokens: are you cheaper?" | Than phone-harness, in three tasks, by 34% (218,357 against 330,375), with the exceptions above. Raw adb was cheaper on the one task it finished. One to three runs per cell. Snapshots are 183 to 822 tokens on measured screens. | A general token claim. |
| 11 | "Looks vibe-coded." | [USER: your answer.] Repo facts: plans, a decision log, 21 Android and 12 iOS scenario suites with raw data, benchmarks, audits. The first agent-eval pass scored 9 of 15 and all six failures were bugs in my own checks. | Anything that reads as defensive. |
| 12 | "Why a daemon?" | A resident daemon holds the device connection and the ref table, so each call costs milliseconds and refs survive a shifted list (D1, D11). | |
| 13 | "Same name as another repo?" | The project was renamed from phone-use to phonedriver before launch (names are free on npm, crates.io and PyPI). | |

## After the first two hours

- Do not delete and repost. If the post stalls, email hn@ycombinator.com and ask for the second-chance pool.
- If someone finds a real bug, say so, open an issue, give a time.
- If you are asked "is this AI-written", answer plainly: the code was written with coding agents and reviewed by you, and the words in your comments are yours. [USER: say what is true.]
- Record the points, rank and the open questions in `progress.md` at minute 120 and next morning.
