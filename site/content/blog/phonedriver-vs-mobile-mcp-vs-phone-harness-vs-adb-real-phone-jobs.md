---
title: "phonedriver vs mobile-mcp vs phone-harness vs adb: benchmarked on real phone jobs"
seo: phonedriver vs mobile-mcp vs phone-harness vs adb on real phone jobs
short: Benchmarked on real phone jobs
meta: The method and results of a benchmark of phonedriver, mobile-mcp, phone-harness and adb on nine jobs only a phone can do, rerun on 10 November 2026. Results in.
description: The method and results of a benchmark that gives Claude nine jobs only a phone can do, with phonedriver, mobile-mcp, phone-harness and adb, rerun on 10 November 2026 on one emulator. phonedriver ran each job three times and the others twice.
date: 2026-11-07
label: BENCHMARK
short_card: phonedriver vs mobile-mcp vs phone-harness vs adb on real phone jobs
card_sub: Nine jobs, independent checks, every result sourced. 27 of 27 for phonedriver, 17 of 18 for mobile-mcp, 14 of 18 for phone-harness.
---

> **Status: finished.** The numbers were rerun on 10 November 2026 on one emulator, with all four harnesses on the same device. phonedriver ran each job three times and the others twice. Every number below that is a measurement is the median of those runs, and the raw runs are in the repository. Nothing here is a projection. The first results, from 7 November, are kept below where the comparison between the two dates is the point.

Speed benchmarks tell you how fast a tool reads a screen. They do not tell you whether an agent finishes a job with it. For that you need jobs, a model, and a check that does not trust the model's word. This post is the comparison I ran, with its method written down before the results existed so that the results could not shape it.

## The short version

Sums of per-job medians over nine jobs, with Claude Sonnet 5.5 on one Android emulator, 10 November 2026:

| Tool | Runs passed | Tokens | Tool calls | Wall time |
|---|---|---|---|---|
| phonedriver | 27 of 27 | 657,542 | 99 | 306 s |
| phone-harness 0.3.3 | 14 of 18 | 1,127,495 | 104.5 | 1,360.5 s |
| mobile-mcp 1.0.16 | 17 of 18 | 4,567,200 | 276.5 | 523 s |

