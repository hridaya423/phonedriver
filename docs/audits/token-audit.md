# Token audit (2026-11-06, main at c2297a1)

Question: why did phonedriver spend 413,534 tokens on the three benchmark tasks against phone-harness's 337,059 (`docs/bench/tokens.md`), and what removes the difference? All numbers come from the raw stream-json logs of those six runs (one run each, `claude-sonnet-5-5`, tablet emulator), re-read turn by turn with `scratchpad/an/turns.py`, `gen.py` and `classify.py` (session scratch, not in the repo). Single samples: they show what happened, not rates.

## Short answer

1. **Tokens are turns times context, and the context is mostly fixed.** Every API turn re-reads the whole context. Tool results are tiny (the six runs return 3,000 to 11,000 tokens of results in total, per run). The fixed prefix, re-read on every turn, is 59% to 81% of each phonedriver run (44% to 74% of phone-harness's).
2. **phonedriver's prefix is 2,900 tokens longer than phone-harness's** (7,740 against 4,840 on the first turn). Over phonedriver's 36 turns that is 104,000 tokens, more than the whole 76,000 gap. It is the `tools/list` (13 tools then, 14 now) plus the server instructions.
3. **15 of phonedriver's 36 turns were wasted** (3 of 9 on `wiki-birthdate`, 4 of 9 on `testbed-web-form`, 8 of 18 on `alarm-gym`). In every case the call before it returned a reply that lacked what the agent needed (a dialog not yet in the tree, a page not yet loaded, the launcher instead of the app), and the agent paid one or two calls, often a screenshot, to see it. phone-harness wasted none by this definition, but spent 6 turns of `alarm-gym` and 3 of `testbed-web-form` on trial and error of its own.
4. **Screenshots are expensive**: a 1280x800 screenshot adds 1,370 to 1,460 tokens to the context and every later turn re-reads it. phonedriver took 4 in these runs, all to see what a reply had hidden; their re-reads cost about 43,000 tokens.

## After the fixes (2026-11-07)

Done in this branch: fix 1 (tools plus instructions 4,204 to 2,768 tokens measured; `tools/list` 10,987 to 5,107 characters), fix 2 (re-read until stable; a loading page, a cold web view up to 10 s, a removal-only reply up to 2 s), fix 3 (`launch` and `open` wait for another app to come to the front) and the clipboard prune of fix 5. Not done: fix 4 (keyboard hides Save), fix 6 (screenshot size), fix 7 (`wait` timeout read twice). Three runs of the benchmark after the fixes gave 216,571, 333,470 and 460,733 tokens (3 of 3 each) against 413,534 before and phone-harness's 337,059; the spread inside one task (`alarm-gym` 75,020 to 360,219 with the same code) is larger than the effect, so see `docs/bench/tokens.md` for the per-run causes. A cold Clock swallowed the first tap on Add alarm in two of three runs and cost 5 to 8 calls each time; no fix for that is in this branch.

## Method

- Tokens per turn: a turn is one assistant message id; `usage` gives input, cache read, cache creation, output. Context at turn k is input + cache read + cache creation. The size of a tool result is the growth of the context from turn k to k+1, minus turn k's output (it includes the harness's few framing tokens). Totals reproduce the `result` event to within 1.5% (the `result` event also counts the final bookkeeping call).
- The "calls" column of `docs/bench/tokens.md` is the `num_turns` of the result event (28, 9, 11 for phonedriver); it counts each parallel tool call, while the tables below count API turns (18, 9, 9).
- "Wasted" is my judgment from the reply the agent had in hand: the call would not have been needed if the previous reply had held the information the agent then went to fetch. The "why" column says what was missing.
- Fixed overhead measured directly with three one-word `claude -p` runs under the benchmark's flags (`--tools ""`, `--strict-mcp-config`, `--setting-sources ""`, `--model claude-sonnet-5-5`): no tools at all 3,217 tokens (Claude Code's system prompt, the appended device sentence and a four-word prompt); with the `Bash` tool 5,139 (so `Bash` is 1,922); with `phonedriver mcp` at c2297a1 plus the shell tool (14 tools, 10,987 characters of `tools/list`) 7,421, so **tools plus instructions are 4,204 tokens, about 2.6 characters per token** (JSON schemas tokenize densely).

