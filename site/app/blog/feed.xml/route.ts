import { esc } from "@/lib/md";
import { POSTS, page } from "@/lib/content";
import { url } from "@/lib/site";

export const dynamic = "force-static";

export function GET() {
  const items = POSTS.map((s) => {
    const p = page("blog", s);
    const date = new Date(`${p.meta.date}T00:00:00Z`).toUTCString();
    return `<item><title>${esc(p.meta.title)}</title><link>${url(p.path)}</link><guid>${url(p.path)}</guid><pubDate>${date}</pubDate><description>${esc(p.meta.description)}</description></item>`;
  }).join("");
  const xml = `<?xml version="1.0" encoding="UTF-8"?><rss version="2.0"><channel><title>phonedriver blog</title><link>${url("/blog/")}</link><description>How phonedriver works and what we measured.</description>${items}</channel></rss>`;
  return new Response(xml, { headers: { "Content-Type": "application/rss+xml; charset=utf-8" } });
}
