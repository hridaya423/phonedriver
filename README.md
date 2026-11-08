<p align="center"><picture><source media="(prefers-color-scheme: dark)" srcset=".github/assets/banner-dark.svg"><img alt="phonedriver" src=".github/assets/banner-light.svg" width="100%"></picture></p>

<h3 align="center">Give your AI agent a phone.</h3>

<p align="center">
  <a href="LICENSE"><img alt="License: AGPL-3.0" src="https://img.shields.io/badge/license-AGPL--3.0-6b675e?style=flat-square&labelColor=161512"></a>
  <a href="https://www.npmjs.com/package/phonedriver"><img alt="npm" src="https://img.shields.io/npm/v/phonedriver?style=flat-square&labelColor=161512&color=6b675e"></a>
</p>

phonedriver lets Claude Code, Codex, Cursor or any MCP client use an Android phone or an iPhone. The model opens apps, taps, types, scrolls and reads the screen. It reads the screen as text instead of screenshots: on an emulator, a 1,000-row screen comes back in 24 ms. After each tap it gets back only the lines that changed. And when it reaches a button labeled Pay or Place order, phonedriver stops and asks you.

<p align="center">
  <img alt="Claude Code drives an Android phone through phonedriver. It finds the best-rated Indian restaurant near Times Square on Uber Eats, adds butter chicken and garlic naan to the cart and stops at checkout. Then, on a test checkout screen, it taps Place order and phonedriver asks the human first. The human declines and nothing is ordered." src=".github/assets/demo.gif" width="100%">
</p>

<p align="center"><sub>Claude Code (Opus 5.5) with phonedriver on an Android emulator. The Uber Eats run took 6 minutes and is shown at 15x; the approval prompt plays at real speed. Nothing was ordered. <a href=".github/assets/demo.mp4">Full video (MP4)</a>.</sub></p>

## Start

```sh
claude mcp add phonedriver -- npx -y phonedriver mcp
```

Start an Android emulator, plug in an Android phone with USB debugging on, or boot the iOS Simulator. Then give Claude a job that only a phone can do:

> Open my app, sign up with a new account, and tell me every bug you find on the way. Give me steps to reproduce each one.

> Find the app that drains my battery and tell me how long it ran in the background today.

> Go through my app's onboarding in dark mode and in light mode, and show me every screen where the text is hard to read.