## Fixed overhead per turn

| Harness | First-turn context | Of which tools and instructions |
|---|---|---|
| phonedriver, benchmark build (13 tools) | 7,737 to 7,756 | about 3,900 (derived) |
| phonedriver, c2297a1 (14 tools, with `shell`) | 7,421 (one-word prompt) | 4,204 (measured) |
| phone-harness | 4,835 to 4,851 | 1,922 (`Bash` tool) plus about 500 for its longer system hint |
| nothing | 3,217 | 0 |

Per-tool size at c2297a1 (compact JSON, characters): shell 1,295, swipe 1,146, type 1,009, snapshot 1,000, tap 924, scroll 837, batch 803, apps 754, launch 622, wait 640, press 555, screenshot 561, devices 489, clipboard 337. Boilerplate in them: `$schema` on every tool, `"format":"uint32","minimum":0`, every optional argument typed `["string","null"]`, a `device` property on every tool (14 times), arguments no agent has used (audit `ax-audit.md`: `snapshot diff/within/interactive/max_lines`, `tap count/long/role`, `swipe distance/fast`).

## Where the extra tokens went (phonedriver against phone-harness)

| | phonedriver | phone-harness | Difference |
|---|---|---|---|
| Total tokens | 413,534 | 337,059 | +76,475 |
| Turns | 36 | 34 | +2 |
| Fixed prefix times turns | 279,000 | 164,600 | +114,400 |
| Growth (results and outputs re-read) | 132,600 | 169,300 | -36,700 |

So the gap is entirely the prefix. phonedriver's replies are more compact (growth is 22% lower) and it takes about as many turns, but each turn costs 2,900 more.

### (a) Fixed overhead per call

About 60% of every phonedriver token (279,000 of 413,534). The only part we control is tools plus instructions: 3,900 to 4,200 tokens on every turn. Removing 2,000 of them saves about 72,000 tokens on these three tasks (36 turns), nearly the whole gap.

### (b) Result verbosity

Small. All tool results of all three phonedriver runs add up to 17,800 tokens, and they cost only what the later turns re-read: 132,600 tokens (growth) in total, a third of it four screenshots. Lines worth cutting, by tokens re-read:

- Screenshots (1,374, 1,405, 1,460 and one of 1,963 together with a tap): the largest single results, and all four were taken to see what a reply had hidden.
- Chrome first-run and new-tab pages: 71 lines, 1,759 characters in one reply (`wiki-birthdate` call 5): Chrome's shortcut tiles and news cards. It is 859 tokens read by 4 later turns.
- The Gboard clipboard overlay after a paste: `group "Clipboard"`, `button "Clipboard"`, `group scrollable`, `group "Content copied"` appear in the reply to the next tap and disappear in the one after (`testbed-web-form` calls 4 and 7, `wiki-birthdate` call 6). About 60 tokens each, twice per paste, and in `testbed-web-form` the agent took a snapshot and a screenshot to find out whether the checkbox had been ticked because the reply showed only this popup.
- Web content: Wikipedia's first screen is 3,600 characters (about 900 tokens) as one line per text fragment and per link. Joining the fragments would save perhaps 300 tokens, once per page. Not worth a format change.
- Repeated subtrees and hints: not measurable here. The timeout hint (`Increase timeout_ms, or run snapshot`) appeared twice and did not change what the agent did.

### (c) Extra calls, and the root cause of each

Cost of a wasted call is its own turn (the whole context, re-read) plus its result re-read by every later turn. Token figures are rough: the sum of the wasted turns' own re-reads and their results' re-reads.

