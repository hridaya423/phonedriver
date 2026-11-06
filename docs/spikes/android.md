# Android spikes A0 to A5 (Phase 1)

Run on 2026-11-05. Everything here was measured on two emulators on win-pc, reached from the MacBook over `ssh -f -N -L 5038:127.0.0.1:5037 win-pc` and `ADB_SERVER_SOCKET=tcp:127.0.0.1:5038`:

| Serial | AVD | Android | Display | Notes |
|---|---|---|---|---|
| `emulator-5554` | `Pixel_API_37` | API 37 (`sdk_gphone64_x86_64`) | 1080x2424, 420 dpi (2.625) | headless, software GL |
| `emulator-5556` | `Pixel_API_26` | API 26 (`Android SDK built for x86_64`, Google APIs) | 1080x1920, 420 dpi | windowed, software GL |

The spike code (`spikes/android-agent`: Java agent plus a test APK; `spikes/android-host`: Rust host with `adb.rs`, `agent.rs`, `video.rs`, one file per spike) lived in the commit `spike(android): agent, host, test app for spikes A0 to A5` and is deleted in the commit `docs: android spike results`. Numbers are p50/p95 of the stated run count. Most latencies include the SSH tunnel: a bare `host:version` round trip on a new connection is p50 38 to 40 ms (p95 138 to 151 ms), and an open `localabstract:` stream pings at p50 13 ms (p95 100 to 106 ms). Budgets in architecture section 12 are judged co-located on win-pc, which this phase did not do (see "Not measured").

## Setup facts and surprises

- API 26 system image installed with `sdkmanager "system-images;android-26;google_apis;x86_64"`; `avdmanager create avd -n Pixel_API_26 ...` printed "Could not load devices" errors for `devices.xml` but created the AVD. Boot took 112 s.
- The API 37 emulator hung at "VkEmulation deviceInfo" (log: `detected a hanging thread 'QEMU2 CPU0 thread'`, no RAM allocated, an `adb shell` to the old instance never returned) when started windowed from an SSH session on win-pc, with the default `lavapipe` Vulkan, and again with `-gpu swiftshader_indirect`. It booted in about 60 s with `emulator.exe -avd Pixel_API_37 -port 5554 -no-snapshot -no-audio -no-boot-anim -no-window -gpu swangle_indirect`, launched as `(nohup ./emulator.exe ... &)` from `ssh win-pc`. Use that for the API 37 AVD. Later (M2, `docs/bench/m2.md`) this turned out to be the root cause of the API 37 hangs: any launch without `-gpu swangle_indirect` runs lavapipe, which crashes the emulator within 2 to 10 s of the first WebView; the AVD's `hw.gpu.mode` is now `swangle_indirect` (the earlier `swiftshader_indirect` value is rejected at launch and auto picks lavapipe).
- Chrome on API 37 needs two taps through first-run screens ("Stay signed out", "No thanks") before a page can take input.
- Scrcpy files copied (Apache-2.0, `a60891a`) into the spike, for the future NOTICE: `server/src/main/aidl/android/content/IOnPrimaryClipChangedListener.aidl`, `.../android/view/IDisplayWindowListener.aidl`, `server/src/main/java/android/content/IContentProvider.java`, and under `server/src/main/java/com/genymobile/scrcpy/`: `AndroidVersions.java`, `FakeContext.java`, `Workarounds.java`, `audio/AudioCaptureException.java`, `display/DisplayInfo.java`, `model/{Orientation,Point,Size}.java`, `util/{AffineMatrix,BinarySearch,Command,IO,Ln,Settings,SettingsException,StringUtils,Threads}.java`, `video/VideoConstraints.java`, `wrappers/{ActivityManager,ClipboardManager,ContentProvider,DisplayControl,DisplayManager,DisplayWindowListener,InputManager,PowerManager,ServiceManager,StatusBarManager,SurfaceControl,WindowManager}.java`. `wrappers/ClipboardManager.java` was modified (added `getClip`/`setClip`). Production code copies only what it uses.
- The video encode loop, input gesture player, and control server were written new from the scrcpy patterns (`SurfaceEncoder.java:318-332`, `ScreenCapture.java`, `PointersState`), not copied.

