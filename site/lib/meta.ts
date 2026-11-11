import type { Metadata } from "next";
import { url } from "./site";

export function meta({
  path,
  title,
  description,
  og = "/og/default.png",
  article = false,
  noindex = false,
  md = false,
}: {
  path: string;
  title: string;
  description: string;
  og?: string;
  article?: boolean;
  noindex?: boolean;
  md?: boolean;
}): Metadata {
  const image = { url: og, width: 1200, height: 630 };
  return {
    title: { absolute: title },
    description,
    alternates: {
      canonical: url(path),
      types: { "application/rss+xml": [{ url: "/blog/feed.xml", title: "phonedriver blog" }], ...(md ? { "text/markdown": [{ url: `${path}index.md`, title: "This page as Markdown" }] } : {}) },
    },
    robots: noindex ? { index: false } : undefined,
    openGraph: { type: article ? "article" : "website", siteName: "phonedriver", title, description, url: url(path), images: [image] },
    twitter: { card: "summary_large_image", title, description, images: [og] },
  };
}
