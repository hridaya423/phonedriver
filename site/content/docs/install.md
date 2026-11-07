---
title: How to install phonedriver
nav: Install
description: Install the phonedriver binary with one command on macOS, Linux or Windows, or build it from source, then check it with phonedriver doctor.
---

phonedriver is one binary: the CLI, the daemon and the MCP server. The Android agent is packed inside it, so there is nothing else to download.

> Nothing is published yet. The commands under "One-line install" work once the first release is out. Until then, use "Build from source".

## One-line install

```sh
# macOS and Linux
curl --proto '=https' --tlsv1.2 -LsSf {{REPO}}/releases/latest/download/phonedriver-installer.sh | sh

# Homebrew (macOS and Linux)
brew install hridaya423/tap/phonedriver
```

```powershell
# Windows
powershell -ExecutionPolicy Bypass -c "irm {{REPO}}/releases/latest/download/phonedriver-installer.ps1 | iex"
```

The shell and PowerShell installers put `phonedriver` in `$CARGO_HOME/bin` (`~/.cargo/bin`) and add it to your `PATH`. The iOS Simulator runner needs no download: its sources are inside the binary, and the first command that needs it builds it on your Mac with Xcode and XcodeGen.

## Build from source

Use Rust 1.99 or later.

```sh
cargo build --release
```

The build packs the Android agent, so it needs the Android SDK with build-tools **37.0.0** and platform **android-37.0** (`ANDROID_HOME`, falling back to `ANDROID_SDK_ROOT`), `d8` and JDK 17. Override the versions with `ANDROID_BUILD_TOOLS` and `ANDROID_PLATFORM`. If anything is missing, the build says exactly what and how to get it. To skip that step, set `PHONEDRIVER_AGENT_JAR` to a built `phonedriver-agent.jar` (`bash android-agent/build.sh` makes one).

For iOS only, with no Android toolchain:

```sh
cargo build --release -p phonedriver --no-default-features
```

That binary has no Android support, and `phonedriver doctor` skips the Android checks. The binary is `target/release/phonedriver`.

## Check it works

```console
$ phonedriver version
phonedriver 0.1.0
```

`phonedriver doctor` checks adb, each Android device, the agent, UiAutomation, the video stream, Xcode, the simulators and the permissions of the daemon socket. Each line says `ok`, `warn` or `fail`, and a fix follows every line that is not `ok`.

```console
$ phonedriver doctor
ok    adb server       0029 at 127.0.0.1:5037
warn  android devices  none connected
      fix: Connect a phone with USB debugging on, or start an emulator; `adb devices` lists what adb sees.
ok    xcode            Xcode 27.0, Build version 27A266a
ok    daemon socket    /Users/you/.phonedriver/run/daemon.sock mode 600 in a directory of mode 700
```

## Next

Connect it to an agent: [Claude Code](/docs/claude-code/) or [Cursor and other MCP clients](/docs/cursor-and-mcp-clients/). Then pick a device: an [Android emulator](/docs/android-emulator/), a [real Android phone](/docs/android-phone/), the [iOS Simulator](/docs/ios-simulator/) or an [iPhone](/docs/iphone/).

## Environment variables

| Name | Default | Meaning |
|---|---|---|
| `PHONEDRIVER_HOME` | `~/.phonedriver` | Where the daemon keeps its socket, `daemon.log`, `traces/`, `tmp/` and the iOS `runner/` builds. Set it to run a separate daemon. |
| `PHONEDRIVER_SOCKET` | under `PHONEDRIVER_HOME` | The daemon's socket or named pipe. Point it at an `ssh -L` forward to use another machine's devices. |
| `PHONEDRIVER_DEVICE` | none | The device to use when `--device` is not given. Also read by `phonedriver mcp`. |
| `PHONEDRIVER_SESSION` | `default` | The session name, as `--session`. |
| `PHONEDRIVER_TEAM` | none | Apple developer team ID that signs the iOS runner on a physical device. |
| `PHONEDRIVER_GUARD` | on | `off` stops the [approval guard](/docs/approval-guard/). |
| `PHONEDRIVER_SECRET` | none | The text for `type --secret`, instead of stdin. |
| `PHONEDRIVER_TRACE_FULL`, `PHONEDRIVER_TRACE_SCREENS` | off | Keep observation text, or screenshots, in traces. |
| `ADB_SERVER_SOCKET` | `tcp:127.0.0.1:5037` | The adb server to use, for example a tunnel to another machine. |
