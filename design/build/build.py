"""Builds the README charts from the Sodium tokens in sodium.py. Text is converted to outlines.
The logo, banners, icons and social images come from site/scripts/brand.ts.
Usage: python3 build.py [any argument also renders hero-social.png]"""
import json
import subprocess
import sys

from sodium import *
import quiet

ROOT = HERE.parent.parent
ASSETS = ROOT / ".github" / "assets"
CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"


def mono(s, x, cy, size, fill, tr=0.0, anchor="start", caps=False):
    return text(s, x, cy + 0.35 * size, MONO, size, fill, tr, anchor, caps)


def runs(x, cy, size, parts, tr=0.0):
    out = ""
    for s, fill in parts:
        t, w = mono(s, x, cy, size, fill, tr)
        out += t
        x += w
    return out, x


def status_mark(x, cy, size, fill):
    return rect(x, cy - size / 2, size, size, fill)


def rule(x, y, w, color):
    return rect(x, y, w, 1, color)


def vrule(x, y, h, color):
    return rect(x, y, 1, h, color)


def loop(tn):
    c = THEMES[tn]
    W, H = 1280, 336
    b = ""
    fx, fy, fw, fh = 24, 24, 1232, 288
    b += frame(fx, fy, fw, fh, c["line"], c["page"])
    b += rect(fx + 1, fy + 1, fw - 2, 45, c["panel"]) + rule(fx + 1, fy + 46, fw - 2, c["line"])
    lws = [500, 340, 390]
    lxs = [fx + 1, fx + 1 + lws[0], fx + 1 + lws[0] + lws[1]]
    lanes = [("agent", "CLI or MCP"), ("daemon", ""), ("device", "android:emulator-5554")]
    for i, (n, m) in enumerate(lanes):
        x = lxs[i]
        t, _ = mono(n, x + 14, fy + 24, 18, c["t1"])
        b += t
        if m:
            t, _ = mono(m, x + lws[i] - 14, fy + 24, 15, c["t3"], anchor="end")
            b += t
        if i < 2:
            b += vrule(x + lws[i], fy + 1, fh - 2, c["line"])
    ry0 = fy + 47
    steps = [
        (0, [("tap @e10", c["t1"])], True),
        (1, [("re-locate @e10", c["t1"])], False),
        (2, [("tap", c["t1"])], False),
        (1, [("settled in 410ms", c["t1"])], False),
        (0, [("~ ", c["atext"]), ("@e10 ", c["t3"]), ("switch ", c["t2"]), ('"Dark theme" ', c["t1"]), ("off → ", c["t2"]), ("on", c["ok"])], False),
    ]
    pts = []
    for i, (lane, parts, sel) in enumerate(steps):
        yy = ry0 + 48 * i
        if sel:
            b += rect(lxs[lane], yy, lws[lane] - 1, 47, c["selected"])
        if i < 4:
            b += rule(fx + 1, yy + 47, fw - 2, c["faint"])
        pts.append((lxs[lane] + 19, yy + 24))
    for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
        if x1 < x0:
            d = f"M{x0 + 0.5} {y0 + 5}V{y0 + 17.5}H{x1 + 0.5}V{y1 - 5}"
        else:
            d = f"M{x0 + 0.5} {y0 + 5}V{y1 + 0.5}H{x1 - 5}"
        b += f'<path d="{d}" fill="none" stroke="{c["mark"]}" stroke-width="1" stroke-dasharray="3 3"/>'
    for i, ((x, y), (lane, parts, sel)) in enumerate(zip(pts, steps)):
        b += rect(x - 5, y - 5, 10, 10, c["ok"] if i == 4 else c["t1"])
        t, _ = runs(x + 19, y, 18, parts)
        b += t
    return svg(W, H, b, "The agent sends tap @e10 to the daemon. The daemon re-locates the ref and taps the device. The daemon waits until the screen has settled, then the diff comes back to the agent.", c["page"])


