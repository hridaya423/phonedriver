# Reddit plan (round 4, rules read 2026-11-07)

Nothing here is posted. The rules below are the sidebar rules of 15 subreddits, read on 2026-11-07 in a real browser. They are quoted from `reddit-rules-2026-11-07.md`, which replaces the secondhand rules of round 3. Rules change, so each subreddit has a one-line checklist to run on the day you post. Every number is in `claims-ledger.md`. Slots: `[DEMO]`, `[USER]`.

## What you must do on every post

- You write the text. Three of the postable subreddits ban or remove AI-written text (r/mcp, r/opensource, r/rust). Those posts are **FACT NOTES**: type them in your own words and do not paste a sentence. The others have no such rule, but a post that reads machine-written is downvoted anyway, so retype all of them.
- Say you are the author in the first line. Say plainly that coding agents wrote part of the code and you reviewed it (`AGENTS.md`, `progress.md`).
- Never post the same text to two subreddits. Never post to two subreddits on one day. Never use the cross-post button.
- Never ask for votes, anywhere.
- The demo is sped up 10x. Say so in the first lines.
- Reply to every comment for two hours, then twice more that day. Concede true limits at once.
- Put the first comment up the minute the post is up. Only moderators can sticky a comment.
- If a moderator removes a post, ask once, politely, which rule it broke. Do not repost.

## Decisions, ordered by fit and reach

Reach is the sidebar counter: weekly visitors, then weekly contributions. Days assume T0 = Monday 2026-11-23, as in `plan.md`.

| # | Subreddit (visitors / contributions) | Decision | Deciding rule | Flair or tag | Day |
|---|---|---|---|---|---|
| 1 | r/ClaudeAI (2M / 31K) | Post | Rule 7: promoting your project is encouraged if "project was built with Claude/Claude Code or specifically for Claude BY YOU", with "a clear description of what was built, how Claude helped in detail, and what it does", "free to try and say so", "promotional language minimal", no referral links. "Posts on the feed now require OP karma>100." | The flair closest to a project or showcase (rule 9). Copy the exact name from the picker. | T+1 Tue 11-24, 14:00 UTC |
| 2 | r/ClaudeCode (1.2M / 26K) | Post only in the weekly showcase thread | Rule 5: "Simple project sharing belongs in the weekly showcase thread. Standalone Built with Claude Code posts must be detailed and explain what you built, how Claude Code was used, and what you learned." Rule 6 names weekly showcases as pinned megathreads. | Not a post, so none. If you later write a standalone post, the "Built with" flair (rule 2). | T+4 Fri 11-27, 14:00 UTC |
| 3 | r/mcp (48K / 1.9K) | Post, FACT NOTES | Rule 2: "No AI generated slop... will result in a ban." Rule 3: "Self-promotion is allowed with proper disclosure." Rule 4: "use showcase tag to indicate authorship." Rule 1 allows it because the project is fully launched. | `showcase` | T+2 Wed 11-25, 14:00 UTC |
| 4 | r/AI_Agents (290K / 8.8K) | Post only in the weekly project display thread | Rule 3: "Put your links in the comments, not the posts... If you have a project you want to show off, link it in the weekly project display thread." Rule 4: about one in ten posts or comments promotional. | None for a comment. | T+7 Mon 11-30, 14:00 UTC |
| 5 | r/SideProject (348K / 20K) | Post, after you read its wiki and pinned posts | The sidebar has no rules widget. Open the wiki and the pinned posts first and follow them over anything below. | Check the picker; use the project-share flair if one exists. | T+6 Sun 11-29, 16:00 UTC |
| 6 | r/iOSProgramming (85K / 982) | Post only on a Saturday, only if the wiki says a developer tool counts, otherwise skip | Rule 7: "Only post your app on Saturday. You may post about one app, once a year." Rule 4: free products get a lenient policy. Rule 5: the post must be about Apple platform development. The slot is used once a year, so spend it only when the wiki page `app-saturday` allows a tool. Rule 7 also asks for "previous activity" in the sub. | Check the picker. | T+5 Sat 11-28, 15:00 UTC |
| 7 | r/rust (153K / 2.3K) | Post only in "What's Everyone Working on this Week?", FACT NOTES | Rule 7: "Applications who just happen to be written in Rust are not on-topic... Casual reports of the joys and pains of working with Rust, or progress on Rust projects, should go in the What's Everyone Working on this Week? thread." Rule 6: "Submissions appearing to contain AI-generated content may be removed." | None for a comment. | T+8 Tue 12-01, 14:00 UTC (or the first day the new weekly thread is up) |
| 8 | r/opensource (147K / 1.8K) | Post last, FACT NOTES, only after you read its wiki and pinned posts | Rule 3: "All AI-generated content is low-effort and ban worthy." Rule 4: the repo "MUST have a LICENSE file that MUST be an OSI listed" license (AGPL-3.0 is). Rule 2: do not use the sub as a link farm. Rule 6: no drive-by posting. Round 3 said the sub barred AI-related projects; the sidebar does not say that, so check the wiki. If the wiki bars them, skip. | `Promotional` (rule 8) | T+9 Wed 12-02, 14:00 UTC |
| - | r/LocalLLaMA (909K / 24K) | Do not post | Rule 3: "Completely/primarily LLM generated copy, code is not allowed." The repo has no local-model result, and a launch cannot claim one. Rule 4 also limits self-promotion to about 10% of your content. | | |
| - | r/programming (214K / 1.6K) | Do not post | Rule 5: "No Product Promotion/'I Made This' Project Demo Posts... just a link to a GitHub page or a list of features is not [allowed]." Rule 1: "No LLM-Written Content." Rule 2: AI content is off-topic "unless they are deeply technical." | | |
| - | r/androiddev (60K / 1.2K) | Do not post | Rule 2: "Sharing applications or recruiting testers is not allowed." Rule 5: "Posts and comments must be written in your own voice." Round 3 planned a showcase-thread comment; the sidebar names no such thread. | | |
| - | r/commandline (33K / 422) | Do not post | Rule 6: "We do not allow projects or software that interacts with generative AIs." Rule 5 also bars projects younger than 30 days. Rule 4 bars AI-generated post text. | | |
| - | r/cursor (72K / 1.6K) | Do not post | Rule 1 and rule 6: content must relate to Cursor. The Cursor config is documented and untested (`ledger`), and rule 4 asks for claims that are "accurate and verifiable". Rule 11 bans AI-generated posts. Revisit after you test Cursor. | | |
| - | r/vibecoding (357K / 6.8K) | Do not post | Rule 2: "Vibe coding dev tools must be approved" through the Vibe Coding community on X, then one post. Rule 3 wants a build story. Not worth the approval step for a tool that is not a vibe-coding product. | | |
| - | r/selfhosted (541K / 7.6K) | Do not post | Rule 1: "All posts must be about self-hosting." Rule 6: projects under 3 months go only in the New Project Megathread. phonedriver has no self-hosted backend. | | |

