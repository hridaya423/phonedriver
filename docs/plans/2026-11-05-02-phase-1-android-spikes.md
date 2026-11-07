# Phase 1: Android spikes

> **For Claude:** REQUIRED SUB-SKILL: Use `executing-plans`. Spikes are throwaway: no tests required, numbers required.

**Goal:** Prove every Android mechanism in architecture section 6 works on API 37 and API 26 before production code depends on it, and measure each one.

**Architecture:** A minimal Java agent in `spikes/android-agent/` and a minimal Rust host in `spikes/android-host/`. Both are deleted at the end of the phase. Results go to `docs/spikes/android.md`, which outlives them.

**Tech Stack:** Java 17 source, `javac` + `d8` (build-tools 37.0.0 and platform `android-37.0` at `/opt/homebrew/share/android-commandlinetools`), Rust tokio, `openh264` crate.

## Setup (do once)

1. On win-pc, start the emulator `Pixel_API_37` and create and start `Pixel_API_26` (system image `system-images;android-26;google_apis;x86_64` via `sdkmanager` under `D:\Android\Sdk`). Use the T3 device host `win-pc` tools or `ssh win-pc`.
2. On the Mac: `ssh -N -L 5038:127.0.0.1:5037 win-pc &` then `ADB_SERVER_SOCKET=tcp:127.0.0.1:5038 adb devices -l` lists both emulators. Record the serials in `progress.md`.
3. Read `docs/research/prior-art.md` scrcpy rows, then open the scrcpy files it names. Copy only what each spike needs. Keep a list of copied files for the future `NOTICE`.

## Spike A0: Rust adb client over the tunnel

**Build:** `spikes/android-host/src/adb.rs`: connect to `ADB_SERVER_SOCKET`, send `host:version`, `host:track-devices-l`, `host:transport:<serial>` + `shell,v2,raw:echo hi`, `sync:` push of a 1 MB file, and `localabstract:<name>`. Request framing: 4 hex digit length + payload; `OKAY`/`FAIL` replies (AOSP `packages/modules/adb/SERVICES.TXT` and `protocol.txt`).

**Record:** round-trip time of `shell echo hi` (20 runs, p50/p95), push throughput, whether `localabstract:` works through the SSH-tunneled server, and whether shell v2 lets the host write to the child's stdin (needed for the agent token).

## Spike A1: UiAutomation from app_process

**Build:** `spikes/android-agent/src/.../Main.java`: create a Looper thread, construct `UiAutomation` with `UiAutomationConnection` via reflection (try the API-specific constructor signatures; log which one worked), `connect()`, set service info flags from architecture 6.2, dump all windows' trees to JSON on stdout, then listen for events for 10 s and print them.

**Build script:** `spikes/android-agent/build.sh` modeled on `scrcpy/server/build_without_gradle.sh` (javac with `-bootclasspath android.jar`, then `d8 --min-api 26 --output out.jar classes/**/*.class`).

**Run:** push, then `adb shell CLASSPATH=/data/local/tmp/spike.jar app_process / dev.phonedriver.spike.Main tree` on both emulators with Settings open.

Connect with `UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES`. Emit the tree in the exact `RawNode` format of `docs/protocol.md` section 2, so the dumps double as real fixtures.

