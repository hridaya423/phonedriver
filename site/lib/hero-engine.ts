import { mountFigure, type Figure, type HandPose, type Scene } from "./chauffeur-gl";
import { scrolled } from "./frame";

export const SAY = "Order me a large pepperoni pizza from the Domino's nearest Leicester Square";
const BRIEF = "Order me a large pepperoni pizza";

type Pt = [number, number];
type Box = { x: number; y: number; w: number; h: number };
type Ribbon = { w: number; s: number; pts: Pt[] };

const RIBBONS: Record<"wide" | "narrow", Ribbon[]> = {
  wide: [
    { w: 0.12, s: 44, pts: [[-0.14, 0.52], [0.03, 0.36], [0.13, 0.2], [0.22, 0.1], [0.36, 0.03], [0.6, -0.05], [1.1, -0.16]] },
    { w: 0.14, s: 46, pts: [[0.62, -0.16], [0.8, 0.04], [0.9, 0.2], [0.97, 0.42], [1.0, 0.64], [0.95, 0.86], [0.84, 1.1]] },
    { w: 0.06, s: 24, pts: [[0.8, -0.16], [0.76, 0.1], [0.84, 0.3], [0.9, 0.5], [1.14, 0.62]] },
  ],
  narrow: [
    { w: 0.07, s: 30, pts: [[-0.3, 0.3], [0.0, 0.2], [0.4, 0.13], [0.8, 0.12], [1.3, 0.02]] },
    { w: 0.06, s: 26, pts: [[-0.3, 0.86], [0.1, 0.95], [0.5, 0.9], [0.9, 0.97], [1.3, 0.9]] },
  ],
};

const curl = (thumb: number[], ...f: number[][]) => [...thumb, ...[0, 1, 2, 3].flatMap((i) => f[Math.min(i, f.length - 1)])];
type Pose = { at: number[]; pin: number[]; fingers: number[]; palm: number[]; curl: number[] };
const POINT = curl([0.5, 0, 0, 0.4, 0.4], [0, 0.05, 0.1, 0.05], [0, 1.4, 1.5, 0.9]);
const POSES: Record<string, Pose> = {
  cradle: { at: [330, 1150, 0], pin: [209, 904], fingers: [-0.5, -0.1, 0.86], palm: [0, -0.85, 0.52], curl: curl([0.3, 0.4, 0, 0.2, 0.2], [0, 0.4, 0.8, 0.5]) },
  offer: { at: [-390, 44, 40], pin: [209, 0], fingers: [0.8, -0.1, 0.55], palm: [-0.1, -0.85, 0.5], curl: curl([0, 0, 0, 0.1, 0.1], [0, 0.15, 0.25, 0.15]) },
  hold: { at: [-382, 32, 40], pin: [209, 0], fingers: [0.8, -0.16, 0.55], palm: [-0.1, -0.85, 0.5], curl: curl([0.45, 0, 0, 0.3, 0.3], [0, 0.7, 0.9, 0.5]) },
  point: { at: [0, 0, 0], pin: [0, 0], fingers: [0.9, -0.12, -0.42], palm: [0.05, 0.85, -0.5], curl: POINT },
  press: { at: [0, 0, 0], pin: [0, 0], fingers: [0.9, -0.07, -0.44], palm: [0.05, 0.85, -0.5], curl: curl([0.5, 0, 0, 0.4, 0.4], [0, 0.16, 0.2, 0.1], [0, 1.4, 1.5, 0.9]) },
  stop: { at: [-330, 610, 60], pin: [0, 600], fingers: [0.12, -0.97, 0.1], palm: [0.25, 0, 0.97], curl: curl([0, 0, 0, 0, 0], [-0.08, 0, 0.05, 0.03], [0, 0, 0.05, 0.03], [0.06, 0, 0.05, 0.03], [0.14, 0, 0.05, 0.03]) },
  tip: { at: [0, 0, 0], pin: [0, 0], fingers: [0.62, -0.74, 0.2], palm: [0.75, 0.6, 0.2], curl: curl([0.45, 0.2, -0.4, 0.3, 0.2], [0, 0.36, 0.5, 0.3], [0, 1.0, 1.1, 0.7]) },
  tipped: { at: [0, 0, 0], pin: [0, 0], fingers: [0.62, -0.74, 0.2], palm: [0.75, 0.6, 0.2], curl: curl([0.6, 0.2, -0.5, 0.4, 0.3], [0, 0.5, 0.7, 0.4], [0, 1.1, 1.2, 0.8]) },
  catch: { at: [-388, 62, 40], pin: [209, 0], fingers: [0.8, -0.04, 0.55], palm: [-0.1, -0.85, 0.5], curl: curl([0.25, 0, 0, 0.2, 0.2], [0, 0.4, 0.55, 0.3]) },
  below: { at: [-412, 86, 40], pin: [209, 0], fingers: [0.82, 0.26, 0.5], palm: [0.08, -0.78, 0.62], curl: curl([0, 0.3, 0, 0.05, 0.05], [-0.04, 0.06, 0.1, 0.06], [0, 0.1, 0.16, 0.08], [0.04, 0.16, 0.22, 0.12], [0.1, 0.24, 0.3, 0.16]) },
};
const CAPS: Record<string, { at: number[]; turn: number[] }> = {
  rest: { at: [209, -62, 70], turn: [-6, -16, -7] },
  look: { at: [209, -52, 70], turn: [17, -6, -3] },
  halt: { at: [209, -76, 70], turn: [-2, 0, 0] },
  tip: { at: [203, -78, 70], turn: [8, -12, -15] },
};
const QUIPS: Record<string, { text: string; key?: boolean }> = {
  on: { text: "On it." },
  call: { text: "Your call.", key: true },
  sure: { text: "Of course." },
  paste: { text: "Copied. Paste it into your agent." },
  go: { text: "After you." },
};
const QUIP_AT = [462, -206];
const TIP = [0.34, 0.94, 1.26];
const tipping = (t: number) => (t < 0 || t >= TIP[2] ? null : t < TIP[0] || t >= TIP[1] ? { act: "tip", wear: "rest" } : { act: "tipped", wear: "tip" });

