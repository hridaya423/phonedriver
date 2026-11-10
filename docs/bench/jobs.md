# Real phone jobs benchmark (2026-11-07)

Status: Phase A (design and checks) and Phase B step 0 (checks proven on emulator-5558) are done. Runs, fixes and medians follow at "go runs".

Files: `bench/agent-eval/tasks.yaml` (sets `jobs` and `jobs-mixed`), `bench/agent-eval/jobs/jobs.py` (seed, check, clean), `bench/agent-eval/jobs/test_jobs.py`, testbed screens in `testbeds/android/.../Buggy.java`, runner `bench/agent-eval/run-harness.sh <harness> <task> <out>`. The prompt in `tasks.yaml` is the same for every harness.

## Rules for a job

- `jobs` (UI-only core, 7 tasks): the result cannot be produced by adb, `am`, `cmd`, `settings`, `content` or a browser. The reason is written per task below.
- `jobs-mixed` (2 tasks): the agent may use any route. Each needs a device-side read (shell) and a UI step, so a tool that holds both in one session needs fewer calls.
- Checks never read the agent's claim as proof. They read device state (private app state as root, appops, MediaStore, the contacts provider, the testbed event log). The two bug-report jobs also match the agent's written report against a planted-bug list (regexes, no LLM judge) and require the event log to show the bug paths were exercised.
- Cleanup removes every contact, SMS, picture, setting and appop a job created.

## Jobs

