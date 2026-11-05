# Phase 3: Observation quality and the full verb set on Android

> **For Claude:** REQUIRED SUB-SKILL: Use `executing-plans` to implement task-by-task.

**Goal:** Every verb in architecture 10.1 works on Android, every state in architecture 6.3 is handled, and a 20-task scenario suite passes on API 37 and API 26. Milestone **M2**.

**Architecture:** `docs/architecture.md` sections 4.3 to 4.7, 6.3, 10.1.

**Tech Stack:** as Phase 2, plus `serde_yaml` for scenarios.

Every task below follows the same loop: write the e2e test against the testbed log or a stock app, run it red, implement, run it green, commit. Only what differs is written out.

---

### Task 3.1: Settle on real signals

**Files:** Modify `crates/cli/src/daemon/session.rs`; create `crates/android/tests/settle.rs`.

**Tests:** after `tap` on a testbed button that opens a screen only after a 700 ms delay (add a "Slow open" button that posts the navigation with `Handler.postDelayed`), the reply shows the new screen with `changed: true`, not an empty diff (first-change window, architecture 4.6). After `tap` on a button that does nothing, the reply has `changed: false` within 600 ms. After `tap` on a testbed button that starts a 400 ms animation, the reply's `settle_ms` is between 400 and 800. On the Busy tab, `snapshot` replies with `settled: true` within 300 ms and `loading` in the header (the spinner is small). A full-screen animation (add a "Flash" button to Busy that animates the whole screen for 3 s) gives `settled: true` only after it ends; with `--timeout 1000`, the reply is `ok` with `settled: false` and `still_changing` covering the screen, and the snapshot is attached. `wait settled --timeout 1000` during the flash fails with `not_settled`.

Add a "Ticker" label to the Busy tab that changes its text every 100 ms in a 40x20 dp area: `snapshot` still settles within 300 ms (this is why content-change events are not a settle input; architecture 4.6).

**Implementation:** feed `WindowChanged` from agent events and `Frame { changed_area_pt2, changed_box }` from the FrameStore into `core::settle::Detector`. After settle, send `sync_frame` so the next screenshot is sharp.

### Task 3.2: Diffs from actions

**Tests:** on Settings, `tap` on "Display" returns `kind: "full"` (navigation). Toggling Dark theme returns a single `~` line. Typing into the testbed field returns a `~` value line; the password field returns `~ … secure field changed` with no value.

### Task 3.3: Text targets and `find`

**Tests:** on Settings, `tap "Network & internet"` taps the row (the actionable ancestor), and the screen changes. Rows deep in the 1,000-row list render `Row 17` with `coll`/`item` so the header can say `list 1000 rows`. `tap "Row 5"` on the List tab taps row 5. `tap "Row"` fails `Ambiguous` and lists refs. `tap "Rwo 5"` fails `NotFound` with `similar` containing `Row 5` (use normalized Levenshtein ≤ 0.3 for suggestions). `find "row 1" --role cell` lists Row 1, Row 10…Row 19 that are visible.

### Task 3.4: Scroll that lands exactly

**Tests:** on the List tab, `scroll down` moves the first visible row by roughly 60% of the viewport and the testbed log shows no fling (the last visible row logged after settle equals the one visible when the finger lifted). `scroll down --until "Row 500"` stops with Row 500 visible and returns its ref; it takes fewer than 20 gestures. `scroll down --until "Row 5000"` fails `NotFound` after reaching the end (the end is detected when two consecutive scrolls produce no diff).

**Implementation:** gesture = down, move in 10 steps over 250 ms, hold 150 ms, up (prior art: competitor `android.py:444-478`). Start inside the scrollable container's bounds, 20% from its edge. `scroll --max` defaults to 30. Refs taken before a scroll that pointed at recycled rows must now be stale or follow their item, never the new item in the old slot: after `scroll down`, `tap <ref of Row 3>` either fails `stale_ref` or taps Row 3 if visible, and the testbed log proves which.

### Task 3.5: Swipe, long press, multi-touch gestures

**Tests:** on the Gestures tab: `tap --count 2` logs two taps 80±20 ms apart. `swipe up --from <ref>` starts at that element's center. `swipe left` logs one pointer path left to right-to-left with monotonic x; `swipe left --fast` releases while moving (velocity at UP > 1,000 dp/s in the log). `tap --long 800 @ref` logs DOWN and UP 800±30 ms apart. `gesture` with two fingers pinching logs two pointer ids, both paths, `ACTION_POINTER_DOWN` and `ACTION_POINTER_UP`. Timing of every logged event is within 20 ms of the requested schedule (played on device).

