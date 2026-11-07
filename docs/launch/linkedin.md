# LinkedIn plan (round 3)

Nothing here is posted. LinkedIn builds classifiers for generic AI text and reduces its reach (Entrepreneur, round 2), so retype each post in your own voice and keep the specifics. Slots: `[DEMO]`, `[USER]`. Jobs result, if you want it in a post: 18 of 18 for phonedriver at 977,780 tokens, 18 of 18 for mobile-mcp at 4,892,241, 14 of 18 for phone-harness at 1,272,617, adb 4 of 4 on the 2 mixed jobs; say where phone-harness is cheaper (photos-crop, clock-settings, log-code) and that mobile-mcp was 0.5 s faster on wakelock-restrict. Every number has a row in `claims-ledger.md`. Sources for the mechanics are in `research-2026-11.md`, LinkedIn: dwell counts when at least half the update is visible (official), engagement bait is demoted (official), documents beat video and text on company pages in one 1.3M-post dataset (anecdotal), and external links cost reach by an amount the sources disagree on (anecdotal).

## What the post does

- The first two lines show before "see more" (about 210 characters on mobile [anecdotal]). They carry the whole claim, so a reader who stops there still gets it.
- **Format: the demo video as a native upload for the launch post.** The video is the strongest asset; a carousel gets its own post three days later (below), so one account is not posting two things at once and you can answer comments on both.
- **No link in the body.** The repo link goes in the first comment, posted the moment the post is live. Expect fewer clicks and keep the repo name in the body, so people can search it.
- 1 to 3 hashtags at the end (LinkedIn's own guidance in round 2). Use none if you prefer; do not use ten.
- No "comment YES for the link", no "agree?", no tagging people who did not work on it. All demoted in the official feed notes.
- Reply to every comment in the first hour, and keep replying that day. Replies count as dwell and conversation.

## Launch post (video)

[DEMO: native upload of the re-recorded MP4, captions burned in. The post must read without the sound.] Length target 1,200 to 1,800 characters.

```
An AI agent with a phone can tap Pay.

I built the thing that stops it. Claude Code ordered butter chicken and garlic naan on Uber Eats from an Android emulator, and when it reached checkout it had to ask me. [DEMO: if the new clip shows the approval prompt, say "and the tap on Place order waited for my Allow".]

The project is phonedriver, an open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones.

What I learned building it:

1. Reading the screen was the slow part. adb takes 2.35 seconds to read a 1,000-row list. A resident agent on the phone takes 24 ms. That is the 98x on the chart.
2. After a tap the agent needs to know what changed, not the whole screen. Every action here returns only the changed lines.
3. A rule in a prompt that says "ask before you spend money" is advice. The check has to live in the tool, where the model cannot answer for itself. phonedriver asks through an MCP prompt: Allow, Allow for this session, or Decline.

Limits, so nobody has to ask: the speed numbers come from an Android emulator, not a phone. iOS needs a Mac with Xcode. The guard matches English labels only. The video is sped up 10x.

It is AGPL-3.0, with no telemetry. Repo in the comments.

#MCP #ClaudeCode #Android
```

Facts per line: opening and demo (README demo caption, `progress.md`), 2,352.4 ms and 24.0 ms (`speed-2026-11-07.md`), the diff (README), the guard (D27), limits (README, ledger). Point 1 is your phrasing of the result; "learned" is true only if you did learn it. [USER: change any line that is not your experience.]

**First comment** (post at once):

```
Code: github.com/hridaya423/phonedriver
Install: claude mcp add phonedriver -- phonedriver mcp
Method for the 98x, with p95 and raw data: github.com/hridaya423/phonedriver/blob/main/docs/bench/speed-2026-11-07.md
Site: phonedriver.dev
```

## Carousel post (T+3, document)

Caption (about 400 characters), then the PDF.

```
Seven slides on how an AI agent reads a phone, and why it asks before it pays. Everything here is measured on an Android emulator and links to the data in the repo.

Code: github.com/hridaya423/phonedriver
```

Fewer than 400 characters; put the link in the caption here, or in the first comment if you want the same treatment as the launch post. [USER: pick one and keep it.]

**PDF spec:** portrait, 1080x1350 per slide [anecdotal; LinkedIn accepts PDF up to 100 MB]. Dark text on the site's near-white background to match `hero-social.png`. One idea per slide, 24 words at most, body text 44 pt or more, nothing in the outer 60 px. Export slide 4 from `.github/assets/hero-light.svg` and slide 6 from a rebuilt tokens chart (the existing `tokens-light.png` is stale).

| # | Slide text (retype if you change it) | Visual | Source |
|---|---|---|---|
| 1 | **An AI agent with a phone can tap Pay.** What stops it? | Phone-sized black frame, no clutter. Brand wordmark small. | D27 |
| 2 | **adb reads one screen in 2.35 seconds.** A 1,000-row list: 2,352 ms. An agent that reads after every tap spends its time waiting. | One big number: 2,352 ms. | `speed-2026-11-07.md` |
| 3 | **phonedriver reads it in 24 ms.** A resident agent on the phone sends numbered text. Every tap returns only the lines that changed. | The three-line diff block from the README. | README Try it |
| 4 | **Up to 98x faster than adb.** Screenshot 41x. Type 100 characters 19x. Tap and see what changed 4.6x. | `hero-light.svg`, with the line "Android 17 emulator, p50 of 50 runs". | `speed.json` |
| 5 | **It asks before it pays.** A tap on Place order or Pay stops and asks you: Allow, Allow for this session, Decline. The model cannot answer. | The elicitation form [DEMO: screenshot from the new recording]. | D27 |
| 6 | **34% fewer tokens than phone-harness.** Three agent tasks, same model: 218,357 against 330,375. It lost one task. | Three-row chart, rebuilt. | `tokens.md` Round 3 |
| 7 | **What it does not do yet.** No Android phone measured. iOS needs a Mac with Xcode. The guard reads English labels only. | Plain text list. | README, D27 |
| 8 | **Open source, AGPL-3.0, no telemetry.** Android and iOS. CLI and MCP. `claude mcp add phonedriver -- phonedriver mcp` and phonedriver.dev | Wordmark, repo, one command. | README |

Slide 7 is the one that earns trust. Do not move it to the end behind a call to action; the order above puts it before the ask.

## After posting

- Check the post at 30, 60 and 120 minutes; answer every comment.
- Do not edit the post in the first hour: edits can reset how the feed treats it [anecdotal; unverified].
- Do not tag companies. If Anthropic or an MCP maintainer comments, answer them; do not chase them.
- Record impressions, reactions, comments and referrals from LinkedIn in GitHub Insights.
