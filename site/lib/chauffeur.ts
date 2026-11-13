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

const hole = (x: number, y: number, w: number, h: number, r: number) =>
  `M${f(x)} ${f(y + r)}V${f(y + h - r)}A${r} ${r} 0 0 0 ${f(x + r)} ${f(y + h)}H${f(x + w - r)}A${r} ${r} 0 0 0 ${f(x + w)} ${f(y + h - r)}V${f(y + r)}A${r} ${r} 0 0 0 ${f(x + w - r)} ${f(y)}H${f(x + r)}A${r} ${r} 0 0 0 ${f(x)} ${f(y + r)}Z`;

/** The cap from the front, as the figure in the hero wears it: a wide crown, a band, a peak, and a phone for a badge. */
export const CAP = {
  w: 120,
  h: 90,
  crown: "M28 54C20 47 7 37 4 28C2.500 23 5 20 9 18.500C24 13 42 3 60 3C78 3 96 13 111 18.500C115 20 117.500 23 116 28C113 37 100 47 92 54C71 58.500 49 58.500 28 54Z",
  band: "M28 53.500C49 58 71 58 92 53.500V66C71 70.500 49 70.500 28 66Z",
  visor: "M26 65.500C49 70 71 70 94 65.500C103 66.500 109 71 105 77C93 89 27 89 15 77C11 71 17 66.500 26 65.500Z",
  seam: "M9.500 27.500C28 37 92 37 110.500 27.500",
  plate: box(50.5, 15.5, 19, 31, 6),
  badge: box(52, 17, 16, 28, 5.2) + hole(55.2, 20.2, 9.6, 21.6, 2) + dot(60, 38.6, 1.5),
};

type CapPaint = { ink: string; band: string; badge: string; plate?: string; seam?: string };

function cap(c: CapPaint, detail: boolean): string {
  const base = `<path d="${CAP.crown}${CAP.visor}" fill="${c.ink}"/><path d="${CAP.band}" fill="${c.band}"/>`;
  if (!detail) return base;
  return `${base}${c.seam ? `<path d="${CAP.seam}" fill="none" stroke="${c.seam}" stroke-width="1.6" stroke-linecap="round"/>` : ""}${c.plate ? `<path d="${CAP.plate}" fill="${c.plate}"/>` : ""}<path d="${CAP.badge}" fill="${c.badge}"/>`;
}

/** `full` is the mark with its details, `text` the one that sits beside the wordmark, `micro` the one for 32 px and under: the cap and the phone, nothing else. */
export type Cut = "full" | "text" | "micro";
/** With `ground` the mark is an app tile and the phone is a solid `phone` colour; without it the phone is an outline in the accent. */
export type MarkPaint = { ink: string; accent: string; page: string; band?: string; badge?: string; plate?: string; seam?: string; ground?: string; phone?: string };

const CAP_AT = { scale: 0.86, y: 9, roll: -7 };
const BODY = { x: 41, y: 76, w: 58, r: 14 };
const TILE_RECT = `<rect width="140" height="140" rx="31.5"`;

const capAt = () => `translate(${f(70 - 60 * CAP_AT.scale)} ${CAP_AT.y}) rotate(${CAP_AT.roll} ${f(60 * CAP_AT.scale)} ${f(45 * CAP_AT.scale)}) scale(${CAP_AT.scale})`;
const body = box(BODY.x, BODY.y, BODY.w, 140 - BODY.y, [BODY.r, BODY.r, 0, 0]);
const island = box(60, 88, 20, 5.6, 2.8);

export function mark(cut: Cut, c: MarkPaint, size = 96): string {
  const detail = cut !== "micro";
  const hat = `<g transform="${capAt()}">${cap({ ink: c.ink, band: c.band ?? c.accent, badge: c.badge ?? c.band ?? c.accent, plate: c.plate, seam: cut === "full" ? c.seam : undefined }, detail)}</g>`;
  const phone = c.ground
    ? `<path d="${body}" fill="${c.phone ?? c.page}"/>${detail ? `<path d="${island}" fill="${c.ink}"/>` : ""}`
    : `<path d="${box(BODY.x + 3.5, BODY.y + 3.5, BODY.w - 7, 58, BODY.r - 2)}" fill="none" stroke="${c.accent}" stroke-width="7"/>${detail ? `<path d="${dot(70, 126.5, 2.6)}" fill="${c.accent}"/>` : ""}`;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 140 140" width="${size}" height="${size}" role="img" aria-label="phonedriver">${c.ground ? `${TILE_RECT} fill="${c.ground}"/>` : ""}${phone}${hat}</svg>`;
}

export const GLYPH = { cap: capAt(), phone: body, island };

const INK = "#0b0b0c";
const PAPER = "#f3f2ed";
const ACCENT = "#ff5b14";

export const PAINT = {
  light: { ink: INK, accent: ACCENT, page: "#ffffff", badge: "#ffffff" },
  dark: { ink: PAPER, accent: ACCENT, page: INK, plate: INK, seam: INK },
  icon: { ink: INK, accent: ACCENT, band: PAPER, page: ACCENT, ground: ACCENT, phone: PAPER, seam: ACCENT },
} satisfies Record<string, MarkPaint>;

export const LOCKUP = { top: -140, height: 180, tile: 160, tileY: -130, word: 208 };

export function lockupSvg(opts: { word: string; wordWidth: number; ink: string; height?: number }): string {
  const w = LOCKUP.word + opts.wordWidth + 4;
  const h = opts.height ?? 64;
  const tile = mark("text", PAINT.icon, LOCKUP.tile).replace("<svg ", `<svg y="${LOCKUP.tileY}" `);
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 ${LOCKUP.top} ${f(w)} ${LOCKUP.height}" width="${f((h * w) / LOCKUP.height)}" height="${h}" role="img" aria-label="phonedriver">${tile}<path transform="translate(${LOCKUP.word} 0)" fill="${opts.ink}" fill-rule="evenodd" d="${opts.word}"/></svg>`;
}

export const lockupWidth = (wordWidth: number, height: number) => (height * (LOCKUP.word + wordWidth + 4)) / LOCKUP.height;
