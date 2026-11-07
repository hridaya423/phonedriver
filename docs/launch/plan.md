# Launch plan (round 3): T-3 to T+7

Nothing has been posted or submitted, and no account has been created. Every outward step is yours. All times are UTC, with US Eastern (ET, UTC-5 in November) and Pacific (PT) beside the ones that matter; add your own zone. `T0` is the **launch Monday**: the first Monday after the readiness gate is green. The calendar below assumes **T0 = Monday 2026-11-23**. If the gate is not green by **Sunday 2026-11-22 at 12:00 UTC (T-1)**, move T0 one week. Never launch with a red item. Note: US Thanksgiving is Thursday 2026-11-26 (T+3); that week has less US attention on Thursday to Sunday. If you prefer a normal week, use T0 = Monday 2026-11-30.

Evidence for every timing choice is in `research-2026-11.md`. Per-channel text: `show-hn.md`, `x.md`, `reddit.md`, `linkedin.md`, `product-hunt.md`, `devto-hashnode.md`, `short-video.md`. Where to list: `directories.md`. Numbers: `claims-ledger.md`.

## The goal, honestly

100 or more GitHub stars in launch week, from real interest. What the evidence says (`research-2026-11.md`, last section): a median Show HN gives about 1.4 stars per HN point within 48 hours, with a star half-life near a day. A 20-point post is about 28 stars. 100 stars in a week needs a front-page Show HN (roughly 70 or more points) or several channels landing inside the same 48 hours, plus a trickle from registries. So the plan stacks X, Reddit and LinkedIn on the days right after HN, and treats the registries as the base that keeps working after the spike. Do not plan around any single channel hitting.

## What must be true first (the gate)

Nothing is posted until every row is done. Owner: **U** = you, **A** = an agent can do it (a coding agent in this repo, with you reviewing), **U+A** = the agent prepares and you do the one-way step. Each needs proof in `progress.md` (a command and its output), per `AGENTS.md`.

| # | Item | Owner | State on 2026-11-07 | Needed by |
|---|---|---|---|---|
| 1 | The rename pass: crate, binary (`phonedriver`), `PHONEDRIVER_*` env vars, MCP server name, docs, SVG and PNG assets, the demo terminal | A | Not done. Code, `Cargo.toml` URLs and the README still say `phone-use`; the site docs already say `PHONEDRIVER_GUARD` and `claude mcp add phonedriver`. The two disagree today. | T-3 |
| 2 | Repo `github.com/hridaya423/phonedriver` created and public (a one-way door; the remote does not exist: the API returns 404) | U | Not done | T-3 |
| 3 | A tagged release with binaries for macOS (arm64, x86_64), Linux (x86_64, arm64) and Windows (x86_64), each with the Android agent, plus the shell, PowerShell and Homebrew installers. `release.yml` and `dist-workspace.toml` exist. | U+A | Not run. README still says "nothing is published yet". | T-3 |
| 4 | Clean-install proof on a machine that never built the repo: one-line installer, `phonedriver doctor`, `claude mcp add phonedriver -- phonedriver mcp`, `claude mcp list` shows Connected, "Open Settings and turn on dark mode" works. Time it. | U+A | Not done | T-2 |
| 5 | `phonedriver.dev` serves the built site (200 on every page; `python3 site/check.py` green). Whois shows the name registered (status ACTIVE); `dig` returned no address, so nothing is deployed. | U | Not done | T-3 |
| 6 | Registry route decided and built: an `.mcpb` bundle or an npm wrapper (the official registry takes no plain binary; `directories.md` section 1) | A | Not done; 1 to 2 days of work | T-3 |
| 7 | `server.json`, `glama.json`, `SECURITY.md` in the repo; the official registry entry live and found by `search=phonedriver`; Glama listing and the awesome-mcp-servers PR open | U+A | Not done | T-1 |
| 8 | The re-recorded demo (Opus 5.5, with the approval-prompt scene): terminal reads `phonedriver`; passes the checks in `short-video.md`; README and site updated | A | In progress | T-2 |
| 9 | The vertical 20 to 30 s cut, burned-in captions, "10x speed" visible | A | Not started; needs 8 | T-1 |
| 10 | `social-preview.png` rebuilt with the new name and uploaded in repo Settings; `tokens-light.png` rebuilt as a three-row sum chart (the current one is stale and says PHONE-USE) | A (build), U (upload) | Not done | T-2 |
| 11 | README: the install section no longer says "not published yet"; limits stated near the top; the Cursor deeplink only if tested | A | Not done | T-2 |
| 12 | Real-jobs benchmark result in: blog post 4 and `docs/bench/jobs.md` have `[RESULTS]` cells today. Fill them or cut every claim that depends on them. Hold post 4's cross-post. | A | Runs pending | T-1 (or cut) |
| 13 | Claims ledger checked against the launch commit; every `[DEMO]`, `[JOBS]`, `[USER]` slot in the drafts filled or deleted | U+A | Open | T-1 |
| 14 | Your accounts: HN (age, karma, email in profile, Show HN not restricted: `news.ycombinator.com/showlim`), Reddit (karma per sub, comment history), X, LinkedIn. Spend the week before commenting honestly where you will post. | U | Unknown | T-3 |
| 15 | Each subreddit's sidebar and rules read in a browser (the research could not read them); flair names copied | U | Open | T-1 |
| 16 | A physical Android phone run, if you have one. Ask first: `AGENTS.md` lists driving a physical device as a one-way door. If not, the drafts say "no Android phone measured", which is true. | U | Optional | |
| 17 | Open `docs/decisions.md` D6 and make it match D21 (`shell` exists). HN readers will quote D6 against you. | A | Check | T-3 |
| 18 | GitHub: topics, description, homepage, Discussions on, a pinned "Known limits" issue, 3 `good first issue` labels (`github-metadata.md`) | U+A | Not done | T-2 |
| 19 | You have 4 clear hours around the HN submission and 2 hours around each other post | U | | |

