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

On an emulator, a command that starts with `emu` goes to the emulator's console: `geo`, `sms`, `power`, `network speed`, `delay` and `status`, `sensor`, `rotate`, `finger`, `avd name` and `status`. The other console commands, some of which write files on your computer or end the emulator, need `PHONEDRIVER_HOST_SHELL=1` (below). A line break in a console command is always refused.

## iOS Simulator

For `simctl` subcommands the device is filled in for you:

```sh
phonedriver shell "simctl ui appearance dark"
phonedriver shell "simctl openurl https://example.com"
```

A Simulator's programs run on your Mac as your Mac user and can read and write your Mac's files. So by default only the `simctl` forms that act on the Simulator alone run: `ui`, `openurl` (not a `file:` URL), `privacy`, `location`, `status_bar`, `launch`, `terminate`, `uninstall`, `listapps`, `appinfo`, `get_app_container`, `pbcopy`, `pbpaste`, `keychain reset` and `io enumerate`. A program in the Simulator, `simctl spawn`, and the forms that read or write a Mac file (`install`, `addmedia`, `push`, `io screenshot`, `io recordVideo`, `keychain add-cert`, `launch --stdout`) are refused. The verbs `install`, `apps notify`, `screenshot` and `record` do those jobs.

With `PHONEDRIVER_HOST_SHELL=1` (below), anything else runs inside the Simulator through `simctl spawn`. iOS 26 Simulators have no shell, so a command there is one of the Simulator's own programs and its arguments, with no pipes, `;`, redirection or `$`:

```sh
phonedriver shell "defaults read -g AppleLocale"
phonedriver shell "log show --last 1m --predicate 'process == \"MyApp\"'"
```

A command that needs a shell, or a program the Simulator does not have (`ls`, `grep`), is refused with a list of what it has.

## Let the shell act on your computer

`PHONEDRIVER_HOST_SHELL=1` in the environment the daemon starts with turns on what the sections above say is refused: programs and every listed `simctl` form on a Simulator, and every console command on an emulator. Set it only for an agent you would let run commands on your computer. A daemon that is already running keeps what it started with, so run `phonedriver daemon stop` first. With an MCP client, put it in the server's `env`.

## Physical devices

A physical iPhone has no shell. A physical Android device refuses wipes and removing apps phonedriver did not install, unless you pass `--force`. Pass it only when the person who owns the phone asked for exactly that.

## Limits

The [approval guard](/docs/approval-guard/) does not see a button pressed by shell input, so `input tap` through the shell skips it. Destructive device commands are guarded separately on physical devices. Design: decisions D21 and D108 in [the repository]({{REPO}}/blob/main/docs/decisions.md), and measurements in `docs/bench/history/shell.md`.
