"""Tokens-per-task chart from docs/bench/tokens.json. Separate from build.py so the speed chart is untouched.
Usage: python3 tokens_chart.py [png path]"""
import json
import pathlib
import subprocess
import sys

from sodium import *

ROOT = HERE.parent.parent
ASSETS = ROOT / ".github" / "assets"
CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
HARNESSES = (("phone-use", "d1"), ("mobile-mcp", "d2"), ("phone-harness", "d5"), ("adb", "d3"))


def mono(s, x, cy, size, fill, tr=0.0, anchor="start", caps=False):
    return text(s, x, cy + 0.35 * size, MONO, size, fill, tr, anchor, caps)


def chart(tn, rows):
    c = THEMES[tn]
    tasks = []
    for r in rows:
        if r["task"] not in tasks:
            tasks.append(r["task"])
    n = len(tasks)
    W = 1280
    fx, fy, fw = 24, 24, 1232
    rh = 130
    fh = 46 + rh * n + 38 + 2
    H = fh + 48
    b = frame(fx, fy, fw, fh, c["line"], c["page"])
    b += rect(fx + 1, fy + 1, fw - 2, 45, c["panel"]) + rect(fx + 1, fy + 46, fw - 2, 1, c["line"])
    t, _ = mono("Tokens per task", fx + 14, fy + 24, 18, c["t1"])
    b += t
    rx = fx + fw - 14
    for name, col in reversed(HARNESSES):
        t, w = mono(name, rx, fy + 24, 15, c["t2"], 0.04, anchor="end", caps=True)
        b += t + rect(rx - w - 10 - 12, fy + 24 - 5, 10, 10, c[col])
        rx = rx - w - 10 - 12 - 24
    labw = 264
    px0, px1 = fx + labw, fx + fw - 16
    b += rect(px0, fy + 47, 1, fh - 48, c["line"])
    vmax = max(r["total_tokens"] for r in rows)
    step = 50000 if vmax < 400000 else 100000
    top = (int(vmax / step) + 1) * step
    X = lambda v: px0 + 16 + v / top * (px1 - px0 - 32 - 190)
    ay = fy + 46 + rh * n
    b += rect(fx + 1, ay, fw - 2, 1, c["line"])
    for v in range(0, top + 1, step):
        x = round(X(v))
        if v:
            b += rect(x, fy + 47, 1, rh * n, c["faint"])
        t, _ = mono(f"{v // 1000}k", x, ay + 20, 14, c["t3"], 0.04, anchor="start" if v == 0 else "middle")
        b += t
    t, _ = mono("Input + output + cache, one run each", fx + 14, ay + 20, 13, c["t3"], 0.05, caps=True)
    b += t
    for i, task in enumerate(tasks):
        y = fy + 47 + rh * i
        if i < n - 1:
            b += rect(fx + 1, y + rh - 1, fw - 2, 1, c["faint"])
        t, _ = mono(task, fx + 14, y + rh / 2 - 1, 15, c["t1"], 0.03, caps=True)
        b += t
        x0 = round(X(0))
        for j, (name, col) in enumerate(HARNESSES):
            r = next((r for r in rows if r["task"] == task and r["harness"] == name), None)
            by = y + 12 + j * 28
            if r is None:
                t, _ = mono("not run", x0 + 6, by + 10, 14, c["t3"])
                b += t
                continue
            x1 = max(round(X(r["total_tokens"])), x0 + 2)
            b += rect(x0, by, x1 - x0, 20, c[col])
            if name == "phone-use":
                b += frame(x0, by, x1 - x0, 20, c["edge"])
            label = f"{r['total_tokens']:,}  {r['tool_calls']} calls"
            if not r["success"]:
                label += "  failed"
            t, _ = mono(label, x1 + 10, by + 10, 15, c["t1"] if r["success"] else c["fail"])
            b += t
    return svg(W, H, b, "Tokens per task for four phone-control harnesses, claude-sonnet-5-5, one run each", c["page"])


def shot(svgp, pngp, w, h):
    html = pngp.with_suffix(".html")
    html.write_text(f"<!doctype html><meta charset=utf-8><body style='margin:0'><img src='file://{svgp}' width={w} height={h} style='display:block'></body>")
    subprocess.run([CHROME, "--headless", "--hide-scrollbars", "--force-device-scale-factor=1", f"--window-size={w},{h}",
                    f"--screenshot={pngp}", f"file://{html}"], check=True, capture_output=True)
    html.unlink()


if __name__ == "__main__":
    rows = json.loads((ROOT / "docs" / "bench" / "tokens.json").read_text())
    for tn in THEMES:
        p = ASSETS / f"tokens-{tn}.svg"
        s = chart(tn, rows)
        p.write_text(s)
        if tn == "light" and len(sys.argv) > 1:
            import re
            m = re.search(r'height="(\d+)"', s)
            shot(p, pathlib.Path(sys.argv[1]), 1280, int(m.group(1)))
