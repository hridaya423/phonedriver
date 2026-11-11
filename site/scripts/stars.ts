import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { REPO } from "../lib/site.ts";

const out = join(import.meta.dirname, "../lib/stars.json");
const api = REPO.replace("https://github.com/", "https://api.github.com/repos/");

// GitHub allows 60 unauthenticated API requests an hour per address, and a build may run offline: then the last count stays.
try {
  const reply = await fetch(api, { headers: { accept: "application/vnd.github+json", "user-agent": "phonedriver-site-build" }, signal: AbortSignal.timeout(8000) });
  if (!reply.ok) throw new Error(`GitHub answered ${reply.status}`);
  const { stargazers_count: stars } = (await reply.json()) as { stargazers_count: number };
  writeFileSync(out, `${JSON.stringify({ stars, at: new Date().toISOString().slice(0, 10) })}\n`);
  console.log(`stars: ${stars}`);
} catch (error) {
  console.warn(`stars: kept the last count (${(error as Error).message})`);
}
