# Phase 5: iOS Simulator tier and PhonedriverRunner

> **For Claude:** REQUIRED SUB-SKILL: Use `executing-plans` to implement task-by-task.

**Goal:** The PhonedriverRunner serves trees, input and screenshots; the Simulator tier passes an iOS scenario suite; the runner's tree is at least 2x faster than WebDriverAgent's. Milestone **M4**.

**Architecture:** `docs/architecture.md` sections 7 and 7.3. Protocol: `docs/protocol.md` section 3. Prior art: `docs/research/prior-art.md` WebDriverAgent rows.

**Tech Stack:** Objective-C XCTest, XcodeGen, Xcode 27, `xcrun simctl`, Rust `crates/ios`.

**Where:** all work runs on the **Mac mini** (cua Space `mini` for shell and files; `mini-desktop` MCP to look at its screen). Never drive the MacBook's screen. Check the mini for Rust, `blaze`, Xcode and XcodeGen first; install what is missing (`brew install xcodegen`; blaze: `curl -fsSL https://ix.dev/blaze | sh`). Sync the repo to the mini with `git` over SSH or the cua volume, and record the method in `progress.md`.

---

### Task 5.1: iOS testbed

**Files:** Create `testbeds/ios/project.yml` (XcodeGen; iOS 17 deployment target; bundle id `dev.phonedriver.testbed`), `testbeds/ios/Testbed/*.swift`.

Same tabs as the Android testbed (Phase 2 task 2.8), in SwiftUI with `accessibilityIdentifier` on every control, plus a **Nav** tab with a `NavigationStack` two levels deep (to test `back`). Logging: append JSON lines to `Documents/putb.log`; touches are captured with a `UIWindow` subclass overriding `sendEvent(_:)` so raw touch phases, locations in points and timestamps are logged. Add `crates/ios/tests/support/testbed.rs`: build and install to a simulator (`xcodegen && xcodebuild -scheme Testbed -destination id=<udid> build`, then `simctl install`), `clear_log`, `read_log` (path from `xcrun simctl get_app_container <udid> dev.phonedriver.testbed data`).

*As built (2026-11-05):* the host helper is `testbeds/ios/testbed.py` (Python 3 standard library), because the mini has no Rust toolchain and tasks 5.1 to 5.3 have no Rust consumer yet; task 5.5 ports it to `crates/ios/tests/support/testbed.rs` when the first Rust live test needs it. Nine tabs do not fit a system `TabView` on a phone (five would hide behind "More"), so the testbed draws its own one-row tab bar (`tab.<name>` identifiers). Tests select a tab with the launch argument `-tab <name>`; `simctl openurl putb://tab/<name>` works too but raises a system "Open in Testbed?" confirmation first. The log is opened with `O_APPEND`, so `clear_log` truncates it rather than deleting it. The mini has only the iOS 26.5 simulator runtime (11 GB free disk, not enough for the iOS 27 runtime), so Phase 5 runs on an iPhone 17 simulator with iOS 26.5, built with the iOS 27 SDK.

**Verify:** boot an iPhone 17 simulator (iOS 27; iOS 26.5 on the mini, see above), install, screenshot each tab with `xcrun simctl io <udid> screenshot`, look at each.

### Task 5.2: Spike I4: WDA baseline and runner snapshot technique

**Files:** `spikes/ios-runner-snapshot/` (deleted after).

Build WebDriverAgent at `3d6a4e0` for the simulator. Time `GET /source` on the testbed List tab and on Settings, 20 runs, with default settings and with `snapshotMaxDepth = 30`. Then, in a minimal XCTest, time `XCUIApplication` snapshot through `XCAXClient_iOS` with bounded `maxDepth` and the reduced attribute list (label, value, identifier, frame, elementType, enabled, selected, hasFocus, placeholderValue). Record both in `docs/spikes/ios.md`. This sets the D4 bar: the runner must be at least 2x faster than tuned WDA.

### Task 5.3: PhonedriverRunner

**Files:** Create `ios-runner/project.yml`, `ios-runner/Runner/PhonedriverRunnerTests.m`, `ios-runner/Runner/{Server,Snapshot,Gesture,Screenshot,Typing,Buttons,Json}.{h,m}`, `ios-runner/PrivateHeaders/` (only the headers used, copied from WDA with license notice in `NOTICE`).

**Step 1: Failing test** (`crates/ios/tests/runner_live.rs`, `#[ignore]`): launch the runner on the simulator, connect, `hello` returns the screen size in points; `snapshot` of the testbed contains the identifier `taps.b17`.