Spacing: one subreddit per day. The longest gap is T+3 Thu 11-26 (US Thanksgiving) with no Reddit at all. r/ClaudeAI and r/ClaudeCode share readers, so they sit three days apart with different text.

## Pre-post checklist (run on the day, one line each)

- **r/ClaudeAI:** reread the sidebar today; confirm your karma is above 100 (rule 7); pick the project flair from the picker; the post says it is free to try and how Claude helped; no referral link; no other subreddit posted today.
- **r/ClaudeCode:** reread the sidebar; find this week's pinned showcase thread; comment there, not a new post; confirm the text differs from r/ClaudeAI; no other subreddit posted today.
- **r/mcp:** reread the sidebar; set the `showcase` tag; the text is typed by you from the notes; the disclosure sentence is in; no other subreddit posted today.
- **r/AI_Agents:** reread the sidebar; find this week's project display thread; put the link in the comment, not a post; count your recent posts so this is not more than one in ten; no other subreddit posted today.
- **r/SideProject:** read the wiki and the pinned posts first; copy the flair; no other subreddit posted today.
- **r/iOSProgramming:** it is Saturday; read the `app-saturday` wiki; confirm a tool qualifies and that you have not posted an app there this year; you have prior activity in the sub; not within a few hours of midnight (rule 7); no other subreddit posted today.
- **r/rust:** reread the sidebar; find this week's "What's Everyone Working on this Week?" thread; the comment is yours, in your words, about the Rust work; no other subreddit posted today.
- **r/opensource:** read the wiki and pinned posts; the `Promotional` flair is set; the `LICENSE` file is on the default branch; you can stay for the replies; no other subreddit posted today.

The sidebar for r/ClaudeAI states a karma gate (over 100). No other sidebar states a karma or account-age minimum. Your history still matters: r/rust, r/opensource and r/iOSProgramming expect prior activity or engagement, so comment in them honestly for a week before.

