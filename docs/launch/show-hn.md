# Show HN draft

Nothing here has been posted. Slots in `[TBD: ...]` wait on a measurement or a build. Slots in `[USER: ...]` need the author's own words or a decision. `claims-ledger.md` names the repo file behind every claim.

> **Retype this by hand before posting.** HN's guidelines say "Don't post generated text or AI-edited text", and dang's Show HN tips say not to use an LLM for any of it, "not even a tiny bit" (see `research.md`). This draft is a fact sheet in a usable order. Keep the facts and numbers. Put every sentence in your own words. Do the same for replies in the thread.

## Decisions for the author

1. **AI-assisted code.** `AGENTS.md` and `progress.md` are public in the repo and show that coding agents did part of the work. HN's rules say "Don't post quickly-generated one-offs." Recommendation: say it in one plain sentence in the first comment, and let the documented plans, tests and benchmarks answer the rest. The text needs your true statement: `[USER: how much of the code and the tests you wrote, and how you reviewed the rest]`.
2. **The name.** `github.com/sauravtom/phone-use` (MIT, 6 stars) has the same name and a similar description. Decide before launch whether to keep the name. Prepared answer 11 assumes you keep it.
3. **Account.** Post from your personal HN account, not a project account, and put an email in the profile so moderators can send a repost invite.

## Title options

HN titles are limited to 80 characters. Each option starts with "Show HN: phone-use – " as the research recommends: a name, then what it does.

| # | Title | Length |
|---|---|---|
| 1 | Show HN: phone-use – Agents read a phone as text and get only what changed | 74 |
| 2 | Show HN: phone-use – Android and iOS control for agents, with text diffs | 72 |
| 3 | Show HN: phone-use – A Rust CLI and MCP server for agents to drive phones | 73 |

**Recommended: option 1.** It says what the tool does and names the one idea that separates it from the others (each action returns only the change). It has no number, so nobody can call it a gratuitous number, and no adjective, so a moderator has nothing to cut. Option 2 reads as a category label. Option 3 says the implementation and not the idea.

Considered and dropped: any title with "fastest", "12x" or "insane". The benchmark is an emulator on a busy machine, and a title claim invites the fairness fight before the reader sees the caveats.

## URL to submit

`https://github.com/hridaya423/phone-use`

Submit the URL and the text below together. HN shows the text as the first comment from you. If it does not appear, post it as a comment yourself. Do not submit before the repo is public and the install line in the README works from a clean machine (`plan.md`).

## First comment (draft)

Hi HN, I am [USER: your name]. phone-use gives an AI agent a text view of an Android or iOS screen and lets it tap, type and swipe. It is one Rust binary: a CLI, a daemon and an MCP server. Every action returns only the lines that changed.

Why I built it: [USER: one true paragraph in your words. Say what you tried first and what failed. Facts you can use: you read the phone-harness source before you started, an `adb shell input tap` costs one process start per call, and `uiautomator dump` took 2.8 s on the test emulator.]

How it works:

1. A daemon keeps the device connection open. On Android it pushes a small dex jar and starts it with `app_process` as the shell user, the way scrcpy does. Nothing is installed on the phone, and no accessibility service needs enabling. The agent reads the tree through UiAutomation and injects input through InputManager.
2. Every action waits for the screen to settle, then returns the diff. Settle means accessibility events and video frames have gone quiet. The agent never sleeps or polls. A tap on a switch returns `~ @e41 switch "Dark Appearance" off → on`.
3. Refs such as `@e10` are found again at tap time. A list that moved by one row still gets the right tap.
4. On iOS, the Simulator and physical devices use our own XCTest runner, not WebDriverAgent. An experimental tier on iOS 27 sends touches through CoreDevice. [TBD: iOS hybrid tier status at launch]
5. The agent calls typed verbs. There is no shell tool and no code execution. [TBD: shell tool; if it ships before launch, rewrite this point and answer 5]

Numbers, from `docs/bench/adb.md`: an Android 17 emulator on a busy machine, median of 50 runs, phone-use and adb measured back to back. A screenshot takes 45 ms against 692 ms with `adb exec-out screencap`. Reading the Settings screen takes 237 ms against 2,800 ms with `uiautomator dump`. "Tap, then know what changed" takes 667 ms against 3,112 ms. A bare tap is only 1.6 times faster. [TBD: re-run on a quiet machine before posting]

Does an agent finish tasks with it? Claude Sonnet 5.5 completed 15 of 15 held-out runs on five Android tasks, with a median of 6 tool calls and 18 s. The first pass scored 9 of 15. I read all six failures: each was a bug in my check or my device reset, not in the tool. I fixed the checks and reran. Details are in `docs/bench/m3.md`. [TBD: token benchmark against mobile-mcp, phone-harness and raw adb]

What does not work yet:

- A tap misses its latency budget. On the emulator it is 130 to 144 ms on a busy machine (30 to 37 ms on a quiet one) against a 15 ms target. On the iOS Simulator it is 357 ms. On an iPhone 14 Plus and an iPad Pro it is 534 and 454 ms, because XCTest takes about 240 ms to confirm an event. [TBD: final tap latency after the latest performance changes]
- No physical Android phone has been measured. Android numbers are emulator only.
- iOS needs a Mac with Xcode. A physical iPhone also needs an Apple signing team. A free Apple ID works and expires every 7 days.
- The accessibility tree can be empty for WebViews and custom-drawn views. phone-use then reads the screen with OCR, which has no checked state for a web checkbox.
- One iOS scenario, `settings-dark-appearance`, failed twice in six full runs, and I have not found the cause.

