---
title: How to run device commands with the phonedriver shell
nav: Shell
meta: Run a setting, intent or key on the device in one call with phonedriver shell and see the exit code, output and what changed on screen.
description: Run a setting, intent or key on the device in one call with phonedriver shell, and see the exit code, the output and what changed on screen. Android, iOS Simulator and physical device rules.
---

`phonedriver shell` runs one command on the device and prints the exit code, the output (cut at 40 lines; `--grep` and `--head` narrow it) and what changed on screen. For a setting or an intent it is one call, with no screens to tap through.

```sh
phonedriver shell "cmd uimode night yes"
phonedriver shell "settings put system screen_brightness 20"
phonedriver shell "am start -a android.intent.action.VIEW -d https://example.com"
phonedriver shell "dumpsys window | grep mCurrentFocus"
```

## Android

The command runs in the agent already on the device, over the open control stream, not in a new `adb shell`. After a command that changes state, phonedriver settles and returns the screen diff, as it does for any action.

## iOS Simulator

The command runs through `simctl spawn`. For `simctl` subcommands the device is filled in for you:

```sh
phonedriver shell "simctl ui appearance dark"
phonedriver shell "simctl openurl https://example.com"
```

## Physical devices

A physical iPhone has no shell. A physical Android device refuses wipes and removing apps phonedriver did not install, unless you pass `--force`. Pass it only when the person who owns the phone asked for exactly that.

## Limits

The [approval guard](/docs/approval-guard/) does not see a button pressed by shell input, so `input tap` through the shell skips it. Destructive device commands are guarded separately on physical devices. Design: decision D21 in [the repository]({{REPO}}/blob/main/docs/decisions.md), and measurements in `docs/bench/shell.md`.
