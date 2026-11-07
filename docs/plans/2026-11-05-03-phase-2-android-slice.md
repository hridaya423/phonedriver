# Phase 2: Android vertical slice

> **For Claude:** REQUIRED SUB-SKILL: Use `executing-plans` to implement task-by-task.

**Goal:** `phonedriver devices | snapshot | tap | type | screenshot` work end to end on the `Pixel_API_37` emulator through the daemon, verified against a testbed app, and benchmarked against phone-harness. Milestone **M1**.

**Architecture:** `docs/architecture.md` sections 5, 6, 8, 10.1. Wire formats: `docs/protocol.md` sections 1 and 2. Spike results: `docs/spikes/android.md` (read first; it overrides any guess below).

**Tech Stack:** tokio, `interprocess` (local sockets), `clap` (derive), `serde_json`, `openh264`, `ulid`, `png`; Java for the agent; Gradle + Java for the testbed.

**Execution order:** 2.1 → 2.8 (testbed) → 2.2 → 2.3 → 2.4 → 2.5 → 2.6 → 2.7 → 2.9 → 2.10. The testbed comes second because the agent, device, and fixture tasks test against it. Task numbers stay as they are because other phases cite them.

Device tests use `#[ignore]` and read the target from `PHONEDRIVER_TEST_DEVICE` (for example `android:emulator-5554`). Run them with `ADB_SERVER_SOCKET=tcp:127.0.0.1:5038 PHONEDRIVER_TEST_DEVICE=… blaze test -- --ignored`.

---

### Task 2.1: adb client

**Files:** Create `crates/android/src/adb.rs`, `crates/android/tests/adb_fake_server.rs`.

**Step 1: Failing tests.** With `ADB_SERVER_SOCKET` set to a closed port, the client returns `Error::Transport` naming that address and does **not** run `adb start-server` (architecture 6.1). A test-only fake adb server (a tokio TCP listener speaking the 4-hex-length protocol, about 80 lines inside the test file) verifies: `devices()` parses `host:devices-l` output into `{serial, state, model, transport_id}`; `track_devices()` yields an update when the fake pushes a new list; `shell(serial, cmd)` returns stdout and exit code via shell v2 framing; `push(serial, bytes, path, mode)` sends `SEND`/`DATA`/`DONE` chunks of at most 64 KiB; `open_abstract(serial, name)` returns a duplex stream after `OKAY`; a `FAIL` reply becomes `Error::Transport` with the server's message.

**Step 2:** `blaze test -p phonedriver-android adb` → FAIL.

**Step 3: Implement** per spike A0 and the D11 result. Server address from `ADB_SERVER_SOCKET` (`tcp:host:port`), default `127.0.0.1:5037`. If connect is refused, run `adb start-server` once, then retry.

**Step 4:** PASS. Add one `#[ignore]` test against the real emulator: `shell("echo hi")` returns `hi`. Run it. Commit.

### Task 2.2: Production device agent

**Files:**
- Create `android-agent/src/dev/phonedriver/agent/{Main,Control,Video,Input,Accessibility,TextInput,Apps,DeviceState,Json}.java`
- Copy from scrcpy only what the agent compiles against, keeping scrcpy's package names and paths (`android-agent/src/com/genymobile/scrcpy/...`, so provenance is file for file); record each in `NOTICE`. Extra agent files: `Ops.java` (op dispatch), `Monitor.java` (display and keyguard events), `Keyguard.java` (`IWindowManager`, because `KeyguardManager` fails on API 37)
- Create `android-agent/build.sh` (outputs `android-agent/build/phonedriver-agent.jar`; reads `ANDROID_HOME`, defaults to `/opt/homebrew/share/android-commandlinetools`; platform `android-37.0`, build-tools `37.0.0`; `d8 --min-api 26 --release`)
- Create `crates/android/build.rs`: runs `android-agent/build.sh` when any file under `android-agent/src` is newer than the jar, then exposes the jar path via `cargo:rustc-env=PHONEDRIVER_AGENT_JAR`. If `PHONEDRIVER_AGENT_JAR` is already set in the environment, use that file and skip the build. CI: GitHub-hosted runners ship an Android SDK in `ANDROID_HOME`; add a CI step `sdkmanager "platforms;android-37.0" "build-tools;37.0.0"` and set `ANDROID_PLATFORM`/`ANDROID_BUILD_TOOLS` for `build.sh`. On Windows runners `build.sh` runs under Git Bash; verify once.
- Create `NOTICE`

