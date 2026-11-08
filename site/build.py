#!/usr/bin/env python3
"""Builds the phonedriver site into site/public. Plain HTML and CSS, plus one inline script on the home page for the copy button.

    python3 site/build.py            build
    PHONEDRIVER_SITE_URL=https://example.test python3 site/build.py

The domain lives in SITE_URL only.
"""
import datetime
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import mdlite  # noqa: E402
from siteparts import css, data as D  # noqa: E402

SITE_URL = os.environ.get("PHONEDRIVER_SITE_URL", "https://phonedriver.dev").rstrip("/")
REPO = "https://github.com/hridaya423/phonedriver"
RELEASED = False
TODAY = "2026-11-07"
OUT = HERE / "public"
CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

esc = mdlite.esc
PAGES = []


def url(path):
    return SITE_URL + path


def raw(path):
    return REPO + "/blob/main/" + path


def write(rel, text):
    p = OUT / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")


def jld(obj):
    return '<script type="application/ld+json">' + json.dumps(obj, ensure_ascii=False, separators=(",", ":")).replace("</", "<\\/") + "</script>"


MARK = (
    '<svg class="mark" viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">'
    '<rect x="6.5" y="2.5" width="11" height="19" fill="none" stroke="currentColor" stroke-width="1.5"/>'
    '<path d="M9 7.5h6M9 17h6" stroke="currentColor" stroke-width="1.5"/>'
    '<rect x="8.5" y="10" width="7" height="4" fill="#ffd400"/></svg>'
)

NAV = [("/docs/", "Docs"), ("/benchmarks/", "Benchmarks"), ("/compare/", "Compare"), ("/blog/", "Blog")]


def layout(path, title, desc, body, kind="WebPage", crumbs=None, extra_ld=None, og=None, date=None, main_class="", noindex=False, headline=None):
    canon = url(path)
    og = og or "/assets/og/default.png"
    nav = ""
    for href, label in NAV:
        cur = ' aria-current="page"' if path.startswith(href) else ""
        nav += '<a href="%s"%s>%s</a>' % (href, cur, label)
    nav += '<a href="%s">GitHub</a>' % REPO
    ld = []
    if crumbs:
        items = [{"@type": "ListItem", "position": i + 1, "name": n, "item": url(p)} for i, (n, p) in enumerate(crumbs)]
        ld.append({"@context": "https://schema.org", "@type": "BreadcrumbList", "itemListElement": items})
    ld += extra_ld or []
    crumb_html = ""
    if crumbs and len(crumbs) > 1:
        parts = []
        for n, p in crumbs[:-1]:
            parts.append('<a href="%s">%s</a>' % (p, esc(n)))
        parts.append("<span>%s</span>" % esc(crumbs[-1][0]))
        crumb_html = '<nav class="crumbs" aria-label="Breadcrumb">%s</nav>' % " / ".join(parts)
    robots = '<meta name="robots" content="noindex">' if noindex else ""
    page = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{esc(title)}</title>
<meta name="description" content="{esc(desc)}">
<link rel="canonical" href="{canon}">
{robots}
<meta name="color-scheme" content="light dark">
<meta name="theme-color" content="#ffffff" media="(prefers-color-scheme: light)">
<meta name="theme-color" content="#141412" media="(prefers-color-scheme: dark)">
<meta property="og:type" content="{'article' if kind in ('BlogPosting', 'TechArticle') else 'website'}">
<meta property="og:site_name" content="phonedriver">
<meta property="og:title" content="{esc(title)}">
<meta property="og:description" content="{esc(desc)}">
<meta property="og:url" content="{canon}">
<meta property="og:image" content="{url(og)}">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="{'675' if og.endswith('hero-social.png') else '630'}">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="{esc(title)}">
<meta name="twitter:description" content="{esc(desc)}">
<meta name="twitter:image" content="{url(og)}">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="alternate" type="application/rss+xml" title="phonedriver blog" href="/blog/feed.xml">
<link rel="preload" href="/assets/fonts/inter-display.woff2" as="font" type="font/woff2" crossorigin>
<link rel="preload" href="/assets/fonts/commit-mono.woff2" as="font" type="font/woff2" crossorigin>
<style>{css.CSS}</style>
{''.join(jld(x) for x in ld)}
</head>
<body>
<a class="skip" href="#main">Skip to content</a>
<header class="top"><div class="wrap bar">
<a class="brand" href="/" aria-label="phonedriver home">{MARK}<span>phonedriver</span></a>
<nav class="nav" aria-label="Main">{nav}</nav>
</div></header>
<main id="main" class="{main_class}">
{crumb_html}
{body}
</main>
<footer class="foot"><div class="wrap">
<div class="fgrid">
<div><p class="fbrand">{MARK}<span>phonedriver</span></p><p class="small">Give your AI agent a phone. Free and open source under AGPL-3.0.</p></div>
<nav aria-label="Learn"><p class="lab">Learn</p><a href="/docs/install/">Install</a><a href="/docs/">Docs</a><a href="/benchmarks/">Benchmarks</a><a href="/compare/">Compare</a><a href="/blog/">Blog</a></nav>
<nav aria-label="Project"><p class="lab">Project</p><a href="{REPO}">GitHub</a><a href="{REPO}/blob/main/LICENSE">License: AGPL-3.0</a><a href="{REPO}/blob/main/docs/decisions.md">Design decisions</a><a href="/llms.txt">llms.txt</a><a href="/blog/feed.xml">RSS</a></nav>
</div>
<p class="small legal">phonedriver sends no telemetry. This site sets no cookies and loads no third-party scripts. Brand: Sodium design system, Inter Display and Commit Mono under the SIL Open Font License 1.1.</p>
</div></footer>
</body>
</html>
"""
    write(path.strip("/") + "/index.html" if path != "/" else "index.html", page)
    PAGES.append({"path": path, "title": title, "desc": desc, "kind": kind, "date": date or TODAY})


def ttl(t):
    return t if len(t) > 52 else t + " | phonedriver"


def crumbs_for(*items):
    return [("phonedriver", "/")] + list(items)


def read_md(path):
    meta, text = mdlite.front(Path(path).read_text(encoding="utf-8"))
    return meta, text


def sub(text):
    return (
        text.replace("{{SITE}}", SITE_URL)
        .replace("{{REPO}}", REPO)
    )


FULL = []


def md_page(section, slug, crumbs_extra, kind, extra=None, og=None, nav_html="", main_class="doc"):
    meta, text = read_md(HERE / "content" / section / (slug + ".md"))
    text = sub(text)
    toc = []
    html_body = mdlite.render(text, toc)
    path = "/%s/%s/" % (section, slug)
    return meta, text, html_body, toc, path



DOCS = [
    "install", "claude-code", "cursor-and-mcp-clients", "android-emulator", "android-phone",
    "ios-simulator", "iphone", "approval-guard", "shell", "troubleshooting",
]


def build_docs():
    metas = {}
    for s in DOCS:
        metas[s] = read_md(HERE / "content" / "docs" / (s + ".md"))[0]
    side = "".join(
        '<a href="/docs/%s/"%s>%s</a>' % (s, " aria-current=\"page\"" if False else "", esc(metas[s]["nav"]))
        for s in DOCS
    )
    for s in DOCS:
        meta, text, body, toc, path = md_page("docs", s, None, "TechArticle")
        full_md = "# %s\n\n%s\n" % (meta["title"], text.strip())
        FULL.append((path, meta["title"], full_md))
        nav = "".join(
            '<a href="/docs/%s/"%s>%s</a>' % (x, ' aria-current="page"' if x == s else "", esc(metas[x]["nav"]))
            for x in DOCS
        )
        idx = DOCS.index(s)
        pn = ""
        if idx > 0:
            pn += '<a class="pn" href="/docs/%s/"><span class="lab">Previous</span>%s</a>' % (DOCS[idx - 1], esc(metas[DOCS[idx - 1]]["nav"]))
        if idx < len(DOCS) - 1:
            pn += '<a class="pn next" href="/docs/%s/"><span class="lab">Next</span>%s</a>' % (DOCS[idx + 1], esc(metas[DOCS[idx + 1]]["nav"]))
        html_page = f"""<div class="wrap docs">