| Job | Set | Prompt (short) | Why adb or the web cannot do it | Check |
|---|---|---|---|---|
| qa-signup | jobs | Test the testbed Signup tab, report every bug with steps | The bugs exist only in how the screen behaves under use (a welcome line that drops the email's last character; the Create button ignores the tap that follows a rejected submit). No intent or command exercises a form's validation path. | Report matches both planted bugs and lists steps; log shows an invalid submit, then an ignored tap, then a success |
| qa-shop | jobs | Test the testbed Shop tab (quantity, totals, SAVE10 = 10%) | Same: total stale after Decrease, SAVE10 takes a flat 10.00. | Report matches both bugs and lists steps; log shows a quantity decrease and SAVE10 applied |
| relay-code | jobs | Copy the Relay tab's pickup code into a new contact "Locker 7" | The code is random per run, exists only in a TextView and the app's private storage. Shell use is detected in the tool log and fails the run. | Contacts provider: exactly one "Locker 7" whose phone equals `run-as ... cat files/relay_code` |
| chrome-prefs | jobs | DuckDuckGo as default search engine, "Always use secure connections" on | Chrome keeps both in its private `Preferences` file. No intent, `cmd` or `settings` key reaches them and the web cannot change a native browser's settings. | Root read of `Preferences`: `default_search_provider_data` is DuckDuckGo and `https_first_balanced_mode_enabled` (or `https_only_mode_enabled`) is true |
| clock-settings | jobs | Tokyo and London world clocks, snooze 15 min, silence after 5 min | World clocks and Clock settings live in Clock's private prefs; the AlarmClock intents set alarms and timers only. | Root read of Clock's prefs: Tokyo and London in the world-clock list, snooze 15, silence 5 |
| messages-pin-archive | jobs | Pin Bob Park's thread, archive Carol Diaz's, leave Alice Chen's | Pin and archive are Messages-private flags. Seeding is `adb emu sms send` (harness side); setting the flags has no command. | Root read of Messages' `bugle_db`: `conversation_pin` and `conversations.archive_status` per person |
| photos-crop | jobs | Crop jobs-harbor to a square, save a copy, keep the original | The Photos editor is the only crop tool on the image; a shell has no image editor. | MediaStore: original still 1200 wide; a new image added after the baseline, square, at most 800 px |
| wakelock-restrict | jobs-mixed | Find the app holding a wakelock, set its battery usage to Restricted | Shell finds the holder (`dumpsys power`: `jobs:drain`, package `dev.phonedriver.testbed`); Settings sets the restriction. A shell could also run `cmd appops set`, so this job rewards the agent that combines them rather than requiring both. | `cmd appops query-op RUN_ANY_IN_BACKGROUND ignore` against a baseline: only the testbed is newly restricted |
| log-code | jobs-mixed | Find the pickup code the testbed wrote to the device log, put it in a contact "Locker 9" | The code is only in logcat (shell); the contact is made in the Contacts UI. | Same contact check as relay-code, shell allowed |

## Dropped candidates and why

Contact add (`content insert` does it), calendar event (provider insert), dark mode and screen timeout (`cmd uimode`, `settings put`), alarm and timer (AlarmClock intents), SMS send (SENDTO intent plus a tap), Chrome live fact and the web shopping cart (a browser does them), the Uber-Eats-style order (needs sign-in). Maps transit time was not added: Maps on the web gives the same answer.

## Phase B step 0: checks proven by hand on emulator-5558 (2026-11-07)

Image: `google/sdk_gphone64_x86_64/emu64xa:17`, userdebug, a 2560x1600 tablet, reached through the 5038 tunnel. `su 0` works; `run-as dev.phonedriver.testbed` works (debug build). Each check was driven by hand with adb input (no agent): a pass state, and a fail state for every distinct failure it reports. Commands were `python3 jobs.py seed|check|clean <task>` with `SERIAL=emulator-5558`.

| Job | Pass | Fail cases shown | What the device showed and what changed |
|---|---|---|---|
| qa-signup | report with both bugs, steps, full log | vague report; no log | Both bugs reproduce by hand (terms error, tick, tap: nothing; third tap: `Welcome, ada@example.co!`). |
| qa-shop | report with both bugs | vague report; no log | Total stays `$36.00` after Decrease; SAVE10 gives `$14.00` at quantity 2 (flat 10). Fix: the report splitter cut sentences at the decimal point in `$36.00`, so a correct report failed; it now splits only at a full stop followed by space or a newline (unit test added). |
| relay-code | UI-made contact is read by the provider | no contact; wrong number; shell in the transcript | The code in the app's `files/relay_code` equals the code on screen. The Contacts app saves a signed-out contact with a null account; phone digits are compared by suffix since the editor may add a `+1`. |
| log-code | contact with the logged code | (same checks as relay-code) | The code is logged as `{"ev":"pickup_code"}` at screen open. |
| chrome-prefs | DuckDuckGo plus the switch on | no Preferences file; default prefs | Changed: Chrome 17-era keeps "Always use secure connections" in `https_first_balanced_mode_enabled`, not `https_only_mode_enabled`; either true passes. The `Preferences` file is written within 5 s of the toggle. Chrome first run needs two taps ("Stay signed out", "No thanks"). A missing file now fails with a message, not a traceback. |
| clock-settings | Tokyo, London, snooze 15, silence 5 | cleared Clock | Keys are `place_timezone_id_N` (`Asia/Tokyo`, `Europe/London`), `snooze_duration`, `auto_silence`. Changed: the old "any file mentions London" test would pass on a fresh Clock, because `home_time_zone` is `Europe/London` on this host; the check now reads only the world-clock list. |
| messages-pin-archive | Bob pinned, Carol archived | both reset | Messages opens signed out via "Use Messages without an account"; pin and archive are by long press. Pins live in `bugle_db` table `conversation_pin`, archive in `conversations.archive_status`; the old check looked for a pin column in `conversations` and would never have passed. Changed: the check joins the two tables. |
| photos-crop | cropped copy present | no copy; original removed | Photos opens signed out after a Back press on the backup screen. Edit, Crop, Square, Save copy produced `jobs-harbor~2.jpg` at 799x800, kept next to the 1200x800 original; the check allows a 1 px difference. Changed: cleanup now deletes `jobs-harbor*` through MediaStore. |
| wakelock-restrict | testbed restricted | testbed not restricted; another app restricted | `dumpsys power` shows `jobs:drain` for uid 10234 (`dev.phonedriver.testbed`). In Settings, App info, App battery usage, turning off "Allow background usage" sets `RUN_ANY_IN_BACKGROUND ignore`. Restricting releases the wakelock. |

Facts a run will meet:
- The local adb client cannot reach a tunnelled emulator's console (`adb emu` connects to local port 5558). Seeding SMS goes through `EMU_ADB="ssh win-ts /d/Android/Sdk/platform-tools/adb.exe"`, which `jobs.py emu` uses. It is the only thing that needs the SSH host besides the tunnel.
- Messages is seeded once: contacts Alice Chen, Bob Park and Carol Diaz and one SMS each stay on the emulator, and `seed`/`clean` only reset pins and archives (`jobs.py purge messages-pin-archive` removes everything). A "Spam protection" dialog was dismissed once.
- Contacts and Photos first-run prompts were dismissed once (Contacts notification prompt: "Don't allow").
- `uiautomator dump` fails with "could not get idle state" on the Contacts list screen. Anything that reads that screen through uiautomator (the `adb` harness) must retry; to look at in Phase B for phonedriver and the others.
- Left on the emulator: the three seeded contacts and SMS above, the installed testbed, a Chrome and Clock state cleared by each cleanup. Nothing else.

## Run budget

Core 7 + mixed 2 = 9 tasks. One pass per harness (phonedriver, phone-harness, mobile-mcp) is 27 runs; the second pass for medians doubles it. The bug-report and Messages/Photos jobs are the longest. Phase B runs phonedriver alone first, as asked.

## Phase B results (2026-11-07)

Setup: `claude -p` on the subscription, `claude-sonnet-5-5`, `ANTHROPIC_API_KEY` unset, emulator-5558 (a 2560x1600 tablet image, API 37) through the 5038 tunnel, one agent at a time. Each harness ran the 9 jobs twice (18 runs), reset and checked by `jobs.py` (root reads of app state, never the agent's reply). Core jobs (7): harness limited to its own tools, no adb. Mixed jobs (log-code, wakelock-restrict): any route allowed, `ALLOW_ADB=1`, and the adb harness runs only these two. Tokens are input + output + cache creation + cache read. Raw runs: `docs/bench/jobs.json` (`runs`, `medians`, `totals`); rebuilt with `python3 bench/agent-eval/jobs/summarize.py p4=... --md`.

phonedriver is the final binary (`126db3e`), runs `p4` and `p5`. phone-harness 0.3.3 ran as `h1`, `h2` (its qa-shop run in `h2` was invalid: a permission pattern blocked the tool, 7 s, 1 call; rerun as `h3`, which is what the table uses). mobile-mcp 1.0.16 (`@mobilenext/mobile-mcp`) ran through a private adb server on port 5097. The first rubric scored the report jobs by sentence; v2 reads headings and sections too and counts a message that stayed as a dead tap (the old rubric failed correct multi-line reports; rescoring is in `summarize.py`, `success_as_run` keeps the original verdict).

### Totals (sum of per-job medians)

![Real phone jobs: passes, tokens and time](../../.github/assets/jobs-light.svg)

| Harness | Passed | Tokens | Tool calls | Wall s |
|---|---|---|---|---|
| phonedriver | 18/18 | 977,780 | 123.5 | 466.5 |
| phone-harness | 14/18 | 1,272,617 | 101 | 1,946 |
| mobile-mcp | 18/18 | 4,892,241 | 270.5 | 910 |
| adb (mixed jobs only) | 4/4 | 144,669 | 17.5 | 94.5 |

### Per job

| Job | Harness | Passes | Median tokens | Median calls | Median wall s |
|---|---|---|---|---|---|
| log-code | adb | 2/2 | 66,158 | 7.5 | 45.5 |
| wakelock-restrict | adb | 2/2 | 78,510 | 10 | 49 |
| chrome-prefs | mobile-mcp | 2/2 | 595,815 | 33.5 | 179.5 |
| clock-settings | mobile-mcp | 2/2 | 843,472 | 41.5 | 112.5 |
| log-code | mobile-mcp | 2/2 | 129,150 | 8.5 | 103.5 |
| messages-pin-archive | mobile-mcp | 2/2 | 497,776 | 24.5 | 56.5 |
| photos-crop | mobile-mcp | 2/2 | 1,071,665 | 47 | 112 |
| qa-shop | mobile-mcp | 2/2 | 476,136 | 31.5 | 85.5 |
| qa-signup | mobile-mcp | 2/2 | 514,062 | 39.5 | 110 |
| relay-code | mobile-mcp | 2/2 | 611,976 | 35.5 | 122 |
| wakelock-restrict | mobile-mcp | 2/2 | 152,188 | 9 | 28.5 |
| chrome-prefs | phone-harness | 2/2 | 193,370 | 19 | 250.5 |
| clock-settings | phone-harness | 2/2 | 76,850 | 8.5 | 162 |
| log-code | phone-harness | 2/2 | 40,748 | 5.5 | 41.5 |
| messages-pin-archive | phone-harness | 2/2 | 45,825 | 5 | 57.5 |
| photos-crop | phone-harness | 2/2 | 103,776 | 12 | 237.5 |
| qa-shop | phone-harness | 1/2 | 338,406 | 13 | 392.5 |
| qa-signup | phone-harness | 1/2 | 246,327 | 14.5 | 459 |
| relay-code | phone-harness | 0/2 | 147,436 | 14 | 280 |
| wakelock-restrict | phone-harness | 2/2 | 79,878 | 9.5 | 65.5 |
| chrome-prefs | phonedriver | 2/2 | 104,302 | 13.5 | 60.5 |
| clock-settings | phonedriver | 2/2 | 95,116 | 13.5 | 48 |
| log-code | phonedriver | 2/2 | 59,470 | 8 | 38 |
| messages-pin-archive | phonedriver | 2/2 | 47,090 | 6 | 30 |
| photos-crop | phonedriver | 2/2 | 220,836 | 20.5 | 78 |
| qa-shop | phonedriver | 2/2 | 138,557 | 18 | 65 |
| qa-signup | phonedriver | 2/2 | 181,518 | 27 | 80 |
| relay-code | phonedriver | 2/2 | 69,994 | 9 | 38 |
| wakelock-restrict | phonedriver | 2/2 | 60,896 | 8 | 29 |

### What each harness failed at

- phone-harness, relay-code (0/2): it reads screens through uiautomator dumps, and the dump fails with "could not get idle state" on the Contacts list screen, so it could not see the Contacts app. Run 1 gave up after 17 calls, run 2 after 11, and asked the user whether to tap blind. The contact was never created.
- phone-harness, qa-signup (1/2) and qa-shop (1/2): pass 1 of each hit the run limit (601 s with 7 calls; 607 s with 20 calls) and failed; pass 2 passed (317 s with 22 calls; 178 s with 6 calls). Why pass 1 stalled was not diagnosed beyond the wall time.
- mobile-mcp: 18/18 but 4.9M tokens, 5x phonedriver. Every action returns a screenshot or an element list; photos-crop (1.07M) and clock-settings (843k) are the worst.
- phonedriver: 18/18. Before the rubric rescoring p4 qa-shop was recorded as a failure (a correct report with a "Bug 3: not a bug" heading); it is a pass under rubric v2 and the original verdict stays in `success_as_run`.

### Where phonedriver does not win

- Tokens: phone-harness is cheaper than phonedriver on photos-crop (104k against 221k), clock-settings (77k against 95k), log-code (41k against 59k), chrome-prefs is higher for phone-harness (193k against 104k), and messages-pin-archive is about equal (46k against 47k). Over all 9 jobs phonedriver is 23% lower in the sum only because phone-harness failed and spent tokens in the three jobs above.
- Wall time: phonedriver is lowest on 8 of 9 jobs; mobile-mcp is 0.5 s ahead on wakelock-restrict (28.5 s against 29 s). Even where a shell can do the work, adb took 45.5 s (log-code) and 49 s (wakelock-restrict) against 38 s and 29 s.
- Not fixed: Photos screens are screenshots (the editor has no tree), the unlabeled child button inside a labeled group needs a point tap, and the first tap on the Messages archive action replies "no element changed" because the list settles late.

### phonedriver fixes from this benchmark

| Fix | Commit | Before | After |
|---|---|---|---|
| A dismissed approval prompt (headless, no one to answer) returned "declined", a dead end; now it says what to retry | `ca5c56b`, D27 note | relay-code and qa-shop ended with the agent giving up on Place order | qa-shop completes; the agent retries with `autonomous: true` |
| `autonomous: "true"` as a string was rejected as an invalid type | `66b96f7` | retry failed with invalid type | accepted |
| Contacts' phone field refuses `set_text` and the error was mapped to "run doctor" | `aa3f4c3` (Java Ctrl+A plus paste; a backspace version was tried and failed on an unmappable key) | type failed, agent fell back to many taps | relay-code 112k tokens, 16 calls, 67 s (p1) to 70k, 9 calls, 38 s (p4/p5 median) |
| A formatted phone number (`(555) 010-0100`) failed verification of a typed value | `ca5c56b` | extra confirming reads | verified |
| `launch` takes an app name; the instructions and the tap description say tools go one at a time | `9e50691`, `ca5c56b` | agents opened with `devices` and `apps` | gone from the runs |
| Server instructions batch sentence removed | `126db3e` | 1 batch call in 18 runs after it | no effect, so removed |

Round 1 (`p1`, before these fixes) was 9/9 passed at 955,535 tokens, 572 s; the final `p5` was 9/9 at 932,417 tokens, 493 s and `p4` 9/9 (rubric v2) at 1,023,143 tokens, 440 s. The fixes mostly removed dead ends in single jobs (relay-code above); the sum of tokens did not move much, and run-to-run spread in qa-shop and photos-crop is larger than the difference.

## Rerun of phonedriver on the current build (2026-11-10)

Why: the totals above date from 7 November, and three days of changes went in after them. Only phonedriver was rerun. mobile-mcp is still 1.0.16 and phone-harness still 0.3.3, the versions measured above.

Setup: build `add1a2b`, three passes (`p6`, `p7`, `p8`), `claude -p` on the subscription, `claude-sonnet-5-5`, Claude Code 2.1.319. The agent and phonedriver ran on the Mac mini. The tablet emulator of 7 November no longer existed, so `emulator-5558` is a new AVD of the same image and size (`Pixel_Tablet_API_37`, 2560x1600, API 37, 4 GB RAM). Raw runs: `docs/bench/jobs.json`, passes `p6` to `p8`, and `rerun_2026_11_10`.

Result on the eight jobs that could be compared: 24 of 24 passed, and the sum of per-job median tokens is 1,066,097, against 916,883 for the same eight jobs on 7 November (16% more). mobile-mcp's sum for these eight jobs is 4,740,053, so the ratio is 4.4x, against 5.2x on 7 November.

| Job | Passed 7 Nov | Passed now | Tokens 7 Nov | Tokens now | Change | Outside the spread? |
|---|---|---|---|---|---|---|
| chrome-prefs | 2/2 | 3/3 | 104,302 | 138,521 | +32.8% | yes, higher |
| clock-settings | 2/2 | 3/3 | 95,116 | 96,562 | +1.5% | no |
| log-code | 2/2 | 3/3 | 59,470 | 53,333 | -10.3% | no |
| messages-pin-archive | 2/2 | 3/3 | 47,090 | 79,799 | +69.5% | no |
| photos-crop | 2/2 | 3/3 | 220,836 | 322,153 | +45.9% | no |
| qa-shop | 2/2 | 3/3 | 138,557 | 93,950 | -32.2% | no |
| qa-signup | 2/2 | 3/3 | 181,518 | 217,250 | +19.7% | no |
| relay-code | 2/2 | 3/3 | 69,994 | 64,529 | -7.8% | no |

"Outside the spread" means no run of one group falls inside the range of the other (`bench/agent-eval/jobs/compare.py`). With two and three passes this shows only large changes.

What the rerun says, plainly:

- The changes since 7 November did not lower tokens on these jobs. Seven of eight jobs moved inside the run-to-run spread. chrome-prefs is higher in all three passes (138,359, 138,521 and 527,834). Not diagnosed. One candidate: a cold Chrome launch now lists the first-run screen where it used to list an empty pager.
- Wall time is not comparable. The agent ran on a Mac mini that other work was loading, through two SSH tunnels. The times are in `jobs.json` and are not used.
- wakelock-restrict is excluded. On the new AVD the seeded `jobs:drain` wakelock was gone before the agent started in most runs (the first `dumpsys power` shows `Wake Locks: size=0`), so the job measured the seed, not the tool. One run with the lock in place passed with 55,251 tokens and 7 calls. The seed now waits for the lock before it presses Home, which was not enough.
- The rerun found a regression and it is fixed. In the first pass, qa-signup failed: since 8 November a plain `type` into a password field is refused, and the refusal said "Ask the user for the text". The agent stopped and asked for test passwords. The message now says to use `secret` and that a made-up test value needs no asking (`add1a2b`). The three passes above ran with that fix.
- Runs that hit a dropped adb connection were rerun: relay-code in `p6` and `p8`. The discarded files are kept outside the repository.

The emulator was the cause of the higher numbers, not the tool: see the next section.

## phonedriver and mobile-mcp on the same device, same day (2026-11-10)

Why: the rerun above ran on a starved emulator. The new AVD drew 2560x1600 with SwiftShader, Contacts was left in front redrawing, and the guest load was 7 on 4 cores. 17 launches answered "not settled", and one job met "Chrome isn't responding" seven times.

Setup: the same AVD started with `-gpu host -cores 8 -memory 8192`. `claude -p` on the subscription, `claude-sonnet-5-5`, Claude Code 2.1.319, agent and tools on the Mac mini, one agent at a time. phonedriver build `cfa37b2`, three passes (`p15` to `p17`). mobile-mcp 1.0.16 (its helper `mobilecli` 1.0.22), two passes (`m3`, `m4`), with adb allowed on the two mixed jobs as on 7 November. phone-harness 0.3.3, two passes (`h4`, `h5`), and adb on the two mixed jobs, two passes (`a3`, `a4`), ran later the same day after the emulator was started again. Raw runs: `docs/bench/jobs.json`, `same_device_2026_11_10`.

| | Passed | Tokens (sum of job medians) | Tool calls | Wall s |
|---|---|---|---|---|
| phonedriver `cfa37b2` | 27/27 | 657,542 | 99 | 306 |
| mobile-mcp 1.0.16 | 17/18 | 4,567,200 | 276.5 | 523 |
| phone-harness 0.3.3 | 14/18 | 1,127,495 | 104.5 | 1,360.5 |
| adb (2 jobs) | 4/4 | 203,751 | 20.5 | 83 |

mobile-mcp used 6.9x the tokens of phonedriver (5.0x on 7 November). Its one failure: chrome-prefs in `m3` ended with "Always use secure connections" off.

| Job | phonedriver passed | phonedriver tokens | mobile-mcp passed | mobile-mcp tokens | Ratio |
|---|---|---|---|---|---|
| qa-signup | 3/3 | 121,854 | 2/2 | 549,354 | 4.5x |
| qa-shop | 3/3 | 92,570 | 2/2 | 473,095 | 5.1x |
| relay-code | 3/3 | 49,126 | 2/2 | 574,428 | 11.7x |
| chrome-prefs | 3/3 | 98,576 | 1/2 | 1,052,348 | 10.7x |
| clock-settings | 3/3 | 95,962 | 2/2 | 691,926 | 7.2x |
| messages-pin-archive | 3/3 | 45,240 | 2/2 | 315,560 | 7.0x |
| photos-crop | 3/3 | 61,477 | 2/2 | 636,680 | 10.4x |
| wakelock-restrict | 3/3 | 38,775 | 2/2 | 122,772 | 3.2x |
| log-code | 3/3 | 53,962 | 2/2 | 151,038 | 2.8x |

phone-harness used 1.7x the tokens of phonedriver and 4.4x the time, and failed four runs: relay-code twice (no contact was saved), wakelock-restrict once, and qa-signup once (stopped by the 10-minute limit; its tokens are summed from the transcript). It used fewer tokens than phonedriver on two jobs (clock-settings and messages-pin-archive).

| Job | phonedriver passed | phonedriver tokens | phone-harness passed | phone-harness tokens |
|---|---|---|---|---|
| qa-signup | 3/3 | 121,854 | 1/2 | 305,766 |
| qa-shop | 3/3 | 92,570 | 2/2 | 136,428 |
| relay-code | 3/3 | 49,126 | 0/2 | 161,444 |
| chrome-prefs | 3/3 | 98,576 | 2/2 | 151,782 |
| clock-settings | 3/3 | 95,962 | 2/2 | 79,931 |
| messages-pin-archive | 3/3 | 45,240 | 2/2 | 42,452 |
| photos-crop | 3/3 | 61,477 | 2/2 | 65,703 |
| wakelock-restrict | 3/3 | 38,775 | 1/2 | 54,694 |
| log-code | 3/3 | 53,962 | 2/2 | 129,294 |

adb on its two jobs: 4 of 4 with 203,751 tokens; phonedriver's sum for the same two jobs is 92,737.

phonedriver against its own 7 November runs (`p4`, `p5`): 657,542 tokens against 977,780 (33% fewer), 99 calls against 123.5. The device is not the same as on 7 November, so part of this is the device.

Earlier builds on the same device, same day: `add1a2b` (`p9` to `p11`) 27/27 with 808,241 tokens and 114 calls; `778b6e7` (`p12` to `p14`) 27/27 with 781,132 and 108. The largest single change is photos-crop, 151,431 to 61,477: the reply to the Edit tap now lists the editor's tabs (`cfa37b2`), and the emulator was not restarted before `p15` to `p17`, so Photos did not show its "Update" prompt in them. The two effects are not separated.

What was thrown away, and why:

- Two chrome-prefs runs (`p9`, `p11`) lost the adb tunnel mid-job and were rerun.
- The first qa-shop run of `p14` ran as the emulator process ended. The emulator was restarted and the run repeated.
- The first 5 mobile-mcp runs went through a tunnelled adb server. `mobilecli` forwards a port to its device server, which a remote adb server binds on the wrong machine, so every tool call failed. The Mac mini was given its own adb server and the passes started again.
- A whole second mobile-mcp pass ran with the guest load between 9 and 16 and failed five jobs for device reasons. The emulator was rebooted and the pass repeated as `m4`.
- Photos shows an "Update" prompt after each emulator restart. Both tools met it; it adds calls to photos-crop.

- The first phone-harness run met Android's one-time taskbar tutorial after a cold boot. The tutorial was cleared and the pass started again.

The discarded files are kept outside the repository. Claims outside this file say "6.9x" and name 10 November. The chart and `totals` in `jobs.json` are this section's numbers; the 7 November totals are kept as `totals_2026_11_07`.

### Notes for the site

The site's blog text now says a field that refuses `set_text` is selected with Ctrl+A and pasted over, which is what `TextInput.java` and `Input.java` do. The results are on the benchmarks page and in blog post 4.
