---
title: phonedriver vs adb for AI agents on a phone
meta: phonedriver against adb on an Android emulator: 98x faster UI tree reads, 41x faster screenshots, 19x faster typing, and a tap only 1.1 to 1.7x faster.
description: phonedriver against plain adb on an Android emulator. 98x faster UI tree reads, 41x faster screenshots, 19x faster typing, and a bare tap that is only 1.1 to 1.7 times faster. What adb still does better.
---

## The short answer

adb is how most agents already touch an Android phone, one shell command at a time. On the same emulator, with both tools run on the emulator's own host and interleaved, phonedriver read a 1,000-row list in 24.0 ms where `uiautomator dump` took 2,352.4 ms at p50. That is 98 times faster (95% CI on the ratio 92.2 to 106.0). The bare tap is close: 1.1 times faster when taps come back to back and 1.7 times when they are 1.5 s apart.

phonedriver is not a replacement for adb. It uses the adb server as its transport, and adb stays the better tool for installs, logs, port forwards and anything you script yourself.

## Measured

p50 of 50 runs after 20 warm-ups, Android 17 emulator (API 37, headless, software rendering). Mean host CPU load was 13% to 39% per operation from other work on the machine. Source: [speed-2026-11-07]({{REPO}}/blob/main/docs/bench/speed-2026-11-07.md), raw samples on the [benchmarks page](/benchmarks/#data).

| Operation | phonedriver | adb | adb / phonedriver (95% CI) |
|---|---|---|---|
| UI tree, 1,000-row list | 24.0 ms | 2,352.4 ms | 98x (92.2 to 106.0) |
| UI tree, Settings | 89.3 ms | 2,433.5 ms | 27x (25.6 to 27.8) |
| Screenshot | 11.6 ms | 476.3 ms | 41x (39.1 to 43.5) |
| Type 100 characters | 251.1 ms | 4,840.9 ms | 19x (17.1 to 22.4) |
| Tap, then what changed | 586.9 ms | 2,725.8 ms | 4.6x (4.5 to 4.7) |
| Tap, 1.5 s apart | 61.6 ms | 106.6 ms | 1.7x (1.6 to 1.9) |
| Tap, back to back | 175.6 ms | 199.5 ms | 1.1x (1.1 to 1.3) |

On the device itself, from the host sending a tap to the testbed's touch listener logging it, phonedriver takes 27.5 ms and adb 71.2 ms (2.6 times; the clock offset is good to about 8 ms).

## What the numbers do not show

- The outputs differ. adb returns raw XML and a tap that returns before the app reacts. phonedriver returns a pruned, numbered text tree and waits for the app. For "tap, then what changed" the adb row is a tap, a `uiautomator dump` and a diff written by the caller.
- `adb shell input text` raises on non-ASCII text. phonedriver typed 100 mixed-script characters (ASCII, accents, CJK, Arabic, emoji) in 225.0 ms.
- `uiautomator dump` waits for the UI to go idle and fails on screens that keep animating. We saw it fail with "could not get idle state" on the Contacts list screen. Plain `uiautomator dump` also fails while phonedriver's agent is attached, because Android allows one UiAutomation connection at a time.
- We did not split adb's 2.35 s between process start and the idle wait, so we do not claim which part is which.
- The run is on an emulator. No physical Android phone is in it.

## Tokens

An agent given only adb finished three Android tasks in a token run as follows: it set an alarm in 2 calls and 18,607 tokens using the `SET_ALARM` intent, which the task allows, found Grace Hopper's birth date in 9 calls and 72,554 tokens, and failed the web form, a WebView with no accessibility nodes, because it could use nothing but adb. When the job has an intent, adb is hard to beat. When the screen is the only way in, it is not ([token benchmark](/benchmarks/#tokens)).

## Where adb is better

- Installing and removing apps, port forwarding, `logcat`, `dumpsys`, file transfer.
- Scripts that always do the same thing: no model, no tokens, no daemon.
- Every Android developer already has it. phonedriver needs adb too.

## Use which

Use adb when you know the command. Use phonedriver when an agent has to look at a screen, act, and decide from what changed. phonedriver's [shell](/docs/shell/) runs adb-style commands in the on-device agent when an agent should run a setting or an intent without leaving the session.
