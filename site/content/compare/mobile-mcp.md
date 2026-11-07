---
title: phonedriver vs mobile-mcp
meta: phonedriver and mobile-mcp compared on platforms, reading, waiting, install, telemetry and license, with the speed and token figures we do not yet have.
description: Two MCP servers that let an AI agent control a phone. What each reads, installs, waits for and sends, with the speed and token figures we do and do not have for mobile-mcp.
---

## The short answer

mobile-mcp and phonedriver are both MCP servers that give an agent a phone. mobile-mcp has wider reach: iOS and Android, simulators, real devices, and cloud devices through Mobile Next Cloud, under Apache-2.0. phonedriver makes different bets: a resident agent on Android, a settled diff after every action, an approval prompt before a tap that spends money, no telemetry, and AGPL-3.0.

**We have no like-for-like speed figure for mobile-mcp.** In our token benchmark it failed every run for an environment reason in our setup, so we do not compare tokens either. Everything below is from its README and from our reading of its source, and the rows say which.

## Side by side

| | phonedriver | mobile-mcp |
|---|---|---|
| Platforms | Android, iOS | Android, iOS, cloud devices |
| Real devices | iPhone and iPad tested; Android hardware not yet tested | Yes for both; iOS over USB, Android with adb and USB debugging (its README) |
| How it reads the screen | Pruned text tree with `@eN` refs, settled; OCR when the tree is empty | Native accessibility tree, screenshots and coordinates when needed (its README) |
| Waits after an action | Yes: settle, then the diff | Its README does not say. We read no waits after actions in `src/server.ts` on 2026-11-06 (through a summarizing fetch tool: re-open the file before quoting) |
| Approval before spending money | Yes, through MCP elicitation | Not documented |
| Telemetry | None | Anonymous usage telemetry through PostHog and Scarf, on by default. Opt out with `MOBILEMCP_DISABLE_TELEMETRY=1` (its README) |
| Install | `brew install hridaya423/tap/phonedriver` (after the first release) | `npx @mobilenext/mobile-mcp@latest install`, which configures detected clients (its README) |
| License | AGPL-3.0-only | Apache-2.0 |
| Adoption | New, unpublished | About 10.4k GitHub stars on 2026-11-06 |

## What mobile-mcp has that phonedriver does not

From our 2026-11-06 reading of its documentation and source, to be re-checked before quoting: a batch tool for many commands in one call, location override, screen recording, device logs and crash lists, and cloud devices. phonedriver has a `batch` verb too, and no location override, recording or logs.

Its license is also easier to live with if you plan to embed it in a closed product. AGPL-3.0 asks you to publish your source if you distribute phonedriver or run a modified version as a network service. Running the unmodified tool as a tool asks nothing of you, but check that reading with your own counsel.

## What phonedriver does differently

- **A settled diff.** Every action waits for the screen to stop changing and returns the lines that changed, so the agent does not re-list the screen after each tap.
- **A resident agent.** On Android, a small agent is pushed over adb and started as the shell user. UI reads cost 24.0 to 89.3 ms at p50 on our benchmark emulator.
- **The approval guard.** See [the guard](/docs/approval-guard/) and [its limits](/blog/letting-claude-order-dinner-without-letting-it-spend-your-money/).
- **No telemetry**, checked by a test that runs the CLI, the daemon and the MCP server under a sandbox that kills any non-local connection.

## What we found when we ran it

In our token run, mobile-mcp 1.0.16 took screenshots and launched apps but failed taps and UI listing with "device server did not start within 5s", and, against a remote adb server, an `adb forward` bind error. It also left an on-device server (`com.mobilenext.mobilecli.DeviceServer`) holding the accessibility bridge, which made other tools fail with `another UiAutomation client` until we killed it between runs. That is a fault of our setup on a tunnelled emulator, not a measured capability. A run on a local emulator or phone is still to do, and we will publish it on the [benchmarks page](/benchmarks/) when it exists.

## Use which

Choose mobile-mcp if you need cloud devices, a permissive license or its wider tool set. Choose phonedriver if you want the settled diff, the approval prompt, no telemetry and the speed on Android shown on the [benchmarks page](/benchmarks/). They can sit side by side on different devices. Do not run both against the same Android device at once: each holds the one UiAutomation connection.
