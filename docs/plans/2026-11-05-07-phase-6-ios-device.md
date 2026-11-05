# Phase 6: Physical iOS devices

> **For Claude:** REQUIRED SUB-SKILL: Use `executing-plans` to implement task-by-task.

**Goal:** Drive physical iPhones and iPads: the CoreDevice tier on iOS 27+ with nothing installed on the phone, and the Runner tier on iOS 17 to 26. Milestone **M5**.

**Architecture:** `docs/architecture.md` sections 7, 7.1, 7.2, 7.4, 8. Prior art: `docs/research/prior-art.md` idevice and go-ios rows.

**Tech Stack:** `idevice` (pinned), `openh264`, VideoToolbox via `objc2-video-toolbox` on macOS only if HEVC is required.

## What is reachable

The CoreDevice tier needs an iOS 27 device. At planning time the only paired device is an iPhone 14 Plus on iOS 26.5, and updating it is the user's call. Plan for three outcomes:
- **An iOS 27 device is approved:** do every task.
- **Only iOS 17 to 26 devices are approved:** do 6.1, 6.2 (I2 and I5 only), 6.3, 6.4, 6.6, 6.7 (Runner tier), 6.8. Mark the CoreDevice tier "built, unverified on hardware" or skip 6.5.
- **No device is approved:** do 6.1 and the unit tests in 6.3, then stop the phase and say so in `progress.md`.

Sequencing: 6.1 (pin) comes before everything. Live tests of 6.5 need the iOS testbed installed on the device, which needs signing, so do 6.6's signing setup before 6.5's live tests.

## Stop first: this phase touches people's devices

Before any command that talks to a physical device, ask the user (one question at a time, with a recommendation):
1. Which device may we use? The MacBook currently sees "Mamata's iPhone" (iPhone 14 Plus, iOS 26.5, paired), "Mukesh's iPhone", and an iPad Pro 13-inch (M5). Recommend a device the user owns and uses least.
2. May we enable Developer Mode on it and, for the CoreDevice tier, update it to iOS 27? Updating an OS is a one-way door.
3. Which Apple developer team signs the runner (Runner tier)?

Record the answers verbatim in `progress.md`. Without a yes, do Tasks 6.1 to 6.3 against no device (unit tests only) and stop.

---

### Task 6.1: Pin idevice

Check whether crates.io `idevice` 0.1.68 has the `display_stream`, `core_device`, `xctest`, and `tunnel_tcp_stack` features (`cargo info idevice@0.1.68`). If yes, depend on `idevice = { version = "=0.1.68", default-features = false, features = [...] }`. If not, pin the git rev `8970209` read in research. Record which in `docs/decisions.md` under D3. The README warns of breaking changes in every release: never use a range.

### Task 6.2: Spikes I1 to I3 (device)

`spikes/ios-device/`, deleted after. Use idevice's own example tools first (`tools/src/hid.rs`, `tools/src/coredevice_stream.rs`); write code only for what they do not cover.

- **I1 (iOS 27 device):** mount the DDI, start the display stream, tap through HID at a known point, confirm by screenshot that it landed. Measure tap latency (send to `sendReport` return) and glass-to-frame latency (tap a toggle, time to a changed frame). Confirm that without the stream, taps do nothing (idevice `hid.rs:1-11`). Create the HID keyboard and check whether the software keyboard disappears from a focused field (screenshot before and after). Set the pasteboard, focus a Notes field, send Cmd+V, and check for an "Allow Paste" prompt. Both results decide how architecture 7.2's text path behaves; update it.
- **I2 (any iOS 17+ device):** pair, tunnel, RSD list. Record which services exist on iOS 26.5 vs 27 (expect no `universalhidservice` below 27, per go-ios `hid.go:70-72`).
- **I3 (iOS 27 device): which codec does the device actually send?** go-ios and idevice label payload types 100 and 123 with opposite codecs (architecture 7.2). Start a stream with idevice's offer, capture 200 RTP packets, and classify by NAL header: the first payload byte's low 5 bits in 1..23, 24 or 28 means H.264; `(byte0 >> 1) & 0x3f` of 32 to 34 (VPS/SPS/PPS) or 49 (fragmentation unit) means HEVC. Record the answer and fix whichever reference doc is wrong in `docs/research/prior-art.md`. If it is HEVC, also try offering only the H.264 bank and record whether the device accepts.
- **I5 (any iOS 17+ device): a tree without signing.** go-ios `ios/accessibility/` drives the Accessibility Inspector audit service: it moves focus element by element and reads each element's label, traits, and frame. Measure the time to walk every element on Settings and on the testbed Taps tab. If a full walk is under 1.5 s, it becomes the CoreDevice tier's tree source when the runner is absent (better than OCR: roles and icon labels). Otherwise record the numbers and keep OCR.