**Record:**
- Which constructor signature works on API 26 and API 37.
- Enable TalkBack on the emulator, connect, and confirm TalkBack keeps working (proves the no-suppress flag).
- Whether the hidden `getSourceNodeId()` is reachable by reflection on both API levels (decides architecture 4.3 step 1).
- Prefetch behavior: how many binder round trips one Settings tree takes (count `findAccessibilityNodeInfo*` calls or time each `getChild`), and the largest batch received. The 80 ms snapshot budget depends on this.
- Scroll the 1,000-row list and check whether a recycled row keeps its source node id with new text (this is the case architecture 4.3 guards against).
- Save the Settings main, Settings > Apps, and a dialog tree to `crates/core/tests/fixtures/android/`, then run the Phase 0 core tests against them and look at the rendered snapshots. If label merging or role mapping is wrong on real data, fix core before Phase 2.
- Tree time on the Settings main screen and on a long list (Settings > Apps > all apps): with `FLAG_PREFETCH_DESCENDANTS_HYBRID` (API 37 only) and without, 20 runs each, p50/p95.
- Node count and JSON size.
- Events arrive on scroll and on navigation.
- Tree works on a screen that never idles (play a video in the emulator's browser): `uiautomator dump` fails there, ours must not.
- What happens when `uiautomator dump` runs at the same time (expect a conflict; record the exact error text, it becomes `AutomationConflict`).

**Fallback if it fails on either API:** package the same class as an instrumentation APK and start it with `am instrument -w`. Record the extra cost. Update decision D2 with the result.

## Spike A2: Input injection

**Build:** in the spike agent, an `inject` command: tap at (x, y) in px; a swipe as DOWN, MOVE×n, hold 200 ms, UP; a two-finger pinch (two pointers, `ACTION_POINTER_DOWN`/`UP` with pointer index bits).

**Run:** time from the host write to the agent's reply after `injectInputEvent(..., WAIT_FOR_FINISH)` returns. Compare with `adb shell input tap` timed from the host.

**Record:** p50/p95 for both methods, 50 taps each. Confirm the pinch zooms Google Maps or Photos on API 37 (look at a screenshot).

## Spike A3: Video stream and host decode

**Build:** agent side from scrcpy `ScreenCapture` + `SurfaceEncoder` with the settings in architecture 6.2 (repeat after 100 ms, priority 0, latency 1, 30 fps cap, 8 Mbps, scrcpy's size fallback). Do not set `KEY_PROFILE` at first. Framing from `docs/protocol.md` section 2. Host side: read the stream through `localabstract:`, decode with `openh264`, keep the latest frame, write a PNG on demand, compute the 4×4-point luma thumbnail difference per frame (architecture 4.6).

**Record:**
- Time from stream start on a **static** screen to the first decoded frame (the repeat setting should make this under 200 ms).
- Frames per second while scrolling and on a static screen; confirm repeated frames have zero changed area.
- The encoder's actual profile and level (from the SPS); whether `openh264` decodes it. Only if it does not, set `KEY_PROFILE` to Constrained Baseline and record that.
- Changed area in pt² for: a blinking caret, a 48 dp indeterminate spinner, a toggle switch animation, a screen transition. These set `significant_area` (default 4,096 pt²); update architecture 4.6 if the numbers disagree.
- Rotation (`settings put system user_rotation 1`), `wm size 720x1600` then `wm size reset`, and screen off/on: the stream recovers each time, how long it takes, and that the new size is reported.
- A `FLAG_SECURE` window (use any app or a quick test activity that sets it): is its region black in the frame while the tree still shows its text? That validates the detection rule in architecture 6.3.
- Decode time per frame on the Mac (p50/p95).
- Glass-to-host latency: inject a tap that toggles a switch, time until a frame with `changed_area_pt2 > 4096` arrives.
- `openh264` decodes the stream with no errors on both emulators. If not, record the profile the encoder actually produced.
- Screenshot from the latest frame vs `adb exec-out screencap -p`: time and visual quality. Look at both PNGs.
- Sync-frame request: time from request to the next keyframe.
- Save 3 s of a Settings scroll as `crates/video/tests/fixtures/settings_scroll.h264` (under 2 MB) **before** deleting the spike; Phase 2 task 2.4 needs it.

## Spike A5: agent launch hygiene

**Run:** start the spike agent through a `shell,v2,raw:` stream from the Rust host, then (a) close that stream and check the agent process is gone within 1 s (`pidof app_process` / `ps -A | grep dev.phonedriver`); (b) confirm `pkill -f dev.phonedriver.agent.Main` matches it and `pkill -f phonedriver-agent` does not; (c) push the jar with mode 0444 and confirm `app_process` loads it on API 26 and 37; (d) write a token to the agent's stdin and confirm a connection without it is dropped.

**Record:** the four results. They fix the launch rules in architecture 6.2.

## Spike A4: Unicode text by paste

**Build:** agent commands `clip_set`, `clip_get`, `paste` (KEYCODE_PASTE with WAIT_FOR_FINISH), and `set_text` (ACTION_SET_TEXT on the focused node).

**Run:** focus the Settings search field; paste `héllo 世界 مرحبا 👋🏽`; read the field back from the tree. Repeat with `set_text`. Repeat the paste into a Chrome page's text input (WebView-backed content) and restore the old clip immediately after injection: does Chrome paste the old clip? That decides the restore delay in architecture 6.2. Put an image URI on the clipboard first and check it is restored intact. Try `Keystrokes` with an emoji and confirm `getEvents` returns `null`.

**Record:** both modes on both API levels, exact readback, time for 100 characters. On API 33+, whether setting the clip from the shell shows the system clipboard overlay ("Copied" preview) on screen; if it does, it will show up as a frame change and in screenshots, so record how long it stays and whether `ClipboardManager` offers a way to set the clip without it (for example the `EXTRA_IS_SENSITIVE` description extra on API 33+).

## Phase gate

`docs/spikes/android.md` contains a section per spike (A0 to A5) with: what was built, the exact commands, a results table, the decision it confirms or changes (D2, D7, D10, D11), and any surprise with evidence. The real fixtures from A1 and the H.264 fixture from A3 are committed. Then delete `spikes/android-*` and commit `docs: android spike results`.

If any spike contradicts the architecture, update `docs/architecture.md` and `docs/decisions.md` in the same commit and say what changed in `progress.md`.
