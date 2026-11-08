# Hidden second screen on Android 13+

> **For Claude:** REQUIRED SUB-SKILL: Use `executing-plans` to implement task-by-task.

**Goal:** `--display hidden` (or `PHONEDRIVER_DISPLAY=hidden`) makes the Android agent work on a virtual display the human cannot see, so the phone's own screen stays the human's. Every verb works on that display. The default stays the main screen. Decision D32.

**Architecture:** `docs/architecture.md` sections 6.2 (agent), 6.3 (states), 8 (video), 10.1 (flags).

**Prior art:** scrcpy 3.x `--new-display` (`video/NewDisplayCapture.java`, commit read 2026-11-08): `DisplayManager.createVirtualDisplay` with `PUBLIC | PRESENTATION | OWN_CONTENT_ONLY | SUPPORTS_TOUCH | ROTATES_WITH_CONTENT | DESTROY_CONTENT_ON_REMOVAL | SHOULD_SHOW_SYSTEM_DECORATIONS`, plus on API 33 `TRUSTED | OWN_DISPLAY_GROUP | ALWAYS_UNLOCKED | TOUCH_FEEDBACK_DISABLED`, on API 34 `OWN_FOCUS | DEVICE_DISPLAY_GROUP`. The shell user holds `ADD_TRUSTED_DISPLAY`. `scrcpy`'s `DisplayManager.createNewVirtualDisplay` is already in `android-agent/`.

## Shape

- The mode is a property of a call, not of the daemon: `display: "hidden"` in the request params (like `share`). The CLI flag and the MCP server's environment variable fill it. An actor whose live device is in the other mode drops it and connects again in the requested mode (the agent restarts, a few seconds). The lease still applies first.
- The agent gets a third argument, `hidden`. On API below 33 it prints why and exits 5; the host turns that into `Unsupported` for `display`. Otherwise it creates the encoder, then the virtual display with the encoder's input surface (same size and density as the main display, scaled down together when the encoder refuses the size), and reports the display id in `hello` as `hidden_display`.
- Everything the agent does is scoped to that id: windows (`getWindowsOnAllDisplays().get(id)`), the focused input node, injected events (`setDisplayId`), `launch` and `open_url` (`ActivityOptions.setLaunchDisplayId`), foreground, power (always on, never locked, so the daemon never wakes or unlocks the phone), video (the codec is fed by the display itself).
- Keys that act on the phone, not on the display (power, sleep, wakeup), are refused in hidden mode: `press lock` would lock the human's phone.

## Tasks

### Task 10.1: Protocol and host plumbing (tests first)

**Tests:** `crates/android/src/agent.rs` unit: `Hello` reads `hidden_display`; exit 5 maps to `Unsupported` for `display`. `crates/cli/tests/daemon.rs`: a call with `display: "hidden"` connects through `connect_on` with the hidden mode; the next call with no `display` connects again in the main mode; a source that does not support hidden answers `Unsupported`. `crates/cli/tests/cli.rs`: `--display hidden` and `PHONEDRIVER_DISPLAY=hidden` put `display` in the params; `devices --json` carries `hidden_display` for a live hidden device.

### Task 10.2: The agent

Java, proven on the API 37 tablet emulator (`emulator-5558`) and the API 26 emulator (the error): `Hidden.java` (display and encoder), and scoping in `Accessibility`, `Input`, `Apps`, `DeviceState`, `Video`, `Monitor`, `Ops`.

### Task 10.3: Proof and measurement

Testbed launched on the hidden display, taps and typing seen in the testbed's event log, a screenshot of the main display (`screencap -d`) that shows the launcher unchanged. Speed bench of snapshot, tap and screenshot on the main and hidden display of the same emulator: `docs/bench/hidden-display.md`.

## Result (2026-11-08)

Proven on `emulator-5558` (API 37 tablet): taps, typing, clipboard, keys, swipe, scroll, launch and screenshots on display 80 with the testbed's event log as the witness, and a screenshot of the main display next to it (`docs/bench/data/hidden-display-2026-11-08/`). API 26 (`emulator-5556`) fails `Unsupported` for `display` with the Android 13 message. Live test `device_live::the_testbed_on_the_hidden_display_gets_the_taps_and_is_a_task_of_that_display`. Speed in `docs/bench/hidden-display.md`. Unproven: physical phones, HONOR and Xiaomi builds, API 33 without `OWN_FOCUS`, an app that forces an orientation.