## A0: Rust adb client over the tunnel

**Built.** `adb.rs`: 4-hex length framing with `OKAY`/`FAIL`, `host:version`, `host:track-devices-l`, `host:transport:<serial>` then `shell,v2,raw:<cmd>` (packet ids 0 stdin, 1 stdout, 2 stderr, 3 exit, 4 close stdin), `sync:` `SEND`/`DATA`/`DONE`, `localabstract:<name>`. About 150 lines. The echo server for the `localabstract:` test is `dev.phoneuse.spike.Main echo <name>` (a `LocalServerSocket`).

**Commands.** `cargo build --release && ./target/release/android-host-spike a0 emulator-5554` (and `emulator-5556`).

| Result | API 37 | API 26 |
|---|---|---|
| `host:version` | `0029` | `0029` |
| `host:track-devices-l` | lists both emulators | same |
| `shell,v2,raw:echo hi`, new connection each, 20 runs | p50 64.1 ms, p95 198.6 ms | p50 52.4 ms, p95 159.2 ms |
| `adb shell echo hi` CLI, 20 runs | p50 247.9 ms, p95 377.9 ms | p50 246.9 ms, p95 438.1 ms |
| Push 1 MB (`sync:`, mode 0444), 5 runs incl. a verifying `ls` | p50 3.56 MB/s, p95 4.05 | p50 3.96 MB/s, p95 5.48 |
| `localabstract:` through the tunneled server | works; 50 pings p50 12.7 ms, p95 100.3 ms; 1 MB echo 182 ms | works; p50 13.4 ms, p95 105.6 ms; 1 MB echo 345 ms |
| shell v2: host writes the child's stdin without closing it | yes: `cat` echoed `token-abc\n` before stdin closed; close-stdin (id 4) then exit code 0 | same |

**Decision.** D11 confirmed (see decisions.md). Our client is about 5x faster per call than the CLI and, more important, keeps streams open. The agent token can be written to stdin after launch.

## A1: UiAutomation from app_process

**Built.** `dev.phoneuse.spike.Main` (`tree`, `bench N mode`, `events secs arch|all`, `recycle`) and `Ui.java`: Looper thread, reflective UiAutomation construction trying every declared constructor, `connect(int)` by reflection, service info with `FLAG_RETRIEVE_INTERACTIVE_WINDOWS | FLAG_INCLUDE_NOT_IMPORTANT_VIEWS | FLAG_REPORT_VIEW_IDS`, `getWindows()` walk to RawNode JSON (protocol section 2 format). `build.sh` follows `build_without_gradle.sh` (`aidl`, `javac -bootclasspath android.jar -source 1.8`, `d8 --min-api 26`). Run: `adb push spike.jar /data/local/tmp/` then `CLASSPATH=/data/local/tmp/spike.jar app_process / dev.phoneuse.spike.Main tree`.

**Constructor.** API 26 declares `UiAutomation(Looper, IUiAutomationConnection)` (public). API 37 declares `(int displayId, Looper, conn)` (private), `(Context, conn)` (public) and `(Looper, conn)` (public). All three worked on API 37 (`UiAutomation: Created with deprecated constructor, assumes DEFAULT_DISPLAY` for the last in logcat); `(Looper, conn)` is the one common signature and is what production uses. Connect: 12 to 14 ms (API 26), 24 to 43 ms (API 37). On API 37 the first `getWindows()` right after connect was empty once (retry after 20 ms fixed it).

**Flag proof (API 37, TalkBack enabled with `settings put secure enabled_accessibility_services com.google.android.marvin.talkback/...TalkBackService`).** `dumpsys accessibility` before: `Bound services:{Service[label=TalkBack, ...]}`. While our agent held UiAutomation with `FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES` (flags=1): TalkBack still bound. With flags=0: `Bound services:{}` (unbound) and bound again after exit. TalkBack's process stayed alive in both cases. Proves the flag is needed. (API 26's google_apis image has no TalkBack, so this was tested on API 37 only.)

