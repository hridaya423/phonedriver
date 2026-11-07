# iOS spikes

## I4: WebDriverAgent baseline and runner snapshot technique (2026-11-05)

**Setup.** Mac mini, Xcode 27.0 (27A266a), iOS 27.0 simulator SDK, iPhone 17 simulator running iOS 26.5 (the only simulator runtime on the mini). WebDriverAgent `3d6a4e0` built with `xcodebuild -scheme WebDriverAgentRunner test` (WDA 16.15.3). Client on the same Mac over loopback. Screens: testbed List tab (`-tab list`), testbed Taps tab (`-tab taps`), Settings root (`com.apple.Preferences`). Each row: 2 warm-up calls, then 20 timed calls; p50 and p95 in milliseconds. Spike code was `spikes/ios-runner-snapshot/` (`time_wda.py` and an Objective-C XCTest), deleted at the end of the phase.

### WDA `GET /session/:id/source` (HTTP round trip, client side)

| Screen | Settings | p50 | p95 | Elements | Tree depth |
|---|---|---|---|---|---|
| List | default (`snapshotMaxDepth` 50) | 380.7 | 388.0 | 150 | 12 |
| List | `snapshotMaxDepth` 30 | 380.7 | 383.9 | 150 | 12 |
| List | depth 30, `waitForIdleTimeout` 0, `animationCoolOffTimeout` 0, `excluded_attributes=visible,accessible,hittable` | **45.0** | 45.4 | 150 | 12 |
| List | same, `format=json` | 193.7 | 198.7 | 150 | |
| Taps | default | 109.2 | 110.2 | 59 | 8 |
| Taps | depth 30 | 109.1 | 109.6 | 59 | 8 |
| Taps | depth 30, idle waits off, excluded attributes | **26.5** | 27.3 | 59 | 8 |
| Taps | same, `format=json` | 83.7 | 85.3 | 59 | |
| Settings | default | 476.7 | 482.6 | 168 | 21 |
| Settings | depth 30 | 477.0 | 481.5 | 168 | 21 |
| Settings | depth 30, idle waits off, excluded attributes | **49.9** | 50.2 | 168 | 21 |
| Settings | same, `format=json` | 218.4 | 220.5 | 168 | |

An earlier pass (same build, 20 runs) also tried smaller depths: `snapshotMaxDepth` 15 cuts Settings to 25 of 168 elements (72 ms) and 10 cuts the List tab to 122 of 150 elements, so depth below about 21 loses real content on stock apps. Depth 30 is the smallest round value that keeps every tree complete; it does not change the time when the tree is shallower than 30.

What dominates WDA's time is the per-element `visible`, `accessible` and `hittable` attributes, not the depth: excluding them cuts 380 ms to 45 ms on the List tab. The JSON format is slower than XML because it computes more per element.

### In-process snapshot inside an XCTest (no transport)

`XCAXClient_iOS` from `[XCUIDevice.sharedDevice accessibilityInterface]`, element = first of `activeApplications`, `requestSnapshotForElement:attributes:parameters:` with `defaultParameters` plus `maxDepth`. The reduced attribute list comes from `+[XCElementSnapshot axAttributesForElementSnapshotKeyPaths:isMacOS:]` with `label, value, identifier, frame, elementType, enabled, selected, hasFocus, placeholderValue`, which returns an `NSSet` of 12 AX names (pass `allObjects`; an `NSSet` raises `array argument is not an NSArray`). Enabled and selected travel inside `XC_kAXXCAttributeTraits`. The standard list (`sanitizedElementSnapshotHierarchyAttributesForAttributes:nil`) is 22 names and is already in AX form. Defaults on this Xcode: `maxDepth`, `maxChildren`, `maxArrayCount` all `INT_MAX`.

| Screen | Variant | p50 | p95 | Nodes | Depth |
|---|---|---|---|---|---|
| List | public `-[XCUIApplication snapshotWithError:]` | 34.2 | 34.8 | 150 | 12 |
| List | AX, standard attributes, depth 30 | 24.9 | 25.0 | 150 | 12 |
| List | AX, reduced attributes, depth 30 | 20.7 | 21.0 | 150 | 12 |
| List | AX reduced + flatten to `RawNode` + JSON | **21.2** | 21.5 | 150 | 12 |
| Taps | public `snapshotWithError:` | 19.0 | 19.4 | 59 | 8 |
| Taps | AX standard, depth 30 | 13.5 | 13.7 | 59 | 8 |
| Taps | AX reduced, depth 30 | 11.8 | 12.4 | 59 | 8 |
| Taps | AX reduced + flatten + JSON | **12.2** | 13.9 | 59 | 8 |
| Settings | public `snapshotWithError:` | 37.6 | 38.0 | 168 | 21 |
| Settings | AX standard, depth 30 | 28.2 | 28.6 | 168 | 21 |
| Settings | AX reduced, depth 30 | 23.0 | 23.9 | 168 | 21 |
| Settings | AX reduced + flatten + JSON | **23.7** | 23.9 | 168 | 21 |

