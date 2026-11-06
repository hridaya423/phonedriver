# LinkedIn plan (round 2)

Nothing here has been posted. Slots: `[TBD: ...]` waits on a build or measurement. `[USER: ...]` needs your fact. `claims-ledger.md` names the repo file behind each claim. LinkedIn builds classifiers for generic AI text and reduces its reach, so write the final post by hand and keep your own details in it.

## What LinkedIn says, and what the post does with it

Source for every row: `research.md`, round 2, LinkedIn section.

| Rule | Evidence | What the post does |
|---|---|---|
| Retrieval matches the post's meaning to each viewer's history, so a niche topic reaches matching viewers ("sparse content niches"). Ranking predicts long dwells, likes, comments, shares. | LinkedIn Engineering, 2026-03-12 (no weights published) | State the topic in the first line: AI agents on phones. Write for people who work on agents, mobile or developer tools. |
| Dwell counts when half the post is visible. A skip-probability model lowers the score of posts people skip. | LinkedIn Engineering, 2020 | The first two lines carry a number. Keep them under about 140 characters so they survive the "see more" cut. |
| Length: highest engagement at 2,000 to 3,000 characters or under 150. The 600 to 1,500 range was weakest. | Ordinal, 248,275 posts (third party) | Write about 2,000 characters, not 1,200. Every paragraph must add a fact. |
| Format: video 6.00%, text 4.50%, link 3.25% in 1.3M business-page posts. | Socialinsider (company pages) | Attach the demo as native video, with burned-in captions that match what the video shows (LinkedIn demotes "videos that don't match the text"). |
| Links: no official penalty. Posts with links reach less (448 against 705 median impressions, LinkPost via Ordinal), mostly on company pages. First-comment links "stopped being free" and draw fewer clicks. | Ordinal | Put the repo link in your first comment, posted the moment the post goes live. The post says "link in the first comment". The clicks come from HN, X and Reddit. LinkedIn is for reach and conversation. If the first hour is weak, test the link in the body next time. |
| Engagement bait is demoted ("comment to agree", pods, comment automation). "It's ok to ask for thoughts and feedback to encourage meaningful conversation." | LinkedIn feed update, content guidance | End with one real question. No "comment YES". No pods. |
| Hashtags: "between 1-3 hashtags", and none tacked on without need. | LinkedIn staff via Buffer | Use two. |
| Replying to comments is said to lift engagement (Buffer: 30%, own data). The "golden hour" has no official definition. | Buffer; contentin.io | Reply to every comment in the first hour anyway. |
| Weekday mornings, weak effect. | Buffer, Sprout; one study found no reliable best hour | Tuesday, 13:30 UTC (9:30 am ET). |

## Post (draft, about 2,100 characters)

First two lines (the hook; 140 characters or fewer on one screen):

```
Reading one Android screen with adb takes 2.8 seconds.
Reading it with phone-use takes 237 milliseconds, on the same emulator.
```

Body, in your words, in this order:

```
I built phone-use so an AI agent can use a phone without waiting on the plumbing. It is a Rust CLI and MCP server for Android and iOS. Every tap returns only the lines that changed on screen, as text.

Here is Claude Code driving a phone with it, in real time. [TBD: confirm the re-recorded clip is real time and uncut between tool calls]

Three decisions did most of the work:

1. A daemon keeps the device connection open. On Android a small agent starts with app_process as the shell user, the way scrcpy does, so nothing is installed on the phone and no accessibility service needs switching on.

2. Every action waits for the screen to settle, then returns a diff. The agent never sleeps or polls.

3. Refs like @e10 are found again at tap time, so a list that moved by one row still gets the right tap.

What I measured, on an Android 17 emulator on a busy machine, median of 50, with adb measured back to back: screenshot 45 ms against 692 ms, screen read 237 ms against 2,800 ms, and "tap, then know what changed" 667 ms against 3,112 ms. [TBD: paired quiet-machine re-run]

Does an agent finish real tasks? Claude Sonnet 5.5 finished 15 of 15 held-out runs on five Android tasks, with a median of 6 tool calls. My first pass scored 9 of 15, and all six failures were bugs in my checks, not in the tool.

What does not work yet: a tap still misses its latency budget (about 34 ms on the emulator, 454 to 534 ms on an iPhone and an iPad, because XCTest is slow to confirm an event). I have not measured a physical Android phone. iOS needs a Mac with Xcode. In my one-run token benchmark, raw adb and phone-harness used fewer tokens than phone-use. [TBD: rerun with the shell tool]

It is AGPL-3.0 and sends no telemetry.

[USER: one sentence on how you built it, including agent assistance]

Which app should an agent be able to drive on your phone that it cannot today? Repo link in the first comment.

#opensource #AIagents
```

Check before posting:

- Length. The draft is about 2,100 characters. If your rewrite drops under 1,500, cut it to under 150 characters or grow it past 2,000. The 600 to 1,500 band had the weakest engagement.
- Do not add a third hashtag, an @mention of a company, or a tag of someone who has not agreed.
- The question is real. If you cannot answer replies about specific apps, change the question.
- First comment, posted at once: `Repo: https://github.com/hridaya423/phone-use (AGPL-3.0). The benchmark method and raw data: docs/bench/adb.md.`

## Video

Native upload of the re-recorded clip. [TBD: new clip] The old clip is 960x660 H.264 MP4, which LinkedIn accepts. The feed autoplays muted, so burn in one short caption naming the task. Make a copy for this; do not overwrite the original. Keep the caption faithful to the video. Vendors suggest 30 to 90 seconds and vertical or square formats; LinkedIn's ad page lists 4:5 (720x900) as its recommended vertical size and gives no organic video rule. The evidence is thin, so a landscape clip is acceptable.

Option to test later, not for launch day: a document carousel of the speed table and the diff sample. Documents scored 7.00% against video's 6.00% in company-page data, and the effect on a personal profile is unknown.

## First hour

- Post the first comment with the link immediately.
- Reply to every comment, with a fact, within the hour. Do not use AI to write replies. LinkedIn builds classifiers for generated comments.
- Do not ask friends to like. Ask people with real questions to say what they would test.
- Expect a modest result. The launch posts the research found drew 7 to 157 reactions.
