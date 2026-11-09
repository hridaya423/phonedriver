# Speed, phonedriver against adb and phone-harness (2026-11-07)

The README chart comes from this run. Raw samples: `docs/bench/data/speed-2026-11-07/` (`samples.csv`, `meta.json`, `run.log`; `second-pass/` holds the three ops that were measured again; `tap-pass-3/`, `device-pass-3/` and `device-pass-4/` re-run the tap rows from main `a1b9eed`, and `guard-baseline/` is the same rows from `aa3f4c3`). The chart reads `docs/bench/speed.json`, which `bench/speed/stats.py` writes from the CSVs. To repeat the run: `PH_PYTHON=<python with phone-harness 0.3.3> bash bench/speed/reproduce.sh`.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../../.github/assets/speed-dark.svg">
    <img alt="Bar chart, log scale, p50 on an Android 17 emulator. phonedriver against plain adb and phone-harness 0.3.3. UI tree, 1,000-row list 98 times faster than adb (phone-harness 111 times); Screenshot 41 times faster than adb (phone-harness 49 times); UI tree, Settings 27 times faster than adb (phone-harness 30 times); Type 100 characters 19 times faster than adb (phone-harness 22 times); Tap, then what changed 4.6 times faster than adb (phone-harness not supported); Tap, 1.5 s apart 1.6 times faster than adb (phone-harness 4.7 times); Tap, back to back 2.1 times faster than adb (phone-harness 2.5 times)." src="../../.github/assets/speed-light.svg" width="100%">
  </picture>
</p>

The bar is p50 and the tick is p95. The headline chart in the README uses four of these seven rows.

## Verdict

phonedriver is faster than adb and phone-harness on every operation measured here. The ratio against adb runs from 1.5x (cold start; the tap 1.5 s apart is 1.6x) to 98x (reading a 1,000-row list); a tap sent back to back with others is 2.1x at p50 with a wide interval, 1.2 to 3.0. Against phone-harness it runs from 1.9x to 111x. The headline "up to 98x faster than adb" is the list read: 24.0 ms against 2,352.4 ms at p50, 95% CI on the ratio 92.2 to 106.0.

The bare tap is the weak row. Host to reply it is 117.6 ms when taps come back to back and 72.4 ms when they are 1.5 s apart, as an agent sends them. The first run of this page had 175.6 and 61.6 ms; the back-to-back gap was the tap guard (D27) reading the screen again behind the previous tap's ripple, fixed in `b1b43af` and `a1b9eed` (below). Measured on the device, from the host sending the tap to the testbed's touch listener logging it, phonedriver is 1.7x faster than adb (45.0 ms against 77.0 ms); this row moves by tens of milliseconds between runs (below).

## What happened to the 153x

The first run (`docs/bench/history/m1.md`) read the 1,000-row list in 15.5 ms against phone-harness's 2,370 ms, a ratio of 153. The later busy-machine run (`docs/bench/history/adb.md`) used a different baseline and a different load, and its chart showed 51x against adb with a tap that looked slower than phone-harness's. This run holds the three tools to one machine state and interleaves them. The list read is 24.0 ms against 2,653.7 ms (111x phone-harness, 98x adb). phonedriver's list read is 8 ms slower than in M1 and phone-harness's is 280 ms slower, so the ratio is lower than 153 on both counts. The 153 is not a current number and is not used anywhere.

## Machine and state

| | |
|---|---|
| Host | `win-ts`: Windows 11 10.0.26300, Intel Core i9-13900K, 64 GB. Every tool ran on this host against `emulator-5554`, with the default local adb server on `127.0.0.1:5037`. Nothing went through an ssh tunnel |
| Emulator | `emulator-5554`, `sdk_gphone64_x86_64`, `google/sdk_gphone64_x86_64/emu64xa:17/CE2A.260420.051.A3/16571990:userdebug/dev-keys`, Android 17 (API 37), 1080x2424 at 420 dpi, `-no-window -no-audio -no-snapshot -gpu swangle_indirect` (software rendering) |
| Also running | `emulator-5556` (API 26) and `emulator-5558` (tablet), both idle, plus the machine's ordinary background load (an IDE, Defender, other agents' shells) |
| Host CPU load | `Processor(_Total)` 5 samples, 1 s apart, before and after every op. Means: tap 15%, screenshot 14%, tap then what changed 21%, Settings tree 18%, list tree 26% (max 45%), type 32% (max 47%), cold start 39%. Second pass: tap, paced 13%, type 20%, cold start 16%. No `claude -p` or benchmark run was active on the Mac when either pass started |
| phonedriver | `0.1.0`, built from main `04b61a1` on the host (the three tap rows were re-run from main `a1b9eed`; see "What this does not show") (`cargo build --release -p phonedriver`, rustc 1.99.1), agent jar built with `android-agent/build.sh` |
| phone-harness | 0.3.3 in the isolated venv from M1 (CPython 3.14.7), helpers called in process, telemetry off (`DO_NOT_TRACK`, `PHONE_HARNESS_TELEMETRY=0`, `PHONE_HARNESS_NO_UPDATE_CHECK=1`) |
| adb | 37.0.1-15733141, one `adb` process per call |
| Testbed | `dev.phonedriver.testbed` built 2026-11-06. Later testbed changes touch only the web and checkout screens, not the screens used here (Taps, Settings, List, Text, Gestures) |