**getSourceNodeId.** The hidden `AccessibilityNodeInfo.getSourceNodeId()` is reachable by reflection on both APIs and returns the packed accessibility node id (for example `-4294967296` for a window root, `-4294967273` for a list row). Architecture 4.3 step 1 stays.

**Recycling (the case 4.3 guards).** Test APK `ListView` of 1,000 rows, `ACTION_SCROLL_FORWARD` four times, tree after each:
- API 37, scroll 1: 3 ids kept their text, 14 ids got new text (`-4294967273: "Row 13" -> "Row 29"`, `-4294967274: "Row 12" -> "Row 27"`, ...). Scrolls 2 to 4: 15, 15, 15 ids with new text.
- API 26: 11, 11, 10, 11 ids with new text (`-4294967284: "Row 10" -> "Row 22"`).
Recycled rows keep their source node id and change text, on both APIs. The label-or-bounds continuity condition in 4.3 is required. (Real Settings lists have only 30 rows on the emulator and did not recycle in the same test.)

**Tree time, cold (cache cleared before each run), 20 runs, p50 / p95 ms, binder round trips in brackets.** "default" = `window.getRoot()` then `getChild(i)`; "hybrid"/"df"/"bf" = `getRoot(FLAG_PREFETCH_DESCENDANTS_*)` (API 33+); "none" = `getRoot(0)`; "df+child" = depth-first on every `getChild(i, flag)`.

| Screen | Nodes (JSON) | default | hybrid | df | none | df+child |
|---|---|---|---|---|---|---|
| API 37 Settings main (2 windows: app 155 + systemui 37) | 192 (27.1 KB) | 130 / 163 [69] | 95 / 120 [98] | 103 / 124 [94] | 109 / 129 [99] | **75 / 82 [52]** |
| API 37 Settings > Apps > All apps | 115 (13.6 KB) | 70 / 87 [45] | 62 / 74 [47] | 49 / 59 [40] | 50 / 64 [45] | **43 / 59 [34]** |
| API 26 Settings main | 59 (8.2 KB) | 24.6 / 31.8 [8] | n/a | n/a | n/a | n/a |
| API 26 Settings > Apps | 89 (11.5 KB) | 27.6 / 31.7 [12] | n/a | n/a | n/a | n/a |

An earlier run on a quieter machine (API 37 Settings main, same 192 nodes) gave default 97 / 114, hybrid 109 / 126, df 94 / 119, none 120 / 152, df+child 69 / 93; the two emulators and the SSH session share win-pc's CPU, so absolute numbers move by about 30%, the ordering did not. Round trips are counted with a `Proxy` around `IAccessibilityServiceConnection` (calls whose name starts with `findAccessibilityNodeInfo`). Hybrid prefetch of the window root was no better than the default: about one round trip per 2 to 4 nodes whatever the strategy (`MAX_NUMBER_OF_PREFETCHED_NODES` is 50 per request, but the batches that come back are far smaller). The only clear win is requesting depth-first on each `getChild`. The 80 ms budget (p50, Settings main) holds only with df+child and without the stale system windows; Phase 2 must measure dropping the status and navigation bars.

**Events.** With only `TYPE_WINDOW_STATE_CHANGED | TYPE_WINDOWS_CHANGED` (11 s runs with a swipe at 3 s, a tap on a row at 6 s, back at 8 s): the swipe produced no event on API 26 or 37; the tap and back produced 5 `WINDOW_STATE_CHANGED` and 14 `WINDOWS_CHANGED` (API 37) in about 2.5 s. With all types: 1 `VIEW_CLICKED`, 3 `VIEW_SCROLLED`, 12 `WINDOW_CONTENT_CHANGED` and 21 `WINDOWS_CHANGED` (API 37). Scrolls are invisible without content events, so frames are the only scroll signal. The window events come in storms.

**Stale cache.** After the spike subscribed only to window events, a tree read after a tap returned the old text of a `TextView` whose text had changed (A2: touch counter stuck at "touch log"). `clearCache()` before each tree fixed it. `clearCache()` is public on `UiAutomation` from API 33; below that, `AccessibilityInteractionClient.getInstance().clearCache()` by reflection worked on API 26.