## 1. r/ClaudeAI

Rule 7 asks you to say what Claude did. It is the only subreddit where "built with Claude" helps you. Keep the comparison with phone-harness out of this post: rule 6 wants a source and no presumption that one tool is better, and a speed number against adb is not a competitor claim about a model.

**Title:**

```
I built an MCP server that lets Claude Code use an Android or iOS phone, and it has to ask me before it taps Pay
```

**Body** (attach the clip first; the first two lines say what it is):

```
I am the author. phonedriver is an open-source MCP server and CLI that lets Claude Code see and control Android and iOS phones. It is free to try (AGPL-3.0, no telemetry).

The clip is Claude Code on an Android emulator, ordering butter chicken and garlic naan on Uber Eats, sped up 10x (the run took 6 min 7 s). It stopped at Uber's sign-in page, so nothing was ordered. [DEMO: update for the new recording and the approval prompt.]

How Claude helped: coding agents wrote part of the code, and I reviewed it. [USER: say in detail what Claude Code did, which parts, what you had to correct, and what you wrote yourself. Rule 7 asks for this in detail. Facts you can use: the repo has plans, a decision log and a test-first rule in AGENTS.md; the first agent-eval pass scored 9 of 15 and all six failures were bugs in my own checks.]

What it does:
- The screen is numbered text. Every tap returns only the lines that changed, so Claude does not list the screen again.
- Before it taps Pay or Place order, phonedriver asks you through an MCP elicitation (Allow, Allow for this session, Decline). The model cannot answer that prompt itself.
- Android needs nothing installed on the phone. iOS works on the Simulator and on an iPhone 14 Plus and an iPad Pro I tested.

Speed, Android 17 emulator, p50 of 50 runs: reading a 1,000-row screen takes 24 ms against 2,352 ms for adb.

Install: claude mcp add phonedriver -- phonedriver mcp
Code: github.com/hridaya423/phonedriver

Limits: I measured no physical Android phone, iOS needs a Mac with Xcode, and the guard only matches English labels. Which app should I try it on?
```

**First comment:**

```
Limits, since people ask: the speed numbers are emulator only. A tap on an iPhone takes 454 to 534 ms. The approval guard matches a fixed list of English labels, so an icon-only button is not guarded, and shell input is not seen by it. Method and raw data: github.com/hridaya423/phonedriver/blob/main/docs/bench/speed-2026-11-07.md
```

