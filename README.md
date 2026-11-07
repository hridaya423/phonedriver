<p align="center"><picture> <source media="(prefers-color-scheme: dark)" srcset=".github/assets/banner-dark.svg"> <img alt="phonedriver: Let your AI agent use a phone. Android and iOS, CLI and MCP, up to 98 times faster than adb." src=".github/assets/banner-light.svg" width="100%"> </picture></p>

phonedriver is an open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones. It shows a screen as numbered text, and every tap, type or swipe returns only the lines that changed.

<p align="center">
  <a href="LICENSE"><img alt="License: AGPL-3.0" src="https://img.shields.io/badge/license-AGPL--3.0-6b675e?style=flat-square&labelColor=161512"></a>
  <img alt="Tested on Android emulators, the iOS Simulator and iOS devices" src="https://img.shields.io/badge/tested-android%20emulator%20%7C%20ios-6b675e?style=flat-square&labelColor=161512">
</p>

<p align="center">
  <a href="docs/bench/speed-2026-11-07.md">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset=".github/assets/hero-dark.svg">
      <img alt="Bar chart of phonedriver against adb at p50 on an Android emulator, up to 98 times faster. read the screen (1,000-row UI tree): 24.0 ms against 2,352.4 ms for adb, 98 times faster; screenshot: 11.6 ms against 476.3 ms for adb, 41 times faster; type 100 characters: 251.1 ms against 4,840.9 ms for adb, 19 times faster; tap and see what changed: 586.9 ms against 2,725.8 ms for adb, 4.6 times faster. phone-harness is 111, 49, 22 times slower than phonedriver on the first 3 rows and does not support tap and see what changed." src=".github/assets/hero-light.svg" width="100%">
    </picture>
  </a>
</p>

<sub>phonedriver against plain adb on an Android 17 emulator, p50 of 50 runs. adb is the full-width bar; phonedriver's bar is its true fraction of it. <a href="docs/bench/speed-2026-11-07.md">Method, p95, confidence intervals and raw data</a>.</sub>

<p align="center">
  <img alt="Claude Code's real terminal on the left and an Android 17 emulator on the right. Claude Code is asked to order a butter chicken and a garlic naan on Uber Eats from the best-rated Indian restaurant near Times Square. It rejects the cookie banner, sets the address, searches for Indian food, picks Patiala Indian Grill and Bar (4.8 stars), adds both items, reaches checkout, stops at Uber's sign-in page without entering anything, and asks the user to sign in and approve the order." src=".github/assets/demo.gif" width="100%">
</p>

<sub>A real Claude Code session driving an Android emulator through phonedriver, sped up 10x: the run took 6 minutes 7 seconds, and the last 3 seconds play at real speed. Signed out, Uber Eats asks for a sign-in at checkout, so nothing was ordered. An <a href=".github/assets/demo.mp4">MP4</a> is at higher quality.</sub>

## Try it

