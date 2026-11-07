# Reddit plan (round 3)

Nothing here is posted. **I could not read any subreddit's rules: Reddit returned 403 for every rules page, so every rule below is secondhand (mirrors, news, search snippets).** Before each post, open the sidebar and the rules page in a browser, copy the exact flair name, and check the karma gate. Write each post yourself; the drafts fix the facts and the shape, and a post that reads machine-written is the most common reason a launch post dies on Reddit. Slots: `[DEMO]`, `[JOBS]`, `[USER]`. Every number is in `claims-ledger.md`.

## Rules for the whole run

- One subreddit per day, different text each time. Never cross-post with the button. Reddit filters the same link across subreddits on a new account [anecdotal].
- First line of every post: you are the author ("I built this"). Say coding agents wrote part of it, in one plain sentence, in the body. Hiding it is the thing that gets projects removed and downvoted.
- Do not post to r/opensource (it barred AI-related projects and MCP tools in late October 2026, and wants a project at least a year old with 3 or more contributors) or r/programming (an LLM-content ban was in trial; comment-only culture). Do not ask the moderators for an exception.
- Never ask for upvotes, in the post, in comments, in DMs, anywhere else. Never link the post from X, HN or Discord with "go upvote".
- Check your own history: an account with no comments will be filtered. Spend the days before launch answering questions in these subs. [USER: account age and karma per sub.]
- Native media: upload the clip yourself. Reddit video and GIF upload is how a demo gets seen; a YouTube link in a text post is skipped. The demo is 10x sped up: say so in the first lines, because the top objection to the closest comparable post (a Claude iPhone MCP with 180 points) was a sped-up demo.
- Pinning: only moderators can sticky a comment on Reddit as far as I know [UNVERIFIED]. So "the comment to pin" below is the **first comment**, posted by you the minute the post is up. Everything a reader needs also sits in the body.
- Answer every comment for two hours, then twice more that day. Concede true limits at once.

## Which subs, in order (days are in `plan.md`)

| Order | Subreddit | Fit | Rule status | Format |
|---|---|---|---|---|
| 1 | r/ClaudeAI | Best audience: Claude Code users. | Showcase needs total karma of at least 50 on the main feed, or goes in the "Built with Claude" megathread; "Rule 7" holds the showcase requirements and a project flair (mirror, 2026-04-15) [UNVERIFIED text]. | Text post, project flair, native video or inline GIF. |
| 2 | r/mcp | MCP builders and users. | Not read [UNVERIFIED]. Earlier phone-MCP posts there scored 7 to 28. | Text post. |
| 3 | r/ClaudeCode | Heavy Claude Code users. | Not read [UNVERIFIED]. | Text post, a different angle. |
| 4 | r/androiddev | Android developers. | Old rules: promotion in a Showcase thread only (2023) [UNVERIFIED]. | A comment in the current showcase or self-promo thread. |
| 5 | r/iOSProgramming | iOS developers. | Old pattern: promotion limited to a weekly thread [UNVERIFIED]. | A comment in that thread, if one exists and iOS is the point. |
| 6 | r/SideProject | General makers. | Not read [UNVERIFIED]. Low developer density. | Text post, story angle. Optional. |
| Hold | r/LocalLLaMA | Largest phone-agent audience, but wants local models. | New rules, karma gates, self-promotion as the main abuse area [anecdotal]. | Only with a tested local-model result, which the repo does not have. Skeleton below. |
| Never | r/opensource, r/programming, r/MachineLearning, r/Android, r/ios | See above. | | |

## 1. r/ClaudeAI

**Title** (a concrete first-person title that names the tool they use beat the generic ones in round 2):

```
I built an MCP server that lets Claude Code use an Android or iOS phone, and it has to ask me before it taps Pay
```

(113 characters; Reddit allows 300.) Flair: the project or showcase flair the sub shows. [USER: copy the exact name.]

**Body** (attach the clip first; say what it is in the first two lines):

```
I'm the author. phonedriver is an open-source MCP server and CLI that lets Claude Code see and control Android and iOS phones. Coding agents wrote part of the code; I reviewed it and the benchmarks are my own runs. [USER: say it your way.]

The clip is Claude Code on an Android emulator, ordering butter chicken and garlic naan on Uber Eats, sped up 10x (the run took 6 min 7 s). It stopped at Uber's sign-in page. Nothing was ordered. [DEMO: update for the new recording and the approval prompt.]

What is different:
- the screen is numbered text, and every tap returns only the lines that changed, so Claude does not poll or re-list the screen
- before it taps Pay or Place order, phonedriver asks you through an MCP elicitation (Allow, Allow for this session, Decline). The model can't answer that prompt itself
- Android needs nothing installed on the phone; iOS works on the Simulator and on an iPhone 14 Plus and iPad Pro I tested

Numbers (Android 17 emulator, p50 of 50 runs): reading a 1,000-row screen takes 24 ms against 2,352 ms for adb (98x). Across three agent tasks with Sonnet 5.5 it used 218,357 tokens against 330,375 for phone-harness, 34% fewer, though it lost one task.

Install: claude mcp add phonedriver -- phonedriver mcp
Code (AGPL-3.0, no telemetry): github.com/hridaya423/phonedriver

What it can't do yet: no Android phone measured, iOS needs a Mac with Xcode, and the guard only matches English labels. Which app should I try it on?
```

**First comment** (post it at once):

```
Limits, since people ask: speed numbers are emulator only. A tap on an iPhone takes 454 to 534 ms (XCTest takes about 240 ms to confirm an event). The approval guard matches a fixed list of English labels, so an icon-only button is not guarded, and `shell` input is not seen by it. Method and raw data: github.com/hridaya423/phonedriver/blob/main/docs/bench/speed-2026-11-07.md. Design note on the guard: phonedriver.dev/blog/letting-claude-order-dinner-without-letting-it-spend-your-money/
```

