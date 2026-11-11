export const ORANGE = "#ff5b14";

type Pt = [number, number, number];
type Glyph = { w: number; c: Pt[][] };
export type Dot = { x: number; y: number; r: number; u: number; i?: number; j?: number };

const X = 100;
const D = 40;
const A = X + D;
const S = 24;
const H = 19;
const W = 80;
const K = 38;
const k = 13;
const G = 13;
const SQUIRCLE = 0.7;

export const METRICS = { X, D, A, S, W, top: -A, bottom: D, height: A + D };

const f = (v: number) => Math.round(v * 100) / 100;
const box = (x0: number, y0: number, x1: number, y1: number, r: number): Pt[] => [
  [x0, y0, r],
  [x1, y0, r],
  [x1, y1, r],
  [x0, y1, r],
];
const shift = (c: Pt[], dx: number, dy = 0): Pt[] => c.map(([x, y, r]) => [x + dx, y + dy, r]);
const turn = (c: Pt[]): Pt[] => c.map(([x, y, r]) => [W - x, -X - y, r]);

type Seg = { a: [number, number]; c0?: [number, number]; c1?: [number, number]; b: [number, number] };

function segments(pts: Pt[]): Seg[] {
  const n = pts.length;
  const out: Seg[] = [];
  let first: [number, number] | null = null;
  let pen: [number, number] | null = null;
  const lineTo = (p: [number, number]) => {
    if (pen) out.push({ a: pen, b: p });
    else first = p;
    pen = p;
  };
  pts.forEach(([x, y, r], i) => {
    if (!r) return lineTo([x, y]);
    const p = pts[(i - 1 + n) % n];
    const q = pts[(i + 1) % n];
    const lp = Math.hypot(p[0] - x, p[1] - y);
    const lq = Math.hypot(q[0] - x, q[1] - y);
    const rr = Math.min(r, lp / 2, lq / 2);
    const a: [number, number] = [x + ((p[0] - x) / lp) * rr, y + ((p[1] - y) / lp) * rr];
    const b: [number, number] = [x + ((q[0] - x) / lq) * rr, y + ((q[1] - y) / lq) * rr];
    lineTo(a);
    out.push({ a, c0: [a[0] + (x - a[0]) * SQUIRCLE, a[1] + (y - a[1]) * SQUIRCLE], c1: [b[0] + (x - b[0]) * SQUIRCLE, b[1] + (y - b[1]) * SQUIRCLE], b });
    pen = b;
  });
  if (pen && first) out.push({ a: pen, b: first });
  return out;
}

function contourPath(pts: Pt[]): string {
  const segs = segments(pts);
  let d = `M${f(segs[0].a[0])} ${f(segs[0].a[1])}`;
  for (const s of segs.slice(0, -1)) {
    d += s.c0 && s.c1 ? `C${f(s.c0[0])} ${f(s.c0[1])} ${f(s.c1[0])} ${f(s.c1[1])} ${f(s.b[0])} ${f(s.b[1])}` : `L${f(s.b[0])} ${f(s.b[1])}`;
  }
  return d + "Z";
}

function flatten(pts: Pt[]): [number, number][] {
  const out: [number, number][] = [];
  for (const s of segments(pts)) {
    if (!s.c0 || !s.c1) {
      out.push(s.a);
      continue;
    }
    for (let i = 0; i < 10; i++) {
      const t = i / 10;
      const m = 1 - t;
      out.push([
        m * m * m * s.a[0] + 3 * m * m * t * s.c0[0] + 3 * m * t * t * s.c1[0] + t * t * t * s.b[0],
        m * m * m * s.a[1] + 3 * m * m * t * s.c0[1] + 3 * m * t * t * s.c1[1] + t * t * t * s.b[1],
      ]);
    }
  }
  return out;
}