Flattening to `RawNode` and encoding JSON costs under 1 ms at these sizes.

### What this sets for D4

Two readings of "WDA with `snapshotMaxDepth` tuned":
- Depth only (the literal D4 wording): 381 / 109 / 477 ms. The technique beats it by 9x to 20x.
- Every WDA knob a careful user would turn (depth 30, idle waits off, `excluded_attributes`): 45.0 / 26.5 / 49.9 ms. Half of that is 22.5 / 13.3 / 25.0 ms. The in-process technique (21.2 / 12.2 / 23.7 ms) is just under half before any transport cost, so the runner can meet 2x only if its transport and request handling add about 1 ms.

We hold the runner to the stricter bar. Task 5.3 measures it end to end over TCP.

## D4: runner end to end vs WebDriverAgent (2026-11-05, task 5.3)

Same simulator, screens and client machine as I4. Runner: `snapshot` with `max_depth` 30 over its TCP socket (length-prefixed JSON, `TCP_NODELAY`), measured from the client. WDA: `GET /session/:id/source` over HTTP. The runner and WDA ran one after the other, never together. 2 warm-up calls, then 50 timed calls per row; times in ms.

| Screen | Runner p50 | Runner p95 | WDA depth 30 p50 | WDA fully tuned p50 | WDA fully tuned p95 | Ratio p50 (tuned / runner) | Ratio p95 |
|---|---|---|---|---|---|---|---|
| List (150 elements) | 22.2 | 22.7 | 380.4 | 44.8 | 46.1 | **2.02x** | 2.03x |
| Taps (59) | 12.9 | 13.5 | 108.8 | 26.1 | 26.5 | **2.02x** | 1.96x |
| Settings (168) | 24.7 | 25.5 | 477.4 | 49.7 | 50.7 | **2.01x** | 1.99x |

"Fully tuned" means depth 30, `waitForIdleTimeout` 0, `animationCoolOffTimeout` 0, `excluded_attributes=visible,accessible,hittable`. With depth 30 and excluded attributes only (idle waits left at defaults), WDA measured 48.6 / 26.3 / 49.9 ms.

**Verdict.** D4's literal bar (WDA with `snapshotMaxDepth` tuned) is met by 8x to 19x. Against WDA with every knob turned, the runner is just over 2x at p50 on all three screens and just under 2x at p95 on Taps and Settings. The margin is about 1%, small enough that run-to-run noise or an Xcode update can flip it.

**How the runner got there.** The first runner build requested the attribute list produced by `axAttributesForElementSnapshotKeyPaths:` (12 AX names, including window context and display ids, automation type and base type). It measured 23.4 / 13.5 / 26.0 ms (20 runs), which is 1.91x to 1.93x of fully tuned WDA, a miss. Profiling inside the runner (List tab) showed `activeApplications` at 1.4 ms, the AX request at 21 to 24 ms, flattening at 0.45 ms, JSON at 0.4 ms and the main-thread hop under 0.05 ms. Requesting 8 raw names (label, value, identifier, frame, element type, traits, placeholder value, native focus) saved about 1.2 to 1.5 ms. It gave byte-identical node lists on all nine testbed tabs, including the WKWebView, the on-screen keyboard (36 key nodes) and an alert; the only diff was the Busy tab's spinning image, which moved between the two snapshots. The per-snapshot `activeApplications` call (1.4 ms) stays, because it is how the runner follows app switches.

**Runner screenshots** (`XCUIScreen.mainScreen.screenshot`, 1206 x 2622 px, 50 runs): JPEG q80 p50 75 to 81 ms, JPEG q30 74 to 78 ms, PNG 62 to 64 ms. Encoding is not the bottleneck (q30 saves 2 ms), so task 5.6's "JPEG q30 at half scale for settle polling" idea needs scaling, not just quality, to help. All are under the 300 ms threshold in task 5.6.

## Device access probe, Mamata's iPhone (2026-11-06, task 6.2 start; stopped)

Not I2 yet: nothing could be paired or tunneled. What was observed, with `xcrun devicectl` and a throwaway idevice 0.1.68 program (`spikes/ios-device/`, deleted):

