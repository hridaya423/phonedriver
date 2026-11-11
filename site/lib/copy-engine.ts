export function mountCopy(root: HTMLElement): () => void {
  const timers = new Map<HTMLElement, number>();
  const onClick = (e: MouseEvent) => {
    const btn = (e.target as HTMLElement).closest<HTMLElement>("[data-copy]");
    if (!btn) return;
    navigator.clipboard.writeText(btn.dataset.copy!).then(() => {
      const label = btn.querySelector<HTMLElement>("[data-label]") ?? btn;
      btn.dataset.was ??= label.textContent ?? "";
      label.textContent = "Copied";
      btn.dataset.done = "";
      clearTimeout(timers.get(btn));
      timers.set(
        btn,
        window.setTimeout(() => {
          label.textContent = btn.dataset.was!;
          delete btn.dataset.done;
        }, 2500),
      );
    });
  };
  root.addEventListener("click", onClick);
  return () => {
    root.removeEventListener("click", onClick);
    timers.forEach((t) => clearTimeout(t));
  };
}