const VS = `
attribute vec4 a0;attribute vec4 a1;attribute vec2 a2;
uniform vec2 res;uniform float dpr,t,enter,scroll;uniform vec3 mouse;uniform vec4 rip,clr;
varying float vA,vG,vT;
float h(float n){return fract(sin(n)*43758.5453);}
void main(){
  vec2 c=a0.xy,n=a0.zw;float w=a1.x,u=a1.y,v=a1.z,seed=a1.w,rb=a2.x;
  float ph=rb*2.17;
  float tw=ph+u*(6.2+rb*1.7)+0.9*sin(u*3.1+t*0.19+ph)+scroll*1.6;
  float th=(v-0.5)*2.5+tw;
  float s=sin(th),co=cos(th);
  float wob=sin(u*6.5-t*0.33+ph)*w*0.14;
  vec2 p=c+n*(w*s+wob);
  vec2 d=p-mouse.xy;float r=length(d)+0.001;
  float f=mouse.z*pow(max(0.0,1.0-r/190.0),2.0);
  p+=d/r*f*40.0;
  float age=rip.z;vec2 dr=p-rip.xy;float rr=length(dr)+0.001;
  float ring=exp(-pow((rr-age*720.0)/80.0,2.0))*rip.w*exp(-age*0.55);
  p+=dr/rr*ring*16.0;
  vT=ring+f*0.25;
  float a=co>0.0?0.56+0.36*co:0.4+0.2*(-co);
  a+=(1.0-abs(co))*0.35;
  a*=smoothstep(0.0,0.06,u)*smoothstep(1.0,0.94,u);
  a*=clamp((enter*1.7-u-v*0.3)*4.0,0.0,1.0);
  vec2 q=abs(p-clr.xy)-clr.zw;float sd=length(max(q,0.0))+min(max(q.x,q.y),0.0);
  a*=smoothstep(-10.0,110.0,sd);
  a*=smoothstep(res.y+2.0,res.y*0.86,p.y)*smoothstep(40.0,110.0,p.y);
  vA=a;
  float flow=floor(u*520.0-t*2.4+seed*97.0);
  vG=floor(h(flow+v*131.0+rb*17.0)*4.0)+(co>0.5?4.0:0.0);
  gl_PointSize=(co>0.0?5.0+3.0*co:4.8)*dpr;
  gl_Position=vec4(p/res*2.0-1.0,0.0,1.0);gl_Position.y*=-1.0;
}`;

const FS = `precision mediump float;uniform sampler2D atlas;uniform vec3 ink,orc;varying float vA,vG,vT;
void main(){float m=texture2D(atlas,vec2((vG+gl_PointCoord.x)/8.0,gl_PointCoord.y)).a;float a=m*vA;if(a<0.01)discard;
vec3 col=mix(ink,orc,clamp(vT*1.6,0.0,1.0));gl_FragColor=vec4(col*a,a);}`;

const UNIFORMS = ["res", "dpr", "t", "enter", "scroll", "mouse", "rip", "clr", "ink", "orc", "atlas"] as const;

const lerp = (a: number, b: number, k: number) => a + (b - a) * k;
const easeOut = (k: number) => 1 - Math.pow(1 - k, 3);
const easeInOut = (k: number) => (k < 0.5 ? 4 * k * k * k : 1 - Math.pow(-2 * k + 2, 3) / 2);

function spline(pts: Pt[], steps: number): Pt[] {
  const out: Pt[] = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const p0 = pts[i - 1] ?? pts[i], p1 = pts[i], p2 = pts[i + 1], p3 = pts[i + 2] ?? p2;
    for (let k = 0; k < steps; k++) {
      const t = k / steps, t2 = t * t, t3 = t2 * t;
      const f = (a: number, b: number, c: number, d: number) =>
        0.5 * (2 * b + (-a + c) * t + (2 * a - 5 * b + 4 * c - d) * t2 + (-a + 3 * b - 3 * c + d) * t3);
      out.push([f(p0[0], p1[0], p2[0], p3[0]), f(p0[1], p1[1], p2[1], p3[1])]);
    }
  }
  out.push(pts[pts.length - 1]);
  return out;
}

function resample(line: Pt[], sp: number): Pt[] {
  const out: Pt[] = [line[0]];
  let acc = 0;
  for (let i = 1; i < line.length; i++) {
    const [ax, ay] = line[i - 1], [bx, by] = line[i];
    const seg = Math.hypot(bx - ax, by - ay);
    let t = 0;
    while (acc + seg - t * seg >= sp) {
      t += (sp - acc) / seg;
      out.push([ax + (bx - ax) * t, ay + (by - ay) * t]);
      acc = 0;
    }
    acc += seg * (1 - t);
  }
  return out;
}

