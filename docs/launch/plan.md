# Launch plan (round 2)

Nothing has been posted, and no account has been created. The author posts by hand. All times are UTC. Today is Friday 2026-11-06. `D0` is the launch Monday: the first Monday after every item in the checklist is done. Platform detail is in `show-hn.md`, `x.md`, `reddit.md` and `linkedin.md`. Evidence is in `research.md`.

## Cross-platform sequence

| When | Platform | Action | Reason from the ranking research |
|---|---|---|---|
| D-7 to D-1 | All | Run the checklist below. | Show HN needs something people can try. |
| D-1 | X | Send a direct message to 5 to 10 mutuals who work on agents or mobile: what it is, the repo, "tell me what breaks". No script. No request for likes. | X adds +15 to the reply weight on an original post from a mutual follow (2026 code). Replies must be real, because the code labels engagement spammers. |
| D0, 00:00 (Sunday 7 pm ET, 4 pm PT) | Hacker News | Submit the Show HN. Stay until 02:00. | Three datasets and a 90-day count put the best hit rate here. Hot rank decays steeply with age, so the first two hours decide the result. |
| D0, 13:30 (9:30 am ET) | X | Post the video post with the link in the body. Reply to every reply for two hours. | Weekday mornings, with no primary evidence. The cold-start slot lasts 2 hours and needs likes per view, so followers must be online. By now the HN result is known. |
| D1 (Tuesday), 13:30 | LinkedIn | Post the video post. Put the repo link in the first comment at once. Reply for an hour. | Tuesday to Thursday mornings, weak evidence. A day apart from X so the author can reply to both. |
| D2 (Wednesday), 14:00 | r/ClaudeAI | Post variant 1 with the project flair. | Best audience, a 180-point precedent. Reply for two hours. |
| D3 (Thursday), 14:00 | r/LocalLLaMA | Post variant 2, only with a tested local-model fact. | Largest phone-agent audience. Skip if there is no local-model result. |
| D4 (Friday), 14:00 | r/rust | Post variant 3. | A different post for a different audience, a day after the last. |
| Next monthly thread | r/androiddev | Comment in the Showcase thread. | Self-promotion is limited to that thread. |

Why this order:

- HN goes first because it has the least tolerance for a weak first hour. A moderator penalty cannot be undone by the author, and the thread is the only place that needs your attention at a fixed time.
- X is second because its cold-start slot rewards a post that gets likes in its first two hours, and your followers are awake at 13:30. By then you know what HN found and can fix any claim a reader proved wrong.
- LinkedIn and Reddit are slower and have no fixed window, so they take one day each. This also respects Reddit's spam filters: four different posts, a day apart.
- Each platform gets its own text. Nothing is cross-posted.

Rules that cut across platforms:

- Never post the HN thread link anywhere. Never ask a person to vote or like, on any platform. HN penalizes vote requests, X labels engagement spammers, LinkedIn demotes "comment to agree" prompts, and Reddit filters coordinated voting.
- Write every post yourself. HN bans generated text, Reddit readers punish it, and LinkedIn reduces reach for it.
- State the same limits everywhere: no physical Android phone measured, iOS needs a Mac with Xcode, phone-use was not the cheapest in tokens in the one-run benchmark.
- Use the same numbers everywhere, from the same source, with the same caveat.
- If HN fails, the plan continues. If HN reveals a false claim, fix it before X posts.
- If the account for any platform has no history, spend the week before D0 commenting honestly there. HN restricts Show HN for new accounts (`news.ycombinator.com/showlim`). r/LocalLLaMA gates by karma.

## Name

`phone-use` is taken on npm, and `github.com/phone-use` is an organization you do not control. `sauravtom/phone-use` has the same name (MIT, Android only, 6 stars). Check done 2026-11-06 (`research.md`, "Name availability").

Free on crates.io, npm, PyPI and as a GitHub account, with almost no repo of the name:

| Name | Why | Risk |
|---|---|---|
| **tapdiff** | Names the idea: a tap returns a diff. Short, unique, easy to type. Two repos exist with this name, both under 2 stars. | Does not say "phone". |
| phonewright | Reads as "Playwright for phones" at once. One repo, 0 stars. | The Playwright association invites a trademark or confusion complaint from Microsoft. Low probability, real cost. |
| diffphone | Says both words. Zero repos. | Awkward to say. |

**Recommendation: rename to `tapdiff` now, or keep `phone-use` on purpose.** The case for renaming: three of the four lookups for `phone-use` are blocked or crowded, a search for it lands on another project, and the demo is being re-recorded anyway, which is the cheapest moment to change the text in it. The case for keeping it: "phone use" echoes "computer use", and a descriptive name helps the "what is this" test on HN. The drafts use `phone-use` until you decide. A rename touches the crate and binary names, `PHONE_USE_*` environment variables, the MCP server name, the Homebrew tap, the README, and the SVG and PNG assets built by `design/build/build.py`.

## Assets

Paths are under `/Users/hridyaagrawal/Honey/phone-use/.github/assets/` (in the worktree: `.github/assets/`). Create nothing new unless a row says so.

