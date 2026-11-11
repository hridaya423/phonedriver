import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const OUT = join(import.meta.dirname, "../out");
const TYPES = new Set(["SoftwareApplication", "SoftwareSourceCode", "WebSite", "FAQPage", "Question", "Answer", "BreadcrumbList", "ListItem", "BlogPosting", "TechArticle", "Article", "Blog", "Dataset", "DataDownload", "Person", "Organization", "Offer", "VideoObject", "CollectionPage", "WebPage", "ItemList"]);
const REQ = {
  SoftwareApplication: ["name", "applicationCategory", "operatingSystem", "offers", "license"],
  SoftwareSourceCode: ["codeRepository"],
  BlogPosting: ["headline", "datePublished", "author", "image"],
  TechArticle: ["headline", "author"],
  Article: ["headline", "author"],
  FAQPage: ["mainEntity"],
  BreadcrumbList: ["itemListElement"],
  Dataset: ["name", "description"],
};

const walk = (d) => readdirSync(d).flatMap((n) => (statSync(join(d, n)).isDirectory() ? walk(join(d, n)) : [join(d, n)]));
const attrs = (tag) => Object.fromEntries([...tag.matchAll(/([\w:-]+)(?:="([^"]*)")?/g)].slice(1).map((m) => [m[1], (m[2] ?? "").replace(/&amp;/g, "&")]));
const resolve = (p) => {
  const f = join(OUT, decodeURIComponent(p));
  return existsSync(f) && statSync(f).isDirectory() ? join(f, "index.html") : f;
};

const pages = new Map();
for (const f of walk(OUT).filter((f) => f.endsWith(".html"))) {
  const html = readFileSync(f, "utf8");
  const tags = [...html.matchAll(/<(a|link|img|source|meta|script|h1|title)\b[^>]*>/g)].map((m) => ({ name: m[1], a: attrs(m[0]) }));
  pages.set(f, {
    html,
    tags,
    ids: new Set([...html.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1])),
    ld: [...html.matchAll(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/g)].map((m) => m[1]),
    title: /<title>([^<]*)<\/title>/.exec(html)?.[1] ?? "",
  });
}

let bad = 0;
const types = new Set();
const fail = (...m) => {
  console.log(...m);
  bad++;
};
for (const [f, p] of pages) {
  const rel = relative(OUT, f);
  for (const t of p.tags) {
    const refs = t.name === "a" ? [t.a.href] : t.name === "link" && t.a.rel !== "canonical" && t.a.rel !== "alternate" ? [t.a.href] : t.name === "img" ? [t.a.src] : t.name === "source" ? (t.a.srcset ?? t.a.src ?? "").split(",").map((x) => x.trim().split(" ")[0]) : [];
    for (const ref of refs.filter(Boolean)) {
      if (/^(https?:|mailto:|data:)/.test(ref)) continue;
      const [u, frag] = ref.split("#");
      const target = u ? resolve(u) : f;
      if (!existsSync(target)) fail("BROKEN", rel, ref);
      else if (frag && target.endsWith(".html") && pages.has(target) && !pages.get(target).ids.has(frag)) fail("BAD ANCHOR", rel, ref);
    }
  }
  for (const s of p.ld) {
    let d;
    try {
      d = JSON.parse(s);
    } catch (e) {
      fail("LD PARSE", rel, e.message);
      continue;
    }
    for (const node of Array.isArray(d) ? d : [d]) {
      types.add(node["@type"]);
      if (!TYPES.has(node["@type"])) fail("LD UNKNOWN TYPE", rel, node["@type"]);
      for (const k of REQ[node["@type"]] ?? []) if (!(k in node)) fail("LD MISSING", rel, node["@type"], k);
    }
  }
  if (!["404.html", "404/index.html", "_not-found/index.html"].includes(rel)) {
    const meta = Object.fromEntries(p.tags.filter((t) => t.name === "meta").map((t) => [t.a.name ?? t.a.property, t.a.content ?? ""]));
    if (!p.title || p.title.length > 68) console.log("TITLE", rel, p.title.length);
    const d = meta.description ?? "";
    if (d.length < 60 || d.length > 160) console.log("DESC LEN", rel, d.length);
    const h1 = p.tags.filter((t) => t.name === "h1").length;
    if (h1 !== 1) fail("H1", rel, h1);
    for (const k of ["og:title", "og:image", "twitter:card", "twitter:image"]) if (!(k in meta)) fail("META", rel, k);
    if (!p.tags.some((t) => t.name === "link" && t.a.rel === "canonical")) fail("CANONICAL", rel);
  }
  for (const t of p.tags.filter((t) => t.name === "img")) if (!t.a.alt || !t.a.width || !t.a.height) fail("IMG", rel, JSON.stringify(t.a));
}
for (const f of ["sitemap.xml", "robots.txt", "llms.txt", "llms-full.txt", "blog/feed.xml", "404.html", "favicon.svg", "benchmarks/data/speed-samples.csv", "og/docs.png"])
  if (!existsSync(join(OUT, f))) fail("MISSING", f);
console.log("pages", [...pages.keys()].filter((f) => !f.includes("_not-found")).length, "ld types", [...types].filter(Boolean).sort(), "problems", bad);
process.exit(bad ? 1 : 0);
