# Reddit plan

Nothing here has been posted, and no account has been created. The research could not read live Reddit pages (it used mirrors and news coverage), so read each subreddit's rules in a browser before posting. Slots: `[TBD: ...]` waits on a build or measurement. `[USER: ...]` needs your fact or decision. `claims-ledger.md` names the repo file behind each claim.

Reddit readers distrust AI-written posts and "vibecoded" projects (`research.md`, round 2). Retype each body in your own words. The drafts below are structured to be rewritten.

## How ranking and moderation shape the plan

| Rule | Consequence |
|---|---|
| Hot rank is `sign * log10(max(net votes, 1)) + age_in_seconds / 45000` (archived code; the live system may differ). Ten times the net votes equal 12.5 hours of age. | The first 10 net votes weigh as much as the next 90. Post when the subreddit's readers are awake, reply to every comment early, and do not ask for votes. |
| Vote manipulation and brand-new accounts that post repository links are filtered (secondary guides). Many subreddits copy the "10% self-promotion" norm. | Use an account with real history. Comment in each subreddit for a week before posting. `[USER: account age and karma]` |
| r/LocalLLaMA has minimum-karma AutoModerator gates (thresholds not retrieved). | Check the thresholds in the sidebar before launch. |
| r/ClaudeAI's mod bot warns about project posts without the project flair. | Use the flair. `[USER: the exact flair name from the sidebar]` |
| r/opensource bars projects that "use AI, are made by AI, or are intended for AI" (announced about 2026-10-24) and demands a year-old project and account. r/programming ran an LLM-content ban. | Do not post in r/opensource, r/programming, r/MachineLearning, r/Android or r/ios. A post in r/opensource risks a permanent ban. |
| Identical text across subreddits looks like spam. | Write four different posts and put a day between them. |

## What worked and failed in comparable posts

| Post | Result | Lesson |
|---|---|---|
| r/ClaudeAI, "Built a MCP server that lets Claude use your iPhone" | 180 points, 55 comments. Short body, a GitHub link, a linked video. | The top comments were about a sped-up demo, safety of unattended use, and speed. Our clip is real time. Say so. |
| r/LocalLLaMA, "[PokeClaw] First working app that uses Gemma 4 to autonomously control an Android phone. Fully on-device, no cloud." | 332 points, 173 comments. | "No cloud" and a named local model. Comments: safety, "vibecoded". |
| r/mcp, "Allowing Claude Code to interact with Android Devices/Emulators through mcp" | 28 points. A results table including a failure, plus a video. | Honest results get respect, and the questions were speed and tokens. |
| r/androiddev, "Control your Android phone with AI agents (Claude, Codex, Gemini, etc.)" | 0 points. | A generic title with a dead repo link. |

Title pattern in the better posts: first person and concrete, naming the reader's tool ("Built a ... that lets Claude ..."), with a qualifier such as "no cloud" or "no ADB needed".

## Order

| Day | Subreddit | Why here, why then |
|---|---|---|
| D2 (Wednesday), 14:00 UTC | r/ClaudeAI | Best audience (Claude Code users) and a close precedent at 180 points. By now the HN, X and LinkedIn results are known. D0 is the launch Monday in `plan.md`. |
| D3 (Thursday), 14:00 UTC | r/LocalLLaMA | Largest phone-agent audience, but only with a local-model angle. Wait until you can state one tested fact. |
| D4 (Friday), 14:00 UTC | r/rust | Rust substance (daemon, adb protocol client, video decode). Allowed if the post is about the engineering. |
| Next monthly Showcase thread | r/androiddev | Self-promotion is generally limited to the monthly Showcase thread. |

14:00 to 17:00 UTC on a Tuesday is the busiest developer-subreddit window in one vendor snippet (not opened). The timing evidence is weak. The reason for 14:00 UTC is that US morning readers are awake and you can reply for two hours.

Optional, low reach: r/ClaudeCode and r/mcp (read their rules first). Phone MCP posts in r/mcp scored 7 to 28 points.

Video: upload the re-recorded clip natively, or link it. The best-scoring comparable used a linked video. The research found no data on native against linked. [TBD: re-recorded clip] Put the repo link in the body (the successful posts did).

After posting: stay for two hours. Answer every comment. Do not edit the title. Do not ask for upvotes, and do not link a Reddit thread from X or HN.

## Variant 1: r/ClaudeAI

**Title (draft):** `Built an MCP server that lets Claude Code use an Android or iOS phone, and every tap returns only what changed`

**Flair:** `[USER: project flair]`

**Body structure:**

