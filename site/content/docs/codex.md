---
title: How to use phonedriver with Codex
seo: How to let Codex control an Android phone or iPhone
nav: Codex
description: Add phonedriver to OpenAI Codex with one command, a config.toml entry, or the plugin—then point it at an Android or iOS device.
updated: 2026-11-20
date: 2026-11-14
---

OpenAI Codex drives Android and iOS through the same phonedriver MCP server. Run `npx -y phonedriver init` (or `codex mcp add` below), connect a device, and ask in plain English.

I verified the plugin install, MCP server load, and skill load in Codex 0.167.0; the nine-job benchmarks on this site ran in Claude Code.

## Add the server

With Node 18+ and nothing installed beforehand:

```sh
codex mcp add phonedriver -- npx -y phonedriver mcp
codex mcp list
```

Inside a session, `/mcp` confirms the server is live. The first launch caches the binary once ([Install](/docs/install/)). If you [installed phonedriver permanently](/docs/install/#permanent-install), pass `phonedriver mcp` instead.

With multiple devices connected, pin one with `PHONEDRIVER_DEVICE` (`phonedriver devices` lists IDs):

```sh
codex mcp add phonedriver --env PHONEDRIVER_DEVICE=android:emulator-5554 -- npx -y phonedriver mcp
```

To keep the agent in your own app on a phone with real accounts, add `--env PHONEDRIVER_APP_SCOPE=com.example.app` ([what it covers](/docs/android-phone/#keep-the-agent-in-your-app)). To let the agent clear, remove or change the permissions of an app you are developing on a physical Android phone (the apps Gradle installed, not phonedriver), add `--env PHONEDRIVER_APPS_UNDER_TEST=com.example.app` the same way, and run `phonedriver daemon stop` once if a daemon is running.

To keep a phone out of the agent's reach on a shared adb server, add `--env PHONEDRIVER_DENY_DEVICES=android:<id>` the same way, or write `deny android:<id>` in `~/.phonedriver/devices`, which every agent on the machine obeys. A denied device is not listed, not probed by `doctor` and never gets the agent.

## Or edit config.toml

Codex stores MCP servers in `~/.codex/config.toml` (or `.codex/config.toml` in a trusted project):

```toml
[mcp_servers.phonedriver]
command = "npx"
args = ["-y", "phonedriver", "mcp"]

[mcp_servers.phonedriver.env]
PHONEDRIVER_DEVICE = "android:emulator-5554"
```

Skip the `env` block if you only have one device connected.

## Or install the plugin

The plugin adds both the MCP server and the phonedriver skill in one step. Don't combine it with `mcp add`, or Codex will see every tool twice.

```sh
codex plugin marketplace add hridaya423/phonedriver
codex plugin add phonedriver@phonedriver
```

## Ask it

Boot an [Android emulator](/docs/android-emulator/), plug in an [Android phone](/docs/android-phone/), or start the [iOS Simulator](/docs/ios-simulator/), then ask:

> Open Settings and turn on dark mode.

Every tap settles automatically and returns only the lines that changed:

```screen
ok tapped @e10 "Dark theme"
~ @e10 switch "Dark theme" off → on
```

## Approval prompts in Codex

The [approval guard](/docs/approval-guard/) pauses taps on Pay, Place order, or Delete account and asks you via MCP elicitation. If your Codex version doesn't render elicitation prompts, phonedriver taps nothing and returns `Needs the human's approval; nothing was tapped` so Codex asks you in chat instead. Read [what the guard misses](/docs/approval-guard/#what-it-does-not-catch) before pointing it at a phone with saved payment methods.

## Without MCP

Any agent with shell access can call the CLI directly for the exact same output:

```sh
npx -y phonedriver snapshot
npx -y phonedriver tap @e4
```

Run `npx -y phonedriver skill` to print the agent instructions.

## Questions

### Can Codex control a real iPhone?

Yes, from a Mac with Xcode. Plug the iPhone in, run `phonedriver ios setup` once, and optionally `phonedriver ios wifi on` to go wireless ([iPhone guide](/docs/iphone/)).

### Does Codex need the phonedriver skill?

It helps. The MCP server includes a short built-in summary, while the skill teaches the full workflow (including multi-step tap paths). The plugin installs it automatically; with `codex mcp add`, `npx -y phonedriver skill` prints it.

### How many tokens do the tools cost?

In Claude Code, turn one of a bare `claude -p` costs 5,606 tokens with the 14 tools it had then attached (it has 18 now). I haven't measured the Codex wrapper separately ([token benchmark](/benchmarks/#tokens)).
