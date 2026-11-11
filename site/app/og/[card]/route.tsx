import { readFileSync } from "node:fs";
import { join } from "node:path";
import { ImageResponse } from "next/og";
import { VERSIONS, board, op, ratio, short, type Harness } from "@/lib/bench";
import { WIDTH, wordPath } from "@/lib/brand";
import { lockupSvg, lockupWidth } from "@/lib/chauffeur";
import { POSTS, page } from "@/lib/content";
import { SITE_URL } from "@/lib/site";

export const dynamic = "force-static";
export const dynamicParams = false;

const FEWER = (() => {
  const b = board();
  return ratio(b.total("mobile-mcp").tokens / b.total("phonedriver").tokens);
})();
const BARS = (() => {
  const b = board();
  const rows = (["phonedriver", "phone-harness", "mobile-mcp"] as Harness[]).map((h) => ({ h, tokens: b.total(h).tokens }));
  const max = Math.max(...rows.map((r) => r.tokens));
  return rows.map((r) => ({ ...r, f: r.tokens / max }));
})();
const faster = (id: string, than: "adb" | "phone_harness" = "adb") => ratio(op(id)[than]! / op(id).phonedriver);

const CARDS: Record<string, [string, string]> = {
  default: ["Give your AI agent a phone", "Android and iOS. MCP and CLI. It reads the screen as text and asks before it pays."],
  docs: ["phonedriver docs", "Install, connect your agent, drive a device, keep it from spending your money."],
  benchmarks: [`${FEWER} fewer tokens on real phone jobs`, `Nine jobs, three MCP servers, one emulator. And ${faster("tree_list")} faster than adb at reading a screen.`],
  "compare-adb": ["phonedriver vs adb", `${faster("tree_list")} faster screen reads, ${faster("screenshot")} faster screenshots. The bare tap is close.`],
  "compare-mobile-mcp": ["phonedriver vs mobile-mcp", `Two MCP servers for phones. ${FEWER} fewer tokens on nine real phone jobs.`],
  "compare-phone-harness": ["phonedriver vs phone-harness", `No telemetry, a settled diff, and ${faster("type100", "phone_harness")} faster typing.`],
};

function card(name: string): [string, string] {
  const post = /^post-(\d+)$/.exec(name);
  if (!post) return CARDS[name];
  const m = page("blog", POSTS[Number(post[1]) - 1]).meta;
  return [m.short_card, m.card_sub];
}

export const generateStaticParams = () => [...Object.keys(CARDS), ...POSTS.map((_, i) => `post-${i + 1}`)].map((n) => ({ card: `${n}.png` }));

const MARK_H = 58;
const MARK = `data:image/svg+xml;base64,${Buffer.from(lockupSvg({ word: wordPath(), wordWidth: WIDTH, ink: "#0b0b0c", height: MARK_H })).toString("base64")}`;

const font = (w: number) => readFileSync(join(process.cwd(), `lib/og/jetbrains-mono-${w}.ttf`));

export async function GET(_: Request, { params }: { params: Promise<{ card: string }> }) {
  const name = (await params).card.replace(/\.png$/, "");
  const [title, sub] = card(name);
  const size = title.length < 30 ? 84 : title.length < 56 ? 66 : 54;
  return new ImageResponse(
    (
      <div style={{ width: "100%", height: "100%", display: "flex", flexDirection: "column", background: "#fff", color: "#0b0b0c", padding: "56px 64px", fontFamily: "JB" }}>
        <div style={{ display: "flex" }}>
          <img src={MARK} width={Math.round(lockupWidth(WIDTH, MARK_H))} height={MARK_H} alt="" />
        </div>
        {name === "benchmarks" ? (
          <div style={{ display: "flex", flex: 1, alignItems: "center", justifyContent: "space-between" }}>
            <div style={{ display: "flex", flexDirection: "column" }}>
              <div style={{ fontSize: 208, fontWeight: 800, lineHeight: 0.9, letterSpacing: -14, color: "#ff5b14" }}>{FEWER}</div>
              <div style={{ display: "flex", flexDirection: "column", marginTop: 22, fontSize: 38, fontWeight: 800, lineHeight: 1.08, letterSpacing: -2.1 }}>
                <span>fewer tokens than</span>
                <span>mobile-mcp on nine</span>
                <span>real phone jobs</span>
              </div>
            </div>
            <div style={{ display: "flex", flexDirection: "column", width: 500 }}>
              {BARS.map((r) => (
                <div key={r.h} style={{ display: "flex", flexDirection: "column", marginBottom: 26 }}>
                  <div style={{ display: "flex", fontSize: 21, letterSpacing: -0.4, color: r.h === "phonedriver" ? "#0b0b0c" : "#6b675e", fontWeight: r.h === "phonedriver" ? 800 : 400 }}>
                    {r.h} {VERSIONS[r.h]}
                  </div>
                  <div style={{ display: "flex", alignItems: "center", marginTop: 9 }}>
                    <div style={{ width: Math.max(6, Math.round(400 * r.f)), height: 34, background: r.h === "phonedriver" ? "#ff5b14" : "#8f8a7c" }} />
                    <div style={{ display: "flex", marginLeft: 14, fontSize: 24, fontWeight: 800, letterSpacing: -0.6 }}>{short(r.tokens)}</div>
                  </div>
                </div>
              ))}
              <div style={{ display: "flex", fontSize: 18, color: "#6b675e", letterSpacing: -0.3 }}>Tokens, Claude Sonnet 5.5, one emulator</div>
            </div>
          </div>
        ) : (
          <>
            <div style={{ display: "flex", flex: 1, alignItems: "center" }}>
              <div style={{ fontSize: size, fontWeight: 800, lineHeight: 1.02, letterSpacing: -size * 0.055, maxWidth: 1040 }}>{title}</div>
            </div>
            <div style={{ display: "flex", fontSize: 26, color: "#55524a", lineHeight: 1.35, letterSpacing: -0.5, maxWidth: 1000 }}>{sub}</div>
          </>
        )}
        <div style={{ display: "flex", alignItems: "center", marginTop: 34, fontSize: 18, color: "#6b675e", letterSpacing: 1.5 }}>
          <span>{SITE_URL.split("//")[1].toUpperCase()}</span>
          <div style={{ display: "flex", flex: 1, justifyContent: "space-between", margin: "0 4px 0 24px" }}>
            {Array.from({ length: 84 }, (_, i) => (
              <div key={i} style={{ width: 3.5, height: 3.5, borderRadius: 4, background: "#ff5b14" }} />
            ))}
          </div>
          <span style={{ color: "#ff5b14", marginLeft: 6, fontWeight: 800 }}>→</span>
        </div>
      </div>
    ),
    {
      width: 1200,
      height: 630,
      fonts: [
        { name: "JB", data: font(400), weight: 400, style: "normal" },
        { name: "JB", data: font(800), weight: 800, style: "normal" },
      ],
    },
  );
}
