# Decisions

Each decision records the choice, the alternatives it beat, and the evidence that would reopen it. Numbered for reference from other docs. Add new ones at the end; never renumber.

## D1. One Rust binary with a resident daemon

**Choice.** A single `phone-use` binary that acts as CLI, daemon, and MCP server. The daemon holds device connections between calls.

**Rejected.** A per-call process (phone-harness): every call pays connection setup, agent startup, decoder warm-up, and loses ref identity. Python or Node: startup cost, packaging, and no good native story for video decode and the iOS stack. Go: `idevice` (Rust) is the most complete iOS protocol stack that embeds cleanly; the user's tooling (`blaze`) is Rust.

**Reopen if.** Never for language. For the daemon: if a measured cold CLI call to a warm daemon exceeds 10 ms of overhead, profile the RPC path.

## D2. Android: a resident agent started with `app_process` as the shell user

**Choice.** Push a dex jar and run it with `app_process`, as scrcpy does. It injects input through `InputManager`, mirrors the display into a MediaCodec encoder, reads the tree through a UiAutomation connection, and keeps one socket open to the host.

**Rejected.**
- `adb shell input …` and `uiautomator dump` (phone-harness): a JVM per tap, seconds per tree, no Unicode, no multi-touch, fails on animating screens.
- An installed AccessibilityService APK (droidrun-style): needs the user to enable the service in Settings, survives as an app on the phone, and cannot inject input without extra permission.
- An instrumentation APK (uiautomator2/Appium): needs install, test-APK signing, and still pays instrumentation overhead.

**Evidence.** scrcpy runs this way on API 21 to current. AOSP's own `uiautomator` command creates a UiAutomation from a shell `app_process`.

**Reopen if.** Spike A1 shows UiAutomation cannot connect from `app_process` on API 26 or API 37. Fallback: the same agent code packaged as an instrumentation APK started with `am instrument`, used only for the tree.

**Spike A1 result (2026-11-05, API 26 and 37 emulators): confirmed, no fallback needed.** `new UiAutomation(Looper, IUiAutomationConnection)` plus hidden `connect(int)` works from an `app_process` shell on both; FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES keeps TalkBack bound (`Bound services` stays listed) while flags 0 unbinds it. Added requirements: `clearCache()` before every tree (the cache goes stale because we do not subscribe to content events), and a second UiAutomation client makes our connect throw `IllegalStateException: UiAutomationService ... already registered!` (the `AutomationConflict` signal). See `docs/spikes/android.md` A1.

## D3. iOS: CoreDevice services first, XCTest runner second, iPhone Mirroring never

**Choice.** On iOS 27+, use the developer services in the Developer Disk Image: display stream, HID, app service, pasteboard. Nothing installed on the phone. Add the PhoneUseRunner for the accessibility tree and for iOS 17 to 26.

**Rejected.** iPhone Mirroring (phone-harness): macOS only, one phone per Mac, no tree, ends when the phone unlocks, moves the user's cursor. WDA alone: needs signing for every use, slow tree dumps by default, HTTP + XML overhead.

**Evidence.** `idevice` implements `hid.rs` and `display_stream/` against iOS 27; go-ios independently implements the same services and documents the iOS 27 minimum.

**Reopen if.** Spike I1 shows the CoreDevice HID path fails on a real iOS 27 device, or Apple gates it behind Xcode-only entitlements.

## D4. Our own XCTest runner, not WebDriverAgent

**Choice.** A small XCTest bundle that serves our framing and uses WDA's techniques (private headers, bounded snapshots, event synthesis).

**Rejected.** Running WDA as-is: a WebDriver HTTP server, XPath engine, and session model we do not use; default full-depth XML snapshots; another protocol to translate.

**Reopen if.** The runner's snapshot on the testbed is not at least 2x faster than WDA `/source` with `snapshotMaxDepth` tuned. Then use WDA and delete the runner.

## D5. Logical points as the only public coordinate space

**Choice.** iOS points, Android dp, top-left origin, current orientation. Screenshots default to 1 px per point.

**Rejected.** Device pixels: differ by density; a model's guess on a downscaled image becomes wrong by 3x. Normalized 0..1: hard for models and humans to reason about.

## D6. Typed operations; never execute agent-written code

**Choice.** The agent calls verbs (CLI, MCP, SDK). Batching is a sequence of verbs, not a program.

**Rejected.** Python-on-stdin with pre-imported helpers (phone-harness): a full-privilege code execution channel with a cosmetic blocklist. Agents that want loops can write scripts in their own sandbox against the CLI or SDK.

## D7. Length-prefixed JSON on every internal wire

