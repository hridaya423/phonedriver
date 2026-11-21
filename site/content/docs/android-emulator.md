---
title: How to control an Android emulator with phonedriver
nav: Android emulator
meta: Start an Android emulator, check that phonedriver sees it, and drive it. Includes the headless flags used in my benchmarks and remote emulators.
description: Start an Android emulator, check phonedriver sees it, and drive it. Includes the headless flags used in the benchmarks and how to use an emulator on another machine.
seo: How to let an AI agent control an Android emulator over MCP
updated: 2026-11-20
---

Android emulators on API 26 and API 37 are the best-tested target: the 21-scenario device suite passes on both. phonedriver needs only adb and nothing installed in the emulator.

## Start an emulator

Create an AVD in Android Studio's Device Manager, or with the command-line tools. Then start it and wait for it to boot with one command:

```console
$ phonedriver boot Pixel_API_37
booted android:emulator-5554 · Pixel_API_37 · Android API 37
```

With nothing connected, any phonedriver command names the virtual devices that `boot` can start. You can also start one yourself:

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

The emulator appears as `android:emulator-5554`. `doctor` probes emulators that no other session holds, and checks adb, the agent, UiAutomation and the video stream, and says how to fix whatever is not `ok`.

## Drive it

```sh
phonedriver snapshot
phonedriver tap "Dark theme"
phonedriver wait settled
```

`snapshot` lists the screen as text, for example `@e3 button "Google" "Services & preferences"`. A tap returns the lines that changed instead of the whole screen:

```screen
ok tapped @e10 "Dark theme"
~ @e10 switch "Dark theme" off → on
```

The first command starts the daemon, finds the agent jar on the device (pushing it if needed) and starts the agent. Fresh daemon to first screen read took 1,620.5 ms at p50 on the benchmark emulator. After that each call costs milliseconds.

## Use the emulator's console

`phonedriver shell "emu geo fix -122.41 37.77"` sends a command to the emulator's console, as `adb emu` does: `geo` sets the location, `sms send` delivers a text message, `power` sets the battery, `network speed` and `network delay` slow the network, `sensor` and `rotate` move the sensors, and `avd name` says which virtual device it is.

Other console commands act on your computer, not on the emulator: `screenrecord` and `network capture` write files on it, `redir` opens a port on it, `kill` ends the emulator. `shell` refuses them. To allow every console command, stop the daemon with `phonedriver daemon stop` and start it again with `PHONEDRIVER_HOST_SHELL=1` set. A console command is one line, so a line break in it is always refused. A phone has no console.

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

## Questions

### Can Claude Code control an Android emulator?

Yes. Start the emulator, run `claude mcp add phonedriver -- npx -y phonedriver mcp`, and ask Claude Code to do something on it. Nothing is installed in the emulator. See [the Claude Code guide](/docs/claude-code/), or [Codex](/docs/codex/) and [other clients](/docs/cursor-and-mcp-clients/).

### Does it work with a headless emulator?

Yes. The speed benchmarks ran on an emulator started with `-no-window`, and phonedriver reads the screen from the device, not from a window on your desktop.

### Which API levels work?

The 21-scenario suite passes on API 26 and API 37. Release 0.1.4 fixed screen reads on Android 12 and was tested on an API 31 emulator. The hidden screen needs API 33 or later.

## Test a VPN or another network app

`phonedriver network` lists what the emulator is connected to: each network and its transports, which is the default, DNS, and for a VPN its app and the user id ranges it carries, plus always-on VPN and lockdown. `phonedriver wait vpn:up` returns once a VPN is connected, `wait vpn:down` once it is gone, and `wait log:"tunnel up" --app "My VPN"` once the app writes a log line with that text from now on. A wait that times out says the last state it saw.

The first time an app starts a VPN, Android asks "Connection request" in a dialog; phonedriver shows it as an alert and `tap OK` answers it. To never see it on an emulator, authorise the app first:

```sh
phonedriver shell "appops set com.example.vpn ACTIVATE_VPN allow"
```

These were run on an API 37 emulator with a small VPN app made for the test. The same reads work on a physical Android phone; they were not run on one.
