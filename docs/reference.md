# Reference

Moved out of the README. Commands use the current binary name, `phonedriver`.

## Install

After the first release (nothing is published yet, so these do not work today), one line gets you the prebuilt binary. Each artifact has the Android agent inside.

The shortest route is `npx`. The `phonedriver` npm package is a small launcher: on first run it downloads the binary for your platform from GitHub Releases, checks its SHA-256, caches it and runs it. It contacts GitHub only for that download and sends no telemetry.

```sh
npx -y phonedriver --version                              # try it
claude mcp add phonedriver -- npx -y phonedriver mcp      # Claude Code
```

The installers below put a permanent `phonedriver` on your `PATH` instead.

```sh
# macOS and Linux
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/hridaya423/phonedriver/releases/latest/download/phonedriver-installer.sh | sh

# Homebrew (macOS and Linux)
brew install hridaya423/tap/phonedriver
```

```powershell
# Windows
powershell -ExecutionPolicy Bypass -c "irm https://github.com/hridaya423/phonedriver/releases/latest/download/phonedriver-installer.ps1 | iex"
```

The shell and PowerShell installers put `phonedriver` in `$CARGO_HOME/bin` (`~/.cargo/bin`) and add it to your `PATH`. The iOS Simulator runner needs no download: its sources are inside the binary, and the first command that needs it builds it on your Mac with Xcode and XcodeGen.

To build from source instead, use Rust 1.99 or later.

```sh
cargo build --release
```

The build packs the Android agent, so it needs the Android SDK with build-tools **37.0.0** and platform **android-37.0** (`ANDROID_HOME`, falling back to `ANDROID_SDK_ROOT`, then `/opt/homebrew/share/android-commandlinetools`), `d8` and JDK 17. Override the versions with `ANDROID_BUILD_TOOLS` and `ANDROID_PLATFORM`. If anything is missing the build says exactly what and how to get it. To skip that step, set `PHONEDRIVER_AGENT_JAR` to a built `phonedriver-agent.jar` (`bash android-agent/build.sh` makes one; none is published yet).

**iOS only, no Android toolchain:**

```sh
cargo build --release -p phonedriver --no-default-features
```

That binary has no Android support (`phonedriver doctor` skips the Android checks). Run the tests with the default features. The binary is `target/release/phonedriver`.

```console
$ target/release/phonedriver version
phonedriver 0.1.0
```

Run `phonedriver doctor`. It checks adb, each Android device, the agent, UiAutomation, the video stream, Xcode, the simulators, and the permissions of the daemon socket. Each line says `ok`, `warn` or `fail`, and a fix follows every line that is not `ok`.

```console
$ phonedriver doctor
ok    adb server       0029 at 127.0.0.1:5037
warn  android devices  none connected
      fix: Connect a phone with USB debugging on, or start an emulator; `adb devices` lists what adb sees.
ok    xcode            Xcode 27.0, Build version 27A266a
ok    xcodegen         /opt/homebrew/bin/xcodegen
warn  simulators       none booted
      fix: Boot one in Simulator.app or with `xcrun simctl boot <udid>`; `xcrun simctl list devices available` lists them.
ok    daemon socket    /Users/you/.phonedriver/run/daemon.sock mode 600 in a directory of mode 700
```

The CLI runs the same loop. `phonedriver skill` prints [`SKILL.md`](../SKILL.md), the instructions an agent reads before it uses the tools.

```sh
phonedriver devices
phonedriver snapshot
phonedriver tap "Dark theme"
phonedriver wait settled
```

### iPhone or iPad

A physical iOS device needs the runner signed with your Apple developer team (a free Apple ID works and expires every 7 days). Plug the device in, unlock it, then:

```sh
phonedriver ios setup                  # pairing, Developer Mode, developer disk image
export PHONEDRIVER_TEAM=<your team ID>   # Xcode > Settings > Accounts, or: security find-identity -v -p codesigning
phonedriver --device ios:<udid> snapshot
```

