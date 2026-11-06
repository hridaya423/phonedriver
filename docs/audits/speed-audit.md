# Speed and impressiveness audit (2026-11-06)

Numbers: `docs/bench/adb.md` (phone-use main `d6d9155` with the physical-iOS source removed in the win-pc copy, back to back with adb, busy machine), `docs/bench/m1.md`, `m4.md`, `m5.md`. Estimates are marked as estimates; nothing below was implemented.

## 1. How fast, against adb and phone-harness

Slower of two phone-use runs, p50, Android 17 emulator:

| op | phone-use | adb | phone-harness (M1, quieter) |
|---|---|---|---|
| tap | 144.1 | 223.7 | 110.7 |
| screenshot | 45.3 | 692.4 | 390.1 |
| UI tree, Settings | 237.0 | 2800.5 | 2426.4 |
| UI tree, List | 52.8 | 2677.9 | 2370.0 |
| type 100 chars | 755.0 | 5204.4 | 2104.5 |
| tap then what changed | 666.9 | 3111.7 | n/a |

phone-use is 4.7x to 51x faster than adb on everything an agent does after a tap, and 1.6x on the bare tap. Against phone-harness the M1 run (quiet machine) gave 3x to 153x; phone-harness's own tap is not slower than phone-use's on a busy machine. iOS Simulator screenshot is 3.6x (CLI) to 4.9x (socket) faster than `simctl io screenshot`.

Can it be made faster? Yes, on three fronts, and one finding is a bug.

## 2. Where the time goes

Traces (D16) hold only `ms` and `settle_ms` per request, no phase timing. In the Mac mini's traces, 84 iOS `act tap` calls with settle: `ms` p50 1069, `settle_ms` p50 502, so about 560 ms is not settle (tap 357 plus the snapshot and diff). Android phase numbers below are from M1's agent-side probes (`m1.md`), which were temporary.

- **tap, Android (M1 30 to 37 ms; 15 ms budget):** agent `WAIT_FOR_FINISH` 18.9 ms (app handling the event on a software renderer), adb transport about 7 ms, daemon plus pipe plus JSON about 2 ms.
- **tap to settled diff (420 to 670 ms):** settle waits at least `quiet` = 250 ms after the last significant frame (`crates/core/src/settle.rs`), and a static emulator screen emits about 3 frames a second, so the end is seen up to a frame late; then a 60 ms tree read.
- **snapshot (60 to 68 ms in M1):** about 50 ms is `getChild` binder round trips for 155 nodes (one per 2 to 4 nodes); JSON 1.5 ms; host prune and render 8 to 15 ms.
- **type 100 (253 to 288 ms in M1, 755 ms busy):** `getClip`, focused-field read, `setClip`, `KEYCODE_PASTE` inject with wait-for-finish (10 to 200 ms), then up to 500 ms waiting for the field text to change before restoring the clipboard (one run in three waits the full 500).
- **iOS Simulator tap (357 ms):** XCTest completion about 240 ms (D15), up to 80 ms waiting for an in-flight settle poll (runner screenshot 77 ms), about 35 ms the rest.
- **iOS device tap (454 to 534 ms):** the same, plus USB round trip and a device settle poll of 285 to 330 ms.
- **Every Windows call (found here):** +2.05 s, see speedup 1.

## 3. Ranked speedups (estimates, none implemented)

| # | Change | Estimated gain | Effort | Evidence |
|---|---|---|---|---|
| 1 | Resolve the request's device through the one source whose prefix matches (`android:`, `sim:`, `ios:`), and cache the physical-iOS listing like `SimSource` does (2 s TTL). Today `Sources::list` queries every source on every request. | Windows: -2.05 s on every call (measured: `devices` 2079 ms against 26 to 33 ms with the source removed; tap 2075 ms against 131 to 144). Mac: tens of ms per call on Android, unmeasured | S | `docs/bench/adb.md`, `crates/cli/src/daemon/sources.rs`, `commands.rs` |
| 2 | Shorten the settle quiet window when a frame arrived after the action (positive proof of a live stream) and the screen then stopped: 250 ms to about 120 ms. | -100 to -130 ms on every settled action (about 20% of a 420 to 670 ms tap-to-diff). Risk: early settle on bursty encoders, so it needs the 21 M2 scenarios as the gate | M | `settle.rs`, architecture 4.6 known limit |
| 3 | iOS tap: do not wait behind an in-flight settle poll (cancel it, or run it so a request preempts it). | -40 ms average, up to -80 ms, per iOS call | S to M | M4 "Waiting behind polls" |
| 4 | iOS Simulator: inject touch through SimulatorKit HID (the route idb uses) instead of XCTest synthesis. | Tap 357 ms to about 100 to 130 ms (removes the 240 ms completion); settled diff 712 ms to about 480 ms. Estimate, unmeasured | L (private API, Xcode-fragile, Simulator only) | D15, M4 |
| 5 | Type, Android: skip the pre-read of the focused field and the first `getClip` when not needed, and cut the 500 ms confirm to about 200 ms with a late verify. | p50 -50 to -100 ms; p95 about -400 ms (the 790 ms outliers are the 500 ms wait) | S to M, trades into D-level clipboard guarantee | M1 type profile |
| 6 | Tap `ASYNC` for the last event only when the call settles anyway (settle detection already covers delivery); keep wait-for-finish when settle is off. | Tap-only path unchanged; settled path about -14 ms. M1 measured ASYNC at 15.1 ms against 30 to 37 ms | S | D14, M1 |
| 7 | Snapshot: skip fetching subtrees that are clipped or off screen at the agent, before the binder walk. | -10 to -20 ms of 60 (estimate; the 155-node tree is mostly on screen, so smaller on Settings, larger on long lists) | M | M1 snapshot profile |
| 8 | Fix the CoreDevice stream freeze (iOS 27 devices), then use it for settle and screenshots and HID for taps. | See below. | L (root cause unknown after PLI, FIR and ack attempts) | architecture 7.2, spike I1 |
| 9 | Add per-phase timings to traces (resolve, input, settle, tree, render). | None directly; makes 2 to 7 measurable without temporary probes | S | traces carry only `ms` and `settle_ms` |

