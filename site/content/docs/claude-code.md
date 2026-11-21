---
title: How to use phonedriver with Claude Code
seo: How to let Claude Code control an Android phone or iPhone
nav: Claude Code
description: Add phonedriver to Claude Code in one command, connect a phone or simulator, and ask it to drive apps in plain English.
updated: 2026-11-20
---

Claude Code is phonedriver's home turf—every benchmark and demo on this site runs through it. Wire it up with `npx -y phonedriver init` (or `claude mcp add` below), connect a phone, and talk to it normally.

## Add the server

With Node 18+ and nothing pre-installed:

```sh
claude mcp add phonedriver -- npx -y phonedriver mcp
claude mcp list    # expect: phonedriver ... Connected
```

The first start downloads and caches the binary once ([Install](/docs/install/)). If you [installed phonedriver permanently](/docs/install/#permanent-install), drop `npx -y` and pass `phonedriver mcp`.

With a single device attached, phonedriver picks it automatically. With several devices plugged in, pin the one you want (`phonedriver devices` lists IDs):

```sh
claude mcp add --env PHONEDRIVER_DEVICE=android:emulator-5554 phonedriver -- npx -y phonedriver mcp
```

## An app you are developing

On a physical Android phone, `apps clear`, `uninstall`, `grant` and `revoke` touch only apps phonedriver installed, so an app that Gradle or Android Studio put there is refused. Name your own apps in the server's environment, which the agent cannot change:

```sh
claude mcp add --env PHONEDRIVER_APPS_UNDER_TEST=com.example.app,com.example.other phonedriver -- npx -y phonedriver mcp
```

To keep a phone out of the agent's reach on a shared adb server, add `--env PHONEDRIVER_DENY_DEVICES=android:<id>` the same way (or write `deny android:<id>` in `~/.phonedriver/devices`, which every agent on the machine obeys). A denied device is not listed, not probed by `doctor` and never gets the agent.

The daemon reads the list when it starts, so run `phonedriver daemon stop` once if one is already running. Emulators and Simulators need none of this.

## Keep the agent in your app

On a phone that is signed in to real accounts, name the apps the agent may use. It then sees nothing from any other app, and its taps, typing, screenshots and `shell` are refused while another app is on screen. The home screen counts as another app.

```sh
claude mcp add --env PHONEDRIVER_APP_SCOPE=com.example.app phonedriver -- npx -y phonedriver mcp
```

This is not `PHONEDRIVER_APPS_UNDER_TEST`, which only lets the agent clear or uninstall an app of yours. The scope is read when the daemon starts, so run `phonedriver daemon stop` once if one is running, and the agent cannot change it. It is not a sandbox: [what it covers and what it does not](/docs/android-phone/#keep-the-agent-in-your-app).

## Or install the plugin

The plugin bundles both the MCP server and the phonedriver skill so Claude gets the full playbook automatically. Remove any manual `mcp add` registration first (`claude mcp remove phonedriver`) so the tools aren't duplicated.

```sh
claude plugin marketplace add hridaya423/phonedriver
claude plugin install phonedriver@phonedriver
```

Inside a running session, `/plugin marketplace add hridaya423/phonedriver` and `/plugin install phonedriver@phonedriver` do the same thing. To pick a specific device with the plugin, export `PHONEDRIVER_DEVICE` before launching Claude Code.

## Try it out

Boot an emulator or Simulator (or plug in a phone) and ask:

> Open Settings and turn on dark mode.

Claude calls `launch`, `swipe`, and `tap` as needed. Each action waits for the screen to settle and hands back only what changed. On the iOS Simulator, flipping the switch returns:

```screen
ok tapped @e41 "Dark Appearance"
- @e65 @e66 @e67
~ @e41 switch "Dark Appearance" off → on
+ @e68 button "L4S"
```

`+` appeared, `-` left, `~` changed. No sleep loops, no polling, no screenshot spam.

## What Claude gets

Eighteen tools (`devices`, `apps`, `snapshot`, `screenshot`, `tap`, `type`, `swipe`, `scroll`, `press`, `launch`, `wait`, `batch`, `clipboard`, `shell`, `setting`, `logs`, `network`, and `replay`), all returning the exact same text as the CLI. `phonedriver skill` prints the full `SKILL.md` guide; the MCP server ships a compact version built in.

## Approvals and passwords

- **Payments & deletions:** Taps on `Place order`, `Pay`, `Buy with GPay`, or `Delete account` pause and pop a prompt straight inside Claude Code showing the app, button, and totals. Claude never sees or answers that form ([approval guard](/docs/approval-guard/)). In headless `claude -p`, nobody is there to click Allow, so guarded taps stay held.
- **Secrets:** Plain `type` into a password or one-time code field is refused on the spot. The error tells Claude to ask you and pass it via `secret: true`, keeping the value out of replies, logs, and traces.

## Questions

### Can Claude Code control an iPhone?

Yes, from a Mac with Xcode. Boot the iOS Simulator and it works immediately; for a physical iPhone or iPad, plug it in and run `phonedriver ios setup` once, then `phonedriver ios wifi on` if you want to drop the cable ([iPhone guide](/docs/iphone/)).

### Does Claude Code need screenshots to use the phone?

No. It sees a short numbered text list, taps by number, and gets a small diff after each action. Screenshots stay available whenever it genuinely needs to look at an image, a map, or a custom canvas.

### How many tokens does phonedriver add to Claude Code?

When the server had 14 tools, they added ~5,606 tokens on turn one of a bare `claude -p`; it now has 18. Across nine real Android phone jobs with Claude Sonnet 5.5, phonedriver summed to 657,542 tokens—6.9x fewer than mobile-mcp ([benchmarks](/benchmarks/#jobs)).

### Does it work in a headless claude -p run?

Yes—every benchmark on this site ran that way. Since nobody is at the terminal to answer approval prompts in headless mode, payment taps are safely held and nothing is tapped.

## Next

Connect an [Android emulator](/docs/android-emulator/), a [real Android phone](/docs/android-phone/), the [iOS Simulator](/docs/ios-simulator/), or an [iPhone](/docs/iphone/). Then try [testing your own app](/docs/test-a-mobile-app/), or see how it stacks up against [mobile-mcp](/compare/mobile-mcp/) and [adb](/compare/adb/).