**Blockers today:** 1 (rename), 2 (public repo), 3 (release), 5 (domain deployed), 6 (registry package route) and 8 (re-recording). Items 7, 9, 10, 11 follow from them.

## Day by day

T-3 is Friday 2026-11-20. Everything is UTC.

| Day | Time (UTC) | Action | Owner |
|---|---|---|---|
| **T-3 Fri 11-20** | any | Rename pass lands. Repo made public. Tag `v0.1.0`; release workflow builds binaries and installers. Deploy the site; check `phonedriver.dev` and `llms.txt`. Fix D6. Add Discussions, topics, description, homepage. | A, U |
| | any | Build the registry package route (`.mcpb` or npm wrapper). | A |
| | any | Review the accounts (item 14). Start commenting honestly in the subs you will use. | U |
| **T-2 Sat 11-21** | any | Clean-install proof on a clean machine (item 4). Fix what it finds. | U+A |
| | any | Publish to the official registry (`mcp-publisher publish`). Add `glama.json`, claim on Glama. Open the awesome-mcp-servers PR. Fill the mcpservers.org form. (`directories.md`.) | U |
| | any | Final demo and the vertical cut. Rebuild `social-preview.png` and the tokens chart; upload the social preview. | A, U |
| **T-1 Sun 11-22** | by 12:00 | **Go or no-go.** All gate rows green, or T0 moves a week. | U |
| | 12:00 to 18:00 | Verify the registry entry (`curl 'https://registry.modelcontextprotocol.io/v0.1/servers?search=phonedriver'`). mcp.so submission. Install from the registry route on a second machine. | U |
| | 18:00 | Message 5 to 10 mutuals on X who work on agents or mobile: what it is, the repo, "tell me what breaks". No script, no like requests. | U |
| | 18:30 to 19:00 ET (15:30 to 16:00 PT) | Last checks, then the Show HN goes in at 19:00 ET, which is **00:00 UTC Mon** (next row). | U |
| **T0 Mon 11-23** | **00:00** (Sun 19:00 ET, 16:00 PT) | **Show HN.** Repo URL, plain title, first comment retyped from `show-hn.md` fact notes. Stay until 02:00. | U |
| | 02:00 to 08:00 | Sleep if it is night for you. Check the thread once at wake-up. Answer open questions. | U |
| | **13:30** (08:30 ET, 05:30 PT) | **X launch thread** (`x.md`): hook with the native video, link in the first reply. Reply to everything for 2 hours. Record HN rank and points at each check. | U |
| | 15:00 | If HN found a real error in a claim, fix the claim in the repo and the post before posting anything else. | U |
| **T+1 Tue 11-24** | 13:30 | **LinkedIn launch post** (video; link in the first comment). Reply for an hour. | U |
| | 14:00 | **r/ClaudeAI** post, project flair, the first comment at once. Reply for two hours. | U |
| | 16:30 | **X follow-up 1**: the 98x chart. | U |
| **T+2 Wed 11-25** | 14:00 | **r/mcp** post. | U |
| | 15:00 | **dev.to and Hashnode** copy of blog post 1 (the engineering post), canonical to the site. **YouTube Shorts** upload of the vertical cut. | U |
| | any | claudemarketplaces.com and mcpmarket.com submissions. | U |
| **T+3 Thu 11-26** (US Thanksgiving) | 14:00 | **LinkedIn carousel** (`linkedin.md`). Light day: one r/androiddev comment in the showcase thread if the sub allows it. | U |
| | 16:30 | **X follow-up 2**: the approval guard (with the clip). | U |
| **T+4 Fri 11-27** | 14:00 | **r/ClaudeCode** post (different angle from r/ClaudeAI). dev.to and Hashnode copy of post 2 (the guard). **TikTok and Reels** upload. | U |
| **T+5 Sat 11-28** | 16:30 | **X follow-up 3**: the tokens. Quiet day otherwise. Triage issues. | U |
| **T+6 Sun 11-29** | any | **r/SideProject** (optional). dev.to and Hashnode copy of post 3 (method). | U |
| | 20:00 | Review the week's numbers. If HN never took off, email hn@ycombinator.com once about the second-chance pool; never repost. | U |
| **T+7 Mon 11-30** | any | Retro in `progress.md`. If the jobs benchmark is in, publish post 4 and its cross-posts. Decide on Product Hunt for T+14 (`product-hunt.md`). Submit to awesome-claude-code when the repo is 14 days old or has 100 stars. | U |

