---
title: Benchmarking AI agents on jobs only a phone can do
short: Benchmarking agents on jobs only a phone can do
meta: How to benchmark phone agents without measuring shell shortcuts: UI-only jobs, database checks with no LLM judge, and 5 grader bugs caught by hand.
description: Ask an agent to "set an alarm" and raw adb finishes in two shell calls without touching a pixel. Here is how to build a phone benchmark that actually tests the phone.
date: 2026-11-07
label: METHOD
short_card: Benchmarking AI agents on jobs only a phone can do
card_sub: UI-only jobs, no LLM judge, and the five bugs I found in my own checks.
updated: 2026-11-20
---

There is an embarrassingly easy way to build a bad phone benchmark: pick tasks that don't actually need a phone.

*"Set an alarm for 7:30"* is one Android intent. *"Turn on dark mode"* is `cmd uimode night yes`. *"What year was Grace Hopper born?"* is a web search. Put those in a phone benchmark and you aren't measuring how well an agent sees or drives screens. You're timing how fast the model realizes it can walk around the phone entirely.

My very first evaluation walked straight into that trap. Raw `adb` cleared the alarm task in 2 calls and 18,607 tokens via `SET_ALARM` without ever glancing at Clock. Clever agent, useless test. So I threw those tasks out and built a suite where the screen is the only door in.

::figure[shortcut-trap]

## The rule: if one shell command fakes it, cut it

Seven core jobs require driving the app's real UI under the benchmark rules: no direct shell edits, no web substitutes. Raw `adb` *can* inject taps, of course, so "UI-only" doesn't mean `adb` is locked out of screens; it means nobody gets a one-command bypass. Raw `adb` competes only on the two mixed jobs below.

- **Chrome settings (`chrome-prefs`):** Make DuckDuckGo the default search engine and switch on "Always use secure connections." Both sit in Chrome's private `Preferences` file; no intent or `settings` key reaches them.
- **Clock preferences (`clock-settings`):** Add Tokyo and London world clocks and change snooze/silence timings. Android's alarm intents only set alarms and timers; preferences live inside Clock.
- **Messages flags (`messages-pin-archive`):** Pin one seeded thread, archive another, leave a third alone. Neither flag has a shell command.
- **Photo crop (`photos-crop`):** Crop a photo to a square and save a copy while leaving the original untouched. The emulator has no shell image editor; you have to use Photos.
- **Private pickup code (`relay-code`):** Read a random per-run code from a testbed app and save it as a new contact. Reading private storage via shell fails the run on the spot.
- **Two bug hunts (`qa-signup`, `qa-shop`):** Uncover planted bugs that only trigger under real interaction: a welcome banner that drops the last character of an email, a Create button that ignores the tap right after a rejected submit, a stale total after hitting Decrease, and a `SAVE10` code that subtracts a flat 10.00 instead of 10%.

I kept **two mixed jobs** on purpose where any route goes: spot which app is holding a wakelock (`dumpsys power`) and restrict its battery usage in Settings (`wakelock-restrict`), and pull a pickup code out of `logcat` into a new contact (`log-code`). Those reward tools that keep shell and UI skills under one roof.

What hit the cutting-room floor: adding a plain contact (`content insert`), calendar events, dark mode (`cmd uimode`), screen timeout (`settings put`), alarms and timers, SMS compose intents, web trivia, Uber Eats (needs sign-in), and Maps transit times (same answer on the web).

## Checks that don't take the agent's word for it

An agent will cheerfully report *"Done, HTTPS-only mode is on!"* right after tapping the wrong row and backing out. So every check reads the phone's actual state after the agent exits: Contacts provider, Messages' `bugle_db`, Clock's and Chrome's private preferences, `cmd appops`, MediaStore, and the testbed app's JSON-lines event log.

- **Contact checks** match phone digits by suffix, because Contacts likes to prepend `+1`.
- **Photo crop** checks that the original is still 1,200 px wide and a brand-new post-baseline image is square (within 1 px) and at most 800 px.
- **Battery restriction** diffs `RUN_ANY_IN_BACKGROUND` against a baseline and passes only if the testbed app—and nothing else—became restricted.
- **Bug hunts** grade the written report against the planted bugs with deterministic regexes (no LLM-as-judge) *and* inspect the event log to confirm the agent actually walked the broken paths: invalid submit, ignored tap, valid submit. No hallucinating bug reports from static text.

Between runs, cleanup wipes every created contact, message, photo, preference, and app-op so state never leaks from one run to the next.

## Prove the check before the agent

If you steal one habit from this post, make it this: **before running any model, drive every check by hand with `adb input` through a passing state and every distinct failure state.**

I caught five real bugs in my own graders that way. Every single one would have silently handed one tool a free pass or flunked a tool that did the work:

::figure[grader-bugs]

Not one of those five would have jumped out of agent transcripts. Testing your grader by hand is twenty dull minutes that save you from publishing nonsense.

## What this benchmark still doesn't prove

Nine jobs on one Android tablet emulator is a useful signal, not the whole mobile universe. There is no iOS job, no physical phone, and no real sign-in flow. Chrome, Messages, Photos, and Contacts are whatever versions ship in that emulator image (recorded alongside the results). And when `uiautomator dump` fails on Contacts with `"could not get idle state"`, it hurts harnesses built on it—which is a real quirk of the tool rather than a flaw in the job, so it stays in the table with a note beside it.

:::receipts Results, raw runs & reproduction
- **Full comparison write-up:** [phonedriver vs mobile-mcp vs phone-harness vs adb on real phone jobs](/blog/phonedriver-vs-mobile-mcp-vs-phone-harness-vs-adb-real-phone-jobs/).
- **Job table & checks:** [Benchmarks page](/benchmarks/#jobs).
- **Raw data & code:** [`jobs.json`](/benchmarks/data/jobs.json) and `bench/agent-eval/` in [the repository]({{REPO}}). Add a job I missed, or open an issue if you find a shortcut through one of mine.
:::