function rgb(color: string): [number, number, number] {
  const c = document.createElement("canvas").getContext("2d")!;
  c.fillStyle = color;
  const h = String(c.fillStyle).replace("#", "");
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255) as [number, number, number];
}

type Run = {
  speed: number;
  lapse: number;
  length: number;
  screen: [number, number];
  chapters: [number, string][];
  focus: [number, string][];
  asked: number;
  reached: number;
  declined: number;
  held: [number, number];
  taps: [number, number, number][];
  looks: [number, number][];
};

export function mountHero(hero: HTMLElement): () => void {
  const still = /[?&]still\b/.test(location.search);
  if (still) document.documentElement.dataset.still = "";
  const reduce = still || matchMedia("(prefers-reduced-motion: reduce)").matches;
  const run = JSON.parse(hero.dataset.run!) as Run;
  const q = <T extends Element = HTMLElement>(s: string) => hero.querySelector<T>(s) as T;
  const canvas = q<HTMLCanvasElement>(".hx-weave");
  const board = q<HTMLCanvasElement>(".hx-fig");
  const ip = q(".ip");
  const rig = q(".hx-rig");
  const screen = q(".ip-screen");
  const video = q<HTMLVideoElement>(".ip-video");
  const touch = q(".ip-touch");
  const title = q("h1");
  const set = q(".hx-set");
  const view = q(".lg-view");
  const quip = q(".hx-quip");
  const rows = [...view.querySelectorAll<HTMLElement>("li")];
  const rowTimes = rows.map((r) => Number(r.dataset.t));
  const ask = q(".lg-ask");
  const chapter = q(".lg-chap");
  const clock = q(".lg-clock");
  const rate = q(".lg-speed");
  const toggle = q<HTMLButtonElement>(".lg-play");
  const range = q<HTMLInputElement>(".lg-track input");
  const svg = q<SVGSVGElement>(".hx-route");
  const path = q<SVGPathElement>("#hx-path");
  const say = q<SVGTextElement>("#hx-say");
  const tp = q<SVGTextPathElement>("#hx-say textPath");
  const reveal = q<SVGUseElement>(".hx-reveal");
  const head = q<SVGPathElement>(".hx-head");
  const packet = q<SVGCircleElement>(".hx-packet");
  const marks = document.querySelectorAll<HTMLElement>(".brand .wm");
  const family = getComputedStyle(title).fontFamily;
  const timers = new Set<number>();
  const cleanups: (() => void)[] = [];
  const on = <E extends Event = Event>(el: EventTarget, type: string, fn: (e: E) => void) => {
    el.addEventListener(type, fn as EventListener);
    cleanups.push(() => el.removeEventListener(type, fn as EventListener));
  };

  let W = 0, H = 0, WH = 0, wide = true, textStart = 0, total = 0, shown = 0, turn = 0, dead = false;
  let line = SAY, started = false, parked = false, seen = false, caught = false, waiting = false, playRaf = 0;
  let fig: Figure | null = null;
  let box: Box = { x: 0, y: 0, w: 1, h: 1 };
  let act = "offer", wear = "rest", nudge = 0, size = 1, quick = false;
  let aside: { t0: number; capOnly: boolean } | null = null;
  let caughtAt = -9e9, typing = 0;
  let note: { key: string; until: number } | null = null;
  let ripple = { x: 0, y: 0, t0: -99, amp: 0 };
  const mouse = { x: -9999, y: -9999, tx: -9999, ty: -9999, z: 0, tz: 0 };
  const gaze = { x: 0, y: 0, tx: 0, ty: 0 };
  const drawn = { rows: -1, pose: "", focus: "", chapter: "", clock: "", rate: "", said: "", quip: "" };

  const rel = (el: Element, to: Element = hero): Box => {
    const h = to.getBoundingClientRect(), r = el.getBoundingClientRect();
    return { x: r.left - h.left, y: r.top - h.top, w: r.width, h: r.height };
  };
  const sleep = (ms: number) =>
    new Promise<void>((r) => {
      const id = window.setTimeout(() => {
        timers.delete(id);
        r();
      }, ms);
      timers.add(id);
    });
  const tween = (ms: number, fn: (k: number) => void, e = easeOut) =>
    new Promise<void>((res) => {
      const t0 = performance.now();
      const step = (now: number) => {
        if (dead) return res();
        const k = Math.min(1, (now - t0) / ms);
        fn(e(k));
        if (k < 1) requestAnimationFrame(step);
        else res();
      };
      requestAnimationFrame(step);
    });
  const flag = (el: HTMLElement | SVGElement, name: string, value: boolean) => {
    if (value) el.dataset[name] = "";
    else delete el.dataset[name];
  };

  const lapsed = run.lapse / run.speed;
  const film = (r: number) => (r <= run.lapse ? r / run.speed : lapsed + (r - run.lapse));
  const real = (v: number) => (v <= lapsed ? v * run.speed : run.lapse + (v - lapsed));
  const taps = run.taps.map(([t, x, y]) => ({ v: film(t), x, y }));
  const looks = run.looks.map(([a, b]) => [film(a), Math.max(film(b), film(a) + 0.3)]);
  const asked = film(run.asked), declined = film(run.declined), reached = film(run.reached);

  const pt = () => ip.offsetWidth / 419;
  const spot = (x: number, y: number) => ({
    x: box.x + screen.offsetLeft + (x / run.screen[0]) * screen.offsetWidth,
    y: box.y + screen.offsetTop + (y / run.screen[1]) * screen.offsetHeight,
  });
  function place(p: Pose): HandPose {
    const k = pt();
    return { at: [box.x + (p.pin[0] + (p.at[0] - p.pin[0]) * size + (p === POSES.cradle ? 0 : nudge)) * k, box.y + (p.pin[1] + (p.at[1] - p.pin[1]) * size) * k, p.at[2] * k * size], fingers: p.fingers as HandPose["fingers"], palm: p.palm as HandPose["palm"], curl: p.curl };
  }
  function finger(p: Pose, T: { x: number; y: number }, lift: number): HandPose {
    const k = pt();
    const posed = place(p);
    const off = fig!.reach(posed, "left");
    return { ...posed, at: [T.x - off[0], T.y - off[1], lift * k - off[2]] };
  }
  function scene(): Scene {
    const k = pt();
    const now = performance.now();
    const extra = aside ? tipping((now - aside.t0) / 1000) : null;
    const pose = extra && !aside!.capOnly ? extra.act : act === "hold" && now - caughtAt < 180 ? "catch" : act;
    const [kind, which] = pose.split(":");
    let left: HandPose;
    if (kind === "hover" || kind === "press") {
      const T = which === "held" ? spot(run.held[0], run.held[1]) : spot(taps[+which].x, taps[+which].y);
      left = kind === "press" ? finger(POSES.press, T, 2) : finger(POSES.point, { x: T.x - 7 * k, y: T.y + 9 * k }, which === "held" ? 30 : 46);
    } else if (kind === "over") {
      const T = spot(taps[+which].x, taps[+which].y);
      left = finger(POSES.point, { x: T.x - 26 * k, y: T.y + 30 * k }, 96);
    } else if (kind === "below") {
      const a = place(POSES.hold), b = place(POSES.below);
      const s = Math.max(0, Math.sin(now / 520)) ** 0.6;
      const mix = (u: number[], w: number[]) => u.map((n, i) => lerp(n, w[i], s));
      left = { at: mix(a.at, b.at) as HandPose["at"], fingers: mix(a.fingers, b.fingers) as HandPose["fingers"], palm: mix(a.palm, b.palm) as HandPose["palm"], curl: mix(a.curl, b.curl) };
    } else if (kind === "ready") left = finger(POSES.point, { x: box.x - 40 * k, y: box.y + 600 * k }, 120);
    else left = place(POSES[kind]);
    const c = CAPS[extra ? extra.wear : wear];
    const cap = [box.x + (209 + (c.at[0] - 209) * size) * k, box.y + c.at[1] * size * k, c.at[2] * k];
    if (kind === "tip" || kind === "tipped") {
      const a = -c.turn[2] * (Math.PI / 180), r = 150 * size * k;
      const off = fig!.reach(left, "left");
      left.at = [cap[0] + r * (-1.04 * Math.cos(a) - 0.1 * Math.sin(a)) - off[0], cap[1] + r * (-1.04 * Math.sin(a) + 0.1 * Math.cos(a)) - off[1], cap[2] + 0.2 * r - off[2]];
    }
    return {
      left,
      right: place(POSES.cradle),
      cap: { at: cap as HandPose["at"], turn: [c.turn[0] + gaze.y * 7, c.turn[1] + gaze.x * 13, c.turn[2]] },
    };
  }
  function ring(x: number, y: number) {
    touch.style.left = `${(x / run.screen[0]) * 100}%`;
    touch.style.top = `${(y / run.screen[1]) * 100}%`;
    delete touch.dataset.go;
    void touch.offsetWidth;
    touch.dataset.go = "";
    const s = rel(screen);
    ripple = { x: s.x + (x / run.screen[0]) * s.w, y: s.y + (y / run.screen[1]) * s.h, t0: performance.now() / 1000, amp: 0.5 };
    const at = spot(x, y);
    fig?.pulse(at.x, at.y);
    rig.dataset.tap = "";
    sleep(130).then(() => delete rig.dataset.tap);
  }

  function hand(v: number): string {
    if (v >= declined + 0.4 + TIP[2] + 0.1) return "offer";
    const bow = tipping(v - declined - 0.4);
    if (bow) return bow.act;
    if (v >= asked) return "stop";
    if (v >= reached) return v >= asked - 0.22 ? "press:held" : "hover:held";
    if (!taps.length || v < taps[0].v - 0.9) return "ready";
    let i = 0;
    for (let n = 1; n < taps.length; n++) if (Math.abs(taps[n].v - v) < Math.abs(taps[i].v - v)) i = n;
    const d = v - taps[i].v;
    if (d >= -0.04 && d < 0.1) return `press:${i}`;
    if (d >= -0.34 && d < 0.2) return `hover:${i}`;
    if (looks.some(([a, b]) => v >= a && v < b)) return "ready";
    return d >= 0.2 && d < 0.46 ? `over:${i}` : "ready";
  }
  function speak(key: string) {
    if (key === drawn.quip) return;
    drawn.quip = key;
    const say = QUIPS[key];
    flag(quip, "on", !!say);
    clearInterval(typing);
    if (!say) return;
    flag(quip, "key", !!say.key);
    quip.textContent = say.text;
    const k = pt(), s = rel(board);
    const x = Math.max(14, Math.min(W - quip.offsetWidth - 14, s.x + box.x + (209 + (QUIP_AT[0] - 209) * size) * k));
    quip.style.left = `${x - s.x}px`;
    quip.style.top = `${box.y + QUIP_AT[1] * size * k}px`;
    if (reduce) return;
    let n = 0;
    quip.textContent = "";
    typing = window.setInterval(() => {
      quip.textContent = say.text.slice(0, ++n);
      if (n >= say.text.length) clearInterval(typing);
    }, 26);
  }

  const stamp = (r: number) => `${Math.floor(r / 60)}:${String(Math.floor(r % 60)).padStart(2, "0")}`;
  function render(v: number, force = false) {
    const r = started ? real(v) : -1;
    let n = 0;
    while (n < rows.length && rowTimes[n] <= r) n++;
    if (n !== drawn.rows || force) {
      rows.forEach((row, i) => {
        if (row.hidden !== i >= n) row.hidden = i >= n;
      });
      drawn.rows = n;
      view.scrollTop = view.scrollHeight;
    }
    let focus = run.focus[0][1];
    for (const [t, where] of run.focus) if (r >= t) focus = where;
    if (focus !== drawn.focus) ask.dataset.focus = drawn.focus = focus;
    flag(ask, "closed", r >= run.declined);
    let name = run.chapters[0][1];
    for (const [t, c] of run.chapters) if (r >= t) name = c;
    if (name !== drawn.chapter) chapter.textContent = drawn.chapter = name;
    flag(chapter, "key", r >= run.asked);
    const time = stamp(Math.max(0, r));
    if (time !== drawn.clock) clock.textContent = drawn.clock = time;
    const speed = r < run.lapse ? `${run.speed}×` : "real time";
    if (speed !== drawn.rate) rate.textContent = drawn.rate = speed;
    range.style.setProperty("--p", `${(v / run.length) * 100}%`);
    if (document.activeElement !== range || !video.paused) range.value = String(v);
    const said = `${time} of ${stamp(real(run.length))}, ${name}`;
    if (said !== drawn.said) range.setAttribute("aria-valuetext", (drawn.said = said));
    const held = started && v >= asked && v < declined;
    flag(hero, "held", held);
    const key = started ? hand(v) : waiting ? "below" : caught ? "hold" : "offer";
    if (key !== drawn.pose || force) {
      const [kind, which] = key.split(":");
      if (kind === "press" && which !== "held" && !reduce && key !== drawn.pose) ring(taps[+which].x, taps[+which].y);
      quick = (kind === "hover" || kind === "press" || kind === "over" || kind === "ready") && which !== "held";
      act = drawn.pose = key;
    }
    wear = held ? "halt" : key === "tipped" ? "tip" : key === "below" ? "look" : started && (looks.some(([a, b]) => v >= a && v < b) || (v >= declined && v < declined + 0.4)) ? "look" : "rest";
    const live = note && note.until > performance.now() ? note.key : "";
    speak(held ? "call" : started && (key === "tip" || key === "tipped") ? "sure" : live || (key === "below" ? "go" : ""));
  }

  function setReveal(L: number) {
    shown = L;
    reveal.style.strokeDashoffset = `${total - Math.min(L, total) + 0.01}`;
  }

  function geometry() {
    const k = pt(), b = rel(board), bottom = rel(set).y + rel(set).h;
    const o = POSES.offer;
    const E = { x: b.x + box.x + (o.pin[0] + (o.at[0] + 150 - o.pin[0]) * size + nudge) * k, y: b.y + box.y + (o.at[1] - 96) * size * k };
    const far = E.x > 330;
    const y = far ? Math.min(E.y - 40, Math.max(bottom + 34, E.y - 150 * k)) : bottom + 30;
    const lead = `M-24,${y + 12} L${Math.max(6, W * 0.012)},${y + 5}`;
    const d = far
      ? `${lead} C${E.x * 0.34},${y - 34} ${E.x - 150},${y - 46} ${E.x - 46},${y - 6} C${E.x - 12},${y + 8} ${E.x},${E.y - 44} ${E.x},${E.y}`
      : `${lead} C${W * 0.35},${y - 24} ${W * 0.62},${y - 24} ${W * 0.76},${y - 8} C${W * 0.87},${y + 6} ${W * 0.84},${y + 30} ${W * 0.7},${y + 30} L${E.x + 50},${y + 30} C${E.x + 14},${y + 30} ${E.x},${y + 44} ${E.x},${Math.max(E.y, y + 60)}`;
    path.setAttribute("d", d);
    const m = document.createElementNS("http://www.w3.org/2000/svg", "path");
    m.setAttribute("d", lead);
    textStart = m.getTotalLength() + 12;
    tp.setAttribute("startOffset", String(textStart));
    total = path.getTotalLength();
    const before = tp.textContent ?? "";
    tp.textContent = SAY + "…";
    line = far && textStart + say.getComputedTextLength() + 60 < total ? SAY + "…" : BRIEF + "…";
    tp.textContent = started || caught || reduce ? line : before;
    reveal.style.strokeDasharray = `${total} ${total + 40}`;
    setReveal(started || caught || reduce ? total : shown);
    const end = path.getPointAtLength(total), pre = path.getPointAtLength(total - 2);
    head.setAttribute("transform", `translate(${end.x} ${end.y}) rotate(${(Math.atan2(end.y - pre.y, end.x - pre.x) * 180) / Math.PI})`);
  }

  function follow() {
    playRaf = 0;
    if (dead) return;
    render(video.currentTime);
    if (!video.paused && !video.ended) playRaf = requestAnimationFrame(follow);
  }
  function begin() {
    turn++;
    started = true;
    caught = true;
    waiting = false;
    tp.textContent = line;
    setReveal(total);
    head.style.opacity = "1";
    packet.style.opacity = "0";
    hero.dataset.live = "";
  }
  async function play() {
    if (!started) begin();
    if (video.ended || video.currentTime >= run.length - 0.05) video.currentTime = 0;
    delete hero.dataset.done;
    try {
      await video.play();
    } catch {
      render(video.currentTime, true);
    }
  }
  function remark(key: string, ms: number) {
    note = { key, until: performance.now() + ms };
    render(video.currentTime);
    sleep(ms + 20).then(() => !dead && render(video.currentTime));
  }

  async function intro() {
    const id = ++turn;
    const live = () => id === turn && !dead;
    render(0, true);
    head.style.opacity = "0";
    // Chrome reports a new largest contentful paint each time this line grows, so it keeps its full width from the first frame.
    tp.textContent = " ".repeat(line.length);
    setReveal(0);
    await Promise.race([ready, sleep(1600)]);
    await sleep(750);
    if (!live()) return;
    aside = { t0: performance.now(), capOnly: false };
    await sleep(TIP[1] * 1000);
    if (!live()) return;
    await tween(350, (k) => setReveal(k * textStart));
    for (let i = 1; i <= line.length; i++) {
      if (!live()) return;
      tp.textContent = line.slice(0, i) + " ".repeat(line.length - i);
      setReveal(textStart + say.getSubStringLength(0, i) + 4);
      await sleep(line[i - 1] === " " ? 34 : 15 + Math.random() * 14);
    }
    if (!live()) return;
    const from = shown;
    packet.style.opacity = "1";
    await tween(
      420,
      (k) => {
        const L = from + (total - from) * k;
        setReveal(L);
        const p = path.getPointAtLength(L);
        packet.setAttribute("cx", String(p.x));
        packet.setAttribute("cy", String(p.y));
      },
      easeInOut,
    );
    if (!live()) return;
    packet.style.opacity = "0";
    head.style.opacity = "1";
    caught = true;
    caughtAt = performance.now();
    const end = path.getPointAtLength(total), b = rel(board);
    fig?.pulse(end.x - b.x, end.y - b.y);
    remark("on", 1700);
    await sleep(650);
    if (!live()) return;
    if (seen) return void play();
    parked = true;
    await sleep(1500);
    if (!live() || started) return;
    waiting = true;
    render(0, true);
  }

  const drum = () => {
    if (!visible || document.hidden || reduce) return;
    fig?.drum();
  };
  async function idle() {
    const id = turn;
    while (id === turn && !dead && video.paused) {
      await sleep(6400);
      if (id === turn && video.paused) drum();
    }
  }

  on(video, "play", () => {
    hero.dataset.playing = "";
    toggle.setAttribute("aria-label", "Pause the recording");
    if (!playRaf) playRaf = requestAnimationFrame(follow);
  });
  on(video, "pause", () => {
    delete hero.dataset.playing;
    toggle.setAttribute("aria-label", video.ended ? "Play the recording again" : "Play the recording");
    render(video.currentTime);
  });
  on(video, "ended", () => {
    hero.dataset.done = "";
    render(run.length);
    marks.forEach((m) => {
      m.dataset.tip = "";
      sleep(320).then(() => delete m.dataset.tip);
    });
    idle();
  });
  on(video, "seeked", () => render(video.currentTime));
  on(toggle, "click", () => {
    parked = false;
    if (video.paused) play();
    else video.pause();
  });
  on(range, "input", () => {
    if (!started) begin();
    delete hero.dataset.done;
    video.currentTime = Number(range.value);
    render(Number(range.value));
  });
  on(rig, "click", () => {
    parked = false;
    if (video.paused) play();
    else video.pause();
  });
  on<MouseEvent>(set, "click", (e) => {
    if (!(e.target as HTMLElement).closest("[data-copy]")) return;
    const busy = started && !video.paused;
    aside = { t0: performance.now() - (busy ? TIP[0] * 1000 : 0), capOnly: busy };
    if (!busy && !hero.dataset.held) remark("paste", 2400);
    kick();
  });

  let gl: WebGLRenderingContext | null = null;
  let prog: WebGLProgram, buf: WebGLBuffer;
  let count = 0, atlasReady = false;
  const U = {} as Record<(typeof UNIFORMS)[number], WebGLUniformLocation | null>;

  function initGL() {
    gl = canvas.getContext("webgl", { antialias: false, alpha: true, premultipliedAlpha: true });
    if (!gl) return;
    const g = gl;
    const sh = (type: number, src: string) => {
      const s = g.createShader(type)!;
      g.shaderSource(s, src);
      g.compileShader(s);
      return s;
    };
    prog = g.createProgram()!;
    g.attachShader(prog, sh(g.VERTEX_SHADER, VS));
    g.attachShader(prog, sh(g.FRAGMENT_SHADER, FS));
    g.linkProgram(prog);
    if (!g.getProgramParameter(prog, g.LINK_STATUS)) {
      gl = null;
      return;
    }
    g.useProgram(prog);
    for (const n of UNIFORMS) U[n] = g.getUniformLocation(prog, n);
    buf = g.createBuffer()!;
    g.enable(g.BLEND);
    g.blendFunc(g.ONE, g.ONE_MINUS_SRC_ALPHA);
  }

  async function atlas(g: WebGLRenderingContext) {
    await document.fonts.load(`700 52px ${family}`).catch(() => undefined);
    const c = document.createElement("canvas");
    c.width = 512;
    c.height = 64;
    const x = c.getContext("2d")!;
    x.fillStyle = "#fff";
    x.textAlign = "center";
    x.textBaseline = "middle";
    [14, 11].forEach((r, i) => {
      x.beginPath();
      x.arc(i * 64 + 32, 32, r, 0, 7);
      x.fill();
    });
    x.font = `800 60px ${family}`;
    x.fillText(":", 2 * 64 + 32, 30);
    x.fillText("-", 3 * 64 + 32, 32);
    x.font = `700 58px ${family}`;
    ["0", "1", "+", "/"].forEach((ch, i) => x.fillText(ch, (i + 4) * 64 + 32, 34));
    const tex = g.createTexture();
    g.bindTexture(g.TEXTURE_2D, tex);
    g.pixelStorei(g.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
    g.texImage2D(g.TEXTURE_2D, 0, g.RGBA, g.RGBA, g.UNSIGNED_BYTE, c);
    g.texParameteri(g.TEXTURE_2D, g.TEXTURE_MIN_FILTER, g.LINEAR);
    g.texParameteri(g.TEXTURE_2D, g.TEXTURE_MAG_FILTER, g.LINEAR);
    g.texParameteri(g.TEXTURE_2D, g.TEXTURE_WRAP_S, g.CLAMP_TO_EDGE);
    g.texParameteri(g.TEXTURE_2D, g.TEXTURE_WRAP_T, g.CLAMP_TO_EDGE);
    atlasReady = true;
  }

  function colors(g: WebGLRenderingContext) {
    const cs = getComputedStyle(hero);
    g.uniform3fv(U.ink, rgb(cs.color));
    g.uniform3fv(U.orc, rgb(cs.getPropertyValue("--or").trim() || "#ff5b14"));
  }

  function buildWeave(g: WebGLRenderingContext) {
    const dpr = Math.min(devicePixelRatio || 1, 2);
    canvas.width = Math.round(W * dpr);
    canvas.height = Math.round(WH * dpr);
    g.viewport(0, 0, canvas.width, canvas.height);
    const small = W < 600;
    const sp = small ? 4.2 : 3.6;
    const data: number[] = [];
    RIBBONS[wide ? "wide" : "narrow"].forEach((rb, ri) => {
      const line = resample(spline(rb.pts.map(([x, y]) => [x * W, y * WH] as Pt), 40), sp);
      const n = line.length;
      const strands = small ? Math.round(rb.s * 0.8) : rb.s;
      const wid = rb.w * (wide ? WH : Math.max(WH, 900));
      for (let j = 0; j < strands; j++) {
        const v = j / (strands - 1);
        const seed = Math.random();
        for (let i = 0; i < n; i++) {
          const a = line[Math.max(0, i - 1)], b = line[Math.min(n - 1, i + 1)];
          const tx = b[0] - a[0], ty = b[1] - a[1];
          const l = Math.hypot(tx, ty) || 1;
          const u = i / (n - 1);
          data.push(line[i][0], line[i][1], -ty / l, tx / l, wid * (0.55 + 0.45 * Math.sin(Math.PI * u)), u, v, seed, ri, Math.random());
        }
      }
    });
    const arr = new Float32Array(data);
    count = arr.length / 10;
    g.bindBuffer(g.ARRAY_BUFFER, buf);
    g.bufferData(g.ARRAY_BUFFER, arr, g.STATIC_DRAW);
    for (const [n, size, offset] of [["a0", 4, 0], ["a1", 4, 16], ["a2", 2, 32]] as const) {
      const l = g.getAttribLocation(prog, n);
      g.enableVertexAttribArray(l);
      g.vertexAttribPointer(l, size, g.FLOAT, false, 40, offset);
    }
    g.uniform1f(U.dpr, dpr);
    const top = rel(title), low = rel(set);
    const pad = wide ? 30 : 12, half = Math.max(top.w, low.w) / 2 + pad;
    g.uniform4f(U.clr, W / 2, (top.y + low.y + low.h) / 2, half, (low.y + low.h - top.y) / 2 + pad);
    colors(g);
  }

  function stage() {
    if (!fig) return;
    const b = board.getBoundingClientRect(), k = pt();
    const page = rgb(getComputedStyle(document.body).backgroundColor);
    fig.layout({
      w: b.width,
      h: b.height,
      dpr: Math.min(devicePixelRatio || 1, 2),
      cell: W < 700 ? 4.5 : 5,
      hand: 3400 * size * k,
      cap: 150 * size * k,
      phone: { ...box, r: 56 * k },
      ink: rgb(getComputedStyle(hero).color),
      accent: rgb(getComputedStyle(hero).getPropertyValue("--or").trim() || "#ff5b14"),
      page,
      inverse: page[0] + page[1] + page[2] > 1.5,
    });
    fig.snap(scene());
  }

  function layout() {
    const r = hero.getBoundingClientRect();
    W = r.width;
    H = r.height;
    WH = canvas.clientHeight;
    wide = W >= 1020;
    box = rel(ip, board);
    size = W < 700 ? 0.8 : 1;
    nudge = Math.max(0, (W < 700 ? 330 : 250) * size - (rel(ip).x - 24) / pt());
    svg.setAttribute("width", String(W));
    svg.setAttribute("height", String(H));
    svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
    geometry();
    drawn.quip = "";
    render(video.currentTime, true);
    if (gl) buildWeave(gl);
    stage();
  }

  let visible = true, raf = 0, enterT0: number | null = null;
  const t0 = performance.now();
  function frame(now: number) {
    raf = 0;
    if (dead) return;
    if (gl && atlasReady) {
      const t = (now - t0) / 1000;
      if (enterT0 === null) enterT0 = now;
      const enter = reduce ? 1 : easeOut(Math.min(1, (now - enterT0) / 2200));
      mouse.x = lerp(mouse.x, mouse.tx, 0.12);
      mouse.y = lerp(mouse.y, mouse.ty, 0.12);
      mouse.z = lerp(mouse.z, mouse.tz, 0.06);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.uniform2f(U.res, W, WH);
      gl.uniform1f(U.t, reduce ? 3 : t);
      gl.uniform1f(U.enter, enter);
      gl.uniform1f(U.scroll, reduce ? 0 : Math.min(1.5, scrolled() / Math.max(1, WH)));
      gl.uniform3f(U.mouse, mouse.x, mouse.y, reduce ? 0 : mouse.z);
      gl.uniform4f(U.rip, ripple.x, ripple.y, now / 1000 - ripple.t0, ripple.amp);
      gl.uniform1i(U.atlas, 0);
      gl.drawArrays(gl.POINTS, 0, count);
    }
    if (fig) {
      gaze.x = lerp(gaze.x, gaze.tx, 0.08);
      gaze.y = lerp(gaze.y, gaze.ty, 0.08);
      if (reduce) fig.snap(scene());
      else fig.goal(scene(), quick ? 24 : act === "stop" ? 15 : 11);
      fig.draw(now, reduce);
    }
    if (visible && !document.hidden && !reduce) raf = requestAnimationFrame(frame);
  }
  const kick = () => {
    if (!raf && !dead) raf = requestAnimationFrame(frame);
  };

  on<PointerEvent>(hero, "pointermove", (e) => {
    const r = hero.getBoundingClientRect();
    mouse.tx = e.clientX - r.left;
    mouse.ty = e.clientY - r.top;
    if (mouse.x < -999) {
      mouse.x = mouse.tx;
      mouse.y = mouse.ty;
    }
    mouse.tz = e.pointerType === "mouse" ? 1 : 0.7;
    if (e.pointerType === "mouse") {
      const c = ip.getBoundingClientRect();
      gaze.tx = Math.max(-1, Math.min(1, (e.clientX - (c.left + c.width / 2)) / (W * 0.4)));
      gaze.ty = Math.max(-1, Math.min(1, (e.clientY - c.top) / 420));
    }
  });
  on(hero, "pointerleave", () => {
    mouse.tz = 0;
    gaze.tx = gaze.ty = 0;
  });

  const io = new IntersectionObserver(([e]) => {
    visible = e.isIntersecting;
    if (visible) kick();
  });
  io.observe(hero);
  const watch = new IntersectionObserver(
    ([e]) => {
      seen = e.intersectionRatio >= 0.5;
      if (e.intersectionRatio < 0.2 && !video.paused) {
        parked = true;
        video.pause();
      } else if (seen && parked) {
        parked = false;
        play();
      }
    },
    { threshold: [0.2, 0.5] },
  );
  watch.observe(ip);
  document.addEventListener("visibilitychange", kick);
  const scheme = matchMedia("(prefers-color-scheme: dark)");
  const recolor = () => {
    if (gl) colors(gl);
    stage();
    kick();
  };
  scheme.addEventListener("change", recolor);
  let rt = 0;
  const ro = new ResizeObserver(() => {
    cancelAnimationFrame(rt);
    rt = requestAnimationFrame(() => {
      layout();
      kick();
    });
  });
  ro.observe(hero);

  initGL();
  layout();
  if (gl) atlas(gl).then(kick);
  const ready = mountFigure(board, "/assets/hand.glb", family).then((f) => {
    if (dead) return f?.destroy();
    fig = f;
    stage();
    kick();
  });
  if (reduce) {
    video.poster = "/assets/run-poster.jpg";
    begin();
    video.currentTime = asked + 1;
    render(asked + 1, true);
  } else {
    const show = () => (hero.dataset.live = "");
    if (video.readyState >= 2) show();
    else on(video, "loadeddata", show);
    intro();
  }

  return () => {
    dead = true;
    turn++;
    cancelAnimationFrame(raf);
    cancelAnimationFrame(rt);
    cancelAnimationFrame(playRaf);
    video.pause();
    timers.forEach(clearTimeout);
    clearInterval(typing);
    cleanups.forEach((f) => f());
    io.disconnect();
    watch.disconnect();
    ro.disconnect();
    fig?.destroy();
    document.removeEventListener("visibilitychange", kick);
    scheme.removeEventListener("change", recolor);
    if (still) delete document.documentElement.dataset.still;
  };
}
