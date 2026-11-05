# Prior art: where each mechanism comes from

Read the source, not summaries. Each entry gives the repo, the commit read on 2026-11-05, the files that matter, and what we take. Clone shallow into a scratch directory outside this repo; do not vendor whole projects.

## scrcpy (Apache-2.0)

https://github.com/Genymobile/scrcpy at `a60891a` (2026-10-08).

The reference for running code on Android as the shell user with no app installed.

| What | File (under `server/src/main/java/com/genymobile/scrcpy/`) | What we take |
|---|---|---|
| Launch | `app/src/server.c:222` builds `CLASSPATH=<jar> app_process / <Main> <version> <args>` | Same launch shape for our agent |
| Gradle-free build | `server/build_without_gradle.sh` | `aidl` + `javac` against `android.jar` + `d8` into a jar |
| Hidden-API access | `wrappers/ServiceManager.java`, `wrappers/*.java`, `FakeContext.java`, `Workarounds.java` | Reflection wrappers for InputManager, DisplayManager, ClipboardManager, ActivityManager |
| Input injection | `wrappers/InputManager.java:41-79`, `device/Device.java:62-78`, `control/PointersState.java` | `injectInputEvent(event, mode)` with `setDisplayId`; multi-pointer MotionEvents |
| Screen capture | `video/ScreenCapture.java:73,134`, `wrappers/DisplayManager.java:164-181`, `wrappers/SurfaceControl.java:81` | Mirror the display into a MediaCodec input Surface |
| Encoder | `video/SurfaceEncoder.java:30-36,79-95,201,318-332` | MediaCodec setup: repeat-previous-frame 100 ms, priority 0, latency 1, size alignment and `MAX_SIZE_FALLBACK` downsizing, restart on size change |
| Rotation and resize | `display/DisplayMonitor.java`, `display/DisplayResizeDebouncer.java` | Restart capture on display change |
| Clipboard | `wrappers/ClipboardManager.java:27,36` | Get and set primary clip as shell |
| App list and launch | `model/DeviceApp.java`, `device/Device.java:~300` | Launcher apps with labels; start by package |
| Uniqueness | `server.c` socket name `scrcpy_<scid>` | One abstract socket per agent instance |

Licensing: copying wrapper code is allowed under Apache-2.0 with attribution. Keep a `NOTICE` file listing every copied file and its origin.

## AOSP uiautomator command

`frameworks/base/cmds/uiautomator` in AOSP. `UiAutomationShellWrapper` builds `new UiAutomation(looper, new UiAutomationConnection())` and calls `connect()` from an `app_process` started by the shell. This is the evidence that a shell-launched process can own a UiAutomation connection. The constructor signature changes across API levels; Phase 1 spike A1 confirms it on API 26 and API 37 before anything depends on it.

Relevant public APIs:
- `UiAutomation.getWindows()` with `AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS` gives every window, including system dialogs and the IME.
- `UiAutomation.getRootInActiveWindow(int prefetchingStrategy)` (API 33) with `AccessibilityNodeInfo.FLAG_PREFETCH_DESCENDANTS_HYBRID` fetches a subtree in one binder round trip. Below API 33, nodes arrive with the default prefetch and child fetches cost extra IPC.
- `UiAutomation.setOnAccessibilityEventListener` streams change events. No idle wait anywhere.
- Only one UiAutomation connection can exist per device. A running Appium or uiautomator2 server holds it.

## WebDriverAgent (BSD-3)

https://github.com/appium/WebDriverAgent at `3d6a4e0` (2026-10-17).

The reference for XCTest private APIs on iOS.

| What | File | What we take |
|---|---|---|
| Event synthesis | `WebDriverAgentLib/Utilities/FBW3CActionsSynthesizer.m`, `FBBaseActionsSynthesizer.m` | `XCSynthesizedEventRecord` + `XCPointerEventPath` for taps, drags, multi-touch |
| Private headers | `PrivateHeaders/XCTest/XCSynthesizedEventRecord.h`, `XCPointerEventPath.h`, `XCAXClient_iOS.h`, `XCUIScreen.h`, `XCUIDevice.h` | Copy only the headers we call |
| Fast snapshots | `WebDriverAgentLib/Utilities/FBConfiguration.h:183-188` (`snapshotMaxDepth` overrides `XCAXClient_iOS` `defaultParams` `maxDepth`, which is `INT_MAX`) | Bounded depth and a reduced attribute set |
| Active app | search `fb_activeApplication` | Snapshot the foreground app without knowing its bundle id |
| Screenshots | `WebDriverAgentLib/Utilities/FBScreenshot.m` | JPEG at chosen quality from `XCUIScreen` |
| Typing | `WebDriverAgentLib/Categories/XCUIElement+FBTyping.m` | Unicode typing fallback |

