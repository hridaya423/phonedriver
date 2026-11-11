import { onFrame, scrolled } from "./frame";
import { createWeave, rgb, type Band, type Pt, type Ribbon } from "./weave";

const BAND = 1.3;
const RIBBONS: Ribbon[][] = [
  [
    { w: 0.17, s: 26, pts: [[-0.14, 0.5], [0.08, 0.4], [0.3, 0.24], [0.55, 0.14], [0.8, 0.2], [1.14, 0.04]] },
    { w: 0.07, s: 12, pts: [[-0.14, 0.14], [0.2, 0.26], [0.5, 0.4], [0.8, 0.34], [1.14, 0.46]] },
  ],
  [
    { w: 0.17, s: 26, pts: [[-0.14, 0.08], [0.12, 0.14], [0.36, 0.28], [0.6, 0.42], [0.84, 0.38], [1.14, 0.5]] },
    { w: 0.07, s: 12, pts: [[0.3, -0.24], [0.42, 0.08], [0.6, 0.2], [0.86, 0.14], [1.14, 0.22]] },
  ],
  [
    { w: 0.17, s: 26, pts: [[-0.14, 0.32], [0.14, 0.16], [0.4, 0.1], [0.64, 0.24], [0.86, 0.44], [1.14, 0.38]] },
    { w: 0.07, s: 12, pts: [[-0.14, 0.5], [0.2, 0.44], [0.46, 0.46], [0.7, 0.5], [1.14, 0.26]] },
  ],
];
const LEAD = 0.1;
const TYPED = 0.78;
const FALL = 40;

const clamp = (v: number) => Math.min(1, Math.max(0, v));
const easeInOut = (k: number) => (k < 0.5 ? 4 * k * k * k : 1 - Math.pow(-2 * k + 2, 3) / 2);

type Stop = {
  el: HTMLElement;
  svg: SVGSVGElement;
  path: SVGPathElement;
  text: SVGTextElement;
  tp: SVGTextPathElement;
  reveal: SVGUseElement;
  head: SVGPathElement;
  packet: SVGCircleElement;
  land: HTMLElement;
  lifes: HTMLElement[];
  say: string;
  short: string;
  line: string;
  widths: number[];
  fall: number[];
  total: number;
  start: number;
  end: number;
  air: number;
  top: number;
  key: string;
  shown: number;
  painted: number;
  chars: number;
  landed: boolean;
  near: boolean;
  intro: boolean;
  ink: [number, number, number];
  clear: number[];
};

