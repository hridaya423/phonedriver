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
| wakelock-restrict | jobs-mixed | Find the app holding a wakelock, set its battery usage to Restricted | Shell finds the holder (`dumpsys power`: `jobs:drain`, package `dev.phoneuse.testbed`); Settings sets the restriction. A shell could also run `cmd appops set`, so this job rewards the agent that combines them rather than requiring both. | `cmd appops query-op RUN_ANY_IN_BACKGROUND ignore` against a baseline: only the testbed is newly restricted |
| log-code | jobs-mixed | Find the pickup code the testbed wrote to the device log, put it in a contact "Locker 9" | The code is only in logcat (shell); the contact is made in the Contacts UI. | Same contact check as relay-code, shell allowed |

## Dropped candidates and why

Contact add (`content insert` does it), calendar event (provider insert), dark mode and screen timeout (`cmd uimode`, `settings put`), alarm and timer (AlarmClock intents), SMS send (SENDTO intent plus a tap), Chrome live fact and the web shopping cart (a browser does them), the Uber-Eats-style order (needs sign-in). Maps transit time was not added: Maps on the web gives the same answer.

## Phase B step 0: checks proven by hand on emulator-5558 (2026-11-07)

Image: `google/sdk_gphone64_x86_64/emu64xa:17`, userdebug, a 2560x1600 tablet, reached through the 5038 tunnel. `su 0` works; `run-as dev.phoneuse.testbed` works (debug build). Each check was driven by hand with adb input (no agent): a pass state, and a fail state for every distinct failure it reports. Commands were `python3 jobs.py seed|check|clean <task>` with `SERIAL=emulator-5558`.

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
| wakelock-restrict | testbed restricted | testbed not restricted; another app restricted | `dumpsys power` shows `jobs:drain` for uid 10234 (`dev.phoneuse.testbed`). In Settings, App info, App battery usage, turning off "Allow background usage" sets `RUN_ANY_IN_BACKGROUND ignore`. Restricting releases the wakelock. |

Facts a run will meet:
- The local adb client cannot reach a tunnelled emulator's console (`adb emu` connects to local port 5558). Seeding SMS goes through `EMU_ADB="ssh win-ts /d/Android/Sdk/platform-tools/adb.exe"`, which `jobs.py emu` uses. It is the only thing that needs the SSH host besides the tunnel.
- Messages is seeded once: contacts Alice Chen, Bob Park and Carol Diaz and one SMS each stay on the emulator, and `seed`/`clean` only reset pins and archives (`jobs.py purge messages-pin-archive` removes everything). A "Spam protection" dialog was dismissed once.
- Contacts and Photos first-run prompts were dismissed once (Contacts notification prompt: "Don't allow").
- `uiautomator dump` fails with "could not get idle state" on the Contacts list screen. Anything that reads that screen through uiautomator (the `adb` harness) must retry; to look at in Phase B for phone-use and the others.
- Left on the emulator: the three seeded contacts and SMS above, the installed testbed, a Chrome and Clock state cleared by each cleanup. Nothing else.

## Run budget

Core 7 + mixed 2 = 9 tasks. One pass per harness (phone-use, phone-harness, mobile-mcp) is 27 runs; the second pass for medians doubles it. The bug-report and Messages/Photos jobs are the longest. Phase B runs phone-use alone first, as asked.
