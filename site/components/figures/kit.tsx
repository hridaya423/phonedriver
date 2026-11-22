"use client";

import { useEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { onFrame } from "@/lib/frame";

/** The frame every blog figure shares: a title row with its controls, the stage, and a caption that names the source. */
export function Fig({ id, title, controls, caption, children, tone }: { id: string; title: ReactNode; controls?: ReactNode; caption: ReactNode; children: ReactNode; tone?: "pane" }) {
  return (
    <figure className={tone === "pane" ? "fx fx-dark" : "fx"} data-figure={id}>
      <div className="fx-head">
        <span className="fx-t">{title}</span>
        {controls && <div className="fx-ctl">{controls}</div>}
      </div>
      <div className="fx-stage">{children}</div>
      <figcaption className="fx-cap">{caption}</figcaption>
    </figure>
  );
}

export function Seg<T extends string>({ label, options, value, onChange }: { label: string; options: readonly (readonly [T, ReactNode])[]; value: T; onChange: (v: T) => void }) {
  return (
    <div className="fx-seg" role="group" aria-label={label}>
      {options.map(([v, text]) => (
        <button type="button" key={v} className="fx-btn" aria-pressed={v === value} onClick={() => onChange(v)}>
          {text}
        </button>
      ))}
    </div>
  );
}

export function Btn({ children, onClick, pressed, primary }: { children: ReactNode; onClick: () => void; pressed?: boolean; primary?: boolean }) {
  return (
    <button type="button" className={primary ? "fx-btn fx-go" : "fx-btn"} aria-pressed={pressed} onClick={onClick}>
      {children}
    </button>
  );
}

export function useReduced(): boolean {
  const [reduced, set] = useState(false);
  useEffect(() => {
    const m = matchMedia("(prefers-reduced-motion: reduce)");
    set(m.matches);
    const on = () => set(m.matches);
    m.addEventListener("change", on);
    return () => m.removeEventListener("change", on);
  }, []);
  return reduced;
}

export function useSeen(ref: RefObject<Element | null>, threshold = 0.2): boolean {
  const [seen, set] = useState(false);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const io = new IntersectionObserver(([e]) => set(e.isIntersecting), { threshold });
    io.observe(el);
    return () => io.disconnect();
  }, [ref, threshold]);
  return seen;
}

/**
 * A clock for one figure: `tick(t, dt)` runs every frame with the milliseconds since the start, only while the figure is on
 * screen and `running`. `seek` moves it. Under reduced motion it calls `tick(still)` once per change of `key`, so the figure
 * shows a meaningful still.
 */
export function useClock(ref: RefObject<Element | null>, tick: (t: number, dt: number) => void, opts: { running?: boolean; still?: number; key?: unknown } = {}) {
  const { running = true, still = 0, key } = opts;
  const seen = useSeen(ref, 0.15);
  const reduced = useReduced();
  const t = useRef(0);
  const cb = useRef(tick);
  cb.current = tick;
  useEffect(() => {
    if (reduced) {
      cb.current(still, 0);
      return;
    }
    if (!seen || !running) return;
    let last = performance.now();
    cb.current(t.current, 0);
    return onFrame((now) => {
      const dt = Math.min(now - last, 64);
      last = now;
      t.current += dt;
      cb.current(t.current, dt);
    });
  }, [seen, running, reduced, still, key]);
  return {
    reduced,
    seen,
    seek(ms: number) {
      t.current = ms;
      if (!reduced) cb.current(ms, 0);
    },
  };
}

export const clamp = (v: number, lo = 0, hi = 1) => Math.min(hi, Math.max(lo, v));
export const ease = (x: number) => 1 - Math.pow(1 - clamp(x), 3);
export const easeInOut = (x: number) => {
  const v = clamp(x);
  return v < 0.5 ? 4 * v * v * v : 1 - Math.pow(-2 * v + 2, 3) / 2;
};
export const grouped = (v: number, digits = 0) => v.toLocaleString("en-US", { minimumFractionDigits: digits, maximumFractionDigits: digits });
