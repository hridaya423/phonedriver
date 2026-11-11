const f = (v: number) => Math.round(v * 100) / 100;

type R4 = [number, number, number, number];

function box(x: number, y: number, w: number, h: number, r: number | R4): string {
  const [a, b, c, d] = (typeof r === "number" ? [r, r, r, r] : r).map((v) => Math.min(v, w / 2, h / 2));
  const k = 0.7;
  let p = `M${f(x + a)} ${f(y)}H${f(x + w - b)}`;
  if (b) p += `C${f(x + w - b + b * k)} ${f(y)} ${f(x + w)} ${f(y + b - b * k)} ${f(x + w)} ${f(y + b)}`;
  p += `V${f(y + h - c)}`;
  if (c) p += `C${f(x + w)} ${f(y + h - c + c * k)} ${f(x + w - c + c * k)} ${f(y + h)} ${f(x + w - c)} ${f(y + h)}`;
  p += `H${f(x + d)}`;
  if (d) p += `C${f(x + d - d * k)} ${f(y + h)} ${f(x)} ${f(y + h - d + d * k)} ${f(x)} ${f(y + h - d)}`;
  p += `V${f(y + a)}`;
  if (a) p += `C${f(x)} ${f(y + a - a * k)} ${f(x + a - a * k)} ${f(y)} ${f(x + a)} ${f(y)}`;
  return p + "Z";
}

const dot = (cx: number, cy: number, r: number) => `M${f(cx - r)} ${f(cy)}a${f(r)} ${f(r)} 0 1 0 ${f(2 * r)} 0a${f(r)} ${f(r)} 0 1 0 ${f(-2 * r)} 0Z`;

export type GlovePaint = { line: string; fill: string; accent: string };

const FINGER = 20;
const LINE = 3;

const keyline = (d: string, p: GlovePaint) => `<path d="${d}" fill="${p.line}" stroke="${p.line}" stroke-width="${2 * LINE}" stroke-linejoin="round"/>`;

export const HAND = { x: -32, y: -12, w: 126, h: 188, tipX: 10, tipY: 0 };
export type Pose = "point" | "open";

const DIGITS = [
  { x: 0, top: 0, w: FINGER, curl: 0 },
  { x: 23, top: -6, w: FINGER, curl: 37 },
  { x: 46, top: 2, w: FINGER, curl: 32 },
  { x: 69, top: 14, w: 18, curl: 26 },
];

/** One glove, two poses. With a pose the fingers are placed for a still image; without one they carry `g-f` and `--c` so CSS can curl and open them. */
export function hand(p: GlovePaint, pose?: Pose): string {
  const thumb = "M1 100V58C1 64 -1.500 66.300 -4.500 62.500L-9.560 56.100A9.500 9.500 0 0 0 -24.400 67.900L-5.700 91.400C-3.600 94 -1 97 1 100Z";
  const cuff = "M14 132C13 142 9.500 151 5 159C22 170 65 170 82 159C77.500 151 74 142 73 132Z";
  const rest = thumb + box(0, 60, 87, 54, [0, 0, 24, 18]) + box(14, 104, 59, 22, 0) + cuff;
  const strap = box(8, 119, 71, 14, 4.5);
  const place = (i: number) => (pose ? ` transform="translate(0 ${pose === "point" ? DIGITS[i].curl : 0})"` : ` class="g-f" style="--c:${DIGITS[i].curl}px;--i:${i}"`);
  const digit = (d: (typeof DIGITS)[number]) => box(d.x, d.top, d.w, 64 - d.top, [d.w / 2, d.w / 2, 0, 0]);
  const lines = DIGITS.map((d, i) => `<path${place(i)} d="${digit(d)}" fill="${p.line}" stroke="${p.line}" stroke-width="${2 * LINE}" stroke-linejoin="round"/>`).join("");
  const fills = DIGITS.map((d, i) => `<path${place(i)} d="${digit(d)}" fill="${p.fill}"/>`).join("");
  const side = (name: string) => (pose ? "" : ` class="${name}"`);
  const perforations = [0, 1, 2, 3, 4].map((i) => dot(10, 15 + i * 8.5, 0.9 + i * 0.28)).join("");
  const knuckles = [10, 33, 56, 78].map((x, i) => dot(x, i ? 70 : 68, 3)).join("");
  const keyhole = box(31, 84, 25, 40, [12.5, 12.5, 0, 0]);
  const back = pose === "open" ? "" : `<path${side("g-back")} d="${knuckles}${keyhole}${perforations}" fill="${p.line}"/>`;
  const snap = pose === "open" ? "" : `<path${side("g-back")} d="${dot(69, 126, 2.6)}" fill="${p.fill}"/>`;
  const front = pose === "point" ? "" : `<path${side("g-front")} d="M11 79C30 68 60 69 79 78M5 86C24 84 35 96 34 116" stroke="${p.line}" stroke-width="2.2" stroke-linecap="round" fill="none"/>`;
  return (
    `<g${pose ? "" : ` class="g-flip"`}>` +
    keyline(rest, p) +
    lines +
    fills +
    `<path d="${rest}" fill="${p.fill}"/>` +
    `<path d="M21.500 57V63M44.500 57V63M67.500 57V63" stroke="${p.line}" stroke-width="${LINE}" stroke-linecap="round" fill="none"/>` +
    back +
    `<ellipse cx="43.5" cy="160" rx="34" ry="5.2" fill="${p.line}"/>` +
    keyline(strap, p) +
    `<path d="${strap}" fill="${p.accent}"/>` +
    snap +
    front +
    `</g>`
  );
}

