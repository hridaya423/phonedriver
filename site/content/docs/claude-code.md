---
title: How to use phonedriver with Claude Code
nav: Claude Code
description: Add phonedriver to Claude Code as an MCP server with one command, point it at a device and ask it to operate a phone in plain language.
---

Claude Code is the client phonedriver is tested against: the benchmarks on this site run `claude -p` with Claude Sonnet 5.5.

## Add the server

From any directory, with Node 18 or later and nothing installed first:

```sh
claude mcp add phonedriver -- npx -y phonedriver mcp
claude mcp list    # expect: phonedriver ... Connected
```

The first start downloads the binary once (see [Install](/docs/install/)). If you [installed phonedriver](/docs/install/#permanent-install) instead, use `claude mcp add phonedriver -- phonedriver mcp`, or the full path to the binary after the `--`.

For the first run, boot one simulator or connect one device, so no device setting is needed. With more than one device, set the one to use in the server's environment:

```sh
claude mcp add --env PHONEDRIVER_DEVICE=android:emulator-5554 phonedriver -- npx -y phonedriver mcp
```

`phonedriver devices` lists the ids. `PHONEDRIVER_DEVICE` is optional when one device is connected.

## Ask it

Boot a device, then ask Claude Code in plain language:

> Open Settings and turn on dark mode.

Claude Code calls the tools, for example `launch`, `swipe` and `tap`. Each reply is plain text. The tap that turns the switch on returns this on the iOS Simulator:

```screen
ok tapped @e41 "Dark Appearance" at 337,198 · settled in 607ms
- @e65 @e66 @e67
~ @e41 switch "Dark Appearance" off → on
+ @e68 button "L4S"
```

`+`, `-` and `~` mark lines that appeared, left and changed. The agent does not poll or sleep: every action waits for the screen to settle.

## What the agent gets

Fourteen tools: `devices`, `apps`, `snapshot`, `screenshot`, `tap`, `type`, `swipe`, `scroll`, `press`, `launch`, `wait`, `batch`, `clipboard` and `shell`. Each prints the same text as the CLI verb of the same name. `phonedriver skill` prints `SKILL.md`, the instructions an agent reads before it uses the tools; the MCP server carries a short version too.

## When Claude Code asks you something

A tap on a button that spends money or cannot be undone (`Place order`, `Pay`, `Delete account`) is held until you answer. Claude Code shows the question with the app, the button and the amounts on screen. See [the approval guard](/docs/approval-guard/). In headless `claude -p` nobody can answer, so the tap is declined.

## Secrets

Text typed with `secret: true` over MCP never reaches logs, traces or replies.
