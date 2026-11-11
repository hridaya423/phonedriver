---
title: phonedriver vs mobile-mcp
seo: phonedriver vs mobile-mcp: an alternative with 6.9x fewer tokens
updated: 2026-11-11
meta: On nine real phone jobs mobile-mcp used 6.9x the tokens of phonedriver and passed 17 of 18 runs against 27 of 27. Platforms, waiting, telemetry and install compared.
description: Two MCP servers that let an AI agent control a phone. On nine real phone jobs on one emulator, mobile-mcp used 6.9 times the tokens of phonedriver and passed 17 of 18 runs against 27 of 27. What each reads, installs, waits for and sends, and what I have not measured.
---

## The short answer

**On nine phone jobs on one emulator, 10 November 2026, mobile-mcp used 6.9 times the tokens of phonedriver:** 4,567,200 against 657,542 as the sum of per-job medians, with 17 of 18 runs passed against 27 of 27. Wall time summed to 523 s against 306 s.

Both are MCP servers that give an agent a phone. phonedriver keeps a resident agent on Android, returns a settled diff after every action, asks before a tap that spends money, and sends no telemetry. mobile-mcp reaches cloud devices through Mobile Next Cloud, which phonedriver does not. I have no per-operation speed comparison with mobile-mcp. The feature rows below are from its README and from my reading of its source, and the rows say which.

## Side by side

| | phonedriver | mobile-mcp |
|---|---|---|
| Platforms | Android, iOS | Android, iOS, cloud devices |
| Real devices | iPhone and iPad tested; one Android phone (HONOR X8) tested | Yes for both; iOS over USB, Android with adb and USB debugging (its README) |
| How it reads the screen | Pruned text tree with `@eN` refs, settled; OCR when the tree is empty | Native accessibility tree, screenshots and coordinates when needed (its README) |
| Waits after an action | Yes: settle, then the diff | Its README does not say. I read no waits after actions in `src/server.ts` on 2026-11-06 (through a summarizing fetch tool: re-open the file before quoting) |
| Approval before spending money | Yes, through MCP elicitation | Not documented |
| Telemetry | None | Anonymous usage telemetry through PostHog and Scarf, on by default. Opt out with `MOBILEMCP_DISABLE_TELEMETRY=1` (its README) |
| Install | `claude mcp add phonedriver -- npx -y phonedriver mcp`, or Homebrew or a shell installer ([install](/docs/install/)) | `npx @mobilenext/mobile-mcp@latest install`, which configures detected clients (its README) |
| License | AGPL-3.0-only | Apache-2.0 |
| Adoption | New: first released on 10 November 2026 | About 10.4k GitHub stars on 2026-11-06 |

## What mobile-mcp has that phonedriver does not

From my 2026-11-06 reading of its documentation and source, to be re-checked before quoting: a batch tool for many commands in one call, location override, screen recording, device logs and crash lists, and cloud devices. phonedriver has a `batch` verb too, and no location override, recording or logs.

## What phonedriver does differently

- **A settled diff.** Every action waits for the screen to stop changing and returns the lines that changed, so the agent does not re-list the screen after each tap.
- **A resident agent.** On Android, a small agent is pushed over adb and started as the shell user. UI reads cost 12.7 to 51.0 ms at p50 on my benchmark emulator.
- **The approval guard.** See [the guard](/docs/approval-guard/) and [its limits](/blog/letting-claude-order-dinner-without-letting-it-spend-your-money/).
- **No telemetry**, checked by a test that runs the CLI, the daemon and the MCP server under a sandbox that kills any non-local connection.

## What I found when I ran it

On 10 November 2026 both tools ran on the same Android emulator, one agent at a time, with Claude Sonnet 5.5 through `claude -p`. mobile-mcp 1.0.16 had its own adb server on the machine that ran the agent. phonedriver ran each of nine jobs three times and mobile-mcp twice, and every run was checked from device state. Tokens are medians per job:

| Job | phonedriver | mobile-mcp | Ratio |
|---|---|---|---|
| qa-signup | 3/3, 121,854 | 2/2, 549,354 | 4.5x |
| qa-shop | 3/3, 92,570 | 2/2, 473,095 | 5.1x |
| relay-code | 3/3, 49,126 | 2/2, 574,428 | 11.7x |
| chrome-prefs | 3/3, 98,576 | 1/2, 1,052,348 | 10.7x |
| clock-settings | 3/3, 95,962 | 2/2, 691,926 | 7.2x |
| messages-pin-archive | 3/3, 45,240 | 2/2, 315,560 | 7.0x |
| photos-crop | 3/3, 61,477 | 2/2, 636,680 | 10.4x |
| wakelock-restrict | 3/3, 38,775 | 2/2, 122,772 | 3.2x |
| log-code | 3/3, 53,962 | 2/2, 151,038 | 2.8x |
| Sum | 27/27, 657,542 | 17/18, 4,567,200 | 6.9x |

Tool calls summed to 99 for phonedriver and 276.5 for mobile-mcp, and wall time to 306 s and 523 s. mobile-mcp's one failure was chrome-prefs, which ended with "Always use secure connections" off. It was 5.0 times phonedriver's tokens on 7 November, on an earlier emulator of the same image. It is a test set I wrote, and mobile-mcp may do better on a job I did not think of. The runs, the checks and what I threw away are in [the benchmark post](/blog/phonedriver-vs-mobile-mcp-vs-phone-harness-vs-adb-real-phone-jobs/) and on the [benchmarks page](/benchmarks/#jobs).

Setup matters for it. In my earlier token run, mobile-mcp took screenshots and launched apps but failed taps and UI listing with "device server did not start within 5s", and, against a remote adb server, an `adb forward` bind error. Its helper forwards a port to its device server, and a remote adb server binds that port on the wrong machine. In that run it also left an on-device server (`com.mobilenext.mobilecli.DeviceServer`) holding the accessibility bridge, which made other tools fail with `another UiAutomation client` until I killed it between runs. Both were faults of my setup on a tunnelled emulator, not a measured capability. I have no per-operation speed comparison with mobile-mcp.

## Use which

Choose phonedriver if you want fewer tokens per job, the settled diff, the approval prompt, no telemetry and the speed on Android shown on the [benchmarks page](/benchmarks/). Choose mobile-mcp if you need cloud devices, location override, screen recording or device logs. They can sit side by side on different devices. Do not run both against the same Android device at once: each holds the one UiAutomation connection.
