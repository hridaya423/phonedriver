---
title: "phonedriver vs mobile-mcp vs phone-harness vs adb: benchmarked on real phone jobs"
seo: phonedriver vs mobile-mcp vs phone-harness vs adb on real phone jobs
short: Benchmarked on real phone jobs
meta: Nine real Android jobs run with Claude Sonnet 5.5 across phonedriver, mobile-mcp, phone-harness, and adb—why context compounding creates a 6.9x token gap.
description: Nine jobs only a phone can do, run across four tools and checked against the phone's own databases. 27/27 for phonedriver at 6.9x fewer tokens than mobile-mcp, plus where it lost.
date: 2026-11-07
label: BENCHMARK
short_card: phonedriver vs mobile-mcp vs phone-harness vs adb on real phone jobs
card_sub: Nine jobs, 6.9x fewer tokens than mobile-mcp, and the two jobs phone-harness won.
updated: 2026-11-20
---

Micro-benchmarks are dangerously easy to admire. Shaving 200 milliseconds off a tap looks lovely in a chart and barely registers when an LLM spends three seconds thinking between moves.

What actually empties your wallet on a phone session is **turns** and **context bloat**: how often the agent gets lost, re-reads the screen, or burns half a million tokens toggling two Chrome settings. So I gave Claude Sonnet 5.5 nine Android jobs where the screen is the only door in, ran them through phonedriver, mobile-mcp, and phone-harness (plus raw `adb` on two mixed jobs), and inspected the phone's own databases afterward.

## The short version

Sums of per-job medians across all nine jobs on one Android 17 emulator (10 November 2026):

| Tool | Passed | Tokens | Tool calls | Wall time |
|---|---|---|---|---|
| **phonedriver** | **27 / 27** | **657,542** | **99** | **306 s** |
| phone-harness 0.3.3 | 14 / 18 | 1,127,495 | 104.5 | 1,360.5 s |
| mobile-mcp 1.0.16 | 17 / 18 | 4,567,200 | 276.5 | 523 s |

mobile-mcp spent **6.9x** the tokens of phonedriver; phone-harness spent **1.7x** and took **4.4x** as long.

It wasn't a clean sweep. **phone-harness beat phonedriver on tokens in two of the nine jobs**, and **mobile-mcp beat it on wall time in one**. I also wrote both one of the tools and the test set—skim [Known biases](#known-biases) before quoting a headline.

## Why two modest factors multiply into 6.9x

Both phonedriver and mobile-mcp are accessibility-first: mobile-mcp reads the native accessibility tree too and saves screenshots for when it needs them. So the 6.9x gap isn't "text vs. pixels."

It's geometry. Every LLM turn re-reads the whole conversation behind it, so total tokens grow like a triangle, not a flat bar. Across these nine jobs, mobile-mcp made **2.8x as many tool calls** (276.5 vs. 99), and each call carried **2.5x as many tokens** (~16,500 vs. ~6,600) because actions return full element lists or screenshots instead of three-line settled diffs. Multiply 2.8 by 2.5 and you land right on 6.9x. (Tokens here sum input, output, cache creation, and cache read, so it is a total-token ratio rather than a straight API bill ratio.)

::figure[token-snowball]

## How I cut phonedriver's own token bloat first

phonedriver didn't start ahead. In an earlier three-task shootout, it actually **lost** to phone-harness on tokens (413,534 vs. 337,059). Reading those six transcripts turn by turn exposed two embarrassing leaks:

1. **Tool schemas are rent you pay every turn.** Across a whole run, phonedriver's actual tool replies totaled only 3,000 to 11,000 tokens. The fixed prefix—system prompt plus 14 tool schemas—was **59% to 81% of the entire run** (4,204 tokens per turn vs. phone-harness's much slimmer CLI). Trimming schema boilerplate, repeated `device` properties, and unused flags cut `tools/list` in half (10,987 to 5,107 chars) and dropped the tool tax to 2,768 tokens.
2. **Incomplete replies buy extra turns.** Fifteen of phonedriver's 36 turns were wasted fetching what the previous reply left out. Tapping "Add alarm" settled pixels before Android's accessibility tree caught up, replied with removals only, and forced the agent into a screenshot. Reading the tree until two reads agree closed that gap.

