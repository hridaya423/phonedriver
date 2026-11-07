# Product Hunt plan

Nothing here is submitted. Rules and sources: `research-2026-11.md`, Product Hunt. Slots: `[DEMO]`, `[USER]`.

## Recommendation: do not launch in launch week. Launch on T+14 or later, only if the gate below is met.

Why:

- No dataset ties a Product Hunt launch to GitHub stars for a command-line tool. The one number found is a single self-reported case (200 to 600 stars for an open-source project) from one author [anecdotal].
- phonedriver's users are people who already run Claude Code or Cursor and an emulator. Product Hunt's audience is mostly founders and consumers. Daily.dev's piece on dev-tool launches says audience match beats the size of the spike [anecdotal].
- It would compete for the same two hours of your attention as HN, X and Reddit, and a Product Hunt launch needs hours of replies on the day itself.
- A launch day is not repeatable: relaunching within about 6 months may face review [anecdotal]. Use it once, when the product has more to show (the real-jobs benchmark result, a physical Android measurement, a Cursor test).

Launch there only if: the site's landing page lets a visitor see the demo and copy the install line in 10 seconds; the release has been out for two weeks without a P0 bug; you have a real physical-phone result to show; and you can be online from 12:01 am Pacific for a full day.

## If you launch

**Rules to follow** (https://www.producthunt.com/launch): a personal account, not a company one. Share the link; do not directly ask for upvotes (ask people to look and comment). You do not need a hunter. 12:01 am Pacific is the recommended start. Do not coordinate votes. Product Hunt clears fake upvotes [anecdotal].

**Name:** phonedriver

**Tagline** (60 characters maximum):

```
Let AI agents see and control Android and iOS phones
```

52 characters. Alternate: `Let Claude Code use a phone. It asks before it pays.` (52).

**Description** (260 characters maximum):

```
Open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones. Every tap returns what changed. Up to 98x faster than adb. Asks you before Pay or Place order. AGPL-3.0, no telemetry.
```

226 characters. "Up to 98x faster than adb" needs the qualifier somewhere on the page: the first comment states emulator and p50.

**Topics:** Developer Tools, Artificial Intelligence, Open Source, Android, GitHub. [USER: pick from the current list; names change.]

**Links:** website `https://phonedriver.dev`, GitHub `https://github.com/hridaya423/phonedriver`.

**Gallery** (all must be your own real captures; check dimensions in the submit form, since they change [UNVERIFIED]):

1. Thumbnail 240x240 [UNVERIFIED size]: the phonedriver mark, from `design/build` brand assets (rebuild after rename).
2. The demo video (30 s or shorter), re-recorded, the Uber Eats order and the approval prompt. [DEMO]
3. `.github/assets/hero-social.png`: "Up to 98x faster than adb".
4. The approval prompt in Claude Code (a screenshot of the real prompt). [DEMO]
5. The tap reply: a terminal screenshot showing the numbered snapshot and the diff after a tap.
6. The iOS Simulator or the iPhone running through phonedriver, from a real run. [USER: capture]
7. The tokens chart, rebuilt (three rows, 218,357 against 330,375).

Look at every image on a phone-width preview before submitting.

## First comment (maker comment, your own words)

Product Hunt readers ask "why did you build this" first. Post it the minute the page is live. Length 150 to 250 words. Facts, not text to paste:

- Who you are, and the sentence you say to a friend: it lets an AI agent use a phone.
- Why: [USER: your reason. Facts available: reading a screen with adb took 2.35 s on a 1,000-row list; an agent that polls and sleeps wastes calls; you read the phone-harness source before starting.]
- What it does: numbered text screen, a diff after each action, Android and iOS, CLI and MCP, a shell on the device, an approval prompt before Pay or Place order.
- The numbers with their setting: 98x for reading a 1,000-row list, 24.0 ms against 2,352.4 ms, Android 17 emulator, p50 of 50 runs; 34% fewer tokens than phone-harness across three tasks (218,357 against 330,375).
- Limits: emulator-only speed numbers, iOS needs a Mac with Xcode, the guard reads English labels only.
- The ask: which apps should it work with, what broke.

## During the day

Reply to every comment within 30 minutes. Do not DM people asking for votes. Do not post the link in groups of people who might upvote for you. Record the rank, the comments and referrals from Product Hunt (GitHub Insights, Traffic).