**Step 2:** FAIL. **Step 3: Implement.**
- `testRun` starts the server on `PHONEDRIVER_PORT` (default 27753) and blocks the test forever on a run loop. Set `executionTimeAllowance` off for this test and launch `xcodebuild` with `-test-timeouts-enabled NO` so XCTest never kills it.
- `open_url`: `XCUIDevice.sharedDevice.system openURL:` (Xcode 14.3+).
- `Server`: `NWListener` (Network.framework), one connection at a time, `u32` length + JSON framing.
- `Snapshot`: active application via the `fb_activeApplication` approach; bounded depth and reduced attributes from spike I4; flatten to `RawNode` preorder with parent indexes.
- `Gesture`: build an `XCSynthesizedEventRecord` with one `XCPointerEventPath` per finger from the timed paths; synthesize and wait for completion (WDA `FBW3CActionsSynthesizer.m`).
- `Screenshot`: `XCUIScreen.mainScreen` screenshot, JPEG at the requested quality.
- `Typing`: `typeText:` on the focused element (Unicode safe).
- `Buttons`: `XCUIDevice.sharedDevice pressButton:` for home and volume; lock through the private method WDA uses (`XCUIDevice+FBHelpers.m` `fb_lockScreen`). `recents` is not supported on iOS.

**Step 4:** PASS. Commit.

*As built (2026-11-05):* the live test is `ios-runner/tests/test_runner.py` (Python 3 standard library, 13 tests, run on the mini), not `crates/ios/tests/runner_live.rs`: the mini has no Rust toolchain and there is no Rust runner client until task 5.4, which moves these checks into Rust. The test method is `testServe` (`testRun` collides with `XCTestCase.testRun`). `hello` reads the screen from SpringBoard's frame because the runner's own process runs letterboxed at 320 x 480. `type` uses a redacted synthesized text-input event instead of `typeText:` (secrets). `snapshot` takes `max_depth` only and requests eight raw AX attribute names; the planned `attributes?` field was dropped. The private API is declared in one trimmed header, `ios-runner/PrivateHeaders/XCTestPrivate.h`, instead of copies of WDA's headers, which pull in dozens of further headers. Volume buttons return `unsupported` on the Simulator. Wire contract in `docs/protocol.md` section 3.

### Task 5.4: Runner lifecycle on simulators

**Files:** Create `crates/ios/src/simulator.rs`, `crates/ios/src/runner.rs`.

**Tests** (`#[ignore]`): `Simulator::list()` parses `xcrun simctl list devices -j` and returns booted devices. Starting the runner builds once (`xcodebuild build-for-testing`, cached by a hash of `ios-runner/`), then launches with `xcodebuild test-without-building -xctestrun <file> -destination id=<udid>` as a child process with `TEST_RUNNER_PHONEDRIVER_PORT` set to `27753 + index` (xcodebuild strips the prefix and passes `PHONEDRIVER_PORT` to the test). Test: two simulators get two runners on two ports with no collision. The daemon polls `hello` on the port every 100 ms (up to 60 s for a cold build) instead of reading xcodebuild's output. Killing the daemon kills the child (process group). A second daemon start reuses a running runner if `hello` answers with the same version.

*As built (2026-11-05):* the port is not `27753 + index` but the first free port from 27753 (an index into the booted list shifts when a simulator boots or shuts down, and then a reused runner would be attributed to the wrong simulator). To make reuse safe the runner's `hello` gained `udid` (from `SIMULATOR_UDID`) and `build` (the source hash, passed as `TEST_RUNNER_PHONEDRIVER_BUILD`); "same version" means same wire version and same build. The sources are embedded in the binary and the build is cached under `~/.phonedriver/runner/<hash>`; it targets `generic/platform=iOS Simulator`, so one build serves every simulator. The 60 s readiness wait starts after the build. Live tests: `crates/ios/tests/runner_live.rs`.

### Task 5.5: IosDevice, Simulator tier

**Files:** Create `crates/ios/src/device.rs`, `crates/ios/src/map.rs`.

