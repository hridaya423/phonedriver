---
title: How phonedriver reads a phone's screen 98x faster than adb
short: Reading a screen 98x faster than adb
meta: How phonedriver reads a 1,000-row list in 24.0 ms against adb's 2,352.4 ms: the on-device agent, H.264 frames, settle detection, measured fixes, and the method.
description: The on-device agent, UiAutomation, an H.264 stream and frame-change detection behind phonedriver's 24.0 ms read of a 1,000-row list against 2,352.4 ms for adb, the fixes we measured, and how we measured them.
date: 2026-11-07
label: ENGINEERING
short_card: How phonedriver reads a screen 98x faster than adb
card_sub: The on-device agent, the video stream, settle detection and the method.
---

On an Android 17 emulator, phonedriver reads a 1,000-row list in 24.0 ms. `adb shell uiautomator dump` takes 2,352.4 ms for the same screen. At p50 of 50 runs that is 98 times faster, with a 95% confidence interval on the ratio of 92.2 to 106.0.

The number needs two warnings before anything else. It is the largest ratio of the seven rows we measured, which is why it is the headline; the median ratio across the seven rows is 19. And it compares two different outputs: adb returns raw XML, phonedriver returns a pruned, numbered text tree. This post explains where the time goes, what we built to avoid paying it, which fixes we measured and which did not help, and how the comparison was run so you can repeat it.

## What adb does on every read

An agent that drives a phone with adb pays the same costs on every look. `adb shell input tap` starts an `adb` client process on the host and an `app_process` JVM on the phone for each tap. `uiautomator dump` waits for the UI to go idle before it reads the tree, and it refuses a screen that keeps animating. We saw it fail with "could not get idle state" on the Contacts list. `screencap -p` PNG-encodes the full-resolution frame on the phone for every screenshot. `input text` handles only ASCII.

We read the phone-harness source at commit `4bebff1` to confirm this for the tool that wraps adb this way (`android.py` lines 380, 430, 493 and 677 to 711 in that commit), and it matches what we see from adb itself. We did not split adb's 2.35 s between the process start and the idle wait, so we do not say how much each costs. The speed comes from not doing either per call, and the rest of this post is how.

## A resident agent instead of a command per call

Decision D2 in our design record: push a dex jar to the phone and run it with `app_process` as the shell user, the way scrcpy does. The agent stays up. It injects input through `InputManager`, reads the UI tree through a UiAutomation connection it holds open, mirrors the display into a MediaCodec encoder, and keeps one socket open to the host. A daemon on the host keeps the other end of that socket, so a CLI call or an MCP tool call is a message on an open connection.

Nothing is installed on the phone, and no accessibility service has to be switched on. We rejected an installed AccessibilityService APK because the user has to enable it in Settings, it stays on the phone as an app, and it cannot inject input without extra permission. We rejected an instrumentation APK (the uiautomator2 and Appium route) because it needs installation, test-APK signing and still pays instrumentation overhead.

On the phone, the agent answers a tree request in 11 ms at p50 for the Settings main screen, warm, from probes we added temporarily (`docs/audits/latency-profile-2026-11-07.md`). About 50 ms of an earlier 60 ms snapshot was `getChild` binder round trips for 155 nodes, one round trip per 2 to 4 nodes. Host-side pruning and rendering were 8 to 15 ms. In the benchmark, with every tool run on the emulator's own host, the Settings read is 89.3 ms and the 1,000-row list is 24.0 ms.

## A video stream for screenshots

D10: keep an H.264 stream running while a device is in use. The encoder runs on the phone, and the host decodes it with `openh264`. A screenshot is the latest decoded frame, not a new capture. That is why a screenshot takes 11.6 ms against 476.3 ms for `adb exec-out screencap`.

The stream is not free. It costs one encoder on the phone and one software decoder on the host, which we measured at up to about one CPU core at 1080p30. Decoding a 1080-wide frame took 3.6 to 4.6 ms at p50 and 7.3 to 9.6 ms at p95 on the Mac. The first decoded frame after opening the stream took 219 ms (API 26) and 433 ms (API 37) at p50, which missed our 200 ms target; the software encoder takes 140 to 230 ms to start, and that cost is paid once per device lease.

One assumption was wrong. We first wrote that the stream sends frames at a steady rate on a static screen. On the API 37 emulator through an ssh tunnel it sent bursts of about 10 frames per second and then nothing for up to 5 s, and "no frame for 1 s" made 18 of 40 screenshots of an idle screen fail. Screenshots now serve the last frame unless the video socket has ended. Settle is unaffected, because it needs frames only when the screen changes.

