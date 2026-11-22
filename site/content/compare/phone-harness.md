---
title: phonedriver vs phone-harness
seo: phonedriver vs phone-harness: a faster phone-harness alternative
updated: 2026-11-20
meta: phonedriver vs phone-harness 0.3.3: 27/27 vs 14/18 passed on nine phone jobs, 4.4x faster wall time, Unicode typing, zero telemetry, and iPhone Mirroring.
description: phonedriver passed 27/27 runs in 306 s where phone-harness 0.3.3 passed 14/18 in 1,360.5 s—yet phone-harness still won two jobs on tokens and drives iPhones without Xcode.
---

## The short answer

phone-harness is a clever idea: instead of exposing individual MCP tools, the agent writes short Python scripts piped to a CLI, driving Android over `adb` and iPhones through macOS iPhone Mirroring + Vision OCR.

In practice, wrapping `adb` one command at a time gets slow and brittle. Across nine real Android phone jobs (10 November 2026), **phonedriver passed 27/27 runs in 306 s (657,542 tokens)** while **phone-harness 0.3.3 passed 14/18 in 1,360.5 s (1,127,495 tokens)**—4.4x slower wall time and 1.7x the tokens.

Credit where it's due: when phone-harness *doesn't* hit a `uiautomator dump` snag, its lean script model is genuinely frugal—it beat phonedriver on tokens in two of the nine jobs (`clock-settings` and `messages-pin-archive`). And if you want to drive an iPhone without touching Xcode or signing an XCTest runner, phone-harness's iPhone Mirroring path is its real superpower.

## Side by side

| | phonedriver | phone-harness 0.3.3 |
|---|---|---|
| Interface | MCP server + CLI (typed tools + `batch`) | CLI that executes agent-written Python scripts |
| Android engine | Resident helper, open `UiAutomation`, H.264 stream | One `adb` call per op (`uiautomator dump`, `screencap`) |
| iPhone engine | Custom XCTest runner (needs Mac + Xcode + signing) | macOS iPhone Mirroring window + Apple Vision OCR |
| Waiting | Visual settle detection + line diff | Fixed sleeps + hash-compare stability check |
| Unicode typing | Full UTF-8 & emoji (160.6 ms / 100 chars) | ASCII only |
| Telemetry | None | Ships scripts, outputs, and calls to PostHog by default (`DO_NOT_TRACK=1` opts out; undocumented in 0.3.3 README) |

## Measured speed (Android 17 emulator)

Even with phone-harness's Python helpers called in-process (sparing it Python startup on every call), keeping a resident helper on Android leaves a wide gap:

| Operation | phonedriver | phone-harness 0.3.3 | Speedup (95% CI) |
|---|---|---|---|
| UI tree, 1,000-row list | 12.7 ms | 2,362.5 ms | **186x** (176.8–193.4) |
| UI tree, Settings | 51.0 ms | 2,393.8 ms | **47x** (44.3–51.7) |
| Screenshot (0.1.1 cached path) | 9.1 ms | 348.3 ms | **38x** (36.9–39.8) |
| Type 100 characters | 158.3 ms | 3,928.7 ms | **25x** (21.2–26.9) |
| Tap, 1.5 s apart | 28.7 ms | 220.4 ms | **7.7x** (7.2–8.3) |
| Tap, back to back | 38.5 ms | 162.8 ms | **4.2x** (3.0–5.1) |
| Tap, then what changed | 514.0 ms | not supported | — |

## What happened on the nine real phone jobs

phone-harness lost four of its 18 runs: **0/2 on `relay-code`** (because `uiautomator dump` fails with `"could not get idle state"` on Android's Contacts screen), **1/2 on `wakelock-restrict`**, and **1/2 on `qa-signup`** (which hit the 10-minute cap). Those failed runs sit inside its 1.13M token total; on jobs it finished cleanly, it was right beside phonedriver—winning `clock-settings` (79,931 vs. 95,962) and `messages-pin-archive` (42,452 vs. 45,240).

:::receipts Versions, telemetry check & raw data
- **Speed run:** 11 November 2026 on `emulator-5554` (API 37), 50 runs after 20 warm-ups, phonedriver `0.1.1` vs. phone-harness `0.3.3`. Full raw data in [`docs/bench/speed.md`]({{REPO}}/blob/main/docs/bench/speed.md).
- **Jobs run:** 10 November 2026 on `Pixel_Tablet_API_37`, Claude Sonnet 5.5, 3 passes for phonedriver and 2 for phone-harness. Full table in [the nine-job benchmark post](/blog/phonedriver-vs-mobile-mcp-vs-phone-harness-vs-adb-real-phone-jobs/).
- **Telemetry note:** Checked in `src/phone_harness/telemetry.py` at commit `4bebff1` (5 November 2026). I disabled telemetry for every benchmark run; the project may have changed since.
:::

## Use which

Pick **phonedriver** for speed, MCP integration, Unicode typing, settled diffs, payment approvals, and zero telemetry. Pick **phone-harness** if you want to drive an iPhone through macOS iPhone Mirroring without setting up Xcode or an Apple signing certificate (trading off multi-touch, icon-only controls, and phone unlocking while mirrored).