Implements `Device`: `tree` and `gesture` and `screenshot` and `text` (mode `Keystrokes` and `Paste`: `simctl pbcopy` then Cmd+V via the runner's keyboard; `SetValue` unsupported) and `button`; `apps`/`launch` via `simctl listapps` and `simctl launch`; `frames()` returns `None`, so the session polls screenshots for settle (architecture 4.6). `open` via `xcrun simctl openurl`. `press notifications` via a top-left edge swipe. `back`: tap the navigation bar's back button when present (`via: "nav-button"`), else a left-edge swipe (`via: "edge-swipe"`).

**Tests:** unit tests for `map` with fixtures captured from the runner; live tests that mirror Phase 2 task 2.9 and Phase 3 tasks 3.3 to 3.7 against the iOS testbed, including `back` on the Nav tab with both `via` values.

*As built (2026-11-05):* `frames()` returns polled gray thumbnails instead of `None` (architecture 4.6), so the session needs no iOS code. Paste is typed at 5000 characters per second because a synthesized Cmd+V does not paste another process's pasteboard text (D15); `open` uses the runner, not `simctl openurl`, which raises a confirmation. The back button is found by its identifier `BackButton`; the edge-swipe case is tested on the Web tab, whose web view now has back-forward gestures and a `#details` link, because a hidden navigation bar disables the swipe. Phase 3's verbs (scroll, swipe, find, launch, open, clear, clipboard) are not in the daemon on this branch (another agent builds them in parallel), so the live tests check those device operations through `IosDevice` directly (`crates/ios/tests/device_live.rs`) and the CLI path for devices, snapshot, tap, type, secret type, press back and screenshot (`crates/cli/tests/e2e_ios.rs`). The testbed helper is `crates/ios/tests/support/testbed.rs`. Core changes: a labeled control absorbs its icon and label-repeating text (prune), and an element with a unique app id keeps its ref when relabeled (refs). Shared daemon changes: `daemon/ios.rs` (simulator source), `daemon/sources.rs` (several sources behind one), `run_daemon` uses both, `press back` reports `via` through the new `Device::back`.

### Task 5.6: Scenario suite and benchmark (M4)

**Files:** Create `bench/scenarios/ios/*.yaml` (12 scenarios: Settings appearance to dark, Settings search, Safari open URL and read title, Contacts add, Reminders add, each testbed tab's core check, Nav back twice, a long emoji text). Extend `phonedriver-bench` for iOS ops.

Record runner screenshot time (p50/p95) as its own number; it sets how fast the Simulator tier can poll for settle. If it is above 300 ms, try JPEG quality 30 at half scale for settle polling only.

**Gate M4:** 12/12 scenarios pass three runs in a row on the iOS 27 simulator. Runner snapshot p50 is at most half of tuned WDA's on the same screens (D4); if not, follow D4: switch to WDA and delete the runner's snapshot code. Numbers in `docs/bench/history/m4.md`.

*As built (2026-11-05):* the scenarios run through the CLI with `phonedriver-bench scenario` (`bench/src/scenario.rs`), and the iOS benchmark is `phonedriver-bench ios` (`bench/src/ios.rs`); `main.rs` only dispatches to them. Three scenarios differ from the list above, because the iOS 26.5 Simulator without an Apple Account cannot do them: Reminders and Calendar have no local list ("New Reminder" and "Add" stay disabled, and Reminders asks to enable iCloud on every launch), so `files-new-folder` (Files, On My iPhone) replaces "Reminders add"; Settings search answers "No Results" for every query on this simulator, so `settings-search` checks that the query reached the field and that Settings answered for it; Settings has no Display & Brightness on the Simulator, so dark appearance is Developer > Dark Appearance, checked with `simctl ui appearance`. The testbed's Taps tab gained a "Dark theme" toggle, as on Android, for the tap-to-settled-diff benchmark row. The gate runs on the iOS 26.5 simulator (see task 5.1). `launch`, `open` and `swipe` were first added here as minimal versions and replaced by Phase 3's at the merge; the scenario runner is the one Android runner with a platform chosen by device id (see `docs/bench/history/m4.md`). WebDriverAgent was not rebuilt for the D4 check: its checkout went with spike I4 and the mini had 2 GB free, so the runner is re-measured against task 5.3's WDA numbers, once as the machine was and once under 5.3's conditions.

### Task 5.7: doctor for iOS Simulator

Add checks: Xcode found, a simulator booted, runner builds, runner answers `hello`. Test on the mini.

*As built (2026-11-05):* `crates/cli/src/doctor.rs` and the `doctor` verb, with the line format Phase 4's task 4.3 asks for (`ok`, `warn` or `fail`, a fix under every problem); Phase 4 adds the Android checks to the same file. Checks: `xcodebuild -version`, `xcodegen` (the runner project is generated with it), booted simulators (`warn` when none), the runner build, and for each booted simulator a snapshot through the daemon in a `doctor` session. The runner serves one connection and a new connection cancels the old one, so `doctor` never probes a runner port itself: that would cut off a daemon that holds it. The daemon's connect says `hello`.

---

## Phase gate

M4 met. `progress.md` updated. `spikes/ios-runner-snapshot` deleted.