def load_speed():
    d = json.loads((ROOT / "docs" / "bench" / "speed.json").read_text())
    rows = []
    for o in d["ops"]:
        pu, ph, adb = o["phonedriver"], o["phone_harness"], o["adb"]
        rows.append(dict(o, vs_ph=(ph / pu if ph else None), vs_adb=adb / pu))
    rows.sort(key=lambda r: -r["vs_adb"])
    return d, rows


def ratio(v):
    if v >= 10:
        return f"{round(v):,}"
    return f"{v:.1f}"


def headline(rows):
    best = max(r.get("headline_adb", r["vs_adb"]) for r in rows)
    return ratio(best)


def speed_alt(rows):
    parts = []
    for r in rows:
        ph = f'phone-harness {ratio(r["vs_ph"])} times' if r["vs_ph"] else "phone-harness not supported"
        parts.append(f'{r["label"]} {ratio(r["vs_adb"])} times faster than adb ({ph})')
    return "Bar chart, log scale, p50 on an Android 17 emulator. phonedriver against plain adb and phone-harness 0.3.3. " + "; ".join(parts) + "."


def speed(tn):
    import math
    c = THEMES[tn]
    d, rows = load_speed()
    n = len(rows)
    W = 1280
    fx, fy, fw = 24, 24, 1232
    hh = 118
    rh = 104
    fh = hh + rh * n + 40 + 2
    H = fh + 48
    b = ""
    b += frame(fx, fy, fw, fh, c["line"], c["page"])
    b += rect(fx + 1, fy + 1, fw - 2, hh - 1, c["panel"]) + rule(fx + 1, fy + hh, fw - 2, c["line"])
    t, w = text("Up to ", fx + 20, fy + 72, INTER, 60, c["t1"], -0.04)
    b += t
    t, w2 = text(headline(rows) + "x", fx + 20 + w, fy + 72, INTER, 60, c["t1"], -0.04)
    b += rect(fx + 20 + w - 4, fy + 78, w2 + 8, 6, c["accent"]) + t
    t, _ = text(" faster than adb", fx + 20 + w + w2, fy + 72, INTER, 60, c["t1"], -0.04)
    b += t
    t, _ = mono(d["title"] + " · " + d["subtitle"], fx + 20, fy + 100, 15, c["t3"], 0.02)
    b += t
    rx = fx + fw - 20
    for i, (name, col) in enumerate((("adb", "d3"), ("phone-harness", "d5"), ("phonedriver", "d1"))):
        t, w = mono(name, rx, fy + 100, 15, c["t2"], 0.04, anchor="end", caps=True)
        b += t + rect(rx - w - 10 - 12, fy + 100 - 5, 10, 10, c[col])
        rx = rx - w - 10 - 12 - 24
    ratw, facw = 250, 250
    px0, px1 = fx + ratw, fx + fw - facw
    y0 = fy + hh + 1
    b += vrule(px0, y0, fh - hh - 2, c["line"]) + vrule(px1, y0, fh - hh - 2, c["line"])
    lo, hi = 10, 10000
    X = lambda ms: px0 + 20 + (math.log10(ms) - 1) / 3 * (px1 - px0 - 40 - 100)
    ay = y0 + rh * n
    b += rule(fx + 1, ay, fw - 2, c["line"])
    for ms, lab in ((10, "10"), (100, "100"), (1000, "1,000"), (10000, "10,000")):
        x = round(X(ms))
        b += vrule(x, y0, rh * n, c["faint"])
        t, _ = mono(lab + " ms", x, ay + 21, 14, c["t3"], 0.04, anchor="start" if ms == 10 else "middle")
        b += t
    t, _ = mono("p50 bar, p95 tick, log", fx + 20, ay + 21, 14, c["t3"], 0.05, caps=True)
    b += t
    t, _ = mono("vs phone-harness", px1 + 20, ay + 21, 14, c["t3"], 0.05, caps=True)
    b += t
    for i, r in enumerate(rows):
        y = y0 + rh * i
        win = r["vs_ph"] is None or r["vs_ph"] >= 1
        if i == 0:
            b += rect(fx + 1, y, ratw - 1, rh - 1, c["selected"])
        if i < n - 1:
            b += rule(fx + 1, y + rh - 1, fw - 2, c["faint"])
        t, w = text(ratio(r["vs_adb"]), fx + 20, y + 66, INTER, 56, c["t1"], -0.04)
        b += t
        t, _ = text("x", fx + 20 + w + 2, y + 66, INTER, 30, c["t2"], -0.02)
        b += t
        t, _ = mono("faster than adb" if r["vs_adb"] >= 1 else "adb is faster", fx + 20, y + 88, 12, c["t3"], 0.03)
        b += t
        t, _ = mono(r["label"], px0 + 20, y + 16, 14, c["t1"], 0.03, caps=True)
        b += t
        x0 = round(X(10))
        series = ((r["phonedriver"], c["d1"], c["edge"], c["t1"], r.get("phonedriver_p95")), (r["phone_harness"], c["d5"], None, c["t2"], r.get("phone_harness_p95")), (r["adb"], c["d3"], None, c["t2"], r.get("adb_p95")))
        for j, (v, col, ed, tc, p95) in enumerate(series):
            by = y + 28 + j * 24
            if v is None:
                t, _ = mono("not supported", x0, by + 8, 13, c["t3"])
                b += t
                continue
            x1 = round(X(v))
            b += rect(x0, by, x1 - x0, 16, col)
            if ed:
                b += frame(x0, by, x1 - x0, 16, ed)
            lx = x1 + 10
            if p95 and p95 > v:
                xt = round(X(p95))
                b += rect(xt, by - 2, 1, 20, c["t2"])
                lx = xt + 10
            t, _ = mono(f"{v:,.1f} ms", lx, by + 8, 14, tc)
            b += t
        if r["vs_ph"] is not None:
            t, w = text(ratio(r["vs_ph"]), px1 + 20, y + 62, INTER, 40, c["t1"], -0.035)
            b += t
            t, _ = text("x", px1 + 20 + w + 2, y + 62, INTER, 24, c["t2"], -0.02)
            b += t
            t, _ = mono("faster than phone-harness" if r["vs_ph"] >= 1 else "phone-harness is faster", px1 + 20, y + 84, 12, c["t3"], 0.03)
            b += t
        else:
            t, _ = text("Only phonedriver", px1 + 20, y + 58, INTER, 24, c["t1"], -0.03)
            b += t
            t, _ = mono("phone-harness: not supported", px1 + 20, y + 84, 12, c["t3"], 0.03)
            b += t
    return svg(W, H, b, speed_alt(rows), c["page"])


