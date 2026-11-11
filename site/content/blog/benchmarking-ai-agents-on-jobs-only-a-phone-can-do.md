---
title: Benchmarking AI agents on jobs only a phone can do
short: Benchmarking agents on jobs only a phone can do
meta: How to benchmark phone agents fairly: jobs a shell cannot do, checks from device state, no model as judge, and why adb and web searches are excluded.
description: How to design a phone-agent benchmark that is fair to every tool. Jobs a shell cannot do, checks from device state, no model as judge, and why adb and web searches are excluded from the core set.
date: 2026-11-07
label: METHOD
short_card: Benchmarking AI agents on jobs only a phone can do
card_sub: UI-only jobs, independent checks, and what I threw out.
---

A phone benchmark has an obvious way to go wrong. If the tasks can be done without a phone, the tool that is best at not using the phone wins. "Set an alarm" can be done with one intent. "Turn on dark mode" is `cmd uimode night yes`. "What year was Grace Hopper born" is a web search. Run those through a phone harness and you are measuring how fast the model finds the shortcut.

My first agent evaluation had tasks of exactly that kind, and it showed the problem. In my token run, raw adb finished the alarm task in 2 calls and 18,607 tokens with the `SET_ALARM` intent. That is a fine result for adb and tells you nothing about screens. So I threw those tasks out and wrote a benchmark around jobs where the screen is the only way in.

## The rule: the job must need the phone

A job goes in the core set only if the result cannot be produced by adb, `am`, `cmd`, `settings`, `content` or a browser. The reason is written per job, in the open, so that anyone can argue with it.

- Chrome keeps the default search engine and "Always use secure connections" in a private `Preferences` file. No intent, `cmd` or `settings` key reaches them, and the web cannot change a native browser's settings.
- World clocks and snooze live in Clock's private preferences. The alarm intents set alarms and timers only.
- Pinning and archiving a Messages thread are Messages' own flags. Seeding the threads is a host-side emulator command. Setting the flags has no command.
- Cropping a photo needs the Photos editor. A shell has no image editor.
- A pickup code shown in a testbed app is random per run and exists in a text view and the app's private storage. Reading the private storage is a shell move, and the job fails the run if the tool log shows shell use.
- Two bug-report jobs hide bugs that only show under use: a welcome line that drops the last character of an email, a Create button that ignores the tap after a rejected submit, a stale total after a quantity decrease, a discount that takes a flat 10.00. No command exercises a form's validation path.

I keep two mixed jobs on purpose. In those, any route is allowed. One needs a device-side read and a UI step: find the app holding a wakelock (a shell reads `dumpsys power`) and restrict its battery usage in Settings. The other: find a pickup code in the device log and put it in a contact. They reward a tool that holds both abilities in one session, and they are the only place raw adb competes.

## What I dropped, and why

The dropped list is part of the method. A contact add is `content insert`. A calendar event is a provider insert. Dark mode and screen timeout are `cmd uimode` and `settings put`. An alarm or timer is an AlarmClock intent. Sending an SMS is a `SENDTO` intent plus a tap. A live fact from Chrome, and a web shopping cart, are things a browser does. An Uber Eats style order needs sign-in. A Maps transit time gives the same answer on the web, so it was not added.

The same logic excludes web searches. A job whose answer is on a web page measures the agent's ability to find the page, and every harness with a browser tool, or none, will tie there.

## Checks that do not trust the agent

The check for a job reads the device, not the transcript. Sources: the contacts provider, Messages' `bugle_db` as root, Clock's and Chrome's private preferences as root, `cmd appops`, MediaStore, and the testbed app's event log, which the app writes as JSON lines when a screen is used.

- A contact check compares phone digits by suffix, because the editor may add a `+1`.
- A crop check wants the original still 1200 wide and a new image, added after a baseline, that is square and at most 800 px, within 1 px.
- The restriction check takes `RUN_ANY_IN_BACKGROUND` state against a baseline and passes only if the testbed alone is newly restricted.
- The bug-report checks match the written report against a list of planted bugs with regular expressions. No model grades. They also require the event log to show an invalid submit, then an ignored tap, then a success, so a report invented from the screen's text does not pass.

Cleanup removes every contact, SMS, picture, setting and app-op a job created, so runs do not leak into each other.

## Prove the check before the agent

This is the part I would repeat on any benchmark. Before a model ran, every check was driven by hand with `adb input` against the emulator: a pass state and a fail state for every distinct way it reports a failure. Every check found something:

- The report splitter cut sentences at the decimal point in `$36.00`, so a correct qa-shop report failed. It now splits at a full stop followed by a space or newline.
- The clock check passed on a freshly reset Clock because the host's home time zone is `Europe/London`. It now reads only the world-clock list.
- The Messages check looked for a pin column in `conversations`. Pins live in a separate table, so it would never have passed. It now joins the two tables.
- A missing Chrome `Preferences` file crashed the checker with a traceback. It now fails with a message.
- The Chrome in the emulator image keeps the secure-connections switch in `https_first_balanced_mode_enabled`, not the older `https_only_mode_enabled`. Either true passes.

A check that is wrong in one direction gives some tool a free pass, and in the other direction fails the tool that did the work. I would not have noticed either from the agent runs alone.

## What the jobs do not cover

Nine jobs on one tablet emulator, Android only. Chrome, Messages, Photos and Contacts are what the emulator image ships, so results depend on that image, and it is recorded with them. There is no iOS job, no physical phone and no sign-in flow. `uiautomator dump` fails on the Contacts list ("could not get idle state"), which will cost a harness that depends on it, and that is a property of the tool and not of the job. I write such things next to the results and do not remove them.

Results are in [the comparison post](/blog/phonedriver-vs-mobile-mcp-vs-phone-harness-vs-adb-real-phone-jobs/), rerun on 10 November 2026 on one emulator, and the job table with its checks is on [the benchmarks page](/benchmarks/#jobs). Every run, with its tokens, calls, time and verdict, is in [`jobs.json`](/benchmarks/data/jobs.json). The jobs, seeds and checks are in `bench/agent-eval/` in [the repository]({{REPO}}), so you can add a job I missed or prove one of mine wrong.
