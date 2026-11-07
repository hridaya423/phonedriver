# Launch research, November 2026 (round 3)

Date of research: 2026-11-07. Rounds 1 and 2 stay in `research.md` (the X algorithm file reading, the HN code, the Reddit and LinkedIn data, the name check). This file adds fresh searches and fetches, and corrects round 2 where it was wrong or too sure. Tags: **[official]** the platform's own page, **[code/data]** outsiders reading code or counting posts, **[anecdotal]** blogs, guides, forum comments, **[UNVERIFIED]** could not be confirmed. A claim tagged anecdotal is a prior, not a fact.

**The biggest gap:** Reddit blocked every direct fetch of a rules page (403 on www, old and .json). All subreddit rule text below is secondhand from mirrors, news and search snippets. Reading each sidebar in a browser is a launch-checklist item (`plan.md`), not optional.

## What this changes in the plan

1. **Evidence for stars is thin and modest.** The best dataset (188k Show HN posts) gives a median of about 1.4 stars per HN point within 48 hours, a star half-life near 24 hours, and a correlation of 0.29 between score and stars. An arXiv study of 138 AI/LLM HN-to-GitHub pairs found averages of 121 stars at 24 h, 189 at 48 h and 289 at 7 days, with medians far lower (a few viral launches pull the mean). 100 stars in launch week is realistic only if one or two channels hit; plan for several channels on a short window, not one.
2. **r/opensource and r/programming are out.** r/opensource barred AI-assisted and AI-related projects, including MCP tools, in late October 2026. Do not post there, do not ask for an exception.
3. **HN title and text are human-only.** The guidelines ban generated and AI-edited text. A project that is about AI agents and was built with them gets read with suspicion. Say so in the comment.
4. **X: the link question is settled enough.** X's own product head says links are not penalized, and the open code has no link multiplier. The user asked for the link in a reply and not the hook, so the thread does that, with the repo also in the profile. See the X section.
5. **Product Hunt is optional.** No data shows it converts to stars for an AGPL CLI. Ship it only on a separate day with a clean landing page, after HN.
6. **Lobsters is closed to a cold start.** Invite only, with restrictions for 70 days. Do not plan on it.

## Hacker News

