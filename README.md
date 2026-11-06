<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/assets/banner-dark.svg">
    <img alt="phone-use: Read the screen as text. Tap a ref. Get the diff." src=".github/assets/banner-light.svg" width="100%">
  </picture>
</p>

<p align="center"><b>One Rust binary. Your agent reads an Android or iOS screen as text, taps a ref, and gets back what changed.</b></p>

<p align="center">
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-FF4A1C?style=flat-square&labelColor=15140F"></a>
  <img alt="Rust 1.99 or later" src="https://img.shields.io/badge/rust-1.99%2B-FF4A1C?style=flat-square&labelColor=15140F">
  <img alt="Tested on Android emulators and the iOS Simulator" src="https://img.shields.io/badge/tested-android%20%7C%20ios%20simulator-FF4A1C?style=flat-square&labelColor=15140F">
</p>

<p align="center">
  <a href="docs/architecture.md">Architecture</a> · <a href="docs/decisions.md">Decisions</a> · <a href="docs/protocol.md">Protocol</a> · <a href="docs/bench">Benchmarks</a> · <a href="progress.md">Progress</a>
</p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/assets/divider-dark.svg">
    <img alt="" src=".github/assets/divider-light.svg" width="100%">
  </picture>
</p>

phone-use lets an AI agent see and operate Android and iOS devices through a CLI or an MCP server. A daemon keeps the device connection open, so each call costs milliseconds. Every action waits for the screen to settle and returns the lines that changed. The agent does not poll or sleep.

This is the shape of a snapshot. It is the golden file for a captured Settings tree (`crates/core/src/snapshots/phone_use_core__render__tests__real_settings_main.snap`), shortened to eight lines.

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

An action returns `+`, `-` and `~` lines instead of the whole screen. This is the design from `docs/architecture.md` section 1, shortened. The `~` line for a switch is also what the live suite saw on an API 37 emulator.

```console
$ phone-use tap @e10
ok tapped @e10 "Dark theme" · settled in 410ms
~ @e10 switch "Dark theme" off → on
```

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

## Quickstart

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

Connect a device or boot an emulator, then run the loop.

```sh
phone-use devices
phone-use snapshot
phone-use tap "Dark theme"
phone-use wait settled
```

`phone-use skill` prints [`SKILL.md`](SKILL.md), the instructions an agent reads before it uses the tools.

## Supported devices

| Device | Status | Needs |
|---|---|---|
| Android emulator, API 26 and 37 | Tested. The 21-scenario suite passes on both (M2, `docs/bench/m2.md`). | adb |
| Android phone or tablet | Built on the same path. Not tested on hardware. | adb, USB debugging on |
| iOS Simulator (iOS 26.5) | Tested. The 12-scenario suite passes (M4, `docs/bench/m4.md`). One flake in `settings-dark-appearance` is unexplained. | A Mac with Xcode 27 and XcodeGen |
| iOS device | In progress. Unit tests pass. Nothing is verified on hardware (M5 is not met). | |

The iOS Simulator tier builds and runs a small XCTest runner. Android needs no install on the phone: the daemon pushes and starts its agent.

## Use with an agent

Add the MCP server. For Claude Code:

```sh
claude mcp add phone-use -- /path/to/phone-use mcp
```

For other clients, use this entry:

```json
{
  "mcpServers": {
    "phone-use": { "command": "/path/to/phone-use", "args": ["mcp"], "env": { "PHONE_USE_DEVICE": "android:emulator-5554" } }
  }
}
```

The server has ten tools: `devices`, `snapshot`, `screenshot`, `tap`, `type`, `swipe`, `scroll`, `press`, `launch`, `wait`. Each prints the same text as the CLI verb of the same name. `PHONE_USE_DEVICE` is optional when one device is connected.

Typed secrets (`phone-use type --secret`, or `secret: true` over MCP) never reach logs, traces or replies. phone-use sends no telemetry. It talks to the device and nothing else.

## How it works

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/assets/loop-dark.svg">
    <img alt="The agent sends tap @e10 to the daemon. The daemon taps the device. The diff comes back after the screen settles." src=".github/assets/loop-light.svg" width="100%">
  </picture>