WDA's HTTP server, XPath engine, and W3C session model are not taken. Our runner is a small XCTest bundle with our own framing (decision D4).

## idevice (MIT)

https://github.com/jkcoxson/idevice at `8970209` (2026-10-27); crates.io `idevice` 0.1.68. The README warns that every point release breaks the API. Pin an exact version or git rev.

Pure-Rust iOS device protocols: usbmuxd, lockdown, pairing, the iOS 17+ CoreDevice tunnel with a userspace TCP stack, RSD, RemoteXPC, DVT, XCTest launch.

| Need | Module (under `idevice/src/`) | Notes |
|---|---|---|
| Device discovery | `usbmuxd/` | USB and network devices |
| Tunnel | `tunnel.rs`, `tcp/`, `services/core_device_proxy.rs`, `services/rsd.rs` | Userspace, so no root and no TUN device on Linux or Windows |
| Touch, buttons, keyboard | `services/core_device/hid.rs` | Served by `dtuhidd` from the Developer Disk Image. Touch coordinates are normalized 0 to 65535 (`tools/src/hid.rs:37`). Up to 5 contacts per report (`TOUCHSCREEN_CONTACT_COUNT_MAXIMUM`). **Events are silently dropped unless a display media stream is running** (`hid.rs:1-11`) |
| Screen stream | `services/core_device/display_stream/` | `com.apple.coredevice.displayservice`; plaintext RTP; `rtcp.rs` for keyframe requests. Its `negotiation.rs:166-186` labels payload type 123 as H.264 and 100 as HEVC, and `hevc.rs` assumes HEVC; go-ios says the opposite (below). Spike I3 decides |
| Screenshot | `services/core_device/screencaptureservices.rs` | PNG, iOS 27+ |
| Apps | `services/core_device/app_service.rs` | `list_apps`, `launch_application`, `list_processes` |
| Pasteboard | `services/core_device/pasteboard_service.rs` | `set_text`, change notifications |
| Orientation | `services/core_device/orientation_service.rs` | |
| DDI mount | `services/mobile_image_mounter.rs`, `tss.rs` | Personalized DDI needs Apple's TSS signing server |
| XCTest launch | feature `xctest`, `services/dvt/xctest` | Starts a test runner without `xcodebuild` |
| Older screenshots | `services/dvt/screenshot.rs` | iOS 17 to 26 |
| Example tools | `tools/src/hid.rs`, `tools/src/coredevice_stream.rs` | Run these first in spikes |

## go-ios (MIT)

https://github.com/danielpaulus/go-ios at `6910f3b` (2026-11-03). A second implementation of the same services. Use it to cross-check behavior, not as a dependency.

- `ios/hid/hid.go:44-72`: universal HID service is iOS 27+; an iOS 18 device answers CoreDeviceError 9021; below 27 the service is absent from RSD; the DDI must be mounted.
- `ios/display/offer.go:35-38,152-165`: the media offer advertises an HEVC bank (payload type 123) and an AVC bank (100); the comment says the device picks and "on iOS 27 it chooses AVC". This contradicts idevice's labels. If go-ios is right, one H.264 decoder serves both platforms (spike I3).
- `ios/screencapture/screencapture.go:1-12`: PNG only; replies need the RemoteXPC host announcement.
- `ios/accessibility/`: the Accessibility Inspector audit service. It can move focus and read the focused element without any app on the phone. Possibly slow for a whole screen; a candidate tree source without signing (spike I5 in Phase 6).

## Android emulator

The emulator works with the same adb path as a phone. Its gRPC controller is not needed.

## Agent ergonomics references

- Vercel `agent-browser`: snapshot with `@eN` refs, then act on refs. Our observation format follows this pattern.
- Playwright ARIA snapshots: indented role + name lines. Our text rendering is in the same family.

## Benchmarks we can borrow

- AndroidWorld (Google Research, https://github.com/google-research/android_world): 116 tasks on an emulator with programmatic success checks. Used in Phase 8 to compare harnesses with the model held fixed.
- phone-harness itself: the latency baseline in Phase 2.
