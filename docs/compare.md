# phonedriver vs adb vs phone-harness vs mobile-mcp

phonedriver is an open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones. This page compares it with the three tools an agent is most likely to use for the same job. Every number comes from `docs/bench/`; every statement about another tool names its source. Where a tool was not measured, the cell says so.

## Summary

| | phonedriver | adb | phone-harness 0.3.3 | mobile-mcp 1.0.16 |
|---|---|---|---|---|
| What it is | MCP server and CLI, one Rust binary | Android Debug Bridge, the SDK's command-line tool | Python CLI; the agent pipes it a script | MCP server (Node) |
| Platforms | Android, iOS Simulator, iPhone and iPad | Android only | Android via adb, iPhone via Mac mirroring | Android, iOS, cloud devices |
| Screen as text | Pruned, numbered tree (`@e10 switch "Dark theme" off`) | Raw XML from `uiautomator dump` | OCR text and `uiautomator dump` | One line per element with position and size |
| After a tap | The lines that changed, once the screen settles | Nothing; the caller dumps and diffs | Nothing measured; "tap, then what changed" is not supported | No wait after actions in `server.ts`; the agent lists elements again |
| Read a 1,000-row UI tree (p50) | 12.7 ms | 2,163.7 ms (170x slower) | 2,362.5 ms (186x slower) | not measured |
| Screenshot (p50) | 9.1 ms in 0.1.1, which returned the last frame without waiting for the encoder (D74); not measured since | 291.2 ms | 348.3 ms | not measured |
| Type 100 characters (p50) | 158.3 ms | 3,638.4 ms (23x slower) | 3,928.7 ms (25x slower) | not measured |
| Tap and see what changed (p50) | 514.0 ms | 2,477.2 ms (4.8x slower; tap, dump and diff in the caller) | not supported | not measured |
| Nine phone jobs, 2026-11-10 (passed; tokens, sum of job medians) | 27 of 27; 657,542 | the two jobs it can do: 4 of 4; 203,751 | 14 of 18; 1,127,495 | 17 of 18; 4,567,200 |
| Approval before a tap that spends money | Yes (the approval guard, D27) | No | Not checked | Not checked |
| Shell on the device | Yes (`shell`, D21) | Yes (`adb shell`) | Not checked | Not documented in the sources read |
| Unicode typing | Yes (mixed-script text typed in 160.6 ms) | No (`input text` raises on non-ASCII) | No | Not checked |
| License | AGPL-3.0-only | Part of the Android SDK | MIT | Apache-2.0 |

Speed is from the run of 2026-11-11 with phonedriver 0.1.1: p50 of 50 runs after 20 warm-ups on one Android 17 emulator (API 37), the three measured tools interleaved on the emulator's host. No physical Android phone and no iOS device is in this run. Method, p95 and confidence intervals: [`bench/speed.md`](bench/speed.md). Data: [`bench/speed.json`](bench/speed.json).

## Speed

phonedriver is faster than adb and phone-harness on every operation measured. Against adb the ratio runs from 1.9x (cold start) to 170x (reading a 1,000-row list; 95% interval on the ratio 162.0 to 177.4). Against phone-harness it runs from 2.4x to 186x. The bare tap is the closest warm row: 28.7 ms for phonedriver and 69.4 ms for adb when taps are 1.5 s apart, host to reply. adb returns raw XML and a tap that returns before the app reacts. phonedriver returns a pruned tree and waits for the app, so the two outputs differ. See [`bench/speed.md`](bench/speed.md), "What this does not show".

## Tokens

Nine jobs that only a phone can do, on one Android emulator on 2026-11-10, `claude -p` with `claude-sonnet-5-5`, each run passed or failed from the phone's own data: phonedriver passed 27 of 27 runs (three passes) with 657,542 tokens, the sum of the nine job medians; mobile-mcp 17 of 18 (two passes) with 4,567,200, 6.9x as many; phone-harness 14 of 18 with 1,127,495. adb was run only on the two jobs that allow a shell route and passed both twice. Details: [`bench/jobs.md`](bench/jobs.md), data in [`bench/jobs.json`](bench/jobs.json).

An earlier, smaller run: three tasks (set an alarm, read a birth date on Wikipedia, submit a web form), `claude -p` with `claude-sonnet-5-5`, a tablet emulator, each task passed or failed from device state. phonedriver used 218,357 tokens (sum of per-task medians) against 330,375 for phone-harness, 34% fewer. Both passed every run. On `wiki-birthdate` phonedriver used more (57,753 against 53,403). Raw adb finished one task in two calls with a shell intent and failed the web form, which has no accessibility nodes. mobile-mcp failed every run because its on-device server did not start in this setup, so its totals say nothing about its design. Runs are single samples per cell. Details: [`bench/tokens.md`](bench/tokens.md), data in [`bench/tokens.json`](bench/tokens.json).

## When to use which

- **adb** when a shell command does the job (`cmd uimode night yes`), or when you need nothing but one tap. It ships with the Android SDK and has no extra moving parts.
- **phonedriver** when an agent must read screens, act, and learn what changed, on Android or iOS, and a human should approve payments.
- **phone-harness** when the agent already writes Python scripts against a phone and the screen is an iPhone mirrored to a Mac.
- **mobile-mcp** when you want an MCP server with cloud devices or the broader action set it documents (batch commands, screen recording, device logs). Its source and README were read on 2026-11-06 through a summarizing fetch, so re-read them before you rely on this page.

## Limits of phonedriver

One Android phone is tested on hardware, a HONOR X8 on Android 13; the emulator suite passes on API 26 and 37. On iPhone and iPad a tap takes 454 to 534 ms at p50, over its 80 ms budget. The approval guard matches a fixed list of labels in eleven languages, so an unlabeled button is not guarded. See the README's [Supported devices](../README.md#devices).

## Sources

- phonedriver, adb, phone-harness, mobile-mcp: [`bench/speed.md`](bench/speed.md), [`bench/jobs.md`](bench/jobs.md), [`bench/tokens.md`](bench/tokens.md).
- mobile-mcp and phone-harness features: read from their source and documentation in November 2026 (phone-harness at commit `4bebff1`).
- Guard: `decisions.md` D27. Shell: D21.
