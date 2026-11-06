# Phase 3: Observation quality and the full verb set on Android

> **For Claude:** REQUIRED SUB-SKILL: Use `executing-plans` to implement task-by-task.

**Goal:** Every verb in architecture 10.1 works on Android, every state in architecture 6.3 is handled, and a 20-task scenario suite passes on API 37 and API 26. Milestone **M2**.

**Architecture:** `docs/architecture.md` sections 4.3 to 4.7, 6.3, 10.1.

**Tech Stack:** as Phase 2, plus `serde_yaml` for scenarios.

Every task below follows the same loop: write the e2e test against the testbed log or a stock app, run it red, implement, run it green, commit. Only what differs is written out.

---

### Task 3.1: Settle on real signals

**Files:** Modify `crates/cli/src/daemon/session.rs`; create `crates/android/tests/settle.rs`.

**Tests** (`crates/cli/tests/settle.rs`, live, against the testbed's Settle and Busy tabs): after `tap "Slow open"` (a flat button, no press feedback, that opens a second activity 700 ms later) with `--expect-change 1500`, the reply shows the new screen with `changed: true`, kind `full`. After `tap "Do nothing"` the reply has `changed: false` and `settle_ms` between 450 and 600 (the default first-change window). After `tap "Animate"` (a 400 ms animation of a 160 dp square) `settle_ms` is at least 400 and below 2,500 (the plan said 800: on the emulators over the SSH tunnel the first frame arrives 300 to 500 ms after the tap and the encoder spreads the animation over about 670 ms, so a phone is where 800 is judged). On the Busy tab, `snapshot` settles in under 300 ms with `loading` in the header while a 40x20 dp ticker changes its text every 100 ms. A 200x200 dp area on the Settle tab that toggles black and white every 80 ms for 12 s ("Flash"): `wait settled --timeout 1000` fails `not_settled`, `snapshot --timeout 1000` is `ok` with `settled: false` and `still_changing` covering the area, and once the log shows the flash ended a snapshot settles. Changes from the first draft of this task: the flash covers an area, not the screen, and lives on the Settle tab, not Busy (see `progress.md`: a full-screen toggle, and the Busy tab's own spinner, made the emulator's video stream deliver frames in bursts with gaps over 250 ms, which the detector rightly reads as quiet); the 700 ms delayed screen needs the wider window because 500 ms is the default.

**Implementation:** settle was already fed from agent events and frames (task 2.6). New: `--expect-change <ms>` (`expect_change_ms` on `act` and `wait`); an action sent with `--no-settle` leaves the daemon holding its start time, and the next `wait` or settling `snapshot` within the first-change window waits for that action's first change instead of declaring the not yet changed screen quiet (found by the flash test: `wait settled` right after `tap --no-settle` returned `settled in 0ms` while the first frame was still 300 ms away). `sync_frame` after settle is not sent: spike A3 measured that the encoder honors it only when a new frame is rendered, so it cannot sharpen a settled screen (architecture 6.2).

Agent fix found on the way (the API 26 empty tree from `progress.md`): `getWindows()` can list nothing, or only the closing activity's window with no children, for 50 to 300 ms after a navigation (API 37, Back from a second activity), so the agent retries a hollow read for up to 800 ms; and on API 26 a connection made while the Busy tab animates sees no window until the first window change, whatever the wait, so the agent opens and closes the notification shade once per connection, which repairs it (16 of 16 connections, 4 of 16 broken before).

### Task 3.2: Diffs from actions

**Tests:** on Settings, `tap` on "Display" returns `kind: "full"` (navigation). Toggling Dark theme returns a single `~` line. Typing into the testbed field returns a `~` value line; the password field returns `~ … secure field changed` with no value.

### Task 3.3: Text targets and `find`

**Tests:** on Settings, `tap "Network & internet"` taps the row (the actionable ancestor), and the screen changes. Rows deep in the 1,000-row list render `Row 17` with `coll`/`item` so the header can say `list 1000 rows`. `tap "Row 5"` on the List tab taps row 5. `tap "Row"` fails `Ambiguous` and lists refs. `tap "Rwo 5"` fails `NotFound` with `similar` containing `Row 5` (normalized optimal-string-alignment distance ≤ 0.3 for suggestions: plain Levenshtein puts the swap `Rwo` at 0.4). `find "row 1" --role cell` lists Row 1, Row 10…Row 19 that are visible. The testbed rows log a `row` event on touch so the test can prove which row was tapped; `item` position is asserted only after the list is created while the agent is connected (RecyclerView attaches item info only then), and a text row of a list is a `cell` without it.

### Task 3.4: Scroll that lands exactly

**Tests:** on the List tab, `scroll down` moves the first visible row by roughly 60% of the viewport and the testbed log shows no fling (the last visible row logged after settle equals the one visible when the finger lifted). `scroll down --until "Row 500"` stops with Row 500 visible and returns its ref; it takes fewer than 20 gestures. `scroll down --until "Row 5000"` fails `NotFound` after reaching the end (the end is detected when two consecutive scrolls produce no diff).

**Implementation:** gesture = down, move in 10 steps over 250 ms, hold 150 ms, up (prior art: competitor `android.py:444-478`). Start inside the scrollable container's bounds, 20% from its edge. `scroll --max` defaults to 30. Refs taken before a scroll that pointed at recycled rows must now be stale or follow their item, never the new item in the old slot: after `scroll down`, `tap <ref of Row 3>` either fails `stale_ref` or taps Row 3 if visible, and the testbed log proves which.

**As built:** a plain `scroll` is the 10-step drag plus hold with stationary touch samples every 25 ms during the hold, because Android 8 flings a list that lifts after a gap with no samples (live: API 26 logged `SCROLL_STATE_SETTLING` and moved 107% of the viewport; with the samples it is a clean drag). A 60% drag crosses about 8 rows, so `--until "Row 500"` in fewer than 20 gestures needs flings. `scroll --until` therefore flings (a fast 80% swipe) when the target text ends in a number, the visible labels share its prefix, and the target is more than three screens away; it reverses with a normal drag or a fling when it overshoots. Any other target scrolls 60% a time. Measured on API 37: `Row 500` in 11 gestures. The end of the content is two consecutive scrolls with no diff; then `NotFound`. `scroll --max` bounds gestures; exceeding it is `Timeout`. After a scroll, a ref to a recycled row is stale or still that row (live test `a_ref_to_a_row_that_scrolled_away_is_stale_or_still_that_row`).

### Task 3.5: Swipe, long press, multi-touch gestures

**Tests:** on the Gestures tab: `tap --count 2` logs two taps 80±20 ms apart. `swipe up --from <ref>` starts at that element's center. `swipe left` logs one pointer path left to right-to-left with monotonic x; `swipe left --fast` releases while moving (velocity at UP > 1,000 dp/s in the log). `tap --long 800 @ref` logs DOWN and UP 800±30 ms apart. `gesture` with two fingers pinching logs two pointer ids, both paths, `ACTION_POINTER_DOWN` and `ACTION_POINTER_UP`. Timing of every logged event is within 20 ms of the requested schedule (played on device).

**As built:** no separate `long_press` action: `tap` takes `count` and `long` (`phone-use tap @ref --long 800`, `--count 2`), protocol 1 updated. `core::gesture::tap` makes a double tap as two non-overlapping fingers 80 ms apart (the agent starts a fresh ACTION_DOWN when no finger is down). `gesture <json>` rejects a finger with fewer than two points with -32602. The CLI argument is named `spec` because `--json` is a global flag. Timing on the testbed log: double tap 80±20 ms, long press 800±30 ms, pinch 400 ms total and its 150 ms move within 20 ms; the other events were not individually checked against the schedule.

### Task 3.6: Keys, buttons, apps

**Tests:** `press home` brings the launcher to the foreground (`state.foreground` is the launcher package). `press back` from the testbed's second screen returns to the first. `press enter` in the multiline field logs a newline. `apps` lists the testbed with label "phone-use testbed". `launch "phone-use testbed"` and `launch dev.phoneuse.testbed` both foreground it and return a full snapshot. `launch "Calculatr"` fails `NotFound` with "Calculator" in `similar`. `press notifications` makes the snapshot show the SystemUI shade window (post a test notification first with `cmd notification post -S bigtext -t "phone-use" tag "code 482913"` and assert its text appears). `open https://example.com` foregrounds the browser and the page heading "Example Domain" appears; `open dev.phoneuse.testbed://list/640` (add this deep link to the testbed) shows Row 640.

**As built:** `launch` and `open` are `act` kinds (`{kind: "launch", app}`, `{kind: "open", url}`), not separate methods; `apps` is a method and lists launchable apps only (no `--all`: the `Device::apps` port has no such parameter and the iOS tier shares it). `launch` matches the package, then the label in any case, then a label that contains the text; several matches are `Ambiguous`; none is `NotFound` with names within the 0.3 distance. The settle window for `launch` and `open` is 3 s (cold start). The emulators have no Calculator, so the misspelling test uses `Settngs` for `Settings`. `cmd notification post` does not exist on API 26, so the testbed posts the notification itself (`am start ... --es notify "code 482913"`, plus `pm grant POST_NOTIFICATIONS` on API 33+). The shade has no application window, so the header's `app` is null while it shows. The testbed label is now "phone-use testbed" and `dev.phoneuse.testbed://list/N` opens the List tab at row N. On API 26 there is no default browser, so `open https://...` shows the system chooser; the live test picks Chrome, "JUST ONCE", and clicks through Chrome's first-run screens.

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

As built (`bench/scenarios/android/*.yaml`, 20 files; run with `phone-use-bench scenario --phone-use <bin> --device android:<serial> [--only name] [--runs N] [--out file.jsonl]`): the YAML needs no `device:` key, steps are `launch open press wait shell sleep_ms tap long_tap type clear scroll swipe testbed set_switch restore_switch dismiss expect if_api`, and `expect` takes `text`, `absent`, `element` (with `state`), `log`, `no_log` and `shell: {cmd, contains}` (ground truth read with adb, used for dark theme and the Files folder). Differences from the list above: "Wi-Fi details" is `settings-network` (the Network & internet page and Back), "set an alarm" is `clock-stopwatch` (the alarm editor is a different UI on API 26 and 37; start, pause, reset the stopwatch), "read the page title" is `chrome-open-page` (the page's `Example Domain` text, after Chrome's first-run screens), and `settings-dark-theme` has a read-only branch on API 26, which has no dark theme (`if_api`). Chrome's first run is dismissed with the `dismiss` step; on API 26 the usage-statistics box is unticked first.

Avoid stock Contacts: on a fresh emulator it raises Google account and sync prompts that make the scenario test the setup, not the harness.

**Gate M2:** 20/20 on API 37 and API 26, three consecutive runs each (flake check). Emulators use a software encoder and no OEM changes, so M2 also needs one physical Android phone: ask the user (master plan question 3). On it, run the Phase 2 e2e tests and the scenarios that do not depend on emulator-only setup; record the encoder name and profile from spike A3's method. If no phone is available, M2 is met for emulators only and `progress.md` says so plainly. Record run times in `docs/bench/m2.md`. A flaky scenario is a bug: find the cause with the trace file, fix it, rerun three times.

---

## Phase gate

M2 met. Every row of architecture 6.3 has a passing test. `progress.md` updated.

**Changed after the agent-experience audit (`docs/audits/ax-audit.md`, finding 1):** a fling moves nothing on the polled iOS runner, so `scroll --until` flings only on a device with a frame stream (`FRAMES`); elsewhere it drags one screen at a time, and without `--max` the limit grows with a numbered target's distance. The end of the search is the `NotReached` error (-32016), not `NotFound`: it says how many times it scrolled and whether the list ended (at which row), never moved, or hit the limit.