Build the agent one component at a time. Each component gets its own failing live test in `crates/android/tests/agent_live.rs` (`#[ignore]`), talking raw protocol frames to the agent (no AndroidDevice yet), checked against the testbed log where input is involved:

| Component | Failing test first |
|---|---|
| Main + Control | `hello` with the token returns `api` and display; without the token the stream closes; an unknown op returns `{ok:false, error}` |
| Accessibility | `tree` on the testbed Taps tab contains 24 buttons with ids `b0`..`b23` and every `RawNode` key the protocol lists for them, and no status or navigation bar window |
| Input | `gesture` tap at button `b17`'s center in px produces one testbed `tap` log entry within ±1 px; a two-pointer gesture logs both pointer ids |
| Video | the video stream's codec header matches `hello`'s display size; a picture frame arrives within 1 s of the stream start (spike A3: cold start to first frame is p50 433 ms on API 37, so 200 ms cannot hold) |
| TextInput | `text {mode: paste}` of the Unicode test string into the testbed field logs it exactly; the prior clip is restored |
| Apps | `apps` includes `dev.phonedriver.testbed`; `launch` foregrounds it (`state.foreground`) |
| DeviceState | `state` reports `screen_on: false` after `KEYCODE_SLEEP` and `true` after `wake` |

Loop per component: write its test, run it red, implement the component as architecture 6.2 says (with the constructor, flags, and settings spikes A1 to A5 recorded), run it green, commit. Control stream: one reader thread, one writer with a queue, requests dispatched to a single-threaded executor except `gesture` (its own thread so a long gesture does not block `tree`). Exit rules: both streams closed, or no connection within 10 s.

### Task 2.3: Agent deploy and lifecycle

**Files:** Create `crates/android/src/agent.rs`.

**Step 1: Failing tests** (`#[ignore]`, live):
- First deploy pushes `/data/local/tmp/phonedriver-agent-<sha8>.jar` with mode 0444; second deploy with the same jar does not push (the path exists).
- A stale agent of another version is killed before launch (start one manually with a fake version string; assert `pgrep -f dev.phonedriver.agent.Main` shows only the new one).
- Closing the host's shell stream makes the agent exit within 1 s.
- An agent with no connection exits after 10 s.
- A connection to the abstract socket without the token is dropped (connect with a raw `localabstract:` stream and send `hello` without it).

**Step 2:** FAIL. **Step 3:** `Agent::start(adb, serial) -> AgentConn { control, video, shell }`. The `shell` stream is held for the agent's lifetime and its output drained into the daemon log (architecture 6.2). Socket name `phonedriver-<version>-<8 random hex>`; token: 32 random bytes from `getrandom`, hex-encoded, written to the agent's stdin. The agent prints `ready` on stdout once listening; wait for that line (up to 3 s) instead of retrying the socket. **Step 4:** PASS. Commit.

### Phase 1 findings that change Phase 2 tasks (2026-11-05, `docs/spikes/android.md`)

- Agent tree (tasks 2.2, 2.5): call `clearCache()` before every tree; walk with `getChild(i, FLAG_PREFETCH_DESCENDANTS_DEPTH_FIRST)` on API 33+ (not window-root HYBRID); construct UiAutomation with the public `(Looper, IUiAutomationConnection)` constructor then `connect(int)`; measure dropping the status-bar and navigation-bar windows to hit the 80 ms snapshot budget (API 37 Settings main took 75 to 130 ms p50 with them).
- Launch (2.2): `pkill -f 'dev.phonedriver.agent.[M]ain'` with the bracket; a Looper must exist on every thread that touches `ClipboardManager` (API 26); closing the shell stream ends the agent in about 1 s.
- Snapshot (2.7): real Settings on API 37 is Compose, so lists are `android.view.View` with collection info (core now maps them to `list`); windows arrive topmost first, so status-bar nodes print before the app and an app bar prints after its list. Decide the reading order (app window first, then children by top then left) with a golden test on `real_settings_apps`.
- Video (2.4): the fixture `crates/video/tests/fixtures/settings_scroll.h264` is the agent's video wire stream exactly as received (12-byte codec header, then `u64 pts | u8 flags | u32 len | Annex-B` frames; 60 packets, 1080x2424, 1 keyframe, 1.6 MB). Static screens give about 3 fps, so `Stalled` means no frame for 1 s.
- Settle (core): thresholds are now `first_change_area = 128 pt²` and `significant_area = 4,096 pt²`.

