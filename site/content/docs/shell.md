---
title: How to run device commands with the phonedriver shell
nav: Shell
meta: Run a setting, intent, or dumpsys command in one call with phonedriver shell and get back exit code, output, and the screen diff.
description: Toggle settings, fire Android intents, or run simctl in one call—returning exit code, trimmed output, and what changed on screen.
updated: 2026-11-20
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

## iOS Simulator

Commands run through `simctl spawn`, with the active Simulator UDID filled in automatically for `simctl` subcommands:

```sh
phonedriver shell "simctl ui appearance dark"
phonedriver shell "simctl openurl https://example.com"
```

Anything else runs inside the Simulator through `simctl spawn`. iOS 26 Simulators have no shell, so a command there is one of the Simulator's own programs and its arguments, with no pipes, `;`, redirection or `$`:

```sh
phonedriver shell "defaults read -g AppleLocale"
phonedriver shell "log show --last 1m --predicate 'process == \"MyApp\"'"
```

A command that needs a shell, or a program the Simulator does not have (`ls`, `grep`), is refused with a list of what it has.

## Physical devices

- **iPhone / iPad:** No device shell is available on physical iOS devices.
- **Android:** Destructive commands (wiping app data or uninstalling apps phonedriver didn't install) are blocked unless you pass `--force`—only do that when the owner explicitly asked.

## One safety note

The [approval guard](/docs/approval-guard/) inspects `tap`, not shell commands, so raw `input tap` via `shell` bypasses it. Full design notes live in [Decision D21]({{REPO}}/blob/main/docs/decisions.md).
