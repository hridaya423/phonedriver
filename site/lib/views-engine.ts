import { onFrame } from "./frame";

const START = 260;
const SCAN = 2400;
const HOLD = 520;
const CLEAR = 520;
const FLIGHT = 520;
const CELL = 6;
const SHED = 56;
const GLYPHS = [".", ":", "-", "+", "/", "1", "0"];

const clamp = (v: number) => Math.min(1, Math.max(0, v));
const easeInOut = (k: number) => (k < 0.5 ? 2 * k * k : 1 - Math.pow(-2 * k + 2, 2) / 2);

type Dot = { x0: number; y0: number; x1: number; y1: number; cx: number; cy: number; t0: number; dur: number; r: number };

/** The read: a scan turns the screenshot into glyphs, each element sheds dots that land as its line. Also the hover link between the two. */
export function mountViews(el: HTMLElement): () => void {
  const lines = el.querySelector<HTMLElement>(".tv-lines")!;
  const shot = el.querySelector<HTMLElement>(".tv-shot")!;
  const screen = el.querySelector<HTMLElement>(".tv-screen")!;
  const img = screen.querySelector("img")!;
  const fx = el.querySelector<HTMLCanvasElement>(".tv-fx")!;
  const pane = el.querySelector<HTMLElement>(".pane")!;
  const note = pane.querySelector<HTMLElement>(".pane-head span:last-child")!;
  const noted = note.textContent!;
  const path = el.querySelector<SVGPathElement>(".tv-link path")!;
  const rows = [...lines.children] as HTMLElement[];
  const count = lines.querySelectorAll("[data-i]").length;
  const rest = Number(el.dataset.on);
  let on = rest;
  let timers: number[] = [];
  let off = () => {};

  const draw = () => {
    const box = shot.querySelector<HTMLElement>(`[data-i="${on}"]`)!.getBoundingClientRect();
    const line = lines.querySelector<HTMLElement>(`[data-i="${on}"]`)!.getBoundingClientRect();
    const o = el.getBoundingClientRect();
    if (line.left < box.right + 24) return path.removeAttribute("d");
    const x1 = box.right - o.left;
    const y1 = Math.min(Math.max(line.top + line.height / 2, box.top + 10), box.bottom - 10) - o.top;
    const x2 = line.left - o.left - 10;
    const y2 = line.top + line.height / 2 - o.top;
    const bend = (x2 - x1) * 0.5;
    path.setAttribute("d", `M${x1} ${y1}C${x1 + bend} ${y1} ${x2 - bend} ${y2} ${x2} ${y2}`);
  };
  const set = (i: number) => {
    on = i;
    for (const n of el.querySelectorAll<HTMLElement>("[data-i]")) n.toggleAttribute("data-on", Number(n.dataset.i) === i);
    draw();
  };
  const stop = () => {
    for (const t of timers) clearTimeout(t);
    timers = [];
    off();
    off = () => {};
    fx.width = 0;
    delete el.dataset.run;
    note.textContent = noted;
    for (const r of rows) delete r.dataset.in;
  };
  const show = (r: HTMLElement) => {
    r.dataset.in = "";
    note.textContent = `${rows.filter((x) => "in" in x.dataset).length} lines`;
  };

  function effects() {
    const o = el.getBoundingClientRect(), s = screen.getBoundingClientRect();
    const dpr = Math.min(devicePixelRatio || 1, 2);
    const sx = s.left - o.left, sy = s.top - o.top, sw = s.width, sh = s.height;
    const cols = Math.round(sw / CELL), rws = Math.round(sh / CELL);
    const cw = sw / cols, ch = sh / rws;
    const tiny = document.createElement("canvas");
    tiny.width = cols;
    tiny.height = rws;
    const tc = tiny.getContext("2d", { willReadFrequently: true })!;
    tc.drawImage(img, 0, 0, cols, rws);
    const px = tc.getImageData(0, 0, cols, rws).data;
    const cs = getComputedStyle(pane);
    const fg = cs.color, orange = cs.getPropertyValue("--or").trim() || "#ff5b14";
    const glyphs = document.createElement("canvas");
    glyphs.width = Math.round(sw * dpr);
    glyphs.height = Math.round(sh * dpr);
    const gc = glyphs.getContext("2d")!;
    gc.scale(dpr, dpr);
    gc.fillStyle = cs.backgroundColor;
    gc.fillRect(0, 0, sw, sh);
    gc.font = `700 ${CELL * 1.35}px ${cs.fontFamily}`;
    gc.textAlign = "center";
    gc.textBaseline = "middle";
    const dens = new Float32Array(cols * rws);
    for (let r = 0; r < rws; r++) {
      for (let c = 0; c < cols; c++) {
        const i = (r * cols + c) * 4;
        const R = px[i] / 255, G = px[i + 1] / 255, B = px[i + 2] / 255;
        const max = Math.max(R, G, B), vivid = max > 0 && (max - Math.min(R, G, B)) / max > 0.38;
        const d = Math.max(vivid ? 0.6 : 0, 1 - (0.2126 * R + 0.7152 * G + 0.0722 * B));
        dens[r * cols + c] = d;
        if (d < 0.16) {
          if (Math.min(R, G, B) < 0.985) continue;
          gc.globalAlpha = 0.2;
          gc.fillStyle = fg;
          gc.fillText(".", (c + 0.5) * cw, (r + 0.5) * ch);
          continue;
        }
        gc.globalAlpha = 0.5 + 0.5 * d;
        gc.fillStyle = vivid ? orange : fg;
        gc.fillText(GLYPHS[Math.min(6, Math.floor(d * 7))], (c + 0.5) * cw, (r + 0.5) * ch);
      }
    }

    const dots: Dot[] = [];
    const side = lines.getBoundingClientRect().left > s.right;
    for (const r of rows.filter((x) => x.dataset.i)) {
      const box = shot.querySelector<HTMLElement>(`.tv-box[data-i="${r.dataset.i}"]`)!;
      const top = parseFloat(box.style.top) / 100, left = parseFloat(box.style.left) / 100;
      const wide = parseFloat(box.style.width) / 100, tall = Math.min(parseFloat(box.style.height) / 100, 0.085);
      const c0 = Math.floor(left * cols), c1 = Math.ceil((left + wide) * cols), r0 = Math.floor(top * rws), r1 = Math.ceil((top + tall) * rws);
      const marks: [number, number][] = [];
      for (let y = r0; y < r1; y++) for (let x = c0; x < c1; x++) if (dens[y * cols + x] > 0.3) marks.push([x, y]);
      const to = r.getBoundingClientRect();
      const t0 = START + top * SCAN;
      for (let k = 0; k < SHED; k++) {
        const [c, y] = marks.length ? marks[Math.floor(Math.random() * marks.length)] : [c0 + Math.random() * (c1 - c0), r0 + Math.random() * (r1 - r0)];
        const x0 = sx + (c + 0.5) * cw, y0 = sy + (y + 0.5) * ch;
        const x1 = to.left - o.left + 14 + Math.random() * Math.min(to.width * 0.5, 280), y1 = to.top - o.top + to.height / 2 + (Math.random() - 0.5) * 5;
        dots.push({
          x0,
          y0,
          x1,
          y1,
          cx: (x0 + x1) / 2 + (Math.random() - 0.5) * (side ? 60 : 140),
          cy: side ? Math.min(y0, y1) - 24 - Math.random() * 60 : (y0 + y1) / 2,
          t0: t0 + Math.random() * 300,
          dur: FLIGHT + Math.random() * 280,
          r: 1.1 + Math.random() * 1.1,
        });
      }
    }

    fx.width = Math.round(o.width * dpr);
    fx.height = Math.round(o.height * dpr);
    fx.style.width = `${o.width}px`;
    fx.style.height = `${o.height}px`;
    const ctx = fx.getContext("2d")!;
    ctx.scale(dpr, dpr);
    const began = performance.now();
    off = onFrame((now) => {
      const t = now - began;
      const k = clamp((t - START) / SCAN), gone = clamp((t - START - SCAN - HOLD) / CLEAR);
      const y = k * sh, y0 = gone * sh;
      ctx.clearRect(0, 0, o.width, o.height);
      if (y > y0) ctx.drawImage(glyphs, 0, y0 * dpr, sw * dpr, (y - y0) * dpr, sx, sy + y0, sw, y - y0);
      ctx.fillStyle = orange;
      if (k > 0 && k < 1) ctx.fillRect(sx - 10, sy + y - 1, sw + 20, 2);
      if (gone > 0 && gone < 1) ctx.fillRect(sx - 10, sy + y0 - 1, sw + 20, 2);
      for (const d of dots) {
        const u = (t - d.t0) / d.dur;
        if (u <= 0 || u >= 1) continue;
        const e = easeInOut(u), m = 1 - e;
        ctx.globalAlpha = u < 0.12 ? u / 0.12 : u > 0.86 ? (1 - u) / 0.14 : 1;
        ctx.beginPath();
        ctx.arc(m * m * d.x0 + 2 * m * e * d.cx + e * e * d.x1, m * m * d.y0 + 2 * m * e * d.cy + e * e * d.y1, d.r * (1.5 - 0.5 * e), 0, 7);
        ctx.fill();
      }
      ctx.globalAlpha = 1;
    });
  }

  const read = () => {
    el.dataset.run = "";
    note.textContent = "0 lines";
    const head = rows.filter((r) => !r.dataset.i);
    head.forEach((r, k) => timers.push(window.setTimeout(() => show(r), k * 110)));
    for (const r of rows.filter((x) => x.dataset.i)) {
      const box = shot.querySelector<HTMLElement>(`.tv-box[data-i="${r.dataset.i}"]`)!;
      timers.push(window.setTimeout(() => show(r), START + (parseFloat(box.style.top) / 100) * SCAN + FLIGHT));
    }
    timers.push(
      window.setTimeout(
        () => {
          stop();
          set(rest);
        },
        START + SCAN + HOLD + CLEAR + 80,
      ),
    );
    img.decode().then(() => "run" in el.dataset && effects(), () => undefined);
  };
  const pick = (e: Event) => {
    const hit = (e.target as HTMLElement).closest<HTMLElement>("[data-i]");
    if (!hit || "run" in el.dataset) return;
    set(Number(hit.dataset.i));
  };
  const onKey = (e: KeyboardEvent) => {
    const step = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    stop();
    set(Math.min(count, Math.max(1, on + step)));
  };

  el.addEventListener("pointerover", pick);
  el.addEventListener("click", pick);
  el.addEventListener("life", read);
  lines.addEventListener("keydown", onKey);
  const ro = new ResizeObserver(draw);
  ro.observe(el);
  el.dataset.live = "";
  draw();
  return () => {
    stop();
    ro.disconnect();
    el.removeEventListener("pointerover", pick);
    el.removeEventListener("click", pick);
    el.removeEventListener("life", read);
    lines.removeEventListener("keydown", onKey);
  };
}