Record everything in `docs/spikes/ios.md`.

### Task 6.3: Transport and discovery

**Files:** Create `crates/ios/src/transport.rs`, `crates/ios/src/discovery.rs`.

**Unit tests:** map idevice errors to `core::Error`: not paired → `NeedsUser(TrustComputer)`; Developer Mode off → `NeedsUser(EnableDeveloperMode)`; DDI missing → `NeedsUser(MountDeveloperImage)` with the `phone-use ios setup` command; lost tunnel → `DeviceGone` after one reconnect attempt.

**Live tests** (`#[ignore]`, approved device only): discovery lists the device with tier chosen by OS version and available services; the daemon re-establishes the tunnel after unplug and replug within 5 s.

### Task 6.4: `phone-use ios setup`

Pairs (prompts the user to tap Trust), reveals the Developer Mode toggle (`amfi`), and mounts the DDI. On macOS, uses Xcode's DDI files; elsewhere, prints the source it would download from and asks for confirmation (one-way door). Idempotent: a second run prints `ok` for each step.

### Task 6.4b: RTP depacketizer

**Files:** Create `crates/video/src/rtp.rs`; fixture: the 200 packets captured in I3, stored as a length-prefixed file under `crates/video/tests/fixtures/`.

**Tests:** depacketizing the fixture yields access units that `openh264` (H.264) or VideoToolbox (HEVC) decodes without error into frames of the stream's size; a packet-loss test (drop every 50th packet) yields a `KeyframeNeeded` signal rather than corrupt frames, and the CoreDevice tier answers it with an RTCP keyframe request. Implement RFC 6184 (single NAL, STAP-A, FU-A) for H.264 or reuse idevice's `hevc.rs` for HEVC, whichever I3 found. Do not build the other one.

### Task 6.5: CoreDevice tier

**Files:** Create `crates/ios/src/coredevice.rs`.

Implements `Device` with: display stream started on lease and kept alive; `frames()` from the video crate; `gesture` via multitouch reports (points → 0..65535, up to 5 fingers, reports paced on a dedicated task with `tokio::time::interval` at 120 Hz); `button`; `text` via pasteboard `set_text` + Cmd+V on the HID keyboard (`Paste`), HID keyboard usages (`Keystrokes`); `apps`/`launch` via `app_service`; `screenshot` from the FrameStore after an RTCP keyframe request; `tree` from the runner when installed, else from the audit service if spike I5 passed, else `Unsupported` (Phase 7 adds OCR).

**Live tests:** the iOS testbed, built for the device and signed with the user's team (same as the runner), installed through `installation_proxy`; Phase 5 task 5.5's tests rerun on the device in this tier, minus tree-dependent ones when the runner is absent.

### Task 6.6: Runner tier on device

Build the runner for the device with the user's team (`xcodebuild build-for-testing -allowProvisioningUpdates DEVELOPMENT_TEAM=<team>`). Install and launch through idevice `xctest` so the same path works without Xcode at runtime. Connect to device port 27753 through the idevice provider (as `services/wda.rs` does). `phone-use devices` shows the runner's profile expiry date.

**Live tests:** Phase 5's iOS live tests pass on the iOS 26.5 device (Runner tier) and, with the runner installed, on the iOS 27 device (CoreDevice tier with tree).

### Task 6.7: Benchmark (M5)

Run `phone-use-bench` on each approved device and tier. **Gate M5:** budgets from architecture section 12 (iOS columns) met or, where missed, profiled with the cause recorded in `docs/bench/m5.md`. Run the iOS scenario suite on each device: 12/12.

### Task 6.8: doctor for devices

Add checks: paired, Developer Mode, DDI mounted, services present, runner installed and its expiry, stream starts, a test tap is delivered.

---

## Phase gate

M5 met on every approved device. `progress.md` records the device, tier, and user approval for each.
