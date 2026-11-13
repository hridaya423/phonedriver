import { mountFigure, type CapPose, type Figure, type HandPose, type Scene } from "./chauffeur-gl";
import { onFrame } from "./frame";
import { rgb } from "./weave";

type Shot = { fig: Figure; w: number; h: number; now: number };
type Prop = { size(w: number, h: number): { hand: number; cap?: number; cell?: number }; scene(s: Shot): Scene; rate?: number };

const curl = (thumb: number[], ...f: number[][]) => [...thumb, ...[0, 1, 2, 3].flatMap((i) => f[Math.min(i, f.length - 1)])];
const POINT = curl([0.5, 0, 0, 0.4, 0.4], [0, 0.05, 0.1, 0.05], [0, 1.4, 1.5, 0.9]);
const PRESS = curl([0.5, 0, 0, 0.4, 0.4], [0, 0.16, 0.2, 0.1], [0, 1.4, 1.5, 0.9]);
const CRADLE = curl([0.3, 0.4, 0, 0.2, 0.2], [0, 0.4, 0.8, 0.5]);

/** A figure on its own canvas below the hero. It is built when the canvas first comes near the window, drawn only while it is on screen, and drawn once and held for reduced motion. The canvas takes its ink from `color`, its ground from `--page` and its accent from `--or`. */
function mountProp(canvas: HTMLCanvasElement, prop: Prop): { fig(): Figure | null; fit(): void; stop(): void } {
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  let fig: Figure | null = null, off = () => {}, dead = false, asked = false, w = 0, h = 0;

  const fit = () => {
    const r = canvas.getBoundingClientRect();
    if (!fig || !r.width || !r.height) return;
    w = r.width;
    h = r.height;
    const cs = getComputedStyle(canvas);
    const page = rgb(cs.getPropertyValue("--page").trim() || "#141412");
    const size = prop.size(w, h);
    fig.layout({
      w,
      h,
      dpr: Math.min(devicePixelRatio || 1, 2),
      cell: size.cell ?? 4.5,
      hand: size.hand,
      cap: size.cap ?? 0,
      phone: { x: w / 2, y: h / 2, w: -2, h: -2, r: 0 },
      ink: rgb(cs.color),
      accent: rgb(cs.getPropertyValue("--or").trim() || "#ff5b14"),
      page,
      inverse: page[0] + page[1] + page[2] > 1.5,
    });
    const now = performance.now();
    fig.snap(prop.scene({ fig, w, h, now }));
    if (reduce) fig.draw(now, true);
  };
  const io = new IntersectionObserver(
    ([e]) => {
      off();
      off = () => {};
      if (!e.isIntersecting) return;
      if (!asked) {
        asked = true;
        mountFigure(canvas, "/assets/hand.glb", getComputedStyle(canvas).fontFamily).then((f) => {
          if (dead) return f?.destroy();
          fig = f;
          fit();
        });
      }
      if (reduce) return;
      off = onFrame((now) => {
        if (!fig || !w) return;
        fig.goal(prop.scene({ fig, w, h, now }), prop.rate ?? 11);
        fig.draw(now);
      });
    },
    { rootMargin: "120px 0px" },
  );
  io.observe(canvas);
  const ro = new ResizeObserver(fit);
  ro.observe(canvas);
  const scheme = matchMedia("(prefers-color-scheme: dark)");
  scheme.addEventListener("change", fit);
  return {
    fig: () => fig,
    fit,
    stop() {
      dead = true;
      off();
      io.disconnect();
      ro.disconnect();
      scheme.removeEventListener("change", fit);
      fig?.destroy();
    },
  };
}

/** A hand that reaches in from the right and taps the element named by `data-point`, once, when its host comes to life. `data-point-x` names the element whose right edge the finger lands beside, `data-point-at` the delay in ms, and `data-point-edge="page"` lets the sleeve run to the edge of the window. */
export function mountPoint(el: HTMLElement): () => void {
  const canvas = el.querySelector<HTMLCanvasElement>(".pf")!;
  const ringEl = el.querySelector<HTMLElement>(".pf-ring");
  const delay = Number(el.dataset.pointAt ?? 700);
  const T = { x: 64, y: 130 };
  let t0 = -1e9, tapped = true;

  const place = () => {
    const mid = el.querySelector<HTMLElement>(el.dataset.point!);
    const end = (el.dataset.pointX && el.querySelector<HTMLElement>(el.dataset.pointX)) || mid;
    if (!mid || !end) return;
    const o = el.getBoundingClientRect(), m = mid.getBoundingClientRect(), e = end.getBoundingClientRect();
    const x = e.right - o.left + 12, y = m.top + m.height / 2 - o.top - 5;
    const right = el.dataset.pointEdge === "page" ? document.documentElement.clientWidth - o.left : o.width;
    const room = right - (x - T.x);
    canvas.hidden = room < 170 || !m.width;
    canvas.style.cssText = `left:${x - T.x}px;top:${y - T.y}px;width:${room}px;height:${T.y * 2}px`;
    if (ringEl) ringEl.style.cssText = `left:${x - 6}px;top:${y}px`;
  };
  const prop = mountProp(canvas, {
    size: (w) => ({ hand: w < 420 ? 1250 : 1500, cell: 4 }),
    rate: 10,
    scene({ fig, w, now }) {
      const aim = (dx: number, dy: number, lift: number, c: number[]): HandPose => {
        const pose: HandPose = { at: [0, 0, 0], fingers: [-0.9, -0.12, -0.42], palm: [-0.05, 0.85, -0.5], curl: c };
        const off = fig.reach(pose, "right");
        pose.at = [T.x + dx - off[0], T.y + dy - off[1], lift - off[2]];
        return pose;
      };
      const t = now - t0;
      if ("wait" in el.dataset) return { right: aim(w + 80, -26, 90, POINT) };
      if (t < delay - 70) return { right: aim(24, -9, 50, POINT) };
      if (t < delay + 160) {
        if (!tapped) {
          tapped = true;
          fig.pulse(T.x, T.y);
          if (ringEl) {
            delete ringEl.dataset.go;
            void ringEl.offsetWidth;
            ringEl.dataset.go = "";
          }
        }
        return { right: aim(0, 0, 2, PRESS) };
      }
      return { right: aim(12, -5, 36, POINT) };
    },
  });
  const play = () => {
    place();
    t0 = performance.now();
    tapped = false;
    prop.fig()?.enter();
  };
  el.addEventListener("life", play);
  const ro = new ResizeObserver(place);
  ro.observe(el);
  place();
  return () => {
    el.removeEventListener("life", play);
    ro.disconnect();
    prop.stop();
  };
}