I also built a `batch` verb and added a stern sentence in the MCP instructions telling agents to batch multi-step taps. Across 18 runs, Claude used `batch` **once**. I deleted the sentence.

## Where phonedriver lost, and what tripped the others

- **phone-harness beat phonedriver on two jobs:** `clock-settings` (79,931 vs. 95,962 tokens) and `messages-pin-archive` (42,452 vs. 45,240), and nearly tied `photos-crop` (65,703 vs. 61,477). Its overall total only climbs because 4 of its 18 runs failed—including a `qa-signup` run that hit the 10-minute wall (601 s, 421k tokens) and 0/2 on `relay-code`, where `uiautomator dump` choked on Contacts with `"could not get idle state"`.
- **mobile-mcp beat phonedriver on wall time on `log-code`** (22 s vs. 31 s). It passed 17 of 18 runs—leaving `"Always use secure connections"` off in one `chrome-prefs` run—but multi-step settings screens got expensive fast (`chrome-prefs` alone crossed 1.05M tokens).
- **Raw adb doesn't need any of us for shell work.** On the two mixed jobs (`wakelock-restrict` and `log-code`), adb passed 4/4 in 203,751 tokens vs. phonedriver's 92,737. When a job *does* have a shell shortcut (like setting an alarm), raw adb clears it in two calls and 18k tokens.

<span id="known-biases-stated-now"></span>

## Known biases

- **I wrote one of the contestants.** When phonedriver hit bugs in earlier passes—a cancelled approval form reading like a refusal, a field ignoring `set_text`, a password refusal making the agent ask for a test password—I fixed phonedriver. The other tools didn't get bugfix passes from me.
- **Small samples, wide spread.** Two or three passes per job show big gaps, not delicate percentages. On `log-code`, phonedriver's three runs spanned 38,396 to 116,768 tokens; on `relay-code`, mobile-mcp spanned 290,633 to 858,223.
- **The emulator changed between dates.** Between 7 November (977,780 tokens, 5.0x vs. mobile-mcp) and 10 November (657,542 tokens, 6.9x), I rebuilt the emulator with `-gpu host -cores 8 -memory 8192`, so part of that 33% drop is a healthier emulator, not just code fixes.
- **One model, Android only, pre-0.1.5.** Every run used Claude Sonnet 5.5 on one tablet emulator before 0.1.5 shipped. No iOS, no physical phone.

If you maintain mobile-mcp or phone-harness and think I wired your tool up wrong, [open an issue]({{REPO}}/issues) and I will rerun it.

:::receipts Full setup, discarded runs & raw data
- **Harness setup:** `claude -p` (Claude Code 2.1.319, Claude Sonnet 5.5 on subscription, no API key), empty directory, strict MCP config, 40-turn cap, reset device between every run. Device: 2560×1600 `Pixel_Tablet_API_37` (`-gpu host -cores 8 -memory 8192`) on a Mac mini, one agent at a time. Versions: phonedriver `cfa37b2` (3 passes), mobile-mcp `1.0.16` (2 passes, local adb server), phone-harness `0.3.3` (2 passes).
- **Discarded runs (kept outside the repo):** 2 phonedriver `chrome-prefs` runs (adb tunnel dropped) and 1 `qa-shop` run (emulator process died); 5 initial mobile-mcp runs (ran through a tunnelled adb server where its helper couldn't bind—fixed by giving it a local adb server) and 1 overloaded-emulator mobile-mcp pass (guest load 9–16); 1 phone-harness run that met Android's one-time taskbar tutorial after cold boot.
- **Scoring & method:** Every check reads device state directly—no LLM judge. See [Benchmarking AI agents on jobs only a phone can do](/blog/benchmarking-ai-agents-on-jobs-only-a-phone-can-do/#prove-the-check-before-the-agent) and [the benchmarks page](/benchmarks/#jobs).
- **Raw files:** [`jobs.json`](/benchmarks/data/jobs.json) (`same_device_2026_11_10`; 7 November totals preserved under `totals_2026_11_07`) and [`docs/bench/jobs.md`]({{REPO}}/blob/main/docs/bench/jobs.md).
:::
