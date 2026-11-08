# phonedriver against plain adb (2026-11-06)

Same method as M1 (`docs/bench/history/m1.md`): win-pc, `emulator-5554` (Pixel_API_37, Android 17, 1080x2424, headless, `-gpu swangle_indirect`), the machine's own adb server, release build, 50 runs after 5 warm-ups, the M1 testbed screens (Taps, Settings main, List tab, Text tab). Raw samples are in `docs/bench/data/adb-*.csv`.

## Which phonedriver

Built on win-pc from main `d6d9155` with **one local change in the win-pc copy only**: the line that registers the physical-iOS source (`("ios:", Arc::new(PhysicalSource::new(...)))`, `crates/cli/src/commands.rs`) was deleted. As shipped, the daemon lists devices on every request through every source, and on Windows (no usbmuxd) the physical-iOS source costs 2.05 s each time. Measured with the unpatched build (main `3408cbb`, `adb-run-phonedriver-physical-source.csv`): `phonedriver devices` 2,079 ms against 26 to 33 ms patched, and every op 2.0 s slower (tap 2075.1, screenshot 2059.3, snapshot Settings 2556.2, snapshot List 2129.2, tap to settled diff 2857.2, type 2525.6 ms p50). The rows below are the patched build. The repo was not changed.

## Conditions

The machine was busy: other agents' emulators (four qemu processes), WSL, Lens Studio and many ssh sessions, CPU load 42% or more, C: full (0 MB free, so the benchmark's temp directory went to D:). Expect noise and absolute numbers worse than M1. phonedriver A and B bracket the adb run (A, adb, B), so the comparison is paired. The phone-harness re-run on the same session did not finish (win-pc went down); the phone-harness column is M1's, taken on a quieter machine.

## Results, p50 / p95 / max ms

| op | adb | phonedriver run A | phonedriver run B | phone-harness (M1) |
|---|---|---|---|---|
| floor, `adb shell true` | 71.5 / 151.2 / 165.4 | | | |
| tap (point) | 223.7 / 325.9 / 427.2 | 130.6 / 225.2 / 257.3 | 144.1 / 220.7 / 241.8 | 110.7 |
| screenshot (Taps tab, to disk) | 692.4 / 746.4 / 768.9 | 42.9 / 56.2 / 78.6 | 45.3 / 71.8 / 99.1 | 390.1 |
| UI tree, Settings main | 2800.5 / 2885.5 / 2933.4 (`exec-out`); 2895.4 / 3036.2 / 3907.1 (dump + pull) | 159.1 / 267.2 / 738.1 (`--no-settle`); 129.8 settled | 237.0 / 376.4 / 403.4; 205.1 settled | 2426.4 |
| UI tree, List tab | 2677.9 / 2800.0 / 3114.2 | 36.0 / 54.6 / 72.6 | 52.8 / 75.9 / 93.9 | 2370.0 |
| type 100 chars | 5204.4 / 6002.3 / 14248.5 (ASCII) | 612.8 (mixed Unicode) | 755.0 (mixed Unicode) | 2104.5 (ASCII) |
| tap, then know what changed | 3111.7 / 3250.0 / 3483.6 (one dump); 5725.5 / 6594.1 / 38213.3 (dump until two agree) | 602.0 / 823.8 / 883.3 | 666.9 / 849.5 / 895.3 | not supported |

Ratios, adb over phonedriver, using the slower phonedriver run (as M1 does): tap 1.6x, screenshot 15x, UI tree Settings 12x (`--no-settle`), List 51x, type 6.9x, tap then what changed 4.7x (8.6x against the dump-until-stable variant).

## How adb was driven (`bench/adb-baseline/run_adb.py`)

One adb client process per call, Python `subprocess`, timed spawn to exit with the output read: tap `shell input tap`; screenshot `exec-out screencap -p` written to a file; tree `exec-out uiautomator dump /dev/tty` (and `shell uiautomator dump` + `pull`); typing `shell input text` with `%s` for spaces; the composite is a tap, a dump, and a diff of node tuples (class, id, text, bounds, checked, selected, enabled, focused) in the caller, asserted non-empty. The second composite repeats the dump until two agree, which is what a caller needs to avoid reading mid-animation. The harness did not fit `phonedriver-bench` (it times a daemon socket, not processes), so it is a stdlib script.

## Fairness caveats

- adb's tree is raw XML (the Settings dump is about 16 KB for the Taps tab alone); phonedriver's is pruned, ref-numbered text. Different outputs, same job for an agent.
- `input tap` returns before the app reacts; phonedriver's tap waits for the app to handle the event (D14). adb's tap is therefore the less strict operation and still slower.
- `uiautomator dump` waits for the UI to go idle, which is most of its 2.8 s. That is also why it is a poor way to learn what changed.
- adb's per-call floor is a process start plus transport (71.5 ms here); phonedriver holds a daemon and an on-device agent, so it avoids that cost. A Python agent calling adb through a long-lived `adb` connection (adbutils, a socket client) would avoid the process start but not `input` or `uiautomator`'s own start (an app_process JVM per call).
- adb and phone-harness type ASCII only; phonedriver typed mixed-script Unicode. `input text` fails on Unicode (M1).
- Run to run spread is visible: phonedriver A and B differ by 10 to 50% on this busy machine. The one `dump until two agree` sample of 38 s is an emulator stall.
- The tap row is a regression risk to check: 130 to 144 ms here against 30 to 37 ms in M1. Load explains much of it (adb's own tap is 224 ms against about 110 ms for phone-harness's tap in M1), but main moved from `d29a48e` to `d6d9155` between the two runs and the two were not separated. Re-run on a quiet machine before quoting a tap number.

## iOS Simulator (Mac mini, iPhone 17, iOS 26.5, no lease held)

`xcrun simctl io screenshot`, 50 runs after 5 warm-ups, one process per call: PNG 406.9 / 416.6 / 420.9 ms, JPEG 383.1 / 390.9 / 693.9 ms. `phonedriver screenshot` through the CLI (process start included; an already-running daemon, build `target-fix`): 112.1 / 197.0 / 251.3 ms. Through the daemon socket M4 measured 83 to 84 ms. simctl has no tap, tree or typing, so those rows have no adb equivalent. About 3.6x on screenshots from the CLI, 4.9x through the socket.

## Not done

- phone-harness re-run in the same session (win-pc went down mid-run).
- A quiet-machine re-run of everything.
- Physical devices.
