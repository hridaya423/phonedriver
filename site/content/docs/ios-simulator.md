---
title: How to control the iOS Simulator with phonedriver
nav: iOS Simulator
meta: Boot an iPhone Simulator on a Mac and let an agent drive it with phonedriver. Needs Xcode and XcodeGen. Covers the runner build and tap latency.
description: Boot an iPhone Simulator on a Mac and let an agent drive it with phonedriver. Needs Xcode and XcodeGen. Covers the first-run runner build and the Simulator's tap latency.
---

The iOS Simulator is a tested target: the 12-scenario suite passes on iOS 26.5, three runs in a row. One scenario, `settings-dark-appearance`, failed twice in six full runs and we have not found the cause.

## What you need

A Mac with Xcode (we use Xcode 27) and XcodeGen (`brew install xcodegen`). There is no Android toolchain involved: the iOS-only build (`--no-default-features`) skips it.

## Boot a Simulator

Boot one in Simulator.app, or:

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

`phonedriver shell "simctl ui appearance dark"` runs through `simctl spawn`, and fills in the device for `simctl` subcommands. See [the shell](/docs/shell/).

## Speed

A tap on the iOS 26.5 Simulator takes 357 ms, against 80 ms for the Simulator's own tool, because XCTest takes about 240 ms to confirm the event (decision D15). Screenshots and the settled diff are within budget. If speed on iOS matters, read [the benchmarks](/benchmarks/) before you decide.

## If it does not work

Run `phonedriver doctor`. It checks Xcode, XcodeGen and the booted simulators. With a booted Simulator and an iPhone that has no signing team, the Simulator is the default device. More in [troubleshooting](/docs/troubleshooting/).