<aside class="side"><p class="lab">Docs</p><nav aria-label="Docs">{nav}</nav></aside>
<article class="prose">
<p class="lab">@e{idx + 1} DOCS</p>
<h1>{esc(meta['title'])}</h1>
<p class="lede">{esc(meta['description'])}</p>
{body}
<div class="pnav">{pn}</div>
</article></div>"""
        ld = [{
            "@context": "https://schema.org", "@type": "TechArticle", "headline": meta["title"],
            "description": meta["description"], "url": url(path), "inLanguage": "en",
            "dateModified": TODAY, "author": AUTHOR, "publisher": PUBLISHER,
            "isPartOf": {"@type": "WebSite", "name": "phonedriver", "url": url("/")},
            "about": {"@id": url("/#software")},
        }]
        layout(path, meta.get("seo") or ttl(meta["title"]), meta.get("meta", meta["description"]), html_page, "TechArticle",
               crumbs_for(("Docs", "/docs/"), (meta["nav"], path)), ld, og="/assets/og/docs.png")
    cards = "".join(
        '<a class="card" href="/docs/%s/"><span class="lab">@e%d</span><h2>%s</h2><p>%s</p></a>' % (s, i + 1, esc(metas[s]["title"]), esc(metas[s]["description"]))
        for i, s in enumerate(DOCS)
    )
    body = f"""<div class="wrap"><div class="pagehead"><p class="lab">@e0 DOCS</p><h1>phonedriver docs</h1>
<p class="lede">Each page is one task: install it, point your agent at it, drive an Android emulator, a real Android phone, the iOS Simulator or an iPhone, and keep the agent from spending your money.</p></div>
<div class="cards">{cards}</div></div>"""
    layout("/docs/", "phonedriver docs: set up an AI agent to control a phone", "How-to guides for phonedriver: install, Claude Code, Cursor, Android and iOS devices, the approval guard, the shell and troubleshooting.",
           body, "CollectionPage", crumbs_for(("Docs", "/docs/")), og="/assets/og/docs.png")


AUTHOR = {"@type": "Person", "name": "Hridya Agrawal", "url": "https://github.com/hridaya423"}
PUBLISHER = {"@type": "Organization", "name": "phonedriver", "url": SITE_URL + "/"}


COMPARE = [("adb", "phonedriver vs adb"), ("mobile-mcp", "phonedriver vs mobile-mcp"), ("phone-harness", "phonedriver vs phone-harness")]


def build_compare():
    cards = ""
    for s, label in COMPARE:
        meta, text, body, toc, path = md_page("compare", s, None, "Article")
        FULL.append((path, meta["title"], "# %s\n\n%s\n" % (meta["title"], text.strip())))
        others = "".join('<a href="/compare/%s/">%s</a>' % (x, l) for x, l in COMPARE if x != s)
        html_page = f"""<div class="wrap"><article class="prose col">
<p class="lab">@e1 COMPARE</p>
<h1>{esc(meta['title'])}</h1>
<p class="lede">{esc(meta['description'])}</p>
{body}
<p class="small">Compared as of {TODAY}. Other comparisons: {others}. Something wrong or out of date? <a href="{REPO}/issues">Open an issue</a>.</p>
</article></div>"""
        ld = [{
            "@context": "https://schema.org", "@type": "Article", "headline": meta["title"],
            "description": meta["description"], "url": url(path), "inLanguage": "en",
            "datePublished": TODAY, "dateModified": TODAY, "author": AUTHOR, "publisher": PUBLISHER,
            "about": [{"@id": url("/#software")}],
            "image": url("/assets/og/compare-%s.png" % s),
        }]
        layout(path, meta.get("seo") or ttl(meta["title"]), meta.get("meta", meta["description"]), html_page, "Article",
               crumbs_for(("Compare", "/compare/"), (label, path)), ld, og="/assets/og/compare-%s.png" % s)
        cards += '<a class="card" href="%s"><span class="lab">VS</span><h2>%s</h2><p>%s</p></a>' % (path, esc(label), esc(meta["description"]))
    body = f"""<div class="wrap"><div class="pagehead"><p class="lab">@e0 COMPARE</p><h1>phonedriver compared with adb, mobile-mcp and phone-harness</h1>
