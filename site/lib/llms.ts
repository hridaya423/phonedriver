import { benchmarksMd } from "./bench";
import { COMPARE, DOCS, POSTS, fullMd, page } from "./content";
import { DEFINITION, FAQ, FAQ_MORE, LLMS_NOTE } from "./data";
import { plain } from "./md";
import { REPO, SITE_URL } from "./site";

export function llmsTxt() {
  const L = [
    "# phonedriver",
    "",
    `> ${DEFINITION}`,
    "",
    LLMS_NOTE,
    "",
    "## Set up",
    "",
    "Needs Node 18 or later. The first run downloads one binary and caches it.",
    "",
    "1. Add the MCP server. Claude Code: `claude mcp add phonedriver -- npx -y phonedriver mcp`. Codex: `codex mcp add phonedriver -- npx -y phonedriver mcp`. Any other client: a stdio server with command `npx` and args `[\"-y\", \"phonedriver\", \"mcp\"]`.",
    "2. Run `npx -y phonedriver doctor`. Each line says ok, warn or fail, and a fix follows every line that is not ok.",
    "3. Run `npx -y phonedriver devices`. With no device listed, start an Android emulator, plug in a phone with USB debugging on, or boot an iOS Simulator.",
    "4. An agent with a shell and no MCP can use the same commands directly: `npx -y phonedriver snapshot`, `npx -y phonedriver tap @e4`. The text is the same as the MCP tools return.",
    "",
    "## Operate",
    "",
    "- `npx -y phonedriver skill` prints the instructions an agent should read before it uses the tools. Read it first.",
    "- `snapshot` lists the screen as lines such as `@e4 button \"Save\"`. Act on a ref (`tap @e4`). Every action waits for the screen to settle and returns only the lines that changed, so do not sleep, poll or call `snapshot` after each action.",
    "- A tap on Pay, Buy now, Place order, Transfer or Delete account is held until the human approves it (CLI exit code 5). Do not work around it; ask the human.",
    "- Exit codes: 0 done, 1 failed, 2 wrong command line, 3 a person must act on the device, 4 no matching device, 5 needs the human's approval.",
    "- Text for a password or a one-time code goes through `type` with `secret`, never as a plain argument.",
    "",
    "## Docs",
    "",
    "Each link is the page as Markdown. Drop `index.md` for the web page.",
    "",
  ];
  const md = (path: string) => `${SITE_URL}${path}index.md`;
  for (const s of DOCS) {
    const p = page("docs", s);
    L.push(`- [${p.meta.title}](${md(p.path)}): ${p.meta.description}`);
  }
  L.push(
    "",
    "## Benchmarks",
    "",
    `- [Benchmarks](${md("/benchmarks/")}): real phone jobs on 10 November 2026 (27 of 27 runs for phonedriver, 17 of 18 for mobile-mcp at 6.9 times the tokens, 14 of 18 for phone-harness); speed against adb and phone-harness with p50, p95 and confidence intervals`,
    `- [Raw job runs](${SITE_URL}/benchmarks/data/jobs.json)`,
    `- [Raw speed samples](${SITE_URL}/benchmarks/data/speed-samples.csv)`,
    "",
    "## Comparisons",
    "",
  );
  for (const [s, label] of COMPARE) {
    const p = page("compare", s);
    L.push(`- [${label}](${md(p.path)}): ${p.meta.description}`);
  }
  L.push("", "## Blog", "");
  for (const s of POSTS) {
    const p = page("blog", s);
    L.push(`- [${p.meta.title}](${md(p.path)}): ${p.meta.description}`);
  }
  L.push("", "## Optional", "", `- [Full text of this site](${SITE_URL}/llms-full.txt)`, `- [Changelog](${SITE_URL}/changelog/)`, `- [Source code and design decisions](${REPO})`, `- [Questions and discussion](${REPO}/discussions)`, "");
  return L.join("\n");
}

export function llmsFullTxt() {
  const home = `# phonedriver\n\n${DEFINITION}\n\n${[...FAQ, ...FAQ_MORE].map(([q, a]) => `## ${q}\n\n${plain(a)}`).join("\n\n")}\n`;
  const parts: [string, string][] = [
    ["/", home],
    ...DOCS.map((s) => page("docs", s)).map((p) => [p.path, fullMd(p)] as [string, string]),
    ["/benchmarks/", benchmarksMd()],
    ...COMPARE.map(([s]) => page("compare", s)).map((p) => [p.path, fullMd(p)] as [string, string]),
    ...POSTS.map((s) => page("blog", s)).map((p) => [p.path, fullMd(p)] as [string, string]),
  ];
  let full = `# phonedriver: full documentation\n\n> ${DEFINITION}\n\nSource: ${SITE_URL}/. Canonical pages are linked per section.\n\n`;
  for (const [p, md] of parts) full += `---\n\nURL: ${SITE_URL}${p}\n\n${md}\n`;
  return full;
}
