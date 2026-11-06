<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/assets/banner-dark.svg">
    <img alt="phone-use: Let your AI agent use a phone. Android and iOS, CLI and MCP, up to 153 times faster than phone-harness." src=".github/assets/banner-light.svg" width="100%">
  </picture>
</p>

<p align="center">
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-6b675e?style=flat-square&labelColor=161512"></a>
  <img alt="Tested on Android emulators, the iOS Simulator and iOS devices" src="https://img.shields.io/badge/tested-android%20emulator%20%7C%20ios-6b675e?style=flat-square&labelColor=161512">
</p>

phone-use gives an AI agent a text view of an Android or iOS screen and the means to tap, type and swipe. Every action returns only the lines that changed.

<p align="center">
  <img alt="Claude Code on the left and the iOS Simulator on the right. The agent opens Settings, scrolls to Developer, opens it and taps the Dark Appearance switch. The reply is one line, off to on, and the screen turns dark." src=".github/assets/demo.gif" width="100%">
</p>

<sub>Claude Code driving the iOS Simulator through the phone-use MCP server, real timing, idle trimmed, 19 s end to end, no failed calls. The prompt names Settings > Developer because the Simulator's Settings has no Display & Brightness: with only "Open Settings and turn on dark mode." two runs ended with the agent reporting it could not find the setting, without opening Developer.</sub>

## Speed

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/assets/speed-dark.svg">
    <img alt="Bar chart, log scale: phone-use against phone-harness 0.3.3 on tap, screenshot, two snapshots and typing 100 characters. phone-use is 3.0 to 153 times faster." src=".github/assets/speed-light.svg" width="100%">
  </picture>
</p>

phone-use against phone-harness 0.3.3 on an Android 17 emulator (API 37, headless, software rendering) on win-pc, median of 50 runs after 5 warm-ups ([`docs/bench/m1.md`](docs/bench/m1.md)). Each phone-use figure is the slower of two runs. phone-harness ran in process, so its Python start is not counted.

> [!NOTE]
> phone-use misses two of its five Android latency budgets. Tap is 29.7 and 36.5 ms against a 15 ms budget (D14). Typing 100 characters is 253 and 288 ms against 150 ms. On the iOS 26.5 Simulator, tap is 357 ms against 80 ms, because XCTest takes about 240 ms to confirm the event (D15). M2 and M4 ran on emulators and the Simulator. On an iPhone 14 Plus and an iPad Pro (M5), tap is 534 and 454 ms against 80 ms, and typing 100 characters on the iPad is 404 ms against 400. No physical Android phone has been measured.

<details>
<summary>All figures</summary>

| Operation | phone-use | phone-harness | Factor |
|---|---|---|---|
| tap | 36.5 ms | 110.7 ms | 3.0x |
| screenshot | 15.2 ms | 390.1 ms | 26x |
| snapshot, Settings | 67.7 ms | 2426.4 ms | 36x |
| snapshot, 1,000-row list | 15.5 ms | 2370.0 ms | 153x |
| type 100 characters | 288.3 ms | 2104.5 ms | 7.3x |
| tap, then the settled diff | 419.5 to 544.0 ms | not supported | |

phone-harness cannot type Unicode through `adb input text`. phone-use typed mixed-script text in the last typing row.

</details>

## Try it