HERO_ROWS = (("tree_list", "Read the screen", "1,000-row UI tree"), ("screenshot", "Screenshot", ""),
             ("type100", "Type 100 characters", ""), ("tap_diff", "Tap and see what changed", "adb: tap, dump, diff"))


def hero_rows():
    d, rows = load_speed()
    byid = {r["id"]: r for r in rows}
    return d, rows, [(byid[k], lab, sub) for k, lab, sub in HERO_ROWS]


def hero_alt(rows, picked):
    parts = []
    for r, lab, sub in picked:
        parts.append(f'{lab.lower() if lab != "Screenshot" else "screenshot"}{" (" + sub + ")" if sub and not sub.startswith("adb") else ""}: '
                     f'{r["phonedriver"]:,.1f} ms against {r["adb"]:,.1f} ms for adb, {ratio(r["vs_adb"])} times faster')
    slower = [f'{ratio(r["vs_ph"])}' for r, _, _ in picked if r["vs_ph"]]
    unsup = [lab.lower() for r, lab, _ in picked if not r["vs_ph"]]
    tail = f' phone-harness is {", ".join(slower)} times slower than phonedriver on the first {len(slower)} rows' + (f' and does not support {unsup[0]}.' if unsup else ".")
    return (f"Bar chart of phonedriver against adb at p50 on an Android emulator, up to {headline(rows)} times faster. "
            + "; ".join(parts) + "." + tail)


