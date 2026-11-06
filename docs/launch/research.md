# Launch research (2026-11-06)

This file holds the evidence behind the drafts in this folder. Every item has a source. "Seen" means the research pass read the page or API reply. "Secondary" means a blog or news article reported it. "Unverified" means a source made the claim and nothing confirmed it. Do not quote an unverified item as fact.

## What the research changes about the plan

1. **HN bans LLM-written text.** The Show HN tips page says: "Write your text by hand. Don't use an LLM to generate any of it (not even a tiny bit, including to edit or spruce it up)." The guidelines say: "Don't post generated text or AI-edited text." The first comment in `show-hn.md` is a fact sheet and a draft. The author must retype it in their own words before posting. The same rule applies to replies in the thread.
2. **The category has no HN track record.** Phone-agent Show HNs scored 2 to 7 points. The best one scored 53. Browser and desktop agents scored 99 to 749. A strong title, a first comment with numbers, and an install that works on the first try must carry this launch. The niche will not.
3. **Another project already uses the name.** `github.com/sauravtom/phone-use` is MIT-licensed, has 6 stars, and describes itself as "Control Android phones from coding agents through MCP, a CLI, or a skill" (GitHub API, seen 2026-11-06). Expect search confusion and one "how is this different" comment. The prepared answers cover it.
4. **"Easy to try" is a hard gate.** The README says nothing is published yet. Show HN rules say: "If your work isn't ready for users to try out, please don't do a Show HN." Release binaries must be live first.
5. **X no longer needs the link in a reply.** Nikita Bier wrote on 2026-07-28: "Hello Mark, you do not need to put the links in replies anymore." Elon Musk wrote on 2026-07-29: "We haven't for over a year." (seen, via ppc.land). The X post keeps the link in the body.
6. **Real-time demos are rare in this niche.** One competing README says its phone capture is "shown at 2x speed". Our clip is real time, cut only at the start and end. The posts say so.

## Show HN

### Rules and moderation (primary, seen)

| Source | What it says |
|---|---|
| https://news.ycombinator.com/showhn.html | On topic: "things people can run on their computers or hold in their hands." Off topic: "blog posts, sign-up pages, newsletters, lists." "Please make it easy for users to try your thing out, ideally without barriers such as signups or emails." "Don't post quickly-generated one-offs; anybody can do that now." "Please don't ask friends to upvote or comment." The project must be something you "worked on personally" and are "around to discuss". |
| https://news.ycombinator.com/newsguidelines.html | "Please use the original title, unless it is misleading or linkbait; don't editorialize." "Please don't do things to make titles stand out, like using uppercase or exclamation points." Crop a "gratuitous number or number + adjective". "Don't solicit upvotes, comments, or submissions." "Don't post generated text or AI-edited text." |
| dang, Show HN tips, https://news.ycombinator.com/item?id=22336638 (edited 2026-03-28) | Include "the backstory of how you came to work on this, and explaining what's different about it." Include "a clear statement of what your project is or does. If you don't, the discussion will consist of 'I can't tell what this is'." "Drop any language that sounds like marketing or sales." Do not name your account after the project. Put your email in your profile so moderators can send a repost invite. Boosters in the thread hurt. |
| dang, https://news.ycombinator.com/item?id=23631761 | "Don't try to sell them, try to interest them. If you sell them, they will close the tab. If you interest them, they will sell themselves." |
| dang, https://news.ycombinator.com/item?id=14841231 | "Avoid overly slick website design and (especially) marketing language... Information density is good." |
| dang, https://news.ycombinator.com/item?id=22850431 | Sign-in walls: "It's particularly repelling to readers if they click on a Show HN title only to go to a page that contains nothing but a call to sign up." |
| dang, https://news.ycombinator.com/item?id=44584688 | Moderators edit titles that use metaphors: "a metaphor probably shouldn't be used in a title like this, since many readers will take it literally." |
| dang, https://news.ycombinator.com/item?id=38259288 | "Show HN: The World's Cheapest Supercomputer" was "baity". |
| dang, https://news.ycombinator.com/item?id=26998308 | The second-chance pool: moderators lift overlooked posts onto the front page at random. |

HN titles are limited to 80 characters. The research sample of 600 Show HN posts with more than 200 points had a median title of 61 characters, and none was over 80 (own count from the Algolia API, 2024 onward; the sample is the first 600 results, not a random draw).

### Analyses of what works (mixed quality)

| Source | Method | Claims |
|---|---|---|
| https://danfking.github.io/blog/2026/04/23/show-hn-by-the-numbers/ | 188,085 Show HNs, 2012 to April 2026 | Median score 2, 90th percentile 24, 99th percentile 263. Highest chance of 50+ points: Monday 00:00 UTC (Sunday 7 pm ET), 10.8%. Lowest: Thursday 06:00 UTC, 2.6%. Mean score peaks 12:00 to 15:00 UTC (7 to 10 am ET). About 1.4 GitHub stars per point; the star bump has a half-life of about 24 hours. |
| https://www.myriade.ai/blogs/when-is-it-the-best-time-to-post-on-show-hn | 157k Show HNs, BigQuery | Breakout (30+ votes) is likeliest on Sunday (11.75%), best hour 12:00 UTC (8 am EDT). No per-bucket sample sizes. |
| https://hn-ph.vercel.app/analysis | About 4M stories | Day effects are significant but small. Hour of day is not significant. Seen only through search summaries; a commenter disputed the method. Unverified. |
| https://lucasfcosta.com/hn-launch | Practitioner advice, no numbers | "There's no best time to post." "Make your title direct and specific." "Your repo is the landing page." |
| https://dev.to/iris1031/how-to-launch-on-hacker-news-the-show-hn-guide-real-data-461e | Practitioner guide, "stats" table unsourced | First comment in five parts: one-sentence technical description, the personal reason, one architectural choice and its tradeoff, one honest limitation, one specific question. Reply within 15 minutes for the first 2 hours. |
| https://blog.nseldeib.com/p/what-i-learned-launching-codeyam | One failed launch | A prepared launch with demo videos and a pre-drafted comment still sank twice. The author asked friends to engage. Polish does not rescue a post. |