## Method

`bench/speed/run_speed.py` runs every tool in one process on the host.

- **Interleaved.** Taps, screenshots, typing and the device-side tap alternate tool by tool, sample by sample, with the start order rotating. The three tools that read the UI tree cannot run at once, because phonedriver's on-device agent holds the one UiAutomation connection that `uiautomator dump` needs (adb and phone-harness fail with "Killed" while it is up). For those ops (both tree reads and tap then what changed) the tools take turns in 5 round-robin blocks of 10 samples, and the phonedriver daemon and agent are stopped while the others run.
- **Warm-up.** 20 runs per tool and op, discarded. Later blocks discard 3.
- **Samples.** 50 per tool and op, plus 50 for phonedriver's settled snapshot and for its mixed-Unicode typing.
- **Statistics.** p50 and p95 per tool. The interval on each p50 ratio is a bootstrap 95% CI: each tool's samples resampled alone, 10,000 times, seed 7 (`bench/speed/stats.py`).
- **Headline rule.** A row's headline ratio is its p50 ratio, unless the lower bound of its interval is below 90% of the p50 ratio, in which case the lower bound is the headline. The list read's bound (92.2) is above 88.3, so its headline is the p50 ratio, 98x. Rows in the chart show p50 ratios.
- **Operations.** As in M1 and `docs/bench/history/adb.md`: tap at a point on the Taps tab (adb and phone-harness at the same pixel); screenshot to a file; tree read of the Settings main screen and of the List tab (phonedriver `snapshot --no-settle`; its settled snapshot is in the table); 100 ASCII characters typed into a text field (the same text for all three); tap on the Dark theme switch and learn what changed (adb: tap, one `uiautomator dump`, diff of node tuples, as in `docs/bench/history/adb.md`). phone-harness has no settle or diff, so that row has no phone-harness bar.
- **Device-side tap.** The Gestures tab logs each `ACTION_DOWN` to logcat (`adb logcat -v epoch`). Latency is the logged time minus the host's send time, with the emulator's clock offset measured through a persistent `adb shell` (offset 0.8122 s before, 0.8126 s after; round trip 15 ms, so each figure is good to about 8 ms). It does not use any reply from phonedriver.

## Results

p50 and p95 in milliseconds. `n` is the sample count.

