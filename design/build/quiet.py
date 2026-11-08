"""The real phone jobs chart (docs/bench/jobs.json) in the quiet hero style.
Called from build.py."""
import json

from sodium import *

ROOT = HERE.parent.parent
W = 1280
MX = 64


def mono(s, x, cy, size, fill, tr=0.0, anchor="start"):
    return text(s, x, cy + 0.35 * size, MONO, size, fill, tr, anchor)


def head(c, title, sub):
    t, _ = text(title, MX, 118, INTER, 84, c["t1"], -0.045)
    s, _ = text(sub, MX, 170, INTER, 30, c["t2"], -0.02)
    return t + s


def foot(c, y, s):
    t, _ = mono(s, MX, y, 20, c["t3"])
    return t


def bar(c, tn, x, y, w, frac, fill, h=14):
    track = c["faint"] if tn == "light" else c["line"]
    return rect(x, y, w, h, track) + rect(x, y, max(w * frac, 4), h, fill)


def short(n):
    if n >= 1_000_000:
        return f"{n / 1e6:.1f}M" if n < 1e7 else f"{n / 1e6:.0f}M"
    return f"{round(n / 1000):,}k"


def load_jobs():
    d = json.loads((ROOT / "docs" / "bench" / "jobs.json").read_text())["totals"]
    return {h: d[h] for h in ("phonedriver", "mobile-mcp", "phone-harness")}


def jobs(tn):
    c = THEMES[tn]
    t = load_jobs()
    pd = t["phonedriver"]
    runs = pd["runs"]
    b = head(c, f"{pd['passed']} of {runs} real jobs passed", "Nine jobs only a phone can do, two runs each, on an Android emulator")
    tmax = max(v["sum_median_tokens"] for v in t.values())
    smax = max(v["sum_median_wall_s"] for v in t.values())
    lab = 150
    bx = MX + lab
    bw = W - 2 * MX - lab - 230
    y = 252
    for name, v in t.items():
        fill = c["accent"] if name == "phonedriver" else c["d3"]
        s, _ = text(name, MX, y, INTER, 38, c["t1"], -0.03)
        b += s
        pw = width("passed", INTER, 30, -0.02)
        s, _ = text("passed", W - MX, y, INTER, 30, c["t2"], -0.02, anchor="end")
        b += s
        s, _ = text(f"{v['passed']}/{v['runs']}", W - MX - pw - 14, y + 2, INTER, 56, c["t1"], -0.045, anchor="end")
        b += s
        for i, (k, val, mx, txt) in enumerate((("tokens", v["sum_median_tokens"], tmax, f"{short(v['sum_median_tokens'])} tokens"),
                                              ("time", v["sum_median_wall_s"], smax, f"{v['sum_median_wall_s']:,.1f} s".replace(".0 s", " s")))):
            by = y + 38 + i * 46
            s, _ = mono(k, MX, by + 7, 24, c["t3"])
            b += s
            b += bar(c, tn, bx, by, bw, val / mx, fill)
            s, _ = mono(txt, bx + max(bw * val / mx, 4) + 14, by + 7, 24, c["t1"])
            b += s
        y += 190
    H = y - 190 + 190
    b += foot(c, H - 40, "sum of per-job medians · 9 jobs, 2 runs each · method: docs/bench/jobs.md")
    alt = ("Bar chart of nine real phone jobs, two runs each, on an Android emulator, sums of per-job medians. "
           + "; ".join(f"{n}: {v['passed']} of {v['runs']} runs passed, {v['sum_median_tokens']:,.0f} tokens, {v['sum_median_wall_s']:,.1f} seconds" for n, v in t.items())
           + ".")
    return svg(W, H, b, alt, c["page"])