<p class="lede">Three pages, each tied to a benchmark or to the other tool's own documentation. Where we have no measurement, the page says so and does not guess.</p></div>
<div class="cards">{cards}</div></div>"""
    layout("/compare/", "phonedriver vs adb, mobile-mcp and phone-harness", "Sourced comparisons of phonedriver with adb, mobile-mcp and phone-harness for AI agents on a phone: speed, tokens, platforms, telemetry, license.",
           body, "CollectionPage", crumbs_for(("Compare", "/compare/")), og="/assets/og/compare-adb.png")



POSTS = [
    "how-phonedriver-reads-a-screen-98x-faster-than-adb",
    "phonedriver-vs-mobile-mcp-vs-phone-harness-vs-adb-real-phone-jobs",
    "letting-claude-order-dinner-without-letting-it-spend-your-money",
    "benchmarking-ai-agents-on-jobs-only-a-phone-can-do",
]
WORDS = {}


def words(md):
    import re
    t = re.sub(r"```.*?```", " ", md, flags=re.S)
    t = re.sub(r"[`#|>*_\[\]()-]", " ", t)
    return len(t.split())


def build_blog():
    items = ""
    feed = ""
    metas = []
    for s in POSTS:
        meta, text, body, toc, path = md_page("blog", s, None, "BlogPosting")
        n = words(text)
        WORDS[s] = n
        metas.append((s, meta, path))
        FULL.append((path, meta["title"], "# %s\n\n%s\n" % (meta["title"], text.strip())))
        mins = max(1, round(n / 220))
        tocs = "".join('<li><a href="#%s">%s</a></li>' % (i, esc(t)) for i, t in toc)
        html_page = f"""<div class="wrap"><article class="prose col post">
<p class="lab">{esc(meta.get('label', 'BLOG'))} · {meta['date']} · {mins} MIN READ</p>
<h1>{esc(meta['title'])}</h1>
<p class="lede">{esc(meta['description'])}</p>
<p class="byline small">By <a href="https://github.com/hridaya423">Hridya Agrawal</a>. Published {meta['date']}.</p>
<nav class="toc" aria-label="In this post"><p class="lab">In this post</p><ol>{tocs}</ol></nav>
{body}
<p class="small">Questions or corrections: <a href="{REPO}/issues">open an issue</a>. Every figure links to the page or file it came from.</p>
</article></div>"""
        ld = [{
            "@context": "https://schema.org", "@type": "BlogPosting", "headline": meta["title"],
            "description": meta["description"], "url": url(path), "mainEntityOfPage": url(path), "inLanguage": "en",
            "datePublished": meta["date"], "dateModified": meta["date"], "author": AUTHOR, "publisher": PUBLISHER,
            "image": url("/assets/og/post-%d.png" % (POSTS.index(s) + 1)), "wordCount": n,
            "about": [{"@id": url("/#software")}],
        }]
        layout(path, meta.get("seo") or ttl(meta["title"]), meta.get("meta", meta["description"]), html_page, "BlogPosting",
               crumbs_for(("Blog", "/blog/"), (meta["short"], path)), ld, og="/assets/og/post-%d.png" % (POSTS.index(s) + 1), date=meta["date"])
        items += '<a class="card post-card" href="%s"><span class="lab">%s · %s</span><h2>%s</h2><p>%s</p></a>' % (path, esc(meta.get("label", "BLOG")), meta["date"], esc(meta["title"]), esc(meta["description"]))
        feed += "<item><title>%s</title><link>%s</link><guid>%s</guid><pubDate>%s</pubDate><description>%s</description></item>" % (
            esc(meta["title"]), url(path), url(path),
            datetime.datetime.strptime(meta["date"], "%Y-%m-%d").strftime("%a, %d %b %Y 00:00:00 GMT"), esc(meta["description"]))
    body = f"""<div class="wrap"><div class="pagehead"><p class="lab">@e0 BLOG</p><h1>phonedriver blog</h1>
<p class="lede">How it works, what we measured and what the numbers do not show.</p></div>
<div class="cards one">{items}</div></div>"""
    layout("/blog/", "phonedriver blog: AI phone agents, benchmarks and design", "Technical posts on how phonedriver lets an AI agent read and operate a phone, the benchmarks behind its speed claims, and the approval guard.", body,
           "Blog", crumbs_for(("Blog", "/blog/")), [{"@context": "https://schema.org", "@type": "Blog", "name": "phonedriver blog", "url": url("/blog/")}])
    write("blog/feed.xml", '<?xml version="1.0" encoding="UTF-8"?><rss version="2.0"><channel><title>phonedriver blog</title><link>%s</link><description>How phonedriver works and what we measured.</description>%s</channel></rss>' % (url("/blog/"), feed))
    return metas




def fmt(v, d=1):
    if v is None:
        return "n/a"
    if v >= 1000:
        return "{:,.1f}".format(v)
    return ("%." + str(d) + "f") % v


def ratio(v):
    return ("%.1f" % v if v < 10 else "%.0f" % v) + "x"


def speed_block():
    d = json.loads((ROOT / "docs/bench/speed.json").read_text())
    blocks = ""
    rows = ""
    for o in d["ops"]:
        mx = max(x for x in (o["phonedriver_p95"], o["adb_p95"], o.get("phone_harness_p95")) if x)
        def bar(name, p50, p95, cls):
            if p50 is None:
                return '<div class="brow"><span class="bn">%s</span><span class="bt na">not supported</span></div>' % name
            w = max(p50 / mx * 100, 0.6)
            t = p95 / mx * 100
            return ('<div class="brow"><span class="bn">%s</span><span class="bt" aria-hidden="true"><i class="fill %s" style="width:%.2f%%"></i><i class="tick" style="left:%.2f%%"></i></span>'
                    '<span class="bv">%s ms<small> p95 %s</small></span></div>') % (name, cls, w, t, fmt(p50), fmt(p95))
        blocks += '<div class="bblock"><h3>%s</h3>%s%s%s<p class="small">phonedriver is %s faster than adb (95%% CI %s to %s)%s.</p></div>' % (
            esc(o["label"]), bar("phonedriver", o["phonedriver"], o["phonedriver_p95"], "pd"),
            bar("adb", o["adb"], o["adb_p95"], "adb"), bar("phone-harness", o["phone_harness"], o.get("phone_harness_p95"), "ph"),
            ratio(o["adb"] / o["phonedriver"]), fmt(o["adb_ratio_ci"][0]), fmt(o["adb_ratio_ci"][1]),
            (", and %s faster than phone-harness (95%% CI %s to %s)" % (ratio(o["phone_harness"] / o["phonedriver"]), fmt(o["phone_harness_ratio_ci"][0]), fmt(o["phone_harness_ratio_ci"][1]))) if o["phone_harness"] else "; phone-harness has no equivalent")
        rows += "<tr><th scope=\"row\">%s</th><td>%s</td><td>%s</td><td>%s</td><td>%s</td><td>%s</td><td>%s</td><td>%s</td></tr>" % (
            esc(o["label"]), fmt(o["phonedriver"]), fmt(o["phonedriver_p95"]), fmt(o["adb"]), fmt(o["adb_p95"]),
            fmt(o["phone_harness"]), fmt(o.get("phone_harness_p95")), ratio(o["adb"] / o["phonedriver"]) + " (%s to %s)" % (fmt(o["adb_ratio_ci"][0]), fmt(o["adb_ratio_ci"][1])))
    table = ('<div class="tw"><table><thead><tr><th scope="col">Operation</th><th scope="col">phonedriver p50</th><th scope="col">p95</th><th scope="col">adb p50</th><th scope="col">p95</th>'
             '<th scope="col">phone-harness p50</th><th scope="col">p95</th><th scope="col">adb / phonedriver (95%% CI)</th></tr></thead><tbody>%s</tbody></table></div>') % rows
    return blocks, table


def tokens_block():
    return """<div class="tw"><table><thead><tr><th scope="col">Task (checked from device state)</th><th scope="col">phonedriver, 2 to 3 runs, median tokens</th><th scope="col">phone-harness, 3 runs, median tokens</th></tr></thead><tbody>
