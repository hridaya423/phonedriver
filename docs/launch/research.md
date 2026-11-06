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
