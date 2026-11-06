# Launch plan

Nothing has been posted, and no account has been created. This plan assumes the author posts by hand. Dates are relative to T, the launch day. Today is Friday 2026-11-06.

## Order and timing

| When | What | Why |
|---|---|---|
| T-3 days or more | Make the repo public, tag a release, run the checklist below | Show HN requires that people can try the project. The README says nothing is published yet. |
| T, 8:00 am ET (13:00 UTC), Tuesday or Wednesday | Submit the Show HN | Mean Show HN score peaks 7 to 10 am ET (Dan King, 188k posts). The author must be awake and present for two hours (HN rules). Weekday mornings also match the common X window. |
| T, +45 to 60 min | Post on X: launch post, then replies as a thread | HN is the slowest to recover from a bad first hour, so the author's attention goes there first. X needs the author's replies in the first hours too, so it starts once the first HN questions are answered. |
| T+1 day, morning | Post on LinkedIn | LinkedIn needs replies in the first hour, which the author cannot give while on HN and X. The post can add one fact from launch day, such as the first hard question and the fix. |
| T+2 to T+7 days | Reply to late comments. Do not repost | dang allows a repost only if it is "significantly different", once or twice a year. |

Where the research disagrees, and what this plan chose:

- HN timing: the two large datasets find Sunday evening US time has the highest rate of breakout posts (Sunday 7 pm ET: 10.8% reach 50+ points). They also find weekday mornings have the best mean score. The plan picks the morning because presence matters more than a small gain in rate. If the author prefers Sunday 7 pm ET, the plan still works with all times shifted. The small-blog advice that "there is no best time" is consistent with either.
- X link placement: X staff said in April and July 2026 that links in the post are not penalized. The counter-evidence is observational. The plan keeps the link in the post and relies on the video and the author's replies.
- LinkedIn link placement: the only study found little penalty on personal profiles. The plan puts the link in the body.

Do not link the Show HN thread from X or LinkedIn, and do not ask anyone to upvote. HN's guidelines forbid soliciting votes, and boosters in the thread hurt (dang's Show HN tips).

## Assets

All paths are under `/Users/hridyaagrawal/Honey/phone-use/.github/assets/`. Create nothing new unless a row says so.

| Asset | Use | State |
|---|---|---|
| `demo.mp4` (960x660, 30 fps, 32 s, 1 MB) | Native video on X and LinkedIn | Exists. Retake it if the daemon or the prompt changes (README says it is recorded by `design/build/demo_record.py` and `demo_compose.py`). |
| `demo.gif` (960x660, 2.8 MB) | The README. Not for X: use MP4 | Exists. |
| `speed-light.svg`, `speed-dark.svg` | The README speed chart. For X thread post 4, export one to PNG | PNG export needed. [TBD: run the chart build or rasterize the SVG; check the log scale reads at phone size] |
| `social-preview.png` (1280x640) | GitHub social preview, so the link card looks right | Exists. Set it in the repository settings after the repo is created. |
| `banner-light.svg`, `banner-dark.svg` | README header | Exists. Its alt text says "up to 153 times faster than phone-harness". That figure comes from the quiet M1 run on a 1,000-row list that holds about a dozen rendered rows, so keep it out of the posts. |
| A terminal screenshot of a tap reply | X thread post 2 | Needs capture. Use the output in the README (iOS Simulator). Look at the screenshot before posting. |
| A caption overlay for the LinkedIn clip | LinkedIn mute autoplay | Needs a copy of `demo.mp4`. Do not overwrite the original. |
| Token benchmark chart | HN, X thread, LinkedIn | [TBD: token benchmark] |

## Pre-launch checklist

Each item needs a command or a screenshot as proof in `progress.md`, per `AGENTS.md`.

**Repo and release**

