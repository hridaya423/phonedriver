type V3 = [number, number, number];
type X = { r: number[]; t: number[] };

/** One hand, in canvas axes (x right, y down, z toward the viewer): where the wrist is in px, which way the fingers and the palm face, and 21 joint angles in radians: thumb x, y, z, knuckle, tip, then spread and three curls for each of index, middle, ring and little finger. */
export type HandPose = { at: V3; fingers: V3; palm: V3; curl: number[] };
export type CapPose = { at: V3; turn: V3 };
/** A phone as a slab: where its centre is, how it is turned in degrees, and its width in px. */
export type SlabPose = { at: V3; turn: V3; size: number };
/** Whatever is left out is not drawn. */
export type Scene = { left?: HandPose; right?: HandPose; cap?: CapPose; phone?: SlabPose };
export type Stage = {
  w: number;
  h: number;
  dpr: number;
  cell: number;
  hand: number;
  cap: number;
  phone: { x: number; y: number; w: number; h: number; r: number };
  ink: V3;
  accent: V3;
  page: V3;
  inverse: boolean;
};
export type Figure = {
  layout(stage: Stage): void;
  goal(scene: Scene, rate?: number): void;
  snap(scene: Scene): void;
  reach(pose: HandPose, side: "left" | "right"): V3;
  pulse(x: number, y: number): void;
  drum(): void;
  enter(): void;
  draw(now: number, still?: boolean): void;
  destroy(): void;
};

const LEVELS = 10;
const ROWS = 10;
/** The figure's ink, lightest first: a blank, three dots (radius as a share of the cell), then six characters. Each material has two rows of characters to alternate between; rows 6 and 7 are the accent's. */
export const DOTS = [0.1, 0.16, 0.22];
export const RAMPS = [
  ["o", "c", "0", "8", "$", "@"],
  ["a", "e", "6", "9", "S", "&"],
  ["x", "x", "X", "#", "#", "@"],
  ["×", "+", "K", "H", "#", "@"],
  ["-", "+", "*", "#", "#", "@"],
  ["~", "+", "x", "#", "%", "@"],
  ["+", "+", "+", "+", "#", "#"],
  ["+", "+", "+", "+", "#", "#"],
  ["=", "e", "8", "8", "@", "@"],
  ["=", "c", "6", "9", "@", "@"],
];
const CHAIN = ["metacarpal", "phalanx-proximal", "phalanx-intermediate", "phalanx-distal", "tip"];
const FINGERS = ["index-finger", "middle-finger", "ring-finger", "pinky-finger"];
const RAD = Math.PI / 180;

const mul3 = (a: number[], b: number[]) => {
  const o = new Array<number>(9);
  for (let c = 0; c < 3; c++) for (let r = 0; r < 3; r++) o[c * 3 + r] = a[r] * b[c * 3] + a[3 + r] * b[c * 3 + 1] + a[6 + r] * b[c * 3 + 2];
  return o;
};
const app3 = (m: number[], v: number[]) => [m[0] * v[0] + m[3] * v[1] + m[6] * v[2], m[1] * v[0] + m[4] * v[1] + m[7] * v[2], m[2] * v[0] + m[5] * v[1] + m[8] * v[2]];
const rx = (a: number) => [1, 0, 0, 0, Math.cos(a), Math.sin(a), 0, -Math.sin(a), Math.cos(a)];
const ry = (a: number) => [Math.cos(a), 0, -Math.sin(a), 0, 1, 0, Math.sin(a), 0, Math.cos(a)];
const rz = (a: number) => [Math.cos(a), Math.sin(a), 0, -Math.sin(a), Math.cos(a), 0, 0, 0, 1];
const euler = (t: number[]) => mul3(rz(t[2] * RAD), mul3(ry(t[1] * RAD), rx(t[0] * RAD)));
const unit = (v: number[]) => {
  const l = Math.hypot(v[0], v[1], v[2]) || 1;
  return [v[0] / l, v[1] / l, v[2] / l];
};
const cross = (a: number[], b: number[]) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const MIRROR = [-1, 0, 0, 0, 1, 0, 0, 0, 1];
function turn(fingers: number[], palm: number[], mirror: boolean) {
  const sx = mirror ? -1 : 1;
  const f = unit([fingers[0] * sx, -fingers[1], fingers[2]]);
  const k = palm[0] * sx * f[0] - palm[1] * f[1] + palm[2] * f[2];
  const p = unit([palm[0] * sx - k * f[0], -palm[1] - k * f[1], palm[2] - k * f[2]]);
  const x = cross(p, f);
  return [x[0], x[1], x[2], -p[0], -p[1], -p[2], -f[0], -f[1], -f[2]];
}
function toQuat(m: number[]) {
  const t = m[0] + m[4] + m[8];
  let q: number[];
  if (t > 0) {
    const s = Math.sqrt(t + 1) * 2;
    q = [(m[5] - m[7]) / s, (m[6] - m[2]) / s, (m[1] - m[3]) / s, s / 4];
  } else if (m[0] > m[4] && m[0] > m[8]) {
    const s = Math.sqrt(1 + m[0] - m[4] - m[8]) * 2;
    q = [s / 4, (m[3] + m[1]) / s, (m[6] + m[2]) / s, (m[5] - m[7]) / s];
  } else if (m[4] > m[8]) {
    const s = Math.sqrt(1 + m[4] - m[0] - m[8]) * 2;
    q = [(m[3] + m[1]) / s, s / 4, (m[7] + m[5]) / s, (m[6] - m[2]) / s];
  } else {
    const s = Math.sqrt(1 + m[8] - m[0] - m[4]) * 2;
    q = [(m[6] + m[2]) / s, (m[7] + m[5]) / s, s / 4, (m[1] - m[3]) / s];
  }
  return q;
}
const qmul = (a: number[], b: number[]) => [
  a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
  a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
  a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
  a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
];