License: AGPL-3.0-only. There is no telemetry. The tool talks to your device and nothing else.

Feedback I want: [USER: pick two. Suggestions: results on a real Android phone, apps where the snapshot comes back empty or wrong, whether the diff format reads well to your agent.]

Install: [TBD: confirm the one-line install and `brew install hridaya423/tap/phone-use` work on a clean machine at launch]

[USER: one sentence on AI-assisted development, per decision 1.]

## Prepared answers

Use these as facts to reply from. Retype each one. Answer within 15 minutes for the first two hours.

**1. "AGPL? I cannot use that."**
The license applies when you distribute phone-use or run a modified copy as a network service. Running the unmodified tool, so an agent can call it, asks nothing of you (README, License section). I chose AGPL-3.0-only so a competing product cannot take the code into a closed project (D13). Every dependency is permissive. The Android agent includes files copied from scrcpy under Apache-2.0, listed in `NOTICE`. `[USER: confirm you stand behind the "asks nothing of you" reading, and say if you would offer other terms]`

**2. "Why not just use adb?"**
You can, and the benchmark uses adb as the baseline. Each `adb shell input` call starts a process, and `uiautomator dump` waits for the UI to go idle, so reading one screen took 2.8 s. adb also cannot tell you what a tap changed: the benchmark builds that from a tap, a dump and a diff in the caller, at 3.1 s. `input text` fails on Unicode, and phone-use typed mixed-script text in the same benchmark. `docs/bench/adb.md` lists the method and the caveats.

**3. "Why not Appium or Maestro?"**
Appium and Maestro are test frameworks. Their unit is a script that a person writes once and runs many times. phone-use's unit is one action that an agent chooses after it sees the screen, so each reply carries the diff. Appium needs an installed UiAutomator2 server and a driver stack. Maestro's MCP says physical iOS devices are not yet supported. `[USER: re-open both pages before you quote them; the repo's notes in docs/research/competitors.md came from a summarizing fetch]`

**4. "Why not mobile-mcp?"**
mobile-mcp is the most used MCP in this space, and it supports real iOS devices and cloud. Two differences from our reading of its source: it has no wait after an action, so the agent calls `list_elements` again to learn what happened, and it prints one line per element with coordinates and size. phone-use waits for settle and returns the diff. I have not measured mobile-mcp's tokens yet. [TBD: token benchmark] `[USER: re-open the mobile-mcp source before quoting it]`

**5. "An agent with a phone is a security problem. Does it run shell?"**
No. The agent calls typed verbs such as `tap`, `type` and `swipe`. There is no shell tool and no way to run agent-written code (D6). [TBD: shell tool; if it ships, say what gates it] The daemon listens on a Unix socket in a directory only your user can open, and nowhere else. Text sent with `--secret` is kept out of logs, traces and replies. Be clear about the limit: the agent can do anything a person can do on the screen, including tap Pay. Use an account and a device you can afford to lose. `[USER: confirm whether any defence against prompt injection from on-screen text exists; the repo does not claim one]`

**6. "iOS only on a Mac? Of course."**
Yes. XCTest, the Simulator and the developer disk image need Xcode on a Mac. Android runs on macOS, Linux and Windows. You can also drive a device on another machine by forwarding the daemon socket over `ssh` (README, "Use a device on another machine").

**7. "Does it work on a real phone?"**
iOS: yes, on an iPhone 14 Plus (iOS 26.1) and an iPad Pro (iOS 27.0), through the CLI and MCP. A tap takes 454 to 534 ms there. I did not run the 12-scenario suite on those devices. Android: the physical-phone path is the same code as the emulator path, and nobody has tested it on hardware yet. That is why I am asking for reports. Wireless debugging works through `adb pair` and `adb connect`, but I tested it only on an emulator over TCP, not on a phone on a real network (`docs/bench/wifi.md`).

**8. "Telemetry?"**
None. There is no install id, no event and no crash upload (D9). I rejected opt-out telemetry because phone-harness ships the script, its output and every helper call to PostHog by default (`docs/research/competitor-teardown.md`). The only outside party is Apple, when you mount a developer disk image. The repo does not yet hold a network-silence test for phone-use itself. `[USER: either add one before launch or drop the "nothing else" claim]`

**9. "Your benchmark is rigged."**
Here is every choice that favors or hurts phone-use, in `docs/bench/adb.md`. It ran on a busy emulator host, so phone-use's own tap was slower than in the quiet run. The phone-harness numbers are from a quieter earlier run and run in process, which favors phone-harness. adb's tap returns before the app reacts and phone-use's waits for the app, so adb does less work and is still slower. The outputs differ: raw XML against pruned text. No physical Android phone is in it. The raw samples are in `docs/bench/data/`. Run it yourself with `phone-use-bench`.

**10. "What about tokens?"**
[TBD: token benchmark against mobile-mcp, phone-harness and raw adb on the same tasks, same model.] What I can say today: a snapshot is 183 to 822 tokens on the screens I measured (a character count divided by four, not a tokenizer), the 13 tool schemas cost about 2.6k tokens, and the 15 held-out runs used a median of 42,817 tokens including cache reads (`docs/audits/ax-audit.md`, `progress.md`, `docs/bench/m3.md`).

**11. "Is this another vibe-coded repo? Why the same name as another project?"**
`[USER: your own answer on how the code was written, per decision 1.]` The repo holds the plans, 21 Android and 12 iOS device scenarios, benchmarks with raw data, and the audits. On the name: another MIT project called phone-use exists (`sauravtom/phone-use`, Android only, 6 stars when checked). `[USER: your answer, for example that yours covers iOS and was named independently]`
