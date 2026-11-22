---
title: phonedriver vs adb for AI agents on a phone
seo: phonedriver vs adb: 170x faster screen reads for AI agents
updated: 2026-11-20
meta: phonedriver vs plain adb: 170x faster list reads, Unicode typing, settled diffs, and why adb is still better for installs, logs, and fixed scripts.
description: Plain adb starts a new JVM and waits a full quiet second on every screen read. phonedriver keeps a helper open—170x faster on a 1,000-row list—while still relying on adb underneath.
---

## The short answer

phonedriver doesn't replace `adb`—it speaks to your `adb` server directly. The difference is what happens when an AI agent has to *look* at a phone.

Plain `adb shell uiautomator dump` starts a fresh process on every call, waits for a full quiet second with zero accessibility events, and dumps raw XML. On an Android 17 emulator, reading a 1,000-row list took **2,163.7 ms** with `adb` and **12.7 ms** with phonedriver (**170x faster**, 95% CI 162.0–177.4). On a real HONOR X8 over USB, reading the screen was **78.5 ms vs. 2,506.6 ms (32x)**—and on Settings, `uiautomator dump` timed out after 11.5 seconds while phonedriver finished in 110 ms.

For installs, `logcat`, port forwarding, or fixed shell scripts, stick with `adb`.

::figure[speed-race]

## Where plain adb trips AI agents up

- **Every look costs two seconds.** Process spawn + `UiAutomation` connect + a 1-second idle wait adds up fast—and screens with an animation or ticker can fail outright with `"could not get idle state"`.
- **No Unicode typing.** `adb shell input text` chokes on accents, CJK, Arabic, and emoji. phonedriver typed 100 mixed-script characters in 160.6 ms without needing a custom keyboard APK.
- **No settle, no diff.** `adb shell input tap` returns before the app reacts, leaving the model to guess how long to sleep and dump 25 KB of XML again. Comparing like-for-like ("tap, then see what changed") is **514.0 ms vs. 2,477.2 ms (4.8x)**.
- **Bare taps are closer.** When you just fire a point tap and don't wait, phonedriver is 2.4x to 4.4x faster (28.7–38.5 ms vs. 69.4–170.5 ms)—the smallest gap in the suite.

## When raw adb actually wins

If a task has a one-line Android intent, raw `adb` is unbeatable. In my early evaluation, an agent given only `adb` set a 7:30 alarm in **2 calls and 18,607 tokens** via `SET_ALARM` without ever looking at the screen. On the two mixed jobs of [the nine-job benchmark](/blog/phonedriver-vs-mobile-mcp-vs-phone-harness-vs-adb-real-phone-jobs/), `adb` passed 4/4 in 203,751 tokens vs. phonedriver's 92,737—and on a WebView with no accessibility nodes, `adb` failed completely.

That's why phonedriver also exposes a [`shell`](/docs/shell/) tool: when a shell command is the cleanest route, your agent can run it without leaving the session.

:::receipts Measurements & 0.1.5 screenshot note
- **Emulator run (11 November 2026, phonedriver 0.1.1):** Android 17 emulator (`emulator-5554`, headless, software rendering), 50 samples after 20 warm-ups, interleaved on `win-ts` (CPU load 31%–80%). Full table & raw CSV on [the benchmarks page](/benchmarks/#speed).
- **0.1.5 screenshot correction:** The 9.1 ms emulator screenshot figure (32x vs. `screencap`'s 291.2 ms) came from 0.1.1's cached-frame path. In 0.1.5 screenshots wait on a freshness barrier ([speed post](/blog/how-phonedriver-reads-a-screen-170x-faster-than-adb/#a-video-stream-for-screenshots)).
- **Fairer rival:** Appium's UiAutomator2 server is also resident on-device; I haven't timed it against phonedriver.
:::

## Use which

Use **adb** when you already know the shell command, or for app installs, port forwards, `dumpsys`, and deterministic scripts. Use **phonedriver** when an agent needs to inspect screens, act, and reason from what changed.

## Questions

### What is a faster alternative to uiautomator dump?

Keep a resident helper on the device with the `UiAutomation` connection already open. Instead of paying a process boot and 1-second idle wait on every call (2,163.7 ms on a 1,000-row list), phonedriver's helper reads the same list in 12.7 ms (and Settings in 51.0 ms). Appium's UiAutomator2 server also stays resident, though I haven't benchmarked it here.

### Why does uiautomator dump fail with "could not get idle state"?

`uiautomator dump` refuses to snapshot until Android's accessibility event stream stays completely silent for one full second, giving up after ten. Any live animation, spinner, or ticking label can break it—including the Contacts list on an emulator and the main Settings screen on a HONOR X8 (failed at 11.5 s; phonedriver read it in 110 ms).

### How do I type Unicode text or emoji over adb?

`adb shell input text` fails on non-ASCII characters. phonedriver injects text through its on-device helper without installing a keyboard app, typing 100 mixed-script characters (accents, CJK, Arabic, emoji) in 160.6 ms at p50 on an Android 17 emulator.

### Does phonedriver replace adb?

No. phonedriver talks to your local `adb` server as its transport, so you still need `adb` installed, and `adb` remains the right tool for installs, logs, port forwarding, and fixed shell scripts.