const mulX = (a: X, b: X): X => {
  const t = app3(a.r, b.t);
  return { r: mul3(a.r, b.r), t: [t[0] + a.t[0], t[1] + a.t[1], t[2] + a.t[2]] };
};
const invX = (a: X): X => {
  const r = [a.r[0], a.r[3], a.r[6], a.r[1], a.r[4], a.r[7], a.r[2], a.r[5], a.r[8]];
  const t = app3(r, a.t);
  return { r, t: [-t[0], -t[1], -t[2]] };
};
const quat = ([x, y, z, w]: number[]) => [1 - 2 * (y * y + z * z), 2 * (x * y + z * w), 2 * (x * z - y * w), 2 * (x * y - z * w), 1 - 2 * (x * x + z * z), 2 * (y * z + x * w), 2 * (x * z + y * w), 2 * (y * z - x * w), 1 - 2 * (x * x + y * y)];

type Rig = {
  count: number;
  rest: Float32Array;
  restN: Float32Array;
  joints: Uint8Array;
  weights: Float32Array;
  index: Uint16Array;
  ibm: X[];
  local: X[];
  parent: number[];
  order: number[];
  slot: Record<string, number>;
  extra: { pos: number[]; nrm: number[]; fade: number[]; index: number[] };
};

function parse(buf: ArrayBuffer): Rig {
  const dv = new DataView(buf);
  const jl = dv.getUint32(12, true);
  const json = JSON.parse(new TextDecoder().decode(new Uint8Array(buf, 20, jl)));
  const bin = 20 + jl + 8;
  const SIZE: Record<string, number> = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 };
  const acc = (i: number) => {
    const a = json.accessors[i], v = json.bufferViews[a.bufferView];
    const off = bin + (v.byteOffset ?? 0) + (a.byteOffset ?? 0), n = a.count * SIZE[a.type];
    if (a.componentType === 5126) return new Float32Array(buf.slice(off, off + n * 4));
    if (a.componentType === 5123) return new Uint16Array(buf.slice(off, off + n * 2));
    return new Uint8Array(buf.slice(off, off + n));
  };
  const prim = json.meshes[0].primitives[0];
  const skin = json.skins[0];
  const names: string[] = skin.joints.map((n: number) => json.nodes[n].name);
  const slot = Object.fromEntries(names.map((n, i) => [n, i]));
  const world: X[] = skin.joints.map((n: number) => ({ r: quat(json.nodes[n].rotation), t: json.nodes[n].translation }));
  const raw = acc(skin.inverseBindMatrices) as Float32Array;
  const ibm: X[] = names.map((_, j) => ({ r: [0, 1, 2, 4, 5, 6, 8, 9, 10].map((k) => raw[j * 16 + k]), t: [12, 13, 14].map((k) => raw[j * 16 + k]) }));
  const wrist = slot.wrist;
  const toWrist = invX(world[wrist]);
  const inWrist = world.map((w) => mulX(toWrist, w));
  const parent = names.map(() => -1);
  const order = [wrist];
  for (const f of ["thumb", ...FINGERS]) {
    let p = wrist;
    for (const part of CHAIN) {
      const j = slot[`${f}-${part}`];
      if (j === undefined) continue;
      parent[j] = p;
      order.push(j);
      p = j;
    }
  }
  const local = names.map((_, j) => (parent[j] < 0 ? { r: [1, 0, 0, 0, 1, 0, 0, 0, 1], t: [0, 0, 0] } : mulX(invX(inWrist[parent[j]]), inWrist[j])));
  const rest = acc(prim.attributes.POSITION) as Float32Array;
  const count = rest.length / 3;

  let far = -1e9;
  const seen: number[][] = [];
  for (let i = 0; i < count; i++) {
    const p = app3(toWrist.r, [rest[i * 3], rest[i * 3 + 1], rest[i * 3 + 2]]).map((v, k) => v + toWrist.t[k]);
    seen.push(p);
    far = Math.max(far, p[2]);
  }
  const rim = seen.filter((p) => p[2] > far - 0.012);
  const lo = [0, 1].map((k) => Math.min(...rim.map((p) => p[k]))), hi = [0, 1].map((k) => Math.max(...rim.map((p) => p[k])));
  const cx = (lo[0] + hi[0]) / 2, cy = (lo[1] + hi[1]) / 2, ax = (hi[0] - lo[0]) / 2, ay = (hi[1] - lo[1]) / 2;
  const extra = { pos: [] as number[], nrm: [] as number[], fade: [] as number[], index: [] as number[] };
  const RINGS = 18, SEG = 30;
  for (let i = 0; i < RINGS; i++) {
    const t = i / (RINGS - 1);
    const grow = 1.1 + 0.62 * Math.pow(t, 0.75);
    for (let s = 0; s < SEG; s++) {
      const a = (s / SEG) * Math.PI * 2;
      extra.pos.push(cx + ax * grow * Math.cos(a), cy + ay * grow * Math.sin(a), far - 0.008 + t * 0.15);
      const n = [Math.cos(a) / ax, Math.sin(a) / ay, -6];
      const l = Math.hypot(n[0], n[1], n[2]);
      extra.nrm.push(n[0] / l, n[1] / l, n[2] / l);
      extra.fade.push(i < 2 ? 1 : 0.86 * (1 - t));
      if (i < RINGS - 1) {
        const a0 = count + i * SEG + s, a1 = count + i * SEG + ((s + 1) % SEG);
        extra.index.push(a0, a1, a0 + SEG, a1, a1 + SEG, a0 + SEG);
      }
    }
  }
  return { count, rest, restN: acc(prim.attributes.NORMAL) as Float32Array, joints: acc(prim.attributes.JOINTS_0) as Uint8Array, weights: acc(prim.attributes.WEIGHTS_0) as Float32Array, index: acc(prim.indices) as Uint16Array, ibm, local, parent, order, slot, extra };
}

