---
title: How to control the iOS Simulator with phonedriver
nav: iOS Simulator
meta: Boot an iPhone Simulator on a Mac and let an agent drive it with phonedriver. Needs Xcode. Covers the runner build and tap latency.
description: Boot an iPhone Simulator on a Mac and let an agent drive it with phonedriver. Needs Xcode. Covers the first-run runner build and the Simulator's tap latency.
seo: How to let an AI agent control the iOS Simulator over MCP
updated: 2026-11-20
---

To let an AI agent control the iOS Simulator, boot a Simulator on a Mac with Xcode and add phonedriver to your agent as an MCP server. The agent reads the screen as text through an XCTest runner that phonedriver builds for you, and its taps go to the Simulator's own touch input.

The iOS Simulator is a tested target: the 12-scenario suite passes on iOS 26.5, three runs in a row. One scenario, `settings-dark-appearance`, failed twice in six full runs and I have not found the cause.

## What you need

A Mac with Xcode 15 or later (I use Xcode 27). There is no Android toolchain involved: the iOS-only build (`--no-default-features`) skips it.

## Boot a Simulator

`phonedriver boot` starts a Simulator that is off and waits until it has booted:

```sh
phonedriver boot "iPhone 17"
```

You can also boot one in Simulator.app, or with `simctl`:

```sh
xcrun simctl list devices available
xcrun simctl boot <udid>
```

Then:

```sh
phonedriver devices
phonedriver snapshot
```

The first command that needs the runner builds a small XCTest runner on your Mac. The sources are inside the phonedriver binary, so nothing is downloaded. The command says what it is doing if it takes more than a second. The build log is at `~/.phonedriver/runner/<hash>/build.log` and the runner's output at `~/.phonedriver/runner/<udid>.log`.

## Use the shell

`phonedriver shell "simctl ui appearance dark"` fills in the device for `simctl` subcommands that act on the Simulator alone, such as `ui`, `openurl`, `privacy`, `location` and `status_bar`.

A Simulator's programs run on your Mac as your Mac user, and they can read and write your Mac's files. So by default `shell` runs no program in the Simulator and refuses the `simctl` forms that touch a Mac file: `spawn`, `install`, `addmedia`, `push`, `io screenshot`, `keychain add-cert`. The verbs `install`, `apps notify`, `screenshot` and `record` do those jobs. To allow the rest, stop the daemon with `phonedriver daemon stop` and start it again with `PHONEDRIVER_HOST_SHELL=1` set, for example in the MCP server's `env`. Then a command such as `defaults read -g AppleLocale` runs inside the Simulator. iOS 26 Simulators have no shell, so a command is one of their own programs, such as `defaults` or `log`. See [the shell](/docs/shell/).

## Push notifications and logs

`phonedriver apps notify <app> '{"aps":{"alert":"Hi"}}'` (or `--file payload.json`) sends the app a push notification, which the Simulator delivers the way Apple's push service would. The app must have asked for notification permission, and a payload without an `aps` object is refused.

`phonedriver wait log:"signed in|ready" --app <app>` follows the Simulator's log from that moment and returns the first line with one of the words. The line is masked like every reply. See [testing your app](/docs/test-a-mobile-app/).

## Speed

Since 0.1.5 a one-finger tap goes to the Simulator's own touch input, and phonedriver watches the Simulator's screen as it is drawn to know when it has settled. On simulator-use's own benchmark a tap with its reply takes 316 ms and a read 29 ms, the fastest of four tools ([the benchmark](/benchmarks/#simulator)). Typing still goes through XCTest, which types exactly, and so do taps with several fingers, in an app on its side and under a notification banner.

## If it does not work

Run `phonedriver doctor`. It checks Xcode and the booted simulators. With a booted Simulator and an iPhone that has no signing team, the Simulator is the default device. More in [troubleshooting](/docs/troubleshooting/).

## Questions

### Can Claude Code control the iOS Simulator?

Yes. Boot a Simulator, run `claude mcp add phonedriver -- npx -y phonedriver mcp`, and ask. Claude Code gets the screen as a numbered text list and taps by number. See [the Claude Code guide](/docs/claude-code/).

### Do I need idb or WebDriverAgent?

No. phonedriver builds its own XCTest runner with Xcode the first time it is needed. Its sources are inside the phonedriver binary, so nothing is downloaded.

### Can I install my app on the Simulator with it?

Yes. `phonedriver apps install` takes the `.app` folder your build produces, and `phonedriver launch` opens it. See [testing your app](/docs/test-a-mobile-app/).