**Never-idle screen.** Test APK indeterminate `ProgressBar` (48 dp): our tree took 21 ms (API 37, 11 nodes) and 87 ms (API 26, 45 nodes). `uiautomator dump` also succeeded on both (it times out its idle wait instead of failing) but took p50 about 2.6 s (API 37) and about 2.0 s (API 26), 3 runs each. A video in Chrome was not tried; the spinner stands in. This is a weaker result than the plan expected: `uiautomator dump` does not fail here, it is only about 40x slower.

**Conflict with `uiautomator dump` (exact text).** While our agent held UiAutomation, `adb shell uiautomator dump` exited 137 ("Killed") with nothing on stdout; logcat has `java.lang.IllegalStateException: UiAutomationService android.accessibilityservice.IAccessibilityServiceClient$Stub$Proxy@83e4a16already registered!` thrown from `registerUiTestAutomationService` (API 26 and 37). Our own second client got the same exception from `UiAutomation.connect`. `AutomationConflict` should match `already registered` on connect.

**Fixtures** (API 37, 420 dpi, committed with screenshots): `crates/core/tests/fixtures/android/real_settings_main.json` (155 nodes), `real_settings_apps.json` (115), `real_dialog.json` (16, an `AlertDialog` from the test APK, "Delete conversation?"), each with a `.png`. A core test parses, prunes and renders each (`render::tests::real_*`, snapshots in `crates/core/src/snapshots/`).

I read all three rendered snapshots. Label merging was right (`@e4 button "Network & internet" "Mobile, Wi‑Fi, hotspot"`, `@e7 button "Android Auto" "291 kB"`, dialog buttons `button "CANCEL"`). Role mapping was wrong: the real Settings is Jetpack Compose, so its lazy lists are `android.view.View` with `CollectionInfo` and scroll actions and rendered as `group scrollable↓` instead of `list`. Fixed in `core::raw` (a node of role `Other` with collection info becomes `List`), test first (`compose_lazy_list_with_collection_info_is_a_list`, failing with `left: Other right: List`). Remaining finding, not fixed (a design choice for Phase 2): windows arrive topmost first and children in view order, so the status-bar nodes print before the app's and, on the Apps screen, the toolbar (`Navigate up`, `All apps`, `Search`) prints after the list.

**Decision.** D2 confirmed on API 26 and 37: no instrumentation fallback needed. Changed: `clearCache()` per tree, depth-first `getChild`, conflict detection text, events are storms. All recorded in architecture 6.2.

## A2: Input injection

**Built.** `Input.java` (`gesture` op: timed paths per pointer, DOWN / POINTER_DOWN / MOVE / POINTER_UP / UP with pointer index bits, last event `WAIT_FOR_FINISH`, scheduling on `SystemClock.uptimeMillis`) and a control server (`dev.phoneuse.agent.Main`, `C` stream, token, length-prefixed JSON). The test APK's `touch` mode counts every MotionEvent action and runs a `ScaleGestureDetector`; the host reads that counter back through the tree, which is the ground truth (not the agent's reply).

**Commands.** `android-host-spike a2 <serial>` and `a2maps <serial> com.google.android.apps.maps <prefix>`.

| Measure (50 taps unless stated) | API 37 | API 26 |
|---|---|---|
| Agent `gesture` tap, host write to reply | p50 22.1 ms, p95 112.6 ms | p50 32.1 ms, p95 130.4 ms |
| Same taps, time inside the agent (DOWN+UP, `WAIT_FOR_FINISH`) | p50 4.1 ms, p95 13.3 ms | p50 7.5 ms, p95 15.0 ms |
| `adb shell input tap` through our client (new shell,v2 per tap) | p50 83.6 ms, p95 185.5 ms | p50 314.9 ms, p95 465.3 ms |
| `adb shell input tap` CLI, 20 taps | p50 285.9 ms, p95 356.0 ms | p50 510.2 ms, p95 541.7 ms |
| Test app counter after 50 agent taps | `down=50 up=50` | `down=50 up=50` |
| ... after 50 more `input tap` | `down=100 up=100` | `down=100 up=100` |
| Swipe (down, 10 moves, hold 200 ms, up), 1 run | reply ok in 446 ms; counter `move=11` | ok in 541 ms; `move=11` |
| Pinch (two pointers, 11 steps over 330 ms) | `maxptr=2 pdown=1 pup=1 move=10 scale=1.42` | `maxptr=2 pdown=1 pup=1 move=10 scale=1.45` |

