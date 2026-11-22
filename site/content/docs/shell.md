---
title: How to run device commands with the phonedriver shell
nav: Shell
meta: Run a setting, intent, or dumpsys command in one call with phonedriver shell and get back exit code, output, and the screen diff.
description: Toggle settings, fire Android intents, or run simctl in one call—returning exit code, trimmed output, and what changed on screen.
updated: 2026-11-21
---

Sometimes tapping through four menus to flip dark mode is silly when one command does it in 200 ms. `phonedriver shell` runs a command on the device and hands back its exit code, output (capped at 40 lines; narrow with `--grep` or `--head`), and the settled screen diff:

```sh
phonedriver shell "cmd uimode night yes"
phonedriver shell "settings put system screen_brightness 20"
phonedriver shell "am start -a android.intent.action.VIEW -d https://example.com"
phonedriver shell "dumpsys window | grep mCurrentFocus"
```

## Android

Commands run inside the helper already alive on the phone over the open control stream—no fresh `adb shell` spawn. Whenever a command changes device state, phonedriver settles and appends the screen diff just like a tap.

On an emulator, a command that starts with `emu` goes to the emulator's console: `geo`, `sms`, `power`, `network speed`, `delay` and `status`, `sensor`, `rotate`, `finger`, `avd name` and `status`. The other console commands, some of which write files on your computer or end the emulator, need `PHONEDRIVER_HOST_SHELL=1` (below). A line break in a console command is always refused.

## iOS Simulator

The active Simulator's UDID is filled in automatically for `simctl` subcommands:

```sh
phonedriver shell "simctl ui appearance dark"
phonedriver shell "simctl openurl https://example.com"
```

The Simulator's own settings and log work with nothing to set up:

```sh
phonedriver shell "defaults read -g AppleLocale"
phonedriver shell "defaults write com.example.app Server -string staging"
phonedriver shell "log show --last 1m --predicate 'process == \"MyApp\"'"
```

Each runs as one program in the Simulator, so pipes, `;` and redirection are refused, and so is an argument that names a file (one that starts with `/` or `~`).

A Simulator's programs run on your Mac as your Mac user and can read and write your Mac's files. So other programs need the switch below, and of `simctl` only the forms that act on the Simulator alone run: `ui`, `openurl` (not a `file:` URL), `privacy`, `location`, `status_bar`, `launch`, `terminate`, `uninstall`, `listapps`, `appinfo`, `get_app_container`, `pbcopy`, `pbpaste`, `keychain reset` and `io enumerate`. Another program in the Simulator, `simctl spawn`, and the forms that read or write a Mac file (`install`, `addmedia`, `push`, `io screenshot`, `io recordVideo`, `keychain add-cert`, `launch --stdout`) are refused. The verbs `install`, `apps notify`, `screenshot` and `record` do those jobs.

With `PHONEDRIVER_HOST_SHELL=1` (below), any command runs inside the Simulator through `simctl spawn`, through its shell where it has one. iOS 26 Simulators have no shell, so a command there is one of the Simulator's own programs and its arguments, with no pipes, `;`, redirection or `$`:

```sh
phonedriver shell "launchctl list"
```

A command that needs a shell, or a program the Simulator does not have (`ls`, `grep`), is refused with a list of what it has.

## Let the shell act on your computer

`PHONEDRIVER_HOST_SHELL=1` in the environment the daemon starts with turns on what the sections above say is refused: any program and every listed `simctl` form on a Simulator, and every console command on an emulator. Set it only for an agent you would let run commands on your computer. A daemon that is already running keeps what it started with, so run `phonedriver daemon stop` first. With an MCP client, put it in the server's `env`.

## Physical devices

- **iPhone / iPad:** No device shell is available on physical iOS devices.
- **Android:** Destructive commands (wiping app data or uninstalling apps phonedriver didn't install) are blocked unless you pass `--force`—only do that when the owner explicitly asked.

## One safety note

The [approval guard](/docs/approval-guard/) inspects taps, not shell commands, so raw `input tap` via `shell` bypasses it. Destructive device commands are guarded separately on physical devices. Design notes live in decisions D21 and D108 in [the repository]({{REPO}}/blob/main/docs/decisions.md); measurements in `docs/bench/history/shell.md`.
