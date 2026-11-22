---
title: How to control the iOS Simulator with phonedriver
nav: iOS Simulator
meta: Boot an iPhone Simulator on a Mac with Xcode and let an AI agent drive it with phonedriver—native Simulator input, display-buffer settling, and app installs.
description: Boot an iOS Simulator on a Mac with Xcode and hand it to your agent. Covers the auto-built runner, 0.1.5 native Simulator input, and simctl shell shortcuts.
seo: How to let an AI agent control the iOS Simulator over MCP
updated: 2026-11-21
---

Boot an iOS Simulator on a Mac with Xcode, wire up phonedriver, and your agent can drive iOS apps immediately—no `idb`, no WebDriverAgent setup, no separate downloads. It reads the screen as text through an XCTest runner that phonedriver builds for you, and its taps go to the Simulator's own touch input.

The 12-scenario iOS suite passes on iOS 26.5. (One scenario, `settings-dark-appearance`, flaked twice across six full runs; I haven't pinned down why yet.)

## What you need

A Mac with Xcode 15 or later (tested on Xcode 27). No Android SDK is required; building from source with `--no-default-features` skips Android altogether.

## Boot and connect

`phonedriver boot` starts a Simulator that is off and waits until it has booted:

```sh
phonedriver boot "iPhone 17"
```

You can also start one from Simulator.app, or via `simctl`:

```sh
xcrun simctl list devices available
xcrun simctl boot <udid>
```

Then check it:

```sh
phonedriver devices
phonedriver snapshot
```

On first use, phonedriver compiles its own small XCTest runner from sources embedded right inside the binary (zero network downloads). If the first build takes more than a second, it tells you what it's doing; logs live at `~/.phonedriver/runner/<hash>/build.log` and `~/.phonedriver/runner/<udid>.log`.

## Shell shortcuts

`phonedriver shell` fills in the target Simulator for `simctl` subcommands that act on the Simulator alone, such as `ui`, `openurl`, `privacy`, `location`, and `status_bar`:

```sh
phonedriver shell "simctl ui appearance dark"
```

The Simulator's own settings and log work with nothing to set up: `defaults read -g AppleLocale`, `defaults write ...`, and `log show --last 1m` run as one program in the Simulator, as long as no argument names a file.

A Simulator's programs run on your Mac as your Mac user and can read and write your Mac's files, so anything else—other programs, and the `simctl` forms that touch a Mac file (`spawn`, `install`, `addmedia`, `push`, `io screenshot`, `keychain add-cert`)—is refused by default. The verbs `install`, `apps notify`, `screenshot`, and `record` do those jobs. To allow the rest, run `phonedriver daemon stop` and restart with `PHONEDRIVER_HOST_SHELL=1` (for example in the MCP server's `env`); then any command, like `launchctl list`, runs inside the Simulator. The same switch allows `open` with a `file:` link. An app you `install` also runs as your Mac user, so install only builds you trust. See [the shell guide](/docs/shell/) for more.

## Push notifications and logs

`phonedriver apps notify <app> '{"aps":{"alert":"Hi"}}'` (or `--file payload.json`) sends the app a push notification, which the Simulator delivers the way Apple's push service would. The app must have asked for notification permission, and a payload without an `aps` object is refused.

`phonedriver wait log:"signed in|ready" --app <app>` follows the Simulator's log from that moment and returns the first line with one of the words. The line is masked like every reply. See [testing your app](/docs/test-a-mobile-app/).

## Speed

As of 0.1.5, single-finger gestures use native Simulator input and settle directly off the Simulator's display buffer, skipping XCTest's ~240 ms event confirmation. On simulator-use's own benchmark a tap with its reply takes **316 ms** and a read **29 ms**, the fastest of four tools ([the benchmark](/benchmarks/#simulator)). Typing, multi-finger taps, apps on their side and taps under a notification banner still go through the XCTest runner, which is exact ([D93]({{REPO}}/blob/main/docs/decisions.md)).

## Troubleshooting

Run `phonedriver doctor` to check Xcode and booted Simulators. If both a Simulator and an unsigned physical iPhone are attached, phonedriver defaults to the Simulator. More in [troubleshooting](/docs/troubleshooting/).

## Questions

### Can Claude Code control the iOS Simulator?

Yes. Boot a Simulator, run `npx -y phonedriver init` (or `claude mcp add phonedriver -- npx -y phonedriver mcp`), and ask Claude in plain English ([Claude Code guide](/docs/claude-code/)).

### Do I need idb or WebDriverAgent?

No. phonedriver compiles its own minimal XCTest runner locally with Xcode on first use, straight from sources packed inside the binary.

### Can I install my app on the Simulator with it?

Yes—`phonedriver apps install YourApp.app` installs the `.app` bundle your Xcode build produces, and `phonedriver launch` opens it ([testing your app](/docs/test-a-mobile-app/)).
