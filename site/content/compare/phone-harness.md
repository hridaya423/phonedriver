---
title: phonedriver vs phone-harness
meta: phonedriver against phone-harness 0.3.3: speed on one emulator, tokens on three tasks, how each drives a phone, telemetry and license.
description: phonedriver against phone-harness 0.3.3 on speed, tokens, how each reads and drives a phone, telemetry and license. Measured on one emulator with both tools interleaved.
---

## The short answer

phone-harness is a Python CLI: the agent pipes a Python script on stdin, and helpers drive an Android phone over adb or an iPhone through the Mac's iPhone Mirroring window. phonedriver is an MCP server and CLI with typed tools. On the same Android 17 emulator, interleaved, phonedriver was faster on every operation measured, by 1.3 times (a tap sent back to back) to 111 times (the 1,000-row list read). On three agent tasks it used 34% fewer tokens in total, with one task where phone-harness was ahead.

## Measured

p50 of 50 runs after 20 warm-ups, Android 17 emulator, both tools on the emulator's own host. phone-harness 0.3.3, helpers called in process, which spares it a Python start per call and favours it. Source: [speed-2026-11-07]({{REPO}}/blob/main/docs/bench/speed-2026-11-07.md).

| Operation | phonedriver | phone-harness | phone-harness / phonedriver (95% CI) |
|---|---|---|---|
| UI tree, 1,000-row list | 24.0 ms | 2,653.7 ms | 111x (104.1 to 119.5) |
| Screenshot | 11.6 ms | 570.5 ms | 49x (47.5 to 51.4) |
| UI tree, Settings | 89.3 ms | 2,700.0 ms | 30x (28.5 to 30.8) |
| Type 100 characters | 251.1 ms | 5,523.3 ms | 22x (19.4 to 25.2) |
| Tap, 1.5 s apart | 61.6 ms | 319.7 ms | 5.2x (4.7 to 5.6) |
| Tap, back to back | 175.6 ms | 232.5 ms | 1.3x (1.2 to 1.5) |
| Tap, then what changed | 586.9 ms | not supported | |

phone-harness cannot type Unicode, and has no equivalent of a settled diff. Fresh process to first screen read: phonedriver 1,620.5 ms, phone-harness 2,999.4 ms.

## Tokens

Claude Sonnet 5.5 through `claude -p`, each tool the only one available, three tasks checked from device state ([benchmarks](/benchmarks/#tokens)). Medians of 2 to 3 runs:

| Task | phonedriver | phone-harness |
|---|---|---|
| Alarm in Clock | 110,419 | 220,723 |
| Wikipedia birth date in Chrome | 57,753 | 53,403 |
| Web form | 50,185 | 56,249 |
| Sum | 218,357 | 330,375 |

All runs passed. These are small samples. phone-harness is ahead on the Wikipedia task, and phonedriver's fixed per-turn cost is higher: the first turn of a one-word `claude -p` is 5,606 tokens against 5,214.

## How they differ

| | phonedriver | phone-harness |
|---|---|---|
| Interface | MCP server and CLI, typed tools | CLI that runs a Python script the agent writes |
| Android | Resident agent, UiAutomation tree, H.264 stream, input injected through InputManager | One `adb` call per operation; `uiautomator dump` and `screencap` |
| iPhone | Own XCTest runner, needs Xcode and a signing team | iPhone Mirroring window and Apple Vision OCR (its README) |
| Waiting | Settle detection, returns the diff | Fixed sleeps and a hash-compare stable check (source at commit `4bebff1`) |
| Telemetry | None | The 0.3.3 source ships the script, its output and helper calls to PostHog by default, opt out with `DO_NOT_TRACK=1`. Its README does not mention it |
| License | AGPL-3.0 | MIT |

The telemetry row is from reading `src/phone_harness/telemetry.py` at commit `4bebff1` on 2026-11-05 ([teardown]({{REPO}}/blob/main/docs/research/competitor-teardown.md)). We ran phone-harness with telemetry off in every benchmark. The project may have changed since.

## Where phone-harness is better

- An MIT license and `pip` or `uv` install.
- A script can do many steps in one run, so an agent that writes good Python can spend few turns.
- It drives an iPhone without a signing team or an Xcode runner, by mirroring. That setup has limits its README lists: unlocking the phone pauses mirroring, OCR sees text but not icons, and there is no multi-touch.

## Use which

If the agent is a coding agent that writes scripts and you want an MIT tool, phone-harness is a fair choice. If the agent calls tools, you want it fast, you want to know what changed after each action and you want an approval prompt before it spends money, use phonedriver.