Without a team, the command stops with exit code 3 and says so. With a booted Simulator and an iPhone that has no team, the Simulator is the default device. The daemon reads `PHONEDRIVER_TEAM` when it starts, so run `phonedriver daemon stop` after setting it.

## Environment variables

| Name | Default | Meaning |
|---|---|---|
| `PHONEDRIVER_HOME` | `~/.phonedriver` | Where the daemon keeps its socket (`run/`), `daemon.log`, `traces/`, `tmp/` and the iOS `runner/` builds. Set it to run a separate daemon. On macOS the socket path must stay under about 100 bytes. |
| `PHONEDRIVER_SOCKET` | under `PHONEDRIVER_HOME` (else `$XDG_RUNTIME_DIR/phonedriver`) | The daemon's socket or named pipe. Point it at an `ssh -L` forward to use a remote machine's devices. |
| `PHONEDRIVER_DEVICE` | none | The device to use when `--device` is not given. Also read by `phonedriver mcp`. |
| `PHONEDRIVER_SESSION` | `default` | The session name, as `--session`. |
| `PHONEDRIVER_TEAM` | none | Apple developer team ID that signs the iOS runner on a physical device. |
| `PHONEDRIVER_IOS_TIER` | `runner` | `coredevice` selects the CoreDevice tier on iOS 27 devices. Experimental. |
| `PHONEDRIVER_GUARD` | on | `off` stops phonedriver asking the human before a tap that spends money or cannot be undone (`Place order`, `Pay`, `Delete account`). Set it for `phonedriver mcp` or the CLI only when you grant the agent full autonomy. |
| `PHONEDRIVER_SECRET` | none | The text for `type --secret`, instead of stdin. |
| `PHONEDRIVER_TRACE_FULL` | off | `1` keeps observation text and text targets in traces (the daemon reads it at start). |
| `PHONEDRIVER_TRACE_SCREENS` | off | `1` copies every screenshot next to its trace file. |
| `PHONEDRIVER_AGENT_JAR` | built by `android-agent/build.sh` | Build time only: use this prebuilt Android agent jar. |
| `ADB_SERVER_SOCKET` | `tcp:127.0.0.1:5037` | The adb server to use, for example a tunnel to another machine. |
| `PHONEDRIVER_TEST_DEVICE`, `PHONEDRIVER_TEST_IOS_UDID`, `PHONEDRIVER_BENCH`, `PHONEDRIVER_BIN` | none | Tests and benchmarks only. |

The runner's own `PHONEDRIVER_PORT`, `PHONEDRIVER_BUILD` and `PHONEDRIVER_UDID` are set by phonedriver for the iOS runner and are not for you.

## Debugging

When something does not work:

- `phonedriver doctor` checks the toolchain, devices and the socket, and says how to fix each problem.
- `$PHONEDRIVER_HOME/daemon.log` (default `~/.phonedriver/daemon.log`) holds the daemon's own messages and startup errors.
- `$PHONEDRIVER_HOME/traces/<id>.jsonl` has one line per request: method, parameters, the reply's summary, `ms` and settle time. Typed text is recorded only as a length. Start the daemon with `PHONEDRIVER_TRACE_FULL=1` to keep observation text, and `PHONEDRIVER_TRACE_SCREENS=1` to keep screenshots. A running daemon keeps its options: `phonedriver daemon stop` first.
- iOS: `$PHONEDRIVER_HOME/runner/<udid>.log` is the runner's output, and `runner/<hash>/build.log` is its first build.
- `phonedriver daemon stop` stops the daemon and the iOS runners it started. The next command starts a new one.

Exit codes: `0` done, `1` the command failed, `2` the command line is wrong, `3` a person must act on the device (unlock, trust, allow), `4` no matching device or it was disconnected, `5` a tap that spends money or cannot be undone needs the human's approval (add `--autonomous` only when the user has granted full autonomy). On a terminal, a first command that takes more than a second says what it is doing (building the runner, starting the agent).

