type Tick = (now: number) => void;

const ticks = new Set<Tick>();
let raf = 0;

function loop(now: number) {
  raf = 0;
  for (const tick of ticks) tick(now);
  if (ticks.size) raf = requestAnimationFrame(loop);
}

/** One animation loop for everything below the hero. The returned function unsubscribes. */
export function onFrame(tick: Tick): () => void {
  ticks.add(tick);
  if (!raf) raf = requestAnimationFrame(loop);
  return () => {
    ticks.delete(tick);
  };
}

let offset = -1;

/** The page's scroll offset, kept from scroll events: reading `scrollY` in a frame that has already written to the DOM forces a layout. */
export function scrolled(): number {
  if (offset < 0) {
    offset = scrollY;
    addEventListener("scroll", () => (offset = scrollY), { passive: true });
  }
  return offset;
}