const eye = box(S, -X + H, W - S, -H, k);
const P: Pt[] = [
  [0, -X, 0],
  [W, -X, K],
  [W, 0, K],
  [S, 0, 0],
  [S, D, 0],
  [0, D, 0],
];
const R_W = 60;
const E_BAR = -42;

const GLYPHS: Record<string, Glyph> = {
  p: { w: W, c: [P, eye] },
  d: { w: W, c: [turn(P), eye] },
  o: { w: W, c: [box(0, -X, W, 0, K), eye] },
  n: { w: W, c: [[[0, 0, 0], [0, -X, 0], [W, -X, K], [W, 0, 0], [W - S, 0, 0], [W - S, -X + H, k], [S, -X + H, 0], [S, 0, 0]]] },
  h: { w: W, c: [[[0, 0, 0], [0, -A, 0], [S, -A, 0], [S, -X, 0], [W, -X, K], [W, 0, 0], [W - S, 0, 0], [W - S, -X + H, k], [S, -X + H, 0], [S, 0, 0]]] },
  r: { w: R_W, c: [[[0, 0, 0], [0, -X, 0], [R_W, -X, 30], [R_W, -X + 40, 0], [R_W - S, -X + 40, 0], [R_W - S, -X + H, 10], [S, -X + H, 0], [S, 0, 0]]] },
  i: { w: S, c: [box(0, -X, S, 0, 0)] },
  e: { w: W, c: [[[0, -X, K], [W, -X, K], [W, E_BAR, 0], [S, E_BAR, 0], [S, -H, k], [W - 1, -H, 0], [W - 1, 0, 0], [0, 0, K]], box(S, -X + H, W - S, E_BAR - H, 9)] },
  v: { w: W, c: [[[0, -X, 0], [S + 3, -X, 0], [W / 2, -28, 0], [W - S - 3, -X, 0], [W, -X, 0], [W / 2 + 14, 0, 3], [W / 2 - 14, 0, 3]]] },
};

const WORD = "phonedriver";

function layout() {
  let x = 0;
  const parts = [...WORD].map((ch, i) => {
    const part = { ch, x, g: GLYPHS[ch] };
    x += GLYPHS[ch].w + (i < WORD.length - 1 ? G : 0);
    return part;
  });
  return { width: x, parts };
}

const LAYOUT = layout();
export const WIDTH = LAYOUT.width;
const I_X = LAYOUT.parts.find((p) => p.ch === "i")!.x;
export const TITTLE = { x: I_X + S / 2, y: -A + 14, r: 14 };

function contours(): Pt[][] {
  return LAYOUT.parts.flatMap((p) => p.g.c.map((c) => shift(c, p.x)));
}

const tittlePath = (r = TITTLE.r) =>
  `M${f(TITTLE.x - r)} ${f(TITTLE.y)}a${r} ${r} 0 1 0 ${2 * r} 0a${r} ${r} 0 1 0 ${-2 * r} 0Z`;

export function wordPath(): string {
  return contours().map(contourPath).join("") + tittlePath();
}

function inside(polys: [number, number][][], x: number, y: number): boolean {
  let hit = false;
  for (const poly of polys) {
    for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
      const [xi, yi] = poly[i];
      const [xj, yj] = poly[j];
      if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) hit = !hit;
    }
  }
  return hit;
}