| Run | Wasted call | Root cause | Approx. tokens |
|---|---|---|---|
| alarm-gym 3, 4, 5 | `tap Add alarm` replied `- @e3 @e4 @e18 ...` (removals only); screenshot; tap by point plus snapshot | **Tree read before the dialog was in it**: the settle is pixel-based and ended while the accessibility tree still lacked the new window; the next snapshot, 214 ms later, had it | 3 calls, 30,000 |
| alarm-gym 9, 10, 11, 12 | `tap` on the alarm card replied with the tab bar and no list; screenshot; a tap by point that collapsed the card; tap again | **Tree read mid-transition**: the expanding card's animation left the list out of the reply | 4 calls, 62,000 |
| alarm-gym 15, 16 | snapshot, then hide-keyboard plus find Save | `type --submit` into the label left the keyboard up and Save under it; the reply did not say so | 2 calls, 35,000 |
| wiki-birthdate 1, 2 | `launch url` replied `ok opened` with the launcher on screen; `wait "Grace Hopper"` for 12.9 s | **Wrong screen counted as arrived**: `until_app_content` treats any application window as the app, and the launcher is one; Chrome had not started yet | 2 calls, 18,000 |
| wiki-birthdate 7 | `wait "Born"` took 30 s | The page was loading behind a progress bar and the reply had no content; the first accessibility read of a WebView takes 6 to 14 s on this emulator | 1 call, 10,000 |
| testbed-web-form 3 | `wait settled` plus snapshot | The reply to the Web tab showed an empty `webview` (loading) | 1 call, 12,000 |
| testbed-web-form 5, 6 | snapshot, screenshot | The reply after ticking the box was the Clipboard popup and removals, not the form | 2 calls, 22,000 |
| testbed-web-form 8 | snapshot to confirm the submit | The reply to `Send` was `- @e45 ...` | 1 call, 5,000 |

The Clock typing failure the task description mentions (type into the hour field) did not happen in this run: the agent used `type mode=set` on `@e27` and it verified. It is being fixed on the dogfood branch.

## phone-harness's turns, for comparison

Its tools are free-form scripts, so it pays for its own mistakes: `alarm-gym` took 20 turns, 6 of them after `Save` raised an exception (Back, retry, then listing helper names), and 3 more guessing the picker's labels (a thin space in "7:30 PM"); `testbed-web-form` never saw the WebView in `screen()` (it listed helper names, then `find_nodes("")`) and answered that it could not confirm the submit, which the checker accepted because the form's event log had the submit. It never took a screenshot.

## Ranked fixes (expected tokens on these three tasks, from the logs)