Pinch on Google Maps (API 37): `a2_maps37_after.png` shows the map zoomed from all of North America (`maps_skip.png`, the same screen before) to the central US with city labels (Omaha, Kansas City, Dallas). I looked at both.

The agent-side time (4 to 8 ms) is what a co-located tap request would cost beyond the loopback round trip; the end-to-end figures include the tunnel's 13 ms floor. Whether p50 15 ms holds co-located was not measured (see "Not measured").

**Decision.** D2's input path confirmed; an agent tap is p50 22 ms (API 37) and 32 ms (API 26) end to end through the tunnel against 84 and 315 ms for `input tap` through the same adb client and 286 and 510 ms through the CLI, and the pinch works.

## A3: Video stream and host decode

**Built.** `Video.java`: `DisplayManager.createVirtualDisplay` (scrcpy wrapper) into a MediaCodec H.264 input Surface with `KEY_REPEAT_PREVIOUS_FRAME_AFTER` 100 ms, `KEY_PRIORITY` 0, `KEY_LATENCY` 1, 30 fps cap (`max-fps-to-encoder`), 8 Mbps, I-frame interval 10 s, size aligned to the codec's alignment (2), `MAX_SIZE_FALLBACK` retry (not triggered), restart when `DisplayInfo` changes (polled every 100 ms). Framing per protocol section 2. Host `video.rs`: reads the `V` stream, decodes with `openh264` 0.9 on a thread, keeps the latest YUV frame, computes a luma thumbnail (10 px blocks = 4 pt at 2.625) and per-frame `changed_area_pt2` (samples changed by more than 12 levels, times 16), writes PNG on demand. Release build, no `nasm` installed (the crate built its C path). Commands: `android-host-spike a3 <serial> [first|fps|areas|latency|shots|secure|sync|recover|fixture|all]`.

| Measure | API 37 | API 26 |
|---|---|---|
| Encoder / profile from the SPS | `c2.android.avc.encoder`, profile_idc 66 (Baseline), constraint flags 0xC0 (Constrained Baseline), level 5.0 | same encoder, profile_idc 66, 0xC0, level 4.1 |
| `openh264` decode errors | 0 (all runs) | 0 |
| Stream start (connect) to codec header, 10 cold starts | p50 227 ms, p95 320 ms | p50 138 ms, p95 320 ms |
| Stream start to first decoded frame, static Settings, 10 cold starts | **p50 433 ms, p95 631 ms** (target 200 ms: missed) | p50 219 ms, p95 504 ms |
| Frame rate, static Settings, 5 s | 2.8 fps; 14 of 14 frames zero changed area | 3.0 fps; 14 of 15 zero (one 1,344 pt² frame right after the app launch) |
| Frame rate, scrolling Settings, 8 swipes | 17.0 fps (35 frames in 2.1 s), max 65,664 pt² | 12.5 fps (27 frames in 2.2 s), max 59,072 pt² |
| Decode time per frame on the Mac, p50 / p95 | 3.64 / 9.55 ms (35 frames, 1080x2424) | 4.60 / 7.30 ms (27 frames, 1080x1920) |
| Packet size scrolling, p50 / p95 | 30,193 / 116,212 B | 27,472 / 86,834 B |
| Latest frame to PNG on the Mac (convert + encode, 10 runs, full size, unoptimized converter) | convert 7.2 / 10.3 ms, PNG 52 / 53 ms | convert 5.7 / 6.1, PNG 34 / 34 ms |
| `screencap -p` over our adb client, 10 runs | p50 329 ms, p95 382 ms (215 KB) | p50 285 ms, p95 325 ms (137 KB) |
| Sync-frame request to keyframe (10 requests) | static screen: 2 of 10 produced one (168 ms); while the screen changed: 10 of 10, p50 162, p95 251 ms | static: 3 of 10 (p50 559, p95 2,932 ms) |
| Glass-to-host: tap toggling a switch to first frame changed by more than 128 pt² (20 taps) | all 20; p50 553 ms, p95 912 ms; to first changed frame (> 0) p50 352 ms, p95 912 ms | all 20; p50 386 ms, p95 460 ms; first changed p50 223 ms, p95 338 ms |
| Recovery: `user_rotation 1` | first frame at 2424x1080 after 785 ms | 658 ms |
| Rotation back | 758 ms | 536 ms |
| `wm size 720x1600` then `wm size reset` | 909 ms, 773 ms | 580 ms, 551 ms |
| Screen off (`KEYCODE_SLEEP`) | frames kept flowing (35 in 2.5 s, zero change); first new frame 512 ms after `KEYCODE_WAKEUP` | 28 frames; 516 ms |
| `FLAG_SECURE` activity | region of the text node black in the frame (luma mean 16.00, max 16 over 115,584 samples) while the tree listed `SECRET code 123456` | same (mean 16.00, max 16) |

