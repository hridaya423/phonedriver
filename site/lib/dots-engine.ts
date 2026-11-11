const RATE = 0.15;

const short = (v: number) => (v >= 1e6 ? `${(v / 1e6).toFixed(1)}M` : `${Math.round(v / 1e3)}k`);

export function mountDots(el: HTMLElement): () => void {
  const unit = Number(el.dataset.unit);
  const big = el.closest("section")!.querySelector<HTMLElement>(".hm-big")!;
  const bigText = big.textContent!;
  const rows = [...el.querySelectorAll<HTMLElement>(".df-row")].map((row) => {
    const num = row.querySelector<HTMLElement>(".df-num b")!;
    return { dots: [...row.querySelectorAll<HTMLElement>(".df-field i")], num, text: num.textContent!, tokens: Number(num.dataset.tokens), lit: 0 };
  });
  const ours = rows[0].dots.length;
  const most = Math.max(...rows.map((r) => r.dots.length));
  let raf = 0;

  const rest = () => {
    delete el.dataset.run;
    big.textContent = bigText;
    for (const r of rows) {
      r.num.textContent = r.text;
      for (const d of r.dots) delete d.dataset.on;
    }
  };
  const run = () => {
    el.dataset.run = "";
    for (const r of rows) r.lit = 0;
    const t0 = performance.now();
    const frame = (now: number) => {
      const want = Math.floor((now - t0) * RATE);
      for (const r of rows) {
        const to = Math.min(r.dots.length, want);
        while (r.lit < to) r.dots[r.lit++].dataset.on = "";
        r.num.textContent = short(Math.min(r.lit * unit, r.tokens));
      }
      big.textContent = `${Math.max(1, Math.min(want, most) / ours).toFixed(1)}x`;
      if (want < most) raf = requestAnimationFrame(frame);
      else raf = window.setTimeout(rest, 400);
    };
    raf = requestAnimationFrame(frame);
  };

  el.addEventListener("life", run);
  return () => {
    cancelAnimationFrame(raf);
    clearTimeout(raf);
    el.removeEventListener("life", run);
    rest();
  };
}
