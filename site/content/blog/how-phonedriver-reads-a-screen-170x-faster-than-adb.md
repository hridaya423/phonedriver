---
title: How phonedriver reads a phone's screen 170x faster than adb
short: Reading a screen 170x faster than adb
meta: How phonedriver reads a 1,000-row list in 12.7 ms against adb's 2,163.7 ms: profiling a 999 ms tap, bursty H.264 streams, and where the speedup disappears.
description: Inside 12.7 ms vs. 2,163.7 ms: the micro-optimizations that barely moved the needle, the idle-stream bug that broke 18 of 40 screenshots, and the weak rows.
date: 2026-11-07
updated: 2026-11-20
label: ENGINEERING
short_card: How phonedriver reads a screen 170x faster than adb
card_sub: Profiling a 999 ms tap, bursty video streams, and where the speedup disappears.
---

On an Android 17 emulator, phonedriver reads a 1,000-row list in **12.7 ms**. `adb shell uiautomator dump` takes **2,163.7 ms** for the exact same screen. At p50 over 50 runs, that's **170x faster** (95% CI: 162.0 to 177.4).

Keep your eyebrows down, though: 170x is the high-water mark, not a magic multiplier on everything. It pits a pruned text tree from a resident process against a one-shot CLI dumping raw XML. Bare taps are only 2.4x to 4.4x faster, and a resident server like Appium's UiAutomator2 would land much closer (I haven't timed it).

What actually makes the numbers interesting is watching where milliseconds go once you profile a real phone loop, and how reliably your first guess about a slow path turns out to be wrong.

::figure[speed-race]

## Why adb pays a two-second toll

Driving Android through plain `adb` means paying a cold-start tax on every look. `adb shell input tap` forks an `adb` client on your laptop and spins up an `app_process` JVM on the phone for every tap. `uiautomator dump` does the same, attaches to `UiAutomation`, insists on one full quiet second with zero accessibility events (failing outright on animating screens like Contacts), and writes raw XML to disk. `screencap -p` PNG-encodes a full frame on the phone every time. `input text` gives up on anything outside ASCII.

phonedriver skips the per-call choreography by keeping a small Java helper alive on the phone (`app_process` as the shell user, the scrcpy way) and holding one socket open to a Rust daemon on your laptop. Nothing gets installed, and no accessibility toggle gets touched. On the phone itself, a warm Settings tree read takes **11 ms** at p50; most of the 51 ms end-to-end Settings read is Android binder round trips (`getChild`, about one per 2 to 4 nodes) plus host pruning.

## A video stream for screenshots

Calling `screencap` on demand is slow, so the helper mirrors the display into an H.264 MediaCodec encoder and streams it to the daemon, which decodes it with OpenH264 (3.6–4.6 ms per 1080p frame on a Mac, up to roughly one CPU core at 1080p30; the first frame takes 219–433 ms once per device lease).

In version 0.1.1, `phonedriver screenshot` simply handed back the latest decoded frame sitting in memory: **9.1 ms** against `adb screencap`'s **291.2 ms**.

Then reality poked two tidy holes right through it.

::figure[bursty-stream]

First, I assumed a still screen still dripped frames at a polite, steady rate. It didn't. Over an SSH tunnel on API 37, the emulator's software encoder fired bursts of roughly 10 fps and then went completely silent for up to 5 seconds. My "no frame for 1 s = dead stream" rule promptly failed **18 of 40 screenshots** of an idle screen.