function flex(rig: Rig, curl: number[]): X[] {
  const turn: (number[] | undefined)[] = [];
  const s = rig.slot;
  turn[s["thumb-metacarpal"]] = mul3(rz(curl[2]), mul3(ry(curl[1]), rx(-curl[0])));
  turn[s["thumb-phalanx-proximal"]] = rx(-curl[3]);
  turn[s["thumb-phalanx-distal"]] = rx(-curl[4]);
  FINGERS.forEach((f, i) => {
    const k = 5 + i * 4;
    turn[s[`${f}-phalanx-proximal`]] = mul3(ry(curl[k]), rx(-curl[k + 1]));
    turn[s[`${f}-phalanx-intermediate`]] = rx(-curl[k + 2]);
    turn[s[`${f}-phalanx-distal`]] = rx(-curl[k + 3]);
  });
  const posed: X[] = [];
  for (const j of rig.order) {
    const p = rig.parent[j];
    let m = p < 0 ? rig.local[j] : mulX(posed[p], rig.local[j]);
    if (turn[j]) m = { r: mul3(m.r, turn[j]!), t: m.t };
    posed[j] = m;
  }
  return posed;
}

function capMesh() {
  const pos: number[] = [], nrm: number[] = [], mat: number[] = [], index: number[] = [];
  const row = (u: number) => PROFILE[Math.min(PROFILE.length - 1, Math.round(u * (PROFILE.length - 1)))];
  const PROFILE = [
    [1.0, 0.0, 0, 3, 1],
    [1.0, 0.26, 0, 3, 1],
    [1.02, 0.3, 0.03, 2, 0.92],
    [1.16, 0.46, 0.35, 2, 0.92],
    [1.36, 0.66, 0.75, 2, 0.92],
    [1.5, 0.8, 1, 2, 0.92],
    [1.56, 0.87, 1, 4, 1],
    [1.54, 0.93, 1, 4, 1],
    [1.4, 0.99, 1, 2, 0.92],
    [0.9, 1.04, 1, 2, 0.92],
    [0.0, 1.06, 1, 2, 0.92],
  ];
  const crown = (u: number, phi: number) => {
    const f = Math.min(PROFILE.length - 1.001, u * (PROFILE.length - 1)), i = Math.floor(f), k = f - i;
    const a = PROFILE[i], b = PROFILE[i + 1];
    const r = (a[0] + (b[0] - a[0]) * k) * (1 + 0.05 * Math.cos(phi) * (a[2] + (b[2] - a[2]) * k));
    const y = a[1] + (b[1] - a[1]) * k + (a[2] + (b[2] - a[2]) * k) * 0.36 * (0.5 + 0.5 * Math.cos(phi));
    return [r * Math.sin(phi), y, r * Math.cos(phi)];
  };
  const visor = (u: number, phi: number) => {
    const w = (phi - Math.PI) * 0.46;
    const r = 1 + u * 0.98 * Math.pow(Math.cos(w * 1.08), 0.7);
    return [r * Math.sin(w), -0.02 - 0.24 * Math.pow(u, 1.4) - 0.1 * u * (1 - Math.cos(w)), r * Math.cos(w)];
  };
  const sheet = (fn: (u: number, phi: number) => number[], nu: number, nv: number, tag: (u: number) => number[]) => {
    const base = pos.length / 3;
    for (let i = 0; i <= nu; i++)
      for (let j = 0; j <= nv; j++) {
        const u = i / nu, phi = (j / nv) * Math.PI * 2;
        const p = fn(u, phi), e = 0.004;
        const du = fn(Math.min(1, u + e), phi).map((v, k) => v - fn(Math.max(0, u - e), phi)[k]);
        const dp = fn(u, phi + e).map((v, k) => v - fn(u, phi - e)[k]);
        const n = [dp[1] * du[2] - dp[2] * du[1], dp[2] * du[0] - dp[0] * du[2], dp[0] * du[1] - dp[1] * du[0]];
        const l = Math.hypot(n[0], n[1], n[2]) || 1;
        pos.push(p[0], p[1], p[2]);
        nrm.push(n[0] / l, n[1] / l, n[2] / l);
        mat.push(...tag(u));
        if (i < nu && j < nv) {
          const a = base + i * (nv + 1) + j, b = a + nv + 1;
          index.push(a, a + 1, b, a + 1, b + 1, b);
        }
      }
  };
  sheet(crown, 60, 72, (u) => [row(u)[3], row(u)[4]]);
  sheet(visor, 12, 40, (u) => [4, 0.72 + 0.28 * Math.pow(u, 5)]);
  return { pos, nrm, mat, index };
}