Every recovery above produced a codec-header message with the new size (headers seen on API 37: 1080x2424, 2424x1080, 1080x2424, 720x1600; the stream never closed). Latency figures include the tunnel (about 13 ms each way) and the software encoder.

**Changed area, per case, 3 s windows started after a 1.8 s settle (API 37 / API 26), max single frame and nonzero frame counts.** Blinking caret (focused `EditText`): max 96 pt², 6 changed frames of 31 / 96, 6 of 32. 48 dp indeterminate spinner (126 px): max 480 pt², 45 of 45 frames changed / max 272, 51 of 51. Switch toggle (tap plus animation, 1.5 s): max 384 pt², 9 of 30 frames / max 432, 7 of 19. Screen transition Settings to All apps: max 78,640 pt², 18 of 47 frames / max 253,952, 13 of 29.

An earlier run that started the window only 0.7 s after launching the activity measured 35,904 pt² for the "caret": that was the launch animation, not the caret. A transition-free window is essential for this measurement.

**Result for `significant_area`.** The architecture's 4,096 pt² default (a 64x64 pt region) cleanly separates the screen transition (13,000 to 254,000 pt²) from the caret (96), spinner (480) and toggle (432). But the same threshold also makes a toggle invisible: zero of 20 toggle taps produced a frame above 4,096, so phase 1 of the settle detector ("wait for the first change") would report `changed: false` for a tap that worked. Changed: two thresholds, `first_change_area` 128 pt² and `significant_area` 4,096 pt² (architecture 4.6; `core::settle` updated test-first: `a_toggle_sized_change_counts_as_the_first_change` failed with `left: Some((true, false)) right: None`).

**Screenshots (looked at).** `frame_api37_0.png` (from the video, converted BT.601 limited range) against `screencap_api37.png`: same Settings main screen, text sharp and legible in both, the video frame's background a touch darker, no visible blocking. The FLAG_SECURE frame `secure_frame_api37.png` is entirely black except the status bar clock and icons and the gesture bar, which are drawn by other layers; the tree at the same moment lists the secret text.

**Fixture.** `crates/video/tests/fixtures/settings_scroll.h264`: 3 s of Settings scrolling on API 37 (alternating swipes), recorded as the agent's wire stream exactly as received (12-byte codec header `h264 | 1080 | 2424`, then `u64 pts | u8 flags | u32 len | Annex-B` per packet): 1,616,124 bytes, 60 packets (1 config, 59 frames, 1 keyframe). `android-host-spike fixture <file>` decodes it with `openh264`: 59 decoded frames, 0 errors, every frame 1080x2424. Task 2.4 reads that format.

**Decision.** D10 holds, with costs: a cold stream start is 219 to 433 ms (paid once per lease), the static frame rate is about 3 fps (stall means no frame for 1 s), a settled screen's sharpness comes from the repeat frames because `sync_frame` is honored only when a new frame renders, and the settle detector needs the two thresholds. `KEY_PROFILE` stays unset on emulators. Hardware encoders were not measured.