/** Runs the route down the page: each stop's line is typed by scroll, lands on its answer and starts it. */
export function mountJourney(root: HTMLElement): () => void {
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const dock = root.querySelector<HTMLElement>(".dock");
  const hero = root.querySelector<HTMLElement>(".hx");
  const canvas = root.querySelector<HTMLCanvasElement>(".weave");
  const marks = dock ? [...dock.querySelectorAll<HTMLAnchorElement>(".dock-map a")] : [];
  const stops: Stop[] = [...root.querySelectorAll<HTMLElement>(".stop")].map((el) => {
    const svg = el.querySelector<SVGSVGElement>(".rt")!;
    return {
      el,
      svg,
      path: svg.querySelector<SVGPathElement>("defs path")!,
      text: svg.querySelector<SVGTextElement>("defs text")!,
      tp: svg.querySelector<SVGTextPathElement>("textPath")!,
      reveal: svg.querySelector<SVGUseElement>(".rt-reveal")!,
      head: svg.querySelector<SVGPathElement>(".rt-head")!,
      packet: svg.querySelector<SVGCircleElement>(".rt-packet")!,
      land: el.querySelector<HTMLElement>("[data-land]")!,
      lifes: [...el.querySelectorAll<HTMLElement>("[data-life]")],
      say: svg.dataset.say!,
      short: svg.dataset.short ?? svg.dataset.say!,
      line: "",
      widths: [],
      fall: [],
      total: 0,
      start: 0,
      end: 0,
      air: 0,
      top: 0,
      key: "",
      shown: 0,
      painted: -1,
      chars: -1,
      landed: false,
      near: false,
      intro: "intro" in el.dataset,
      ink: [0, 0, 0],
      clear: [0, 0, 0, 0, 0, 0],
    };
  });
  const family = getComputedStyle(root).getPropertyValue("--jb") || "monospace";
  const weave = canvas ? createWeave(canvas, family, getComputedStyle(root).getPropertyValue("--or").trim() || "#ff5b14") : null;
  const mouse = { x: -9999, y: -9999, z: 0, tz: 0 };
  let W = 0, vh = 0, last = 0, at = -1, drawn = false, born = 0, lastY = -1, lastDraw = 0;

  function geometry(s: Stop) {
    const box = s.el.getBoundingClientRect(), t = s.land.getBoundingClientRect();
    const wide = W >= 1000;
    const air = (s.air = parseFloat(getComputedStyle(s.el).paddingTop));
    const frac = Number(s.land.dataset.at);
    const E = { x: t.left - box.left + (frac ? t.width * frac : Math.min(54, t.width * 0.2)), y: t.top - box.top - 9 };
    let d: string, lead: string;
    if (wide) {
      const a = Math.min(air, E.y);
      const y0 = a * 0.86;
      const J = { x: E.x * 0.55, y: a * 0.34 };
      const drop = Math.min(E.y, a);
      lead = `M-24,${y0} L${W * 0.012},${y0 - a * 0.05}`;
      d = `${lead} C${E.x * 0.14},${a * 0.52} ${E.x * 0.36},${a * 0.28} ${J.x},${J.y} C${J.x + E.x * 0.19},${J.y + a * 0.06} ${E.x},${a * 0.14} ${E.x},${drop}`;
      if (E.y > drop) d += ` L${E.x},${E.y}`;
    } else {
      E.x = t.right - box.left - Math.min(34, t.width * 0.1);
      const y = air * 0.62;
      lead = `M-24,${y + 12} L6,${y + 5}`;
      const x = W - 7, turn = Math.min(E.y - 12, y + 96);
      d = `${lead} C${W * 0.36},${y - 24} ${W * 0.7},${y - 24} ${W * 0.84},${y + 8} C${x + 4},${y + 34} ${x},${y + 60} ${x},${turn}`;
      d += E.y - turn > 60 ? ` L${x},${E.y - 44} C${x},${E.y - 16} ${E.x + 14},${E.y - 12} ${E.x},${E.y}` : ` L${E.x},${E.y}`;
    }
    s.svg.setAttribute("width", String(W));
    s.svg.setAttribute("height", String(E.y + 24));
    s.svg.setAttribute("viewBox", `0 0 ${W} ${E.y + 24}`);
    s.path.setAttribute("d", d);
    const m = document.createElementNS("http://www.w3.org/2000/svg", "path");
    m.setAttribute("d", lead);
    s.start = m.getTotalLength() + 12;
    s.tp.setAttribute("startOffset", String(s.start));
    s.total = s.path.getTotalLength();
    const room = wide ? E.x * 0.92 : W * 0.8;
    s.tp.textContent = s.say;
    s.line = s.start + s.text.getComputedTextLength() + 24 < room ? s.say : s.short;
    s.tp.textContent = s.line;
    s.widths = [0];
    for (let i = 1; i <= s.line.length; i++) s.widths.push(s.text.getSubStringLength(0, i));
    const said = s.widths[s.line.length];
    s.clear = [s.start - 6, s.start + said / 2, s.start + said + 6].flatMap((l) => {
      const at = s.path.getPointAtLength(l);
      return [at.x, at.y - 5];
    });
    const from = s.start + said + 4;
    s.fall = [];
    for (let i = 0; i <= FALL; i++) {
      const at = s.path.getPointAtLength(from + ((s.total - from) * i) / FALL);
      s.fall.push(at.x, at.y);
    }
    s.reveal.style.strokeDasharray = `${s.total} ${s.total + 40}`;
    const end = s.path.getPointAtLength(s.total), pre = s.path.getPointAtLength(s.total - 2);
    s.head.setAttribute("transform", `translate(${end.x} ${end.y}) rotate(${(Math.atan2(end.y - pre.y, end.x - pre.x) * 180) / Math.PI})`);
    s.end = E.y;
    s.ink = rgb(getComputedStyle(s.el).color);
    s.chars = -1;
    s.painted = -1;
    paint(s);
  }

  function paint(s: Stop) {
    const k = s.shown;
    if (k === s.painted) return;
    s.painted = k;
    let L: number, chars = s.line.length, fell = 0;
    if (k < LEAD) {
      L = (k / LEAD) * s.start;
      chars = 0;
    } else if (k < TYPED) {
      chars = Math.round(((k - LEAD) / (TYPED - LEAD)) * s.line.length);
      L = s.start + s.widths[chars] + 4;
    } else {
      const from = s.start + s.widths[s.line.length] + 4;
      fell = easeInOut((k - TYPED) / (1 - TYPED));
      L = from + (s.total - from) * fell;
    }
    // Padded to full width: Chrome reports a new largest contentful paint each time a typed line grows.
    if (chars !== s.chars) s.tp.textContent = s.line.slice(0, (s.chars = chars)) + "\u00a0".repeat(s.line.length - chars);
    s.reveal.style.strokeDashoffset = `${s.total - Math.min(L, s.total) + 0.01}`;
    const falling = k >= TYPED && k < 1;
    s.packet.style.opacity = falling ? "1" : "0";
    if (falling) {
      const i = Math.min(FALL - 1, Math.floor(fell * FALL)) * 2, m = fell * FALL - i / 2;
      s.packet.setAttribute("cx", String(s.fall[i] + (s.fall[i + 2] - s.fall[i]) * m));
      s.packet.setAttribute("cy", String(s.fall[i + 1] + (s.fall[i + 3] - s.fall[i + 1]) * m));
    }
    s.head.style.opacity = k >= 1 ? "1" : "0";
  }

  function arrive(s: Stop, quiet: boolean) {
    s.landed = true;
    s.shown = 1;
    paint(s);
    s.el.dataset.landed = "";
    for (const el of s.lifes) {
      const waited = "wait" in el.dataset;
      delete el.dataset.wait;
      if (quiet || !waited) continue;
      el.dataset.enter = "";
      el.dispatchEvent(new CustomEvent("life"));
    }
  }

  const want = (s: Stop, top: number) => {
    const from = vh * 0.9, to = Math.max(vh * 0.6 - s.end, vh * 0.14);
    return clamp((from - top) / (from - to));
  };

  function layout(force = false) {
    const w = document.documentElement.clientWidth;
    if (w !== W || innerHeight !== vh) {
      W = w;
      vh = innerHeight;
      weave?.size(W, vh);
    }
    const y = scrollY;
    const boxes = stops.map((s) => [s.el.getBoundingClientRect(), s.land.getBoundingClientRect()]);
    stops.forEach((s, i) => {
      const [box, t] = boxes[i];
      s.top = box.top + y;
      const key = [W, t.left - box.left, t.top - box.top, t.width].map(Math.round).join(" ");
      if (key === s.key && !force) return;
      s.key = key;
      geometry(s);
      const ribbons = RIBBONS[i % RIBBONS.length];
      const thin = s.el.dataset.weave === "thin" ? ribbons.slice(1) : ribbons;
      weave?.build(i, W < 700 ? ribbons.slice(1).map((r) => ({ ...r, pts: r.pts.map(([x, y]) => [x, y * 0.3] as Pt) })) : thin, W, s.air * BAND);
    });
  }

  function tick(now: number) {
    const dt = Math.min(0.05, (now - last) / 1000);
    last = now;
    born ||= now;
    let moving = false;
    const bands: (Band | null)[] = [];
    let here = -1;
    const y = scrolled();
    stops.forEach((s, n) => {
      const top = s.top - y;
      if (top < vh * 0.55) here = n;
      if (!s.near) {
        bands.push(null);
        return;
      }
      if (!s.landed) {
        const target = s.intro ? clamp((now - born - 100) / 1100) : want(s, top);
        const eased = s.shown + (target - s.shown) * (1 - Math.exp(-dt * 9));
        s.shown = Math.min(target, Math.max(s.shown, Math.min(eased, s.shown + dt * 1.5)));
        if (target - s.shown < 0.002) s.shown = target;
        paint(s);
        if (s.shown >= 1) arrive(s, false);
      }
      bands.push({ x: 0, y: top, h: s.air * BAND, enter: s.shown, twist: reduce ? 0 : -top / vh, ink: s.ink, clear: s.clear });
      if (top - s.air * 0.4 < vh && top + s.air * BAND > 0) moving = true;
    });
    const still = y === lastY && now - lastDraw < 15;
    lastY = y;
    if (weave && (moving || drawn) && !still) {
      lastDraw = now;
      mouse.z += (mouse.tz - mouse.z) * 0.06;
      weave.draw(moving ? bands : [], reduce ? 3 : now / 1000, [mouse.x, mouse.y, reduce ? 0 : mouse.z]);
      drawn = moving;
    }
    if (here !== at) {
      at = here;
      marks.forEach((a, n) => {
        a.toggleAttribute("data-past", n < here);
        if (n === here) a.setAttribute("aria-current", "true");
        else a.removeAttribute("aria-current");
      });
    }
  }

  const cleanups: (() => void)[] = [];
  const io = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        const s = stops.find((x) => x.el === e.target)!;
        s.near = e.isIntersecting;
        s.el.toggleAttribute("data-near", s.near);
      }
    },
    { rootMargin: "20% 0px 20% 0px" },
  );
  if (hero && dock) {
    const seen = new IntersectionObserver(([e]) => {
      dock.toggleAttribute("data-on", !e.isIntersecting);
      dock.inert = e.isIntersecting;
    });
    seen.observe(hero);
    cleanups.push(() => seen.disconnect());
  }

  for (const s of stops) {
    s.el.dataset.live = "";
    io.observe(s.el);
  }
  layout();
  for (const s of stops) {
    if (reduce || (!s.intro && want(s, s.top - scrolled()) >= 1)) arrive(s, true);
    else for (const el of s.lifes) el.dataset.wait = "";
  }

  let rt = 0;
  const ro = new ResizeObserver(() => {
    cancelAnimationFrame(rt);
    rt = requestAnimationFrame(() => layout());
  });
  ro.observe(root);
  const onMove = (e: PointerEvent) => {
    mouse.x = e.clientX;
    mouse.y = e.clientY;
    mouse.tz = e.pointerType === "mouse" ? 1 : 0;
  };
  const onLeave = () => (mouse.tz = 0);
  window.addEventListener("pointermove", onMove, { passive: true });
  document.addEventListener("pointerleave", onLeave);
  const scheme = matchMedia("(prefers-color-scheme: dark)");
  const recolor = () => layout(true);
  scheme.addEventListener("change", recolor);
  const off = onFrame(tick);

  return () => {
    off();
    io.disconnect();
    ro.disconnect();
    cancelAnimationFrame(rt);
    cleanups.forEach((f) => f());
    window.removeEventListener("pointermove", onMove);
    document.removeEventListener("pointerleave", onLeave);
    scheme.removeEventListener("change", recolor);
    for (const s of stops) {
      delete s.el.dataset.live;
      delete s.el.dataset.near;
      delete s.el.dataset.landed;
      for (const el of s.lifes) {
        delete el.dataset.wait;
        delete el.dataset.enter;
      }
    }
  };
}
