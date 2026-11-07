# Real phone jobs benchmark (2026-11-07)

Status: Phase A (design and checks). No device was touched. Phase B (runs, fixes, medians) starts at "go devices"; its sections are added below then.

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
| chrome-prefs | jobs | DuckDuckGo as default search engine, "Always use secure connections" on | Chrome keeps both in its private `Preferences` file. No intent, `cmd` or `settings` key reaches them and the web cannot change a native browser's settings. | Root read of `Preferences`: `default_search_provider_data` is DuckDuckGo and `https_only_mode_enabled` is true |
| clock-settings | jobs | Tokyo and London world clocks, snooze 15 min, silence after 5 min | World clocks and Clock settings live in Clock's private prefs; the AlarmClock intents set alarms and timers only. | Root read of Clock's files: Tokyo and London present, snooze 15, silence 5 |
| messages-pin-archive | jobs | Pin Bob Park's thread, archive Carol Diaz's, leave Alice Chen's | Pin and archive are Messages-private flags. Seeding is `adb emu sms send` (harness side); setting the flags has no command. | Root read of Messages' `bugle_db` `conversations` row per person: pin/archive columns |
| photos-crop | jobs | Crop jobs-harbor to a square, save a copy, keep the original | The Photos editor is the only crop tool on the image; a shell has no image editor. | MediaStore: original still 1200 wide; a new image added after the baseline, square, at most 800 px |
| wakelock-restrict | jobs-mixed | Find the app holding a wakelock, set its battery usage to Restricted | Shell finds the holder (`dumpsys power`: `jobs:drain`, package `dev.phoneuse.testbed`); Settings sets the restriction. A shell could also run `cmd appops set`, so this job rewards the agent that combines them rather than requiring both. | `cmd appops query-op RUN_ANY_IN_BACKGROUND ignore` against a baseline: only the testbed is newly restricted |
| log-code | jobs-mixed | Find the pickup code the testbed wrote to the device log, put it in a contact "Locker 9" | The code is only in logcat (shell); the contact is made in the Contacts UI. | Same contact check as relay-code, shell allowed |

## Dropped candidates and why

Contact add (`content insert` does it), calendar event (provider insert), dark mode and screen timeout (`cmd uimode`, `settings put`), alarm and timer (AlarmClock intents), SMS send (SENDTO intent plus a tap), Chrome live fact and the web shopping cart (a browser does them), the Uber-Eats-style order (needs sign-in). Maps transit time was not added: Maps on the web gives the same answer.

## Not yet proven (Phase B, step 0, before any agent run)

No device was available, so the checks were proven only against canned device output (`python3 -m unittest test_jobs`, 9 tests, pass and fail cases for the rubric, the locker contact, Clock, Messages, Photos, the wakelock and the sign-up log). With the device, step 0 proves each check by hand with adb, pass and fail, before any agent runs. Things that are guesses until then:

- Whether the chosen emulator allows `su 0` (existing `alarm-gym` uses it on emulator-5558) and whether `run-as dev.phoneuse.testbed` works (debug build).
- Chrome `Preferences` key names and flush timing (the check presses Home and waits 5 s before reading).
- Clock pref key names (`snooze`, `silence` matched by substring; minutes, seconds or ms accepted) and where world clocks are stored (any file under Clock's data dir containing the city name).
- Messages: that it is default SMS and past onboarding (do once by hand, not per run), and the `conversations` column names (the check finds pin and archive columns by substring).
- Photos: that it opens signed out, finds the seeded PNG after a media scan, and where it saves the copy.
- The wakelock survives the testbed being backgrounded; the Settings path to Restricted flips `RUN_ANY_IN_BACKGROUND`.
- The testbed APK with the new tabs is built (`testbeds/android/app/build/outputs/apk/debug/app-debug.apk`) and must be installed with the force-stop the install helper already does.

## Run budget

Core 7 + mixed 2 = 9 tasks. One pass per harness (phone-use, phone-harness, mobile-mcp) is 27 runs; the second pass for medians doubles it. The bug-report and Messages/Photos jobs are the longest. Phase B runs phone-use alone first, as asked.