## Device on another machine, Wi-Fi and shell

### Use a device on another machine

The daemon listens on a Unix socket and nowhere else. To drive a simulator or phone that is plugged into another machine, forward that socket over `ssh` and point the CLI at the local end:

```sh
ssh -N -L /tmp/phonedriver-mini.sock:/Users/you/.phonedriver/run/daemon.sock mini &
export PHONEDRIVER_SOCKET=/tmp/phonedriver-mini.sock
phonedriver snapshot
```

On the other machine, run any `phonedriver` command once, or `phonedriver daemon`, so that the socket exists. Both machines need the same `phonedriver` version. If the forward is down, the error names `PHONEDRIVER_SOCKET`.

Two things differ from a local run. `phonedriver screenshot` prints a path on the machine that has the device; `phonedriver screenshot -o shot.png` writes the file on your machine instead. The MCP `screenshot` tool returns the image itself, so it works unchanged.

### Android over Wi-Fi

An Android 11 or later phone on the same network needs no cable. On the phone, turn on Developer options, then Wireless debugging. `phonedriver pair` shows a QR code in the terminal (the flow Android Studio uses):

```sh
phonedriver pair
```

On the phone choose Wireless debugging, then Pair device with QR code, and scan it. phonedriver finds the phone on the network, pairs, connects, and prints `connected: android:192.168.1.20:41233`. It waits 2 minutes for the scan. Exit code 3 means nobody scanned; 1 means adb's mDNS is unavailable or pairing failed, and the message says what to do. *Not yet tested on a physical phone.*

If the QR does not work (the phone and computer are on different networks, a guest network, or a router that blocks mDNS), pair with the code instead. On the phone choose Pair device with pairing code. Then:

```sh
adb pair 192.168.1.20:37099
adb connect 192.168.1.20:41233
phonedriver devices
```

The first address and the six-digit code are on the pairing screen; the second address is on the Wireless debugging screen (the port changes each time wireless debugging is switched on). The phone then appears as `android:192.168.1.20:41233`. If it shows `offline`, `phonedriver doctor` says what to check.

#### Away from your Wi-Fi

Wireless debugging works only on the Wi-Fi it was switched on for. To use a phone from anywhere you can reach it, for example over [Tailscale](https://tailscale.com), switch the phone to classic adb over TCP while it is connected by cable or already over Wi-Fi:

```sh
phonedriver untether
```

It lists the phone's addresses (`wlan0 (Wi-Fi)`, `tun0 (Tailscale)`), runs `adb tcpip 5555`, connects to the first address that answers and prints `connected: android:<ip>:5555`. With more than one device pass `--device android:<serial>`. *Not yet tested on a physical phone.*

This is opt-in on purpose. adb then listens on port 5555 on every network the phone joins, until it reboots. A new computer still has to be allowed on the phone's screen, but do this only on networks you trust, or with Tailscale on both ends and the phone's other networks untrusted. `adb -s <ip>:5555 usb` or a reboot turns it off. See decision D30.

### Shell

`phonedriver shell "cmd uimode night yes"` runs a command on the device and prints the exit code, the output (cut at 40 lines; `--grep`, `--head`) and what changed on screen. On Android it runs in the agent already on the device, not in a new `adb shell`. On an iOS Simulator it runs through `simctl spawn`, and `shell "simctl ui appearance dark"` fills in the device for `simctl` subcommands. A physical iPhone has no shell. A physical device refuses wipes and removing apps phonedriver did not install unless you pass `--force`. See `docs/bench/history/shell.md` and decision D21.

## Development

```sh
blaze build
BLAZE_TESTS=all blaze test
cargo clippy --all-targets -- -D warnings
```

Tests that need a device are ignored by default. Set `PHONEDRIVER_TEST_DEVICE` (for example `android:emulator-5554` or `sim:<udid>`) and pass `--ignored`. [`AGENTS.md`](AGENTS.md) holds the rules for working in this repository.

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