const REACH = [18, 20.5, 19.5, 15.5];
const FAN = [-5, -1.5, 2, 6];
const OUT = 13;
const STACK = 4 * FINGER - 2 + 3 * LINE;

export const HOLD = { x: -20, y: -14, w: 46, h: STACK + 28 };

export function hold(p: GlovePaint): string {
  return REACH.map((reach, i) => {
    const h = i === 3 ? FINGER - 2 : FINGER;
    const y = i * (FINGER + LINE);
    const d = box(-OUT, y, reach + OUT, h, [4.5, h / 2, h / 2, 4.5]);
    const cy = y + h / 2;
    const seam = `M${f(reach - 5.5)} ${f(cy - 6)}Q${f(reach - 12.5)} ${f(cy)} ${f(reach - 5.5)} ${f(cy + 6)}`;
    return `<g class="ch-f" style="--i:${i}"><g transform="rotate(${FAN[i]} -70 ${f(STACK / 2)})">${keyline(d, p)}<path d="${d}" fill="${p.fill}"/><path d="${seam}" stroke="${p.line}" stroke-width="2" stroke-linecap="round" fill="none"/></g></g>`;
  }).join("");
}

export const CAP = {
  w: 120,
  h: 90,
  crown: "M31 46C22 42 12 35 9 27C7.500 22.500 9 20 13 18.500C26 13.500 42 2 60 2C78 2 94 13.500 107 18.500C111 20 112.500 22.500 111 27C108 35 98 42 89 46C70 50.500 50 50.500 31 46Z",
  band: "M30 45.500C50 50.500 70 50.500 90 45.500V60C70 65 50 65 30 60Z",
  visor: "M29 59.500C50 64.500 70 64.500 91 59.500C99 60 104.500 64.500 101.500 71C91 87 29 87 18.500 71C15.500 64.500 21 60 29 59.500Z",
  shine: "M27.500 71.500C33 76.500 43 79 55 79.300",
  strap: "M30 58C50 63 70 63 90 58",
  buttons: dot(31.5, 58.2, 2.5) + dot(88.5, 58.2, 2.5),
  badge: box(54, 20, 12, 17, 3.6),
  island: box(57.6, 22.6, 4.8, 1.9, 0.95),
};

type CapPaint = { ink: string; accent: string; page: string; visor?: string };

function cap(c: CapPaint, detail: boolean): string {
  const visor = c.visor ?? c.ink;
  const cut = c.visor ? c.ink : c.page;
  const base = `<path d="${CAP.crown}" fill="${c.ink}"/><path d="${CAP.visor}" fill="${visor}"${c.visor ? ` stroke="${c.ink}" stroke-width="1.6"` : ""}/><path d="${CAP.band}" fill="${c.accent}"/><path d="${CAP.badge}" fill="${c.accent}"/>`;
  if (!detail) return base;
  return `${base}<path d="${CAP.strap}" stroke="${visor}" stroke-width="3" fill="none"/><path d="${CAP.buttons}" fill="${cut}" stroke="${visor}" stroke-width="1.6"/><path d="${CAP.shine}" stroke="${cut}" stroke-width="2.4" stroke-linecap="round" fill="none"/><path d="${CAP.island}" fill="${c.visor ? c.page : c.ink}"/>`;
}

