---
title: "phonedriver vs mobile-mcp vs phone-harness vs adb: benchmarked on real phone jobs"
seo: phonedriver vs mobile-mcp vs phone-harness vs adb on real phone jobs
short: Benchmarked on real phone jobs
meta: The method and results of a benchmark of phonedriver, mobile-mcp, phone-harness and adb on nine jobs only a phone can do. Results in.
description: The method and results of a benchmark that gives Claude nine jobs only a phone can do, with phonedriver, mobile-mcp, phone-harness and adb. Every harness ran each job twice.
date: 2026-11-07
label: BENCHMARK
short_card: phonedriver vs mobile-mcp vs phone-harness vs adb on real phone jobs
card_sub: Nine jobs, independent checks, every result sourced. 18 of 18 for phonedriver and mobile-mcp, 14 of 18 for phone-harness.
---

> **Status: finished.** Each harness ran each job twice on a tablet emulator. Every number below that is a measurement is the median of those two runs, and the raw runs are in the repository. Nothing here is a projection.

Speed benchmarks tell you how fast a tool reads a screen. They do not tell you whether an agent finishes a job with it. For that you need jobs, a model, and a check that does not trust the model's word. This post is the comparison we ran, with its method written down before the results existed so that the results could not shape it.

## What is being compared

Four ways for Claude Sonnet 5.5 to reach a phone:

- **phonedriver**, as an MCP server.
- **mobile-mcp** 1.0.16, as an MCP server.
- **phone-harness** 0.3.3, as a CLI that runs Python scripts.
- **Raw adb**, as a shell tool limited to `adb`.

The jobs are the same for every harness, and so is the prompt, which is stored in `bench/agent-eval/tasks.yaml`. Each run starts from a reset device, runs `claude -p` from an empty directory with strict MCP configuration, no slash commands, no settings sources, no session persistence and a 40-turn cap, and then a script checks the device. Runs use the Claude subscription, never an API key. The device is the 2560x1600 Pixel Tablet emulator, reached through an ssh tunnel.

## The nine jobs

