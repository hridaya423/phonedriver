export type Pt = [number, number];
export type Ribbon = { w: number; s: number; pts: Pt[] };
export type Band = { x: number; y: number; h: number; enter: number; twist: number; ink: [number, number, number]; clear: number[] };

const VS = `
attribute vec4 a0;attribute vec4 a1;attribute vec2 a2;
uniform vec2 res,off,c0,c1,c2;uniform float dpr,t,enter,twist,band;uniform vec3 mouse;
varying float vA,vG,vT;
float h(float n){return fract(sin(n)*43758.5453);}
float sd(vec2 p,vec2 a,vec2 b){vec2 pa=p-a,ba=b-a;return length(pa-ba*clamp(dot(pa,ba)/max(dot(ba,ba),1.0),0.0,1.0));}
void main(){
  vec2 c=a0.xy,n=a0.zw;float w=a1.x,u=a1.y,v=a1.z,seed=a1.w,rb=a2.x;
  float ph=rb*2.17;
  float tw=ph+u*(6.2+rb*1.7)+0.9*sin(u*3.1+t*0.19+ph)+twist*1.6;
  float th=(v-0.5)*2.5+tw;
  float s=sin(th),co=cos(th);
  float wob=sin(u*6.5-t*0.33+ph)*w*0.14;
  vec2 q=c+n*(w*s+wob);
  vec2 p=q+off;
  vec2 d=p-mouse.xy;float r=length(d)+0.001;
  float f=mouse.z*pow(max(0.0,1.0-r/190.0),2.0);
  p+=d/r*f*40.0;
  vT=f*0.25;
  float a=co>0.0?0.56+0.36*co:0.4+0.2*(-co);
  a+=(1.0-abs(co))*0.35;
  a*=smoothstep(0.0,0.06,u)*smoothstep(1.0,0.94,u);
  a*=clamp((enter*1.7-u-v*0.3)*4.0,0.0,1.0);
  a*=smoothstep(band*0.8,band*0.5,q.y)*smoothstep(-band*0.5,-band*0.1,q.y);
  a*=0.08+0.92*smoothstep(10.0,46.0,min(sd(q,c0,c1),sd(q,c1,c2)));
  vA=a*0.74;
  float flow=floor(u*520.0-t*2.4+seed*97.0);
  vG=floor(h(flow+v*131.0+rb*17.0)*4.0)+(co>0.5?4.0:0.0);
  gl_PointSize=(co>0.0?5.0+3.0*co:4.8)*dpr;
  gl_Position=vec4(p/res*2.0-1.0,0.0,1.0);gl_Position.y*=-1.0;
}`;

const FS = `precision mediump float;uniform sampler2D atlas;uniform vec3 ink,orc;varying float vA,vG,vT;
void main(){float m=texture2D(atlas,vec2((vG+gl_PointCoord.x)/8.0,gl_PointCoord.y)).a;float a=m*vA;if(a<0.01)discard;
vec3 col=mix(ink,orc,clamp(vT*1.6,0.0,1.0));gl_FragColor=vec4(col*a,a);}`;

const UNIFORMS = ["res", "off", "c0", "c1", "c2", "dpr", "t", "enter", "twist", "band", "mouse", "ink", "orc", "atlas"] as const;