## A4: Unicode text by paste

**Built.** Agent ops `clipboard_set` / `clipboard_get` / `clipboard_set_uri`, and `text` with modes `paste` (save current `ClipData`, set ours, `KEYCODE_PASTE` with `WAIT_FOR_FINISH`, restore after `restore_ms`), `set` (`ACTION_SET_TEXT` on the focused input node) and `keys` (`KeyCharacterMap.getEvents`), each with a tree readback of the focused node. `android-host-spike a4 <serial>` (test APK `edit` mode) and `a4chrome <serial>`. Test string `héllo 世界 مرحبا 👋🏽` (19 UTF-16 units).

| Measure | API 37 | API 26 |
|---|---|---|
| `paste`, exact readback | yes (agent 46.7 ms) | yes (29.3 ms) |
| `set_text`, exact readback | yes (16.9 ms) | yes (38.4 ms) |
| `keys` with `héllo` | fails before typing: `unmappable character U+E9 at 1` | same |
| `keys` with `hi 👋` | fails before typing: `unmappable character U+1F44B at 3` (so `getEvents` returns `null` for the emoji; nothing typed) | same |
| `keys` ASCII `hello world` | exact | exact |
| 100 characters, 10 runs each, agent-side time p50 / p95 | paste 51.0 / 68.9 ms (earlier run 97.6 / 163.1), set 20.7 / 42.2, keys 1,562 / 1,710 ms; exact readback 10/10 for all | paste 31.7 / 69.7, set 21.0 / 58.1, keys 2,391 / 2,602; 10/10 |
| Old clipboard text restored after paste | yes (`OLD-CLIP`) | yes |
| Clip with a `content://settings/system/screen_off_timeout` URI on the clipboard, then paste | URI clip intact afterwards (`text/uri-list`, same item 0 URI) | same (`content://media/...`) |

(The URI could not be a media URI on API 37: the shell uid got `SecurityException: UID 2000 does not have permission to content://media/external/images/media/42`, so an image on the clipboard that the shell cannot read cannot be restored by it either. The agent reads the `ClipData` object back and sets the same object, which kept the URI intact for a URI it may access.)

An early API 26 run failed every clipboard call with `Can't create handler inside thread that has not called Looper.prepare()`; the control and video threads now call `Looper.prepare()`.

**Chrome (API 37, a `data:` page with `<input>`, field cleared by 16 DEL keys between runs, old clip `OLD-CLIP`, paste `PASTED-NEW`; screenshots looked at).**

| `restore_ms` | Field after paste |
|---|---|
| 0 | `OLD-CLIP` (wrong: Chrome read the clipboard after we restored it) |
| 50 | `PASTED-NEW` |
| 150 | `PASTED-NEW` |
| 500 | `PASTED-NEW` |

So restoring right after injection does break WebView pastes, as architecture 6.2 says; 50 ms was enough in this run (one run per setting, so the 500 ms cap stays). The tree readback of a web `<input>` returned `""` while the field visibly held `PASTED-NEW` (Chrome exposes web input text lazily), so `verified` for WebView content can be a false negative.

**Clipboard UI on API 37 (looked at).** Setting the clip from the shell shows the clipboard preview chip at the bottom left (text preview, "copied" actions) 0.3 s after the call; it was gone at 4 s (screenshots at 0.3, 1.2 and 4.0 s; I viewed 0.3 s and 4.0 s). With `ClipDescription.setExtras(PersistableBundle)` carrying `android.content.extra.IS_SENSITIVE` = true the chip still appeared at 0.3 s but showed dots instead of the text. There is no way found to suppress it. On paste, the system also drew the toast "spikeapp pasted from your clipboard" / "Chrome pasted from your clipboard" for a few seconds (visible in the Chrome screenshots). Both appear in frames and screenshots. I did not measure the chip's changed area in pt², so whether a `type` reply can wait on it is open: Phase 2's type task should measure it with the frame stream before the settle thresholds are trusted after a paste.