- The phone is **iOS 26.1** (build 23B85), not 26.5 as the planning notes say. Developer Mode is enabled. Model iPhone14,8, UDID 00008110-00142036217A201E.
- It is reachable **only over the network**, not USB: `devicectl` reports `transportType: localNetwork`, `tunnelState: disconnected` when idle (the CoreDevice tunnel exists only while a `devicectl` or `xcodebuild` command holds it), `system_profiler SPUSBDataType` and `ioreg -p IOUSB` list no iPhone, `dns-sd` finds a `_remotepairing._tcp` advertisement and no `_apple-mobdev2._tcp`, and idevice's `UsbmuxdConnection::get_devices()` returned an empty list. The usbmuxd and lockdown path that `crates/ios/src/transport.rs` assumes therefore cannot see this phone. idevice also has a `remote_pairing` module for the network path; it needs its own pairing record, created by a person approving a prompt on the phone, and was not tried.
- Signing works: the testbed built for `generic/platform=iOS` with `-allowProvisioningUpdates DEVELOPMENT_TEAM=DV6NC6KRYL CODE_SIGN_STYLE=Automatic CODE_SIGN_IDENTITY="Apple Development"` in 10 s, signed by "Apple Development: Mukesh Agrawal (HSLSDPZZDT)", profile "iOS Team Provisioning Profile: *" (wildcard, team Mukesh Agrawal, expires 2027-10-20, lists this UDID). `devicectl device install app` installed `dev.phonedriver.testbed` without a prompt.
- `devicectl device process launch dev.phonedriver.testbed` failed: `Locked ("Unable to launch dev.phonedriver.testbed because the device was not, or could not be, unlocked")`. The phone is locked with a passcode, and phonedriver never enters passcodes, so work stopped here.

## I2: RSD services by iOS version (2026-11-06, task 6.2)

Both devices wired over USB, usbmuxd and lockdown, then `CoreDeviceProxy` over the userspace TCP stack and `RsdHandshake` (`crates/ios/src/transport.rs`, `Tunnel::open`).

| Device | iOS | RSD services | `coredevice.displayservice` | `hid.universalhidservice` | `hid.indigo` | `pasteboardservice` | `axAuditDaemon.remoteAXService` |
|---|---|---|---|---|---|---|---|
| iPhone 14 Plus | 26.1 | 83 | listed | listed | listed | listed | listed |
| iPad Pro 13-inch (M5) | 27.0 | 84 | listed | listed | listed | listed | listed |

Listing is not availability. Starting the display stream on the iPhone (iOS 26.1) fails with "Remote control requires iOS 27.0 or later on this device", so the CoreDevice tier needs iOS 27 and `choose_tier` keeps its `major >= 27` condition. The planning notes' "expect no `universalhidservice` below 27" was wrong: the service is listed on 26.1 and refuses to work. Live discovery (`crates/ios/tests/physical_live.rs`) returns the iPad as `coredevice` and the iPhone as `runner`.

## I3: which codec the device sends (2026-11-06, task 6.2)

On the iPad (iOS 27.0), with idevice's default offer, the device sends **HEVC on payload type 100** (NAL header test of the task plan): idevice is right, go-ios's labels are wrong. With an offer that lists only the H.264 bank, the device accepts and sends **H.264 on payload type 123** at 2736 x 2064 (High profile, level 5.2), and `openh264` decodes it. Decision (D3, architecture 7.2 and 8): offer H.264 only, so one decoder (`openh264`) serves every OS and VideoToolbox is not needed.

## I1: HID on the iPad (2026-11-06, iOS 27.0), as far as it was measured