| Asset | Use | State |
|---|---|---|
| Re-recorded demo MP4 | Native video on X, LinkedIn and Reddit | [TBD: re-recording in progress]. Confirm length (over 10 s for X), real time, no cuts between tool calls, readable without sound. |
| `demo.mp4`, `demo.gif` (old clip, 960x660, 32 s) | README | Replace with the re-recorded clip when it exists. |
| `speed-light.svg`, `speed-dark.svg` | README. For X reply 2, export to PNG. | PNG export needed. Check that the log scale reads at phone size. |
| `social-preview.png` (1280x640) | GitHub social preview | Exists. Set it in the repository settings. If the name changes, rebuild it. |
| `banner-*.svg` | README header | Its alt text says "up to 153 times faster than phone-harness". That figure comes from a 1,000-row list with about a dozen rendered rows on a quiet run. Keep it out of the posts. |
| `tokens-light.png` (`docs/bench/tokens.md` on `main`) | Reply on X, Reddit, HN answer | Exists on `main`. It does not favor phone-use. Use it only to show honest limits. |
| A terminal screenshot of a tap reply | X, Reddit | Capture from the README sample. Look at it before posting. |
| A caption overlay copy of the demo | LinkedIn | New copy. Do not overwrite the original. |

## Pre-launch checklist

Each item needs a command or screenshot as proof in `progress.md`, per `AGENTS.md`.

**Repo and release**

- [ ] Name decided (above). If renamed, every asset and the demo rebuilt.
- [ ] The repo is public at `github.com/hridaya423/<name>`. Creating a remote is a one-way door: ask first.
- [ ] `LICENSE` is AGPL-3.0-only and `NOTICE` lists scrcpy and the WebDriverAgent headers. D13 has an open item on `openh264` (Cisco's binary or a source build). Close it.
- [ ] `docs/decisions.md` D6 ("never execute agent-written code") is amended to match D21 (`shell`).
- [ ] A tagged release has binaries for macOS (arm64, x86_64), Linux (x86_64, arm64) and Windows (x86_64), each with the Android agent. Check the `dist` output and the installers.
- [ ] `brew install hridaya423/tap/phone-use` works. The tap repository exists.
- [ ] The README no longer says "nothing is published yet". Its "Tested on" badge matches `docs/bench/`.
- [ ] A network-silence check for phone-use exists, or the "talks to the device and nothing else" claim is cut.

**Clean-install proof (a machine that has never built the repo)**

- [ ] Run the one-line installer. `phone-use doctor` says ok or gives a fix on every line.
- [ ] `claude mcp add`, then `claude mcp list` shows Connected.
- [ ] Boot an emulator or Simulator. Ask "Open Settings and turn on dark mode." Confirm it, with a screenshot.
- [ ] Repeat the demo prompt. Look at the result.
- [ ] Time install to first working action. Record it. Ask the user before touching any physical device.

**Numbers (fill each slot or delete the sentence)**

- [ ] [TBD: paired quiet-machine re-run of phone-use against adb, including tap]. The latest tap figure, 33.7 to 35.9 ms p50 (`progress.md` on `main`), is an emulator number and is not paired with adb.
- [ ] [TBD: token benchmark rerun with the shell tool; mobile-mcp rerun on a local emulator]. Current result: phone-use did not use the fewest tokens.
- [ ] [TBD: iOS hybrid tier]. On this branch it is "UNFINISHED" and "unproven on device".
- [ ] [TBD: re-recorded demo]. Update every "real time" claim to match it.
- [ ] [TBD: local-model result] for r/LocalLLaMA.

**Accounts and posts**

- [ ] HN: account age and karma checked against the Show HN restriction. An email is in the profile.
- [ ] Reddit: each subreddit's current rules read in a browser. Karma gates met. Flair names copied.
- [ ] X: mutuals messaged on D-1.
- [ ] Every post retyped by hand from the structure files. `claims-ledger.md` checked against the launch commit.
- [ ] D0 calendar: four hours free from 23:30 UTC on D-1 (HN) or the equivalent in your time zone, plus two hours at 13:30 and each later post.

## First two hours on HN (D0, 00:00 to 02:00)

Minute 0: submit the URL and your text together. Open the thread and the repo issues in two tabs. If your text does not show as your comment, post it.

Minutes 0 to 15: confirm the post shows on `/shownew`. Re-run the install line in a clean shell. Do not share the link.

Minutes 15 to 120:

- Answer each question within 15 minutes, in your own words, from the fact table in `show-hn.md`. Hard questions first: security, benchmark fairness, tokens, license.
- Keep replies short and calm. Many comments per point trips the flamewar penalty.
- If someone finds a real bug, say so, open an issue, and say when you will look.
- Concede what is true. State limits before others do.
- Do not argue about AGPL. State what it covers once.
- Do not ask for stars or votes.

Minute 120: write down the open questions, bugs and the point count in `progress.md`. Check the thread again in the morning. If it stalls, email hn@ycombinator.com for the second-chance pool. Do not delete and repost.

## Risks the research found

- Phone-agent Show HNs scored 2 to 7 points; the best scored 53. Plan for a modest result.
- HN readers check for LLM-written text and AI-built repos. Say it first.
- Show HN may be restricted for a new HN account.
- r/opensource and r/programming would remove an AI-agent project. Do not post there.
- The token benchmark does not favor phone-use. Readers will ask. Answer with the numbers.
- Decision D6 contradicts the shipped `shell` tool until it is amended.
- The name clashes with an existing project and is taken on npm.
