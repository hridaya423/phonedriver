import { scrolled } from "./frame";

export const SAY = "Order me a large pepperoni pizza from the Domino's nearest Leicester Square";
const BRIEF = "Order me a large pepperoni pizza";

type Pt = [number, number];
type Box = { x: number; y: number; w: number; h: number };
type Ribbon = { w: number; s: number; pts: Pt[] };

const RIBBONS: Record<"wide" | "narrow", Ribbon[]> = {
  wide: [
    { w: 0.12, s: 44, pts: [[-0.14, 0.62], [0.06, 0.4], [0.26, 0.27], [0.46, 0.21], [0.64, 0.15], [0.8, 0.06], [1.14, -0.04]] },
    { w: 0.14, s: 46, pts: [[0.5, -0.16], [0.72, 0.0], [0.9, 0.16], [0.99, 0.44], [0.92, 0.72], [0.76, 0.94], [0.56, 1.16]] },
    { w: 0.11, s: 39, pts: [[-0.14, 1.04], [0.12, 0.93], [0.36, 0.88], [0.56, 0.82], [0.76, 0.74], [0.94, 0.7], [1.16, 0.74]] },
    { w: 0.06, s: 24, pts: [[0.64, -0.16], [0.57, 0.1], [0.52, 0.34], [0.56, 0.58], [0.6, 0.8], [0.52, 1.16]] },
  ],
  narrow: [
    { w: 0.07, s: 30, pts: [[-0.3, 0.5], [0.0, 0.4], [0.4, 0.37], [0.8, 0.4], [1.3, 0.5]] },
    { w: 0.07, s: 30, pts: [[-0.3, 1.02], [0.04, 0.86], [0.2, 0.72], [0.5, 0.64], [1.0, 0.66], [1.3, 0.74]] },
    { w: 0.05, s: 22, pts: [[-0.3, 0.92], [0.3, 1.0], [0.8, 0.93], [1.3, 0.99]] },
  ],
};

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
  const ip = q(".ip");
  const rig = q(".hx-rig");
  const glove = q<SVGSVGElement>(".ch-tap");
  const screen = q(".ip-screen");
  const video = q<HTMLVideoElement>(".ip-video");
  const touch = q(".ip-touch");
  const copy = q(".hx-copy");
  const pane = q(".hx-log");
  const view = q(".lg-view");
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
  const family = getComputedStyle(q("h1")).fontFamily;
  const timers = new Set<number>();
  const cleanups: (() => void)[] = [];
  const on = <E extends Event = Event>(el: EventTarget, type: string, fn: (e: E) => void) => {
    el.addEventListener(type, fn as EventListener);
    cleanups.push(() => el.removeEventListener(type, fn as EventListener));
  };

  let W = 0, H = 0, wide = true, textStart = 0, total = 0, shown = 0, turn = 0, dead = false;
  let line = SAY, started = false, parked = false, playRaf = 0;
  let ripple = { x: 0, y: 0, t0: -99, amp: 0 };
  const mouse = { x: -9999, y: -9999, tx: -9999, ty: -9999, z: 0, tz: 0 };
  const tilt = { x: 0, y: 0, tx: 0, ty: 0 };
  const drawn = { rows: -1, pose: "", focus: "", chapter: "", clock: "", rate: "", said: "" };

  const rel = (el: Element): Box => {
    const h = hero.getBoundingClientRect(), r = el.getBoundingClientRect();
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
  function pose(x: number, y: number, rotate: number, scale: number) {
    const w = pt() * 201.6;
    glove.style.translate = `${x - w / 3}px ${y - (w * 12) / 126}px`;
    glove.style.rotate = `${wide ? rotate : -rotate}deg`;
    glove.style.scale = wide ? String(scale) : `${-scale} ${scale}`;
  }
  function spot(x: number, y: number) {
    return { x: screen.offsetLeft + (x / run.screen[0]) * screen.offsetWidth, y: screen.offsetTop + (y / run.screen[1]) * screen.offsetHeight };
  }
  function at(T: { x: number; y: number }, dx: number, dy: number, rotate: number, scale: number) {
    const k = pt();
    pose(T.x + (wide ? dx : -dx) * k, T.y + dy * k, rotate, scale);
  }
  function palm() {
    const k = pt(), turn = wide ? -8 : -10, size = wide ? 1 : 0.8;
    const c = wide ? { x: 476 * k, y: 690 * k } : { x: -36 * k, y: 850 * k };
    const a = (turn * Math.PI) / 180, u = 1.6 * k * size;
    const vx = (33.5 * Math.cos(a) - 88 * Math.sin(a)) * u, vy = (33.5 * Math.sin(a) + 88 * Math.cos(a)) * u;
    pose(c.x - (wide ? vx : -vx), c.y - vy, turn, size);
  }
  function ring(x: number, y: number) {
    touch.style.left = `${(x / run.screen[0]) * 100}%`;
    touch.style.top = `${(y / run.screen[1]) * 100}%`;
    delete touch.dataset.go;
    void touch.offsetWidth;
    touch.dataset.go = "";
    const s = rel(screen);
    ripple = { x: s.x + (x / run.screen[0]) * s.w, y: s.y + (y / run.screen[1]) * s.h, t0: performance.now() / 1000, amp: 0.5 };
  }

  function hand(v: number): string {
    if (v >= declined) return "away";
    if (v >= asked) return "palm";
    if (v >= reached) return v >= asked - 0.22 ? "press:held" : "hover:held";
    if (!taps.length || v < taps[0].v - 0.9) return "away";
    let i = 0;
    for (let n = 1; n < taps.length; n++) if (Math.abs(taps[n].v - v) < Math.abs(taps[i].v - v)) i = n;
    const d = v - taps[i].v;
    if (d >= -0.04 && d < 0.1) return `press:${i}`;
    if (d >= -0.3 && d < 0.26) return `hover:${i}`;
    return "ready";
  }
  function setHand(key: string) {
    const [kind, which] = key.split(":");
    const T = which === "held" ? spot(run.held[0], run.held[1]) : which ? spot(taps[+which].x, taps[+which].y) : null;
    const k = pt();
    flag(glove, "in", kind !== "away");
    flag(rig, "reach", kind !== "away");
    flag(glove, "open", kind === "palm");
    flag(glove, "quick", kind !== "palm" && which !== "held");
    flag(glove, "press", kind === "press");
    flag(rig, "brace", kind === "press");
    if (kind === "palm") palm();
    else if (kind === "press" && T) at(T, which === "held" ? 4 : 0, which === "held" ? 7 : 0, which === "held" ? -64 : -40, which === "held" ? 0.84 : 0.72);
    else if (kind === "hover" && T) at(T, 12, 19, which === "held" ? -62 : -38, which === "held" ? 0.88 : 0.78);
    else pose(wide ? ip.offsetWidth + 26 * k : -26 * k, 660 * k, -58, kind === "ready" ? 0.78 : 0.7);
    if (kind === "press" && which !== "held" && !reduce) ring(taps[+which].x, taps[+which].y);
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
    let title = run.chapters[0][1];
    for (const [t, name] of run.chapters) if (r >= t) title = name;
    if (title !== drawn.chapter) chapter.textContent = drawn.chapter = title;
    flag(chapter, "key", r >= run.asked);
    const time = stamp(Math.max(0, r));
    if (time !== drawn.clock) clock.textContent = drawn.clock = time;
    const speed = r < run.lapse ? `${run.speed}×` : "real time";
    if (speed !== drawn.rate) rate.textContent = drawn.rate = speed;
    range.style.setProperty("--p", `${(v / run.length) * 100}%`);
    if (document.activeElement !== range || !video.paused) range.value = String(v);
    const said = `${time} of ${stamp(real(run.length))}, ${title}`;
    if (said !== drawn.said) range.setAttribute("aria-valuetext", (drawn.said = said));
    flag(hero, "held", started && v >= asked && v < declined);
    flag(rig, "halt", started && v >= asked && v < declined);
    flag(rig, "look", started && (looks.some(([a, b]) => v >= a && v < b) || (v >= declined && v < declined + 0.8)));
    const key = started ? hand(v) : "away";
    if (key !== drawn.pose || force) setHand((drawn.pose = key));
  }

  function setReveal(L: number) {
    shown = L;
    reveal.style.strokeDashoffset = `${total - Math.min(L, total) + 0.01}`;
  }

  function geometry() {
    const p = rel(pane.firstElementChild!), f = rel(ip);
    let E = { x: p.x + Math.min(54, p.w * 0.2), y: p.y - 9 };
    let d: string, lead: string;
    if (wide) {
      lead = `M-24,${H * 0.44} L${W * 0.012},${H * 0.412}`;
      d = `${lead} C${W * 0.1},${H * 0.27} ${W * 0.25},${H * 0.15} ${W * 0.37},${H * 0.175} C${W * 0.45},${H * 0.19} ${E.x},${E.y - H * 0.11} ${E.x},${E.y}`;
    } else if (W >= 700) {
      E = { x: p.x + p.w - 44, y: p.y - 9 };
      const y = E.y - 64;
      lead = `M-24,${y + 18} L8,${y + 11}`;
      d = `${lead} C${E.x * 0.4},${y - 16} ${E.x - 110},${y - 20} ${E.x - 34},${y - 4} C${E.x - 8},${y + 3} ${E.x},${y + 22} ${E.x},${E.y}`;
    } else {
      E = { x: f.x + f.w + 9, y: f.y + f.h * 0.085 };
      const c = rel(copy);
      const y = c.y + c.h + 46;
      lead = `M-24,${y + 12} L6,${y + 5}`;
      d = `${lead} C${W * 0.36},${y - 30} ${W * 0.7},${y - 30} ${W * 0.84},${y + 8} C${W * 0.99},${y + 50} ${W * 0.99},${E.y - 10} ${E.x},${E.y}`;
    }
    path.setAttribute("d", d);
    const m = document.createElementNS("http://www.w3.org/2000/svg", "path");
    m.setAttribute("d", lead);
    textStart = m.getTotalLength() + 12;
    tp.setAttribute("startOffset", String(textStart));
    total = path.getTotalLength();
    const before = tp.textContent ?? "";
    tp.textContent = SAY + "…";
    line = wide && textStart + say.getComputedTextLength() + 36 < total ? SAY + "…" : BRIEF + "…";
    tp.textContent = started || reduce ? line : before;
    reveal.style.strokeDasharray = `${total} ${total + 40}`;
    setReveal(started || reduce ? total : shown);
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

  async function intro() {
    const id = ++turn;
    const live = () => id === turn && !dead;
    render(0, true);
    head.style.opacity = "0";
    // Chrome reports a new largest contentful paint each time this line grows, so it keeps its full width from the first frame.
    tp.textContent = "\u00a0".repeat(line.length);
    setReveal(0);
    await sleep(250);
    if (!live()) return;
    drum();
    await tween(350, (k) => setReveal(k * textStart));
    for (let i = 1; i <= line.length; i++) {
      if (!live()) return;
      tp.textContent = line.slice(0, i) + "\u00a0".repeat(line.length - i);
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
    if (live()) play();
  }

  const drum = () => {
    if (!visible || document.hidden || reduce) return;
    rig.dataset.drum = "";
    sleep(900).then(() => delete rig.dataset.drum);
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
    if (!started) return;
    parked = false;
    if (video.paused) play();
    else video.pause();
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
    canvas.height = Math.round(H * dpr);
    g.viewport(0, 0, canvas.width, canvas.height);
    const small = W < 600;
    const sp = small ? 4.2 : 3.6;
    const data: number[] = [];
    RIBBONS[wide ? "wide" : "narrow"].forEach((rb, ri) => {
      const line = resample(spline(rb.pts.map(([x, y]) => [x * W, y * H] as Pt), 40), sp);
      const n = line.length;
      const strands = small ? Math.round(rb.s * 0.8) : rb.s;
      const wid = rb.w * (wide ? H : Math.max(H, 900));
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
    const cr = rel(copy);
    const pad = wide ? 20 : 12;
    g.uniform4f(U.clr, cr.x + cr.w / 2, cr.y + cr.h / 2, cr.w / 2 + pad, cr.h / 2 + pad);
    colors(g);
  }

  function layout() {
    const r = hero.getBoundingClientRect();
    W = r.width;
    H = r.height;
    wide = W >= 1020;
    svg.setAttribute("width", String(W));
    svg.setAttribute("height", String(H));
    svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
    geometry();
    render(video.currentTime, true);
    if (gl) buildWeave(gl);
  }

  let visible = true, raf = 0, enterT0: number | null = null;
  const t0 = performance.now();
  function frame(now: number) {
    raf = 0;
    if (!gl || !atlasReady || dead) return;
    const t = (now - t0) / 1000;
    if (enterT0 === null) enterT0 = now;
    const enter = reduce ? 1 : easeOut(Math.min(1, (now - enterT0) / 2200));
    mouse.x = lerp(mouse.x, mouse.tx, 0.12);
    mouse.y = lerp(mouse.y, mouse.ty, 0.12);
    mouse.z = lerp(mouse.z, mouse.tz, 0.06);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.uniform2f(U.res, W, H);
    gl.uniform1f(U.t, reduce ? 3 : t);
    gl.uniform1f(U.enter, enter);
    gl.uniform1f(U.scroll, reduce ? 0 : Math.min(1.5, scrolled() / Math.max(1, H)));
    gl.uniform3f(U.mouse, mouse.x, mouse.y, reduce ? 0 : mouse.z);
    gl.uniform4f(U.rip, ripple.x, ripple.y, now / 1000 - ripple.t0, ripple.amp);
    gl.uniform1i(U.atlas, 0);
    gl.drawArrays(gl.POINTS, 0, count);
    if (!reduce) {
      tilt.x = lerp(tilt.x, tilt.tx, 0.07);
      tilt.y = lerp(tilt.y, tilt.ty, 0.07);
      if (Math.abs(tilt.x - tilt.tx) + Math.abs(tilt.y - tilt.ty) > 0.001) {
        rig.style.transform = `rotateX(${tilt.x}deg) rotateY(${tilt.y}deg)`;
      }
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
    if (wide && e.pointerType === "mouse") {
      tilt.ty = (mouse.tx / W - 0.5) * 7;
      tilt.tx = -(mouse.ty / H - 0.5) * 4;
    }
  });
  on(hero, "pointerleave", () => {
    mouse.tz = 0;
    tilt.tx = tilt.ty = 0;
  });

  const io = new IntersectionObserver(([e]) => {
    visible = e.isIntersecting;
    if (visible) delete hero.dataset.off;
    else hero.dataset.off = "";
    if (visible) kick();
  });
  io.observe(hero);
  let met = reduce;
  const watch = new IntersectionObserver(
    ([e]) => {
      if (e.isIntersecting && !met) {
        met = true;
        intro();
      } else if (!e.isIntersecting && !video.paused) {
        parked = true;
        video.pause();
      } else if (e.isIntersecting && parked) {
        parked = false;
        play();
      }
    },
    { threshold: 0.3 },
  );
  watch.observe(ip);
  document.addEventListener("visibilitychange", kick);
  const scheme = matchMedia("(prefers-color-scheme: dark)");
  const recolor = () => {
    if (gl) {
      colors(gl);
      kick();
    }
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
  if (reduce) {
    video.poster = "/assets/run-poster.jpg";
    begin();
    video.currentTime = asked + 1;
    render(asked + 1, true);
  } else {
    const show = () => (hero.dataset.live = "");
    if (video.readyState >= 2) show();
    else on(video, "loadeddata", show);
  }

  return () => {
    dead = true;
    turn++;
    cancelAnimationFrame(raf);
    cancelAnimationFrame(rt);
    cancelAnimationFrame(playRaf);
    video.pause();
    timers.forEach(clearTimeout);
    cleanups.forEach((f) => f());
    io.disconnect();
    watch.disconnect();
    ro.disconnect();
    document.removeEventListener("visibilitychange", kick);
    scheme.removeEventListener("change", recolor);
    if (still) delete document.documentElement.dataset.still;
  };
}
