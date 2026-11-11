import { readFileSync } from "node:fs";
import { join } from "node:path";
import { front, render, type Toc } from "./md";
import { BOOT, REPO, SITE_URL } from "./site";

export const DOCS = [
  "install",
  "claude-code",
  "cursor-and-mcp-clients",
  "android-emulator",
  "android-phone",
  "ios-simulator",
  "iphone",
  "approval-guard",
  "no-telemetry",
  "shell",
  "troubleshooting",
];

export const COMPARE: [string, string][] = [
  ["adb", "phonedriver vs adb"],
  ["mobile-mcp", "phonedriver vs mobile-mcp"],
  ["phone-harness", "phonedriver vs phone-harness"],
];

export const POSTS = [
  "how-phonedriver-reads-a-screen-170x-faster-than-adb",
  "phonedriver-vs-mobile-mcp-vs-phone-harness-vs-adb-real-phone-jobs",
  "letting-claude-order-dinner-without-letting-it-spend-your-money",
  "benchmarking-ai-agents-on-jobs-only-a-phone-can-do",
];

export type Section = "docs" | "compare" | "blog";

export type Page = { meta: Record<string, string>; text: string; html: string; toc: Toc; path: string };

export function page(section: Section, slug: string): Page {
  const src = readFileSync(join(process.cwd(), "content", section, `${slug}.md`), "utf8");
  const { meta, body } = front(src);
  const text = body.replaceAll("{{SITE}}", SITE_URL).replaceAll("{{REPO}}", REPO).replaceAll("{{BOOT}}", BOOT);
  const toc: Toc = [];
  return { meta, text, html: render(text, toc), toc, path: `/${section}/${slug}/` };
}

export function words(md: string) {
  return md
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/[`#|>*_[\]()-]/g, " ")
    .split(/\s+/)
    .filter(Boolean).length;
}

export const fullMd = (p: Page) => `# ${p.meta.title}\n\n${p.text.trim()}\n`;

export const pageMd = (p: Page) => `# ${p.meta.title}\n\n> ${p.meta.description}\n\nWeb page: ${SITE_URL}${p.path}\n\n${p.text.trim()}\n`;

export const markdown = (text: string) => new Response(text, { headers: { "Content-Type": "text/markdown; charset=utf-8" } });