<tr><th scope="row">alarm-gym: 7:30 AM weekday alarm "Gym" in Clock</th><td>110,419 (19.5 calls)</td><td>220,723 (19 calls)</td></tr>
<tr><th scope="row">wiki-birthdate: Chrome, Wikipedia, Grace Hopper's birth date</th><td>57,753 (7 calls)</td><td>53,403 (7 calls)</td></tr>
<tr><th scope="row">testbed-web-form: fill and submit a web form</th><td>50,185 (7 calls)</td><td>56,249 (6 calls)</td></tr>
<tr><th scope="row">Sum of the three medians</th><td>218,357</td><td>330,375</td></tr>
</tbody></table></div>"""


def build_benchmarks():
    blocks, table = speed_block()
    (OUT / "benchmarks" / "data").mkdir(parents=True, exist_ok=True)
    for name in ("speed.json", "tokens.json"):
        shutil.copy(ROOT / "docs/bench" / name, OUT / "benchmarks" / "data" / name)
    shutil.copy(ROOT / "docs/bench/data/speed-2026-11-07/samples.csv", OUT / "benchmarks/data/speed-samples.csv")
    shutil.copy(ROOT / "docs/bench/data/speed-2026-11-07/second-pass/samples.csv", OUT / "benchmarks/data/speed-samples-second-pass.csv")
    shutil.copy(ROOT / "docs/bench/data/speed-2026-11-07/meta.json", OUT / "benchmarks/data/speed-meta.json")
    jobs_data = json.loads((ROOT / "docs/bench/jobs.json").read_text())
    jmed = {(m["harness"], m["task"]): m for m in jobs_data["medians"]}
    jtot = jobs_data["totals"]
    jobs_rows = ""
    for j in D.JOBS:
        task = j[0].split(" ")[0]
        cells = []
        for h in ("phonedriver", "mobile-mcp", "phone-harness", "adb"):
            m = jmed.get((h, task))
            if m:
                cells.append("%s %d/%d, %s tokens, %s s" % (h, m["passed"], m["runs"], f"{m['median_tokens']:,.0f}", f"{m['median_wall_s']:g}"))
        jobs_rows += '<tr><th scope="row">%s</th><td>%s</td><td>%s</td><td>%s</td></tr>' % (esc(j[0]), esc(j[1]), esc(j[2]), "<br>".join(esc(c) for c in cells))
    jobs_tot = "; ".join("%s %d/%d runs, %s tokens" % (h, jtot[h]["passed"], jtot[h]["runs"], f"{jtot[h]['sum_median_tokens']:,.0f}") for h in ("phonedriver", "mobile-mcp", "phone-harness", "adb"))
    body = f"""<div class="wrap"><article class="prose col wide">
<p class="lab">@e0 BENCHMARKS</p>
<h1>phonedriver benchmarks: speed, tokens and real phone jobs</h1>
<p class="lede">phonedriver reads a 1,000-row list in 24.0 ms where adb takes 2,352.4 ms: 98 times faster at p50 (95% CI on the ratio 92.2 to 106.0). The rest of this page is every operation we timed, what the agent spent in tokens, and nine real phone jobs.</p>
<ul class="toc2"><li><a href="#speed">Speed: every operation</a></li><li><a href="#tokens">Tokens: agent runs</a></li><li><a href="#jobs">Real phone jobs</a></li><li><a href="#data">Raw data</a></li></ul>

<h2 id="speed">Speed against adb and phone-harness</h2>
<p>p50 of 50 runs after 20 warm-ups, on an Android 17 emulator (API 37, headless, software rendering). Everything ran on the emulator's own host with the three tools interleaved. Mean host CPU load was 13% to 39% per operation from other work on the machine. Bars are drawn to scale within each operation; the tick is p95.</p>
<div class="bars">{blocks}</div>
<h3>All figures, milliseconds</h3>
{table}
<h3>How to read it</h3>
<ul>
<li>The bare tap is the closest row. Host to reply, phonedriver takes 117.6 ms when taps come back to back (2.1 times faster than adb) and 72.4 ms when they are 1.5 s apart (1.6 times). Measured on the device, from the host sending a tap to the testbed's touch listener logging it, it is 27.5 ms against adb's 71.2 ms and phone-harness's 76.6 ms (clock offset good to about 8 ms).</li>
<li>Our tap misses its own 15 ms budget (decision D14), and typing 100 characters misses its 150 ms budget (251.1 ms).</li>
<li>adb returns raw XML and a tap that returns before the app reacts. phonedriver returns a pruned, numbered tree and waits for the app. Different outputs for the same job. For "tap, then what changed", the adb row is a tap, one <code>uiautomator dump</code> and a diff in the caller.</li>
<li><code>adb input text</code> and phone-harness cannot type Unicode. phonedriver typed 100 mixed-script characters (ASCII, accents, CJK, Arabic, emoji) in 225.0 ms.</li>
<li>Fresh daemon to first screen read: phonedriver 1,620.5 ms, phone-harness 2,999.4 ms (new Python process), adb 2,488.0 ms (one dump; adb has no cold start, so this is for scale only).</li>
<li>phone-harness's helpers ran in process, which spares it a Python start per call and favours it.</li>
<li>Emulator only. No physical Android phone and no iOS device is in this run. On the iOS 26.5 Simulator a tap takes 357 ms against 80 ms for the Simulator's own tool, because XCTest takes about 240 ms to confirm the event (D15). On an iPhone 14 Plus and an iPad Pro (M5) it takes 534 and 454 ms.</li>
</ul>
<p>The earlier 153x figure came from a quieter, single-tool run and is not current. <a href="{raw('docs/bench/speed.md')}">The full method, machine state and the history of the number</a> are in the repo.</p>

