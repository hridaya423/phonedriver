# Two devices at once (task 7.5)

One daemon, `PHONEDRIVER_HOME=/tmp/.../home75c`, two `phonedriver-bench scenario` clients at the same time, each with its own session (`scenario-android:<serial>`) and its own emulator: `Pixel_Tablet_API_37` (`emulator-5558`, 1280x800 pt, landscape, API 37) and `Pixel_API_26` (`emulator-5556`, API 26), both on win-pc behind one adb server and one SSH tunnel.

Scenarios: 15, not the 21 of M2. The six left out do not run on a tablet or are first-run app flows that need a quiet device (the phone-axis rotation scenario, Chrome, Clock, Files, `testbed-busy`, `testbed-web-form`; see `docs/spikes/tablets.md`).

## Result (one concurrent run, 2026-11-06)

| Client | Device | Passes | Wall time |
|---|---|---|---|
| A | emulator-5558 (API 37 tablet) | 15 of 15 | about 3 min |
| B | emulator-5556 (API 26) | 15 of 15 | about 3 min |

Two earlier concurrent runs are not counted: the first had `testbed-secure` fail on API 26 (`secure content hidden` absent; this scenario already failed alone on API 26 in M2 and is not explained), and the second stopped when the API 26 emulator hung (adb shell silent); I restarted it and ran again. The table is the third run. I did not repeat it.

No cross-talk: the trace of client A has 92 requests and all name `android:emulator-5558`; client B's has 79, all `android:emulator-5556`. The third trace file (session `default`) belongs to the single client in the next paragraph and its `hello` handshakes.

One client driving both with `--device`, six calls alternating `snapshot --no-settle` and `press home` between the two: each reply's header named the device asked for, and its trace file shows 6 requests for each device.

## Daemon CPU and memory (one run, `ps` every 4 s while both clients ran, 50 samples)

| | p50 | p95 | max |
|---|---|---|---|
| CPU, percent of one core | 101 | 201 | 277 |
| Resident memory, MB | 159 | - | 175 (minimum 125) |

The CPU is high: the video decoders for two streams (the tablet's is 2560x1600) and the scenarios' own requests share the daemon. Per-stream cost was not separated; with one stream only I did not measure it.
