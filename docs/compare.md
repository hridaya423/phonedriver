# phonedriver vs adb vs phone-harness vs mobile-mcp

phonedriver is an open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones. This page compares it with the three tools an agent is most likely to use for the same job. Every number comes from `docs/bench/`; every statement about another tool names its source. Where a tool was not measured, the cell says so.

## Summary

| | phonedriver | adb | phone-harness 0.3.3 | mobile-mcp 1.0.16 |
|---|---|---|---|---|
| What it is | MCP server and CLI, one Rust binary | Android Debug Bridge, the SDK's command-line tool | Python CLI; the agent pipes it a script | MCP server (Node) |
| Platforms | Android, iOS Simulator, iPhone and iPad | Android only | Android via adb, iPhone via Mac mirroring | Android, iOS, cloud devices |
| Screen as text | Pruned, numbered tree (`@e10 switch "Dark theme" off`) | Raw XML from `uiautomator dump` | OCR text and `uiautomator dump` | One line per element with position and size |
| After a tap | The lines that changed, once the screen settles | Nothing; the caller dumps and diffs | Nothing measured; "tap, then what changed" is not supported | No wait after actions in `server.ts`; the agent lists elements again |
| Read a 1,000-row UI tree (p50) | 24.0 ms | 2,352.4 ms (98x slower) | 2,653.7 ms (111x slower) | not measured |
| Screenshot (p50) | 11.6 ms | 476.3 ms (41x slower) | 570.5 ms (49x slower) | not measured |
| Type 100 characters (p50) | 251.1 ms | 4,840.9 ms (19x slower) | 5,523.3 ms (22x slower) | not measured |
| Tap and see what changed (p50) | 586.9 ms | 2,725.8 ms (4.6x slower; tap, dump and diff in the caller) | not supported | not measured |
| Tokens, three agent tasks (sum of medians) | 218,357, all passed | one task failed | 330,375, all passed | 0 of 3 passed; an environment failure, not comparable |
| Approval before a tap that spends money | Yes (the approval guard, D27) | No | Not checked | Not checked |
| Shell on the device | Yes (`shell`, D21) | Yes (`adb shell`) | Not checked | Not documented in the sources read |
| Unicode typing | Yes (mixed-script text typed in 225.0 ms) | No (`input text` raises on non-ASCII) | No | Not checked |
| License | AGPL-3.0-only | Part of the Android SDK | MIT | Apache-2.0 |

Speed is p50 of 50 runs after 20 warm-ups on one Android 17 emulator (API 37), the three measured tools interleaved on the emulator's host. No physical Android phone and no iOS device is in this run. Method, p95 and confidence intervals: [`bench/speed.md`](bench/speed.md). Data: [`bench/speed.json`](bench/speed.json).

## Speed

phonedriver is faster than adb and phone-harness on every operation measured. Against adb the ratio runs from 1.1x (taps sent back to back) to 98x (reading a 1,000-row list; 95% interval on the ratio 92.2 to 106.0). Against phone-harness it runs from 1.3x to 111x. The bare tap is the closest row: 175.6 ms for phonedriver and 199.5 ms for adb, host to reply. adb returns raw XML and a tap that returns before the app reacts. phonedriver returns a pruned tree and waits for the app, so the two outputs differ. See [`bench/speed.md`](bench/speed.md), "What this does not show".

## Tokens

Three tasks (set an alarm, read a birth date on Wikipedia, submit a web form), `claude -p` with `claude-sonnet-5-5`, a tablet emulator, each task passed or failed from device state. phonedriver used 218,357 tokens (sum of per-task medians) against 330,375 for phone-harness, 34% fewer. Both passed every run. On `wiki-birthdate` phonedriver used more (57,753 against 53,403). Raw adb finished one task in two calls with a shell intent and failed the web form, which has no accessibility nodes. mobile-mcp failed every run because its on-device server did not start in this setup, so its totals say nothing about its design. Runs are single samples per cell. Details: [`bench/tokens.md`](bench/tokens.md), data in [`bench/tokens.json`](bench/tokens.json).

## When to use which

- **adb** when a shell command does the job (`cmd uimode night yes`), or when you need nothing but one tap. It ships with the Android SDK and has no extra moving parts.
- **phonedriver** when an agent must read screens, act, and learn what changed, on Android or iOS, and a human should approve payments.
- **phone-harness** when the agent already writes Python scripts against a phone and the screen is an iPhone mirrored to a Mac.
- **mobile-mcp** when you want an MCP server with cloud devices or the broader action set it documents (batch commands, screen recording, device logs). Its source and README were read on 2026-11-06 through a summarizing fetch, so re-read them before you rely on this page.

## Limits of phonedriver

It is not published yet; build it from source. Android phones are untested on hardware (the emulator suite passes on API 26 and 37). On iPhone and iPad a tap takes 454 to 534 ms at p50, over its 80 ms budget. The approval guard matches fixed English labels, so an unlabeled button is not guarded. See the README's [Supported devices](../README.md#supported-devices).

## Sources

- phonedriver, adb, phone-harness: [`bench/speed.md`](bench/speed.md), [`bench/tokens.md`](bench/tokens.md).
- mobile-mcp and phone-harness features: [`research/competitors.md`](research/competitors.md), [`research/competitor-teardown.md`](research/competitor-teardown.md).
- Guard: `decisions.md` D27. Shell: D21.
