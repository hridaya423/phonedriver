"use client";

import { useEffect, useRef, type CSSProperties } from "react";

export function FootMark({ svg, sweep, view }: { svg: string; sweep: number; view: string }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    el.dataset.arm = "";
    const io = new IntersectionObserver(
      ([e]) => {
        if (!e.isIntersecting) return;
        el.dataset.in = "";
        io.disconnect();
      },
      { threshold: 0.35 },
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);
  return (
    <div ref={ref} className="fmark" aria-hidden="true" style={{ "--sweep": `${sweep}px` } as CSSProperties}>
      <svg viewBox={view} dangerouslySetInnerHTML={{ __html: svg }} />
    </div>
  );
}
