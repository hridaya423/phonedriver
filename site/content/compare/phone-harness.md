---
title: phonedriver vs phone-harness
seo: phonedriver vs phone-harness: a faster phone-harness alternative
updated: 2026-11-11
meta: phonedriver passed 27 of 27 runs on nine phone jobs where phone-harness 0.3.3 passed 14 of 18, with 1.7x fewer tokens, and is 2.4 to 186x faster per operation.
description: On nine real phone jobs phonedriver passed 27 of 27 runs and phone-harness 0.3.3 passed 14 of 18, using 1.7 times the tokens and 4.4 times the time. Per operation phonedriver was 2.4 to 186 times faster. Measured on one emulator, with how each tool drives a phone and what each sends home.
---

## The short answer

phonedriver finished more jobs, with fewer tokens, in less time. On nine real phone jobs on one emulator on 10 November 2026, phonedriver passed 27 of 27 runs with 657,542 tokens in 306 s, and phone-harness passed 14 of 18 with 1,127,495 tokens in 1,360.5 s: 1.7 times the tokens and 4.4 times the time. On the same Android 17 emulator, interleaved, phonedriver was faster on every operation measured, by 2.4 times (a cold start) to 186 times (the 1,000-row list read).

phone-harness is a Python CLI: the agent pipes a Python script on stdin, and helpers drive an Android phone over adb or an iPhone through the Mac's iPhone Mirroring window. phonedriver is an MCP server and CLI with typed tools, a `batch` tool for several steps in one call, a diff after every action, an approval prompt before a tap that spends money, and no telemetry.

## Measured

p50 of 50 runs after 20 warm-ups, Android 17 emulator, both tools on the emulator's own host, 11 November 2026. phonedriver 0.1.1 against phone-harness 0.3.3, helpers called in process, which spares it a Python start per call and favours it. Source: [speed-2026-11-11]({{REPO}}/blob/main/docs/bench/speed.md).

| Operation | phonedriver | phone-harness | phone-harness / phonedriver (95% CI) |
|---|---|---|---|
| UI tree, 1,000-row list | 12.7 ms | 2,362.5 ms | 186x (176.8 to 193.4) |
| UI tree, Settings | 51.0 ms | 2,393.8 ms | 47x (44.3 to 51.7) |
| Screenshot | 9.1 ms | 348.3 ms | 38x (36.9 to 39.8) |
| Type 100 characters | 158.3 ms | 3,928.7 ms | 25x (21.2 to 26.9) |
| Tap, 1.5 s apart | 28.7 ms | 220.4 ms | 7.7x (7.2 to 8.3) |
| Tap, back to back | 38.5 ms | 162.8 ms | 4.2x (3.0 to 5.1) |
| Tap, then what changed | 514.0 ms | not supported | |

phone-harness cannot type Unicode, and has no equivalent of a settled diff. Fresh process to first screen read: phonedriver 1,176.0 ms, phone-harness 2,776.6 ms.

## Tokens and passes

Claude Sonnet 5.5 through `claude -p`, each tool the only one available, nine jobs that only a phone can do, each result checked from device state (the [benchmark post](/blog/phonedriver-vs-mobile-mcp-vs-phone-harness-vs-adb-real-phone-jobs/) and the [jobs table](/benchmarks/#jobs)). Both tools ran on the same emulator on 10 November 2026. phonedriver ran each job three times and phone-harness twice. Tokens are medians per job:

| Job | phonedriver | phone-harness |
|---|---|---|
| qa-signup | 3/3, 121,854 | 1/2, 305,766 |
| qa-shop | 3/3, 92,570 | 2/2, 136,428 |
| relay-code | 3/3, 49,126 | 0/2, 161,444 |
| chrome-prefs | 3/3, 98,576 | 2/2, 151,782 |
| clock-settings | 3/3, 95,962 | 2/2, 79,931 |
| messages-pin-archive | 3/3, 45,240 | 2/2, 42,452 |
| photos-crop | 3/3, 61,477 | 2/2, 65,703 |
| wakelock-restrict | 3/3, 38,775 | 1/2, 54,694 |
| log-code | 3/3, 53,962 | 2/2, 129,294 |
| Sum | 27/27, 657,542 | 14/18, 1,127,495 |

Tool calls summed to 99 for phonedriver and 104.5 for phone-harness. phone-harness failed four runs: relay-code twice (no contact was saved), wakelock-restrict once, and qa-signup once, which hit the 10-minute limit. The failed runs are part of its sum. These are small samples on a test set I wrote. I also ran a smaller, earlier test with three tasks, where phonedriver's medians were 218,357 tokens against phone-harness's 330,375 (34% fewer), phone-harness was ahead on the Wikipedia task, and all runs passed. phonedriver's fixed per-turn cost is higher: the first turn of a one-word `claude -p` is 5,606 tokens against 5,214 ([benchmarks](/benchmarks/#tokens)).

## How they differ

| | phonedriver | phone-harness |
|---|---|---|
| Interface | MCP server and CLI, typed tools | CLI that runs a Python script the agent writes |
| Android | Resident agent, UiAutomation tree, H.264 stream, input injected through InputManager | One `adb` call per operation; `uiautomator dump` and `screencap` |
| iPhone | Own XCTest runner, needs Xcode and a signing team | iPhone Mirroring window and Apple Vision OCR (its README) |
| Waiting | Settle detection, returns the diff | Fixed sleeps and a hash-compare stable check (source at commit `4bebff1`) |
| Telemetry | None | The 0.3.3 source ships the script, its output and helper calls to PostHog by default, opt out with `DO_NOT_TRACK=1`. Its README does not mention it |

The telemetry row is from reading `src/phone_harness/telemetry.py` at commit `4bebff1` on 2026-11-05. I ran phone-harness with telemetry off in every benchmark. The project may have changed since.

## Where phone-harness is better

- It drives an iPhone without a signing team or an Xcode runner, by mirroring. That setup has limits its README lists: unlocking the phone pauses mirroring, OCR sees text but not icons, and there is no multi-touch.

## Use which

Use phonedriver when the agent calls tools, when you want it fast, when you want to know what changed after each action, and when you want an approval prompt before it spends money. Use phone-harness when you need to drive an iPhone through iPhone Mirroring with no Xcode runner and no signing team.