### Task 2.4: Video decode and FrameStore

**Files:** Create `crates/video/src/{lib.rs,h264.rs,frame_store.rs}`; test fixture `crates/video/tests/fixtures/settings_scroll.h264` (record 3 s from the emulator with the spike or agent; under 2 MB).

**Step 1: Failing tests.**
- Decoding the fixture yields N frames with the size from its codec header.
- `FrameStore::publish` increments `seq`; a `watch::Receiver` sees the newest frame only.
- `changed_area_pt2` between two identical frames is 0; between the first and last fixture frames (scrolled) it exceeds 4,096 pt²; `changed_box` covers the scrolled list (on the fixture: x 33 to 322 pt, y 142 to 921 pt of a 411×923 pt screen). Size changes count as the whole screen changing.
- A stream with no frame for 1 s reports `Stalled` (architecture section 8).
- `encode_png(frame, scale)` at scale 1/2.625 gives an image of `412x915` (±1) for a 1080x2400 frame.

**Step 2:** FAIL. **Step 3:** `openh264` decoder on a dedicated blocking thread fed by a bounded channel (capacity 8; when full, the reader blocks, which applies backpressure to the agent socket rather than dropping inter-predicted frames). Thumbnail: box-filter luma to one sample per 4×4 points (architecture 4.6; a 1/8 scale would make the measure depend on density). Files: `h264.rs` (wire format and decoder), `frame_store.rs`, `image.rs` (`encode_png`), `stream.rs` (`VideoStream`, the socket reader and decoder thread). **Step 4:** PASS. Commit.

### Task 2.5: AndroidDevice

**Files:** Create `crates/android/src/device.rs`, `crates/android/src/map.rs`.

**Step 1: Failing tests.**
- Unit: `map::raw_tree(agent_json, density)` converts px bounds to points, maps classes to roles, decodes flags; uses the fixture captured in Task 2.7 once it exists (start with the handmade one).
- Live (`#[ignore]`): `AndroidDevice::connect(&adb, serial)` then `tree()` on Settings contains a `Button`-role element labeled "Network & internet" (API 26's Settings says "Network & Internet", so the match ignores case).
- Live: `gesture(tap at the center of that element)` then within 2 s the tree contains an "Airplane mode" element and the main list's "Connected devices" button is gone. (The next screen's title is not a heading on API 37: Compose gives it role `Other`, a Phase 3 item.)

**Step 2:** FAIL. **Step 3:** Implement the `Device` trait. Capabilities: everything except `Ocr` for now. Convert points to px only here. **Step 4:** PASS. Commit.

### Task 2.6: Daemon core

**Files:** Create `crates/cli/src/daemon/{mod.rs,socket.rs,actor.rs,session.rs,rpc.rs}`, `crates/cli/tests/daemon.rs`.

**Step 1: Failing tests** using `FakeDevice` (Phase 0 task 0.10) injected through a `DeviceSource` the test controls:
- `hello` with the same version succeeds; a different version returns `-32010`.
- `devices` lists fake devices with tier and capabilities.
- Sessions outlive connections: connection A (session `default`) takes a snapshot and disconnects; connection B (session `default`) calls `act tap @e3` and the ref resolves. This is the CLI's normal case: one process per command.
- Two sessions: session `x` leases the device; session `y` gets `-32004 device_busy`; after `release` from `x`, `y` succeeds. `--share` lets `y` in without a release.
- A lease expires after 10 minutes without calls (`tokio::time::pause()` and advance).
- `snapshot` returns the rendered text from core render for the fake tree.
- `act tap @e3` resolves the ref against a fresh tree and sends a gesture at the element's center in points; when the fake's next tree lacks `@e3`, returns `-32006 stale_ref` with `closest`.
- `act` returns a diff after the fake reports a changed tree and goes quiet (drive the settle clock with `tokio::time::pause()`).
- `act` on a fake that never stops changing returns `ok` with `settled: false` and `still_changing`, not an error; `wait settled` on the same fake returns `-32012 not_settled`.
- `act` on a fake that never changes returns `changed: false` after 500 ms.
- Socket file mode is 0600 and its directory 0700 (Unix).