| op | tool | n | p50 ms | p95 ms |
|---|---|---|---|---|
| tap | phonedriver | 50 | 117.6 | 321.2 |
| tap | phone-harness | 50 | 289.0 | 863.1 |
| tap | adb | 50 | 250.5 | 346.0 |
| screenshot | phonedriver | 50 | 11.6 | 22.5 |
| screenshot | phone-harness | 50 | 570.5 | 809.6 |
| screenshot | adb | 50 | 476.3 | 613.9 |
| tap, device side | phonedriver | 50 | 45.0 | 68.6 |
| tap, device side | phone-harness | 50 | 88.5 | 348.1 |
| tap, device side | adb | 50 | 77.0 | 89.6 |
| tap, then what changed | phonedriver | 50 | 586.9 | 704.8 |
| tap, then what changed | adb | 50 | 2725.8 | 2853.9 |
| tree, Settings | phonedriver | 50 | 89.3 | 189.0 |
| tree, Settings | phone-harness | 50 | 2700.0 | 2816.7 |
| tree, Settings | adb | 50 | 2433.5 | 2517.6 |
| tree, Settings | phonedriver, settled | 50 | 92.5 | 124.4 |
| tree, list | phonedriver | 50 | 24.0 | 36.6 |
| tree, list | phone-harness | 50 | 2653.7 | 2803.8 |
| tree, list | adb | 50 | 2352.4 | 2446.9 |
| tree, list | phonedriver, settled | 50 | 25.3 | 38.1 |
| type 100 characters | phonedriver | 50 | 251.1 | 395.0 |
| type 100 characters | phone-harness | 50 | 5523.3 | 6409.6 |
| type 100 characters | adb | 50 | 4840.9 | 5631.2 |
| type 100 characters | phonedriver, mixed Unicode | 50 | 225.0 | 385.7 |
| cold start, first screen read | phonedriver | 10 | 1620.5 | 1853.8 |
| cold start, first screen read | phone-harness | 10 | 2999.4 | 3107.1 |
| cold start, first screen read | adb | 10 | 2488.0 | 2584.4 |
| tap, paced | phonedriver | 50 | 72.4 | 103.7 |
| tap, paced | phone-harness | 50 | 342.9 | 422.1 |
| tap, paced | adb | 50 | 114.5 | 150.5 |
| type 100 characters (first pass) | phonedriver | 50 | 263.8 | 428.1 |
| type 100 characters (first pass) | phone-harness | 50 | 5972.1 | 7218.6 |
| type 100 characters (first pass) | adb | 50 | 5371.5 | 7159.9 |
| type 100 characters (first pass) | phonedriver, mixed Unicode | 50 | 245.3 | 387.3 |
| tap (first pass) | phonedriver | 50 | 175.6 | 229.7 |
| tap (first pass) | phone-harness | 50 | 232.5 | 684.1 |
| tap (first pass) | adb | 50 | 199.5 | 272.3 |
| tap, paced (first pass) | phonedriver | 50 | 61.6 | 78.7 |
| tap, paced (first pass) | phone-harness | 50 | 319.7 | 359.5 |
| tap, paced (first pass) | adb | 50 | 106.6 | 134.4 |
| tap, device side (first pass) | phonedriver | 50 | 27.5 | 59.3 |
| tap, device side (first pass) | phone-harness | 50 | 76.6 | 290.0 |
| tap, device side (first pass) | adb | 50 | 71.2 | 88.3 |
| cold start, first screen read (first pass) | phonedriver | 10 | 1807.8 | 1881.4 |
| cold start, first screen read (first pass) | phone-harness | 10 | 3232.4 | 3305.7 |
| cold start, first screen read (first pass) | adb | 10 | 2556.7 | 2617.0 |

Ratios of p50s, with the bootstrap 95% CI:

| op | adb / phonedriver (p50) | 95% CI | phone-harness / phonedriver (p50) | 95% CI |
|---|---|---|---|---|
| tap | 2.1x | 1.2 to 3.0 | 2.5x | 1.4 to 3.4 |
| screenshot | 41.0x | 39.1 to 43.5 | 49.2x | 47.5 to 51.4 |
| tap, device side | 1.7x | 1.6 to 2.2 | 2.0x | 1.8 to 2.7 |
| tap, then what changed | 4.6x | 4.5 to 4.7 | not supported |  |
| tree, Settings | 27.2x | 25.6 to 27.8 | 30.2x | 28.5 to 30.8 |
| tree, list | 98.1x | 92.2 to 106.0 | 110.7x | 104.1 to 119.5 |
| type 100 characters | 19.3x | 17.1 to 22.4 | 22.0x | 19.4 to 25.2 |
| cold start, first screen read | 1.5x | 1.4 to 1.6 | 1.9x | 1.7 to 1.9 |
| tap, paced | 1.6x | 1.5 to 1.7 | 4.7x | 4.4 to 5.2 |
| type 100 characters (first pass) | 20.4x | 18.2 to 23.4 | 22.6x | 20.2 to 26.8 |
| tap (first pass) | 1.1x | 1.1 to 1.3 | 1.3x | 1.2 to 1.5 |
| tap, paced (first pass) | 1.7x | 1.6 to 1.9 | 5.2x | 4.7 to 5.6 |
| tap, device side (first pass) | 2.6x | 2.2 to 2.9 | 2.8x | 2.4 to 3.1 |
| cold start, first screen read (first pass) | 1.4x | 1.4 to 1.5 | 1.8x | 1.8 to 1.9 |


Typing 100 ASCII characters ran in both passes; the table shows the second (host load 20% against 32%), and the first pass is in the CSV and in the "(first pass)" rows. phonedriver typed 100 mixed-script characters (ASCII, accents, CJK, Arabic, emoji) in 225.0 ms at p50, which adb and phone-harness cannot type at all (`input text` raises on non-ASCII).

phonedriver's settled snapshot, the call an agent makes, costs about the same as the no-settle read: 92.5 ms against 89.3 ms (Settings) and 25.3 ms against 24.0 ms (list).

## Cold start

