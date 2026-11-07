---
title: "phonedriver vs mobile-mcp vs phone-harness vs adb: benchmarked on real phone jobs"
seo: phonedriver vs mobile-mcp vs phone-harness vs adb on real phone jobs
short: Benchmarked on real phone jobs
meta: The method and result slots for a benchmark of phonedriver, mobile-mcp, phone-harness and adb on nine jobs only a phone can do. Results pending.
description: The method and the slots for results of a benchmark that gives Claude nine jobs only a phone can do, with phonedriver, mobile-mcp, phone-harness and adb. Results are marked pending where the runs have not finished.
date: 2026-11-07
label: BENCHMARK
short_card: phonedriver vs mobile-mcp vs phone-harness vs adb on real phone jobs
card_sub: Nine jobs, independent checks, every result sourced. Results pending.
---

> **Status: the checks are built and proven, the agent runs are not finished.** Every number below that is a measurement comes from a finished run. Cells marked [RESULTS] are empty on purpose, and this post is updated in place when they fill. Nothing here is a projection.

Speed benchmarks tell you how fast a tool reads a screen. They do not tell you whether an agent finishes a job with it. For that you need jobs, a model, and a check that does not trust the model's word. This post is the comparison we are running, with its method written down before the results exist so that the results cannot shape it.

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

Pass counts and median total tokens, per job and harness. The two mixed jobs have an adb column.

| Job | phonedriver | mobile-mcp | phone-harness | adb |
|---|---|---|---|---|
| qa-signup | [RESULTS] | [RESULTS] | [RESULTS] | not applicable |
| qa-shop | [RESULTS] | [RESULTS] | [RESULTS] | not applicable |
| relay-code | [RESULTS] | [RESULTS] | [RESULTS] | not applicable |
| chrome-prefs | [RESULTS] | [RESULTS] | [RESULTS] | not applicable |
| clock-settings | [RESULTS] | [RESULTS] | [RESULTS] | not applicable |
| messages-pin-archive | [RESULTS] | [RESULTS] | [RESULTS] | not applicable |
| photos-crop | [RESULTS] | [RESULTS] | [RESULTS] | not applicable |
| wakelock-restrict (mixed) | [RESULTS] | [RESULTS] | [RESULTS] | [RESULTS] |
| log-code (mixed) | [RESULTS] | [RESULTS] | [RESULTS] | [RESULTS] |
| Jobs passed, of 9 | [RESULTS] | [RESULTS] | [RESULTS] | [RESULTS] |

Wall time, tool calls and the written reason for each failure: [RESULTS].

## Known biases, stated now

- **We are the author of one contestant.** phonedriver runs first, on its own, and a failure caused by a bug in phonedriver gets fixed before the next round. We have already fixed some: a dismissed approval prompt is not an answer, a field that refuses `set_text` is wiped with backspaces and then with select-all and paste, a formatted phone number now verifies, and the tool description now says that tools go one at a time. The other harnesses get no such rounds from us. When their failures look like setup problems rather than product limits, we will say which, and we will rerun them on a better setup before we publish a loss that is ours.
- **mobile-mcp has not yet had a fair run.** In our earlier token benchmark it failed every run for an environment reason on a tunnelled emulator, so that table is not a comparison. The jobs run needs a local emulator or phone for it.
- **Small samples.** Two passes per cell give a median of two. That is a description of what happened, not a rate.
- **One model, one device.** Claude Sonnet 5.5 on a tablet emulator. No physical phone.
- **Chrome and Messages change.** The jobs depend on app versions in the emulator image. The image is recorded with the results.

## What would change our mind

A job where adb finishes a core job we claimed it cannot would remove that job. A check that passes a wrong result would void every run of that job. A run where the agent reached the right state by a route we did not expect, for example by editing a database, would be reported and the check tightened. Raw run logs go in the repository next to the results so that you can read what the agent did.

Method and the other posts: [the jobs method](/blog/benchmarking-ai-agents-on-jobs-only-a-phone-can-do/), [how speed was measured](/blog/how-phonedriver-reads-a-screen-98x-faster-than-adb/), and [the token benchmark](/benchmarks/#tokens), which is already finished.