export type Cut = "full" | "text" | "micro";
export type MarkPaint = { ink: string; accent: string; page: string; band?: string; line?: string; ground?: string; visor?: string };

const FREE = { cap: 0.66, capY: 3, phone: { x: 48, y: 60, w: 44, h: 76, r: 10.5 } };
const TILE = { cap: 0.76, capY: 13, phone: { x: 43, y: 77, w: 54, h: 63, r: 13.5 } };
const SWITCH_Y = 27;
const TAP = { scale: 0.4, rotate: -38 };
const TILE_RECT = `<rect width="140" height="140" rx="31.5"`;

const capAt = (g: typeof FREE) => `translate(${f(70 - 60 * g.cap)} ${g.capY}) rotate(-7 ${f(60 * g.cap)} ${f(45 * g.cap)}) scale(${g.cap})`;
const phoneAt = (g: typeof FREE) => box(g.phone.x, g.phone.y, g.phone.w, g.phone.h, g === TILE ? [g.phone.r, g.phone.r, 0, 0] : g.phone.r);
const islandAt = (g: typeof FREE) => box(g.phone.x + g.phone.w * 0.34, g.phone.y + 5.2, g.phone.w * 0.32, 4.4, 2.2);

export function mark(cut: Cut, c: MarkPaint, size = 96): string {
  const g = c.ground ? TILE : FREE;
  const page = c.ground ?? c.page;
  const sx = g.phone.x + g.phone.w / 2;
  const sy = g.phone.y + SWITCH_Y;
  const glove = `<g transform="translate(${f(sx + 7)} ${f(sy + 5)}) rotate(${TAP.rotate}) scale(${TAP.scale}) translate(${-HAND.tipX} ${-HAND.tipY})">${hand({ line: c.line ?? c.ink, fill: "#fff", accent: c.accent }, "point")}</g>`;
  const tap =
    cut === "full"
      ? `<path d="${box(sx - 11, sy - 5.5, 22, 11, 5.5)}" fill="${c.accent}"/><path d="${dot(sx + 5.5, sy, 3.7)}" fill="#fff"/>${c.ground ? `<defs><clipPath id="pd-tile">${TILE_RECT}/></clipPath></defs><g clip-path="url(#pd-tile)">${glove}</g>` : glove}`
      : "";
  const fit = cut === "full" && !c.ground ? ` transform="translate(3 4) scale(.88)"` : "";
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 140 140" width="${size}" height="${size}" role="img" aria-label="phonedriver">${c.ground ? `${TILE_RECT} fill="${c.ground}"/>` : ""}<g${fit}><g transform="${capAt(g)}">${cap({ ink: c.ink, accent: c.band ?? c.accent, page, visor: c.visor }, cut !== "micro")}</g><path d="${phoneAt(g)}" fill="${c.ink}"/><path d="${islandAt(g)}" fill="${page}"/>${tap}</g></svg>`;
}

export const GLYPH = { cap: capAt(TILE), phone: phoneAt(TILE), island: islandAt(TILE) };

const INK = "#0b0b0c";
const PAPER = "#f3f2ed";
const ACCENT = "#ff5b14";

export const PAINT = {
  light: { ink: INK, accent: ACCENT, page: "#ffffff" },
  dark: { ink: PAPER, accent: ACCENT, page: INK, visor: "#1d1d20", line: INK },
  icon: { ink: INK, accent: ACCENT, band: PAPER, page: ACCENT, ground: ACCENT },
} satisfies Record<string, MarkPaint>;

export const LOCKUP = { top: -140, height: 180, tile: 160, tileY: -130, word: 208 };

export function lockupSvg(opts: { word: string; wordWidth: number; ink: string; height?: number }): string {
  const w = LOCKUP.word + opts.wordWidth + 4;
  const h = opts.height ?? 64;
  const tile = mark("text", PAINT.icon, LOCKUP.tile).replace("<svg ", `<svg y="${LOCKUP.tileY}" `);
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 ${LOCKUP.top} ${f(w)} ${LOCKUP.height}" width="${f((h * w) / LOCKUP.height)}" height="${h}" role="img" aria-label="phonedriver">${tile}<path transform="translate(${LOCKUP.word} 0)" fill="${opts.ink}" fill-rule="evenodd" d="${opts.word}"/></svg>`;
}

export const lockupWidth = (wordWidth: number, height: number) => (height * (LOCKUP.word + wordWidth + 4)) / LOCKUP.height;