</p>

The CLI and the MCP server are thin clients. The daemon owns the device connection, the ref table and the settle logic. Refs such as `@e10` are re-located when the tap happens, so a list that shifted by a row still gets the right tap. See [`docs/architecture.md`](docs/architecture.md).

### Use a device on another machine

The daemon listens on a Unix socket and nowhere else. To drive a simulator or phone that is plugged into another machine, forward that socket over `ssh` and point the CLI at the local end:

```sh
ssh -N -L /tmp/phone-use-mini.sock:/Users/you/.phone-use/run/daemon.sock mini &
export PHONE_USE_SOCKET=/tmp/phone-use-mini.sock
phone-use snapshot
```

On the other machine, run any `phone-use` command once, or `phone-use daemon`, so that the socket exists. Both machines need the same `phone-use` version. If the forward is down, the error names `PHONE_USE_SOCKET`.

Two things differ from a local run. `phone-use screenshot` prints a path on the machine that has the device; fetch the file with `ssh mini cat <path> > shot.png`. The MCP `screenshot` tool returns the image itself, so it works unchanged.

## Speed

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/assets/speed-dark.svg">
    <img alt="Bar chart, log scale: phone-use against phone-harness 0.3.3 on tap, screenshot, two snapshots and typing 100 characters. phone-use is 3.0 to 153 times faster." src=".github/assets/speed-light.svg" width="100%">
  </picture>
</p>

phone-use against phone-harness 0.3.3 on an Android 17 emulator (API 37, headless, software rendering) on win-pc, median of 50 runs after 5 warm-ups (`docs/bench/m1.md`). Each phone-use figure is the slower of two runs. phone-harness ran in process, so its Python start is not counted.

| Operation | phone-use | phone-harness | Factor |
|---|---|---|---|
| tap | 36.5 ms | 110.7 ms | 3.0x |
| screenshot | 15.2 ms | 390.1 ms | 26x |
| snapshot, Settings | 67.7 ms | 2426.4 ms | 36x |
| snapshot, 1,000-row list | 15.5 ms | 2370.0 ms | 153x |
| type 100 characters | 288.3 ms | 2104.5 ms | 7.3x |
| tap, then the settled diff | 419.5 to 544.0 ms | not supported | |

phone-harness cannot type Unicode through `adb input text`. phone-use typed mixed-script text in the last typing row.

> [!NOTE]
> phone-use misses two of its five Android latency budgets. Tap is 29.7 and 36.5 ms against a 15 ms budget (D14). Typing 100 characters is 253 and 288 ms against 150 ms. On the iOS 26.5 Simulator, tap is 357 ms against 80 ms, because XCTest takes about 240 ms to confirm the event (D15). Both M2 and M4 ran on emulators and the Simulator. No physical Android phone or iOS device has been measured.

## Development

```sh
blaze build
BLAZE_TESTS=all blaze test
cargo clippy --all-targets -- -D warnings
```

Tests that need a device are ignored by default. Set `PHONE_USE_TEST_DEVICE` (for example `android:emulator-5554` or `sim:<udid>`) and pass `--ignored`. [`AGENTS.md`](AGENTS.md) holds the rules for working in this repository. [`progress.md`](progress.md) records what was built, with the evidence.

<details>
<summary>Brand</summary>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/assets/brand-dark.svg">
    <img alt="Brand sheet: ink, paper, tap orange, and the added, removed and changed colors. JetBrains Mono is the one typeface." src=".github/assets/brand-light.svg" width="100%">
  </picture>
</p>

The mark is a phone screen of text lines with a tap landing on one. The wordmark is drawn, and its hyphen is the tap. [`.github/assets/mark.svg`](.github/assets/mark.svg) is the app icon. Text in the assets is JetBrains Mono, converted to outlines, under the SIL Open Font License 1.1.

</details>

## License

MIT. See [`LICENSE`](LICENSE). The Android agent contains files copied from scrcpy under Apache-2.0; see [`NOTICE`](NOTICE).
