---
title: phonedriver vs adb for AI agents on a phone
seo: phonedriver vs adb: 170x faster screen reads for AI agents
updated: 2026-11-11
meta: phonedriver against adb on an Android emulator: 170x faster UI tree reads, 32x faster screenshots, 23x faster typing, and a tap 2.4 to 4.4x faster.
description: phonedriver against plain adb on an Android emulator. 170x faster UI tree reads, 32x faster screenshots, 23x faster typing, and a bare tap that is 2.4 to 4.4 times faster. A HONOR X8 phone gave 32x and 61x on two operations. What adb still does better.
---

## The short answer

adb is how most agents already touch an Android phone, one shell command at a time. On the same emulator, with both tools run on the emulator's own host and interleaved, phonedriver read a 1,000-row list in 12.7 ms where `uiautomator dump` took 2,163.7 ms at p50. That is 170 times faster (95% CI on the ratio 162.0 to 177.4). The bare tap is the closest row: 4.4 times faster when taps come back to back (the interval on that ratio is wide, 3.0 to 5.2) and 2.4 times when they are 1.5 s apart.

phonedriver is not a replacement for adb. It uses the adb server as its transport, and adb stays the better tool for installs, logs, port forwards and anything you script yourself.

## Measured

p50 of 50 runs after 20 warm-ups, Android 17 emulator (API 37, headless, software rendering), phonedriver 0.1.1, 11 November 2026. Host CPU load was 31% to 80% around the rows, from other work on the machine. Source: [speed-2026-11-11]({{REPO}}/blob/main/docs/bench/speed.md), raw samples on the [benchmarks page](/benchmarks/#data).

| Operation | phonedriver | adb | adb / phonedriver (95% CI) |
|---|---|---|---|
| UI tree, 1,000-row list | 12.7 ms | 2,163.7 ms | 170x (162.0 to 177.4) |
| UI tree, Settings | 51.0 ms | 2,215.7 ms | 43x (41.1 to 48.0) |
| Screenshot | 9.1 ms | 291.2 ms | 32x (30.9 to 33.2) |
| Type 100 characters | 158.3 ms | 3,638.4 ms | 23x (19.7 to 25.1) |
| Tap, then what changed | 514.0 ms | 2,477.2 ms | 4.8x (4.7 to 5.2) |
| Tap, back to back | 38.5 ms | 170.5 ms | 4.4x (3.0 to 5.2) |
| Tap, 1.5 s apart | 28.7 ms | 69.4 ms | 2.4x (2.3 to 2.6) |

The tap timed on the device itself, from the host sending it to the testbed's touch listener logging it, is left out of this run: the difference was inside the clock-offset error. On 7 November it was 45.0 ms against 77.0 ms for adb.

## What the numbers do not show

- The outputs differ. adb returns raw XML and a tap that returns before the app reacts. phonedriver returns a pruned, numbered text tree and waits for the app. For "tap, then what changed" the adb row is a tap, a `uiautomator dump` and a diff written by the caller.
- `adb shell input text` raises on non-ASCII text. phonedriver typed 100 mixed-script characters (ASCII, accents, CJK, Arabic, emoji) in 160.6 ms.
- `uiautomator dump` waits for the UI to go idle and fails on screens that keep animating. I saw it fail with "could not get idle state" on the Contacts list screen. Plain `uiautomator dump` also fails while phonedriver's agent is attached, because Android allows one UiAutomation connection at a time.
- I did not split adb's 2.16 s between process start and the idle wait, so I do not claim which part is which.
- The main run is on an emulator. A smaller pass on a HONOR X8 (Android 13) over USB, on 9 November, measured two operations. Reading the screen took 78.5 ms against 2,506.6 ms for `uiautomator dump` (32 times faster), and a screenshot took 16.4 ms against 1,005.7 ms for `screencap` (61 times faster). On the phone's main Settings page `uiautomator dump` failed with "could not get idle state" after 11.5 s, and phonedriver read the page in 110 ms. Typing and taps were not measured on the phone.

## Tokens

An agent given only adb finished three Android tasks in a token run as follows: it set an alarm in 2 calls and 18,607 tokens using the `SET_ALARM` intent, which the task allows, found Grace Hopper's birth date in 9 calls and 72,554 tokens, and failed the web form, a WebView with no accessibility nodes, because it could use nothing but adb. When the job has an intent, adb is hard to beat. When the screen is the only way in, it is not ([token benchmark](/benchmarks/#tokens)). On the two mixed jobs of the [real phone jobs benchmark](/blog/phonedriver-vs-mobile-mcp-vs-phone-harness-vs-adb-real-phone-jobs/), where any route was allowed, adb passed 4 of 4 with 203,751 tokens in total, and phonedriver passed 6 of 6 with 92,737 for the same two jobs (10 November 2026).

## Where adb is better

- Installing and removing apps, port forwarding, `logcat`, `dumpsys`, file transfer.
- Scripts that always do the same thing: no model, no tokens, no daemon.
- Every Android developer already has it. phonedriver needs adb too.

## Use which

Use adb when you know the command. Use phonedriver when an agent has to look at a screen, act, and decide from what changed. phonedriver's [shell](/docs/shell/) runs adb-style commands in the on-device agent when an agent should run a setting or an intent without leaving the session.