**Decision.** D7 and the text design in 6.2 confirmed: Paste is the default, SetValue and Keystrokes are fallbacks with the limits above. `keys` cannot be the Unicode path (also fails on `é`).

## A5: Agent launch hygiene

**Run.** `android-host-spike a5 <serial>`.

| Check | API 37 | API 26 |
|---|---|---|
| (a) Close the shell stream, agent gone | gone after 1,054 ms (polling `ps` costs about 60 ms per round trip) | 1,137 ms |
| (b) `pkill -f phone-use-agent` | no match; agent stays alive | same |
| (b) `pkill -f dev.phoneuse.agent.Main` | kills the agent, but also the `sh -c` that ran it: the trailing `echo rc=$?` never printed | same |
| (b) `pkill -f 'dev.phoneuse.agent.[M]ain'` | kills the agent, prints `rc=0` | same |
| (c) Jar pushed 0444 | loads (`agent listening`) | loads |
| (c) Jar pushed 0644 | also loads (the file is owned by shell); the API 34+ rule against writable dex files did not trip for the shell uid | loads |
| (d) Connection with no token / wrong token | closed with no reply; agent log `control: bad token, dropping` | same |
| (d) Right token | `hello` reply with `api`, `model`, `display` | same |
| Extra: nothing connects | agent exits by itself after 10.17 s (exit code 3) | 10.12 s |

The process list while running shows two entries (`sh -c CLASSPATH=... app_process / dev.phoneuse.agent.Main 0.1 <name>` and `app_process / dev.phoneuse.agent.Main ...`), so a `pgrep -f` count is 2, not 1.

**Decision.** Launch rules in 6.2 hold with three changes: the `pkill` pattern needs the bracket, the stream-close kill takes about 1 s (tests must wait 1.5 s, not 1 s), and 0444 is hygiene rather than a requirement.

## Decisions

| Decision | Outcome |
|---|---|
| D2 resident agent via `app_process` | Confirmed on API 26 and 37; no instrumentation fallback. Amended: `clearCache()`, depth-first `getChild`, conflict text |
| D7 length-prefixed JSON | Held; tree JSON is 8 to 27 KB; serialization not separated from tree time |
| D10 continuous video stream | Held; cold start 219 to 433 ms per lease, static 3 fps, two settle thresholds |
| D11 own adb client | Confirmed; works through the tunnel, p50 52 to 64 ms per shell call vs 247 ms for the CLI |
| Architecture 4.3 | Step 1 (source node id) available on both APIs; recycling confirmed on both |
| Architecture 4.6 | Changed: `first_change_area` 128 pt² added |
| Architecture 6.2 | Changed: construction, `clearCache`, prefetch, pkill, restore delay, overlay |
| Architecture 6.3 | `FLAG_SECURE` detection rule confirmed on both |
| Core | `Other` with collection info maps to `List`; `settle::Config::first_change_area` |

## Not measured, and why

- Co-located budgets (tap p50 15 ms, snapshot 80 ms, screenshot 30 ms): the host binary was not run on win-pc (a Windows build and run would be a separate setup); all end-to-end numbers include the tunnel. Agent-side tap time (4 to 8 ms) and tree times (on-device) are not affected by the tunnel.
- Screenshot at 1/2.625 scale: only full-size PNG conversion was timed (7 + 52 ms on API 37), so the 30 ms screenshot budget is unproven.
- Hardware encoders, physical-phone profiles and OEM quirks: no physical device was touched (they belong to people). The emulator uses a software encoder.
- A video playing in the emulator's browser (never idle): an indeterminate spinner was used.
- TalkBack on API 26: not in the Google APIs image.
- The Chrome paste test on API 26: Chrome first-run navigation was only done on API 37.
- The Settings search field as the paste target: the test APK's `EditText` and Chrome were used instead.
- Largest prefetch batch per round trip: the batch contents cannot be hooked from the client side; only round trips per tree and the 50-node cap were recorded.
- Tablet AVD (`Pixel_Tablet_API_37`): not part of Phase 1's plan items.