**Step 2:** FAIL. **Step 3:** Architecture section 5. One `DeviceActor` per device; `Session` holds `RefTable`, last `Rendered`, settle `Detector`. JSON-RPC parsing with `serde_json`; unknown methods return `-32601`. **Step 4:** PASS. Commit.

### Task 2.7: CLI verbs, auto-start, fixtures

**Files:** Modify `crates/cli/src/main.rs`; create `crates/cli/src/{client.rs,commands.rs}`; extend `crates/cli/tests/cli.rs`.

**Step 1: Failing tests.**
- With no daemon running, `phonedriver devices` starts one (the test sets `PHONEDRIVER_HOME` to a temp dir), prints the device table, and leaves a daemon that answers a second call without restarting (same pid in `hello`).
- `--json` output parses and matches the RPC result.
- Error exit codes: `needs_user` → 3, `device_gone`/`no_device` → 4, everything else → 1. Message on stderr, hint on the next line.
- `snapshot --json --save-fixture settings_main` writes `crates/core/tests/fixtures/android/settings_main.json` and `.png`.

**Step 2:** FAIL. **Step 3:** `clap` derive for the verbs in architecture 10.1 that this phase implements (`devices`, `snapshot`, `screenshot`, `tap`, `type`, `daemon`, `version`), plus the global `--session` and `PHONEDRIVER_SESSION`. Spawn with `std::process::Command::new(current_exe()).arg("daemon").arg("--detach")` and read the readiness line from its stdout pipe. Detaching: on Unix use `std::os::unix::process::CommandExt::process_group(0)` and have the daemon close stdout after the readiness line; on Windows `creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)`. Test the auto-start on macOS and on win-pc. **Step 4:** PASS. Capture fixtures for Settings main, Settings > Apps list, the testbed's screens. Replace the handmade fixture in core golden tests with real ones; review every insta snapshot by eye. Commit.

### Task 2.8: Android testbed

**Files:** Create `testbeds/android/` Gradle project (`settings.gradle.kts`, `app/build.gradle.kts`, minSdk 26, targetSdk 37, Java). Package `dev.phonedriver.testbed`.

Screens (one Activity with a tab bar, each tab a View-based screen, no Compose to keep the tree predictable; add one Compose screen later in Phase 3 to cover Compose semantics):
1. **Taps**: a 4×6 grid of buttons; each logs `{"ev":"tap","id":"b17","x_dp":…,"y_dp":…}` from the raw `MotionEvent` (not the click listener).
2. **Text**: three EditTexts (plain, multiline, password); logs every `afterTextChanged` as `{"ev":"text","field":"plain","value":…}` (password logs length only).
3. **List**: RecyclerView of 1,000 rows `Row 0…Row 999`; logs first and last visible row after each scroll settles.
4. **Gestures**: a full-screen view that logs every `MotionEvent` with action, pointer ids, coordinates in dp, and event time.
5. **Busy**: an endless animation plus a ProgressBar.
6. **Secure**: sets `FLAG_SECURE`, shows a label.
7. **Dialogs**: buttons that open an AlertDialog and a runtime permission request.
8. **Web**: a WebView loading a bundled HTML form.

Logs go to logcat tag `PUTB`, one JSON object per line. Add `crates/android/tests/support/testbed.rs`: build-and-install helper (`./gradlew installDebug` on the Mac with `ADB_SERVER_SOCKET` set), `clear_log()`, `read_log() -> Vec<serde_json::Value>`.

**Verify:** install on `Pixel_API_37`, open each tab, take a screenshot with `phonedriver screenshot`, and look at each one.

### Task 2.9: End-to-end ground-truth tests

**Files:** Create `crates/cli/tests/e2e.rs` (`#[ignore]`; it lives in the cli crate because `cargo_bin("phonedriver")` finds the binary only from the crate that owns it, and it includes the testbed helper from `crates/android/tests/support/testbed.rs` with `#[path]`).