Seven jobs are UI-only: the result cannot be produced by adb, `am`, `cmd`, `settings`, `content` or a browser, and the reason is written per job. Two are mixed: any route is allowed, and each needs a device-side read and a UI step, so a tool that holds both in one session should need fewer calls. The full table, with why adb cannot do each, is on [the benchmarks page](/benchmarks/#jobs), and the reasoning behind the rule is in [Benchmarking AI agents on jobs only a phone can do](/blog/benchmarking-ai-agents-on-jobs-only-a-phone-can-do/).

Raw adb is not a competitor on the seven UI-only jobs by construction. Those jobs were chosen because adb cannot do them, so its row there is expected to be empty, and we will not count that as a win. It competes on the two mixed jobs, where any route is allowed.

## How a run is scored

A run passes if the check passes. A check reads device state, never the agent's claim: the contacts provider, Messages' database, Clock's and Chrome's private preferences as root, `appops`, MediaStore, or the testbed app's event log. The two bug-report jobs also match the written report against a list of planted bugs with regular expressions, no model as judge, and require the event log to show that the bug paths were exercised. A job that forbids a shell fails the run if the tool log shows shell use.

Each check was proven by hand before any agent ran: a pass state, and a fail state for every way it can fail. That found real problems in our checks. A report splitter cut sentences at the decimal point in `$36.00`, so a correct report failed. A "London is in the world clock list" test would pass on a fresh Clock, because the home time zone on the host is `Europe/London`. A Messages check looked for a pin column that does not exist. All three were fixed before a model touched them.

For every run we record whether it passed, tool calls, total tokens (input, output, cache creation and cache read, the definition from our earlier agent evaluation) and wall time. Per job and harness we report the median of the runs and the pass count, and we show every run.

## Results

Pass counts and medians (total tokens, tool calls, wall time) per job and harness. The two mixed jobs have an adb column.

| Job | phonedriver | mobile-mcp | phone-harness | adb |
|---|---|---|---|---|
| qa-signup | 2/2, 181,518 tokens, 27 calls, 80 s | 2/2, 514,062 tokens, 39.5 calls, 110 s | 1/2, 246,327 tokens, 14.5 calls, 459 s | not applicable |
| qa-shop | 2/2, 138,557 tokens, 18 calls, 65 s | 2/2, 476,136 tokens, 31.5 calls, 85.5 s | 1/2, 338,406 tokens, 13 calls, 392.5 s | not applicable |
| relay-code | 2/2, 69,994 tokens, 9 calls, 38 s | 2/2, 611,976 tokens, 35.5 calls, 122 s | 0/2, 147,436 tokens, 14 calls, 280 s | not applicable |
| chrome-prefs | 2/2, 104,302 tokens, 13.5 calls, 60.5 s | 2/2, 595,815 tokens, 33.5 calls, 179.5 s | 2/2, 193,370 tokens, 19 calls, 250.5 s | not applicable |
| clock-settings | 2/2, 95,116 tokens, 13.5 calls, 48 s | 2/2, 843,472 tokens, 41.5 calls, 112.5 s | 2/2, 76,850 tokens, 8.5 calls, 162 s | not applicable |
| messages-pin-archive | 2/2, 47,090 tokens, 6 calls, 30 s | 2/2, 497,776 tokens, 24.5 calls, 56.5 s | 2/2, 45,825 tokens, 5 calls, 57.5 s | not applicable |
| photos-crop | 2/2, 220,836 tokens, 20.5 calls, 78 s | 2/2, 1,071,665 tokens, 47 calls, 112 s | 2/2, 103,776 tokens, 12 calls, 237.5 s | not applicable |
| wakelock-restrict (mixed) | 2/2, 60,896 tokens, 8 calls, 29 s | 2/2, 152,188 tokens, 9 calls, 28.5 s | 2/2, 79,878 tokens, 9.5 calls, 65.5 s | 2/2, 78,510 tokens, 10 calls, 49 s |
| log-code (mixed) | 2/2, 59,470 tokens, 8 calls, 38 s | 2/2, 129,150 tokens, 8.5 calls, 103.5 s | 2/2, 40,748 tokens, 5.5 calls, 41.5 s | 2/2, 66,158 tokens, 7.5 calls, 45.5 s |
| Total (sum of per-job medians) | 18/18 runs, 977,780 tokens, 123.5 calls, 466.5 s | 18/18 runs, 4,892,241 tokens, 270.5 calls, 910 s | 14/18 runs, 1,272,617 tokens, 101 calls, 1946 s | 4/4 runs, 144,669 tokens, 17.5 calls, 94.5 s (2 mixed jobs only) |

Each cell is passes out of 2, then the median of the two runs. The total adds the per-job medians, so phonedriver's 123.5 calls is a sum of medians and not a count of calls in any one run.

### Where phonedriver does not win

- **phone-harness is cheaper on three jobs.** photos-crop is 104k tokens against phonedriver's 221k, clock-settings 77k against 95k, and log-code 41k against 59k. messages-pin-archive is about equal (46k against 47k). phonedriver is cheaper on chrome-prefs (104k against 193k) and the rest.
- **The 23% token lead over phone-harness (977,780 against 1,272,617) comes mostly from phone-harness's failed runs.** They spent tokens and finished nothing: two runs hit the 10-minute limit at 265k and 623k tokens. On the jobs phone-harness completed, it is often the cheaper one.
- **mobile-mcp is 0.5 s faster on wakelock-restrict** (28.5 s against 29 s). phonedriver has the lowest median wall time on the other 8 jobs.
- **A shell does not need us for the mixed jobs.** adb passed 4 of 4 on log-code and wakelock-restrict, in 45.5 s and 49 s against phonedriver's 38 s and 29 s, using 66k and 79k tokens against 59k and 61k.

### What each harness failed at

- **phone-harness, relay-code, 0 of 2.** It reads screens through uiautomator dumps, and the dump fails with "could not get idle state" on the Contacts list, so it could not see the Contacts app. One run gave up after 17 calls and the other after 11, asking the user whether to tap blind. The contact was never created.
- **phone-harness, qa-signup and qa-shop, 1 of 2 each.** The first pass of each hit the 10-minute limit (601 s with 7 calls; 607 s with 20 calls). The second passed (317 s with 22 calls; 178 s with 6 calls). We did not diagnose why the first passes stalled beyond the wall time.
- **mobile-mcp passed all 18 runs** but spent 4.9M tokens, 5 times phonedriver's, because every action returns a screenshot or an element list. photos-crop (1.07M) and clock-settings (843k) are the worst. It ran against a private adb server on the same machine, so the tunnel problem from the earlier token benchmark did not apply.
- **phonedriver passed all 18.** One run was first scored a failure, a correct report with a "Bug 3: not a bug" heading, and passes under the second version of the rubric, which reads headings and sections as well as sentences. The original verdict is kept in `success_as_run` in the raw data.

Raw runs, medians and totals: [`docs/bench/jobs.json`](https://github.com/hridaya423/phonedriver/blob/main/docs/bench/jobs.json) and the write-up in `docs/bench/jobs.md`. Setup: `claude -p` on a subscription, Claude Sonnet 5.5, a 2560x1600 tablet emulator.

## Known biases, stated now

- **We are the author of one contestant.** phonedriver runs first, on its own, and a failure caused by a bug in phonedriver gets fixed before the next round. We have already fixed some: a dismissed approval prompt is not an answer, a field that refuses `set_text` is selected with Ctrl+A and pasted over, a formatted phone number now verifies, and the tool description now says that tools go one at a time. The other harnesses get no such rounds from us. When their failures look like setup problems rather than product limits, we will say which, and we will rerun them on a better setup before we publish a loss that is ours.
- **mobile-mcp needed a different setup.** In our earlier token benchmark it failed every run for an environment reason on a tunnelled emulator. Here it ran through a private adb server and passed all 18 runs, so its earlier numbers are not comparable.
- **Small samples.** Two passes per cell give a median of two. That is a description of what happened, not a rate.
- **One model, one device.** Claude Sonnet 5.5 on a tablet emulator. No physical phone.
- **Chrome and Messages change.** The jobs depend on app versions in the emulator image. The image is recorded with the results.

## What would change our mind

A job where adb finishes a core job we claimed it cannot would remove that job. A check that passes a wrong result would void every run of that job. A run where the agent reached the right state by a route we did not expect, for example by editing a database, would be reported and the check tightened. Raw run logs go in the repository next to the results so that you can read what the agent did.

Method and the other posts: [the jobs method](/blog/benchmarking-ai-agents-on-jobs-only-a-phone-can-do/), [how speed was measured](/blog/how-phonedriver-reads-a-screen-98x-faster-than-adb/), and [the token benchmark](/benchmarks/#tokens), which is already finished.