So I loosened the rule: serve the last decoded frame as long as the video socket stays open. Idle screenshots went green, and hole number two walked right in. Right after a tap, the frame sitting in memory could still be a photo from *before* the tap. In 0.1.5, `screenshot` waits on a device barrier and drains any frames still inside the encoder ([D74]({{REPO}}/blob/main/docs/decisions.md#d74-a-screenshot-waits-for-the-frames-the-encoder-still-holds-and-a-hidden-display-on-a-software-encoder-is-lighter)). That makes 9.1 ms a historical 0.1.1 number, not a current 0.1.5 timing.

## Profiling a 999 ms tap (and winning almost nothing)

The same stream drives settle detection: wait for a change above 128 pt² (Material toggles count, blinking carets don't), then wait 250 ms with no frame above 4,096 pt² (spinners don't trap you). [How phonedriver works](/blog/how-phonedriver-works/#knowing-when-the-screen-has-stopped-moving) lays out the two thresholds in detail.

Once settle worked, I profiled a settled tap over a 100 ms SSH tunnel expecting to find easy fat to trim. Total time: **999 ms**. Host-side pruning and rendering? **0.2 ms**. Almost every other millisecond was waiting on the phone, or waiting to be sure the phone was finished.

::figure[tap-waterfall]

Worse, 17 of 36 taps never triggered a frame above 128 pt² and sat out the entire 500 ms first-change timeout.

So I engineered three neat optimizations: start probing the accessibility tree 120 ms into the quiet window so matching reads skip stabilize; catch silent taps with a tree check at 100 ms; and skip stabilize's 100 ms sleep when the starting tree is already that old.

Then I ran an interleaved A/B test. The grand victory: **918 ms down to 843 ms** (and **860 to 825 ms** on a second 90-sample set). Forty to seventy-five milliseconds, sitting well inside the random jitter of a bare tap between two identical builds. Over a 100 ms tunnel, only one probe read even fits inside a 250 ms quiet window, so stabilize still ran half the time anyway. I kept the code because it helps more on local USB, and went looking for actual mistakes instead.

## Three mistakes that beat every micro-optimization

- **Cold start (6,750 ms to 3,950 ms over tunnel; 1,176 ms local):** Packing jar check, stale-agent kill, and launch into one shell call—and opening the control and video sockets in parallel—cut pre-boot round trips from `6 + devices` down to 3.
- **"The cold app swallowed my tap" (it didn't):** Clock simply took 0.6 to 1.1 seconds to open its time picker, arriving *after* my 500 ms first-change window. phonedriver replied `no change after 507ms`, so the agent obligingly tapped a second time, landing straight on the dialog that had just popped up. Widening the first-change window to 2 s for the 30 seconds after `launch` dropped false `no change` replies from 3/8 to 0/8.
- **Back-to-back point taps (126 ms to 20 ms on API 26):** The approval guard reads the screen before every tap, and a read right after a tap queues behind Android's button ripple on the app's UI thread (94 ms vs. 33 ms at rest). Reusing the previous guard read for up to a second (cleared on any window change or significant repaint) cut tap replies 6x. The catch is written right next to the code: a tiny repaint that spawns a guarded button inside that second would slip past.

## What the headline hides

- **Bare taps are the weak row.** 28.7 ms paced (2.4x faster than adb) and 38.5 ms back to back (4.4x, wide 3.0–5.2 CI, 175.6 ms p95 from ripple queueing). That misses my own 15 ms target, and 100-char typing misses its 150 ms target at 158.3 ms.
- **Different outputs.** adb's tap returns before the app even blinks; phonedriver waits and returns a diff. Comparing like-for-like ("tap, then what changed", where adb has to dump and diff) is **514.0 ms vs. 2,477.2 ms (4.8x)**.
- **Real phones are slower than an i9 emulator host.** On a HONOR X8 (Android 13, USB), reading the screen took **78.5 ms** vs. adb's **2,506.6 ms (32x)**, and screenshots took **16.4 ms** vs. **1,005.7 ms (61x)**. (iOS timings live in [the iOS runner section](/blog/how-phonedriver-works/#ios-my-own-runner). Since 0.1.5 a one-finger Simulator tap goes to the Simulator's own touch input, and a tap with its reply takes 316 ms on [simulator-use's benchmark](/benchmarks/#simulator).)
- **The LLM still dominates wall time.** Fast device calls kill the 2-second stalls per look, but cutting *how many turns* the model spends matters far more. That is what the settled diff is really for.

:::receipts Measurement method, machine & raw data (11 November 2026)
- **Host:** `win-ts` (Windows 11, Intel Core i9-13900K, 64 GB), local `adb`, headless Android 17 emulator (`emulator-5554`, 1080×2424, 420 dpi, software rendering), CPU load 31%–80% from other work.
- **Protocol:** 20 discarded warm-ups, 50 timed samples per op. Taps, screenshots, and typing alternated sample-by-sample with rotating start order; tree reads ran in 5 round-robin blocks of 10 because Android allows one `UiAutomation` client at a time. `phone-harness` ran in-process (sparing it Python startup).
- **Headline rule:** Report p50 ratio (10,000-resample bootstrap 95% CI, seed 7), or the lower CI bound if it drops below 90% of the ratio (so typing's 23.0x with a 19.7 lower bound is headlined 20x). The list read gave 193.1x on pass 1 and 170.0x on pass 2; I report the lower pass.
- **Raw files:** [speed-samples.csv](/benchmarks/data/speed-samples.csv), [speed.json](/benchmarks/data/speed.json), [speed-meta.json](/benchmarks/data/speed-meta.json), [`docs/bench/speed.md`]({{REPO}}/blob/main/docs/bench/speed.md), and `bench/speed/reproduce.sh`.
:::
