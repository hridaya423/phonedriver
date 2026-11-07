#!/usr/bin/env python3
"""Checks site/public: internal links and anchors resolve, JSON-LD parses and uses known schema.org types, every page has the basics."""
import json
import re
import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import urldefrag, urlparse

PUB = Path(__file__).resolve().parent / "public"
TYPES = {"SoftwareApplication", "SoftwareSourceCode", "WebSite", "FAQPage", "Question", "Answer", "BreadcrumbList", "ListItem", "BlogPosting",
         "TechArticle", "Article", "Blog", "Dataset", "DataDownload", "Person", "Organization", "Offer", "VideoObject", "CollectionPage", "WebPage"}
REQ = {"SoftwareApplication": ["name", "applicationCategory", "operatingSystem", "offers", "license"], "SoftwareSourceCode": ["codeRepository"],
       "BlogPosting": ["headline", "datePublished", "author", "image"], "TechArticle": ["headline", "author"], "Article": ["headline", "author"],
       "FAQPage": ["mainEntity"], "BreadcrumbList": ["itemListElement"], "Dataset": ["name", "description"]}


class P(HTMLParser):
    def __init__(self):
        super().__init__()
        self.links, self.ids, self.ld, self.meta, self.title, self.h1, self._in, self.imgs = [], set(), [], {}, "", 0, None, []

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if "id" in a:
            self.ids.add(a["id"])
        if tag == "a" and a.get("href"):
            self.links.append(a["href"])
        if tag in ("link",) and a.get("href") and a.get("rel") != "canonical":
            self.links.append(a["href"])
        if tag == "img":
            self.imgs.append(a)
            if a.get("src"):
                self.links.append(a["src"])
        if tag == "source" and a.get("srcset"):
            self.links.append(a["srcset"])
        if tag == "h1":
            self.h1 += 1
        if tag == "meta" and a.get("name"):
            self.meta[a["name"]] = a.get("content", "")
        if tag == "meta" and a.get("property"):
            self.meta[a["property"]] = a.get("content", "")
        if tag == "script" and a.get("type") == "application/ld+json":
            self._in = "ld"
            self.buf = ""
        if tag == "title":
            self._in = "title"

    def handle_data(self, d):
        if self._in == "ld":
            self.buf += d
        elif self._in == "title":
            self.title += d

    def handle_endtag(self, tag):
        if tag == "script" and self._in == "ld":
            self.ld.append(self.buf)
        if tag in ("script", "title"):
            self._in = None


def resolve(path):
    p = PUB / path.lstrip("/")
    if p.is_dir():
        p = p / "index.html"
    return p


pages = {}
for f in PUB.rglob("*.html"):
    p = P()
    p.feed(f.read_text())
    pages[f] = p
bad = 0
types = set()
for f, p in pages.items():
    rel = f.relative_to(PUB)
    for l in p.links:
        for part in [x.strip().split(" ")[0] for x in l.split(",")]:
            if part.startswith(("http:", "https:", "mailto:")):
                continue
            u, frag = urldefrag(part)
            tgt = f if not u else resolve(urlparse(u).path)
            if not tgt.exists():
                print("BROKEN", rel, part)
                bad += 1
            elif frag and tgt.suffix == ".html":
                q = pages.get(tgt)
                if q and frag not in q.ids:
                    print("BAD ANCHOR", rel, part)
                    bad += 1
    for s in p.ld:
        try:
            d = json.loads(s)
        except Exception as e:
            print("LD PARSE", rel, e)
            bad += 1
            continue
        for node in (d if isinstance(d, list) else [d]):
            t = node.get("@type")
            types.add(t)
            if t not in TYPES:
                print("LD UNKNOWN TYPE", rel, t)
                bad += 1
            for k in REQ.get(t, []):
                if k not in node:
                    print("LD MISSING", rel, t, k)
                    bad += 1
    if rel.name != "404.html":
        if not p.title or len(p.title) > 68:
            print("TITLE", rel, len(p.title))
        d = p.meta.get("description", "")
        if not (60 <= len(d) <= 160):
            print("DESC LEN", rel, len(d))
        if p.h1 != 1:
            print("H1", rel, p.h1)
        for k in ("og:title", "og:image", "twitter:card"):
            if k not in p.meta:
                print("META", rel, k)
                bad += 1
    for i in p.imgs:
        if not i.get("alt") or not i.get("width") or not i.get("height"):
            print("IMG", rel, i)
            bad += 1
print("pages", len(pages), "ld types", sorted(t for t in types if t), "problems", bad)
sys.exit(1 if bad else 0)