What fixing the CoreDevice stream buys on the iPad (estimate; HID tap latency was never measured, spike I1 says so): taps by HID are wire-bound, plausibly 20 to 60 ms against 454 ms; screenshots become the latest decoded frame, as on Android, about 20 to 40 ms against 332 ms; settle polls stop costing a 285 to 330 ms runner capture, so tap-to-diff falls from 1053 ms to about 400 to 500 ms (quiet window 250 ms plus frame gap plus tree read). Snapshot stays the runner's 254 ms. Only iOS 27 devices qualify (the iPhone 14 Plus on iOS 26.1 cannot start the stream).

Perspective: M3 puts a whole agent task at about 18 s and 6 tool calls, so tool time is a minority of it; model time dominates. The largest practical latency win for an agent is fewer calls and fewer tokens, which the settled diff already does. Speedups 1 and 2 matter most because every action pays them.

## 4. Making it impressive

Lead with, and only with, what is measured (chart in the README):

- **Lead number: "one call that returns what changed: 667 ms against 3.1 s for tap + dump + diff with adb."** It is the operation agents actually do, and it is the honest 4.7x. Support it with screenshot 15x and tree reads 12x to 51x.
- Say the tap number plainly: 1.6x over adb, below the 15 ms budget miss, not a headline.
- Do not lead with "153x faster than phone-harness": that is from the M1 List row on a quiet machine and phone-harness is a weaker baseline than adb.

Proposals, with what each needs:

| Idea | What it shows | Needs new measurement? |
|---|---|---|
| Token proxy per screen: bytes of `uiautomator` XML and of a raw screenshot against phone-use's snapshot text, same screens (the Taps dump is 16,380 bytes) | Context cost per observation, 5x to 20x smaller (estimate) | Yes, cheap, no model calls; report bytes and a stated chars-per-token approximation, not a tokenizer claim |
| Scripted race: the same fixed step list (the M2 scenario YAML) run through adb (`input tap` + `uiautomator dump` per step) and through phone-use, side by side, recorded as a GIF | Tool wall time without any model; M2 already has phone-use wall times for 21 scenarios (4.6 to 30 s) | Yes for the adb half; no model calls needed |
| Agent end-to-end time and tokens against other tools | M3 gives phone-use only: 15 of 15, median 6 calls, 42.8k tokens, 18 s, $0.36 for 15 runs | Yes, and it needs model runs against each tool; proposal only, not run |
| Tokens per task against a screenshot-loop agent | The big claim if true | Yes, model runs; proposal only |
| AndroidWorld score | A number the field recognizes | Yes, model runs, a later project |
| Physical phone numbers over USB | Removes the emulator and busy-machine caveat; the tap budget may be met on hardware | Yes, needs a phone |
| `phone-use-bench compare` command and CSVs in the README ("run it yourself") | Reproducibility is itself impressive | S, no measurement |
| iOS: simctl screenshot 3.6x to 4.9x (measured), and tap after speedup 4 | A second platform in the chart | Partly measured |

## 5. Open items from this audit

- Re-run `docs/bench/adb.md` on a quiet win-pc with phone-harness in the same session; the tap regression question (130 to 144 ms against M1's 30 to 37 ms) is unresolved.
- Speedup 1 is a real bug on Windows today: every request is 2 s slow with the default daemon. It was not fixed here (no crate edits).
- win-pc's C: drive is full and its sshd was resetting connections; that is not caused by this work but it cost several runs.
