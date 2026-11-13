import { mountFigure } from "../lib/chauffeur-gl.ts";
import { bearerScene, bearerSize } from "../lib/figures.ts";

declare const POSTER: { glb: string; w: number; h: number; ink: [number, number, number]; page: [number, number, number]; cell: number };

(async () => {
  const canvas = document.querySelector("canvas")!;
  const fig = await mountFigure(canvas, POSTER.glb, "JB");
  if (!fig) throw new Error("no figure");
  const size = bearerSize(POSTER.w);
  fig.layout({ w: POSTER.w, h: POSTER.h, dpr: 2, cell: POSTER.cell, hand: size.hand, cap: size.cap, phone: { x: POSTER.w / 2, y: POSTER.h / 2, w: -2, h: -2, r: 0 }, ink: POSTER.ink, accent: [1, 0.357, 0.078], page: POSTER.page, inverse: POSTER.page[0] > 0.5 });
  fig.snap(bearerScene(POSTER.w, POSTER.h));
  const paint = () => {
    fig.draw(performance.now(), true);
    requestAnimationFrame(paint);
  };
  paint();
  document.title = "ready";
})();