mobile-mcp used 6.9 times the tokens of phonedriver, and phone-harness 1.7 times. phonedriver did not win everywhere: phone-harness used fewer tokens on two of the nine jobs, and mobile-mcp finished one job faster. I wrote one of these tools and the test set, so read [the biases](#known-biases-stated-now) before you quote a number.

## What is being compared

Four ways for Claude Sonnet 5.5 to reach a phone:

- **phonedriver** (build `cfa37b2`), as an MCP server.
- **mobile-mcp** 1.0.16, as an MCP server.
- **phone-harness** 0.3.3, as a CLI that runs Python scripts.
- **Raw adb**, as a shell tool limited to `adb`.

The jobs are the same for every harness, and so is the prompt, which is stored in `bench/agent-eval/tasks.yaml`. Each run starts from a reset device, runs `claude -p` from an empty directory with strict MCP configuration, no slash commands, no settings sources, no session persistence and a 40-turn cap, and then a script checks the device. Runs use the Claude subscription, never an API key. The device is one 2560x1600 Pixel Tablet emulator (API 37), started with `-gpu host -cores 8 -memory 8192`. The agent and the tools ran on a Mac mini, one agent at a time.

## The nine jobs

Seven jobs are UI-only: the result cannot be produced by adb, `am`, `cmd`, `settings`, `content` or a browser, and the reason is written per job. Two are mixed: any route is allowed, and each needs a device-side read and a UI step, so a tool that holds both in one session should need fewer calls. The full table, with why adb cannot do each, is on [the benchmarks page](/benchmarks/#jobs), and the reasoning behind the rule is in [Benchmarking AI agents on jobs only a phone can do](/blog/benchmarking-ai-agents-on-jobs-only-a-phone-can-do/).

Raw adb is not a competitor on the seven UI-only jobs by construction. Those jobs were chosen because adb cannot do them, so its row there is expected to be empty, and I will not count that as a win. It competes on the two mixed jobs, where any route is allowed.

## How a run is scored

A run passes if the check passes. A check reads device state, never the agent's claim: the contacts provider, Messages' database, Clock's and Chrome's private preferences as root, `appops`, MediaStore, or the testbed app's event log. The two bug-report jobs also match the written report against a list of planted bugs with regular expressions, no model as judge, and require the event log to show that the bug paths were exercised. A job that forbids a shell fails the run if the tool log shows shell use.

Each check was proven by hand before any agent ran: a pass state, and a fail state for every way it can fail. That found real problems in my checks. A report splitter cut sentences at the decimal point in `$36.00`, so a correct report failed. A "London is in the world clock list" test would pass on a fresh Clock, because the home time zone on the host is `Europe/London`. A Messages check looked for a pin column that does not exist. All three were fixed before a model touched them.

For every run I record whether it passed, tool calls, total tokens (input, output, cache creation and cache read, the definition from my earlier agent evaluation) and wall time. Per job and harness I report the median of the runs and the pass count, and I show every run.

## Results

Pass counts and medians (total tokens, tool calls, wall time) per job and harness. The two mixed jobs have an adb column.

| Job | phonedriver | mobile-mcp | phone-harness | adb |
|---|---|---|---|---|
| qa-signup | 3/3, 121,854 tokens, 23 calls, 50 s | 2/2, 549,354 tokens, 45 calls, 95 s | 1/2, 305,766 tokens, 20.5 calls, 477 s | not applicable |
| qa-shop | 3/3, 92,570 tokens, 15 calls, 41 s | 2/2, 473,095 tokens, 33.5 calls, 63.5 s | 2/2, 136,428 tokens, 13.5 calls, 242.5 s | not applicable |
| relay-code | 3/3, 49,126 tokens, 8 calls, 31 s | 2/2, 574,428 tokens, 35.5 calls, 68 s | 0/2, 161,444 tokens, 14.5 calls, 190 s | not applicable |
| chrome-prefs | 3/3, 98,576 tokens, 13 calls, 45 s | 1/2, 1,052,348 tokens, 52.5 calls, 99 s | 2/2, 151,782 tokens, 16.5 calls, 140.5 s | not applicable |
| clock-settings | 3/3, 95,962 tokens, 13 calls, 39 s | 2/2, 691,926 tokens, 44 calls, 61.5 s | 2/2, 79,931 tokens, 9 calls, 84 s | not applicable |
| messages-pin-archive | 3/3, 45,240 tokens, 6 calls, 21 s | 2/2, 315,560 tokens, 17.5 calls, 33.5 s | 2/2, 42,452 tokens, 5.5 calls, 37 s | not applicable |
| photos-crop | 3/3, 61,477 tokens, 8 calls, 31 s | 2/2, 636,680 tokens, 33 calls, 60.5 s | 2/2, 65,703 tokens, 8 calls, 77 s | not applicable |
| wakelock-restrict (mixed) | 3/3, 38,775 tokens, 5 calls, 17 s | 2/2, 122,772 tokens, 7.5 calls, 20 s | 1/2, 54,694 tokens, 7.5 calls, 43.5 s | 2/2, 72,424.5 tokens, 10 calls, 40 s |
| log-code (mixed) | 3/3, 53,962 tokens, 8 calls, 31 s | 2/2, 151,038 tokens, 8 calls, 22 s | 2/2, 129,294 tokens, 9.5 calls, 69 s | 2/2, 131,326.5 tokens, 10.5 calls, 43 s |
| Total (sum of per-job medians) | 27/27 runs, 657,542 tokens, 99 calls, 306 s | 17/18 runs, 4,567,200 tokens, 276.5 calls, 523 s | 14/18 runs, 1,127,495 tokens, 104.5 calls, 1,360.5 s | 4/4 runs, 203,751 tokens, 20.5 calls, 83 s (2 mixed jobs only) |

Each cell is passes out of runs (three for phonedriver, two for the others), then the median of those runs. The total adds the per-job medians, so phonedriver's 99 calls is a sum of medians and not a count of calls in any one run.

mobile-mcp used 6.9 times the tokens of phonedriver. phone-harness used 1.7 times the tokens and 4.4 times the time. On the two mixed jobs, adb used 203,751 tokens and phonedriver 92,737.

### What changed since 7 November

On 7 November phonedriver passed 18 of 18 runs with 977,780 tokens, 123.5 calls and 466.5 s. mobile-mcp passed 18 of 18 with 4,892,241 tokens, which was 5.0 times phonedriver. phone-harness passed 14 of 18 with 1,272,617 tokens and 1,946 s. On 10 November phonedriver used 657,542 tokens, 33% fewer, and 99 calls.

Part of that gap is the device. The tablet emulator of 7 November no longer existed, so I made a new one of the same image and size. A first rerun of phonedriver on it, with software rendering, ran on a starved emulator: 17 launches answered "not settled", and phonedriver's tokens on eight comparable jobs were 16% higher than on 7 November. Started with `-gpu host -cores 8 -memory 8192`, the same device gave the numbers above. On that device, earlier builds of phonedriver took 808,241 tokens (`add1a2b`) and 781,132 (`778b6e7`). The largest single change is photos-crop, from 151,431 to 61,477 tokens: the reply to the Edit tap now lists the editor's tabs, and the emulator was not restarted before the last passes, so Photos did not show its "Update" prompt. I did not separate those two effects.

### Where phonedriver does not win

- **phone-harness is cheaper on two jobs.** clock-settings is 79,931 tokens against phonedriver's 95,962, and messages-pin-archive 42,452 against 45,240. photos-crop is close (65,703 against 61,477). phonedriver is cheaper on the other six.
- **mobile-mcp is faster on log-code** (22 s against 31 s). phonedriver has the lowest median wall time on the other 8 jobs.
- **A shell does not need phonedriver for the mixed jobs.** adb passed 4 of 4 on log-code and wakelock-restrict, in 43 s and 40 s against phonedriver's 31 s and 17 s, using 131k and 72k tokens against 54k and 39k.
- **Failed runs are in phone-harness's sum.** Four of its 18 runs failed and still spent tokens, and one of them, a qa-signup run, reached the 10-minute limit. On the jobs it completed, it is often close to phonedriver or cheaper.

### What each harness failed at

- **phone-harness, relay-code, 0 of 2.** No contact was saved in either run. On 7 November I saw why: it reads screens through uiautomator dumps, and the dump fails with "could not get idle state" on the Contacts list, so it could not see the Contacts app. I did not diagnose the 10 November runs further.
- **phone-harness, qa-signup and wakelock-restrict, 1 of 2 each.** The failed qa-signup run stopped at the 10-minute limit (601 s, 24 calls, 421,160 tokens summed from the transcript). The failed wakelock-restrict run (7 calls, 46 s) I did not diagnose.
- **mobile-mcp, chrome-prefs, 1 of 2.** The run ended with "Always use secure connections" off. mobile-mcp passed the other 17 runs but spent 4.6M tokens, because every action returns a screenshot or an element list. chrome-prefs (1.05M), clock-settings (692k) and photos-crop (637k) are the worst.
- **phonedriver passed all 27.** The rubric for the two bug-report jobs reads headings and sections as well as sentences. A 7 November run that the first version of the rubric scored as a failure was a correct report with a "Bug 3: not a bug" heading, and passes under the second version. The original verdict is kept in `success_as_run` in the raw data.

Raw runs, medians and totals: [`docs/bench/jobs.json`](https://github.com/hridaya423/phonedriver/blob/main/docs/bench/jobs.json) (`same_device_2026_11_10`; the 7 November totals are kept as `totals_2026_11_07`) and the write-up in `docs/bench/jobs.md`. Setup: `claude -p` on a subscription, Claude Sonnet 5.5, a 2560x1600 tablet emulator.

### What I threw away

- Two phonedriver chrome-prefs runs lost the adb tunnel mid-job, and one qa-shop run started as the emulator process ended. Each was rerun.
- The first five mobile-mcp runs went through a tunnelled adb server, where its helper cannot reach its device server, so every tool call failed. I gave the Mac mini its own adb server and started again.
- A whole second mobile-mcp pass ran with the emulator overloaded and failed five jobs for device reasons. I rebooted the emulator and repeated the pass.
- The first phone-harness run met Android's one-time taskbar tutorial after a cold boot. I cleared it and started the pass again.

The discarded files are kept outside the repository.

## Known biases, stated now

- **I am the author of one contestant.** phonedriver runs first, on its own, and a failure caused by a bug in phonedriver gets fixed before the next round. I have already fixed some: a dismissed approval prompt is not an answer, a field that refuses `set_text` is selected with Ctrl+A and pasted over, a formatted phone number now verifies, and the tool description now says that tools go one at a time. The rerun on 10 November found one more: a refusal to type into a password field told the agent to ask the user for the text, and the agent stopped and asked for test passwords. The message now says to use `secret`, and the passes reported here ran with that fix. The other harnesses get no such rounds from me. When their failures look like setup problems rather than product limits, I will say which, and I will rerun them on a better setup before I publish a loss that is mine.
- **mobile-mcp needed a different setup.** In my earlier token benchmark it failed every run for an environment reason on a tunnelled emulator. Here it ran through a private adb server on the same machine as the agent and passed 17 of 18 runs, so its earlier numbers are not comparable.
- **Small samples.** Two passes per cell give a median of two, and three give a median of three. That is a description of what happened, not a rate.
- **The device changed between dates.** The 10 November numbers come from a new emulator of the same image, so part of phonedriver's drop since 7 November is the device.
- **One model, one device.** Claude Sonnet 5.5 on a tablet emulator. No physical phone.
- **Chrome and Messages change.** The jobs depend on app versions in the emulator image. The image is recorded with the results.

## What would change my mind

A job where adb finishes a core job I claimed it cannot would remove that job. A check that passes a wrong result would void every run of that job. A run where the agent reached the right state by a route I did not expect, for example by editing a database, would be reported and the check tightened. Raw run logs go in the repository next to the results so that you can read what the agent did.

Method and the other posts: [the jobs method](/blog/benchmarking-ai-agents-on-jobs-only-a-phone-can-do/), [how speed was measured](/blog/how-phonedriver-reads-a-screen-170x-faster-than-adb/), and [the token benchmark](/benchmarks/#tokens), which is already finished.