1. One sentence: what it is. Facts: a Rust CLI and MCP server, Android and iOS, `claude mcp add phone-use -- <path> mcp`.
2. The clip, with "real time, no speed-up" if true for the new recording. [TBD]
3. What Claude sees: a snapshot sample and a tap reply (README samples).
4. Two numbers: Settings screen read 237 ms against 2,800 ms for `uiautomator dump`; tap now about 34 ms on the emulator. [TBD: quiet-machine paired re-run] Say the setup: Android 17 emulator, busy host.
5. Safety, before they ask: the agent can do what you can do on screen. A `shell` tool exists; on a physical Android device it refuses wipes and unknown-app removal without `--force`; the guard is not a sandbox (D21). The daemon is local, with no telemetry (D9). Do not claim prompt-injection protection.
6. Limits: no physical Android measured, iOS needs a Mac with Xcode, token cost not the lowest (`docs/bench/tokens.md`: raw adb and phone-harness used fewer tokens in one run each). [TBD: rerun with shell]
7. AI disclosure: `[USER: one sentence on how the code was written]`.
8. Ask: which app or task should it try; send the trace if the snapshot is empty.
9. Link: `github.com/hridaya423/phone-use`, AGPL-3.0.

## Variant 2: r/LocalLLaMA

Use only when you can state a tested local-model fact. The README says any client that starts a stdio MCP server works, and nothing in the repo shows a local model driving it. `[USER: name one local-model client and model you ran, and the result. If none, skip this subreddit or run a test first.]`

**Title (draft):** `phone-use: a Rust MCP server so a local model can drive Android and iOS, with text diffs instead of screenshots`

**Body structure:**

1. The reader's concern first: no vision model needed for most screens. A snapshot is 183 to 822 tokens on the screens measured (`docs/audits/ax-audit.md`, characters divided by four). Screenshots are optional, and `screenshot` can mark `@eN` labels on the image for vision models (commit `3d52a9a`).
2. What a tap returns (README sample).
3. `[USER: the local-model result]`.
4. Numbers as in variant 1.
5. Honest limits: the evaluation used Claude Sonnet 5.5 (15 of 15 held-out runs, `docs/bench/m3.md`); small local models are untested here; WebViews can have an empty tree.
6. Privacy: no telemetry. The model endpoint is your choice.
7. AI disclosure and the ask, as in variant 1.

## Variant 3: r/rust

**Title (draft):** `phone-use: a Rust daemon that speaks the adb protocol directly and drives Android and iOS for AI agents`

**Body structure:** this audience wants the engineering, not the pitch.

1. One line on what it does. The code is one workspace: CLI, daemon, MCP server.
2. Technical points to choose from (D1, D2, D10, D11): a resident daemon with one open device connection per device; adb server protocol spoken directly; a dex agent over `app_process` with a UiAutomation connection; H.264 frames decoded on the host to detect when the screen settles; length-prefixed JSON on every internal wire; one `unsafe` module for a Windows handle list (`docs/bench/m1.md`).
3. A hard problem solved: `getChild` prefetch is cancelled by the next call unless `FLAG_PREFETCH_UNINTERRUPTIBLE` is also set, which cut a tree read from 78 to 103 ms down to 41 to 46 ms (`docs/bench/m1.md`).
4. Build: Rust 1.99, blaze and cargo; the build packs the Android agent and needs the Android SDK.
5. License and AI disclosure, as in variant 1.
6. Ask: review of the adb client and the video settle logic.

## Variant 4: r/androiddev Showcase thread

Reply in the monthly Showcase thread (find it by search; do not create a separate post). Keep it short and Android-specific.

**Comment structure:** what it is in one sentence; how it works on Android with no APK on the phone and no accessibility service to enable (D2); the numbers on the emulator and the missing physical-phone data; ask for reports from real phones and for apps where UiAutomation returns an empty tree; link; AI disclosure line.

## Likely comments and facts to answer with

| Comment | Facts |
|---|---|
| "The demo is sped up." | The new clip is real time. [TBD: confirm] |
| "Safety: it can do anything." | See variant 1, item 5. |
| "Why not mobile-mcp, Maestro, Appium?" | `show-hn.md`, answers 3 and 4. Re-read their pages first. |
| "Token cost?" | `show-hn.md`, answer 10. Say phone-use was not cheapest. |
| "Single maintainer, will it be abandoned?" | `[USER: your commitment]`. Do not promise what you will not do. |
| "AGPL?" | `show-hn.md`, answer 1. |
| "Is this vibecoded?" | `[USER]`, per `show-hn.md` decision 1. |
