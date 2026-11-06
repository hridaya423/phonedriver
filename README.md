# phone-use

phone-use is one Rust binary that lets an AI agent see and operate Android and iOS devices through a CLI or an MCP server.

```console
$ phone-use snapshot
device android:emulator-5554 · Pixel 9 · Android 17 · 412x915pt · portrait
app com.android.settings (Settings) · keyboard hidden · settled in 212ms
@e1 searchbox "Search settings"
@e6 button "Display & touch" "Dark theme, font size, brightness"

$ phone-use tap @e6
ok tapped @e6 "Display & touch" at 206,488 · settled in 340ms · new screen
@e10 switch "Dark theme" off

$ phone-use tap @e10
ok tapped @e10 "Dark theme" · settled in 410ms
~ @e10 switch "Dark theme" off → on
```

The agent reads the screen as text, acts on refs such as `@e6`, and gets back what changed after the screen settled. It does not poll or sleep. This example shows the design (`docs/architecture.md`, section 1), shortened.

## Install

phone-use is not published. Build it from source with Rust 1.99 or later.

```sh
cargo build --release
```

The build packs the Android agent, so it needs the Android SDK (`ANDROID_HOME` with build-tools and platform 37), `d8`, and JDK 17. To skip that step, set `PHONE_USE_AGENT_JAR` to a built `phone-use-agent.jar`. The binary is `target/release/phone-use`.

Then run `phone-use doctor`. It checks adb, each Android device, the agent, UiAutomation, the video stream, Xcode, the simulators, and the permissions of the daemon socket. Each line says `ok`, `warn` or `fail`, and a fix follows every line that is not `ok`.

## Supported devices

| Device | Status | Needs |
|---|---|---|
| Android emulator, API 26 and 37 | Tested. The 20-scenario suite passes on API 26 (M2). API 37 is open. | adb |
| Android phone or tablet | Built on the same path. Not tested on hardware. | adb, USB debugging on |
| iOS Simulator (iOS 26.5) | Tested. The 12-scenario suite passes (M4). | A Mac with Xcode 27 and XcodeGen |
| iOS device | Not built yet. | |

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

[`SKILL.md`](SKILL.md) tells an agent how to use the tools: the loop, targets, waiting, errors that need the user, and safety rules. `phone-use skill` prints it.

Typed secrets (`phone-use type --secret`, or `secret: true` over MCP) never reach logs, traces or replies. phone-use sends no telemetry. It talks to the device and nothing else.

## Speed

phone-use against phone-harness 0.3.3 on an Android 17 emulator, median of 50 runs (`docs/bench/m1.md`).

| Operation | phone-use | phone-harness | Factor |
|---|---|---|---|
| tap | 36.5 ms | 110.7 ms | 3.0x |
| screenshot | 15.2 ms | 390.1 ms | 26x |
| snapshot, Settings | 67.7 ms | 2426.4 ms | 36x |
| snapshot, 1,000-row list | 15.5 ms | 2370.0 ms | 153x |
| type 100 characters | 288.3 ms | 2104.5 ms | 7.3x |
| tap, then the settled diff | 419.5 to 544.0 ms | not supported | |

Each phone-use figure is the slower of two runs. phone-harness cannot type Unicode through `adb input text`; phone-use typed mixed-script text in the row above. Two of five latency budgets are missed, tap and type; the file says why.

## Documentation

- [`docs/architecture.md`](docs/architecture.md): the design.
- [`docs/decisions.md`](docs/decisions.md): why each choice beat its alternatives.
- [`docs/protocol.md`](docs/protocol.md): every wire format.
- [`docs/bench/`](docs/bench): the M1, M2 and M4 results.
- [`progress.md`](progress.md): what was built, with the evidence.

## Development

```sh
blaze build
BLAZE_TESTS=all blaze test
cargo clippy --all-targets -- -D warnings
```

Tests that need a device are ignored by default. Set `PHONE_USE_TEST_DEVICE` (for example `android:emulator-5554` or `sim:<udid>`) and pass `--ignored`. `AGENTS.md` holds the rules for working in this repository.

## License

Not chosen yet. The recommendation is Apache-2.0 (decision D13).