1. **Cut tools plus instructions from about 4,200 to about 1,500 tokens** (-2,700 per turn, about -95,000 on 36 turns). Drop `$schema`, `format`, `minimum`, nullable arrays and the 14 `device` properties from the schema; hide the arguments no agent has used from `tools/list` (they stay accepted); shorten descriptions to what the model cannot guess. The dogfood branch already removes `$schema`, `format`, `minimum` and `null` (about -1,700 characters); the rest is new.
2. **A reply that holds the screen the action produced** (done in this branch): after a settle that saw a change, read the tree again until two reads agree (at most 1 s), and while a progress bar or an empty WebView shows, keep reading up to 4 s. Removes the wasted calls of `alarm-gym` 3 to 5 and 9 to 12 and `testbed-web-form` 3 and 5 to 6: up to 11 calls, about -130,000 tokens at best.
3. **`launch` and `open` wait for an app other than the one that was in front** (done in this branch): removes `wiki-birthdate` 1 and 2, about -18,000.
4. **Tell the agent when the keyboard hides the action it needs** (`type --submit` replies that end with `keyboard shown` and a Save under it): `alarm-gym` 15 and 16, about -35,000. Not done.
5. **Prune the Gboard clipboard overlay** (`group "Clipboard"` and children when the window is the keyboard's): about -2,000 directly, and it removed a verification round trip in `testbed-web-form`. Not done.
6. **A screenshot that costs fewer tokens**: 1,400 tokens at 1280x800. Scaling the long side to 1,024 would save 35% per image but changes the coordinates the agent taps; not done.
7. `wait` honors `timeout_ms` once: with 15,000 it read the tree for 15 s, then again for 15 s (a read of a WebView takes that long on this emulator). Wall time only.

## Per-turn tables

`call` is the tool call (arguments trimmed; `device` removed); `result tokens` is the growth the call's result added to the context; `why` is my reading of what the agent was missing.


### phonedriver alarm-gym: 18 turns, 236,292 tokens, first-turn context 7,737

| # | in | cache read | cache create | out | call | result tokens | why |
|---|---|---|---|---|---|---|---|
| 1 | 2 | 4922 | 2813 | 24 | launch {"app": "Clock", } | 358 | needed: open Clock |
| 2 | 2 | 7735 | 382 | 28 | tap {"target": "@e6"} | 410 |  |
| 3 | 2 | 8117 | 438 | 24 | tap {"target": "@e5"} | 101 | WASTED: reply was only removals (`- @e3 @e4 ...`); the time dialog was not in the tree yet |
| 4 | 2 | 8555 | 125 | 32 | screenshot {} | 1374 | WASTED: screenshot to see what the empty reply hid |
| 5 | 2 | 8680 | 1406 | 39 | tap {"target": "383,553"} + snapshot {} | 674 | WASTED: tap by point plus snapshot; the dialog was already open |
| 6 | 2 | 10086 | 713 | 24 | type {"text": "7", "into": "@e27", "mode": "set"} + tap {"target": "@e29"} | 403 |  |
| 7 | 2 | 10799 | 427 | 22 | type {"text": "30", "into": "@e27", "mode": "set"} + tap {"target": "@e32"} | 334 |  |
| 8 | 2 | 11226 | 356 | 24 | tap {"target": "@e36"} | 511 |  |
| 9 | 2 | 11582 | 535 | 24 | tap {"target": "@e47"} | 270 | WASTED: reply showed tabs and no alarm list (tree read mid-transition) |
| 10 | 2 | 12117 | 294 | 32 | screenshot {} | 1405 | WASTED: screenshot to see the card |
| 11 | 2 | 12411 | 1437 | 52 | tap {"target": "400,170"} | 472 | WASTED: tap by point, collapsed the card again |
| 12 | 2 | 13848 | 524 | 8 | tap {"target": "@e59"} + screenshot {} | 1963 | needed: re-open the card (plus a screenshot, wasted) |
| 13 | 2 | 14372 | 1971 | 8 | tap {"target": "@e72"} + tap {"target": "@e73"} + tap {"target": "@e74"} + tap {"target": "@e75"} + tap {"target": "@e76"} + tap {"target": "@e81"} | 923 |  |
| 14 | 2 | 16343 | 931 | 25 | type {"text": "Gym", "into": "@e89", "mode": "set", "submit": tru | 212 |  |
| 15 | 2 | 17274 | 237 | 27 | snapshot {} | 388 | WASTED: snapshot after typing; Save was under the keyboard |
| 16 | 2 | 17511 | 415 | 22 | press {"key": "hide-keyboard"} + snapshot {"find": "Save"} | 325 | WASTED: hide-keyboard plus find Save |
| 17 | 2 | 17926 | 347 | 45 | tap {"target": "@e96"} | 534 |  |
| 18 | 2 | 18273 | 579 | 89 | final answer | 0 |  |

### phonedriver wiki-birthdate: 9 turns, 85,696 tokens, first-turn context 7,756

| # | in | cache read | cache create | out | call | result tokens | why |
|---|---|---|---|---|---|---|---|
| 1 | 2 | 4922 | 2832 | 24 | launch {"url": "https://en.wikipedia.org/wiki/Grace_Hopper", } | 581 | WASTED: `launch url` replied `ok opened` with the launcher on screen (Chrome not in front yet) |
| 2 | 2 | 7754 | 605 | 42 | wait {"condition": "Grace Hopper", } | 126 | WASTED: wait for a text that could not appear (Chrome never started loading it) |
| 3 | 2 | 8359 | 168 | 24 | launch {"app": "Chrome", } | 310 | needed: launch Chrome by name (first-run screen) |
| 4 | 2 | 8527 | 334 | 24 | tap {"target": "@e33", } | 253 | needed: first-run dialog |
| 5 | 2 | 8861 | 277 | 24 | tap {"target": "@e37", } | 859 | needed: first-run dialog |
| 6 | 2 | 9138 | 883 | 24 | type {"text": "en.wikipedia.org/wiki/Grace_Hopper", "into": "@e44 | 694 | needed: type the URL; reply `loading`, no page |
| 7 | 2 | 10021 | 718 | 29 | wait {"condition": "Born", "timeout_ms": 15000, } | 174 | WASTED: wait for "Born" on a page whose tree read took 30 s; reply had no page text |
| 8 | 2 | 10739 | 203 | 54 | snapshot {"find": "1906", } | 87 | needed: read the date |
| 9 | 2 | 10942 | 141 | 9 | final answer | 0 |  |

### phonedriver testbed-web-form: 9 turns, 89,356 tokens, first-turn context 7,742

| # | in | cache read | cache create | out | call | result tokens | why |
|---|---|---|---|---|---|---|---|
| 1 | 2 | 4922 | 2818 | 32 | snapshot {} | 493 | needed: first look |
| 2 | 2 | 7740 | 525 | 28 | tap {"target": "@e8"} | 279 |  |
| 3 | 2 | 8265 | 307 | 29 | wait {"condition": "settled"} + snapshot {} | 718 | WASTED: wait settled plus snapshot; the reply to the Web tab showed an empty webview |
| 4 | 2 | 8572 | 747 | 24 | type {"into": "@e41", "text": "Ada Lovelace"} + tap {"target": "@e42"} | 367 |  |
| 5 | 2 | 9319 | 391 | 25 | snapshot {} | 386 | WASTED: snapshot to check a tick; the reply showed a Clipboard popup and removals, not the form |
| 6 | 2 | 9710 | 411 | 9 | screenshot {} | 1460 | WASTED: screenshot to check the tick |
| 7 | 2 | 10121 | 1469 | 24 | tap {"target": "@e43"} | 82 |  |
| 8 | 2 | 11590 | 106 | 32 | snapshot {} | 297 | WASTED: snapshot to confirm the submit; the reply to Send was `- @e45 ...` |
| 9 | 2 | 11696 | 329 | 97 | final answer | 0 |  |

### phone-harness alarm-gym: 20 turns, 218,853 tokens, first-turn context 4,840

| # | in | cache read | cache create | out | call | result tokens | why |
|---|---|---|---|---|---|---|---|
| 1 | 2 | 2077 | 2761 | 16 | print(screen_info()); open_app("Clock"); print(screen()); | 1125 | screen_info and open_app in one script |
| 2 | 2 | 4838 | 1141 | 16 | tap_text("Alarms"); print(screen()); | 1413 |  |
| 3 | 2 | 5979 | 1429 | 16 | tap_text("Add alarm"); import time; time.sleep(1); print(screen()); | 1301 |  |
| 4 | 2 | 7408 | 1317 | 57 | tap_text("7"); import time; time.sleep(1); print([t['text'] for t in s | 168 | taps digit "7" by text (guessing the picker) |
| 5 | 2 | 8725 | 225 | 16 | tap_text("30"); import time; time.sleep(1); print([t['text'] for t in  | 172 |  |
| 6 | 2 | 8950 | 188 | 16 | tap_text("OK"); import time; time.sleep(1); print([t['text'] for t in  | 396 |  |
| 7 | 2 | 9138 | 412 | 8 | tap_text("7:30 PM alarm"); import time; time.sleep(1); print([t['text' | 316 |  |
| 8 | 2 | 9550 | 324 | 7 | tap_text("7:30 PM"); import time; time.sleep(1); print([t['text'] for  | 1175 |  |
| 9 | 2 | 9874 | 1182 | 45 | import time; tap_text("7:30 PM"); time.sleep(1); print([t['text'] for  | 104 | text had a thin space; retried with variants (turns 8-10) |
| 10 | 2 | 11056 | 149 | 16 | import time; tap_text("7:30"); time.sleep(1); print([t['text'] for t i | 119 |  |
| 11 | 2 | 11205 | 135 | 16 | import time; tap_text("AM"); time.sleep(1); tap_text("OK"); time.sleep | 249 |  |
| 12 | 2 | 11340 | 265 | 33 | import time; for d in ["Mon","Tue","Wed","Thu","Fri"]:;     tap_text(d | 271 |  |
| 13 | 2 | 11605 | 304 | 8 | import time; type_text("Gym"); time.sleep(1); print([t['text'] for t i | 249 |  |
| 14 | 2 | 11909 | 257 | 5 | import time; try:;     tap_text("Save"); except Exception as e:;     k | 1193 | Save failed with an exception; Back, then retry (turns 14-19), turn 17 lists the helper names |
| 15 | 2 | 12166 | 1198 | 53 | adb -s emulator-5558 shell input keyevent 4; sleep 1; phone-harness << | 132 |  |
| 16 | 2 | 13364 | 185 | 16 | print([t['text'] for t in screen()]); | 204 |  |
| 17 | 2 | 13549 | 220 | 6 | print([n for n in dir() if not n.startswith('_')]); | 539 |  |
| 18 | 2 | 13769 | 545 | 16 | import time; back(); time.sleep(1); print([t['text'] for t in screen() | 236 |  |
| 19 | 2 | 14314 | 252 | 17 | import time; tap_text("Save"); time.sleep(1); print([t['text'] for t i | 413 |  |
| 20 | 2 | 14566 | 430 | 129 | final answer | 0 |  |

### phone-harness wiki-birthdate: 6 turns, 39,078 tokens, first-turn context 4,851

| # | in | cache read | cache create | out | call | result tokens | why |
|---|---|---|---|---|---|---|---|
| 1 | 2 | 2077 | 2772 | 16 | print(screen_info()); open_app("Chrome"); print(screen()); | 527 | open_app and screen() in one script |
| 2 | 2 | 4849 | 543 | 16 | tap_text("Stay signed out"); import time; time.sleep(2); print(screen( | 359 |  |
| 3 | 2 | 5392 | 375 | 16 | tap_text("No thanks"); import time; time.sleep(2); print(screen()); | 1298 |  |
| 4 | 2 | 5767 | 1314 | 16 | tap_text("Search Google or type URL"); type_text("en.wikipedia.org/wik | 230 | tap the URL box, type, submit in one script |
| 5 | 2 | 7081 | 246 | 8 | import time; time.sleep(2); print([r['text'] for r in screen()]); | 1234 | sleep 2 and print every text: the date is in it |
| 6 | 2 | 7327 | 1242 | 9 | final answer | 0 |  |

### phone-harness testbed-web-form: 8 turns, 75,948 tokens, first-turn context 4,835

| # | in | cache read | cache create | out | call | result tokens | why |
|---|---|---|---|---|---|---|---|
| 1 | 2 | 2077 | 2756 | 16 | print(screen_info()); print(screen()); | 2266 | screen_info and screen() |
| 2 | 2 | 4833 | 2282 | 16 | tap_text("Web"); import time; time.sleep(2); print(screen()); | 730 |  |
| 3 | 2 | 7115 | 746 | 17 | import time; time.sleep(2); print(screen()); | 1134 | second screen() after a 2 s sleep for the webview |
| 4 | 2 | 7861 | 1151 | 7 | import time; tap_text("Name"); time.sleep(1); type_text("Ada Lovelace" | 1294 |  |
| 5 | 2 | 9012 | 1301 | 16 | import time; tap_text("Agree"); time.sleep(1); print([e for e in scree | 1006 |  |
| 6 | 2 | 10313 | 1022 | 2 | print([n for n in dir() if not n.startswith('_')]); | 550 | listed helper names: the webview is not in screen() |
| 7 | 2 | 11335 | 552 | 16 | for n in find_nodes(""): ;     print(n); | 1574 | find_nodes("") to see the webview nodes; then answered it could not confirm the submit |
| 8 | 2 | 11887 | 1590 | 9 | final answer | 0 |  |
