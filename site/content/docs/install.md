---
title: How to install phonedriver
nav: Install
description: Set up phonedriver in one command with npx, Homebrew, a plugin, or a single prompt pasted into your agent—then check your machine with phonedriver doctor.
updated: 2026-11-21
seo: Install phonedriver: npx, Homebrew, plugin or one prompt
---

phonedriver ships as one self-contained binary: CLI, daemon, MCP server, and the Android helper jar packed inside. Nothing extra to download on the phone.

> The current release is phonedriver 0.1.8, on npm, Homebrew and GitHub Releases. You only need to build from source if you want to change it.

## The one-command setup

```sh
npx -y phonedriver init
```

`init` finds supported clients on your machine (Claude Code, Codex, Cursor, VS Code, Gemini CLI), wires up the MCP server without clobbering existing configs, and checks your tools and connected devices. Want to see what it will touch first? Run `npx -y phonedriver init --dry-run`.

Pick either `init` or a plugin install below—doing both registers the tools twice.

## Or let your agent set itself up

Paste this into Claude Code, Codex, Cursor, or any agent with a shell. It checks your machine, registers the MCP server, and tells you what to plug in:

```prompt
{{BOOT}}
```

Everything below is the same setup by hand.

## By hand: npx

The `phonedriver` npm package is a thin launcher (Node 18+). On first run it downloads the binary for your OS from GitHub Releases, verifies its SHA-256 against the published `.sha256` file, caches it locally, and runs it. Every run after that stays completely offline.

Cache paths: `~/Library/Caches/phonedriver/npm` (macOS), `~/.cache/phonedriver/npm` (Linux), `%LOCALAPPDATA%\phonedriver\npm` (Windows).

```sh
npx -y phonedriver --version
claude mcp add phonedriver -- npx -y phonedriver mcp
```

## As a plugin

For Claude Code and Codex, the plugin installs both the MCP server and the phonedriver skill in one go. Remove any manual `mcp add` entry first so the tools only appear once.

```sh
# Claude Code
claude plugin marketplace add hridaya423/phonedriver
claude plugin install phonedriver@phonedriver

# Codex
codex plugin marketplace add hridaya423/phonedriver
codex plugin add phonedriver@phonedriver
```

## Permanent install

Put `phonedriver` straight onto your `PATH`:

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

The shell and PowerShell installers drop `phonedriver` into `$CARGO_HOME/bin` (`~/.cargo/bin`). On macOS, the iOS runner sources live inside the binary too—Xcode builds them automatically the first time you target a Simulator or iPhone.

## Build from source

Requires Rust 1.99 or later:

```sh
cargo build --release
```

The default build packs the Android helper, so it expects the Android SDK (`ANDROID_HOME` or `ANDROID_SDK_ROOT`) with build-tools **37.0.0**, platform **android-37.0**, `d8`, and JDK 17. Override with `ANDROID_BUILD_TOOLS` and `ANDROID_PLATFORM`, or point `PHONEDRIVER_AGENT_JAR` at a prebuilt jar (`bash android-agent/build.sh`).

Building for iOS only, without an Android toolchain:

```sh
cargo build --release -p phonedriver --no-default-features
```

The compiled binary lands at `target/release/phonedriver`.

## Check it works

```console
$ phonedriver version
phonedriver 0.1.8
```

`phonedriver doctor` checks `adb`, connected devices, the helper, `UiAutomation`, the video stream, Xcode, booted simulators, and daemon socket permissions. It starts the agent only on emulators and Simulators that no other session holds; a physical phone or tablet is listed as `not probed` with the command to probe it (`phonedriver doctor --device <id>`), and `--all-devices` probes them all. Anything that isn't `ok` prints the exact fix right underneath:

```console
$ phonedriver doctor
ok    adb server       0029 at 127.0.0.1:5037
warn  android devices  none connected
      fix: Connect a phone with USB debugging on, or start an emulator; `adb devices` lists what adb sees.
ok    xcode            Xcode 27.0, Build version 27A266a
ok    daemon socket    /Users/you/.phonedriver/run/daemon.sock mode 600 in a directory of mode 700
```

## Next

Wire up your client—[Claude Code](/docs/claude-code/), [Codex](/docs/codex/), [VS Code](/docs/vs-code/), or [Cursor and other MCP clients](/docs/cursor-and-mcp-clients/)—then connect an [Android emulator](/docs/android-emulator/), a [real Android phone](/docs/android-phone/), the [iOS Simulator](/docs/ios-simulator/), or a [real iPhone](/docs/iphone/).

## Environment variables

| Name | Default | Meaning |
|---|---|---|
| `PHONEDRIVER_HOME` | `~/.phonedriver` | Daemon home: socket, `daemon.log`, `traces/`, `tmp/`, and cached iOS `runner/` builds. Override to run an isolated daemon. |
| `PHONEDRIVER_SOCKET` | under `PHONEDRIVER_HOME` | Daemon socket or named pipe. Point at an `ssh -L` forward to drive another machine's phones. |
| `PHONEDRIVER_DEVICE` | none | Default device ID when `--device` is omitted (also read by `phonedriver mcp`). |
| `PHONEDRIVER_DISPLAY` | `main` | Set to `hidden` (or `--display hidden`) on Android 13+ to run the agent on a hidden second display while you keep using the phone ([Android phone guide](/docs/android-phone/#a-hidden-screen-for-the-agent)). |
| `PHONEDRIVER_SESSION` | `default` | Session name (same as `--session`). |
| `PHONEDRIVER_ALLOW_DEVICES`, `PHONEDRIVER_DENY_DEVICES` | none | Comma lists of device ids (`android:emulator-5554`, `ios:<udid>`) or an Android phone's hardware serial. A device not allowed, or denied, is not listed, not probed by `doctor`, not booted, and never gets the agent; `--force` does not change that. `~/.phonedriver/devices` (`allow <id>` and `deny <id>` lines) does the same for every agent on the machine ([reference]({{REPO}}/blob/main/docs/reference.md#devices-an-agent-must-not-touch)). |
| `PHONEDRIVER_TEAM` | your Mac's Apple Development cert team | Apple developer team ID for signing the iOS runner on physical devices. Only needed if you have multiple signing teams. |
| `PHONEDRIVER_GUARD` | on | Set to `off` to disable the [approval guard](/docs/approval-guard/). |
| `PHONEDRIVER_ASK` | none | Comma-separated extra tap categories to hold for approval: `send`, `delete`, `install` ([approval guard](/docs/approval-guard/)). |
| `PHONEDRIVER_SECRET` | none | Secret input for `type --secret` when not piping on stdin. |
| `PHONEDRIVER_TRACE_FULL`, `PHONEDRIVER_TRACE_SCREENS` | off | Include full screen text or screenshots inside local traces. |
| `ADB_SERVER_SOCKET` | `tcp:127.0.0.1:5037` | Target `adb` server address (handy for SSH tunnels). |
