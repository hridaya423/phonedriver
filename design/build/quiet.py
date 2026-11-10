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


def minutes(sec):
    return f"{sec / 60:.1f} min"


def jobs(tn):
    c = THEMES[tn]
    t = load_jobs()
    pd = t["phonedriver"]
    fold = t["mobile-mcp"]["sum_median_tokens"] / pd["sum_median_tokens"]
    b = head(c, f"{fold:.1f}x fewer tokens, same jobs",
             "Nine jobs only a phone can do · Claude Sonnet 5.5 · one Android emulator · 10 Nov 2026")
    tmax = max(v["sum_median_tokens"] for v in t.values())
    bw = W - 2 * MX
    y = 262
    for name, v in t.items():
        fill = c["accent"] if name == "phonedriver" else c["d3"]
        s, _ = text(name, MX, y, INTER, 38, c["t1"], -0.03)
        b += s
        s, _ = text(f"{short(v['sum_median_tokens'])} tokens", W - MX, y + 4, INTER, 52, c["t1"], -0.045, anchor="end")
        b += s
        b += bar(c, tn, MX, y + 30, bw, v["sum_median_tokens"] / tmax, fill, h=18)
        s, _ = mono(f"{v['passed']}/{v['runs']} passed · {minutes(v['sum_median_wall_s'])}", MX, y + 78, 24, c["t2"])
        b += s
        y += 172
    H = y + 40
    b += foot(c, H - 40, "sums of per-job medians · a script checks the phone, not the reply · docs/bench/jobs.md")
    alt = (f"Bar chart: on nine real phone jobs, on one Android emulator on 10 November 2026, phonedriver used {fold:.1f} times fewer tokens than mobile-mcp. "
           + "; ".join(f"{n}: {v['sum_median_tokens']:,.0f} tokens, {v['passed']} of {v['runs']} runs passed, {v['sum_median_wall_s']:,.1f} seconds" for n, v in t.items())
           + ". Sums of per-job medians, Claude Sonnet 5.5, Android emulator.")
    return svg(W, H, b, alt, c["page"])