**Choice.** `u32 big-endian length + UTF-8 JSON` for the Android agent and the iOS runner. Newline-delimited JSON-RPC 2.0 on the daemon socket. Video is binary (Section "video" in `docs/protocol.md`).

**Rejected.** Protobuf: a codegen step on three languages and a runtime dependency in the dex jar. Custom binary: faster but only worth it with evidence.

**Reopen if.** Benchmarks show tree serialization or parsing above 20 ms on the testbed's 1,000-row screen.

**Spike A1 evidence:** the full tree JSON is 8 to 27 KB for 59 to 192 nodes (Settings main 27 KB on API 37 with the status and navigation bars); serialization is inside the 43 to 130 ms measured tree time and was not separated out. The 20 ms reopen trigger is not reached by anything we measured, but is not proven either; Phase 2 task 2.5 measures parse time on the host.

## D8. Android minimum API 26 (Android 8.0)

**Choice.** API 26 and above. Prefetch-based tree fetch on API 33+, node-by-node below.

**Rejected.** API 21 like scrcpy: more reflection branches for a shrinking share of devices.

## D9. No telemetry

**Choice.** None. No install id, no events, no crash upload.

**Rejected.** Opt-out telemetry (phone-harness), which shipped scripts, output and arguments. If usage data is ever needed, it will be opt-in, field-listed, and reviewed by the user first.

## D10. Continuous video stream for screenshots and settle

**Choice.** Keep an encoder stream running while a device is leased. Screenshot = encode the latest decoded frame. Settle uses frame arrival and difference.

**Rejected.** On-demand capture (`screencap`, `SurfaceControl.screenshot`): every look pays capture and encode on the device, and there is no change signal between looks.

**Cost.** One encoder on the phone, one software decoder on the host (about one CPU core at most at 1080p30). Stream stops when the lease ends. The encoder repeats the last frame every 100 ms on a static screen (as scrcpy does), so the host always has a current frame and can tell a stalled stream from a quiet screen.

**Spike A3 evidence (API 26 and 37 emulators):** the stream decodes with `openh264` with no errors; decode p50 3.6 to 4.6 ms and p95 7.3 to 9.6 ms per 1080-wide frame on the Mac (release build). First decoded frame after opening the stream: p50 219 ms (API 26) and 433 ms (API 37), which misses the 200 ms target and is the one-time cost per lease (the software encoder takes 140 to 230 ms to start); the stream's static frame rate is about 3 fps, not 10; the 4,096 pt² `significant_area` cannot see a switch toggle (see architecture 4.6, now two thresholds). Tap-to-changed-frame latency p50 223 to 352 ms on the emulators. Holds as a decision; see `docs/spikes/android.md` A3.

**Task 2.7 correction (2026-11-05):** the stream's frame rate on a static screen is not a heartbeat. On the API 37 emulator through the tunnel the stream sent bursts of about 10 frames per second and then nothing for up to 5 s while the screen did not change, so `no frame for 1 s` made 18 of 40 screenshots of an idle screen fail. Screenshots now serve the last frame unless the video socket has ended (architecture 8). Settle is unaffected: it needs frames only when the screen changes.

## D11. Speak the adb server protocol directly

**Choice.** A small async client in `crates/android/src/adb.rs` for the five services listed in architecture 6.1.

**Rejected.** Spawning `adb` per call (latency, process churn). A third-party adb crate: the protocol subset is small and well documented (AOSP `packages/modules/adb/SERVICES.TXT`), and spike A0 writes most of it anyway.

**Spike A0 evidence:** the whole client (version, track-devices-l, transport + shell,v2,raw, sync push, localabstract) is about 150 lines of Rust and worked on the first try over the SSH tunnel on API 26 and 37. A `shell,v2,raw:echo hi` from our client takes p50 52 to 64 ms (tunnel floor 38 to 40 ms) against 247 to 248 ms for the `adb` CLI; `localabstract:` streams work through the tunnel with a 13 ms ping; a shell,v2 child receives stdin without closing it (the token path works). Confirmed.

## D12. Actions return a settled diff by default

**Choice.** Every action waits for settle (bounded) and returns the diff. `--no-settle` opts out.

**Rejected.** Fire-and-forget actions plus a separate snapshot: doubles round trips and invites sleep-and-hope.

## D13. License

**Recommendation.** Apache-2.0, which matches scrcpy (the code we borrow wrappers from) and includes a patent grant. **Needs the user's decision before any public release.** Development proceeds without a LICENSE file until then.

**Also before release.** `openh264` built from source (the crate's default) does not carry Cisco's H.264 patent license, which covers only Cisco's prebuilt binaries. Decide with the user whether release builds load Cisco's binary at runtime or ship the source build.