export function halftone(pitch: number, ramp: (u: number) => number): Dot[] {
  const glyphs = LAYOUT.parts.map((p) => ({
    x0: p.x,
    x1: p.x + p.g.w,
    polys: p.g.c.map((c) => flatten(shift(c, p.x))),
  }));
  glyphs.find((_, i) => LAYOUT.parts[i].ch === "i")!.polys.push(Array.from({ length: 24 }, (_, i) => [TITTLE.x + Math.cos((i / 24) * Math.PI * 2) * TITTLE.r, TITTLE.y + Math.sin((i / 24) * Math.PI * 2) * TITTLE.r] as [number, number]));
  const dots: Dot[] = [];
  const sub = 4;
  for (let j = 0, y = -A + pitch / 2; y < D; y += pitch, j++) {
    for (let i = 0, x = pitch / 2; x < WIDTH; x += pitch, i++) {
      const g = glyphs.find((g) => x + pitch / 2 > g.x0 && x - pitch / 2 < g.x1);
      if (!g) continue;
      let cover = 0;
      for (let l = 0; l < sub; l++)
        for (let m = 0; m < sub; m++) if (inside(g.polys, x - pitch / 2 + ((l + 0.5) * pitch) / sub, y - pitch / 2 + ((m + 0.5) * pitch) / sub)) cover++;
      if (!cover) continue;
      const u = x / WIDTH;
      const r = (pitch / 2) * ramp(u) * Math.sqrt(cover / (sub * sub));
      if (r > pitch * 0.06) dots.push({ x, y, r, u, i, j });
    }
  }
  return dots;
}

const smooth = (a: number, b: number, v: number) => {
  const t = Math.min(1, Math.max(0, (v - a) / (b - a)));
  return t * t * (3 - 2 * t);
};

export const DISPLAY_RAMP = (u: number) => 0.34 + 1.4 * smooth(0, 0.62, u);
const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
export const lit = (d: Dot) => (BAYER[((d.j ?? 0) % 4) * 4 + ((d.i ?? 0) % 4)] + 0.5) / 16 > smooth(0.03, 0.34, d.u);

const circle = (d: Dot, extra = "") => `<circle cx="${f(d.x)}" cy="${f(d.y)}" r="${f(d.r)}"${extra}/>`;

export function displaySvg(opts: { ink: string; accent?: string; pitch?: number; animate?: "inline" | "vars"; id?: string; transform?: string }): string {
  const pitch = opts.pitch ?? 10;
  const id = opts.id ?? "pd";
  const solid = wordPath();
  const dots = halftone(pitch, DISPLAY_RAMP)
    .map((d) => {
      const tint = opts.accent && lit(d) ? ` fill="${opts.accent}"` : "";
      const delay = Math.round(d.u * 900 + ((d.y + A) / (A + D)) * 140);
      return circle(d, tint + (opts.animate ? ` style="--d:${delay}ms"` : ""));
    })
    .join("");
  const css =
    opts.animate === "inline"
      ? `<style>#${id} circle{transform-box:fill-box;transform-origin:center;animation:${id}-in .7s cubic-bezier(.34,1.56,.64,1) var(--d) both}#${id}-sweep{animation:${id}-sw 1.1s cubic-bezier(.65,0,.35,1) .55s both}@keyframes ${id}-in{from{transform:scale(0)}}@keyframes ${id}-sw{from{transform:translateX(${f(WIDTH * 0.62)}px)}}@media (prefers-reduced-motion:reduce){#${id} circle,#${id}-sweep{animation:none}}</style>`
      : "";
  const band = `x="${f(-WIDTH)}" y="${-A - 20}" width="${f(WIDTH * 3)}" height="${A + D + 40}"`;
  return `${css}<g id="${id}"${opts.transform ? ` transform="${opts.transform}"` : ""}><defs><linearGradient id="${id}-g" gradientUnits="userSpaceOnUse" x1="0" x2="${f(WIDTH)}" y1="0" y2="0"><stop offset=".5" stop-color="#fff" stop-opacity="0"/><stop offset=".76" stop-color="#fff"/></linearGradient><mask id="${id}-m" maskUnits="userSpaceOnUse" ${band}><rect id="${id}-sweep" class="sweep" ${band} fill="url(#${id}-g)"/></mask><clipPath id="${id}-c"><path clip-rule="evenodd" d="${solid}"/></clipPath></defs><g fill="${opts.ink}"><g clip-path="url(#${id}-c)" class="dots">${dots}</g><path mask="url(#${id}-m)" fill-rule="evenodd" d="${solid}"/></g></g>`;
}