HN is first because it is the least tolerant of a weak first hour and the one place that needs you at a fixed time. X goes thirteen and a half hours later, once you know what HN found. LinkedIn and Reddit are a day apart because each needs a free hour of replies, and because Reddit filters the same link posted in several subs the same day. If HN does not take off, the plan continues: X, Reddit, LinkedIn and the registries each stand on their own.

Rules across channels:

- Never post, send or tag the HN thread link. Never ask anyone, anywhere, to vote, like, star or retweet. No sock accounts, no coordination, no vote rings, no paid engagement.
- Each channel gets its own text. Nothing is cross-posted.
- You write every word that is posted. Hacker News bans generated text; LinkedIn reduces reach for it; Reddit readers punish it.
- State the same limits everywhere: emulator-only speed numbers, no physical Android measured, iOS needs a Mac with Xcode, the guard reads English labels only, the demo is sped up 10x.
- Same number, same source, same caveat, everywhere.
- Do not post to r/opensource or r/programming.
- Do not post Show HN twice, and do not delete and repost.

## How to respond in comments

| Situation | Do |
|---|---|
| A question | Answer within 15 minutes in the first two hours (HN) or the first hour (everywhere else), in your own words, from the fact tables in `show-hn.md`. Hard ones first: security, benchmark fairness, tokens, license. |
| "It does not work" | Ask for `phonedriver doctor` output, say you will look, open an issue, link it, say when. Fix the install path first; a first-run failure kills more launches than any criticism. |
| A true criticism | Concede it in the first sentence. "You are right, the speed numbers are emulator only." |
| A false claim about the tool | One calm reply with the source. Do not argue twice. A long reply chain trips HN's flamewar penalty. |
| "Is this AI-written?" | Say plainly what is true: coding agents wrote part of the code, you reviewed it, the benchmarks are your own runs, the words in your replies are yours. [USER: your words.] |
| "Why not mobile-mcp / Appium / adb?" | Fair comparison with the number and its setting; say you did not measure mobile-mcp's speed. |
| AGPL objection | State what it covers once (distribution or a modified network service) and stop. Do not give legal advice. |
| Hostile | Do not reply. Report if it breaks the rules. |
| Praise | Thank once. Ask what they would drive with it. |
| Someone offers to upvote or share | Decline: "Thanks, but please do not vote on my account. Tell me what you tried instead." |

