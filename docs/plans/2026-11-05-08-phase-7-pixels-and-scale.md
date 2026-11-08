# Phase 7: Pixels, tablets, and many devices

> **For Claude:** REQUIRED SUB-SKILL: Use `executing-plans` to implement task-by-task.

**Goal:** Work where there is no tree (OCR), on tablets and large screens, over Wi-Fi, from another machine, and on several devices at once.

**Architecture:** `docs/architecture.md` sections 5, 9, 11.

**Tech Stack:** `objc2-vision` (macOS).

---

### Task 7.1: OCR

**Files:** Create `crates/core/src/ocr.rs` (the `TextBox` type and merge logic, pure), `crates/cli/src/ocr.rs` (Apple Vision through `objc2-vision`, macOS only; other OSes return `Unsupported`, architecture section 9).

**Tests:**
- Golden: OCR on committed screenshots (testbed Taps tab at 1x and at 2x, Android Settings at native resolution, the iOS testbed Gestures tab; the Gestures tab has no drawn text, only "Gesture pad") finds the labels listed in `crates/cli/tests/ocr.rs`, which were written by looking at each image. Not found, by design of the engine at these sizes: the lower rows of Taps buttons at 1x and the iOS tab bar captions other than the selected one and the ones next to it; recorded in `progress.md`.
- Merge (pure): OCR boxes that overlap a tree element with the same text (case-insensitive, IoU > 0.5) are dropped; the rest become `Source::Ocr` elements with refs, rendered with `(ocr)`.
- Speed: Vision p50 under 120 ms on a 412x915 frame on the Mac.

### Task 7.2: When pixels are used

**Tests:** on the testbed Web tab with accessibility disabled in the WebView (add a toggle), a plain `snapshot` merges OCR automatically and marks elements `(ocr)`. `snapshot --pixels` forces OCR on any screen. On a CoreDevice-tier iPhone without the runner, `snapshot` is OCR-only and the header says `tree unavailable: install the runner for full fidelity`.

### Task 7.3: Tablets and large screens

Create AVD `Pixel_Tablet_API_37` on win-pc and boot an iPad Pro 13-inch simulator on the mini.

**Tests:** testbed and scenario subsets in landscape and portrait on both tablets; snapshots of a two-pane layout (Settings on a tablet shows list and detail side by side) keep both panes. Rotation mid-scenario on both. Split-screen and Split View: only check that the snapshot does not lie (Android lists both apps' windows; iPad header says another app shares the screen); no further work unless a task needs it. Record any gap in `docs/spikes/tablets.md` and fix or document it.

### Task 7.4: Android over Wi-Fi

No new code: `adb pair` and `adb connect` already exist, and a connected Wi-Fi device appears in `track-devices` like any other. Add a `doctor` hint for devices in state `offline` over TCP, and document the two adb commands in `README.md`. **Test** on a phone or emulator with wireless debugging (Android 11+): pair and connect with adb, run the Phase 2 e2e tests over Wi-Fi, and record latency next to USB in `docs/bench/history/wifi.md`.

### Task 7.5: Two devices at once

**Tests:** two clients each lease a different emulator and run the full Android scenario suite concurrently. Both pass; no cross-talk (each trace only names its own device). Then one client drives both devices alternately with `--device`. Measure daemon CPU and memory with two video streams; record in `docs/bench/history/multi.md`.

### Task 7.6: Remote use over SSH

Document in `README.md` and test once: daemon on the mini with a simulator, client on the MacBook through `ssh -N -L /tmp/phonedriver-mini.sock:/Users/<user>/.phonedriver/run/daemon.sock mini`, and `PHONEDRIVER_SOCKET=/tmp/phonedriver-mini.sock phonedriver snapshot`. `PHONEDRIVER_SOCKET` was already read (`daemon/socket.rs`, since Phase 2); the only code change is that a dead forward names the variable in the error. `screenshot` prints a path on the device's machine (README says how to fetch it). No new network code.

---

## Phase gate

Tasks 7.1 to 7.6 green with evidence in `progress.md`.
