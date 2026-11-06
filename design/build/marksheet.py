"""Renders the three explored marks at 16, 32 and 128 px, light and dark. Usage: python3 marksheet.py out.png"""
import sys
import pathlib

from sodium import *
from build import shot


def opt(kind, size, c, x, y):
    k = size / 16
    st = 1 if size < 48 else 0.5
    out = []

    def R(a, b, w, h, fill):
        out.append(rect(round(x + a * k, 3), round(y + b * k, 3), round(w * k, 3), round(h * k, 3), fill))

    ink, ye = c["t1"], c["accent"]
    if kind in "AB":
        R(4, 0, 8, st, ink); R(4, 16 - st, 8, st, ink); R(4, 0, st, 16, ink); R(12 - st, 0, st, 16, ink)
    if kind == "A":
        R(6, 6, 4, 4, ye)
    if kind == "B":
        R(5, 5 + st, 6, 5 - st, ye); R(5, 5, 6, st, ink); R(5, 10, 6, st, ink)
    if kind == "C":
        R(0, 0, st, 16, ink); R(16 - st, 0, st, 16, ink)
        for xx in (0, 12):
            R(xx, 0, 4, st, ink); R(xx, 16 - st, 4, st, ink)
        R(5, 5, 6, 6, ye)
    return "".join(out)


names = {
    "A": "A  Frame and one cell",
    "B": "B  Frame, ruled rows, one on the wash (chosen)",
    "C": "C  Brackets and one cell",
}
W, H = 1280, 780
body = ""
for ti, (tn, c) in enumerate(THEMES.items()):
    x0 = ti * 640
    body += rect(x0, 0, 640, H, c["page"])
    for ri, kd in enumerate("ABC"):
        y0 = 24 + ri * 252
        t, _ = text(names[kd], x0 + 24, y0 + 16, MONO, 13, c["t2"], 0.05, caps=True)
        body += t + rect(x0 + 24, y0 + 28, 592, 1, c["line"])
        x = x0 + 24
        for s in (16, 32, 128):
            body += opt(kd, s, c, x, y0 + 52)
            t, _ = text(str(s), x, y0 + 52 + 128 + 28, MONO, 11, c["t3"], 0.06)
            body += t
            x += s + 56
open(pathlib.Path(sys.argv[1]).with_suffix(".svg"), "w").write(svg(W, H, body, "Three explored marks at 16, 32 and 128 pixels, light and dark"))
shot(pathlib.Path(sys.argv[1]).with_suffix(".svg"), pathlib.Path(sys.argv[1]), W, H)
pathlib.Path(sys.argv[1]).with_suffix(".svg").unlink()