## Settle: knowing the screen has stopped

The stream does a second job. After every action, phonedriver waits until the screen stops changing in a way that matters, then returns what changed. The agent never sleeps or polls. Settle takes two kinds of input:

- A window event from accessibility (`TYPE_WINDOW_STATE_CHANGED`, `TYPE_WINDOWS_CHANGED`): a new activity, dialog, popup or keyboard.
- A decoded frame, compared with the previous one by a luma thumbnail with one sample per 4 by 4 points. A sample counts as changed above 12 luma levels, and the changed area is the changed samples times 16.

Accessibility content-change events are deliberately not an input. Apps emit them for invisible changes and for tickers: a clock label that updates every 100 ms would never settle. Pixels are the ground truth for "the screen moved".

The detector is a state machine with two phases. First it waits up to 500 ms (3 s after `launch` or `open`) for a first change: a window event, or a frame that changes more than 128 pt². Only then does it wait for quiet: no window event and no frame above 4,096 pt² for 250 ms.

The two thresholds come from a spike, 20 runs per case on API 26 and 37. A blinking caret changes at most 96 pt², an indeterminate spinner at most 480 pt², a Material switch toggle at most 432 pt², and a screen transition 13,000 to 254,000 pt². One threshold of 4,096 pt² would never see a toggle and would report that a tap that worked changed nothing. 128 pt² is just above the caret, so a toggle counts as a first change, and 4,096 pt² for quiet ignores the spinner and the caret.

The known limit is written down next to the code. A video stream that delivers frames in bursts, such as the software encoder on the emulators during sustained full-screen motion, can show gaps over 250 ms in the middle of an animation, and the screen is then reported settled early. Phones with hardware encoders have not been measured. At the timeout (5 s for actions) the detector returns what it has with `settled: false` and the regions still changing. Looping video and carousels never settle, and the agent still needs to see the screen.

## What comes back

The reply is the diff, not the screen. Each line has a ref that the daemon keeps pointing at the same element while it is on screen:

```screen
ok tapped @e10 "Dark theme" · settled in 410ms
~ @e10 switch "Dark theme" off → on
```