1. Build phone-use (see [Install](#install)), then add it to Claude Code as an MCP server.

   ```sh
   claude mcp add phone-use -- /path/to/phone-use mcp
   ```

2. Boot a simulator or connect a device, then ask the agent.

   > Open Settings and turn on dark mode.

3. The agent calls the tools, for example `launch`, `swipe` and `tap`. Each reply is plain text. The tap that turns the switch on returns this (iOS Simulator):

   ```console
   ok tapped @e41 "Dark Appearance" at 337,198 · settled in 607ms
   - @e65 @e66 @e67
   ~ @e41 switch "Dark Appearance" off → on
   + @e68 button "L4S"
   ```

   `+`, `-` and `~` mark lines that appeared, left and changed. The agent does not poll or sleep: every action waits for the screen to settle.

## Install

phone-use is not published. Build it from source with Rust 1.99 or later.

```sh
cargo build --release
```

The build packs the Android agent, so it needs the Android SDK (`ANDROID_HOME` with build-tools and platform 37), `d8`, and JDK 17. To skip that step, set `PHONE_USE_AGENT_JAR` to a built `phone-use-agent.jar`. The binary is `target/release/phone-use`.

```console
$ target/release/phone-use version
phone-use 0.1.0
```

Run `phone-use doctor`. It checks adb, each Android device, the agent, UiAutomation, the video stream, Xcode, the simulators, and the permissions of the daemon socket. Each line says `ok`, `warn` or `fail`, and a fix follows every line that is not `ok`.

```console
$ phone-use doctor
ok    adb server       0029 at 127.0.0.1:5037
warn  android devices  none connected
      fix: Connect a phone with USB debugging on, or start an emulator; `adb devices` lists what adb sees.
ok    xcode            Xcode 27.0, Build version 27A266a
ok    xcodegen         /opt/homebrew/bin/xcodegen
warn  simulators       none booted
      fix: Boot one in Simulator.app or with `xcrun simctl boot <udid>`; `xcrun simctl list devices available` lists them.
ok    daemon socket    /Users/you/.phone-use/run/daemon.sock mode 600 in a directory of mode 700
```

The CLI runs the same loop. `phone-use skill` prints [`SKILL.md`](SKILL.md), the instructions an agent reads before it uses the tools.

```sh
phone-use devices
phone-use snapshot
phone-use tap "Dark theme"
phone-use wait settled
```

## Supported devices

| Device | Status | Needs |
|---|---|---|
| Android emulator, API 26 and 37 | Tested. The 21-scenario suite passes on both (M2, `docs/bench/m2.md`). | adb |
| Android phone or tablet | Built on the same path. Not tested on hardware. | adb, USB debugging on |
| iOS Simulator (iOS 26.5) | Tested. The 12-scenario suite passes (M4, `docs/bench/m4.md`). One flake in `settings-dark-appearance` is unexplained. | A Mac with Xcode 27 and XcodeGen |
| iPhone or iPad (Runner tier) | Tested on an iPhone 14 Plus (iOS 26.1) and an iPad Pro (iOS 27.0) through the CLI and MCP (M5, `docs/bench/m5.md`). Screenshot, snapshot and the settled tap diff are within budget. A tap misses its 80 ms budget (454 to 534 ms at p50). The 12-scenario suite was not run on devices. | A Mac with Xcode and a signing team (`PHONE_USE_TEAM`) |

The iOS Simulator tier builds and runs a small XCTest runner. Android needs no install on the phone: the daemon pushes and starts its agent.

## How it works

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/assets/loop-dark.svg">
    <img alt="The agent sends tap @e10 to the daemon. The daemon taps the device. The diff comes back after the screen settles." src=".github/assets/loop-light.svg" width="100%">
  </picture>
</p>

The CLI and the MCP server are thin clients. A daemon keeps the device connection open, so each call costs milliseconds. The daemon owns the ref table and the settle logic. Refs such as `@e10` are re-located when the tap happens, so a list that shifted by a row still gets the right tap. Design notes: [`docs/architecture.md`](docs/architecture.md), [`docs/protocol.md`](docs/protocol.md), [`docs/decisions.md`](docs/decisions.md).

A snapshot lists the screen as text. This is the Android Settings app, cut to eight lines.

```console
$ phone-use snapshot
device android:emulator-5554 · Pixel 9 · Android 17 · 360x800pt · portrait
app com.android.settings (Settings) · keyboard hidden · settled in 212ms
@e1 button "Search Settings"
@e2 group scrollable↓
  @e3 button "Google" "Services & preferences"
  @e4 button "Network & internet" "Mobile, Wi‑Fi, hotspot"
  @e5 button "Connected devices" "Bluetooth, pairing"
  @e6 button "Apps" "Assistant, recent apps, default apps"
```

A tap returns the lines that changed instead of the whole screen.

```console
$ phone-use tap @e10
ok tapped @e10 "Dark theme" · settled in 410ms
~ @e10 switch "Dark theme" off → on
```

### Use a device on another machine

The daemon listens on a Unix socket and nowhere else. To drive a simulator or phone that is plugged into another machine, forward that socket over `ssh` and point the CLI at the local end:

```sh
ssh -N -L /tmp/phone-use-mini.sock:/Users/you/.phone-use/run/daemon.sock mini &
export PHONE_USE_SOCKET=/tmp/phone-use-mini.sock
phone-use snapshot
```

On the other machine, run any `phone-use` command once, or `phone-use daemon`, so that the socket exists. Both machines need the same `phone-use` version. If the forward is down, the error names `PHONE_USE_SOCKET`.

Two things differ from a local run. `phone-use screenshot` prints a path on the machine that has the device; `phone-use screenshot -o shot.png` writes the file on your machine instead. The MCP `screenshot` tool returns the image itself, so it works unchanged.

### Android over Wi-Fi

An Android 11 or later phone on the same network needs no cable. On the phone, turn on Developer options, then Wireless debugging, and choose Pair device with pairing code. Then:

```sh
adb pair 192.168.1.20:37099
adb connect 192.168.1.20:41233
phone-use devices
```

The first address and the six-digit code are on the pairing screen; the second address is on the Wireless debugging screen (the port changes each time wireless debugging is switched on). The phone then appears as `android:192.168.1.20:41233`. If it shows `offline`, `phone-use doctor` says what to check.

## Use with other MCP clients

Any client that starts a stdio server can use this entry.

```json
{
  "mcpServers": {
    "phone-use": { "command": "/path/to/phone-use", "args": ["mcp"], "env": { "PHONE_USE_DEVICE": "android:emulator-5554" } }
  }
}
```

The server has eleven tools: `devices`, `apps`, `snapshot`, `screenshot`, `tap`, `type`, `swipe`, `scroll`, `press`, `launch`, `wait`. Each prints the same text as the CLI verb of the same name. `PHONE_USE_DEVICE` is optional when one device is connected.

Typed secrets (`phone-use type --secret`, or `secret: true` over MCP) never reach logs, traces or replies. phone-use sends no telemetry. It talks to the device and nothing else.

## Development

```sh
blaze build
BLAZE_TESTS=all blaze test
cargo clippy --all-targets -- -D warnings
```

Tests that need a device are ignored by default. Set `PHONE_USE_TEST_DEVICE` (for example `android:emulator-5554` or `sim:<udid>`) and pass `--ignored`. [`AGENTS.md`](AGENTS.md) holds the rules for working in this repository.

<details>
<summary>Brand</summary>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/assets/brand-dark.svg">
    <img alt="Brand sheet: the lockup, the mark at 16, 32, 64 and 128 pixels, ten Sodium colour tokens, and the two typefaces Inter Display and Commit Mono." src=".github/assets/brand-light.svg" width="100%">
  </picture>
</p>

The brand is built on the [Sodium](https://design.how/systems/sodium) design system: square corners, 1px rules, one weight, and one yellow. The mark is a phone frame drawn in rule weight with one row on the selection yellow, the ref that was tapped. Names and sentences are Inter Display 400; refs, code and labels in capitals are Commit Mono 400. [`design/logo/`](design/logo) holds the logo and favicon files, [`design/brand.md`](design/brand.md) the rules, and `python3 design/build/build.py` rebuilds every image. Text in the images is converted to outlines; both fonts are under the SIL Open Font License 1.1, and Sodium is MIT licensed (see [`NOTICE`](NOTICE)).

</details>

## License

MIT. See [`LICENSE`](LICENSE). The Android agent contains files copied from scrcpy under Apache-2.0; see [`NOTICE`](NOTICE).