1. Install phone-use first (see [Install](#install)), then add it to Claude Code as an MCP server. Use the binary's full path.

   ```sh
   claude mcp add phone-use -- "$(pwd)/target/release/phone-use" mcp
   claude mcp list    # expect: phone-use ... Connected
   ```

   For the first run, boot one simulator or connect one device, so no `--device` or `PHONE_USE_DEVICE` is needed. `phone-use skill` prints the instructions an agent reads; the MCP server also carries a short version.

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

After the first release (nothing is published yet, so these do not work today), one line installs the prebuilt binary. Each artifact has the Android agent inside.

```sh
# macOS and Linux
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/hridaya423/phone-use/releases/latest/download/phone-use-installer.sh | sh

# Homebrew (macOS and Linux)
brew install hridaya423/tap/phone-use
```

```powershell
# Windows
powershell -ExecutionPolicy Bypass -c "irm https://github.com/hridaya423/phone-use/releases/latest/download/phone-use-installer.ps1 | iex"
```

The shell and PowerShell installers put `phone-use` in `$CARGO_HOME/bin` (`~/.cargo/bin`) and add it to your `PATH`. The iOS Simulator runner needs no download: its sources are inside the binary, and the first command that needs it builds it on your Mac with Xcode and XcodeGen.

To build from source instead, use Rust 1.99 or later.

```sh
cargo build --release
```

The build packs the Android agent, so it needs the Android SDK with build-tools **37.0.0** and platform **android-37.0** (`ANDROID_HOME`, falling back to `ANDROID_SDK_ROOT`, then `/opt/homebrew/share/android-commandlinetools`), `d8` and JDK 17. Override the versions with `ANDROID_BUILD_TOOLS` and `ANDROID_PLATFORM`. If anything is missing the build says exactly what and how to get it. To skip that step, set `PHONE_USE_AGENT_JAR` to a built `phone-use-agent.jar` (`bash android-agent/build.sh` makes one; none is published yet).

**iOS only, no Android toolchain:**

```sh
cargo build --release -p phone-use --no-default-features
```

That binary has no Android support (`phone-use doctor` skips the Android checks). Run the tests with the default features. The binary is `target/release/phone-use`.

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

### iPhone or iPad

A physical iOS device needs the runner signed with your Apple developer team (a free Apple ID works and expires every 7 days). Plug the device in, unlock it, then:

```sh
phone-use ios setup                  # pairing, Developer Mode, developer disk image
export PHONE_USE_TEAM=<your team ID>   # Xcode > Settings > Accounts, or: security find-identity -v -p codesigning
phone-use --device ios:<udid> snapshot
```

Without a team, the command stops with exit code 3 and says so. With a booted Simulator and an iPhone that has no team, the Simulator is the default device. The daemon reads `PHONE_USE_TEAM` when it starts, so run `phone-use daemon stop` after setting it.

## Numbers

Speed is p50 of 50 runs on an Android 17 emulator (API 37). Tokens are the sum of per-task medians over three agent tasks with `claude-sonnet-5-5`; every run passed for phonedriver and phone-harness. The speed-up over adb is in brackets.

| | phonedriver | adb | phone-harness 0.3.3 | mobile-mcp 1.0.16 |
|---|---|---|---|---|
| Read a 1,000-row UI tree | 24.0 ms (98x) | 2,352.4 ms | 2,653.7 ms | not measured |
| Screenshot | 11.6 ms (41x) | 476.3 ms | 570.5 ms | not measured |
| Type 100 characters | 251.1 ms (19x) | 4,840.9 ms | 5,523.3 ms | not measured |
| Tap and see what changed | 586.9 ms (4.6x) | 2,725.8 ms | not supported | not measured |
| Tokens, three tasks | 218,357 (34% fewer than phone-harness) | one task failed | 330,375 | 0 of 3 passed (environment failure) |
| Diff after each action | Yes | No | Not supported | No |
| Approval guard on payments | Yes | No | Not checked | Not checked |
| iOS | Simulator, iPhone, iPad | No | iPhone via Mac mirroring | Yes |
| Shell on the device | Yes | Yes | Not checked | Not documented |
| MCP and CLI | Both | CLI | CLI | MCP |

Full benchmark: [every op, p95, confidence intervals, raw data](docs/bench/speed-2026-11-07.md). Tokens: [`docs/bench/tokens.md`](docs/bench/tokens.md). Machine-readable: [`speed.json`](docs/bench/speed.json), [`tokens.json`](docs/bench/tokens.json). Side by side: [`docs/compare.md`](docs/compare.md).

> [!NOTE]
> The bare tap is the closest row. Host to reply, phonedriver takes 175.6 ms when taps come back to back and 61.6 ms when they are 1.5 s apart, against 199.5 and 106.6 ms for adb. Tap misses its 15 ms budget (D14), and typing 100 characters misses its 150 ms budget (251.1 ms). On the iOS 26.5 Simulator, tap is 357 ms against 80 ms, because XCTest takes about 240 ms to confirm the event (D15). On an iPhone 14 Plus and an iPad Pro (M5), tap is 534 and 454 ms. No physical Android phone has been measured in this run. mobile-mcp has no speed figure here, and its token runs failed for an environment reason, so its column says nothing about its design.

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

## Use with other MCP clients

Any client that starts a stdio server can use this entry.

```json
{
  "mcpServers": {
    "phone-use": { "command": "/path/to/phone-use", "args": ["mcp"], "env": { "PHONE_USE_DEVICE": "android:emulator-5554" } }
  }
}
```

The server has fourteen tools: `devices`, `apps`, `snapshot`, `screenshot`, `tap`, `type`, `swipe`, `scroll`, `press`, `launch`, `wait`, `batch`, `clipboard`, `shell` (`apps` also installs, stops, clears and uninstalls). Each prints the same text as the CLI verb of the same name. `PHONE_USE_DEVICE` is optional when one device is connected.

Typed secrets (`phone-use type --secret`, or `secret: true` over MCP) never reach logs, traces or replies. phone-use sends no telemetry. It talks to the device and nothing else: the local daemon socket, the local adb server (or the address in `ADB_SERVER_SOCKET`) and the device, plus Apple only when mounting an iOS developer disk image. `cargo test -p phone-use --test network` runs the CLI, the daemon and the MCP server under a macOS sandbox that kills a process the moment it opens a connection to a non-local address (it runs on the macOS CI job; Linux and Windows are not covered).

## FAQ

### How do I let Claude control my Android phone?

Add phonedriver to Claude Code as an MCP server with `claude mcp add`, then connect a phone with USB debugging on or start an emulator; nothing is installed on the phone. The suite passes on emulators (API 26 and 37); a phone uses the same path and is not tested on hardware. See [Try it](#try-it).

### What is the fastest way for an AI agent to control a phone?

phonedriver reads a 1,000-row Android screen in 24.0 ms where `adb` takes 2,352.4 ms, 98x faster at p50 on an emulator. It is 41x faster for a screenshot and 19x faster for typing 100 characters; a tap sent back to back is 1.1x faster. No physical phone is measured yet.

### Does phonedriver work with iPhone?

Yes. It drives the iOS Simulator, and iPhones and iPads when the runner is signed with an Apple developer team, tested on an iPhone 14 Plus and an iPad Pro. A tap on a device takes 454 to 534 ms. It needs a Mac with Xcode.

### Can an AI agent buy things with phonedriver?

Not without a person. The approval guard stops a tap on a label such as `Place order`, `Pay` or `Delete account` and waits for the human to approve (exit code 5 on the CLI). It matches a fixed list of English labels, so a button with no label is not guarded.

### How is phonedriver different from adb or mobile-mcp?

adb sends raw input and returns raw XML; phonedriver returns a pruned numbered tree and, after each action, the lines that changed. mobile-mcp is another MCP server for Android and iOS; its source has no wait after actions, so an agent lists elements again, and it has no speed figure here. See [`docs/compare.md`](docs/compare.md).

### How many tokens does it use?

Across three tasks phonedriver used 218,357 tokens against 330,375 for phone-harness, 34% fewer, and it passed every run. It used more on one task (57,753 against 53,403), and each figure is a median of two to three runs.

### Is phonedriver free?

Yes. It is AGPL-3.0-only: if you run a modified version as a network service, you must publish your source under the same license. It sends no telemetry.

## More

[`docs/reference.md`](docs/reference.md) holds the environment variables, debugging and exit codes, using a device on another machine, Android over Wi-Fi, the `shell` verb, and the development and brand notes. [`llms.txt`](llms.txt) indexes the docs for language models.

## License

AGPL-3.0-only. See [`LICENSE`](LICENSE). If you distribute phone-use, or run a modified version as a network service, you must publish your source under the same license. The Android agent contains files copied from scrcpy under Apache-2.0; see [`NOTICE`](NOTICE).
