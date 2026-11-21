---
title: How to control an Android emulator with phonedriver
nav: Android emulator
meta: Start an Android emulator, check that phonedriver sees it, and drive it—including headless flags, hidden displays, and remote SSH forwarding.
description: Boot an Android emulator, verify with phonedriver doctor, and drive it locally or over SSH—including the exact headless flags used in the benchmarks.
seo: How to let an AI agent control an Android emulator over MCP
updated: 2026-11-20
---

Android emulators (API 26 and API 37) are phonedriver's most thoroughly exercised target: the full 21-scenario suite passes on both. All you need is `adb`; nothing gets installed inside the emulator.

## Start an emulator

Create an AVD in Android Studio's Device Manager (or with `avdmanager`), then let phonedriver start it and wait for it to boot:

```console
$ phonedriver boot Pixel_API_37
booted android:emulator-5554 · Pixel_API_37 · Android API 37
```

With nothing connected, any phonedriver command names the virtual devices that `boot` can start. You can also start one yourself:

```sh
emulator -avd Pixel_API_37
```

For headless CI or background runs:

```sh
emulator -avd Pixel_API_37 -no-window -no-audio -no-snapshot -gpu swangle_indirect
```

One hard-earned tip: on high-resolution tablet images (2560×1600), software rendering can starve the guest so badly that Chrome hangs and launches fail to settle. Giving the emulator `-gpu host -cores 8 -memory 8192` cleared that up immediately. If an emulator run feels flaky, check the emulator's CPU load before blaming the agent.

## Check phonedriver sees it

```sh
phonedriver devices
phonedriver doctor
```

The emulator shows up as `android:emulator-5554`. `doctor` probes emulators that no other session holds, verifies `adb`, the helper, `UiAutomation`, and the video stream, and prints exact fixes for anything not `ok`.

## Drive it

```sh
phonedriver snapshot
phonedriver tap "Dark theme"
phonedriver wait settled
```

`snapshot` prints the screen as a short numbered list (`@e3 button "Google" "Services & preferences"`), and every tap settles automatically before returning just what changed:

```screen
ok tapped @e10 "Dark theme"
~ @e10 switch "Dark theme" off → on
```

The very first command starts the daemon and pushes the helper jar (about 1.2–1.6 s cold start); everything after that runs in milliseconds.

## Keep the agent on a hidden screen

On Android 13 (API 33) or later, `PHONEDRIVER_DISPLAY=hidden` (or `--display hidden`) gives the agent a virtual display separate from the main screen. On a headless emulator that barely changes anything, though it's handy for testing the same switch you'd use on [a real phone](/docs/android-phone/#a-hidden-screen-for-the-agent). Switching modes restarts the helper (a few seconds); API levels below 33 report unsupported.

## Use a device on another machine

The daemon listens on a local Unix socket only. To drive an emulator plugged into another computer (a beefier desktop or CI box), forward that socket over `ssh` and point `PHONEDRIVER_SOCKET` at your end:

```sh
ssh -N -L /tmp/phonedriver-remote.sock:/Users/you/.phonedriver/run/daemon.sock remote-host &
export PHONEDRIVER_SOCKET=/tmp/phonedriver-remote.sock
phonedriver snapshot
```

Run any `phonedriver` command (or `phonedriver daemon`) on the remote host once so the socket exists, and match versions on both machines. `phonedriver screenshot -o shot.png` writes locally; the MCP `screenshot` tool returns the image bytes directly.

You can also forward `adb` itself (`ssh -N -L 5038:127.0.0.1:5037 remote-host` + `ADB_SERVER_SOCKET=tcp:127.0.0.1:5038`), though forwarding the daemon socket avoids paying tunnel latency on every internal helper round trip.

## Questions

### Can Claude Code control an Android emulator?

Yes. Boot the emulator, run `npx -y phonedriver init` (or `claude mcp add phonedriver -- npx -y phonedriver mcp`), and ask Claude to use it. Nothing gets installed inside the emulator ([Claude Code guide](/docs/claude-code/)).

### Does it work with a headless emulator?

Yes. Every speed and job benchmark on this site ran against `-no-window` emulators—phonedriver reads directly from the device, not your desktop window.

### Which API levels work?

The 21-scenario suite passes on API 26 and API 37, and Android 12 (API 31) screen reads were verified in 0.1.4. Hidden displays require Android 13 (API 33) or later.

## Test a VPN or another network app

`phonedriver network` lists what the emulator is connected to: each network and its transports, which is the default, DNS, and for a VPN its app and the user id ranges it carries, plus always-on VPN and lockdown. `phonedriver wait vpn:up` returns once a VPN is connected, `wait vpn:down` once it is gone, and `wait log:"tunnel up" --app "My VPN"` once the app writes a log line with that text from now on. A wait that times out says the last state it saw.

The first time an app starts a VPN, Android asks "Connection request" in a dialog; phonedriver shows it as an alert and `tap OK` answers it. To never see it on an emulator, authorise the app first:

```sh
phonedriver shell "appops set com.example.vpn ACTIVATE_VPN allow"
```

These were run on an API 37 emulator with a small VPN app made for the test. The same reads work on a physical Android phone; they were not run on one.