function slabMesh() {
  const pos: number[] = [], nrm: number[] = [], mat: number[] = [], index: number[] = [];
  const W = 0.5, H = 1.04, R = 0.15, D = 0.05, N = 44;
  const ring = (inset: number) =>
    Array.from({ length: N }, (_, i) => {
      const a = (i / N) * Math.PI * 2, c = Math.cos(a), s = Math.sin(a), r = Math.max(0.02, R - inset);
      return [Math.sign(c) * (W - inset - r) + r * c, Math.sign(s) * (H - inset - r) + r * s, c, s];
    });
  const put = (x: number, y: number, z: number, n: number[], m: number[]) => {
    pos.push(x, y, z);
    nrm.push(n[0], n[1], n[2]);
    mat.push(m[0], m[1]);
    return pos.length / 3 - 1;
  };
  const strip = (a: number[], b: number[]) => {
    for (let i = 0; i < N; i++) {
      const j = (i + 1) % N;
      index.push(a[i], a[j], b[i], a[j], b[j], b[i]);
    }
  };
  const fan = (c: number, a: number[]) => {
    for (let i = 0; i < N; i++) index.push(c, a[i], a[(i + 1) % N]);
  };
  const out = ring(0), inn = ring(0.085);
  const EDGE = [3, 1], GLASS = [1, 0.62];
  strip(out.map((q) => put(q[0], q[1], D, [0, 0, 1], EDGE)), inn.map((q) => put(q[0], q[1], D, [0, 0, 1], EDGE)));
  fan(put(0, 0, D, [0, 0, 1], GLASS), inn.map((q) => put(q[0], q[1], D, [0, 0, 1], GLASS)));
  strip(out.map((q) => put(q[0], q[1], D, [q[2], q[3], 0], EDGE)), out.map((q) => put(q[0], q[1], -D, [q[2], q[3], 0], EDGE)));
  fan(put(0, 0, -D, [0, 0, -1], EDGE), out.map((q) => put(q[0], q[1], -D, [0, 0, -1], EDGE)));
  const bar = [[-0.15, -0.8], [0.15, -0.8], [-0.15, -0.74], [0.15, -0.74]].map((q) => put(q[0], q[1], D + 0.012, [0, 0, 1], EDGE));
  index.push(bar[0], bar[1], bar[2], bar[1], bar[3], bar[2]);
  return { pos, nrm, mat, index };
}

const SCENE_VS = `attribute vec3 p,n;attribute vec2 a;uniform mat3 rot;uniform vec3 at;uniform vec2 res;uniform float size;
varying vec3 vN;varying vec2 vA;varying float vZ;
void main(){vec3 w=rot*(p*size)+at;vN=rot*n;vA=a;vZ=w.z;gl_Position=vec4(w.x/res.x*2.0-1.0,w.y/res.y*2.0-1.0,-w.z/900.0,1.0);}`;
const SCENE_FS = `precision mediump float;varying vec3 vN;varying vec2 vA;varying float vZ;
void main(){vec3 N=normalize(vN);if(N.z<0.0)N=-N;
float d=max(0.0,dot(N,normalize(vec3(-0.52,0.7,0.5))));
float f=max(0.0,dot(N,normalize(vec3(0.75,-0.3,0.45))));
float lum=0.05+0.7*pow(d,1.2)+0.12*f+0.2*N.z*N.z;
if(vA.x>3.5)lum=0.6+0.4*lum;else if(vA.x>1.5&&vA.x<2.5)lum=0.34+0.66*lum;
float rim=pow(1.0-N.z,3.0);
gl_FragColor=vec4(clamp(lum,0.0,1.0),(vA.x+0.1+0.8*vA.y)/8.0,clamp(vZ/900.0+0.5,0.0,1.0),0.3+0.7*rim);}`;
const QUAD_VS = `attribute vec2 p;void main(){gl_Position=vec4(p,0.0,1.0);}`;
const ASCII_FS = `precision highp float;uniform sampler2D rt,atlas;uniform vec2 grid,res;uniform float cs,dpr,t,enter,inv;uniform vec4 phone;uniform float pr;uniform vec3 ink,acc,page,pulse;
float h(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}
float tone(vec4 v){float rm=(v.a-0.3)/0.7;return mix(v.r+rm*0.42*smoothstep(0.15,0.5,v.r),(1.0-v.r*0.9)*0.86+rm*0.3,floor(v.g*8.0)==3.0?0.0:inv)*pow(clamp((fract(v.g*8.0)-0.1)/0.8,0.0,1.0),1.3);}
float over(vec2 cell){vec4 v=texture2D(rt,(cell+0.5)/grid);return v.a<0.15||v.b<0.497?0.0:clamp((fract(v.g*8.0)-0.1)/0.8*3.0,0.0,1.0);}
void main(){vec2 px=gl_FragCoord.xy;vec2 c=floor(px/cs),f=fract(px/cs);
vec2 css=vec2(px.x,res.y*dpr-px.y)/dpr;
vec2 q=abs(css-phone.xy)-phone.zw+pr;float sd=length(max(q,0.0))+min(max(q.x,q.y),0.0)-pr;
float back=0.0;
if(sd<0.0&&enter>0.999){vec2 u=px/cs-0.5;vec2 i=floor(u),w=fract(u);
back=smoothstep(0.16,0.3,mix(mix(over(i),over(i+vec2(1.0,0.0)),w.x),mix(over(i+vec2(0.0,1.0)),over(i+vec2(1.0,1.0)),w.x),w.y));}
vec4 s=texture2D(rt,(c+0.5)/grid);
if(s.a<0.15||(sd<0.0&&s.b<0.497)){if(back<=0.0)discard;gl_FragColor=vec4(page*back,back);return;}
float occ=0.0,peers=0.0;
for(int i=0;i<4;i++){vec2 o=i==0?vec2(1.0,0.0):i==1?vec2(-1.0,0.0):i==2?vec2(0.0,1.0):vec2(0.0,-1.0);
vec4 nb=texture2D(rt,(c+o+0.5)/grid);if(nb.a>0.15){occ+=smoothstep(0.012,0.05,nb.b-s.b);peers+=step(4.0,pow(tone(nb),1.15)*float(${LEVELS}));}}
float mat=floor(s.g*8.0);
float k=h(c);
float d=tone(vec4(s.r*(1.0-0.42*min(1.0,occ)),s.gba))*clamp((enter*1.7-k*0.5-min(1.0,distance(css,phone.xy)/720.0)*0.6)*5.0,0.0,1.0);
float base=pow(d,1.15)*float(${LEVELS});float lv=base+(h(c+7.3)-0.5)*0.9;
lv=clamp(base<4.0||peers<1.0?min(lv,3.99):max(lv,4.0),0.0,float(${LEVELS})-0.01);
float level=floor(lv);if(level<1.0&&back<=0.0)discard;
float alt=step(0.5,fract(k*17.0+floor(t*0.35+k*9.0)*0.37));
float g=level<1.0?0.0:texture2D(atlas,vec2((level+f.x)/float(${LEVELS}),(mat*2.0+alt+1.0-f.y)/float(${ROWS}))).a;
vec3 col=mat==3.0?acc:ink;
float ring=pulse.z<0.7?exp(-pow((distance(css,pulse.xy)-pulse.z*300.0)/20.0,2.0))*(1.0-pulse.z/0.7):0.0;
col=mix(col,acc,clamp(ring*1.6,0.0,1.0));
gl_FragColor=vec4(mix(page*back,col,g),max(g,back));}`;
const BADGE_VS = `attribute vec3 p;attribute vec2 uv;uniform mat3 rot;uniform vec3 at;uniform vec2 res;uniform float size;varying vec2 vU;
void main(){vec3 w=rot*(p*size)+at;vU=uv;gl_Position=vec4(w.x/res.x*2.0-1.0,w.y/res.y*2.0-1.0,0.0,1.0);}`;
const BADGE_FS = `precision mediump float;uniform sampler2D tex;uniform float enter;varying vec2 vU;void main(){gl_FragColor=texture2D(tex,vU)*enter;}`;

