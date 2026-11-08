# Hidden display against the main display (2026-11-08)

`--display hidden` (decision D32) renders and reads a virtual display instead of the phone's own screen. This page measures whether that costs time.

## Verdict

Hidden is not slower. Across two runs of 40 samples per cell, the hidden p50 is within 4 ms of the main p50, or lower, in all six comparisons. The first run had a worse tail on the hidden snapshot (p95 424 ms against 205 ms); the repeat did not reproduce it (151.8 against 148.2).

## Machine and method

| | |
|---|---|
| Device | `emulator-5558`, Pixel Tablet image, Android 17 (API 37), 2560x1600 at 320 dpi, software rendering, on `win-ts` |
| Host | the Mac, reaching win-ts's adb server through `ssh -L`, so every number includes that round trip and is higher than the in-host rows of `docs/bench/speed.md` |
| Build | phonedriver 0.1.0 `--release`, agent jar from `android-agent/build.sh` |
| Also running | `emulator-5554` (a demo recording) and `emulator-5556`, untouched by this run |
| Script | `bench/speed/run_display.py`, one daemon, session `display` |

Per round the script runs a block on the main display, then a block on the hidden display. A block launches the testbed, discards 5 warm-up runs of each op and records 20 (0.3 s apart). Switching mode restarts the agent, so modes cannot interleave sample by sample; the blocks alternate main, hidden, main, hidden (2 rounds, 40 samples per cell) so a drifting machine hits both. Times are measured on the client side of the daemon socket. Ops: `snapshot` (no settle), `act tap` at one point (no settle), `screenshot`.

```
PHONEDRIVER_HOME=/private/tmp/pdh1 ADB_SERVER_SOCKET=tcp:127.0.0.1:5041 \
  python3 bench/speed/run_display.py --phonedriver target/release/phonedriver \
  --device emulator-5558 --out docs/bench/data/hidden-display-2026-11-08 --runs 20 --warmup 5 --rounds 2
```

## Results

p50 and p95 in ms, n = 40 each. Raw samples and summaries: `docs/bench/data/hidden-display-2026-11-08/` (`samples.csv`, `summary.csv`; `repeat/` is the second run).

| op | mode | run 1 p50 | run 1 p95 | run 2 p50 | run 2 p95 |
|---|---|---|---|---|---|
| snapshot | main | 129.4 | 205.2 | 67.0 | 148.2 |
| snapshot | hidden | 116.7 | 424.2 | 70.4 | 151.8 |
| tap | main | 258.4 | 764.9 | 224.8 | 463.2 |
| tap | hidden | 220.6 | 524.9 | 228.0 | 396.6 |
| screenshot | main | 15.9 | 108.0 | 9.6 | 11.6 |
| screenshot | hidden | 12.1 | 29.9 | 9.8 | 10.6 |

## Where the differences come from

- Run 1 ran while the host was busier: its main snapshot p50 is 129 ms against 67 ms in run 2. Two hidden snapshots of 869 and 1,029 ms, both in the first hidden block, made the p95. Without them the hidden p95 is 293 ms. They did not return in run 2, so they are treated as host noise, not a cost of the display.
- There is no reason for a systematic cost. The tree read asks for one display's windows (`getWindowsOnAllDisplays().get(id)`) instead of all of them, the screenshot reads the same frame store, and the encoder's input surface is the virtual display's own surface, with no copy of the main display.

## What this does not show

- A switch of mode costs a full agent start: 4 to 8 s for the first hidden call on this emulator (encoder at 2560x1600, display creation, first frame). Not in the table; D32 says so.
- Screenshots taken straight after a tap can show the frame before it, 0.4 to 1.6 s behind, in both modes (measured the same way on the main display). The frame store is read as it is. Not changed here.
- One emulator, software rendering, Android 17. A phone's GPU encoder and a maker's window manager may behave differently.