def hero(tn, W=1280, H=None, social=False):
    c = THEMES[tn]
    d, rows, picked = hero_rows()
    k = 0.8 if social else 1.0
    mx = round(64 * k)
    bw = W - 2 * mx
    b = ""
    y = round(118 * k)
    t, _ = text(f"Up to {headline(rows)}\u00d7 faster than adb", mx, y, INTER, 84 * k, c["t1"], -0.045)
    b += t
    t, _ = text("How long an AI agent waits on an Android phone", mx, y + 52 * k, INTER, 30 * k, c["t2"], -0.02)
    b += t
    y += round(130 * k)
    track = round(10 * k)
    for r, lab, _ in picked:
        t, _ = text(lab, mx, y, INTER, 38 * k, c["t1"], -0.03)
        b += t
        t, _ = text(ratio(r["vs_adb"]) + "\u00d7", W - mx, y + 2 * k, INTER, 56 * k, c["t1"], -0.045, anchor="end")
        b += t
        by = y + round(26 * k)
        pw = max(bw * r["phonedriver"] / r["adb"], 4)
        b += rect(mx, by, bw, track, c["faint"] if tn == "light" else c["line"])
        b += rect(mx, by, pw, track, c["accent"])
        ly = by + track + round(24 * k)
        t, _ = mono(f'phonedriver {r["phonedriver"]:,.0f} ms', mx, ly, 26 * k, c["t1"])
        b += t
        t, _ = mono(f'adb {r["adb"]:,.0f} ms', W - mx, ly, 26 * k, c["t3"], anchor="end")
        b += t
        y = ly + round(84 * k)
    t, _ = mono("p50 of 50 runs \u00b7 Android 17 emulator \u00b7 method and raw data: docs/bench/speed.md", mx, y - 22 * k, 20 * k, c["t3"])
    b += t
    H = H or round(y + 8 * k)
    return svg(W, H, b, hero_alt(rows, picked), c["page"]), W, H


def write(path, s):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(s)


def shot(svgp, pngp, w, h):
    from PIL import Image
    pngp.parent.mkdir(parents=True, exist_ok=True)
    html = pngp.with_suffix(".html")
    html.write_text(f"<!doctype html><meta charset=utf-8><body style='margin:0'><img src='file://{svgp}' width={w} height={h} style='display:block'></body>")
    tmp = pngp.with_suffix(".full.png")
    subprocess.run([CHROME, "--headless", "--hide-scrollbars", "--force-device-scale-factor=1", f"--window-size={max(w, 800)},{max(h, 600)}",
                    f"--screenshot={tmp}", f"file://{html}"], capture_output=True)
    Image.open(tmp).convert("RGBA").crop((0, 0, w, h)).save(pngp)
    html.unlink()
    tmp.unlink()


def main():
    for tn in THEMES:
        write(ASSETS / f"loop-{tn}.svg", loop(tn))
        write(ASSETS / f"speed-{tn}.svg", speed(tn))
        write(ASSETS / f"hero-{tn}.svg", hero(tn)[0])
        write(ASSETS / f"jobs-{tn}.svg", quiet.jobs(tn))
    write(ASSETS / "hero-social.svg", hero("light", 1200, 675, social=True)[0])
    if len(sys.argv) > 1:
        shot(ASSETS / "hero-social.svg", ASSETS / "hero-social.png", 1200, 675)
    (ASSETS / "hero-social.svg").unlink()


if __name__ == "__main__":
    main()
