# X plan (round 3)

Nothing here is posted. X does not ban AI text, but readers and the 2026 code both punish posts that read as slop (`research-2026-11.md`, X). Retype each post in your own voice; the drafts below fix the facts and the shape. Slots: `[DEMO: ...]` waits on the re-recording, `[USER: ...]` needs you. Every number has a row in `claims-ledger.md`. Character counts are measured, with each URL counted as 23 characters (what X does).

## Mechanics this plan uses (sources in `research-2026-11.md`)

| Mechanic | Evidence | What the thread does |
|---|---|---|
| Replies, quotes and copy-link shares carry the weights (reply 5.0, plus 15.0 from a mutual on an original post; copy-link 20.0; like 0.708). | Open 2026 ranking code, read 2026-11-06 [code]. | The hook is one claim, one video, and a question at the end of the thread. Make it easy to send to a friend. |
| Cold start: an original post from an account under 50,000 followers, under 2 hours old, under 200 impressions can be lifted, chosen by likes per home view. | Same code. | Post when your own followers are awake. The first frame of the video must make a stranger stop. |
| A second post from you in the same feed batch keeps 62.5% of its score, a third 43.75%. | Same code. | One hook post. The rest are replies inside the thread, not new top-level posts, and the standalone follow-ups go out on other days. |
| Video quality view counts only above 10 s. Dwell is scored. | Same code. | Video of 25 to 35 s, native upload. |
| No link multiplier in the code. X's product head says links are not penalized [anecdotal]. Out-of-network replies are dropped from ranking [code], so a link in a reply is seen by fewer people. | Research. | **Your brief: link in a reply, not the hook.** Done below, with the cost known: people who see only the hook will not see the link. The video's last frame shows `phonedriver.dev` and your profile links the repo. |
| Negative actions are heavily weighted (not interested -53.07, mute -58.8, report -234.0). | Same code. | No hashtags, no "like and retweet", no tag spam, no engagement-bait question like "reply 1 for the link". |

## Before posting

1. Pick 5 to 10 mutuals (people who follow you and you follow) who work on agents, Android or iOS. Message each a day before: what it is, the repo, "tell me what breaks". No script, no like requests. The mutual boost on replies only counts if the replies are real.
2. Put the repo and `phonedriver.dev` in your profile bio and pin nothing yet.
3. Do not post, DM or tag the HN thread link. Do not ask anyone to boost.
4. Post when your followers are online. The plan puts the thread on launch day at 13:30 UTC (see `plan.md`) [UNVERIFIED as best hour].

## The launch thread

Attach the demo as a native upload (not a link, not a GIF). [DEMO: the re-recorded MP4; 25 to 35 s, readable on mute, with burned-in captions, `phonedriver` in the terminal. Today's `.github/assets/demo.mp4` is 38.9 s, 960x660, 10x sped up, terminal says `phone-use`, and ends at Uber's sign-in page.] Set alt text to one sentence that names what the clip shows.

### 1. Hook (video attached, no link)

```
I gave Claude Code a phone.

It picked the best-rated Indian place near Times Square, added butter chicken and garlic naan to the cart, and stopped at checkout to ask me.

10x speed. Android emulator. Nothing was ordered.
```

221 characters. True of today's clip as: "stopped at Uber's sign-in page and asked me to sign in and approve". [DEMO: if the new recording shows the approval prompt, use this instead:]

```
I gave Claude Code a phone and asked it to order dinner.

When it got to Place order, it stopped. A prompt asked me: Allow, Allow for this session, or Decline.

The model can't answer that prompt. I can.
```

Alt text: "Claude Code's terminal beside an Android emulator. Claude orders butter chicken and garlic naan on Uber Eats at 10x speed and stops at checkout."

### 2. The link (reply to your hook)

```
It's called phonedriver: an open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones.

claude mcp add phonedriver -- phonedriver mcp

Code: github.com/hridaya423/phonedriver
Site: phonedriver.dev
```

245 characters. Test the `claude mcp add` line from the released binary before posting.

### 3. What the agent sees (reply to 2)

```
Every screen is numbered text. Every tap returns only the lines that changed:

ok tapped @e41 "Dark Appearance" · settled in 607ms
~ @e41 switch "Dark Appearance" off → on
+ @e68 button "L4S"

The agent never polls or sleeps.
```

225 characters. Source: README, "Try it", step 3 (iOS Simulator).

### 4. Speed (reply to 3), with `.github/assets/hero-social.png`

```
It is also fast. On an Android 17 emulator, p50 of 50 runs:

read a 1,000-row screen: 24 ms vs 2,352 ms for adb (98x)
screenshot: 41x
type 100 chars: 19x
tap, then see what changed: 4.6x

Emulator only. No Android phone measured yet.
```

233 characters. I opened `hero-social.png` (1200x675): the title reads "Up to 98x faster than adb", four rows with the right figures (24 vs 2,352, 12 vs 476, 251 vs 4,841, 587 vs 2,726), a source line. It shows 12 ms for the screenshot where the text says 11.6 ms elsewhere: the chart rounds. Fine. It has no product name, so the post carries it. Alt text: "Bar chart: phonedriver against adb at p50 on an Android emulator. Read the screen 24 ms against 2,352 ms, 98 times faster."

