export function mountType(el: HTMLElement): () => void {
  const node = [...el.childNodes].find((n) => n.nodeType === Node.TEXT_NODE && n.textContent!.trim()) as Text;
  const text = node.data;
  let raf = 0;

  const run = () => {
    el.style.minHeight = `${el.offsetHeight}px`;
    el.dataset.typing = "";
    node.data = "";
    const t0 = performance.now();
    const length = text.length * 7;
    const frame = (now: number) => {
      const n = Math.min(text.length, Math.floor(((now - t0) / length) * text.length));
      node.data = text.slice(0, n);
      if (n < text.length) raf = requestAnimationFrame(frame);
      else {
        delete el.dataset.typing;
        el.style.minHeight = "";
      }
    };
    raf = requestAnimationFrame(frame);
  };

  el.addEventListener("life", run);
  return () => {
    cancelAnimationFrame(raf);
    el.removeEventListener("life", run);
    node.data = text;
    delete el.dataset.typing;
    el.style.minHeight = "";
  };
}