### Task 3.6: Keys, buttons, apps

**Tests:** `press home` brings the launcher to the foreground (`state.foreground` is the launcher package). `press back` from the testbed's second screen returns to the first. `press enter` in the multiline field logs a newline. `apps` lists the testbed with label "phone-use testbed". `launch "phone-use testbed"` and `launch dev.phoneuse.testbed` both foreground it and return a full snapshot. `launch "Calculatr"` fails `NotFound` with "Calculator" in `similar`. `press notifications` makes the snapshot show the SystemUI shade window (post a test notification first with `cmd notification post -S bigtext -t "phone-use" tag "code 482913"` and assert its text appears). `open https://example.com` foregrounds the browser and the page heading "Example Domain" appears; `open dev.phoneuse.testbed://list/640` (add this deep link to the testbed) shows Row 640.

### Task 3.7: Clear and clipboard

**Tests:** `clear <plain field>` logs an empty value. `clipboard set "x"` then `clipboard` prints `x`. After any `type --mode paste`, the device clipboard equals what it was before.

### Task 3.8: Device states

One test per row of architecture 6.3, except the Xiaomi row, which stays "unverified until hardware" unless the user provides a Xiaomi phone. Use `adb shell input keyevent KEYCODE_SLEEP` to turn the screen off; `locksettings set-pin 1234` to create a PIN on the emulator (clear it after with `locksettings clear --old 1234`); start `uiautomator dump` in a loop to create a conflict; `settings put system user_rotation 1` with auto-rotate off to rotate; the testbed Secure tab; the Dialogs tab for the permission dialog (assert the dialog's buttons appear in the snapshot with the dialog's window above the app). Each test asserts the exact error code or header text from the architecture.

### Task 3.9: Keyboard awareness

**Tests:** focusing the plain field shows `keyboard shown` in the header and marks buttons under the IME `under-keyboard`. `tap` on an element marked `under-keyboard` fails `OffScreen` with a hint to `press back` (hides the IME) or `scroll`.

### Task 3.9b: `wait` and `snapshot --within`

**Tests:** `wait "Row 640"` after `open dev.phoneuse.testbed://list/640` returns as soon as the row appears (well under the timeout). `wait gone:"Loading"` on a testbed screen that hides a label after 1 s returns after about 1 s. `wait "Never"` with `--timeout 500` fails `timeout` naming the condition. `wait` resolves on frame or window events, not by polling the tree on a timer: check the trace shows one tree fetch per significant change, not one per 100 ms. `snapshot --within @eN` renders only that subtree.

### Task 3.10: Compose and WebView coverage

**Files:** add a Compose tab to the testbed (`Button`, `Switch`, `TextField`, `LazyColumn` with `testTag`s).

**Tests:** snapshot shows Compose elements with roles and labels; `testTag` appears as `id` when the app sets `testTagsAsResourceId`. WebView tab: form inputs appear as `TextField` elements and accept `type`.

### Task 3.11: Scenario suite (M2)

**Files:** create `bench/scenarios/android/*.yaml` and a `scenario` subcommand in `phone-use-bench`.

A scenario is a list of verbs and expectations, run without an LLM:
```yaml
name: settings-dark-theme
device: android
steps:
  - launch: com.android.settings
  - tap: "Display"            # text target; recorded label on the emulator
  - tap: {text: "Dark theme", role: switch}
  - expect: {element: {text: "Dark theme", role: switch}, state: on}
cleanup:
  - shell: "cmd uimode night no"
```

Twenty scenarios across stock apps and the testbed: dark theme, Wi-Fi details, set an alarm in Clock, create a folder in Files, open a URL in Chrome and read the page title, search in Settings, scroll to Row 750 and tap it, the permission dialog, fill and submit the WebView form, Compose list to item 300, each testbed tab's core check, rotation mid-task, screen off mid-task, a long text with emoji. Each must leave the device as it found it.

Avoid stock Contacts: on a fresh emulator it raises Google account and sync prompts that make the scenario test the setup, not the harness.

**Gate M2:** 20/20 on API 37 and API 26, three consecutive runs each (flake check). Emulators use a software encoder and no OEM changes, so M2 also needs one physical Android phone: ask the user (master plan question 3). On it, run the Phase 2 e2e tests and the scenarios that do not depend on emulator-only setup; record the encoder name and profile from spike A3's method. If no phone is available, M2 is met for emulators only and `progress.md` says so plainly. Record run times in `docs/bench/m2.md`. A flaky scenario is a bug: find the cause with the trace file, fix it, rerun three times.

---

## Phase gate

M2 met. Every row of architecture 6.3 has a passing test. `progress.md` updated.