<h2 id="tokens">Tokens: what an agent spends on a task</h2>
<p>Claude Sonnet 5.5 through <code>claude -p</code>, each harness the only tool, three tasks checked from device state (the alarm database, the testbed event log, the answer text), never from the agent's claim. Tokens are input, output, cache creation and cache read. Device: a 2560x1600 tablet emulator.</p>
{tokens_block()}
<p>On the sum of medians phonedriver used 34% fewer tokens than phone-harness (218,357 against 330,375). It is ahead on the longest task, level on the form and 8% behind on the Wikipedia task, where individual runs ranged from 26,147 to 64,522 tokens. These are 2 to 3 runs per cell: they show what happened, not a rate. Raw adb finished <code>alarm-gym</code> in 2 calls with a <code>SET_ALARM</code> intent (18,607 tokens), a shortcut the task allows, and failed the web form, a WebView with no accessibility nodes.</p>
<p><strong>mobile-mcp is not compared here.</strong> It failed all runs in this setup ("device server did not start within 5s", plus an <code>adb forward</code> bind error against a remote adb server). That is a fault of the setup, not a measured capability, and its token totals are the cost of retrying a broken tool. It needs a run on a local emulator or phone.</p>
<p>The fixed cost per turn is also measured: the first turn of a one-word <code>claude -p</code> costs 5,606 tokens with phonedriver's 14 MCP tools and 5,214 with phone-harness. <a href="{raw('docs/bench/tokens.md')}">All three rounds, the fixes between them and the caveats</a>.</p>

<h2 id="jobs">Real phone jobs</h2>
<p>Nine jobs only a phone can do: the result cannot be produced by adb, <code>am</code>, <code>cmd</code>, <code>settings</code>, <code>content</code> or a web search. Each is checked from device state by a script, not by the agent's say-so. Each harness ran each job twice on a tablet emulator (Claude Sonnet 5.5). Totals are sums of per-job medians: {jobs_tot}. adb ran only the two mixed jobs.</p>
<p>Where phonedriver does not win: phone-harness is cheaper on photos-crop, clock-settings and log-code; mobile-mcp is 0.5 s faster on wakelock-restrict; and phonedriver's 23% token lead over phone-harness comes mostly from phone-harness's failed runs (14 of 18 passed). <a href="{raw('docs/bench/jobs.md')}">What each harness failed at</a>.</p>
<div class="tw"><table><thead><tr><th scope="col">Job</th><th scope="col">What the agent is asked</th><th scope="col">Why adb or the web cannot do it</th><th scope="col">Result: passes, median tokens, median wall time</th></tr></thead><tbody>{jobs_rows}</tbody></table></div>
<p>Method: <a href="/blog/benchmarking-ai-agents-on-jobs-only-a-phone-can-do/">Benchmarking AI agents on jobs only a phone can do</a>. Comparison write-up: <a href="/blog/phonedriver-vs-mobile-mcp-vs-phone-harness-vs-adb-real-phone-jobs/">phonedriver vs mobile-mcp vs phone-harness vs adb on real phone jobs</a>.</p>

<h2 id="data">Raw data and reproduction</h2>
<ul>
<li><a href="/benchmarks/data/speed-samples.csv">speed-samples.csv</a> and <a href="/benchmarks/data/speed-samples-second-pass.csv">second-pass samples</a>: every timed sample.</li>
<li><a href="/benchmarks/data/speed.json">speed.json</a>: p50, p95 and bootstrap intervals per operation. <a href="/benchmarks/data/speed-meta.json">speed-meta.json</a>: machine and versions.</li>
<li><a href="/benchmarks/data/tokens.json">tokens.json</a>: every agent run.</li>
<li>Repeat the speed run: <code>PH_PYTHON=&lt;python with phone-harness 0.3.3&gt; bash bench/speed/reproduce.sh</code> in the <a href="{REPO}">repository</a>.</li>
</ul>
</article></div>"""
    ld = [{
        "@context": "https://schema.org", "@type": "Dataset", "name": "phonedriver speed benchmark, 2026-11-07",
        "description": "p50 and p95 latency of phonedriver, adb and phone-harness 0.3.3 on an Android 17 emulator: tap, screenshot, UI tree, typing, tap and diff, cold start. 50 samples per operation after 20 warm-ups, tools interleaved.",
        "url": url("/benchmarks/"), "license": "https://www.gnu.org/licenses/agpl-3.0.html", "creator": AUTHOR, "dateModified": TODAY,
        "distribution": [
            {"@type": "DataDownload", "encodingFormat": "text/csv", "contentUrl": url("/benchmarks/data/speed-samples.csv")},
            {"@type": "DataDownload", "encodingFormat": "application/json", "contentUrl": url("/benchmarks/data/speed.json")},
        ],
    }, {
        "@context": "https://schema.org", "@type": "TechArticle", "headline": "phonedriver benchmarks: speed, tokens and real phone jobs",
        "url": url("/benchmarks/"), "dateModified": TODAY, "author": AUTHOR, "publisher": PUBLISHER, "inLanguage": "en",
        "about": {"@id": url("/#software")},
    }]
    layout("/benchmarks/", "phonedriver benchmarks: up to 98x faster than adb, with raw data", "phonedriver against adb and phone-harness on an Android emulator: p50, p95 and confidence intervals, agent token counts and real phone jobs. Raw data.",
           body, "TechArticle", crumbs_for(("Benchmarks", "/benchmarks/")), ld, og="/assets/hero-social.png")
    FULL.append(("/benchmarks/", "phonedriver benchmarks", benchmarks_md()))


def benchmarks_md():
    d = json.loads((ROOT / "docs/bench/speed.json").read_text())
    s = "# phonedriver benchmarks\n\nSpeed, p50 and p95 in ms, Android 17 emulator, 50 runs after 20 warm-ups, tools interleaved on one host.\n\n| Operation | phonedriver p50 | p95 | adb p50 | p95 | phone-harness p50 | p95 | adb / phonedriver (95% CI) |\n|---|---|---|---|---|---|---|---|\n"
    for o in d["ops"]:
        s += "| %s | %s | %s | %s | %s | %s | %s | %s (%s to %s) |\n" % (o["label"], fmt(o["phonedriver"]), fmt(o["phonedriver_p95"]), fmt(o["adb"]), fmt(o["adb_p95"]), fmt(o["phone_harness"]), fmt(o.get("phone_harness_p95")), ratio(o["adb"] / o["phonedriver"]), fmt(o["adb_ratio_ci"][0]), fmt(o["adb_ratio_ci"][1]))
    s += "\nTokens (Claude Sonnet 5.5, medians of 2 to 3 runs): sum over three tasks 218,357 for phonedriver and 330,375 for phone-harness (34% fewer). mobile-mcp failed every run for an environment reason and is not compared.\n\nReal phone jobs (nine jobs checked from device state, two runs each, sums of per-job medians): phonedriver 18/18 passed, 977,780 tokens, 466.5 s; mobile-mcp 18/18, 4,892,241 tokens, 910 s; phone-harness 14/18, 1,272,617 tokens, 1,946 s; adb 4/4 on the 2 mixed jobs. phone-harness is cheaper on photos-crop, clock-settings and log-code, mobile-mcp is 0.5 s faster on wakelock-restrict, and the 23% token lead over phone-harness comes mostly from its failed runs.\n"
    return s




def faq_html():
    out = ""
    for q, a in D.FAQ:
        out += '<div class="qa"><h3>%s</h3><p>%s</p></div>' % (esc(q), mdlite.inline(a))
    return out


INSTALL_CMD = "claude mcp add phonedriver -- npx -y phonedriver mcp"

COPY_JS = """(function(){var c=document.querySelector("[data-copy]");if(!c||!navigator.clipboard)return;var b=document.createElement("button");b.type="button";b.className="btn";b.setAttribute("aria-live","polite");b.textContent="Copy";b.addEventListener("click",function(){navigator.clipboard.writeText(c.textContent.trim()).then(function(){b.textContent="Copied";setTimeout(function(){b.textContent="Copy"},2000)})});c.parentNode.appendChild(b)})()"""


def term(lines, label):
    out = ""
    for ln in lines:
        cls = ""
        if ln.startswith("$ "):
            cls = " cmd"
        elif ln.startswith("~ "):
            cls = " chg"
        n = len(ln) - len(ln.lstrip(" "))
        out += '<span class="tl%s" style="--i:%d">%s</span>' % (cls, n + 4, esc(ln.lstrip(" ")))
    return '<figure class="term"><pre tabindex="0" aria-label="%s"><code>%s</code></pre></figure>' % (esc(label), out)


SNAPSHOT = [
    "$ phonedriver snapshot",
    "app com.android.settings (Settings) · keyboard hidden · settled in 0ms",
    '@e1 button "Search Settings"',
    "@e2 group scrollable↓",
    '  @e3 button "Google" "Services & preferences"',
    '  @e4 button "Network & internet" "Mobile, Wi‑Fi, hotspot"',
    '  @e5 button "Connected devices" "Bluetooth, pairing"',
    "  ...",
    '  @e10 button "Display & touch" "Dark theme, font size, touch"',
]
TAPDIFF = [
    "$ phonedriver tap @e24",
    'ok tapped @e24 "Dark theme" at 353,532 · settled in 2034ms',
    '~ @e23 button "Dark theme" "Will turn on when Bedtime starts" → "Will never turn off automatically"',
    '~ @e24 switch "Dark theme" off → on',
]


def build_home():
    cap = esc(D.DEMO_CAPTION)
    note = "" if RELEASED else '<p class="small">Not released yet. Until the first release, <a href="/docs/install/#build-from-source">build from source</a>.</p>'
    body = f"""<div class="wrap hero">