Timing reading: the two large datasets say Sunday evening US time has the best breakout rate, and weekday mornings US time have the best mean score. They disagree, and content outweighs timing in every source that tested it. `plan.md` picks a weekday morning because the author must be present for two hours and awake.

### Twelve Show HN posts that did well (dev tools, agents, MCP)

Points and comment counts are from the Algolia API or the item page. "Why it worked" is inference from the first comment and top replies, not the authors' statement.

| Title | Points / comments | Date | Item | Why it worked |
|---|---|---|---|---|
| Show HN: Browser MCP – Automate your browser using Cursor, Claude, VS Code | 616 / 217 | 2025-04-07 | https://news.ycombinator.com/item?id=43613194 | One clear pain: Playwright MCP opens a fresh browser, this uses your logged-in Chrome. The author answered "why not Playwright" with a mechanism. |
| Show HN: Factorio Learning Environment – Agents Build Factories | 749 / 209 | 2025-03-11 | https://news.ycombinator.com/item?id=43331582 | Thread not read. Likely a vivid demo. Unverified. |
| Show HN: Semble – Code search for agents that uses 98% fewer tokens than grep | 445 / 151 | 2026-05-17 | https://news.ycombinator.com/item?id=48169874 | A hard number in the title, with the method and benchmark size (about 1,250 query pairs, 63 repos) in the text. |
| Show HN: Vibium – Browser automation for AI and humans, by Selenium's creator | 443 / 124 | 2025-12-24 | https://news.ycombinator.com/item?id=46377597 | Credibility in the title. A short technical first comment: single binary, one install command, one line to add it to Claude. |
| Show HN: Skyvern – Browser automation using LLMs and computer vision | 422 / 139 | 2024-03-14 | https://news.ycombinator.com/item?id=39706004 | Open source, demo video, grounded in talks with companies. |
| Show HN: Agent.exe, a cross-platform app to let 3.5 Sonnet control your machine | 406 / 232 | 2024-10-23 | https://news.ycombinator.com/item?id=41926770 | Posted on the day Anthropic launched computer use. Most comments were safety warnings. Timing, not craft. |
| Show HN: Stagehand – an open source browser automation framework powered by AI | 326 / 86 | 2025-01-08 | https://news.ycombinator.com/item?id=42635942 | A code sample and a one-line way to try it. |
| Show HN: Ghidra MCP Server – 110 tools for AI-assisted reverse engineering | 298 / 68 | 2026-02-04 | https://news.ycombinator.com/item?id=46882389 | Niche audience, one real technical idea explained in the first comment. |
| Show HN: MCP server for searching and downloading documents from Anna's Archive | 256 / 79 | 2025-07-09 | https://news.ycombinator.com/item?id=44514753 | Simple and useful. The author shipped a CLI after a commenter asked. |
| Show HN: WhatsApp MCP Server | 229 / 138 | 2025-03-31 | https://news.ycombinator.com/item?id=43532967 | A plain local-first privacy statement and a plain reason for building it. |
| Show HN: Drive any macOS app in the background without stealing the cursor | 192 / 43 | 2026-04-28 | https://news.ycombinator.com/item?id=47936312 | A specific problem and a write-up of the dead ends. One commenter objected to telemetry on by default. |
| Show HN: Playwright Skill for Claude Code – Less context than playwright-MCP | 189 / 45 | 2025-10-20 | https://news.ycombinator.com/item?id=45642911 | A token-cost complaint with a link to the upstream issue. |
| Show HN: Agent-desktop – Native desktop automation CLI for AI agents | 99 / 44 | 2026-05-02 | https://news.ycombinator.com/item?id=47982708 | Closest design to ours (accessibility tree, one Rust binary). Its thread is the best preview of ours; see below. |

Patterns across them: a number or a named mechanism in the title; the author answers "why not X" with how the two differ; the first comment is short and technical. In the title sample, 36% used a dash separator, 24% held a digit, and 10 of 600 used "fastest", "best" or "first".

### Phone and mobile agents on HN

| Post | Result | Item |
|---|---|---|
| Show HN: MCP server to automate your mobile (runablehq) | 2 points | https://news.ycombinator.com/item?id=43559394 |
| Show HN: Droidrun – LLM Agent for Android | 4 points, 0 comments (two earlier posts: 2 and 3 points) | https://news.ycombinator.com/item?id=45731214 |
| Show HN: I built an AI Agent that uses the iPhone (PhoneAgent) | 53 points, 13 comments | https://news.ycombinator.com/item?id=44155426 |
| Show HN: Palmier – bridge your AI agents and your phone | 7 points, 8 comments | https://news.ycombinator.com/item?id=47843841 |
| Show HN: MCP app for Android, drive apps via AI (no root, PII redacted locally) | 6 points | https://news.ycombinator.com/item?id=49362047 |
| Maestro – Next generation mobile UI automation (link post) | 185 points, 66 comments | https://news.ycombinator.com/item?id=43174453 |

The research found no HN item for `mobile-next/mobile-mcp`. That absence is unverified.

Objections found in the threads, with the answers that worked:

| Objection | Where | Answer that landed |
|---|---|---|
| Giving an agent control of a device is dangerous ("like knowingly installing malware"; "it just used my card to book round trip tickets") | Agent.exe, https://news.ycombinator.com/item?id=41927358 and https://news.ycombinator.com/item?id=41927379; Palmier, https://news.ycombinator.com/item?id=47846399 | State the blast radius and the controls. Do not claim safety you cannot show. |
| "Local" privacy claims are false if screen text goes to a model API | Browser MCP, https://news.ycombinator.com/item?id=43613935 | The author narrowed the claim: the tool is local; the model provider is a separate decision (https://news.ycombinator.com/item?id=43613971). |
| Cost and tokens ("Sonnet 3.5 is reaaaallly expensive"; Maestro "seems to be token hungry") | https://news.ycombinator.com/item?id=41927413, https://news.ycombinator.com/item?id=47983840 | Lead with token and speed numbers (Semble, the Android MCP post). |
| Flakiness: "a LLM (non-deterministic) solution for deterministic problems" | AI mobile testing threads, https://news.ycombinator.com/item?id=39769412 | Publish an evaluation. "How do you evaluate your tool, and have you published your evaluation?" |
| "Why not Appium or Maestro?" | https://news.ycombinator.com/item?id=43221698 | Compare on a concrete mechanism and a number. |
| Platform gaps: macOS-only tools, no iOS | Agent-desktop, https://news.ycombinator.com/item?id=47982708 | Say the gaps first. |
| Accessibility trees miss lazy lists and custom views | https://news.ycombinator.com/item?id=47984524, https://news.ycombinator.com/item?id=47992922 | Name the limit and the fallback before a commenter does. |
| "Seems LLM-generated" text | https://news.ycombinator.com/item?id=47983478 | Write by hand. |
| No license, no trial, closed source | https://news.ycombinator.com/item?id=43614373 (no license); Momentic, https://news.ycombinator.com/item?id=50339913 ("Closed source, non self-hosted in 2026? Nah.") | Link a license and a one-line install. |
| "What stops the model vendor from absorbing this?" | Momentic thread | Not answered well in any thread read. |

## X

### Mechanics

| Claim | Source | Quality |
|---|---|---|
| The open-sourced ranking code lists these predicted actions in a weighted sum: favorite, reply, repost, quote, share, link click, dwell, video quality view, follow author. Negatives: not interested, mute, block, report, "not dwelled". It has no separate rule for posts that contain links. | https://github.com/xai-org/x-algorithm | Primary, seen |
| The 2023 published weights: like 0.5, retweet 1.0, reply 13.5, reply the author answers 75.0, good profile click 12.0, negative feedback -74.0, report -369.0. Old, but the only published numbers. | https://github.com/twitter/the-algorithm-ml/blob/main/projects/home/recap/README.md | Primary, dated |
| Links: Bier, 2026-04-13, said the link penalty claim was "Fake news" and advised posting the link in the body "and provide sufficient context about what the user will see before they click it". Bier, 2026-07-28, and Musk, 2026-07-29, as quoted above. | https://roboin.io/article/en/2026/04/13/x-denies-link-deboost-recommends-in-post-links/ ; https://ppc.land/x-drops-year-old-link-penalty-musk-tells-zuckerberg-on-platform/ | Statements by X staff, seen via news |
| Counter-evidence: a Nieman Lab analysis of 18 accounts found link-heavy accounts with lower median engagement. The open-source release does not count link clicks as engagement, so link posts may score lower by that route. Bier blamed bare one-line captions. | https://gigazine.net/gsc_news/en/20260410-x-post-with-link-low-engagement-with-link | Observational, confounded |
| Native video autoplays muted; link cards to YouTube or Loom do not. Free accounts: 140 s and 512 MB, MP4 or MOV. | https://www.flowjam.com/blog/x-launch-video | Vendor |
| Timing: vendor studies disagree between Wednesday and Thursday, with 9 to 11 am in the audience's time zone as the common window. | https://opentweet.io/blog/best-time-to-post-on-twitter-data-study ; https://posteverywhere.ai/blog/best-time-to-post-on-x | Vendor, pooled accounts |
| Tagging: blogs claim that tagging many accounts throttles reach. No primary source. | https://budgetpixel.com/blog/why-x-posts-are-getting-hidden-after-the-midmay-update-and-how-creators-can-avoid-it | Unverified |
| Thread length and format: no controlled data. | https://supabird.io/articles/what-makes-developer-posts-go-viral-on-x-(twitter) | Vendor |

Reading: the post must earn replies, dwell and reposts on its own. The author's replies carry the largest published weight, so answer every reply. The research could not verify GIF autoplay or X's alt-text rules, so the plan uses MP4 and writes plain alt text.

### Examples (what the research could verify)

| Example | What happened | Source |
|---|---|---|
| Rork, posted by a third party (Matt Shumer), 2025-02-24 | A video demo and one line ("Rork lets you create entire iOS apps just by describing them!"). Over 1M views. The founders' own post on Feb 12 "didn't take off". Third-party endorsement worked. | https://techcrunch.com/2025/05/05/rorks-founders-were-almost-broke-when-a-viral-tweet-led-to-2-8m-and-a16z/ |
| browser-use, through a post about Manus (@jianxliao), March 2025 | 2.4M+ views on someone else's post. Daily downloads went from about 5,000 to 28,000 in a week. | https://techcrunch.com/2025/03/12/browser-use-one-of-the-tools-powering-manus-is-also-going-viral/ |
| Droidrun, April 2025 | A short demo clip. The founders report 2,000+ GitHub stars in 48 hours (self-reported). Later a Product Hunt #1 with 406 upvotes. | https://coderlegion.com/5020/from-nobody-to-category-leader-how-we-accidentally-started-the-phoneuse-race ; https://hunted.space/product/droidrun-framework-for-mobile-agent/launches/droidrun |
| Jaana Dogan on Claude Code, 2026-01-04 | Opens "I'm not joking and this isn't funny." A concrete claim (a comparable result in an hour) and an admission ("toy version"). Reported at about 7M views (secondary). | https://simonwillison.net/2026/jan/4/jaana-dogan |
| uv, @charliermarsh, 2024-02-15 | 13-post thread. Opens "Announcing uv: an extremely fast Python package installer and resolver, written in Rust." | https://threadreaderapp.com/user/charliermarsh |
| Ruff, @charliermarsh, 2022-08-30 | 7-post thread. Opens "Python tooling could be much, much faster." Carries a benchmark. | same |
| Karpathy, "vibe coding", 2025-02-02 | Text only, a named concept. 38,004 likes at the time of the check. | https://x.com/karpathy/status/1886192184808149383 |
| Tiny Projects, 2021-11-10 | One video post: about 160k impressions, 31.5k video views, about 1% link clicks. People shared it because it looked good. | https://daily.tinyprojects.dev/42 |
| Rybbit | 5,000 stars in 9 days with no viral X post from the founder. Distribution came from elsewhere. | https://rybbit.com/blog/5k-stars |
| OpenClaw, January 2026 | A demo video of an agent booking calendar slots over WhatsApp. Account, hook and views unverified. | https://www.morphllm.com/clawdbot |