Likely questions: "Is it safe unattended?" (Not without the guard. Headless `claude -p` has nobody to ask, so the tap is not made; D27.) "Does Apple allow this?" (It uses an XCTest runner on your own Mac and devices. Say nothing about Apple's policy.) "Sped up?" (Yes, 10x, said in the post.)

## 2. r/ClaudeCode (weekly showcase thread)

Rule 5 sends a simple project share to the weekly thread. A short comment is the right size. If you later want a standalone post, rule 5 asks for what you built, how Claude Code was used and what you learned; do that as a separate post at least a week later, with new text.

**Comment** (typed by you; this is a base):

```
I built phonedriver, an open-source MCP server and CLI that lets Claude Code drive an Android or iOS phone. I am the author, and coding agents wrote part of it. The screen is numbered text and every tap returns only the lines that changed. Before it taps Pay or Place order it asks you in the Claude Code prompt, and the model cannot answer for you. Clip, sped up 10x: [DEMO]. Limit: the guard only reads English labels.
claude mcp add phonedriver -- phonedriver mcp
github.com/hridaya423/phonedriver
```

Story material for a later standalone post (`ledger`, demo content row): the agent rejected the cookie banner, set the Times Square address, searched for Indian food, mis-tapped into an Uber One page and went back, found Patiala Indian Grill and Bar, added Butter Chicken ($20.95) and Garlic Naan ($7.95), subtotal $28.90, and stopped at the sign-in page.

## 3. r/mcp: FACT NOTES (type this yourself)

Rule 2 bans AI-generated posts and bans the account. Do not paste any sentence below. Write the post in your own words from these facts. Tag: `showcase`. Open with that you are the author and that coding agents helped write the code.

Title idea (rewrite it): an MCP server for Android and iOS where every action returns a diff of the screen.

Facts:
- A resident daemon holds the device connection. The MCP server is a thin stdio client, so a call costs milliseconds (D1).
- Every action waits for the screen to settle and returns only the changed lines, so the model does not call a snapshot tool after each tap (D10, D12).
- 14 tools, about 51 tokens each in Claude Code's wrapper. Snapshots are 183 to 822 tokens on the screens measured (`ledger`).
- A human-approval guard on taps such as Pay and Place order, through MCP elicitation. Clients that cannot ask get a plain reply telling the agent to ask in chat. Limits: English labels only, `shell` input not seen, headless `claude -p` declines (D27).
- `--secret` text never reaches logs, traces or replies. A test runs the server under a macOS sandbox that kills any non-local connection (`crates/cli/tests/network.rs`).
- Speed, Android 17 emulator, p50 of 50 runs: a 1,000-row screen read in 24.0 ms against 2,352.4 ms for adb; tap then see what changed 4.6x. Emulator only.
- Tested with Claude Code. The Cursor config is documented and untested.
- Install line and repo URL.

Ask to end on: which MCP clients it does not work with.

**First comment** (your words): the limits, the benchmark method link (`docs/bench/speed-2026-11-07.md`), and the disclosure sentence again.

## 4. r/AI_Agents (weekly project display thread)

Rule 3: the link goes in a comment, in that thread. Rule 5: give context, not only a link.

**Comment** (base; retype):

```
I built phonedriver, an open-source MCP server and CLI (AGPL-3.0) that lets an agent see and control Android and iOS phones. I am the author; coding agents wrote part of the code and I reviewed it. The agent reads the screen as numbered text, and every action returns only what changed, so it does not poll. A guard asks the human before taps like Pay. Speed numbers are emulator only: a 1,000-row screen read takes 24 ms against 2,352 ms for adb. I measured no physical Android phone. github.com/hridaya423/phonedriver
```

## 5. r/SideProject

The sidebar has no rules. Read the wiki and the pinned posts and follow them over this file. This sub wants the story, so I will not write it: I do not have it. [USER: why you built it, what you built it with, what surprised you.]

Facts that make the story concrete: the first agent-eval pass scored 9 of 15, and all six failures were bugs in your own checks (`docs/bench/m3.md`); the repo has plans, a decision log and benchmarks with raw data. Add the repo and the clip, say it is free and open source, and state the same limits as everywhere. Skip this subreddit if the story does not feel like yours to tell.

## 6. r/iOSProgramming (Saturday only)

You get one app post a year here. Use it on the iOS half of the project, and only if the `app-saturday` wiki allows a developer tool. Rule 5 needs the post to be about Apple platform development, so lead with the XCTest runner, not with the agent.

Facts: the iOS side is our own XCTest runner, not WebDriverAgent (D4). A tap takes 357 ms on the Simulator and 534 ms on an iPhone 14 Plus, because XCTest takes about 240 ms to confirm an event (`docs/bench/m4.md`, `m5.md`). Physical devices need your Apple signing team, and a free Apple ID expires every 7 days (D15). iPhone 14 Plus (iOS 26.1) and iPad Pro (iOS 27.0) were tested through the CLI and MCP; the 12-scenario suite was not run on devices. Ask: has anyone found a faster way to inject taps through XCTest. Free, so rule 4 is lenient; still say what it does, the hard parts, and the limits. Write it yourself.

## 7. r/rust: FACT NOTES (weekly thread comment)

Rule 7 makes the Rust side the topic, not the product. Write two or three sentences about the work. Facts: one Rust binary is the CLI, the daemon and the MCP server (D1); length-prefixed JSON on internal wires; the adb protocol is spoken directly (D7, D11); one `unsafe` module, for a Windows handle list (`docs/bench/m1.md`). Say you are the author and that coding agents wrote part of the code. Do not promote the product; give the repo once.

## 8. r/opensource: FACT NOTES

Last and optional. Read the wiki first; if it bars AI-related projects, skip. Flair: `Promotional`. Type the post yourself (rule 3 bans AI-generated content). Facts for it: the license is AGPL-3.0-only and applies when you distribute it or run a modified copy as a network service; the reason is to stop a competitor lifting the code into a closed project (D13); no telemetry (D9); a test that proves no outside connection; plans, decisions and raw benchmark data are in the repo. Say what you want back: results on a physical Android phone, and apps where the snapshot is empty. Stay for the replies (rule 6).

## After posting

- Record score, comments and referrals (GitHub Insights, Traffic) per post. Export traffic on T+7.
- A removal in one subreddit is a signal for the next. Reread the rule that removed it and change the plan.