<h1>Give your AI agent <span class="hl">a phone</span>.</h1>
<div class="hcopy">
<p class="lede">phonedriver lets Claude Code, Codex, Cursor or any MCP client use an Android phone or an iPhone. The model opens apps, taps, types, scrolls and reads the screen. It reads the screen as text instead of screenshots: on an emulator, a 1,000-row screen comes back in 24 ms. After each tap it gets back only the lines that changed. And when it reaches a button labeled Pay or Place order, phonedriver stops and asks you.</p>
<div class="install"><code data-copy>{INSTALL_CMD}</code></div>
<p class="small">Start an Android emulator, plug in a phone with USB debugging on, or boot the iOS Simulator. Then ask Claude: <q>Open Settings and turn on Dark theme.</q> Other clients, Homebrew and real iPhones: <a href="/docs/install/">install guide</a>.</p>
{note}
</div>
<figure class="demo"><video controls preload="none" width="1280" height="720" poster="/assets/demo-poster.jpg"><source src="/assets/demo.mp4" type="video/mp4"></video><figcaption class="small">{cap}</figcaption></figure>
</div>

<section class="sec" id="what-the-model-sees"><div class="wrap">
<h2>What the model sees</h2>
<div class="pair">
<div class="pc">
<p class="a">Each screen arrives as a short numbered list. This is the Settings app on an Android emulator:</p>
<div class="b">{term(SNAPSHOT, "phonedriver snapshot output")}</div>
</div>
<div class="pc">
<p class="c">The model taps by number. phonedriver waits until the screen stops changing, then replies. After <code>tap @e10</code> the Display screen opens and the reply lists it, including <code>@e24 switch "Dark theme" off</code>. The next tap returns only what changed:</p>
<div class="d">{term(TAPDIFF, "phonedriver tap output")}</div>
<p class="e"><code>+</code> marks a line that appeared, <code>-</code> one that left, <code>~</code> one that changed. The model doesn't need to poll or guess how long a screen takes to load. It takes a screenshot only when it needs to see a picture.</p>
</div>
</div></div></section>

<section class="sec" id="speed"><div class="wrap">
<h2>Speed</h2>
<figure class="chart"><a href="/benchmarks/"><picture><source media="(prefers-color-scheme: dark)" srcset="/assets/hero-dark.svg"><img src="/assets/hero-light.svg" width="1280" height="{D.HERO_H}" alt="phonedriver against adb on an Android emulator, median of 50 runs. Reading the screen: 24 ms against 2,352 ms, 98 times faster. Screenshot: 12 ms against 476 ms, 41 times faster. Typing 100 characters: 251 ms against 4,841 ms, 19 times faster. Tapping and seeing what changed: 587 ms against 2,726 ms, 4.6 times faster." decoding="async"></picture></a></figure>
<p class="narrow">Measured on an Android 17 emulator, median of 50 runs, against <code>adb shell uiautomator dump</code>, <code>screencap</code> and <code>input</code>. The difference comes from a small helper process that phonedriver starts on the phone. It keeps the accessibility connection open, where adb starts a new process and writes XML each time. <a href="/benchmarks/">Method and raw data.</a></p>
</div></section>