- Touch: normalized 16-bit touchscreen reports land where aimed. A tap at (895, 540) of the stream's 1032 x 1368 frame reached the testbed as `touch began x 895 y 543` (the real screen is 1376 pt high, so y is 0.6% further down than the frame suggests) and as `tap b3`. Without a display stream HID input does nothing (idevice `hid.rs`); the stream is always started first.
- Buttons: Home (consumer usage page 0x0C, usage 0x40) sent the testbed to the background.
- Keyboard: creating the HID keyboard makes iOS treat a hardware keyboard as attached, so the software keyboard does not appear (the runner's snapshot said "keyboard hidden" with the Plain field focused). Modifiers in the same report as the key are ignored; one report per key, modifiers first, 25 ms apart typed `Ab1!` exactly (10 ms dropped the character after a Shift).
- Paste: Cmd+V raises "Testbed would like to paste from dtpasteboardd" every time; after "Allow Paste" the field received `héllo 😀 世界` intact.
- Tap latency and glass-to-frame latency: see I6 (measured once the stream fix of I6 was in).

## I6: the frozen CoreDevice stream (2026-11-06, iPad iOS 27.0, USB)

Root cause: a receiver backlog, not stale content. Method: spike code with environment switches in `coredevice.rs` (deleted), the testbed's event log as truth, a hash of the app area and screenshots after taps on B3 and B7 (the Taps tab shows "Last tap: bN").

| # | Hypothesis | Experiment | Result |
|---|---|---|---|
| 1 | The depacketizer drops or merges slices | Parsed `ipad_testbed_h264.rtp` independently: NAL types, FU-A, slices, `first_mb_in_slice`, markers per RTP timestamp | Every access unit is one slice (`first_mb=0`), no STAP-A in pictures, markers and timestamps group as our depacketizer does. Ruled out for the 200 packets of the fixture. |
| 2 | Acks need the frame counter in the RTP header extension (profile 0x9311, one word: high 16 bits packet count of the frame, low 16 bits a per-frame counter) | `PU_ACK=ext` and `extcount` against the baseline (acks carry the RTP timestamp, as Apple's capture shows) | No change: same stale app area. |
| 3 | The device waits for acknowledgments | no acks; no RCTL reports; both | No change. |
| 4 | The audio stream or the offer matters | H.264 offer vs idevice's default HEVC offer (packet log only) | HEVC arrives in real time: a picture every 50 ms with RTP timestamps advancing 1200 (24 kHz) per 50 ms. H.264 arrives at the same rate. |
| 5 | The device sends stale content | Compared packet arrival time with the time our decoder finished each picture | Lag grew 2 s per 10 s, 34 s at 40 s: pictures arrive on time and are processed 5 per second. |
| 6 | Where the time goes | Timed each step | decode 11 ms; `Frame::rotated_cw` 115 ms; `FrameStore::publish` 71 ms (unoptimized build): 190 ms per picture against a 50 ms arrival interval. |

Fix (commit "fix(ios): the CoreDevice stream was a backlog..."): `decode_stream` decodes everything and publishes only when the queue is empty; test `a_decoder_that_falls_behind_publishes_only_the_newest_picture...` was red (no `receive` module), then green. Live: taps on B3 and B7 every 2.2 s for 30 s, the screenshot's "Last tap" followed the taps in every shot checked (b3 on even, b7 on odd taps; before the fix it stayed on the first value).

Measured on the iPad after the fix (development build with `CARGO_PROFILE_DEV_PACKAGE_*_OPT_LEVEL=3` for openh264, video and ios, `watch` and `latency` spike tests, 800 ms between taps, 3 warm-ups): 23 taps, 0 timeouts, tap (HID touch down and up) p50 31.3 ms p95 32.3 ms, tap to the first changed frame p50 377.8 ms p95 1102.7 ms. Not met: stability. Runs of 40, 60, 70 and 100 taps at 1.8 s spacing (up to 3 min) had stretches with no changed frame within 3 s (0 of 40, 20 of 60, 16 of 70, 75 of 100). With a per-second counter in the decoder, those stretches showed 3 to 15 pictures decoded per second while the device sent about 20 and the queue growing from 75 to 245 datagrams: the decode thread was starved while other agents' builds kept the load average at 20 to 30. It is not proven that a release build on an idle host keeps up (decode alone is 11 ms); that is the next measurement. If it does not, the options are to drop the queue and wait for a keyframe when it grows past a bound (keyframe requests were ignored in Phase 6, so this needs a test) or to move the decode off the shared pool.

## I5: signing-free tree from the audit service (not run)

Not attempted. The CoreDevice tier returns an empty tree for the session to read with OCR (Phase 7), and the runner provides the tree where it is installed. `axAuditDaemon.remoteAXService` is listed on both devices (I2); walking it remains open.

### I6 follow-up: cheap publish, release-build stability, and two gaps (2026-11-06, iPad iOS 27.0, battery 28% charging)

Change: `Frame::turned_cw()` marks the quarter turn instead of doing it (115 ms in a debug build); the thumbnail for the settle detector is computed straight from the unturned luma plane (test `a_turned_frame_has_the_thumbnail_and_size_of_its_upright_picture`); `encode_png` turns the picture only when a screenshot is taken. `decode_stream` now takes `Pacing`: it shows the newest picture when the queue is empty or every 100 ms during a backlog, and above 400 waiting datagrams only decodes and counts `FrameStore::publishes_skipped` (a skip is the observed failure mode: a starved host, I6 above). Per-stage cost was not re-timed in isolation; the end-to-end release run below is the evidence.

Release build (`CARGO_INCREMENTAL=0 cargo build --release -p phonedriver`), own daemon with `PHONEDRIVER_IOS_TIER=coredevice`, host load average 3 to 15: 135 s of alternating taps on B3 and B7, one per second plus the settle wait: 95 taps, 0 missed changes (`changed` true for each), tap-to-settled-diff p50 430 ms p95 524 ms; the testbed's event log holds exactly 95 `tap` events in that window, alternating b3, b7. No backlog was seen.

Not met, found on the way: (1) the stream is not turned by device orientation. With the testbed in landscape the screenshot comes out sideways (1032x1368, content rotated), and taps must be given in that frame's coordinates (B3 at 661,1188), not the app's. The tier assumes the portrait case measured earlier. (2) The tier has no accessibility tree: `snapshot` is OCR text without refs, so tap-by-text and refs do not work. Because of both, CoreDevice stays opt-in.