The first run downloads one binary for your machine (macOS, Linux or Windows) and caches it. `phonedriver doctor` checks adb, Xcode and each device and tells you what to fix. Codex, Cursor and other clients: see [Other clients](#other-clients). Homebrew, the shell installer and real iPhones: [docs/reference.md](docs/reference.md#install).

<!-- prelaunch -->
<sub>Not released yet. Until the first release, build from source with <code>cargo build --release</code>.</sub>
<!-- /prelaunch -->

## What the model sees

Each screen arrives as a short numbered list. This is the Settings app on an Android emulator:

```console
$ phonedriver snapshot
app com.android.settings (Settings) · keyboard hidden · settled in 0ms
@e1 button "Search Settings"
@e2 group scrollable↓
  @e3 button "Google" "Services & preferences"
  @e4 button "Network & internet" "Mobile, Wi‑Fi, hotspot"
  @e5 button "Connected devices" "Bluetooth, pairing"
  ...
  @e10 button "Display & touch" "Dark theme, font size, touch"
```

The model taps by number. phonedriver waits until the screen stops changing, then replies. After `tap @e10` the Display screen opens and the reply lists it, including `@e24 switch "Dark theme" off`. The next tap returns only what changed:

```console
$ phonedriver tap @e24
ok tapped @e24 "Dark theme" at 353,532 · settled in 2034ms
~ @e23 button "Dark theme" "Will turn on when Bedtime starts" → "Will never turn off automatically"
~ @e24 switch "Dark theme" off → on
```

`+` marks a line that appeared, `-` one that left, `~` one that changed. The model doesn't need to poll or guess how long a screen takes to load. It takes a screenshot only when it needs to see a picture.

## Speed

<p align="center">
  <picture><source media="(prefers-color-scheme: dark)" srcset=".github/assets/hero-dark.svg"><img alt="phonedriver against adb on an Android emulator, median of 50 runs. Reading the screen: 24 ms against 2,352 ms, 98 times faster. Screenshot: 12 ms against 476 ms, 41 times faster. Typing 100 characters: 251 ms against 4,841 ms, 19 times faster. Tapping and seeing what changed: 587 ms against 2,726 ms, 4.6 times faster." src=".github/assets/hero-light.svg" width="100%"></picture>
</p>

Measured on an Android 17 emulator, median of 50 runs, against `adb shell uiautomator dump`, `screencap` and `input`. The difference comes from a small helper process that phonedriver starts on the phone. It keeps the accessibility connection open, where adb starts a new process and writes XML each time. [Method and raw data.](docs/bench/speed.md)

## Asking before it pays

When a tap lands on a button labeled Pay, Buy now, Place order, Subscribe, Transfer or Delete account, phonedriver holds the tap and asks you in Claude Code's own prompt. The model never sees that question and can't answer it. In a client that can't show such a prompt, the tap waits and the model has to ask you in chat.

<p align="center"><img alt="Claude Code shows phonedriver's question: your agent wants to tap Place order, $12.00, in the testbed app. The human declined, and Claude reports that nothing was submitted." src=".github/assets/demo-poster.png" width="100%"></p>

You choose Allow, Allow for this session, or Decline. If you want an agent to run without asking, you can say so in its instructions, and it passes `autonomous: true`; you can also turn the check off with `PHONEDRIVER_GUARD=off`. The check reads button labels in English, Spanish, Portuguese, French, German, Italian, Dutch, Japanese, Chinese (Simplified), Korean and Hindi. It does not catch a payment button that is only an icon, or one labeled in another language.

## Real jobs

I tested phonedriver on nine jobs done in apps' own screens. They included setting Chrome's search engine, cropping a photo, pinning and archiving Messages threads, finding the app that keeps the phone awake, and testing a sign-up flow for planted bugs. Each job ran twice with Claude Sonnet 5.5. A script checked the phone's own data afterwards, not the model's report. All 18 runs passed. This is my own test set, published with every run: [the jobs and the checks](docs/bench/jobs.md).

## Works with

| Phone | You need |
|---|---|
| Android emulator | adb |
| Android phone or tablet | adb and USB debugging |
| iOS Simulator | a Mac with Xcode |
| iPhone or iPad | a Mac with Xcode and an Apple developer team ([setup](docs/reference.md#iphone-or-ipad)) |

On Android there is no app to install. phonedriver pushes its helper over adb when it connects. On iOS it builds a small XCTest runner on your Mac.

The MCP server has 14 tools: `devices`, `apps`, `snapshot`, `screenshot`, `tap`, `type`, `swipe`, `scroll`, `press`, `launch`, `wait`, `batch`, `clipboard` and `shell`. The `phonedriver` CLI has the same commands, for scripts and for agents that prefer a shell.

## Limits

- Tested on Android emulators (API 26 and 37), the iOS Simulator, an iPhone 14 Plus and an iPad Pro. Real Android phones use the same path but have had little testing. Bug reports from real phones are welcome.
- A tap takes 70 to 120 ms on an Android emulator and 450 to 530 ms on a real iPhone.
- The model can do anything on the phone you could do with your finger. For unattended runs, use an emulator or a spare phone.
- phonedriver never enters a passcode and can't pass Face ID or a fingerprint check. Screens that block screenshots, as some banking apps do, come out black in a screenshot, though their text can still be read.

## Other clients

```json
{
  "mcpServers": {
    "phonedriver": { "command": "npx", "args": ["-y", "phonedriver", "mcp"] }
  }
}
```

With more than one device connected, add `"env": { "PHONEDRIVER_DEVICE": "android:emulator-5554" }`. `phonedriver devices` lists the ids.

## How it works

On Android, phonedriver starts a small Java helper on the phone over adb. The helper reads the accessibility tree through UiAutomation and streams the screen as H.264 video, which is how phonedriver knows when the screen has stopped changing. On iOS, an XCTest runner does the same job. A daemon on your computer keeps the connection open and remembers which `@e` number is which element, so a tap still lands after a list has moved. The MCP server and the CLI are thin clients of that daemon.

phonedriver sends no telemetry. After the first download from GitHub, it connects only to your device, and a test runs the whole tool in a sandbox that blocks every other connection. Design notes: [architecture](docs/architecture.md), [protocol](docs/protocol.md), [decisions](docs/decisions.md).

## FAQ

**How do I let Claude control my Android phone?** Run `claude mcp add phonedriver -- npx -y phonedriver mcp`, turn on USB debugging, and plug the phone in.

**Does it work with an iPhone?** Yes, from a Mac with Xcode, on the Simulator or on an iPhone or iPad signed with your Apple developer team.

**Can my agent buy things with it?** Not without your answer, unless you have told it to run autonomously. See [Asking before it pays](#asking-before-it-pays).

**Can I use it at work?** Yes. It is AGPL-3.0: you can use it freely. If you change it and offer it to others as a network service, you must publish your changes.

## License

AGPL-3.0-only. See [LICENSE](LICENSE). The Android helper includes files from scrcpy under Apache-2.0; see [NOTICE](NOTICE).