export function spline(pts: Pt[], steps: number): Pt[] {
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

export function resample(line: Pt[], sp: number): Pt[] {
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

export function rgb(color: string): [number, number, number] {
  const c = document.createElement("canvas").getContext("2d")!;
  c.fillStyle = color;
  const h = String(c.fillStyle).replace("#", "");
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255) as [number, number, number];
}

/** The hero's ribbon of glyph points, drawn for any number of bands on one fixed canvas. */
export function createWeave(canvas: HTMLCanvasElement, family: string, orange: string) {
  const found = canvas.getContext("webgl", { antialias: false, alpha: true, premultipliedAlpha: true });
  if (!found) return null;
  const g: WebGLRenderingContext = found;
  const sh = (type: number, src: string) => {
    const s = g.createShader(type)!;
    g.shaderSource(s, src);
    g.compileShader(s);
    return s;
  };
  const prog = g.createProgram()!;
  g.attachShader(prog, sh(g.VERTEX_SHADER, VS));
  g.attachShader(prog, sh(g.FRAGMENT_SHADER, FS));
  g.linkProgram(prog);
  if (!g.getProgramParameter(prog, g.LINK_STATUS)) return null;
  g.useProgram(prog);
  const U = {} as Record<(typeof UNIFORMS)[number], WebGLUniformLocation | null>;
  for (const n of UNIFORMS) U[n] = g.getUniformLocation(prog, n);
  const A = (["a0", "a1", "a2"] as const).map((n) => g.getAttribLocation(prog, n));
  for (const l of A) g.enableVertexAttribArray(l);
  g.enable(g.BLEND);
  g.blendFunc(g.ONE, g.ONE_MINUS_SRC_ALPHA);
  g.uniform3fv(U.orc, rgb(orange));
  g.uniform1i(U.atlas, 0);

  const buffers: { buf: WebGLBuffer; count: number }[] = [];
  let W = 0, H = 0, dpr = 1, ready = false;

  document.fonts
    .load(`700 52px ${family}`)
    .catch(() => undefined)
    .then(() => {
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
      g.bindTexture(g.TEXTURE_2D, g.createTexture());
      g.pixelStorei(g.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
      g.texImage2D(g.TEXTURE_2D, 0, g.RGBA, g.RGBA, g.UNSIGNED_BYTE, c);
      g.texParameteri(g.TEXTURE_2D, g.TEXTURE_MIN_FILTER, g.LINEAR);
      g.texParameteri(g.TEXTURE_2D, g.TEXTURE_MAG_FILTER, g.LINEAR);
      g.texParameteri(g.TEXTURE_2D, g.TEXTURE_WRAP_S, g.CLAMP_TO_EDGE);
      g.texParameteri(g.TEXTURE_2D, g.TEXTURE_WRAP_T, g.CLAMP_TO_EDGE);
      ready = true;
    });

  return {
    size(w: number, h: number) {
      W = w;
      H = h;
      dpr = Math.min(devicePixelRatio || 1, 2);
      canvas.width = Math.round(W * dpr);
      canvas.height = Math.round(H * dpr);
      g.viewport(0, 0, canvas.width, canvas.height);
      g.uniform2f(U.res, W, H);
      g.uniform1f(U.dpr, dpr);
    },
    /** `ribbons` are in units of the band: x of its width, y of its height. */
    build(i: number, ribbons: Ribbon[], width: number, height: number) {
      const sp = width < 600 ? 4.2 : 3.6;
      const data: number[] = [];
      ribbons.forEach((rb, ri) => {
        const line = resample(spline(rb.pts.map(([x, y]) => [x * width, y * height] as Pt), 40), sp);
        const n = line.length;
        const strands = width < 600 ? Math.round(rb.s * 0.7) : rb.s;
        for (let j = 0; j < strands; j++) {
          const v = j / (strands - 1);
          const seed = Math.random();
          for (let k = 0; k < n; k++) {
            const a = line[Math.max(0, k - 1)], b = line[Math.min(n - 1, k + 1)];
            const tx = b[0] - a[0], ty = b[1] - a[1];
            const l = Math.hypot(tx, ty) || 1;
            const u = k / (n - 1);
            data.push(line[k][0], line[k][1], -ty / l, tx / l, rb.w * height * (0.55 + 0.45 * Math.sin(Math.PI * u)), u, v, seed, ri + i * 0.37, Math.random());
          }
        }
      });
      const arr = new Float32Array(data);
      const slot = (buffers[i] ??= { buf: g.createBuffer()!, count: 0 });
      slot.count = arr.length / 10;
      g.bindBuffer(g.ARRAY_BUFFER, slot.buf);
      g.bufferData(g.ARRAY_BUFFER, arr, g.STATIC_DRAW);
    },
    draw(bands: (Band | null)[], t: number, mouse: [number, number, number]) {
      g.clearColor(0, 0, 0, 0);
      g.clear(g.COLOR_BUFFER_BIT);
      if (!ready) return;
      g.uniform1f(U.t, t);
      g.uniform3f(U.mouse, mouse[0], mouse[1], mouse[2]);
      bands.forEach((b, i) => {
        const slot = buffers[i];
        if (!b || !slot || b.enter <= 0 || b.y > H || b.y + b.h * 1.1 < 0) return;
        g.bindBuffer(g.ARRAY_BUFFER, slot.buf);
        g.vertexAttribPointer(A[0], 4, g.FLOAT, false, 40, 0);
        g.vertexAttribPointer(A[1], 4, g.FLOAT, false, 40, 16);
        g.vertexAttribPointer(A[2], 2, g.FLOAT, false, 40, 32);
        g.uniform2f(U.off, b.x, b.y);
        g.uniform1f(U.enter, b.enter);
        g.uniform1f(U.twist, b.twist);
        g.uniform1f(U.band, b.h);
        g.uniform2f(U.c0, b.clear[0], b.clear[1]);
        g.uniform2f(U.c1, b.clear[2], b.clear[3]);
        g.uniform2f(U.c2, b.clear[4], b.clear[5]);
        g.uniform3fv(U.ink, b.ink);
        g.drawArrays(g.POINTS, 0, slot.count);
      });
    },
  };
}