`+`, `-` and `~` mark lines that appeared, left and changed. Refs are located again at tap time, so a list that shifted by a row still gets the right tap. This is also where the token savings in [our token benchmark](/benchmarks/#tokens) come from: the agent reads a few lines after each action, not a screen.

## The fixes we measured

We profiled before we tuned. `docs/audits/latency-profile-2026-11-07.md` records each phase of a settled tap on the emulator through an ssh tunnel, where every device round trip costs about 100 ms. Before any change, a tap with a settled diff took 999 ms at p50: 131 ms to prepare the screen state, 130 ms for input, 318 ms of settle, 166 ms for the first tree read and 254 ms in a stabilize step that sleeps 100 ms, reads and compares twice. Host work (render) was 0.2 ms. Real work was input plus two tree reads. The rest was waiting, including 17 of 36 taps that never saw a frame above 128 pt² and sat out the whole 500 ms first-change window.

Three changes followed, after a fourth that added per-phase timings to the traces. While the screen is quiet but the quiet window is not over, the daemon now reads the tree starting 120 ms after the last significant frame, and if two reads agree it needs no stabilize step. A tap whose frame never shows is caught by a tree read 100 ms in that differs from what the agent last saw. Stabilize skips its 100 ms sleep when the tree it starts from was read at least 100 ms ago.

The measured gain was small. Interleaved, same host, 12 alternations of 12 runs: 918 ms at p50 before, 843 ms after. A second set of 90 samples each gave 860 against 825. That is 40 to 75 ms, inside the noise of a bare tap between two identical builds (108 against 288 ms at p50). We kept the changes, because they remove round trips and sleeps that cost more on a local connection. We also wrote down what did not help: reading earlier than 120 ms after the last frame was not tried, and with a 100 ms round trip only one probe read fits in the quiet window, so stabilize still ran in about half of the settled taps.

Other fixes were bigger because they removed a mistake rather than tuned a path:

- Cold start, fresh daemon to first snapshot, dropped from 6,750 ms to 3,950 ms at p50 on a busy host through the tunnel, by checking for the jar, killing a stale agent and starting a new one in a single shell, opening the control and video streams at once, and not asking each device for its properties. Round trips before the agent boots went from 6 plus the number of devices to 3. In the final benchmark, on the host itself, cold start is 1,620.5 ms.
- A cold app swallowed the first tap, we thought. We were wrong. The tap landed; Clock opened its time picker 0.6 to 1.1 s later, after the 500 ms first-change window, so the reply said `no change after 507ms` and the agent tapped again, onto the dialog. Actions in the 30 s after `launch` now wait up to 2 s for their first change. The reply said `no change` in 3 of 8 runs before and in 0 of 8 after.
- Point taps in a row. A guard check reads the screen before a tap, and a read right after a tap queues behind the button's ripple on the app's UI thread: 94 ms at p50 back to back against 33 ms at rest. Reusing the screen the last check read for up to a second brought the tap reply from 126 ms to 20 ms on the API 26 emulator. The limit is that a change smaller than a significant repaint that adds a guarded button in that second is missed ([the approval guard post](/blog/letting-claude-order-dinner-without-letting-it-spend-your-money/) says what that means).

## How we measured the 98x

The first benchmark, M1, quoted 153 times against phone-harness for the same list read: 15.5 ms against 2,370 ms. A later run on a busy machine used a different baseline and a different load. Neither held the tools to one machine state. We ran it again.

`bench/speed/run_speed.py` runs every tool in one process on one host, `win-ts` (Windows 11, Intel Core i9-13900K, 64 GB), against `emulator-5554`, an API 37 emulator at 1080x2424 and 420 dpi, headless with software rendering, with the default local adb server and no tunnel. Mean host CPU load was 13% to 39% per operation from other work on the machine. We recorded load before and after every operation.

- Taps, screenshots and typing alternate tool by tool, sample by sample, with the start order rotating.
- The tree reads cannot run at once, because phonedriver's agent holds the one UiAutomation connection that `uiautomator dump` needs. For those operations the tools take turns in five round-robin blocks of 10 samples, and phonedriver's daemon and agent are stopped while the others run.
- 20 warm-up runs per tool and operation, discarded. 50 samples each.
- p50 and p95 per tool. The interval on each p50 ratio is a bootstrap 95% CI, resampling each tool's samples alone 10,000 times with seed 7.
- A row's headline is its p50 ratio, unless the interval's lower bound is below 90% of it, in which case the bound is the headline. The list read's bound (92.2) is above 88.3, so 98x stands.
- The tap on the device is also timed from the other side: the Gestures tab logs each `ACTION_DOWN` to logcat, and latency is the logged time minus the host's send time with the emulator's clock offset measured (0.8122 s before, 0.8126 s after; good to about 8 ms). That row does not use any reply from phonedriver.

phone-harness ran in process, which spares it a Python start per call and favours it. The raw samples, the machine description and the script that regenerates every table are linked from [the benchmarks page](/benchmarks/#data), and `bench/speed/reproduce.sh` repeats the run.

## What the numbers do not show

- **The bare tap.** This is the weak row. Host to reply it is 175.6 ms when taps come back to back (1.1 times faster than adb) and 61.6 ms when they are 1.5 s apart (1.7 times). On the device, from send to the app's touch listener, phonedriver takes 27.5 ms against adb's 71.2 ms. Our tap misses its 15 ms budget, and typing 100 characters misses its 150 ms budget at 251.1 ms.
- **Different outputs.** adb's tap returns before the app reacts. phonedriver's waits. For "tap, then what changed" the adb row is a tap, a dump and a diff in the caller: 2,725.8 ms against 586.9 ms, 4.6 times.
- **Emulator only.** No physical Android phone is in this run, and no iOS device. On the iOS 26.5 Simulator a tap takes 357 ms against 80 ms for the Simulator's own tool, because XCTest takes about 240 ms to confirm an event. On an iPhone 14 Plus and an iPad Pro it takes 534 and 454 ms.
- **Absolute times move with load.** Typing 100 characters ran in two passes at different host load, and gave 20.4 times (32% load) and 19.3 times (20%). phone-harness's 100-character typing took 2,104.5 ms in M1 and 5,523.3 ms here. Absolute numbers are for this host.
- **Model time dominates a task.** An agent task in our earlier evaluation took about 18 s and 6 tool calls. Faster tools shorten the part that is not the model. Fewer calls and fewer tokens matter more, and the settled diff is for that.

Method, machine state, both passes and every row are in [`docs/bench/speed-2026-11-07.md`]({{REPO}}/blob/main/docs/bench/speed-2026-11-07.md). If you find a flaw in the setup, [open an issue]({{REPO}}/issues) with the run that shows it.