## What to watch (a sheet, one row per hour for T0, per day after)

| Metric | Where | Why |
|---|---|---|
| HN points, comments, rank on /shownew and /show, time in the top 30 | news.ycombinator.com | The 2-hour rule: points in the first two hours decide the rest. |
| GitHub stars per hour | repo, or `gh api repos/hridaya423/phonedriver --jq .stargazers_count` | Stars have a half-life near 24 hours; velocity matters for Trending. |
| Referring sites and unique visitors | repo Insights, Traffic (14 days of data only: export on T+7) | Which channel makes stars. |
| Release downloads by asset | `gh api repos/hridaya423/phonedriver/releases` | Installs, not views. |
| Installer or tap failures, `doctor` outputs in issues | Issues | First-run failures. |
| X: impressions, replies, link clicks, profile clicks, follows | X analytics | The hook's cold-start slot depends on likes per view in the first 2 hours. |
| LinkedIn: impressions, dwell proxy (reactions per impression), comments | LinkedIn analytics | |
| Reddit: score, comments, upvote ratio, removals | each post | A removal in one sub is a signal for the next. |
| Site visits and the top pages | the host's analytics only if you have one that sends no data to third parties; otherwise skip | The product promises no telemetry; check that the site does not break the promise (`site/` has no scripts). |
| Registry views and installs, if shown | registry, Glama | The base that persists after the spike. |
| Issues opened and their types | GitHub | A bug found by strangers is the best outcome you can get. |

Targets, as bands, not promises: 30 stars by T0 end is a healthy HN day; 100 by T+7 needs HN on the front page or two other channels over 40 points. If you are at fewer than 15 stars at T+1 midday, the likely cause is the install path or the first screen, not the channel. Look at the repo as a stranger on a phone, then fix that before posting the next channel.

## Risks the research found

- Phone-agent Show HNs scored 2 to 7 points in round 2; the best scored 53. Plan for a modest result.
- HN readers look for LLM-written text and AI-built repos. Say it first, plainly.
- Show HN may be restricted for a new HN account.
- r/opensource barred AI-related projects in October 2026; r/programming ran an LLM ban trial. Do not post there.
- The speed numbers are emulator-only; the tap row is the closest (1.6x to 2.1x), the 98x is for one specific read. Readers will test. The drafts state the setting everywhere.
- The guard's limits (English labels only; a model set on bypassing it can; `shell` input is not seen; headless `claude -p` has nobody to ask) are written in D27 and the blog post. Readers will find them. Say them first.
- The demo is sped up 10x and, today, stops at a sign-in page without showing the approval prompt. The re-recording fixes that; do not caption the old clip with the guard.
- The first-run path is where launches die. Items 3, 4 and 6 are the highest risk.
- The real-jobs benchmark may show phonedriver losing a job. Publish it either way; a result you hide is a claim you cannot make.

## If it goes wrong

- A real bug surfaces during HN: fix, tag a patch release, say so in the thread.
- The post is flagged or killed: do not repost. Email hn@ycombinator.com and ask; people can vouch.
- A claim is wrong: correct it in the repo and in the thread within the hour, and say what you changed.
- The release binary will not run on someone's machine: the answer is a link to `phonedriver doctor` and an issue, not a defense.
