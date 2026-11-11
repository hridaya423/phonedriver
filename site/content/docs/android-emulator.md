---
title: How to control an Android emulator with phonedriver
nav: Android emulator
meta: Start an Android emulator, check that phonedriver sees it, and drive it. Includes the headless flags used in my benchmarks and remote emulators.
description: Start an Android emulator, check phonedriver sees it, and drive it. Includes the headless flags used in the benchmarks and how to use an emulator on another machine.
---

Android emulators on API 26 and API 37 are the best-tested target: the 21-scenario device suite passes on both. phonedriver needs only adb and nothing installed in the emulator.

## Start an emulator

Create an AVD in Android Studio's Device Manager, or with the command-line tools, then start it:

```sh
emulator -avd Pixel_API_37
```

The speed benchmarks on this site ran headless, with software rendering:

```sh
emulator -avd Pixel_API_37 -no-window -no-audio -no-snapshot -gpu swangle_indirect
```

The real phone jobs benchmark of 10 November 2026 started its tablet emulator with `-gpu host -cores 8 -memory 8192`. A first rerun on software rendering at 2560x1600 starved the emulator: 17 launches answered "not settled" and one job met "Chrome isn't responding" seven times. If an agent run on an emulator is slow or flaky, check the emulator's load before you blame the tool.

## Check phonedriver sees it

```sh
phonedriver devices
phonedriver doctor
```

The emulator appears as `android:emulator-5554`. `doctor` checks adb, the agent, UiAutomation and the video stream, and says how to fix whatever is not `ok`.

## Drive it

```sh
phonedriver snapshot
phonedriver tap "Dark theme"
phonedriver wait settled
```

`snapshot` lists the screen as text, for example `@e3 button "Google" "Services & preferences"`. A tap returns the lines that changed instead of the whole screen:

```screen
ok tapped @e10 "Dark theme" · settled in 410ms
~ @e10 switch "Dark theme" off → on
```

The first command starts the daemon, finds the agent jar on the device (pushing it if needed) and starts the agent. Fresh daemon to first screen read took 1,620.5 ms at p50 on the benchmark emulator. After that each call costs milliseconds.

## Keep the agent on a hidden screen

On an emulator running Android 13 (API 33) or later, set `PHONEDRIVER_DISPLAY=hidden` in the MCP config, or pass `--display hidden` to the CLI, and the agent works on a second screen. On an emulator nobody is holding, that matters little, but it is the same switch as on [a real phone](/docs/android-phone/#a-hidden-screen-for-the-agent), where it keeps your own screen yours. It has been tested on an emulator (API 37) and on a HONOR X8. Switching between the two screens restarts the agent, which takes a few seconds. Below API 33 the call fails as unsupported.

## Use a device on another machine

The daemon listens on a Unix socket and nowhere else. To drive an emulator or phone plugged into another machine, forward that socket over `ssh` and point the CLI at the local end:

```sh
ssh -N -L /tmp/phonedriver-remote.sock:/Users/you/.phonedriver/run/daemon.sock remote-host &
export PHONEDRIVER_SOCKET=/tmp/phonedriver-remote.sock
phonedriver snapshot
```

On the other machine, run any `phonedriver` command once, or `phonedriver daemon`, so that the socket exists. Both machines need the same version. `phonedriver screenshot` then prints a path on the machine that has the device; `phonedriver screenshot -o shot.png` writes the file on yours. The MCP `screenshot` tool returns the image itself.

Alternatively, forward only adb: `ssh -N -L 5038:127.0.0.1:5037 remote-host` and `ADB_SERVER_SOCKET=tcp:127.0.0.1:5038`. Each device round trip then pays the tunnel's latency.
