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

## D3. iOS: CoreDevice services first, XCTest runner second, iPhone Mirroring never

**Choice.** On iOS 27+, use the developer services in the Developer Disk Image: display stream, HID, app service, pasteboard. Nothing installed on the phone. Add the PhoneUseRunner for the accessibility tree and for iOS 17 to 26.

**Rejected.** iPhone Mirroring (phone-harness): macOS only, one phone per Mac, no tree, ends when the phone unlocks, moves the user's cursor. WDA alone: needs signing for every use, slow tree dumps by default, HTTP + XML overhead.

**Evidence.** `idevice` implements `hid.rs` and `display_stream/` against iOS 27; go-ios independently implements the same services and documents the iOS 27 minimum.

**Reopen if.** Spike I1 shows the CoreDevice HID path fails on a real iOS 27 device, or Apple gates it behind Xcode-only entitlements.

## D4. Our own XCTest runner, not WebDriverAgent

**Choice.** A small XCTest bundle that serves our framing and uses WDA's techniques (private headers, bounded snapshots, event synthesis).

**Rejected.** Running WDA as-is: a WebDriver HTTP server, XPath engine, and session model we do not use; default full-depth XML snapshots; another protocol to translate.

**Reopen if.** The runner's snapshot on the testbed is not at least 2x faster than WDA `/source` with `snapshotMaxDepth` tuned. Then use WDA and delete the runner.

**Measured (2026-11-05, iPhone 17 simulator, iOS 26.5, 50 runs, `docs/spikes/ios.md`).** Runner snapshot p50 22.2 / 12.9 / 24.7 ms on the testbed List tab, Taps tab and Settings. WDA with `snapshotMaxDepth` 30 alone: 380.4 / 108.8 / 477.4 ms (runner 8x to 19x faster). WDA with every knob turned (depth 30, `excluded_attributes=visible,accessible,hittable`, idle waits off): 44.8 / 26.1 / 49.7 ms, so the runner is 2.02x / 2.02x / 2.01x faster at p50 and 2.03x / 1.96x / 1.99x at p95. The decision stands, but against fully tuned WDA the margin is about 1%; the runner reaches it only by requesting eight raw AX attributes instead of XCTest's key-path mapping. Re-measure after any Xcode update, and judge future runner changes against this bar.

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

## D11. Speak the adb server protocol directly

**Choice.** A small async client in `crates/android/src/adb.rs` for the five services listed in architecture 6.1.

**Rejected.** Spawning `adb` per call (latency, process churn). A third-party adb crate: the protocol subset is small and well documented (AOSP `packages/modules/adb/SERVICES.TXT`), and spike A0 writes most of it anyway.

## D12. Actions return a settled diff by default

**Choice.** Every action waits for settle (bounded) and returns the diff. `--no-settle` opts out.

**Rejected.** Fire-and-forget actions plus a separate snapshot: doubles round trips and invites sleep-and-hope.

## D13. License

**Recommendation.** Apache-2.0, which matches scrcpy (the code we borrow wrappers from) and includes a patent grant. **Needs the user's decision before any public release.** Development proceeds without a LICENSE file until then.

**Also before release.** `openh264` built from source (the crate's default) does not carry Cisco's H.264 patent license, which covers only Cisco's prebuilt binaries. Decide with the user whether release builds load Cisco's binary at runtime or ship the source build.
