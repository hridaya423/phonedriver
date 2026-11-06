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

**Pin (2026-11-06, task 6.1).** crates.io `idevice` 0.1.68 has all four features the plan lists (`display_stream`, `core_device`, `xctest`, `tunnel_tcp_stack`; read from the crate's `Cargo.toml` after `cargo info idevice@0.1.68`), so the dependency is the registry crate, not the git rev `8970209`: `idevice = { version = "=0.1.68", default-features = false, features = ["ring", "usbmuxd", "pair", "core_device", "display_stream", "xctest", "tunnel_tcp_stack"] }` in `crates/ios`. `default-features = false` drops `aws-lc` (a C++ build); `ring` is the TLS backend lockdown needs, `pair` adds the pairing error variants and `usbmuxd` discovery. The exact `=` matters: `IdeviceError` is `#[non_exhaustive]` and the README says every release breaks the API. Features for later tasks (`amfi`, `mobile_image_mounter`, `installation_proxy` comes in through `xctest`) are added when their task needs them. The CoreDevice service names `phone-use` checks (`com.apple.coredevice.hid.universalhidservice`, `com.apple.coredevice.displayservice`) were read from idevice's `hid.rs` and `display_stream/client.rs`.

**Codec (2026-11-06, spike I3, iPad iOS 27.0).** The CoreDevice tier offers only the H.264 bank (RTP payload type 123). The device accepts it and sends H.264, High profile, level 5.2, 2736 x 2064, which `openh264` decodes. With idevice's default offer the device sends HEVC on payload type 100 (idevice's labels are right, go-ios's are wrong). One software decoder on every OS beats VideoToolbox plus a fallback. Reopen if a device refuses an H.264-only offer.

**Status (2026-11-06).** The CoreDevice tier is built (stream, HID touch, buttons, keyboard, pasteboard, app service) and its input is proven on the iPad against the testbed's event log. The frozen screen was a receiver backlog, not stale content (architecture 7.2, spike I6): fixed by publishing only the newest decoded picture. Measured on the iPad in a development build with the video crates optimized: tap 31.7 ms p50 (n=20), tap to the first changed frame 378 ms p50 in a quiet 23-tap run, but runs of 2 to 3 minutes on a host with a load average above 20 still fell behind again (20 to 75 of 60 to 100 taps saw no change within 3 s). The daemon keeps the Runner tier on iOS 27 unless `PHONE_USE_IOS_TIER=coredevice`; Update 2026-11-06: with cheap publishing (the quarter turn is deferred to the screenshot) a release build kept up for 135 s, 95 taps, 0 missed (spike I6 follow-up, m5.md), and is 5 to 30 times faster than the Runner. It still is not the default because it has no accessibility tree (snapshot is OCR without refs) and does not follow device orientation (landscape comes out sideways); default when those two are solved.

**Floor (2026-11-06, spike I2).** The CoreDevice services are listed on an iPhone with iOS 26.1, but starting the display stream there fails with "Remote control requires iOS 27.0 or later on this device". The CoreDevice tier needs iOS 27; iOS 17 to 26 use the Runner tier.

## D4. Our own XCTest runner, not WebDriverAgent

**Choice.** A small XCTest bundle that serves our framing and uses WDA's techniques (private headers, bounded snapshots, event synthesis).

**Rejected.** Running WDA as-is: a WebDriver HTTP server, XPath engine, and session model we do not use; default full-depth XML snapshots; another protocol to translate.

**Reopen if.** The runner's snapshot on the testbed is not at least 2x faster than WDA `/source` with `snapshotMaxDepth` tuned. Then use WDA and delete the runner.

**Measured (2026-11-05, iPhone 17 simulator, iOS 26.5, 50 runs, `docs/spikes/ios.md`).** Runner snapshot p50 22.2 / 12.9 / 24.7 ms on the testbed List tab, Taps tab and Settings. WDA with `snapshotMaxDepth` 30 alone: 380.4 / 108.8 / 477.4 ms (runner 8x to 19x faster). WDA with every knob turned (depth 30, `excluded_attributes=visible,accessible,hittable`, idle waits off): 44.8 / 26.1 / 49.7 ms, so the runner is 2.02x / 2.02x / 2.01x faster at p50 and 2.03x / 1.96x / 1.99x at p95. The decision stands, but against fully tuned WDA the margin is about 1%; the runner reaches it only by requesting eight raw AX attributes instead of XCTest's key-path mapping. Re-measure after any Xcode update, and judge future runner changes against this bar.

**Re-measured (2026-11-05, task 5.6, M4, `docs/bench/m4.md`).** Runner p50 21.5 to 21.9 ms (List) and 24.1 to 24.4 ms (Settings), 2.04x to 2.08x of 5.3's fully tuned WDA; on the Taps tab as 5.3 measured it (59 nodes) 12.6 to 12.7 ms in three series, 2.06x. WDA itself was not re-measured (its checkout went with spike I4 and the mini had 2 GB free). The Taps tab now has a toggle (62 nodes, 13.1 to 13.6 ms), so future comparisons need WDA re-measured on the new screen. The decision stands.

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

**Choice.** AGPL-3.0-only, decided by the user on 2026-11-06, replacing MIT before anything was published. The user wanted a restrictive license so a competing tool cannot take the code into a closed or permissive project; AGPL is the strongest OSI open-source license (a distributed or network-served modified version must be published under AGPL). `LICENSE` holds the text and every crate sets `license = "AGPL-3.0-only"` through the workspace. Code copied from scrcpy (Apache-2.0) and the WebDriverAgent headers (BSD) keep their own licenses and attribution in `NOTICE`; both are compatible with AGPL-3.0. Every dependency is permissive (MIT, Apache-2.0, BSD, ISC, Zlib, 0BSD, Unlicense; `cargo metadata`, 2026-11-06).

**Rejected.** MIT (chosen earlier the same day): lets a competitor take the code without giving anything back. FSL-1.1 and BSL-1.1 (non-compete source-available): stronger against a competing product, but not open source. PolyForm Strict: forbids modification, so no forks or contributions. Apache-2.0: permissive like MIT. `-only` rather than `-or-later` keeps the license under the owner's control rather than the FSF's future versions.

**Also before release.** `openh264` built from source (the crate's default) does not carry Cisco's H.264 patent license, which covers only Cisco's prebuilt binaries. Decide with the user whether release builds load Cisco's binary at runtime or ship the source build.

## D14. The last event of a gesture waits for the app (tap budget stays a target)

**Choice.** The agent injects the last event of a gesture with `WAIT_FOR_FINISH` (architecture 6.2), so a tap's reply means the app handled it. Measured on the API 37 emulator, tap p50 is 30 to 37 ms against the 15 ms budget: about 14 ms is the app handling the event, about 7 ms adb transport.

**Rejected.** Injecting `ASYNC`: tap p50 15.1 ms, still not under 15, and "delivered" would mean "queued", so a `--no-settle` snapshot straight after a tap could read the old screen.

**Reopen if.** A physical phone over USB, with a hardware renderer, shows the same gap (Phase 3), or an agent workload shows taps dominating wall time.

## D15. The Simulator tier waits for XCTest's event completion, and types instead of pasting

**Choice.** The runner replies to `gesture`, `type` and `key` only after XCTest's synthesis completion. On the Simulator, `paste` mode and secret text are typed through the runner's redacted text input at 5000 characters per second; the pasteboard is not used.

**Measured (2026-11-05, iPhone 17 simulator, iOS 26.5).** A tap's reply takes 237 to 250 ms (10 runs each: on an empty spot, on a button, zero-length or 50 ms long; a 200 ms press 427 ms, a 300 ms swipe 528 ms), while the testbed logs `began` 14 to 17 ms and `tap` 30 to 50 ms after the request. Setting XCTest's implicit event confirmation interval to 0 or 0.05 s changed nothing. So the 80 ms tap budget is missed by about 3x, and every action's settle starts about 200 ms late. Typing 102 mixed characters: 1,890 ms at 60 per second, 553 at 300, 321 at 1000, 267 at 5000, exact every time. A synthesized Cmd+V pasted text the app had copied itself but not text set by the runner or by `simctl pbcopy`.

**Rejected.** Replying when the event's own duration has passed (about 40 ms for a tap): faster, but "delivered" would be a guess from a schedule, the same objection as D14. Pasting through the pasteboard: it does not work for text from another process, and it would leave a secret there.

**Reopen if.** A newer Xcode removes the completion delay, the CoreDevice tier (which injects HID directly) shows taps are needed faster on Simulators too, or an agent workload is dominated by Simulator taps.


## D17. Batching is a daemon method and one MCP tool, not a client loop

**Choice.** `batch` takes a list of steps and runs them inside the actor, each settled like a single call, and returns one reply: the steps that ran and a diff from what the session last saw to the end. It stops at the first error and names the step. The MCP side is one new tool, `batch`, whose steps are `tool` plus that tool's arguments, so the schema costs about 100 tokens and does not repeat the other tools' arguments.

**Why.** The agent audit found that calls, not snapshot tokens, are the cost (each call re-reads 6k to 28k tokens of context) and that 7 of 9 agents sent dependent calls in parallel. A client-side loop over single calls would return one reply per step (or discard the intermediate diffs while still paying for them) and could not keep the steps inside one lease and one baseline.

**Rejected.** Running steps without settling between them: a later step would resolve its target on a stale screen. A step list that carries refs only: refs from a screen the earlier step replaces go stale, so the failure is reported by step and the agent re-reads. Secret text in a batch: the trace and the reply paths for secrets are written for a single `type`.

**Reopen if.** Agents use batches of one step, or a batch that fails halfway too often to be worth the saved calls.

## D18. CLI verbs the MCP lacked are folded into existing tools, except the clipboard

**Choice.** `drag` and `pinch` are new daemon actions and CLI verbs; on MCP they are arguments of `swipe` (`to`, `pinch`). `clear` is `type` with empty text, `find` is `snapshot` with `find`, `release` is `devices` with `release`. Only `clipboard` became a tool (13 tools), because reading and writing the device clipboard fits no other tool.

**Why.** Every tool's schema is paid for in every session (the audit measured about 1.8k tokens for ten tools), and the audit found agents never used most optional arguments, so an argument costs a few tokens where a tool costs 40 to 100. Competitors' own multiplexing (Appium's `appium_gesture` has eight actions) agrees.

**Rejected.** One MCP tool per CLI verb (`find`, `clear`, `release`, `gesture`, `drag`, `pinch`): +6 tools. Exposing the raw `gesture` JSON on MCP: an agent would compute finger coordinates itself, which `drag` and `pinch` do from targets.

**Reopen if.** An agent run shows models failing to find `drag` under `swipe`.

## D19. App lifecycle is one `app` method, folded into `apps` on MCP, with a ledger on physical devices

**Choice.** `install`, `stop`, `clear` and `uninstall` are one daemon method and arguments of the existing `apps` tool. On a physical device phone-use clears or removes only what it installed (a per-device record in its home); everywhere else it acts on any app.

**Why.** An agent that can uninstall can lose a person's data; an emulator or Simulator is disposable, a phone is not. A record of our own installs is the narrowest rule that still lets an agent test the app it just installed.

**Rejected.** Allowing uninstall on a phone after a prompt (no way to ask a person from a tool call). Four MCP tools (+300 tokens every session).

**Reopen if.** An agent needs to clear the data of an app it did not install on a phone; that is a decision for the person, per app.

## D16. Traces keep typed text only as a length, and text from the screen only with `--trace-full`

**Choice.** A trace line holds the request's params, the reply's summary, `ms` and the settle time. A `type` action's text, secret or not, becomes `{"secret_len": n}` (flag set) or `{"text_len": n}`; so does the text of `clipboard set`. Text targets (`tap "Save"`, `wait gone:"Loading"`, `find`) become `{"text_len": n}` unless the daemon runs with `--trace-full` (env `PHONE_USE_TRACE_FULL`), which also adds the observation text. An `open` URL loses its query and fragment. `--trace-screens` (env `PHONE_USE_TRACE_SCREENS`) copies each `screenshot` next to the trace file. The trace directory is mode 0700 and its files 0600.

**Why not keep unflagged typed text (architecture 5 said to).** Whether a field is a `SecureField` is known only inside the actor, and a typing error never reaches the point where it is known; a rule that depends on it can leak on exactly the failing path. Typed text is also what a person says in a message. Lengths still show that typing happened and how much.

**Rejected.** Recording typed text unless the actor reports `secure`: needs a new reply field and still cannot cover errors.

**Reopen if.** Tuning agents (task 4.5) shows that traces without typed text cannot explain failures; then add the text under `--trace-full` only.

## D17. Android support is a cargo feature of the CLI, on by default

**Choice.** `phone-use` has a feature `android` (default) that pulls in `phone-use-android`, whose build script packs the on-device agent jar. `cargo build --release -p phone-use --no-default-features` builds an iOS-only binary that needs no Android SDK, `d8` or JDK; it has no adb source and `phone-use doctor` skips the Android checks. The default build is unchanged and still packs the jar. `android-agent/build.sh` checks its tools first and, in one message, names each one that is missing, what it found, and the four ways out (install, `ANDROID_HOME`/`ANDROID_BUILD_TOOLS`, `PHONE_USE_AGENT_JAR`, `--no-default-features`).

**Why.** A newcomer who wants the iOS Simulator on a Mac without Android build-tools 37.0.0 could not build at all (dx-ux audit, finding 1 and 2), and the jar the build asked for is not published anywhere. There is no `ios` feature: iOS code builds on every host already, so a second switch would have nothing to turn off.

**Rejected.** Accepting any build-tools at or above 34: the jar is compiled against `android-37.0` and nothing has verified an older toolchain; `ANDROID_BUILD_TOOLS` and `ANDROID_PLATFORM` already let a user try. Downloading a prebuilt jar in the build script: sends a request nobody asked for (D9 spirit) and no release exists to download from.

**Reopen if.** Releases attach `phone-use-agent.jar`, which would let a source build skip the SDK without losing Android.

## D18. A device that cannot be driven yet is not the default; a missing signing team is a user action

**Choice.** With several devices connected and none named, the daemon picks the one device that has no blocker. Only the physical iOS source reports a blocker: a Runner-tier iPhone or iPad while `PHONE_USE_TEAM` is unset. So a paired iPhone with no team no longer makes a booted Simulator "ambiguous", and a Simulator is chosen over it. With one device connected it is still the default, because the error that follows (`NeedsUser(SetSigningTeam)`, exit 3) is the useful answer. With two ready devices, or none ready, the answer stays `AmbiguousDevice`.

**Why.** The audit (dx-ux finding 3) showed a newcomer with one paired iPhone and a README written for the Simulator getting a failure about a signing team. Choosing the device that works costs nothing when the user has said nothing; naming a device is still how to pick the iPhone. The missing team was a `Transport` error ("connection error", hint `doctor`) and quoted a team id that looked real; it is now `NeedsUser(SetSigningTeam)` with no example id, where to find the id, and the `ios setup` and `daemon stop` steps.

**Rejected.** Picking a physical device over a Simulator when both are ready: a person who plugged in a phone and has a Simulator booted may mean either. Hiding blocked devices from `devices`: the table should show everything the daemon sees.

**Reopen if.** Another source gains a blocker (an Android device that needs a setting only a person can change), which would make the rule matter more.