From a stopped daemon to the first screen read, 10 rounds, alternating tools (second pass): phonedriver 1,620.5 ms (`daemon stop`, then one CLI `snapshot`, which starts the daemon, finds the agent jar on the device and boots the agent), phone-harness 2,999.4 ms (a fresh Python process, import, first `screen()`), adb 2,488.0 ms (one `uiautomator dump` against the already running adb server). adb has no cold start of its own, so its figure is the same call as the Settings tree row; the comparison is for scale, not a like-for-like. First pass: 1,807.8, 3,232.4 and 2,556.7 ms.

## A physical phone

On 2026-11-09, I ran a smaller pass on a HONOR X8 (TFY-LX2, MagicOS, Android 13, USB). The build was `main` at `aa1f9d3`, in release mode. The phone was on the Display & brightness page in Settings. Each phonedriver row has 30 samples after 3 warm-ups, through the CLI: a new process per call, talking to a running daemon. Each adb row has 20 samples after 2 warm-ups, run after `daemon stop`, so the two tools never ran at once.

| Operation | phonedriver p50 | p95 | adb p50 | p95 | Ratio |
|---|---|---|---|---|---|
| Read the screen (`snapshot --no-settle` against `uiautomator dump /dev/tty`) | 78.5 ms | 128.5 ms | 2,506.6 ms | 2,583.9 ms | 32x |
| Screenshot (`screenshot -o` against `screencap -p`) | 16.4 ms | 21.4 ms | 1,005.7 ms | 1,097.6 ms | 61x |

The screen read was 461 bytes against 25,334 bytes of XML, 55 times smaller. On the main Settings page, `uiautomator dump` failed with `ERROR: could not get idle state.` after 11.5 s, because the page never goes idle, while phonedriver read it in 110 ms. The phone is slower than the emulator host, so phonedriver's own read takes about three times as long as on the emulator (78.5 ms against 24 ms). adb's dump takes about the same time on both.

## What this does not show

- Taps. The first run of this page had back to back at 175.6 ms (first-pass rows) and paced at 61.6 ms. The extra 110 ms came from the tap guard (D27, added after M1): a point tap read the whole tree before injecting, and a read right after a tap queues behind the button's ripple on the app's UI thread (94 ms p50 back to back against 33 ms at rest on the API 26 emulator, from the per-phase trace). `b1b43af` reuses the screen the previous point tap's guard read for up to 1 s; `a1b9eed` stops a ripple (1,000 to 4,500 pt2) from dropping it, so only a window change or a repaint of a quarter of the screen does. Same session, same host, before and after on emulator-5554 (pass `guard-baseline/` is `aa3f4c3`, passes `tap-pass-3/` and `device-pass-3/` are main `a1b9eed`; each 50 samples after 20 warm-ups, interleaved with adb and phone-harness): back to back 220.8 ms before, 117.6 ms after; paced 68.8 before, 72.4 after (within noise: the 1.5 s gap is past the 1 s reuse, so it still reads); host load means 20 to 45% before (the paced block ran at 27 and 45%), 15 to 18% after, so the before paced row is the less trustworthy one. The 117.6 ms is not at paced: its per-phase trace has `input` at 73 ms against 26 ms paced, the agent waiting (D14) for the app to finish handling the event while the app is still painting the ripples of the adb and phone-harness taps sent just before it. D14 stays. On the API 26 emulator with phonedriver alone, back to back went from 126-151 ms to 20 ms. The device-side row (host send to the app's logged touch) is not stable enough to show a change: 28.0 ms before, 45.0 and 4.5 ms in two runs after (`device-pass-3/`, `device-pass-4/`), with adb and phone-harness steady at 62 to 88 ms; a tap that reaches the app while it still paints the previous ripple is logged late, and the clock offset is good to 8 ms at best. A 4.5 ms latency is below what the path can take, so that run's offset is off. Do not read a gain or a loss from that row. The earlier note, 35 taps with phonedriver alone before the fix: p50 179.7 ms back to back, 127.8 ms with 0.5 s between taps, 56.7 ms with 1.5 s between.
- Host load was 10% to 47% during the first pass, from other work on the host. Typing, the one op measured at two load levels, gave close ratios against adb in both passes (20.4x at 32% load, 19.3x at 20%), and absolute times move with load (phone-harness's 100-character typing took 2,104.5 ms in M1 and 5,523.3 ms here; adb's 4,840.9 ms is in line with `docs/bench/history/adb.md`). Absolute numbers are for this host.
- The main pass is emulator only. The physical-phone pass above covers two operations, not typing or taps, and no iOS device is in this run.
- adb returns raw XML and a tap that returns before the app reacts. phonedriver returns a pruned, numbered tree and waits for the app. Different outputs, same job for an agent.