Likely comments and true answers: "Is it safe to run unattended?" (not without the guard on; headless `claude -p` has nobody to ask so it declines; D27). "Does Apple allow this?" (it uses the XCTest runner on your own Mac and your own devices; I have not asked Apple. [USER: do not claim anything about Apple's policy]). "Why not mobile-mcp?" (it also does Android and iOS; it has no wait after actions in its source, so the agent re-lists; I did not benchmark its speed). "Sped up?" (yes, 10x, said in the post).

## 2. r/mcp

**Title:**

```
phonedriver: an MCP server for Android and iOS where every action returns a diff of the screen (14 tools, Rust, no telemetry)
```

**Body:**

```
I built this (coding agents helped write the code). phonedriver is an open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones.

Design points that matter for an MCP server:
- a resident daemon holds the device connection; the MCP server is a thin stdio client, so a call costs milliseconds
- every action waits for the screen to settle and returns only the lines that changed, so the model does not call a snapshot tool after every tap
- 14 tools, about 51 tokens each in Claude Code's wrapper. Snapshots were 183 to 822 tokens on the screens I measured
- a human-approval guard on taps like Pay and Place order, through MCP elicitation. On protocol 2026-07-28 the server returns an input_required result and the client calls again with the answer; older protocols get elicitation/create. Clients that cannot ask get a plain reply telling the agent to ask in chat
- `--secret` text never reaches logs, traces or replies; a test runs the server under a macOS sandbox that kills any non-local connection

Speed on an Android 17 emulator, p50 of 50 runs: a 1,000-row screen read in 24 ms against 2,352 ms for adb; tap then see what changed 4.6x. Emulator only.

claude mcp add phonedriver -- phonedriver mcp
github.com/hridaya423/phonedriver

I would like to hear which MCP clients it does not work with. I tested Claude Code. The Cursor config is documented and untested.
```

(Facts: README "Use with other MCP clients"; the site's Cursor page says untested; D27; `ledger`.) Flair: whatever the sub offers for projects. [USER]

## 3. r/ClaudeCode

Angle: what it is like to use, not the architecture. Do not post the same body as r/ClaudeAI.

**Title:**

```
Claude Code ordered dinner on my phone and stopped to ask before paying (MCP, open source)
```

Body: the video, then three short paragraphs: what I asked, what it did, what the prompt looked like [DEMO]; the install line; one honest limit (the guard only matches English labels); the repo. Tell the story of the session the clip shows, in your words. Facts available: the agent rejected the cookie banner, set the Times Square address, searched for Indian food, scrolled past a mis-tap into an Uber One page and went back, found Patiala Indian Grill and Bar (4.8 stars, 900+ reviews), added Butter Chicken ($20.95) and Garlic Naan ($7.95), subtotal $28.90, and stopped at the sign-in page (`progress.md`, README demo retake). That mis-tap is a real moment of the run and is more convincing than a clean story.

## 4. r/androiddev (a comment in the showcase thread, not a post)

Find the current thread and read its rules. One comment, written for developers, no AI hype:

```
I built phonedriver, an open-source CLI and MCP server (Rust, AGPL-3.0) that lets coding agents drive a phone or emulator through UiAutomation. It pushes a dex jar and starts it with app_process as the shell user, as scrcpy does, so nothing is installed on the phone and no accessibility service is needed. Reading a 1,000-row list takes 24 ms against 2,352 ms for uiautomator dump on an API 37 emulator (p50 of 50). Emulator only: I have not measured a physical phone. Looking for people to try it on hardware. github.com/hridaya423/phonedriver
```

Facts: D2, speed doc. Expect "Anti-AI sentiment" (round 2's 0-point post in this sub). Do not argue. If the sub bans AI-agent tools, drop it.

## 5. r/iOSProgramming (only if the sub's rules allow a tool comment or post)

The angle is the iOS half: **our own XCTest runner, not WebDriverAgent**; a tap on the Simulator takes 357 ms and on an iPhone 14 Plus 534 ms because XCTest takes about 240 ms to confirm an event; physical devices need your Apple signing team (a free Apple ID expires every 7 days). Ask: has anyone found a faster way to inject taps through XCTest. Facts: D4, D15, `docs/bench/m4.md`, `m5.md`. [USER: skip this if the sub has no tooling thread.]

## 6. r/SideProject (optional)

A story post: why you built it, what you built it with, what surprised you. [USER: your words, your real story. I will not draft the backstory; I do not have it.] Facts that make it concrete: the first agent-eval pass scored 9 of 15 and all six failures were bugs in my own checks (`docs/bench/m3.md`); the repo has plans, a decision log, and benchmarks with raw data. Add the repo and the video. Skip if the day is crowded.

## Hold: r/LocalLLaMA (skeleton for when a local-model result exists)

The repo has no local-model run. The launch cannot claim "works with local models": README says any stdio MCP client works and nothing more. If you run a local model through a client first, the post is:

- Title: `[name of model] drives an Android phone through an open-source MCP server, tool calls and diffs inside N tokens`
- Body: the model, the client, the task, the result, the failure cases, the token counts. [LOCAL: all of it.]

Without that result, skip: the sub's most common reply is "why not a local model".

## After posting

- Reply to every comment. A comment from you in the first hour helps the post; a deleted or silent author hurts it.
- If a mod removes the post, ask once, politely, what rule it broke. Do not repost.
- Record upvotes, comments and referrals (GitHub Insights, Traffic) per post.
