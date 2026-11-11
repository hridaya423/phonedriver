import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { displaySvg, METRICS, ORANGE, WIDTH, wordPath } from "../lib/brand.ts";
import { lockupSvg, mark, PAINT } from "../lib/chauffeur.ts";

const SITE = join(import.meta.dirname, "..");
const ROOT = join(SITE, "..");
const ASSETS = join(ROOT, ".github/assets");
const LOGO = join(ROOT, "design/logo");
const CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const INK = "#0b0b0c";
const PAPER = "#f3f2ed";
const MUTED = { light: "#55524a", dark: "#a29d92" };

const font = readFileSync(join(SITE, "app/fonts/jetbrains-mono.woff2")).toString("base64");
const fontFace = `@font-face{font-family:JB;src:url(data:font/woff2;base64,${font}) format("woff2");font-weight:100 800}`;
const domain = /"https?:\/\/([^"]+)"/.exec(readFileSync(join(SITE, "lib/site.ts"), "utf8"))![1];
const tree = (JSON.parse(readFileSync(join(ROOT, "docs/bench/speed.json"), "utf8")).ops as { id: string; headline_adb: number }[]).find((o) => o.id === "tree_list")!;
const FASTER = `${Math.round(tree.headline_adb)}×`;

const write = (path: string, s: string | Buffer) => {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, s);
  console.log("wrote", path.replace(ROOT + "/", ""), typeof s === "string" ? `${(s.length / 1024).toFixed(1)} KB` : "");
};

const fit = (width: number, x: number, y: number) => {
  const s = width / WIDTH;
  return { transform: `translate(${x} ${y - METRICS.top * s}) scale(${+s.toFixed(4)})`, height: METRICS.height * s };
};

const at = (svg: string, x: number, y: number) => svg.replace("<svg ", `<svg x="${x}" y="${Math.round(y)}" `);

function banner(theme: "light" | "dark"): string {
  const W = 1280;
  const H = 400;
  const ink = theme === "light" ? INK : PAPER;
  const word = fit(1184, 48, 44);
  const y = 44 + word.height + 62;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img" aria-label="phonedriver. Let your AI agent use a phone. Android and iOS, CLI and MCP. Reads the screen up to ${Math.round(tree.headline_adb)} times faster than adb."><style>${fontFace}text{font-family:JB}.t{animation:up .8s cubic-bezier(.23,1,.32,1) 1.2s both}@keyframes up{from{opacity:0;transform:translateY(10px)}}@media (prefers-reduced-motion:reduce){.t{animation:none}}</style>${displaySvg({ ink, accent: ORANGE, animate: "inline", id: "w", transform: word.transform })}<g class="t">${at(mark("text", PAINT.icon, 52), 48, y - 38)}<text x="116" y="${y}" font-size="34" font-weight="800" letter-spacing="-1.4" fill="${ink}">Let your AI agent use a phone.</text><text x="${W - 50}" y="${y - 3}" font-size="15" font-weight="500" letter-spacing="1.4" fill="${MUTED[theme]}" text-anchor="end">ANDROID + IOS · CLI + MCP · <tspan fill="${ORANGE}" font-weight="800">${FASTER} FASTER THAN ADB</tspan></text></g></svg>`;
}

function card(w: number, h: number): string {
  const word = fit(w - 160, 80, h * 0.26);
  const y = h * 0.26 + word.height;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" width="${w}" height="${h}"><style>${fontFace}text{font-family:JB}</style><rect width="${w}" height="${h}" fill="${INK}"/><text x="80" y="${h * 0.14}" font-size="20" font-weight="500" letter-spacing="2" fill="${MUTED.dark}">ANDROID + IOS · CLI + MCP · OPEN SOURCE</text>${at(mark("full", PAINT.icon, 104), w - 80 - 104, h * 0.14 - 66)}${displaySvg({ ink: PAPER, accent: ORANGE, id: "w", transform: word.transform })}<text x="82" y="${y + 92}" font-size="54" font-weight="800" letter-spacing="-2.4" fill="${PAPER}">Let your AI agent use a phone.</text><text x="82" y="${h - 62}" font-size="22" font-weight="500" letter-spacing="1" fill="${MUTED.dark}"><tspan fill="${ORANGE}" font-weight="800">${FASTER} faster</tspan> than adb at reading the screen</text><text x="${w - 80}" y="${h - 62}" font-size="22" font-weight="500" letter-spacing="1" fill="${MUTED.dark}" text-anchor="end">${domain}</text></svg>`;
}

function png(svg: string, w: number, h: number, out: string) {
  const dir = mkdtempSync(join(tmpdir(), "brand-"));
  const html = join(dir, "i.html");
  const shot = join(dir, "i.png");
  writeFileSync(html, `<!doctype html><meta charset="utf-8"><body style="margin:0;background:transparent">${svg}</body>`);
  try {
    execFileSync(CHROME, ["--headless", "--hide-scrollbars", "--force-device-scale-factor=1", "--default-background-color=00000000", `--window-size=${w},${h}`, `--screenshot=${shot}`, `file://${html}`], { stdio: "ignore" });
  } catch (e) {
    // Headless Chrome can exit 2 ("Teardown watchdog expired") after it has already written the screenshot.
    if (!existsSync(shot)) throw e;
  }
  write(out, readFileSync(shot));
  rmSync(dir, { recursive: true });
}

const word = { word: wordPath(), wordWidth: WIDTH };
write(join(SITE, "public/favicon.svg"), mark("micro", PAINT.icon, 32));
write(join(LOGO, "icon.svg"), mark("full", PAINT.icon, 512));
write(join(LOGO, "chauffeur.svg"), mark("full", PAINT.light, 512));
write(join(LOGO, "chauffeur-dark.svg"), mark("full", PAINT.dark, 512));
write(join(LOGO, "lockup.svg"), lockupSvg({ ...word, ink: INK }));
write(join(LOGO, "lockup-dark.svg"), lockupSvg({ ...word, ink: PAPER }));
for (const [name, ink] of [
  ["display", INK],
  ["display-dark", PAPER],
] as const) {
  const m = fit(WIDTH, 0, 0);
  write(join(LOGO, `${name}.svg`), `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${WIDTH} ${METRICS.height}" width="${WIDTH}" height="${METRICS.height}" role="img" aria-label="phonedriver">${displaySvg({ ink, accent: ORANGE, id: "w", transform: m.transform })}</svg>`);
}
png(mark("full", PAINT.icon, 512), 512, 512, join(LOGO, "png/icon-512.png"));
const small = join(tmpdir(), "phonedriver-icon-small.png");
png(mark("micro", PAINT.icon, 512), 512, 512, small);
for (const [n, from] of [
  [180, join(LOGO, "png/icon-512.png")],
  [32, small],
  [16, small],
] as const) {
  const out = join(LOGO, `png/icon-${n}.png`);
  execFileSync("sips", ["-z", String(n), String(n), from, "--out", out], { stdio: "ignore" });
  console.log("wrote", out.replace(ROOT + "/", ""));
}
rmSync(small);
write(join(ASSETS, "banner-light.svg"), banner("light"));
write(join(ASSETS, "banner-dark.svg"), banner("dark"));
png(card(1280, 640), 1280, 640, join(ASSETS, "social-preview.png"));
png(card(1200, 630), 1200, 630, join(SITE, "public/assets/og-home.png"));
