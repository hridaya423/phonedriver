# dev.to and Hashnode cross-post plan

Nothing here is posted. The four posts live on phonedriver.dev first and stay the canonical copy. dev.to and Hashnode get a republished copy with the canonical URL pointing home, so search credit goes to the site and the platform copy adds reach. Platform facts and sources are in `research-2026-11.md` (dev.to and Hashnode section).

## Rules for the republished copy

- Publish on the site first, wait until Google can see it (the page is in `sitemap.xml`; open Search Console and request indexing), then republish a day later. A republished copy that goes up first can outrank the original.
- dev.to: set `canonical_url` in the front matter to the site URL. Hashnode: in the post settings, turn on "Are you republishing?" and paste the original URL. Both are the supported way to say "the site is the original".
- Both platforms have rules about AI-generated content. The four posts were written with agent help, so the user rewrites the intro paragraph of each in their own words before publishing, and reads the post once end to end. [USER: this is the same decision as HN. Say the truth in the post if asked: agents wrote part of the code; the numbers are measured.]
- Images: upload the SVG charts as PNG (PNG is the safe choice; SVG upload on both platforms was not verified). Export `.github/assets/hero-light.svg` and `speed-light.svg` to PNG at 2x.
- Links inside the copy: change every relative link to an absolute `https://phonedriver.dev/...` URL. The one link to GitHub stays.
- Add the line at the end: "Originally published at https://phonedriver.dev/blog/<slug>/". The canonical setting is the signal; the line is for readers.
- Do not put the HN link anywhere, and do not ask for reactions.
- Post 4 now has its results (`docs/bench/jobs.json`). Cross-post it with the non-wins section intact; do not cut it to a headline.

## The four posts

Site paths are `https://phonedriver.dev/blog/<slug>/`. Word counts from `site/content/blog/` on 2026-11-07.

| # | Slug | Words | What it is | Day | dev.to title (under 128 chars) | dev.to tags (max 4) | Hashnode tags |
|---|---|---|---|---|---|---|---|
| 1 | `how-phonedriver-reads-a-screen-98x-faster-than-adb` | 2,662 | Engineering: on-device agent, H.264 frames, settle, the measurement method. The one for HN and the technical audience. | T+1 | How I made an AI agent read a phone screen 98x faster than adb | `android`, `mcp`, `rust`, `performance` | Android, Rust, Performance, AI |
| 2 | `letting-claude-order-dinner-without-letting-it-spend-your-money` | 1,427 | Design: the approval guard (D27), what it does not catch. The most shareable one. | T+2 | Letting Claude order dinner without letting it spend your money | `ai`, `mcp`, `security`, `claude` | AI, Security, MCP, Claude |
| 3 | `benchmarking-ai-agents-on-jobs-only-a-phone-can-do` | 1,244 | Method: how to benchmark fairly, checks from device state, no model judge. | T+4 | How to benchmark an AI phone agent without letting it cheat | `ai`, `testing`, `android`, `benchmarks` | Testing, AI, Android, Benchmarking |
| 4 | `phonedriver-vs-mobile-mcp-vs-phone-harness-vs-adb-real-phone-jobs` | 1,233 | Benchmark results, filled from `docs/bench/jobs.json`. | after the runs | phonedriver, mobile-mcp, phone-harness and adb on 9 real phone jobs: 18/18, 18/18, 14/18 | `ai`, `android`, `mcp`, `benchmarks` | same |

dev.to allows at most four tags per post, lower case, no spaces. Check each tag page for activity before using it; `mcp` is smaller than `ai` but its readers are the target. Hashnode tags map to its own tag list; pick from the suggestions it offers.

## Front matter for dev.to (copy, change per post)

```yaml
---
title: "How I made an AI agent read a phone screen 98x faster than adb"
published: false
description: "phonedriver reads a 1,000-row Android list in 24.0 ms against adb's 2,352.4 ms. The on-device agent, the video stream, settle detection and the method."
tags: android, mcp, rust, performance
canonical_url: https://phonedriver.dev/blog/how-phonedriver-reads-a-screen-98x-faster-than-adb/
cover_image: https://phonedriver.dev/assets/hero-social.png
---
```

Save as a draft first (`published: false`), look at the preview, then flip it. `cover_image` must be a URL: the site serves `assets/hero-social.png` (1200x675). Confirm it returns 200 after deploy.

## Per-post framing line (first paragraph, your words)

The posts open the same way on the site. On the platform copy, open with one personal sentence on why you wrote it. Facts you can use, each in the claims ledger:

1. Post 1: "I read the phone-harness source and found `uiautomator dump` costs 2.4 s per screen on the emulator, so I wrote a resident agent." Backed by `speed-2026-11-07.md`, `competitor-teardown.md`.
2. Post 2: "Giving an agent a phone means it can tap Pay." Backed by D27.
3. Post 3: "My first benchmark was won by the tool that skipped the phone." Check this against `docs/bench/m3.md` and the jobs doc before using it; if it is not literally true, do not write it. [USER: say it only if it is yours.]

## Hashnode specifics

- Custom domain blogs: if you create a Hashnode blog at a subdomain such as `blog.phonedriver.dev`, you split SEO away from the site. Do not. Publish on the default `hashnode.dev` blog with the canonical set to the site.
- Hashnode's "Share to dev.to" integrations duplicate: do not use them. Publish by hand on each.

## After posting

- Reply to every comment for the first 24 hours. Comments here are search-indexed and long-lived; a good answer under the post is a permanent asset.
- Do not drop the link in other channels as "new post on dev.to". The site URL is the one to share.
- Record per post: views, reactions, GitHub referrers (Insights, Traffic, Referring sites).
