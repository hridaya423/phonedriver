# Reference

Moved out of the README. Commands use the current binary name, `phonedriver`.

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

An Android 11 or later phone on the same network needs no cable. On the phone, turn on Developer options, then Wireless debugging, and choose Pair device with pairing code. Then:

```sh
adb pair 192.168.1.20:37099
adb connect 192.168.1.20:41233
phonedriver devices
```

The first address and the six-digit code are on the pairing screen; the second address is on the Wireless debugging screen (the port changes each time wireless debugging is switched on). The phone then appears as `android:192.168.1.20:41233`. If it shows `offline`, `phonedriver doctor` says what to check.

### Shell

`phonedriver shell "cmd uimode night yes"` runs a command on the device and prints the exit code, the output (cut at 40 lines; `--grep`, `--head`) and what changed on screen. On Android it runs in the agent already on the device, not in a new `adb shell`. On an iOS Simulator it runs through `simctl spawn`, and `shell "simctl ui appearance dark"` fills in the device for `simctl` subcommands. A physical iPhone has no shell. A physical device refuses wipes and removing apps phonedriver did not install unless you pass `--force`. See `docs/bench/shell.md` and decision D21.

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

