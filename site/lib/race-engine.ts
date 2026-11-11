export function mountRace(el: HTMLElement): () => void {
  const slow = Number(el.dataset.slow);
  const btn = el.querySelector<HTMLButtonElement>("button")!;
  const balls = [...el.querySelectorAll<HTMLElement>("[data-ms]")].map((dot) => ({ dot, trip: Number(dot.dataset.ms) * slow, run: 0 }));
  let raf = 0;
  let last = 0;
  let t = 0;
  let wanted = !matchMedia("(prefers-reduced-motion: reduce)").matches;
  let seen = false;

  const paint = () => {
    for (const b of balls) {
      const p = (t % b.trip) / b.trip;
      b.dot.style.transform = `translate3d(${(p < 0.5 ? p * 2 : 2 - p * 2) * b.run}px,0,0)`;
    }
  };
  const measure = () => {
    for (const b of balls) b.run = b.dot.parentElement!.clientWidth - b.dot.offsetWidth;
    paint();
  };
  const frame = (now: number) => {
    t += Math.min(now - last, 100);
    last = now;
    paint();
    raf = requestAnimationFrame(frame);
  };
  const sync = () => {
    btn.textContent = wanted ? "Pause" : "Play";
    if (wanted && seen && !raf) {
      last = performance.now();
      raf = requestAnimationFrame(frame);
    } else if (raf && !(wanted && seen)) {
      cancelAnimationFrame(raf);
      raf = 0;
    }
  };
  const toggle = () => {
    wanted = !wanted;
    sync();
  };

  const ro = new ResizeObserver(measure);
  ro.observe(el);
  const io = new IntersectionObserver(
    ([e]) => {
      seen = e.isIntersecting;
      sync();
    },
    { threshold: 0.15 },
  );
  io.observe(el);
  btn.hidden = false;
  btn.addEventListener("click", toggle);
  sync();
  return () => {
    cancelAnimationFrame(raf);
    ro.disconnect();
    io.disconnect();
    btn.removeEventListener("click", toggle);
    for (const b of balls) b.dot.style.transform = "";
  };
}