Each test drives the real CLI binary (`assert_cmd`) against the testbed and checks the testbed's log:
- Tap on `@ref` of button `b17` → logged tap within ±1 dp of the element's center.
- Tap at `120,300` → logged at `120±1, 300±1` dp.
- `type "héllo 世界 مرحبا 👋🏽" --into <plain field>` → logged value equals the input exactly; reply has `verified: true`.
- `echo -n hunter2 | phonedriver type --secret --into <password field>` → logged length 7; `verified: true` by length; the trace file contains no `hunter2` (grep it).
- `phonedriver type "hunter2" --into <password field>` (no flag) → the trace still contains no `hunter2`, because the target is a `SecureField`.
- `phonedriver type --secret hunter2` (secret on argv) → usage error exit 2, nothing typed.
- `screenshot` of the Taps tab → PNG whose size equals the display size in px divided by density (±1), as reported by the agent's `hello`; look at it.
- Busy tab: `snapshot` replies `settled: true` with `settle_ms` under 100 and `loading` in the header (no idle wait); wall time through the SSH tunnel stays under 600 ms (a tree read alone is about 120 ms there, and the debug binary adds more; the 300 ms of the first draft is judged co-located in 2.10). The testbed's rotating label is 12 sp: at 24 sp it changed 2,500 to 3,100 pt² per frame and sometimes passed the 4,096 pt² threshold, which is not a small spinner.
- Secure tab: header shows `secure content hidden`; tree still contains the label.

Commit when all pass on API 37. Run again on API 26 and record results; failures on API 26 become Phase 3 tasks.

### Task 2.10: Benchmark and baseline (M1)

**Files:**
- Create `bench/Cargo.toml` (workspace member, binary `phonedriver-bench`, `publish = false`), `bench/src/main.rs`
- Create `bench/baseline/run_baseline.py` (not `phone_harness.py`: beside the venv's `phone_harness` package the script would shadow the import)
- Create `docs/bench/m1.md`

**Where:** build `phonedriver` and `phonedriver-bench` for Windows on win-pc (`ssh win-pc`; install Rust there if missing) and run both harnesses there, next to the emulator, with the default local adb server. Architecture section 12 explains why the tunnel is not used for budgets. If the Windows build needs fixes (named pipe, process detach), they are part of this task.

**phonedriver-bench**: for each op in architecture section 12 (Android column), run 50 iterations against the testbed after 5 warm-ups, through the daemon socket (the same path the CLI uses). Report p50/p95/max as a Markdown table. Ops: tap (Taps tab), snapshot (Settings main and the 1,000-row List tab), screenshot (static Taps tab), type 100 mixed chars, tap-to-settled-diff (a testbed toggle).

**Baseline**: on win-pc, install phone-harness 0.3.3 into an isolated venv (`uv venv ph && uv pip install --python ph phone-harness==0.3.3`). Run with `DO_NOT_TRACK=1 PHONE_HARNESS_TELEMETRY=0 PHONE_HARNESS_NO_UPDATE_CHECK=1 PHONE_HARNESS_PLATFORM=android` (its telemetry sends scripts and output; it must be off). Before the timed run, confirm no outbound connection during one call (`Get-NetTCPConnection -OwningProcess <pid>` in PowerShell) and the same `ADB_SERVER_SOCKET`. Time `tap`, `screenshot`, `screen()`, and `type_text` (ASCII only; record that the Unicode case is unsupported) with the same iteration counts.

**Gate M1:** `docs/bench/m1.md` holds both tables, machine and emulator details, and the commands. phonedriver must beat phone-harness on every op it supports. Where a budget from architecture section 12 is missed, profile (`samply` on the host, `simpleperf` or method tracing on the device), fix, and re-run before moving on. Record what you changed and why.

**Result (2026-11-05):** M1 met (3x to 150x faster on every op phone-harness supports). Budgets met: snapshot, screenshot, tap to settled diff; missed: tap (30 to 37 ms against 15) and type 100 chars (253 to 288 ms against 150), both explained in `docs/bench/m1.md` and decision D14. Fixes from profiling: `FLAG_PREFETCH_UNINTERRUPTIBLE` on the tree walk, a division-free `resample` for screenshots, and a handle-list `CreateProcessW` for the Windows daemon.

---

## Phase gate

- All non-ignored tests green; all ignored device tests green on API 37 and recorded for API 26.
- `docs/bench/m1.md` complete; M1 met.
- `progress.md` updated.