### 5. The approval guard (reply to 4)

```
An agent with a phone can tap Pay. So phonedriver asks you first.

Before a tap on Place order or Pay, Claude Code shows a prompt: Allow, Allow for this session, or Decline. The model can't answer for itself.

It matches English labels only. Limits are in the post below.
```

271 characters. [DEMO: attach the 6 s approval-prompt cut from the new recording.] "The post below" does not exist in a thread; replace with the URL if you want, as a fifth reply: `phonedriver.dev/blog/letting-claude-order-dinner-without-letting-it-spend-your-money/`.

### 6. Limits and the question (reply to 5)

```
Limits: iOS needs a Mac with Xcode. I tested an iPhone 14 Plus and an iPad Pro, a tap there takes 454 to 534 ms. Android hardware is untested. AGPL-3.0, no telemetry.

Which app should I point it at next?
```

204 characters. The question is real: replies of the form "WhatsApp" or "my banking app" give you something to answer, and a place to say what the guard does not cover. Answer every reply for two hours.

Optional, if you want a seventh: the tokens result (see follow-up 3). Do not post it in the thread: it is the weakest claim, and the thread is long enough.

## Three standalone follow-ups (separate top-level posts, other days)

Post each at the same hour as the thread, one per day on T+1, T+3 and T+5 (see `plan.md`). Never two top-level posts the same day.

### Follow-up 1 (T+1): the 98x chart

Attach `.github/assets/hero-social.png`.

```
98x is the headline. Here is where it comes from, with the adb number beside each one.

The tap row is the closest: 2.1x back to back, 1.6x when 1.5 s apart.

Method, p95 and raw samples: github.com/hridaya423/phonedriver/blob/main/docs/bench/speed-2026-11-07.md
```

211 characters. Numbers: `speed-2026-11-07.md`, `speed.json` (back to back 117.6 against 250.5 ms, 1.5 s apart 72.4 against 114.5 ms).

### Follow-up 2 (T+3): the approval guard

Attach the approval-prompt clip [DEMO] or, until it exists, nothing: do not post this follow-up with a screenshot you did not take from a real run.

```
What should stop an AI agent from tapping Place order?

A rule in a prompt is advice. phonedriver asks a human through an MCP elicitation, inside the tool, where the model can't answer. Allow, Allow for this session, Decline.

What it misses is written down too: phonedriver.dev/blog/letting-claude-order-dinner-without-letting-it-spend-your-money/
```

Check the length before posting: about 270 characters with the URL counted as 23.

### Follow-up 3 (T+5): the tokens

Attach a three-row tokens chart [`.github/assets/tokens-light.png`, rebuilt 2026-11-07: phonedriver against phone-harness, per task and the 218,357 against 330,375 sum].

```
Three agent tasks, same model (Claude Sonnet 5.5), same emulator, every run passed:

phonedriver: 218,357 tokens
phone-harness: 330,375

34% fewer. phonedriver lost one task, 57,753 against 53,403. One to three runs per cell, so read it as a direction. phone-harness is from the earlier round of the same setup.
```

About 290 characters: trim the last sentence if X will not take it. Source: `docs/bench/tokens.md`, "Round 3". Do not post this alone as "34% cheaper"; mobile-mcp is not in it because its runs failed for an environment reason.

## Accounts: tag only where it is relevant

No handle below was verified as active for this research. Open each profile and read the last 10 posts before you decide. The default is to tag nobody. A tag costs you if the account mutes or reports you (the negative weights are the largest in the code).

| Who | Tag when | Do not |
|---|---|---|
| @badlogicgames (Mario Zechner) | His 2026-02-02 post ("anfroid + termux + pi", an adb-driven phone agent) is the closest prior art, and a reply under it that adds a number is fair. Read the post first. | Tag him in your launch thread. |
| Claude Code and Anthropic accounts (`@AnthropicAI`, `@claudeai`) and Claude Code's maintainers | Never in the launch. In follow-up 2 only if the post is about a feature of Claude Code (MCP elicitation), and say what you built on it. | Ask for a retweet. |
| MCP maintainers and registry accounts | When you publish to the official registry: one post saying so, no tag. | Tag to ask for a feature. |
| Android and iOS tooling accounts (Appium, Maestro, mobile-mcp authors, scrcpy's author) | Only when a comparison names them and you are fair: the compare page is the place. | Tag in a post that says you are faster than their tool. |
| Mutuals who tried it | Quote their post when they share a result; say what they did. | Ask a mutual to post. |

Communities: X Communities for MCP or Claude Code exist [UNVERIFIED]. Post there only if the community's rules allow project posts, and after the thread, as a different text, not a copy.

## While it runs

- Reply to every reply for two hours, then check hourly for six more.
- When someone says it broke, say you will look, open an issue, link it.
- Do not delete and repost. Do not quote-post your own thread to bump it.
- Record likes, replies, link clicks (GitHub, Insights, Traffic, Referring sites) and stars per hour in `progress.md`.
