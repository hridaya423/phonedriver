# Latency profile (2026-11-07)

Method: traces now carry `phases` (ms) per request (`connect`, `prepare`, `resolve`, `input`, `settle`, `last_change_at`, `tree`, `stabilize`, `render`, `text`; commit d4789e9). Agent-side times came from temporary `Log.i` probes in `Control.answer` and `TextInput.paste` (not committed). Device: `emulator-5554` (Pixel_API_37) on win-ts, reached through `ssh -L 5038:127.0.0.1:5037`, host busy with other agents. Before/after builds were run interleaved with `bench/` (`PHONE_USE_BENCH_ONLY` filters ops). Samples are 30 to 90 per op.

**The tunnel dominates what the host sees.** The agent answers a `tree` op in 11 ms (p50) and a `power` op in 7 ms, yet the daemon's `tree` and `prepare` phases read 100 to 170 ms. Every device round trip costs about 100 ms here, so the fixes below remove round trips and sleeps, which also helps on a local USB or localhost adb, where each is smaller.

## Tap then settled diff (toggle on the Taps tab), before

36 settled taps, p50 / p95 ms, total 999 / 1567:

| phase | p50 | p95 | note |
|---|---|---|---|
| prepare (screen state) | 131 | 359 | one round trip; cached 1 s, but every window event clears the cache |
| input | 130 | 326 | agent inject (about 19 ms) plus transport |
| settle | 318 | 654 | quiet window 250 after the last frame; 17 of 36 taps never saw a frame over 128 pt2 and waited the whole 500 ms first-change window |
| tree (first read) | 166 | 544 | one round trip |
| stabilize | 254 | 480 | sleep 100, read, compare, again |
| render | 0.2 | 0.5 | host work is nothing |

Real work is input plus two tree reads. The rest is waiting: 250 ms quiet, a 100 ms sleep, and up to 500 ms for a frame that did not come.

## Changes

1. `d4789e9` phases in traces.
2. `e9270e4`-series settle change: while the screen is quiet but the quiet window is not over, the daemon reads the tree (starting 120 ms after the last significant frame, up to two reads). If the last two agree, the reply needs no read and no `stabilize` (unit test: reply at 350 ms, was 450 ms; same dialog/page tests stay green).
3. A tap whose frame never shows is detected by a tree read 100 ms in that differs from what the agent was last shown (counts as a window change). Only when the first-change window is 400 ms or longer, so typed text (150 ms) is unchanged. Test: reply under 450 ms, was 500 ms.
4. `stabilize` skips its 100 ms sleep when the tree it starts from was read at least 100 ms ago.

## After

Interleaved, same host, `tap to settled diff`: 12 alternations of 12 runs, base daemon 918 ms p50 / 2082 p95 (60 samples), new 843 / 3584 (60). A second set (90 samples each, whole bench): 860 / 1546 against 825 / 1601. The gain is about 40 to 75 ms p50 in this environment, inside the noise of a bare tap between two identical runs (108 against 288 ms p50). In a quiet sample of the new build the phases were input 36, prepare 0 to 130, settle 258 to 490, tree 0, stabilize 170 to 280 (when the two probe reads disagreed, because the theme change was still animating).

What did not help, with numbers: reading earlier than 120 ms after the last frame was not tried (it shortens the tree-lag evidence below what `stabilize` gave); with a 100 ms tunnel round trip only one probe read fits in the quiet window, so `stabilize` still ran in about half of the settled taps; the reduction there is the skipped sleep.

## Bare tap (settle off)

Host to reply 34 ms on a quiet machine: agent inject with wait-for-finish (D14) 19 ms, adb transport 7 ms, daemon 2 ms (M1). `prepare` adds one round trip when a window event cleared the awake cache. Not changed: `ASYNC` for the last event would drop the delivery check (D14) for about 15 ms.

## Type 100 characters

Agent p50: focused-field read 8, `getClip` 1, `setClip` 6, `KEYCODE_PASTE` inject 35, wait for the field text to change 104 (includes the inject), restore. Host sees 325 ms for the `text` phase and 0 to 130 for `prepare`; the rest is transport. Nothing cheap to cut on the agent: the wait is the app applying the paste (D-level clipboard guarantee, 1,000 ms ceiling only in WebViews).

## UI tree, Settings

Agent `tree` op p50 11 ms (Settings main, warm); the host phase is 166 ms, all round trip. No change.

## Cold start (fresh daemon to first `snapshot` reply)

Phases before (3 sample runs, jar already on the device), ms: spawn plus CLI 85, `adb devices` + one `getprop` per device (3 emulators), `ls` jar 606, `pkill` +530, shell stream open +507, `ready` +347, control hello +495, video +90, first frame +498, then settle 254 and first tree 150 to 350.

Changes: (a) the jar check, `pkill` and `app_process` start run in one shell (`[ -f jar ] || exit 99; pkill ...; exec app_process`), and the jar is pushed only when it exits 99; (b) control and video streams open concurrently; (c) resolving a named device skips the `getprop` per device, and the OS labels of unseen devices are fetched together.

Interleaved wall time of `phone-use --device emulator-5554 snapshot` after `daemon stop` (host busy, 8 rounds): before p50 6,750 ms, after (a)+(b) 4,970, after (a)+(b)+(c) 3,950. Earlier quieter run (6 rounds): 3,700 against 2,000 (that run's binary failed the agent start, so it is not counted). Round trips before the agent boots: 6 + number of devices; after: 3.

Not changed: the 500 ms wait for the first video frame in `AndroidDevice::connect` (screenshots wait for it lazily, but the first settle counts quiet from connect), and the 250 ms quiet window of the first snapshot.

## Also found

`open https://...` failed on main after the token-round merge (`"package": null` read by `org.json` as the string "null"); fixed in the same series.