<section class="sec" id="asking-before-it-pays"><div class="wrap">
<h2>Asking before it pays</h2>
<div class="split">
<div class="txt">
<p>When a tap lands on a button labeled Pay, Buy now, Place order, Subscribe, Transfer or Delete account, phonedriver holds the tap and asks you in Claude Code's own prompt. The model never sees that question and can't answer it. In a client that can't show such a prompt, the tap waits and the model has to ask you in chat.</p>
<p>You choose Allow, Allow for this session, or Decline. If you want an agent to run without asking, you can say so in its instructions, and it passes <code>autonomous: true</code>; you can also turn the check off with <code>PHONEDRIVER_GUARD=off</code>.</p>
<p>The check reads button labels in English, Spanish, Portuguese, French, German, Italian, Dutch, Japanese, Chinese (Simplified), Korean and Hindi. It does not catch a payment button that is only an icon, or one labeled in another language. By default it asks about payments and deleting an account. To be asked about more, set <code>PHONEDRIVER_ASK</code> to any of <code>send</code> (Send, Reply, Post, Share), <code>delete</code> (Delete, Remove, Clear all) and <code>install</code> (Install, Uninstall), for example <code>PHONEDRIVER_ASK=send,delete</code>. The prompt says which kind of tap it is.</p>
<p>The model can't type plain text into a password field or a one-time code field. It gets an error that tells it to use <code>secret</code>, which keeps the text out of logs and traces.</p>
</div>
<figure class="shot"><img src="/assets/guard-poster.jpg" width="1280" height="720" alt="Claude Code shows phonedriver's question: your agent wants to tap Place order, $12.00, in the testbed app. The choices are Allow, Accept and Decline." decoding="async" loading="lazy"></figure>
</div></div></section>

<section class="sec" id="real-jobs"><div class="wrap">
<h2>Real jobs</h2>
<p class="big">I tested phonedriver on nine jobs done in apps' own screens. They included setting Chrome's search engine, cropping a photo, pinning and archiving Messages threads, finding the app that keeps the phone awake, and testing a sign-up flow for planted bugs. Each job ran twice with Claude Sonnet 5.5. A script checked the phone's own data afterwards, not the model's report. All 18 runs passed. This is my own test set, published with every run: <a href="/benchmarks/#jobs">the jobs and the checks</a>.</p>
</div></section>

<section class="sec" id="works-with"><div class="wrap">
<h2>Works with</h2>
<div class="split rev"><div class="tw plain"><table><thead><tr><th scope="col">Phone</th><th scope="col">You need</th></tr></thead><tbody>
<tr><th scope="row">Android emulator</th><td>adb</td></tr>
<tr><th scope="row">Android phone or tablet</th><td>adb and USB debugging</td></tr>
<tr><th scope="row">iOS Simulator</th><td>a Mac with Xcode</td></tr>
<tr><th scope="row">iPhone or iPad</th><td>a Mac with Xcode and an Apple developer team (<a href="/docs/iphone/">setup</a>)</td></tr>
</tbody></table></div>
<div class="txt"><p>On Android there is no app to install. phonedriver pushes its helper over adb when it connects. On iOS it builds a small XCTest runner on your Mac.</p>
<p>The MCP server has 14 tools: <code>devices</code>, <code>apps</code>, <code>snapshot</code>, <code>screenshot</code>, <code>tap</code>, <code>type</code>, <code>swipe</code>, <code>scroll</code>, <code>press</code>, <code>launch</code>, <code>wait</code>, <code>batch</code>, <code>clipboard</code> and <code>shell</code>. The <code>phonedriver</code> CLI has the same commands, for scripts and for agents that prefer a shell.</p></div>
</div></div></section>

<section class="sec" id="limits"><div class="wrap rail">
<h2>Limits</h2>
<ul class="limits">
<li>Tested on Android emulators (API 26 and 37), the iOS Simulator, an iPhone 14 Plus and an iPad Pro. Real Android phones use the same path but have had little testing. Bug reports from real phones are welcome.</li>
<li>A tap takes 70 to 120 ms on an Android emulator and 450 to 530 ms on a real iPhone.</li>
<li>The model can do anything on the phone you could do with your finger. For unattended runs, use an emulator or a spare phone.</li>
<li>phonedriver never enters a passcode and can't pass Face ID or a fingerprint check. Screens that block screenshots, as some banking apps do, come out black in a screenshot, though their text can still be read.</li>
</ul>
</div></section>