**Show HN rules** [official] (https://news.ycombinator.com/showhn.html)
- Must be something people can run or hold. Hardware may use a video or a detailed article.
- Must be a non-trivial project. "Quickly generated one-offs don't count." You built it and can discuss it.
- Usable without sign-up or email gets more feedback.
- Blog posts, landing and sign-up pages, newsletters, lists and fundraisers do not qualify. Routine version releases do not. A major overhaul may.
- The title starts with "Show HN". Posts go to shownew first and move to the Show page after a small points threshold.
- Do not ask friends to upvote or comment. Reply to feedback with curiosity.

**Guidelines** [official] (https://news.ycombinator.com/newsguidelines.html)
- Titles: no shouting, no exclamation points, no praise, no gratuitous numbers, do not editorialize.
- "Don't post generated/AI-edited comments. HN is for conversation between humans." It also says not to put generated or AI-edited text in posts, and not to automate posting. The date this was added: reported as about 2026-03-11 [anecdotal]; dang said earlier that generated comments were never welcome. Moderators have said they ban accounts for it (https://news.ycombinator.com/item?id=47331381).
- Do not solicit upvotes, comments or submissions.
- The 80-character title limit is enforced by the submit form [UNVERIFIED in the guidelines text; widely cited]. A URL shortener is not allowed in practice; use the plain URL.

**Ranking**
- [official] The FAQ: points divided by a power of age, plus flags, anti-abuse software, demotion of overheated threads, account or site weighting and moderator action. Karma does not raise rank (https://news.ycombinator.com/newsfaq.html).
- [code/data] The commonly published form is `(points - 1) / (age_hours + 2)^1.8`, with a comment-heavy penalty. Shirriff notes constants change and hidden penalties are widespread (https://www.righto.com/2013/11/how-hacker-news-ranking-really-works.html). Treat the exponent as indicative.
- [official] The voting-ring detector exists and its mechanics are secret (dang, 2026-03-20, https://news.ycombinator.com/item?id=47458525, from round 2). "Our software ignores most promo-votes."
- Round 2 said GitHub or domain handling and a "second-chance pool" were documented. The FAQ does not describe either by name; the pool is a moderator practice (https://news.ycombinator.com/item?id=26998308). Email hn@ycombinator.com to ask.
- [official] Reposts are fine only after a significant gap. Do not delete and repost.
- `news.ycombinator.com/showlim` [official, round 2]: new accounts may be restricted from Show HN. Check yours.

**Timing**
- [code/data] 188k Show HNs: Monday 00:00 UTC (Sunday 7 pm ET, 4 pm PT) gives the best chance to reach 50+ points, 10.8%. Next Sunday 02:00 UTC (9.8%), Saturday 19:00 UTC (9.2%). Worst: Thursday 06:00 UTC (2.6%). (https://danfking.github.io/blog/2026/04/23/show-hn-by-the-numbers/)
- [code/data] A 157k-post BigQuery analysis: Sunday best (11.75% breakout), peak hour 12:00 UTC. The two analyses disagree on the hour and agree that weekends beat weekdays (Myriade, https://www.myriade.ai/blogs/when-is-it-the-best-time-to-post-on-show-hn).
- [code/data] An arXiv study of AI/LLM HN-to-GitHub pairs: 12 to 17 UTC gave the best star gains (https://arxiv.org/html/2511.04453v1). That is about stars, not points.
- About 28k Show HNs in 2025, roughly 200 a day.
- Round 2 own count (11,542 Show HNs, Aug to Nov 2026): 00 UTC best at 9.1% reaching 20+ points; 13 to 17 UTC 5.5 to 6.1%; 08 UTC worst at 1.9%.
- Reading: weekend evening US time (Monday 00:00 UTC) has the best hit rate; the weekday 13 to 17 UTC window has the most readers and the best star conversion in the one study that measured stars. The plan uses Sunday evening for HN and spends the Monday 13:00 to 15:00 UTC window on X.

**Norms**
- Post the repo URL. 27% of Show HNs link GitHub [code/data]. No source compares repo against blog; the rules exclude blog posts, so the repo is the safe choice [UNVERIFIED as best].
- Post the first comment right after submitting: who you are, what it does, how it works, what is rough. dang: backstory and what is different; "Don't say nice things about yourself" (https://news.ycombinator.com/item?id=22336638, round 2).

## X

**Algorithm** (https://github.com/xai-org/x-algorithm, released 2026-01-20 per reports; the README lists dated updates through October 2026)
- [official] The ranker predicts the probability of each action and sums weight times probability. Actions: favorite, reply, repost, quote, share, share via DM, share via copy link; clicks (post, profile, link, photo, video open); attention (video quality view, dwell, click dwell, continuation seconds, profile visit seconds); follow author; negatives (not interested, mute, block, report).
- Weights are in `home-mixer/params/param.rs`. Round 2 read them: reply 5.0 (plus 15.0 on an original post by a mutual follow), quote 5.0, share via copy link 20.0, share via DM 5.0, follow author 4.0, repost 1.0, like 0.708, click dwell 0.385, click 0.274, open link 0.2. Negatives: not interested -53.07, block -31.2, mute -58.8, report -234.0. Treat as the 2026-11-06 clone.
- [official] Post-score adjustments: author diversity (a second post from one author in a batch keeps 0.625 of its score, the third 0.4375, floor 0.25), a factor below 1 for out-of-network posts, a cold-start boost for authors under 50k followers (original post under 2 h old with fewer than 200 impressions, picked by likes per home view).
- [code/data] The 2023 weights (reply that the author answers 75, reply 13.5, like 0.5, report -369) are old. The direction holds: conversation beats likes.
- The README names no link penalty. "Link" appears only as a click action.

**Links** [anecdotal, from X executives]
- Nikita Bier, April 2026: link penalties are a myth, "just post the link in the body" (round 2 read the status through the syndication endpoint). July 2026: told Zuckerberg a link need not go in a reply; Musk said X has not penalized links for over a year (https://roboin.io/article/en/2026/04/13/x-denies-link-deboost-recommends-in-post-links/, https://freepressjournal.in/tech/x-product-head-nikita-bier-confirms-link-penalty-removed-over-a-year-ago-tells-mark-zuckerberg-he-can-post-them-directly). Unverified independently.
- A study cited in these articles says links correlate with lower reach [UNVERIFIED, not opened].
- Out-of-network replies are dropped from ranking (round 2, `OONRetweetReplyFilter`), so a link in a reply is seen by fewer people than a link in the post.
- **Decision for the thread:** the hook post is the native video, no link. The first self-reply carries the repo link with one line of context. This follows the user's brief, avoids the doubtful link-in-hook case, and the cost (non-followers see fewer replies) is accepted: the profile and the video carry the discovery, and the video's closing frame shows `phonedriver.dev`.

**Video, dwell, threads**
- Video quality view counts only above 10 s and has weight 0 in the round 2 read. Autoplay and best length [UNVERIFIED]. A 25 to 35 s video with a first frame that reads on mute is the safe guess.
- Dwell and share variants are scored. Bookmark weight [UNVERIFIED].
- Author diversity discounts several posts from one account in one feed. Replies in a self-thread are also filtered as duplicated conversation after selection [official README]. One strong hook beats a long thread of equal strength; the thread's job is the reader who clicked into it.
- Premium reach boost: not found in the 2026 code [UNVERIFIED].
- Best time: no primary data. Weekday mornings US time is the working guess.
- Accounts and communities: no handle was verified as active for this research. `x.md` lists candidates to check by hand before tagging anyone.

## Reddit (secondhand; read each sidebar before posting)

- [UNVERIFIED] The "9:1" self-promotion guideline: third parties repeat it (https://redship.io/blog/reddit-self-promotion-rules-2026); Reddit's own page was not readable. Most subs write their own rules.
- Disclose authorship in the first line ("I built this") [anecdotal norm].
- **r/ClaudeAI** [anecdotal, mod comments on a mirror, 2026-04-15]: a "Built with Claude Project Showcase Megathread" exists. A showcase on the main feed needs the poster to have total karma of at least 50; newer accounts are sent to the megathread. Requirements are in "Rule 7", whose text could not be read (https://redlib.groet-infra.nl/r/ClaudeAI/comments/1sly3jm/built_with_claude_project_showcase_megathread/oh1d3gn/?context=3). Round 2: a mod bot comments on project posts without the project flair.
- **r/opensource** [anecdotal, news 2026-10-26 and 27]: projects that use AI, are made by AI or are for AI are barred, MCP tools named; also project age at least a year, 3 or more contributors, account at least a year old (https://winbuzzer.com/2026/10/27/reddits-open-source-forum-bans-ai-assisted-projects-xcxwbn/, https://helpnetsecurity.com/2026/10/26/reddit-r-opensource-ai-ban). phonedriver fails all of it. **Do not post.**
- **r/programming:** an LLM-content ban trial began 2026-04-01; current rule not read. Treat as comment-only. **Do not post.**
- **r/LocalLLaMA:** new rules set announced in 2026 with self-promotion as the main abuse area, partly enforced by AutoMod with karma gates; thresholds not found. A local-model angle is required for fit; phonedriver has no local-model result in the repo, so skip until one exists.
- **r/androiddev:** old rules (2023) capped promotion under 50% of an account's activity and routed projects to a Showcase thread; current rules unknown [UNVERIFIED].
- **r/selfhosted:** "New Project Friday"; a 3-month age rule was discussed. Weak fit. Skip.
- **r/mcp, r/ClaudeCode, r/iOSProgramming, r/SideProject:** no rules found [UNVERIFIED]. Round 2 saw phone-MCP posts in r/mcp score 7 to 28 points.
- Day and time: no reliable data. Round 2's one vendor snippet puts the busiest developer window at 14:00 to 17:00 UTC on a Tuesday [anecdotal, unopened].
- Hot rank (archived code, round 2): `log10(|score|) + age / 45000 s`. Ten times the net score equals 12.5 hours of age. Early votes count most.

## LinkedIn

No official ranking source for the numbers below was found beyond round 2's LinkedIn engineering posts. What round 2 verified from LinkedIn itself: dwell counts when at least half the update is visible; engagement bait is demoted; "it's ok to ask for thoughts and feedback"; 1 to 3 hashtags; no official link penalty statement (https://www.linkedin.com/blog/engineering/feed/understanding-feed-dwell-time, https://news.linkedin.com/2026/ImprovingTheFeed, https://www.linkedin.com/blog/content-recommendations-avoid).
- [anecdotal] Dwell time, saves, substantive comments and private shares outweigh first-hour likes (https://www.webtures.com/insights/linkedin-algorithm-2026/).
- [anecdotal, round 2 dataset] Document (PDF) posts: 7.00% engagement against text 4.50% and link 3.25% on company pages (Socialinsider, 1.3M posts). One guide says carousels get 2 to 3x the dwell of text; its source for 6.8% is not shown.
- [anecdotal] External links in the body cut reach by 10% to 60% across sources; a first-comment link works but "stopped being free" (Ordinal). Plan: no link in the body, link in the first comment, and be fine with fewer clicks.
- [anecdotal] "5 substantive comments in the first hour gives 4x reach" is unsourced; do not rely on the number. Replying to comments helps (Buffer, own data).
- Length, native video, hashtags: no reliable source. Round 2: 2,000 to 3,000 characters and under 150 both did well (Ordinal, 248k posts).

## Product Hunt

- [official] 12:01 am Pacific is the recommended launch time. You may share the link but not ask directly for upvotes. No hunter needed. Personal accounts only. (https://www.producthunt.com/launch)
- [anecdotal] Hunter reputation no longer matters for ranking; relaunches within about 6 months may face review; launches can be scheduled up to a month ahead; fake upvotes are cleared.
- Verified-user changes 2025 to 2026: [UNVERIFIED].
- No dataset for dev-tool star conversion. A single self-reported OSS case claims 200 to 600 stars (https://gingiris.github.io/growth-tools/blog/2026/03/25/how-to-get-more-github-stars-the-definitive-guide-33k-stars-case-study/) [anecdotal].
- **Recommendation:** optional. Do it on a separate day, after HN, only if the site's landing page lets a visitor see the demo and copy the install line in 10 seconds. See `product-hunt.md`.

## Lobsters

- [official] Invite only, public invite tree. New users are restricted for 70 days: no invites, no flags, no links to unfamiliar domains. Self-promotion should stay under 25% of your stories and comments over a year. Tags come from a fixed list. (https://lobste.rs/about)
- Practical read: not available for launch week unless a regular finds the project and submits it. Do not ask for an invite for this purpose.

## dev.to and Hashnode

- [official] dev.to supports `canonical_url` in front matter (https://dev.to/help/writing-editing-scheduling).
- [anecdotal] Hashnode: canonical URL field in post settings; the 2019 guide describes import. The current UI is [UNVERIFIED].
- [official] dev.to's AI-assisted article guidelines allow AI help with a label and accuracy checks, and say AI-generated articles must not promote a business, program or course (https://dev.to/guidelines-for-ai-assisted-articles-on-dev, last updated April 2024). Hashnode's terms make you responsible for AI output (https://hashnode.com/terms).
- Tags and reach for dev tools: no data found [UNVERIFIED].
- SVG upload: not verified. The plan exports PNG to be safe.
- **Practical read:** the user rewrites each post's intro in their own words and discloses AI assistance if any; the posts are about a product the author owns, so the "promotes a business" rule is a risk to read, not to ignore.

## Short video

- [official] YouTube Shorts: up to 3 minutes, square or vertical (https://support.google.com/youtube/answer/15424877).
- [anecdotal] 9:16, 1080x1920, MP4 for all three (https://www.kapwing.com/resources/social-media-video-aspect-ratios-and-sizes-the-2025-guide/, https://adwave.com/resources/video-sizes-specs-2026). Safe-zone figures differ by guide (bottom overlay 250 to 480 px); keep text in the central 1080x1350 and preview on a phone.
- Reels and TikTok maximum lengths: sources conflict [UNVERIFIED]. 20 to 30 s fits all.
- Ideal length, hook norms, hashtags, caption norms: no reliable source. `short-video.md` uses common practice and says so.

## What drives stars in launch week

- [code/data] Median about 1.4 stars per HN point at 48 h; star effect half-life about 24 h, about 92% done by 48 h; AI/ML and CLI projects convert slightly better; score explains about 8% of star variance (https://danfking.github.io/blog/2026/04/23/show-hn-by-the-numbers/).
- [code/data] arXiv, 138 AI/LLM pairs: average +121 stars at 24 h, +189 at 48 h, +289 at 7 days; medians much lower; Show HN did not differ significantly from non-Show HN (p = 0.39) (https://arxiv.org/html/2511.04453v1).
- [code/data, not opened] A difference-in-differences study of 18,253 repos reports HN posts raise stars and forks, most for small projects [UNVERIFIED citation].
- [anecdotal] Trending: GitHub publishes no criteria; third parties read it as star velocity over 24 to 48 hours against the repo's own baseline. Stacking channels inside one day builds velocity.
- **Reading for phonedriver:** a median HN post of 20 points would give about 28 stars. 100 stars in a week needs a front-page Show HN, or X plus Reddit plus registry traffic stacked on the same two days. Registry listings add a slow trickle after launch and are the cheapest source of installs from people who already want an MCP server.