- [ ] The repo exists at `github.com/hridaya423/phone-use` and is public. (Creating a remote is a one-way door: ask the user first.)
- [ ] Decision on the name, given `github.com/sauravtom/phone-use` (MIT, Android only, 6 stars on 2026-11-06). Check crates.io, PyPI and npm names once more.
- [ ] `LICENSE` is AGPL-3.0-only and `NOTICE` lists scrcpy (Apache-2.0) and the WebDriverAgent headers. Decision D13 still lists an open item about shipping `openh264` built from source or loading Cisco's binary. Close it first.
- [ ] A tagged release has binaries for macOS (arm64 and x86_64), Linux (x86_64 and arm64) and Windows (x86_64). Each contains the Android agent. Check the `dist` workflow output and the installer scripts.
- [ ] `brew install hridaya423/tap/phone-use` works. The tap repository `hridaya423/homebrew-tap` exists.
- [ ] The README install section no longer says "nothing is published yet". Remove that sentence.
- [ ] The README claim "Tested on Android emulators, the iOS Simulator and iOS devices" still matches `docs/bench/m*.md`.

**Clean-install proof (on a machine that has never built the repo)**

- [ ] Run the one-line installer. Run `phone-use doctor`. Every line says ok or gives a fix.
- [ ] Add it to Claude Code with `claude mcp add`, then `claude mcp list` shows Connected.
- [ ] Boot an emulator or Simulator. Ask "Open Settings and turn on dark mode." Confirm it works, with the screenshot.
- [ ] Repeat the demo prompt: "Open Safari, search Wikipedia for Grace Hopper and tell me when she was born." Look at the result.
- [ ] Time the path from install to first working action. Put the figure in `progress.md`. (The DX audit timed a source build at 67 s with a cold target directory. A release install should be faster.)
- [ ] An iPhone: the `phone-use ios setup` path works with a free Apple ID (README). Ask the user before touching any physical device.

**Numbers (fill every slot or delete the sentence)**

- [ ] [TBD: token benchmark] Run it, or cut every sentence that depends on it. It needs paid model calls: ask the user first.
- [ ] [TBD: final tap latency] Re-run the Android benchmark on a quiet machine after the latest performance commits. Update `docs/bench/`, the README speed section, and every draft here.
- [ ] [TBD: iOS hybrid tier] Decide what to say. Today CoreDevice is experimental, off by default (`PHONE_USE_IOS_TIER=coredevice`).
- [ ] [TBD: shell tool] If it ships, update D6, README, the first comment (point 5) and answer 5.
- [ ] Add a network-silence check for phone-use itself, or drop "nothing else" from the telemetry claim.

**Posts**

- [ ] Retype `show-hn.md` by hand. Read `claims-ledger.md` and confirm every row against the repo at the launch commit.
- [ ] The HN account is personal and has an email in the profile.
- [ ] Clear the calendar for four hours after the Show HN goes up.

## First two hours

Minute 0: submit the URL and the text together. Open the thread and the repo's issue page in two tabs. If the text does not show as your comment, post it as a comment.

Minutes 0 to 15: check that the post is visible on `/shownew`. Reload the install command in a clean shell once more. Do not ask anyone to upvote. Do not share the HN link.

Minutes 15 to 120:

- Answer every question within 15 minutes. Use the prepared answers for the facts. Retype them. Do not paste.
- Answer the hard objections first (security, benchmark fairness, license), then the friendly ones.
- If a commenter finds a real bug, say so, open an issue with the number, and say when you will look. If you fix it in the thread's lifetime, say what changed.
- Concede what is true. The research found that authors who narrowed a claim honestly (Browser MCP on privacy) and who named limits first did well.
- Do not argue about AGPL. State what it covers, once.
- Do not ask for stars. Ask for the two kinds of feedback in the first comment.
- Minute 45 to 60: post on X. Answer replies there in batches of ten minutes, and go back to HN.
- Minute 120: write down the questions you could not answer, the bugs found, and the points count. Add them to `progress.md`.

If the post gets no traction in the first hour: do not delete and repost. Email `hn@ycombinator.com` and ask whether the post fits the second-chance pool (dang describes the pool in `item?id=26998308`). Check that the title and the first comment make the idea clear, since "I can't tell what this is" is the failure dang names.

If the post goes well: expect the GitHub star bump to decay with a half-life of about a day (Dan King). Keep replying through the second day.

## Risks the research found

- Phone-agent Show HNs scored 2 to 7 points. Plan for a modest result.
- HN readers will check for LLM-written text. Retype everything.
- A reader will find `AGENTS.md`. Say it first.
- A reader will ask for physical Android numbers. The honest answer is that none exist yet. If the user has a phone and agrees, measure one before launch; that single row would remove the largest gap in the evidence.
