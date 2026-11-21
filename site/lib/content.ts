import { readFileSync } from "node:fs";
import { join } from "node:path";
import { FIGURE_FALLBACK, type FigureId } from "./blog-figure-meta";
import { front, plain, render, splitBlocks, type Block, type Toc } from "./md";
import { BOOT, REPO, SITE_URL } from "./site";

export const DOCS = [
  "install",
  "claude-code",
  "codex",
  "vs-code",
  "cursor-and-mcp-clients",
  "android-emulator",
  "android-phone",
  "ios-simulator",
  "iphone",
  "test-a-mobile-app",
  "replay-in-ci",
  "approval-guard",
  "no-telemetry",
  "shell",
  "read-a-web-page",
  "troubleshooting",
];

export const COMPARE: [string, string][] = [
  ["adb", "phonedriver vs adb"],
  ["mobile-mcp", "phonedriver vs mobile-mcp"],
  ["phone-harness", "phonedriver vs phone-harness"],
  ["appium", "phonedriver vs Appium"],
  ["simulator-use", "phonedriver vs simulator-use"],
  ["argent", "phonedriver vs Argent"],
];

export const POSTS = [
  "how-phonedriver-works",
  "how-phonedriver-reads-a-screen-170x-faster-than-adb",
  "phonedriver-vs-mobile-mcp-vs-phone-harness-vs-adb-real-phone-jobs",
  "letting-claude-order-dinner-without-letting-it-spend-your-money",
  "benchmarking-ai-agents-on-jobs-only-a-phone-can-do",
];

export type Section = "docs" | "compare" | "blog";

export type Page = { meta: Record<string, string>; text: string; html: string; blocks: Block[]; toc: Toc; path: string; faq: [string, string][] };

function cleanMd(md: string): string {
  return md
    .replace(/^::figure\[([a-z0-9-]+)\]\s*$/gm, (_, id: string) => FIGURE_FALLBACK[id as FigureId] ?? "")
    .replace(/^:::receipts(?:\s+(.+))?$/gm, (_, title?: string) => `### ${(title || "Method & receipts").trim()}`)
    .replace(/^:::\s*$/gm, "");
}

export function page(section: Section, slug: string): Page {
  const src = readFileSync(join(process.cwd(), "content", section, `${slug}.md`), "utf8");
  const { meta, body } = front(src);
  const text = body.replaceAll("{{SITE}}", SITE_URL).replaceAll("{{REPO}}", REPO).replaceAll("{{BOOT}}", BOOT);
  const toc: Toc = [];
  const html = render(text, toc);
  return { meta, text, html, blocks: splitBlocks(html), toc, path: `/${section}/${slug}/`, faq: questions(text) };
}

/** The `## Questions` section of a page, as question and answer pairs: each `###` heading and the paragraph under it. */
export function questions(text: string): [string, string][] {
  const at = text.indexOf("\n## Questions\n");
  if (at < 0) return [];
  const end = text.indexOf("\n## ", at + 1);
  return text
    .slice(at, end < 0 ? undefined : end)
    .split("\n### ")
    .slice(1)
    .map((b) => {
      const nl = b.indexOf("\n");
      return [plain(b.slice(0, nl).trim()), plain(b.slice(nl).replace(/\s+/g, " ").trim())];
    });
}

export const faqLd = (faq: [string, string][]) => ({
  "@context": "https://schema.org",
  "@type": "FAQPage",
  mainEntity: faq.map(([q, a]) => ({ "@type": "Question", name: q, acceptedAnswer: { "@type": "Answer", text: a } })),
});

export function words(md: string) {
  return md
    .replace(/^::figure\[[^\]]+\]\s*$/gm, " ")
    .replace(/^:::(?:receipts.*)?$/gm, " ")
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/[`#|>*_[\]()-]/g, " ")
    .split(/\s+/)
    .filter(Boolean).length;
}

export const fullMd = (p: Page) => `# ${p.meta.title}\n\n${cleanMd(p.text).trim()}\n`;

export const pageMd = (p: Page) => `# ${p.meta.title}\n\n> ${p.meta.description}\n\nWeb page: ${SITE_URL}${p.path}\n\n${cleanMd(p.text).trim()}\n`;

export const markdown = (text: string) => new Response(text, { headers: { "Content-Type": "text/markdown; charset=utf-8" } });