type Body = { x: number[]; v: number[]; to: number[]; q: number[]; w: number[]; qt: number[]; rate: number; damp: number };
const targets = (s: Scene) => [
  s.left && { to: [...s.left.at, ...s.left.curl], qt: toQuat(turn(s.left.fingers, s.left.palm, false)) },
  s.right && { to: [...s.right.at, ...s.right.curl], qt: toQuat(turn(s.right.fingers, s.right.palm, true)) },
  s.cap && { to: [...s.cap.at, ...s.cap.turn], qt: [0, 0, 0, 1] },
  s.phone && { to: [...s.phone.at, ...s.phone.turn, s.phone.size], qt: [0, 0, 0, 1] },
];
const DAMP = [0.84, 0.84, 0.5, 0.7];

/** The chauffeur as glyphs: two rigged hands and a peaked cap are shaded into a grid one cell per character, and each cell is drawn as the character whose ink matches its light. The hand mesh is the WebXR generic hand (MIT, see public/assets/hand.LICENSE.txt). Resolves to null when WebGL or the mesh is unavailable. */
export async function mountFigure(canvas: HTMLCanvasElement, asset: string, family: string): Promise<Figure | null> {
  const context = canvas.getContext("webgl", { antialias: false, alpha: true, premultipliedAlpha: true });
  if (!context) return null;
  const gl: WebGLRenderingContext = context;
  let rig: Rig;
  try {
    const res = await fetch(asset);
    if (!res.ok) return null;
    rig = parse(await res.arrayBuffer());
  } catch {
    return null;
  }
  await document.fonts.load(`800 20px ${family}`).catch(() => undefined);

  const program = (vs: string, fs: string) => {
    const p = gl.createProgram()!;
    for (const [type, src] of [[gl.VERTEX_SHADER, vs], [gl.FRAGMENT_SHADER, fs]] as const) {
      const s = gl.createShader(type)!;
      gl.shaderSource(s, src);
      gl.compileShader(s);
      gl.attachShader(p, s);
    }
    gl.linkProgram(p);
    return gl.getProgramParameter(p, gl.LINK_STATUS) ? p : null;
  };
  const programs = [program(SCENE_VS, SCENE_FS), program(QUAD_VS, ASCII_FS), program(BADGE_VS, BADGE_FS)];
  if (programs.includes(null)) return null;
  const [scene, ascii, badge] = programs as WebGLProgram[];
  const U = (p: WebGLProgram, n: string) => gl.getUniformLocation(p, n);

  const total = rig.count + rig.extra.fade.length;
  const handIndex = new Uint16Array([...rig.index, ...rig.extra.index]);
  const handAttr = new Float32Array(total * 2);
  for (let i = 0; i < total; i++) {
    handAttr[i * 2] = i < rig.count ? 0 : 1;
    handAttr[i * 2 + 1] = i < rig.count ? 1 : rig.extra.fade[i - rig.count];
  }
  const buffer = (data: BufferSource, target: number = gl.ARRAY_BUFFER, usage: number = gl.STATIC_DRAW) => {
    const b = gl.createBuffer()!;
    gl.bindBuffer(target, b);
    gl.bufferData(target, data, usage);
    return b;
  };
  const hands = (["left", "right"] as const).map(() => ({
    pos: new Float32Array(total * 3),
    nrm: new Float32Array(total * 3),
    posB: buffer(new Float32Array(total * 3), gl.ARRAY_BUFFER, gl.DYNAMIC_DRAW),
    nrmB: buffer(new Float32Array(total * 3), gl.ARRAY_BUFFER, gl.DYNAMIC_DRAW),
  }));
  for (const hd of hands) {
    hd.pos.set(rig.extra.pos, rig.count * 3);
    hd.nrm.set(rig.extra.nrm, rig.count * 3);
  }
  const handAttrB = buffer(handAttr), handIndexB = buffer(handIndex, gl.ELEMENT_ARRAY_BUFFER);
  const cm = capMesh();
  const capPosB = buffer(new Float32Array(cm.pos)), capNrmB = buffer(new Float32Array(cm.nrm));
  const capAttrB = buffer(new Float32Array(cm.mat)), capIndexB = buffer(new Uint16Array(cm.index), gl.ELEMENT_ARRAY_BUFFER);
  const sm = slabMesh();
  const slabPosB = buffer(new Float32Array(sm.pos)), slabNrmB = buffer(new Float32Array(sm.nrm));
  const slabAttrB = buffer(new Float32Array(sm.mat)), slabIndexB = buffer(new Uint16Array(sm.index), gl.ELEMENT_ARRAY_BUFFER);
  const quadB = buffer(new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]));
  const BW = 0.17, BH = 0.29, BY = 0.6, BZ = 1.26;
  const badgeB = buffer(new Float32Array([-BW, BY - BH, BZ, 0, 1, BW, BY - BH, BZ, 1, 1, -BW, BY + BH, BZ + 0.1, 0, 0, BW, BY + BH, BZ + 0.1, 1, 0]));

  const rt = gl.createTexture()!, depth = gl.createRenderbuffer()!, fb = gl.createFramebuffer()!;
  const atlas = gl.createTexture()!, badgeTex = gl.createTexture()!;
  const nearest = (tex: WebGLTexture) => {
    gl.bindTexture(gl.TEXTURE_2D, tex);
    for (const [k, v] of [[gl.TEXTURE_MIN_FILTER, gl.NEAREST], [gl.TEXTURE_MAG_FILTER, gl.NEAREST], [gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE], [gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE]]) gl.texParameteri(gl.TEXTURE_2D, k, v);
  };

  let stage: Stage | null = null, cs = 12, cols = 1, rows = 1, enter0 = -1, last = 0, drum0 = -9, dead = false;
  let ring = { x: 0, y: 0, t0: -1e9 };
  const bodies: (Body | undefined)[] = [];

  function paint() {
    const s = stage!;
    const c = document.createElement("canvas");
    c.width = cs * LEVELS;
    c.height = cs * ROWS;
    const x = c.getContext("2d")!;
    x.fillStyle = "#fff";
    x.textAlign = "center";
    x.textBaseline = "middle";
    for (let row = 0; row < ROWS; row++) {
      const cy = row * cs + cs / 2;
      DOTS.forEach((r, i) => {
        x.beginPath();
        x.arc((i + 1) * cs + cs / 2, cy, r * cs + 0.25, 0, 7);
        x.fill();
      });
      RAMPS[row].forEach((ch, i) => {
        x.font = `${i > 3 ? 800 : 700} ${cs * 1.16}px ${family}`;
        x.fillText(ch, (i + 4) * cs + cs / 2, cy + cs * 0.06);
      });
    }
    nearest(atlas);
    gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, c);

    const b = document.createElement("canvas");
    b.width = 96;
    b.height = 160;
    const y = b.getContext("2d")!;
    const hex = (v: V3) => `rgb(${v.map((n) => Math.round(n * 255)).join(",")})`;
    y.fillStyle = hex(s.page);
    y.beginPath();
    y.roundRect(4, 4, 88, 152, 22);
    y.fill();
    y.strokeStyle = y.fillStyle = hex(s.accent);
    y.lineWidth = 11;
    y.beginPath();
    y.roundRect(19, 17, 58, 126, 13);
    y.stroke();
    y.beginPath();
    y.arc(48, 126, 6.5, 0, 7);
    y.fill();
    gl.bindTexture(gl.TEXTURE_2D, badgeTex);
    gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, true);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, b);
    for (const [k, v] of [[gl.TEXTURE_MIN_FILTER, gl.LINEAR], [gl.TEXTURE_MAG_FILTER, gl.LINEAR], [gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE], [gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE]]) gl.texParameteri(gl.TEXTURE_2D, k, v);
  }

  function layout(s: Stage) {
    stage = s;
    cs = Math.max(4, Math.round(s.cell * s.dpr));
    canvas.width = Math.round(s.w * s.dpr);
    canvas.height = Math.round(s.h * s.dpr);
    cols = Math.ceil(canvas.width / cs);
    rows = Math.ceil(canvas.height / cs);
    nearest(rt);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, cols, rows, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    gl.bindRenderbuffer(gl.RENDERBUFFER, depth);
    gl.renderbufferStorage(gl.RENDERBUFFER, gl.DEPTH_COMPONENT16, cols, rows);
    gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, rt, 0);
    gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.RENDERBUFFER, depth);
    paint();
  }

  function skin(i: number, curl: number[], now: number) {
    const hd = hands[i];
    const live = curl.slice();
    for (let f = 0; f < 4; f++) {
      const sway = Math.sin(now * 0.0011 + f * 0.9 + i * 2) * 0.022;
      const age = (now - drum0) / 1000 - (3 - f) * 0.075;
      const tap = i === 1 && age > 0 && age < 0.66 ? Math.sin((age % 0.33) * (Math.PI / 0.33)) * 0.3 : 0;
      live[5 + f * 4 + 1] += sway - tap;
      live[5 + f * 4 + 2] += sway * 0.7;
    }
    const posed = flex(rig, live).map((p, j) => (p ? mulX(p, rig.ibm[j]) : null));
    const { rest, restN, joints, weights, count } = rig;
    for (let v = 0; v < count; v++) {
      let px = 0, py = 0, pz = 0, nx = 0, ny = 0, nz = 0;
      const x = rest[v * 3], y = rest[v * 3 + 1], z = rest[v * 3 + 2], a = restN[v * 3], b = restN[v * 3 + 1], c = restN[v * 3 + 2];
      for (let k = 0; k < 4; k++) {
        const w = weights[v * 4 + k];
        if (!w) continue;
        const m = posed[joints[v * 4 + k]]!, r = m.r, t = m.t;
        px += w * (r[0] * x + r[3] * y + r[6] * z + t[0]);
        py += w * (r[1] * x + r[4] * y + r[7] * z + t[1]);
        pz += w * (r[2] * x + r[5] * y + r[8] * z + t[2]);
        nx += w * (r[0] * a + r[3] * b + r[6] * c);
        ny += w * (r[1] * a + r[4] * b + r[7] * c);
        nz += w * (r[2] * a + r[5] * b + r[8] * c);
      }
      hd.pos[v * 3] = px;
      hd.pos[v * 3 + 1] = py;
      hd.pos[v * 3 + 2] = pz;
      hd.nrm[v * 3] = nx;
      hd.nrm[v * 3 + 1] = ny;
      hd.nrm[v * 3 + 2] = nz;
    }
    gl.bindBuffer(gl.ARRAY_BUFFER, hd.posB);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, hd.pos);
    gl.bindBuffer(gl.ARRAY_BUFFER, hd.nrmB);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, hd.nrm);
  }

  const bind = (p: WebGLProgram, name: string, buf: WebGLBuffer, size: number, stride = 0, offset = 0) => {
    const l = gl.getAttribLocation(p, name);
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.enableVertexAttribArray(l);
    gl.vertexAttribPointer(l, size, gl.FLOAT, false, stride, offset);
  };

  function draw(now: number, still = false) {
    const s = stage;
    if (!s || dead || !bodies.some(Boolean)) return;
    if (enter0 < 0) enter0 = now;
    const dt = Math.min(0.034, Math.max(0.001, (now - last) / 1000));
    last = now;
    for (const b of bodies) {
      if (!b) continue;
      if (still) {
        b.x = b.to.slice();
        b.q = b.qt.slice();
        continue;
      }
      for (let i = 0; i < b.x.length; i++) {
        b.v[i] += (b.rate * b.rate * (b.to[i] - b.x[i]) - 2 * b.damp * b.rate * b.v[i]) * dt;
        b.x[i] += b.v[i] * dt;
      }
      let e = qmul(b.qt, [-b.q[0], -b.q[1], -b.q[2], b.q[3]]);
      if (e[3] < 0) e = e.map((v) => -v);
      const sin = Math.hypot(e[0], e[1], e[2]);
      const k = sin > 1e-6 ? (2 * Math.atan2(sin, e[3])) / sin : 2;
      for (let i = 0; i < 3; i++) b.w[i] += (b.rate * b.rate * e[i] * k - 2 * b.damp * b.rate * b.w[i]) * dt;
      const speed = Math.hypot(b.w[0], b.w[1], b.w[2]), half = (speed * dt) / 2, f = speed > 1e-9 ? Math.sin(half) / speed : dt / 2;
      const q = qmul([b.w[0] * f, b.w[1] * f, b.w[2] * f, Math.cos(half)], b.q);
      const l = Math.hypot(q[0], q[1], q[2], q[3]);
      b.q = q.map((v) => v / l);
    }
    const t = still ? 0 : now;
    const bob = still ? 0 : Math.sin(now * 0.0012) * 1.6;

    gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    gl.viewport(0, 0, cols, rows);
    gl.disable(gl.BLEND);
    gl.enable(gl.DEPTH_TEST);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.useProgram(scene);
    gl.uniform2f(U(scene, "res"), (cols * cs) / s.dpr, (rows * cs) / s.dpr);
    hands.forEach((hd, i) => {
      const body = bodies[i];
      if (!body) return;
      const x = body.x, r = quat(body.q);
      skin(i, x.slice(3), t);
      gl.uniformMatrix3fv(U(scene, "rot"), false, i ? mul3(MIRROR, r) : r);
      gl.uniform3f(U(scene, "at"), x[0], s.h - x[1] - (i ? bob * 0.6 : -bob), x[2]);
      gl.uniform1f(U(scene, "size"), s.hand);
      bind(scene, "p", hd.posB, 3);
      bind(scene, "n", hd.nrmB, 3);
      bind(scene, "a", handAttrB, 2);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, handIndexB);
      gl.drawElements(gl.TRIANGLES, handIndex.length, gl.UNSIGNED_SHORT, 0);
    });
    const hat = bodies[2]?.x;
    const cap = hat && { r: euler([hat[3], hat[4], hat[5] + (still ? 0 : Math.sin(now * 0.0009) * 1.1)]), t: [hat[0], s.h - hat[1] - bob, hat[2]] };
    if (cap) {
      gl.uniformMatrix3fv(U(scene, "rot"), false, cap.r);
      gl.uniform3f(U(scene, "at"), cap.t[0], cap.t[1], cap.t[2]);
      gl.uniform1f(U(scene, "size"), s.cap);
      bind(scene, "p", capPosB, 3);
      bind(scene, "n", capNrmB, 3);
      bind(scene, "a", capAttrB, 2);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, capIndexB);
      gl.drawElements(gl.TRIANGLES, cm.index.length, gl.UNSIGNED_SHORT, 0);
    }
    const slab = bodies[3]?.x;
    if (slab) {
      gl.uniformMatrix3fv(U(scene, "rot"), false, euler([slab[3], slab[4], slab[5]]));
      gl.uniform3f(U(scene, "at"), slab[0], s.h - slab[1] - bob * 0.6, slab[2]);
      gl.uniform1f(U(scene, "size"), slab[6]);
      bind(scene, "p", slabPosB, 3);
      bind(scene, "n", slabNrmB, 3);
      bind(scene, "a", slabAttrB, 2);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, slabIndexB);
      gl.drawElements(gl.TRIANGLES, sm.index.length, gl.UNSIGNED_SHORT, 0);
    }

    const enter = still ? 1 : Math.min(1, (now - enter0) / 1300);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.disable(gl.DEPTH_TEST);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(ascii);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, rt);
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, atlas);
    gl.uniform1i(U(ascii, "rt"), 0);
    gl.uniform1i(U(ascii, "atlas"), 1);
    gl.uniform2f(U(ascii, "grid"), cols, rows);
    gl.uniform2f(U(ascii, "res"), canvas.width / s.dpr, canvas.height / s.dpr);
    gl.uniform1f(U(ascii, "cs"), cs);
    gl.uniform1f(U(ascii, "dpr"), s.dpr);
    gl.uniform1f(U(ascii, "t"), t / 1000);
    gl.uniform1f(U(ascii, "enter"), enter);
    gl.uniform1f(U(ascii, "inv"), s.inverse ? 1 : 0);
    gl.uniform4f(U(ascii, "phone"), s.phone.x + s.phone.w / 2, s.phone.y + s.phone.h / 2, s.phone.w / 2, s.phone.h / 2);
    gl.uniform1f(U(ascii, "pr"), s.phone.r);
    gl.uniform3fv(U(ascii, "ink"), s.ink);
    gl.uniform3fv(U(ascii, "acc"), s.accent);
    gl.uniform3fv(U(ascii, "page"), s.page);
    gl.uniform3f(U(ascii, "pulse"), ring.x, ring.y, still ? 9 : (now - ring.t0) / 1000);
    bind(ascii, "p", quadB, 2);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);

    if (!cap) return;
    gl.useProgram(badge);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, badgeTex);
    gl.uniform1i(U(badge, "tex"), 0);
    gl.uniform1f(U(badge, "enter"), Math.max(0, enter * 2.5 - 1.5));
    gl.uniformMatrix3fv(U(badge, "rot"), false, cap.r);
    gl.uniform3f(U(badge, "at"), cap.t[0], cap.t[1], cap.t[2]);
    gl.uniform2f(U(badge, "res"), s.w, s.h);
    gl.uniform1f(U(badge, "size"), s.cap);
    bind(badge, "p", badgeB, 3, 20, 0);
    bind(badge, "uv", badgeB, 2, 20, 12);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  }

  return {
    layout,
    goal(sc, rate = 14) {
      targets(sc).forEach((t, i) => {
        const b = bodies[i];
        if (!t) bodies[i] = undefined;
        else if (b) Object.assign(b, t);
        else bodies[i] = { ...t, x: t.to.slice(), v: t.to.map(() => 0), q: t.qt.slice(), w: [0, 0, 0], rate: i === 2 ? 9 : 10, damp: DAMP[i] };
      });
      if (bodies[0]) bodies[0].rate = rate;
    },
    snap(sc) {
      targets(sc).forEach((t, i) => (bodies[i] = t ? { ...t, x: t.to.slice(), v: t.to.map(() => 0), q: t.qt.slice(), w: [0, 0, 0], rate: i === 2 ? 9 : 10, damp: DAMP[i] } : undefined));
    },
    reach(pose, side) {
      const tip = flex(rig, pose.curl)[rig.slot["index-finger-tip"]].t;
      const r = turn(pose.fingers, pose.palm, side === "right");
      const w = app3(side === "right" ? mul3(MIRROR, r) : r, tip.map((v) => v * stage!.hand));
      return [w[0], -w[1], w[2]];
    },
    pulse(x, y) {
      ring = { x, y, t0: performance.now() };
      if (bodies[1]) bodies[1].v[1] += 46;
    },
    enter() {
      enter0 = -1;
    },
    drum() {
      drum0 = performance.now();
    },
    draw,
    destroy() {
      dead = true;
      gl.getExtension("WEBGL_lose_context")?.loseContext();
    },
  };
}