<section class="sec" id="faq"><div class="wrap">
<h2>FAQ</h2>
<div class="faq">{faq_html()}</div></div></section>
<script>{COPY_JS}</script>
"""
    sw = software_ld()
    faq = {"@context": "https://schema.org", "@type": "FAQPage", "mainEntity": [
        {"@type": "Question", "name": q, "acceptedAnswer": {"@type": "Answer", "text": plain(a)}} for q, a in D.FAQ]}
    video = {"@context": "https://schema.org", "@type": "VideoObject", "name": "Claude Code drives an Android emulator with phonedriver and asks before it pays",
             "description": D.DEMO_CAPTION, "thumbnailUrl": url("/assets/demo-poster.jpg"), "uploadDate": TODAY, "contentUrl": url("/assets/demo.mp4"), "duration": "PT46S", "width": 1280, "height": 720}
    site = {"@context": "https://schema.org", "@type": "WebSite", "name": "phonedriver", "url": url("/"), "inLanguage": "en"}
    layout("/", "phonedriver: give your AI agent a phone", D.HOME_DESC, body, "WebPage", None, sw + [site, video, faq], og="/assets/hero-social.png")
    FULL.insert(0, ("/", "phonedriver", "# phonedriver\n\n" + D.DEFINITION + "\n\n" + "\n\n".join("## %s\n\n%s" % (q, plain(a)) for q, a in D.FAQ + D.FAQ_MORE) + "\n"))


def plain(s):
    import re
    s = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", r"\1", s)
    return s.replace("`", "")


def software_ld():
    app = {
        "@context": "https://schema.org", "@type": "SoftwareApplication", "@id": url("/#software"),
        "name": "phonedriver", "alternateName": "phonedriver MCP server",
        "description": D.DEFINITION, "url": url("/"),
        "applicationCategory": "DeveloperApplication",
        "operatingSystem": "macOS, Linux, Windows (controls Android and iOS devices)",
        "softwareVersion": "0.1.0", "license": "https://www.gnu.org/licenses/agpl-3.0.html",
        "offers": {"@type": "Offer", "price": "0", "priceCurrency": "USD"},
        "featureList": ["Text snapshot of an Android or iOS screen", "Tap, type, swipe, scroll and press", "Every action returns the lines that changed after the screen settles", "MCP server and CLI", "Approval guard for taps that spend money", "No telemetry"],
        "sameAs": [REPO], "author": AUTHOR, "isAccessibleForFree": True,
    }
    code = {
        "@context": "https://schema.org", "@type": "SoftwareSourceCode", "@id": url("/#source"),
        "name": "phonedriver source", "codeRepository": REPO, "programmingLanguage": "Rust",
        "license": "https://www.gnu.org/licenses/agpl-3.0.html", "targetProduct": {"@id": url("/#software")},
    }
    return [app, code]




def build_extras(posts):
    base = url("")
    sm = ""
    for p in PAGES:
        sm += "<url><loc>%s</loc><lastmod>%s</lastmod></url>" % (url(p["path"]), p["date"])
    write("sitemap.xml", '<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">%s</urlset>' % sm)
    bots = ["GPTBot", "ChatGPT-User", "OAI-SearchBot", "ClaudeBot", "Claude-User", "Claude-SearchBot", "PerplexityBot", "Perplexity-User", "Google-Extended", "Applebot-Extended", "CCBot"]
    r = "".join("User-agent: %s\nAllow: /\n\n" % b for b in bots)
    r += "User-agent: *\nAllow: /\n\nSitemap: %s/sitemap.xml\n" % SITE_URL
    write("robots.txt", r)
    write("404.html", "")
    # llms.txt
    L = ["# phonedriver", "", "> " + D.DEFINITION, "", D.LLMS_NOTE, "", "## Start here", ""]
    L += ["- [Install](%s/docs/install/): one-line install and build from source" % SITE_URL,
          "- [Claude Code setup](%s/docs/claude-code/): `claude mcp add phonedriver -- npx -y phonedriver mcp`" % SITE_URL,
          "- [Cursor and other MCP clients](%s/docs/cursor-and-mcp-clients/): the stdio server entry" % SITE_URL, "", "## Docs", ""]
    for s in DOCS:
        m = read_md(HERE / "content" / "docs" / (s + ".md"))[0]
        L.append("- [%s](%s/docs/%s/): %s" % (m["title"], SITE_URL, s, m["description"]))
    L += ["", "## Benchmarks", "", "- [Benchmarks](%s/benchmarks/): speed against adb and phone-harness with p50, p95 and confidence intervals; agent token counts; real phone jobs (18 of 18 for phonedriver and mobile-mcp, 14 of 18 for phone-harness)" % SITE_URL,
          "- [Raw speed samples](%s/benchmarks/data/speed-samples.csv)" % SITE_URL, "", "## Comparisons", ""]
    for s, label in COMPARE:
        m = read_md(HERE / "content" / "compare" / (s + ".md"))[0]
        L.append("- [%s](%s/compare/%s/): %s" % (label, SITE_URL, s, m["description"]))
    L += ["", "## Blog", ""]
    for s, m, path in posts:
        L.append("- [%s](%s%s): %s" % (m["title"], SITE_URL, path, m["description"]))
    L += ["", "## Optional", "", "- [Full text of this site](%s/llms-full.txt)" % SITE_URL, "- [Source code and design decisions](%s)" % REPO, ""]
    write("llms.txt", "\n".join(L))
    full = "# phonedriver: full documentation\n\n> %s\n\nSource: %s/. Canonical pages are linked per section.\n\n" % (D.DEFINITION, SITE_URL)
    for p, t, md in FULL:
        full += "---\n\nURL: %s%s\n\n%s\n" % (SITE_URL, p, md)
    write("llms-full.txt", full)


def copy_assets():
    A = OUT / "assets"
    (A / "fonts").mkdir(parents=True, exist_ok=True)
    gh = ROOT / ".github/assets"
    for n in ("hero-light.svg", "hero-dark.svg", "hero-social.png", "demo.mp4"):
        if (gh / n).exists():
            shutil.copy(gh / n, HERE / "assets" / n)
        elif not (HERE / "assets" / n).exists():
            raise SystemExit("missing " + n)
    for f in (HERE / "assets").glob("*"):
        if f.is_file():
            shutil.copy(f, A / f.name)
    from siteparts import fonts
    m = fonts.build(A / "fonts", ROOT / "design/build/fonts")
    for k, v in m.items():
        css.CSS = css.CSS.replace(k, v)
    shutil.copy(HERE / "assets" / "favicon.svg", OUT / "favicon.svg") if (HERE / "assets" / "favicon.svg").exists() else None


def cards():
    og = OUT / "assets" / "og"
    og.mkdir(parents=True, exist_ok=True)
    from siteparts import cards as C
    specs = {"default": ("Let your AI agent use a phone", "Android and iOS. CLI and MCP. Up to 98x faster than adb."),
             "docs": ("phonedriver docs", "Install, connect your agent, drive a device, keep it from spending your money."),
             "compare-adb": ("phonedriver vs adb", "98x faster tree reads, 41x faster screenshots. The bare tap is close."),
             "compare-mobile-mcp": ("phonedriver vs mobile-mcp", "Two MCP servers for phones, compared fact by fact."),
             "compare-phone-harness": ("phonedriver vs phone-harness", "No telemetry, a settled diff, and 22x faster typing."),
             }
    for i, s in enumerate(POSTS):
        specs["post-%d" % (i + 1)] = (read_md(HERE / "content/blog" / (s + ".md"))[0]["short_card"], read_md(HERE / "content/blog" / (s + ".md"))[0]["card_sub"])
    C.render(og, specs, ROOT / "design/build/fonts", CHROME, HERE / ".cache", SITE_URL.split("//")[-1].upper())


def main():
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir(parents=True)
    copy_assets()
    cards()
    build_home()
    build_docs()
    build_benchmarks()
    build_compare()
    posts = build_blog()
    layout("/404.html".replace(".html", "/"), "Page not found | phonedriver", "This page does not exist.", '<div class="wrap"><article class="prose col"><h1>Page not found</h1><p>Try the <a href="/docs/">docs</a>, the <a href="/benchmarks/">benchmarks</a> or the <a href="/">home page</a>.</p></article></div>', noindex=True)
    shutil.move(str(OUT / "404" / "index.html"), str(OUT / "404.html"))
    shutil.rmtree(OUT / "404")
    PAGES[:] = [p for p in PAGES if p["path"] != "/404/"]
    build_extras(posts)
    print("pages", len(PAGES))
    for s in POSTS:
        print(WORDS[s], s)


if __name__ == "__main__":
    main()
