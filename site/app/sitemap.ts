import type { MetadataRoute } from "next";
import { COMPARE, DOCS, POSTS, page } from "@/lib/content";
import { PUBLISHED, TODAY, url } from "@/lib/site";

export const dynamic = "force-static";

export default function sitemap(): MetadataRoute.Sitemap {
  const at = (path: string, date = TODAY) => ({ url: url(path), lastModified: date });
  return [
    at("/"),
    at("/docs/"),
    ...DOCS.map((s) => at(`/docs/${s}/`, page("docs", s).meta.updated ?? PUBLISHED)),
    at("/benchmarks/"),
    at("/compare/"),
    ...COMPARE.map(([s]) => at(`/compare/${s}/`, page("compare", s).meta.updated ?? PUBLISHED)),
    at("/blog/"),
    ...POSTS.map((s) => at(`/blog/${s}/`, page("blog", s).meta.updated ?? page("blog", s).meta.date)),
  ];
}