The research did not verify the first-line text for browser-use, Droidrun or Rork's launch. It found nothing usable for Cursor, Amp, Stagehand, Maestro, mobile-mcp, Ghostty, Bun, Zed, Lovable or Replit. Three launches (Rork, browser-use, Droidrun's second wave) grew through other people's posts. Plan for that: make the demo easy to quote.

What reads as slop (secondary, anecdotal): "I built X in a weekend with Claude", undisclosed AI code, course-funnel posts, aggregator posts that end in a bare link. The Show HN flood is described as "tripled and now mostly vibe-coded" (https://hnshowcase.com/post/47864393/show-hn-submissions-tripled-and-are-now-mostly-vibe-coded). What the verified examples share: a stated measurement (Ruff), a real task running unedited (Rork, Droidrun), and a claim that admits its limits (Dogan).

Phone-agent angle: privacy is the live counter-story, because phone agents arrived after the Doubao phone privacy backlash (https://www.scmp.com/tech/tech-trends/article/3335746/chinas-zai-open-sources-ai-agent-tool-phones-after-bytedance-privacy-backlash). The no-telemetry decision (D9) earns one line.

## LinkedIn

Most evidence here comes from vendor blogs. Treat it as direction, not fact.

| Claim | Source | Quality |
|---|---|---|
| The feed uses an LLM retrieval model and a transformer ranker that reads over 1,000 of a member's past interactions. The engineering post gives no advice to creators. | https://www.linkedin.com/blog/engineering/feed/engineering-the-next-generation-of-linkedins-feed | Primary |
| The cut-off before "see more" is about 140 characters on mobile and 210 on desktop. Line breaks count against it. The hard limit is 3,000 characters. | https://medium.com/@artyom.shimanski/the-linkedin-see-more-cutoff-why-210-characters-decide-your-posts-fate-bc05bf211e0f ; https://socialrails.com/blog/linkedin-post-character-limits | Vendor |
| Link penalty: Ordinal's study found 26.5% less reach on posts with links, concentrated on company pages, "almost none" on personal profiles, and that the first-comment trick "stopped being free" around 2024. LinkPost's 358k-post study (cited by Ordinal): median 448 impressions with a link, 705 without. | https://www.tryordinal.com/blog/linkedin-link-penatly-study | Data, method partly unclear |
| LinkedIn has made no public statement on link penalties that the research could find. | n/a | Gap |
| Document carousels draw the most engagement in one analysis (7.00%, 1.3M business posts). No dataset for technical posts. | https://nealschaffer.com/linkedin-post/ | Vendor, secondary |
| Saves and substantive comments outweigh likes. | https://www.socialpilot.co/?p=694640 | Vendor, secondary |
| LinkedIn launched a "Seems like AI slop" option in July 2026. Its product chief said members now see 40% fewer posts classed as AI slop (the article's headline says "views"; the body says "posts"). Write by hand. | https://analyticsindiamag.com/ai-news/ai-slop-on-linkedin-gets-40-fewer-views-confirms-company | Secondary |
| Ad video specs (LinkedIn help): MP4, H.264, AAC; 4:5 at 720x900 recommended vertical; 16:9 at 1920x1080; SRT captions. Organic limits are not documented there. Vendors say 5 GB for organic video. | https://www.linkedin.com/help/lms/answer/85306 | Primary for ads only |
| Timing, reply behavior in the first hour, polls, tagging: no verified data. | n/a | Gap |

Examples (reaction counts as shown when the page was read):

| Author | Opening line | Format | Reactions / comments |
|---|---|---|---|
| Michael Bolin | "Today at Meta we open sourced DotSlash, which is something I came up with in early 2019" | Personal "I made this" text | 505 / 13 |
| Michael Bolin | "It has been awhile since I've published any long-form technical writing," (the Codex agent loop) | Long technical write-up with code links | 447 / 16 |
| Michael Kennedy | "I rewrote Chameleon in Rust." | One-line hook | 89 / 19 |
| Pedro Nauck | "Today we are (re-)launching Compozy as an AI SDLC!" | Launch post | 535 / 62 |
| Manthan Patel (181k followers) | "Someone open-sourced an entire AI agency on GitHub. 147 specialised subagents. 91K stars. MIT licensed. Free." | Curation with a list | 121 / 65 |

Profile pages: https://www.linkedin.com/in/michael-bolin-7632712/, https://www.linkedin.com/in/mkennedy/, https://www.linkedin.com/in/pedronauck/. The Patel post: https://www.linkedin.com/posts/leadgenmanthan_someone-open-sourced-an-entire-ai-agency-activity-7459640972910456832-pPxS.

What the examples show: personal posts about a specific artifact beat company-page milestone posts, and account reach dominates (a 91K-star repo drew 121 reactions from an account with 181k followers). One write-up of a zero-audience CLI launch on LinkedIn reports about 1,000 impressions and almost no comments (https://dev.to/krit83/i-launched-an-open-source-cli-tool-with-zero-audience-heres-what-happened-in-10-days-1d6g). Expect a modest result and treat LinkedIn as the lowest-priority channel.

Tone: "thrilled to announce" reads as a template (https://www.teamblind.com/post/anyone-else-think-thrilled-to-linkedin-posts-are-cringe-ft3fdh78). One-sentence-per-line "broetry" draws mockery (https://boingboing.net/2017/12/07/broetry-is-the-perfect-bloggin.html). What the vendor advice and examples favor: a result in the first line, a stated failure or tradeoff, and links to real code.

## Gaps

- No verified viral open-source launch by a solo engineer on LinkedIn.
- No verified first-line text for the biggest X launches.
- No verified X alt-text rules or GIF autoplay behavior.
- No primary data on posting time for X or LinkedIn.
- Competitor facts used in `show-hn.md` come from `docs/research/competitors.md`, which says its fetches went through a summarizing tool. Re-open each competitor page before quoting it.

---

# Round 2: ranking mechanics per platform (2026-11-06)

Round 1 above stays as the evidence for tone, examples and objections. This part reads each platform's ranking rules from the primary source where one exists. "Code" means a file the research read. "dang" means a Hacker News moderator comment. Anything marked unverified came from one secondary source.

## Hacker News

### Ranking

| Fact | Source | Quality |
|---|---|---|
| Old Arc code: rank is `(points-1)^0.8 / (age_hours + 2)^1.8`. A story with no URL is multiplied by 0.4. "Lightweight" stories (image posts, listed domains) by 0.3. A story with more than 20 comments is scaled by `min(1, (points / comments)^2)`. Votes from suspected sockpuppets are subtracted. | https://github.com/wting/hackernews/blob/master/news.arc | Code, circa 2008. Shape of the formula, not today's constants. |
| The current formula and constants are not public. The FAQ lists other factors: "user flags, anti-abuse software, software which demotes overheated discussions, account or site weighting, and moderator action." Karma does not raise rank. | https://news.ycombinator.com/newsfaq.html | Primary |
| dang names three reasons a story drops: "user flags, the flamewar detector (a software penalty for discussions that usually aren't substantive), and a moderation downweight for not containing new information." | https://news.ycombinator.com/item?id=9965574 | Primary |
| In 2013 a crawl saw the comment penalty active "for any post with more comments than upvotes and at least 40 comments". | https://www.righto.com/2013/11/how-hacker-news-ranking-really-works.html | Secondary |
| Nobody has published a dataset that links first-hour votes to final rank. The numbers in marketing guides ("15 to 20 upvotes by 9:30", "30 to 50 in the first hour") show no data. | https://github.com/social-protocols/quality-news and the guides | Gap, unverified |
| The #1 slot gets about 10.2% of all upvotes, rank 40 about 0.4%. | https://github.com/social-protocols/quality-news | Secondary |

What this means: the age term is steep. A post loses value fast, so the first hours decide the result. A discussion with many more comments than points trips the flamewar penalty. A thread of questions and calm answers does not.

### Flags and the voting-ring detector

| Fact | Source |
|---|---|
| Old code: a story is killed at more than 7 flags. The flag link needs karma above 30 (dang today: "a small karma threshold (31)"). The flag weights and ratio rule today are not public. | news.arc; https://news.ycombinator.com/item?id=11040183 |
| "It's a cumulative effect of individual user flags. When this happens to an active discussion, we typically unkill the post so the discussion can continue." | https://news.ycombinator.com/item?id=9742055 |
| dang, 2026-03-20: "Obviously we don't publish how HN's voting ring detector works... it's a software penalty that has nothing to do with the content of a story." | https://news.ycombinator.com/item?id=47458525 |
| Moderators learn about vote requests from tips: "if someone is asking for HN upvotes on Twitter, we sometimes get links from eagle-eyed HNers." | https://news.ycombinator.com/item?id=24789805 |
| dang, 2025-10-26: friends who create new accounts to upvote are "naive", and software already catches them. An organized hidden ring is "an obvious ban". | https://news.ycombinator.com/item?id=45709215 |
| "Don't ask for upvotes. Our software ignores most promo-votes." | https://news.ycombinator.com/item?id=14841231 |
| The FAQ: "We penalize or ban submissions, accounts, and sites that break this rule." | https://news.ycombinator.com/newsfaq.html |

The research found no dang quote on IP clustering or on direct `item?id=` links. Both are plausible and undocumented. The safe rule: never post the HN thread link anywhere, and never ask a person to vote.

### /shownew, the Show HN limit, the second-chance pool

| Fact | Source |
|---|---|
| "All Show HNs appear on newest and shownew, but there is a small points threshold before a post makes it to ask or show." The threshold is not published. On 2026-11-06 the research counted about 5 new Show HNs per hour on `/shownew`. | https://news.ycombinator.com/newsfaq.html ; own fetch |
| 2026: "We're temporarily restricting Show HNs because of a massive influx, mostly by users who aren't yet familiar with the site or its culture." dang: "we want people to be genuine community members, which is not a checklist process." A new account should expect this. | https://news.ycombinator.com/showlim ; https://news.ycombinator.com/item?id=47389960 |
| Second-chance pool: moderators and reviewers pick old submissions, and software "randomly picks one every so often and lobs it randomly onto the lower part of the front page." To ask for it, email hn@ycombinator.com. Put an email in your profile for repost invites. | https://news.ycombinator.com/item?id=26998308 ; https://news.ycombinator.com/item?id=43824814 |
| Reposts: "Please don't delete and repost the same story." Repost only if "significantly different", once or twice a year. | https://news.ycombinator.com/newsfaq.html ; https://news.ycombinator.com/item?id=22336638 |

### Timing, from data

Own computation: 11,542 Show HNs from 2026-08-06 to 2026-11-04 (Algolia, 3-hour windows, deduplicated). Overall 4.6% reached 20 points and 1.5% reached 100. Points are final scores, so these rates say nothing about speed of climb.

| UTC hour | Posts | % reaching 20+ | Note |
|---|---|---|---|
| 00 | 242 | 9.1 | Sunday 7 pm ET in winter. Few posts, best rate. |
| 23 | 294 | 8.2 | |
| 13 to 17 | 700 to 950 each | 5.5 to 6.1 | Peak volume, middling rate. |
| 08 | 369 | 1.9 | Worst. |

By weekday (UTC): Monday 5.6%, Friday 5.0%, Thursday 4.8%, Tuesday 4.7%, Sunday 4.5%, Wednesday 4.0%, Saturday 3.4%. The weekday differences are small. Hour buckets with 100+ points hold 3 to 25 posts, so they are noisy.

Published studies agree on the low-competition window: Dan King (188k posts) finds Monday 00:00 UTC best (10.8% reach 50+ points) and Thursday 06:00 UTC worst (2.6%); Myriade finds Sunday best (11.75% reach 30+ votes) and 12:00 UTC the best hour; Chanind (2019) found Sunday 06:00 UTC about 2.5 times likelier than Wednesday 09:00 UTC. Sources in round 1, plus https://chanind.github.io/2019/05/07/best-time-to-submit-to-hacker-news.html.

Reading: three datasets and the own computation point to Monday 00:00 UTC (Sunday 7 pm ET, 4 pm PT). Midday weekday posts have the most readers and the most competition.

### Titles, first comment, AI text

Own cut of the same 11,542 posts (correlation only, no controls). Share reaching 20+ points, with the feature and without:

| Title feature | With | Without |
|---|---|---|
| Contains a digit | 5.9% | 4.4% |
| "open source" or "OSS" | 7.0% | 4.5% |
| AI, LLM, agent, GPT or Claude | 3.5% | 5.1% |
| "Rust" | 6.7% | 4.6% |
| Length over 70 characters | 3.9% | 4.7% |
| Length 40 or under | 5.5% | 4.5% |

AI and agent titles do worse than average in this sample, and "Rust" and "open source" do better. A roughly 100k-post analysis found no link between title length and score (https://hn-ph.vercel.app/analysis). Treat all of these as weak.

dang on titles: "accurate and neutral"; baity titles get rewritten (https://news.ycombinator.com/item?id=8489914). On the first comment: "Include text giving the backstory of how you came to work on this, and explaining what's different about it" and "Don't say nice things about yourself or your work. It invites comeuppance" (https://news.ycombinator.com/item?id=22336638 and https://news.ycombinator.com/item?id=14841231). No dataset shows the first comment moves rank.

AI text, exact wording (https://news.ycombinator.com/newsguidelines.html): "Please don't put generated text in HN posts. Write your text yourself" and "Don't post generated text or AI-edited text." dang, 2026-03-11: "we're banning accounts that break it" (https://news.ycombinator.com/item?id=47331381). Detection is by user reports and moderator judgment (https://news.ycombinator.com/item?id=48228318). Readers name "Claude-speak", em-dash chains and generic headings as tells (replies on https://news.ycombinator.com/genai-pushback). The README the post links to counts too, in the community's eyes (https://news.ycombinator.com/item?id=47213630).

## X

All code citations are from the 2026-11-06 clone of https://github.com/xai-org/x-algorithm and the 2023 repo https://github.com/twitter/the-algorithm.

### Scoring

Final score is the sum of weight times predicted probability of each action (README, "Scoring and Ranking"; `xai-value-model/scoring.rs`). The weights are published in `home-mixer/params/param.rs`.

| Action | Weight (2026 default) | Line |
|---|---|---|
| Reply | 5.0 (plus 15.0 on an original post by a mutual follow) | 410, 412 to 416 |
| Quote | 5.0 | 459 |
| Share via copy link | 20.0 | 453 |
| Share via DM | 5.0 | 447 |
| Follow author | 4.0 | 473 |
| Repost | 1.0 | 423 |
| Like | 0.708 | about 404 |
| Click dwell time | 0.385 | 503 |
| Click | 0.274 | 436 |
| Open link | 0.2 | 437 |
| Profile click, video open, video quality view | 0.0 | 439, 431, 444 |
| Not interested, block, mute, report | -53.07, -31.2, -58.8, -234.0 | 510 to 527 |

- The 2023 weights (like 0.5, retweet 1.0, reply 13.5, reply the author answers 75.0, negative feedback -74.0, report -369.0) are in https://github.com/twitter/the-algorithm-ml/blob/main/projects/home/recap/README.md. The "reply the author answers" head is gone from the 2026 list.
- The mutual-follow boost is documented in `docs/BIDIRECTIONAL_BOOST_CHANGE.md` (A/B test 2026-07-10, rolled out at 20 on 2026-07-13, cut to 15 on 2026-07-24). Nikita Bier announced it: "We're rolling out a small tweak to boost visibility of your posts to your mutuals" (https://x.com/nikitabier/status/2076747704248758617).
- Video quality view counts only for videos over 10 seconds and has weight 0 (`MinVideoDurationMs` = 10,000). Video value flows through dwell and continuation signals, whose weights the research did not find.
- Premium: the 2023 code had a 4x in-network and 2x out-of-network multiplier for blue-verified authors (`HomeGlobalParams.scala:89-103`). The 2026 release has none the research could find.

### Reach

- Cold start (`home-mixer/scorers/author_cold_start.rs`, `param.rs` about 609 to 640): an original post from an author with 50,000 followers or fewer, at most 2 hours old, with fewer than 200 home impressions, can be lifted to about rank 15 in a feed. The pick uses Thompson sampling on likes per home view.
- Out-of-network posts are scaled by 0.75 (`vm-ranker/params.rs` about 218 to 222). Replies and reposts from accounts the viewer does not follow are dropped (`OONRetweetReplyFilter`).
- Author diversity: the second-ranked post from the same author in one batch keeps 0.625 of its score, the third 0.4375, the floor is 0.25 (`vm-ranker/params.rs:176-193`).
- Age: posts older than 48 hours are dropped (`filters/age_filter.rs`). The research found no half-life constant. "6-hour half-life" claims are unverified.
- Quality: the 2026 repo labels and drops posts from `EngagementSpammer` authors for non-followers and has an `llm_slop_post` label (`visibility-filtering/rules/author_rules.rs:260-265`). The size of any penalty on a single question post is not in the code.

### Links

- The 2026 scoring code has no link multiplier. `has_url` is only an input feature, and open-link has a small positive weight.
- Bier, 2026-04-12: "Fake news. Link clicks are higher than they've ever been. Just post the link in the body of the post and provide sufficient context about what the user will see before they click it." (https://x.com/nikitabier/status/2043428612108644848, read through the syndication endpoint.)
- Bier 2026-07-28 and Musk 2026-07-29 ("We haven't for over a year") are verified only through https://ppc.land/x-drops-year-old-link-penalty-musk-tells-zuckerberg-on-platform/. The status IDs were not found.

### What the research could not find

No effect for tagging, pinned posts, delete and repost, aspect ratio, caption or first frame, or posting time exists in the 2026 code. All advice on them is unverified.

### Dev-tool launch posts, read through the syndication endpoint

| Post | Media | Likes / replies | Link |
|---|---|---|---|
| @vercel_dev, 2026-08-18: "Introducing fx, a tiny, open, native coding agent from Vercel Labs... Today, we're open sourcing it." | 11 s video | 2,830 / 187 | https://x.com/vercel_dev/status/2089828083415355806 |
| @cursor_ai, 2025-09-29: "Cursor can now control your browser." | 30 s video | 6,162 / 241 | https://x.com/cursor_ai/status/1972778817854067188 |
| @charliermarsh, 2025-05-13: "...the preview release of ty, an extremely fast type checker... written in Rust... 10x, 50x, even 100x faster" | image | 4,845 / 120 | https://x.com/charliermarsh/status/1922333022658978089 |
| @mitchellh, 2026-10-27: "Ghostty 1.4 is now out!" with link in the body | none | 4,956 / 132 | https://x.com/mitchellh/status/2115124583170974193 |
| @jarredsumner, 2026-05-09: "99.8% of bun's pre-existing test suite passes on Linux x64 glibc in the rust rewrite" | image | 3,411 / 126 | https://x.com/jarredsumner/status/2053047748191232310 |
| @badlogicgames, 2026-02-02: "Tired: mac mini + clawdbot / Wired: anfroid + termux + pi" (an adb-driven phone agent) | 37 s portrait phone video | 378 / 28 | https://x.com/badlogicgames/status/2018200939979526335 |
| @JPoehnelt, 2026-06-23: "Two months ago I was fired by Google for creating the Google Workspace CLI. It went viral, hit #1 on Hacker News..." | image | 14,718 / 600 | https://x.com/JPoehnelt/status/2069482265953087602 |

Most are large accounts. The adb phone agent post is the nearest match in size and subject. Posts with the link in the body (Ghostty) show no sign of a penalty.

## Reddit

The research could not read live Reddit pages. It used mirrors and news coverage. Verify every rule in a browser before posting.

### Ranking and spam rules

- Hot rank (archived code, https://github.com/reddit-archive/reddit/blob/master/r2/r2/lib/db/_sorts.pyx): `sign * log10(max(|ups - downs|, 1)) + (post_time - 1134028003) / 45000`. 45,000 seconds is 12.5 hours. Ten times the net score equals 12.5 hours of age. The first 10 net votes weigh as much as the next 90. The live system may differ.
- Comments sort by the Wilson lower bound at 80% confidence (same file).
- Vote fuzzing exists (https://techcrunch.com/2016/12/06/reddit-overhauls-upvote-algorithm-to-thwart-cheaters-and-show-the-sites-true-scale/amp/).
- The old "10% self-promotion" guideline is disputed as official, and many subreddits copy it (https://redship.io/blog/reddit-self-promotion-rules-2026). Subreddit rules are stricter than site rules.
- Filters: brand-new accounts that post repository links, the same link in many subreddits, and coordinated voting get removed (secondary guides, unverified).
- r/LocalLLaMA added minimum-karma AutoModerator rules around April 2026 (https://reddit.sentinel-team.org/posts/1t1a3j7/snapshots/2026-05-02T03%3A06%3A21.60641Z). The thresholds were not retrieved.

### Subreddits

| Subreddit | What the research found | Fit |
|---|---|---|
| r/opensource | Moderators announced on about 2026-10-24 that projects that "use AI, are made by AI, or are intended for AI" are barred. They also require a project at least a year old, 3 or more contributors, and an account at least a year old. Asking for an exception is a permanent ban. (https://helpnetsecurity.com/2026/10/26/reddit-r-opensource-ai-ban) | Do not post |
| r/programming | An LLM-content ban trial began 2026-04-01. Current rule not read. (https://dev.to/pickuma/rprogramming-banned-ai-content-for-a-month-heres-what-the-trial-actually-showed-44k0) | Do not post |
| r/MachineLearning, r/Android, r/ios | Research or consumer audiences, not tooling. | Do not post |
| r/ClaudeAI | A mod bot comments on project posts without the project flair and warns of removal. The best audience: Claude Code users. | Post |
| r/LocalLLaMA | Highest-reach audience for phone agents (DroidRun's post showed about 777 upvotes in a search snippet, unverified). Karma gates. Expect "why not a local model" comments. | Post, with a local-model angle |
| r/rust | Extra scrutiny for commercial promotion. Open source passes. AI-assisted projects are accepted if the work is good. (rules page not read) | Post, if the Rust substance is the subject |
| r/androiddev | Self-promotion is generally limited to a monthly Showcase thread. | Showcase thread only |
| r/iOSProgramming | "App Saturday" limits promotion. | Optional, Saturday only |
| r/selfhosted | Projects under 3 months old only on "New Project Friday". Weak fit. | Skip |
| r/mcp, r/ClaudeCode | Exist. Phone MCP posts in r/mcp got 7 to 28 points. Rules not read. | Optional, low reach |
| r/SideProject | Low developer density. | Skip |

### Comparable posts

| Post | Result | What the comments asked |
|---|---|---|
| r/ClaudeAI, "Built a MCP server that lets Claude use your iPhone" (2026-03) | 180 points, 55 comments. Short body, a GitHub link, a linked demo video. | The demo was sped up 3 to 5 times (30 points). Safety of unattended use. Speed. "Does Apple allow this?" (https://reddit.sentinel-team.org/posts/1riryys/snapshots/2026-03-04T15%3A12%3A56.583662Z) |
| r/LocalLLaMA, "[PokeClaw] First working app that uses Gemma 4 to autonomously control an Android phone. Fully on-device, no cloud." | 332 points, 173 comments. Feature list and changelog. | Safety, "vibecoded" distrust, install bugs. (https://reddit.sentinel-team.org/posts/1sdv3lo/snapshots/2026-04-09T16%3A11%3A00.655487Z) |
| r/LocalLLaMA, "DroidRun: Enable AI agents to control Android" (2025-04) | About 777 upvotes (snippet). 900+ waitlist signups reported. | ADB-only? Local vision models? (search snippets only) |
| r/mcp, "Allowing Claude Code to interact with Android Devices/Emulators through mcp" | 28 points, 6 comments. A results table with an honest failure, plus a video. | Speed, token cost. (https://reddit.sentinel-team.org/posts/1rxgg2y/snapshots/2026-03-19T14%3A17%3A04.930232Z) |
| r/androiddev, "Control your Android phone with AI agents (Claude, Codex, Gemini, etc.)" | 0 points, 4 comments, a dead repo link. | "Anti-AI sentiment". (https://reddit.sentinel-team.org/posts/1r0irv7/snapshots/2026-02-10T01%3A19%3A44.439852Z) |
| r/FlutterDev, "Open-sourced today: flutter-dev-agents" | 5 points. The author asked for stars. | Single maintainers get abandoned. (https://reddit.sentinel-team.org/posts/1tijsyw/snapshots/2026-05-21T16%3A25%3A18.436398Z) |

The research found 3 posts that did well, not 12, and no confirmed removals. Patterns: a concrete first-person title naming the audience's tool ("Built a... that lets Claude..."), "no cloud" or "no ADB" qualifiers in the better ones, and a generic title ("Control your Android phone with AI agents") at 0 points. Recurring objections: safety and permissions, speed and tokens, "why not X", single maintainer, demo speed, "vibecoded". r/golang requires disclosure of AI use in linked projects (https://engineered.at/articles/r-golang-draws-a-line-on-ai-generated-projects, secondhand). Timing data per subreddit is weak; one vendor snippet puts the busiest developer-subreddit window at 14:00 to 17:00 UTC on Tuesday (https://mentionme.appspot.com/blog/best-time-to-post-on-reddit/, not opened).

## LinkedIn

### What LinkedIn says

| Fact | Source |
|---|---|
| Retrieval is one LLM-embedding system. "LLM embeddings generalize beyond observed behavior", which helps "sparse content niches". Ranking reads more than 1,000 of a member's past interactions and predicts "long dwells, likes, comments, shares". No weights are published. New posts get embeddings in near real time, so fresh content can appear "within minutes, not hours". | https://www.linkedin.com/blog/engineering/feed/engineering-the-next-generation-of-linkedins-feed |
| Dwell counts when "at least half of a feed update is visible". A skip-probability model lowers a post's score in proportion to the predicted chance of a skip. | https://www.linkedin.com/blog/engineering/feed/understanding-feed-dwell-time |
| Engagement bait is demoted: "comment to agree" prompts, "videos that don't match the text", comment automation, engagement pods. "We want the Feed to be relevant to your interests, not a popularity contest." | https://news.linkedin.com/2026/ImprovingTheFeed |
| "It's ok to ask for thoughts and feedback to encourage meaningful conversation." Asking for likes, shares or follows is discouraged. Posts that "solely attempt to promote or sell" are discouraged. | https://www.linkedin.com/blog/content-recommendations-avoid |
| Hashtags: "between 1-3 hashtags per post". "Do not tack unnecessary hashtags to the end of posts, it may actually hurt your reach potential." | https://buffer.com/resources/what-really-works-on-linkedin-answers-from-the-linkedin-team/ |
| The "golden hour" has no official definition. | https://contentin.io/glossary/golden-hour/ |
| LinkedIn builds classifiers for generic AI posts and comments and reduces their reach; it does not delete them. | https://www.entrepreneur.com/business-news/linkedin-is-fighting-back-against-ai-slop-and-ai-comments |
| Links: no official penalty statement. | https://www.tryordinal.com/blog/linkedin-link-penalty-study |

Not found in any official source: weights for comments, saves or sends, how edits or reposts are treated, any rule for video length, captions or autoplay.

### What the data says (third-party)

- Length: in 248,275 posts (Ordinal), engagement rate was highest at 2,000 to 3,000 characters (1.33%), near that under 150 characters (1.22%), and weakest from 600 to 1,500 (0.81% to 1.06%). https://www.tryordinal.com/blog/linkedin-post-size
- Format: in 1.3M business-page posts (Socialinsider), document 7.00%, multi-image 6.45%, video 6.00%, image 5.30%, text 4.50%, link 3.25%. Company pages, not personal profiles. https://www.socialinsider.io/blog/linkedin-content-strategy/
- Links: Ordinal reports 26.5% less reach for posts with links, concentrated on company pages. LinkPost (358k posts, via Ordinal): median 448 impressions with a link against 705 without. First-comment links "stopped being free somewhere around 2024" and draw fewer clicks. https://www.tryordinal.com/blog/linkedin-link-penalty-study
- Replies: Buffer says replying to comments can raise engagement by 30% (Buffer's own data, sample not shown). https://buffer.com/resources/linkedin-algorithm/
- Time: weekday mornings, Tuesday to Thursday, with a weak effect (Buffer 4.8M posts, Sprout). A 1,836-post study found "no reliable best hour". https://postfa.st/blog/best-time-to-post-on-linkedin
- Launch posts by engineers: the research found only modest examples. "We just open sourced X" posts drew 7 to 157 reactions. https://www.linkedin.com/in/edward-moscardini-50974439/

## Name availability (checked 2026-11-06)

| Name | crates.io | npm | PyPI | GitHub account | GitHub repos with this name |
|---|---|---|---|---|---|
| phone-use | free | taken | free | taken (organization `phone-use`) | 794 results, including `sauravtom/phone-use` (6 stars, MIT) |
| tapdiff | free | free | free | free | 2, both under 2 stars |
| phonewright | free | free | free | free | 1, 0 stars |
| diffphone | free | free | free | free | 0 |

Also checked and free on all four registries and accounts: deviceuse, glanceuse. Taken or crowded: fingertip, handset, thumbs, nudge, tactile, palmtop, tapline, tapwise.