/** The chauffeur in small, for a canvas `w` by `h`: the cap over a phone held up from below. `look` turns the cap down toward the phone, `on` (0 to 1) tips the cap and lifts the phone, `gx` and `gy` (-1 to 1) turn the cap toward a point. */
export function bearerScene(w: number, h: number, o: { look?: boolean; on?: number; gx?: number; gy?: number } = {}): Scene {
  const u = w / 300, on = o.on ?? 0, gx = o.gx ?? 0, gy = o.gy ?? 0;
  const cap: CapPose = { at: [(150 - on * 5) * u, (o.look ? 98 : 90) * u - on * 12 * u, 60 * u], turn: [(o.look ? 16 : -4) + on * 12 + gy * 7, (o.look ? -5 : -14) + gx * 13, (o.look ? -3 : -6) - on * 9] };
  return {
    cap,
    phone: { at: [150 * u, h - 128 * u - on * 16 * u, (60 + on * 40) * u], turn: [-6 - on * 8, -14 + gx * 5, 3], size: 84 * u },
    right: { at: [207 * u, h + 54 * u - on * 16 * u, on * 40 * u], fingers: [-0.5, -0.1, 0.86], palm: [0, -0.85, 0.52], curl: CRADLE },
  };
}
/** What `bearerScene` is drawn at, for a canvas `w` wide. */
export const bearerSize = (w: number) => ({ hand: w * 4.2, cap: w * 0.2, cell: w < 240 ? 3.5 : 4 });

function bearer(canvas: HTMLCanvasElement, look: boolean) {
  const gaze = { x: 0, y: 0, tx: 0, ty: 0 };
  let tipped = -1e9;
  const prop = mountProp(canvas, {
    size: bearerSize,
    rate: 9,
    scene({ w, h, now }) {
      const t = (now - tipped) / 1000;
      gaze.x += (gaze.tx - gaze.x) * 0.08;
      gaze.y += (gaze.ty - gaze.y) * 0.08;
      return bearerScene(w, h, { look, on: t > 0.08 && t < 0.7 ? 1 : 0, gx: gaze.x, gy: gaze.y });
    },
  });
  const onMove = (e: PointerEvent) => {
    const r = canvas.getBoundingClientRect();
    gaze.tx = Math.max(-1, Math.min(1, (e.clientX - r.left - r.width / 2) / 600));
    gaze.ty = Math.max(-1, Math.min(1, (e.clientY - r.top - r.height * 0.2) / 500));
  };
  addEventListener("pointermove", onMove, { passive: true });
  return {
    tip() {
      tipped = performance.now();
      prop.fig()?.drum();
    },
    stop() {
      removeEventListener("pointermove", onMove);
      prop.stop();
    },
  };
}

/** The closing figure. It tips the cap and lifts the phone when anything in its section is copied. */
export function mountEnd(el: HTMLElement): () => void {
  const section = el.closest<HTMLElement>("section")!;
  const figure = bearer(el.querySelector<HTMLCanvasElement>(".end-fig")!, false);
  const onCopy = (e: MouseEvent) => {
    if ((e.target as HTMLElement).closest("[data-copy]")) figure.tip();
  };
  section.addEventListener("click", onCopy);
  return () => {
    section.removeEventListener("click", onCopy);
    figure.stop();
  };
}

/** The figure for a page that is not there: the cap looks down at a phone with nothing on it. */
export function mountHalt(el: HTMLElement): () => void {
  return bearer(el.querySelector<HTMLCanvasElement>("canvas")!, true).stop;
}

const PALM = curl([0, 0, 0, 0, 0], [-0.08, 0, 0.05, 0.03], [0, 0, 0.05, 0.03], [0.06, 0, 0.05, 0.03], [0.14, 0, 0.05, 0.03]);

/** The raised palm at the approval stop. It comes up from the bottom edge of the pane when the recorded question does. */
export function mountPalm(el: HTMLElement): () => void {
  const canvas = el.querySelector<HTMLCanvasElement>(".ask-fig")!;
  const ask = el.querySelector<HTMLElement>(".ask")!;
  const delay = 1100;
  let t0 = -1e9;
  const prop = mountProp(canvas, {
    size: (w) => ({ hand: w * 6, cell: w < 170 ? 4 : 5 }),
    rate: 8,
    scene({ w, h, now }) {
      const up = !("wait" in ask.dataset) && now - t0 > delay;
      return { right: { at: [w * 0.5, up ? h * 1.29 : h * 2.7, up ? 50 : 0], fingers: [-0.1, -0.98, 0.1], palm: [-0.22, 0, 0.97], curl: PALM } };
    },
  });
  const play = () => {
    t0 = performance.now();
    prop.fig()?.enter();
  };
  ask.addEventListener("life", play);
  return () => {
    ask.removeEventListener("life", play);
    prop.stop();
  };
}
