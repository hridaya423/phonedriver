"""Builds every brand image from the Sodium tokens in sodium.py. Text is converted to outlines.
Usage: python3 build.py [scratch dir for PNG renders]"""
import json
import pathlib
import subprocess
import sys

from sodium import *

ROOT = HERE.parent.parent
ASSETS = ROOT / ".github" / "assets"
LOGO = ROOT / "design" / "logo"
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


def banner(tn):
    c = THEMES[tn]
    W, H = 1280, 400
    b = ""
    top, bottom = 52, 351
    b += lockup(c, 48, top, 32, solid=True)[0]
    for i, s in enumerate(["Let your AI agent", "use a phone."]):
        t, _ = text(s, 48, 176 + 70 * i, INTER, 68, c["t1"], -0.04)
        b += t
    ry = 292
    b += rule(48, ry, 608, c["line"])
    for i, s in enumerate(["Android + iOS · CLI + MCP", f"Up to {headline(load_speed()[1])}× faster than adb"]):
        t, _ = mono(s, 48, ry + 20 + 28 * i + 6, 16, c["t2"], 0.05, caps=True)
        b += t
    gx = 656
    b += dots(gx, top, 32, bottom - top + 1, c["dot"], r=1)
    px, pw, ph = 688, 544, 299
    b += frame(px, top, pw, ph, c["line"], c["page"])
    b += rect(px + 1, top + 1, pw - 2, 56, c["panel"]) + rule(px + 1, top + 57, pw - 2, c["line"])
    cy = top + 29
    t, _ = mono("android:emulator-5554", px + 18, cy, 19.5, c["t1"])
    b += t
    t, w = mono("settled in 410ms", px + pw - 18, cy, 16, c["t2"], anchor="end")
    b += t
    b += status_mark(px + pw - 18 - w - 12 - 12, cy, 12, c["ok"])
    ry0 = top + 58
    rows = [
        [("@e9 ", c["t3"]), ("heading ", c["t2"]), ('"Display & touch"', c["t1"])],
        [("@e10 ", c["t1"]), ("switch ", c["t1"]), ('"Dark theme" ', c["t1"]), ("off", c["t1"])],
        [("@e11 ", c["t3"]), ("button ", c["t2"]), ('"Brightness level" ', c["t1"]), ('"64%"', c["t2"])],
        [("~ ", c["t2"]), ("@e10 ", c["t3"]), ("switch ", c["t2"]), ('"Dark theme" ', c["t1"]), ("off → ", c["t2"]), ("on", c["ok"])],
    ]
    for i, parts in enumerate(rows):
        yy = ry0 + 60 * i
        if i == 1:
            b += rect(px + 1, yy, pw - 2, 59, c["selected"])
        if i == 3:
            b += rect(px + 1, yy, pw - 2, 59, c["panel"])
        if i < 3:
            b += rule(px + 1, yy + 59, pw - 2, c["line"] if i == 2 else c["faint"])
        t, _ = runs(px + 18, yy + 30, 19.5, parts)
        b += t
    return svg(W, H, b, f"phone-use. Let your AI agent use a phone. Android and iOS, CLI and MCP, up to {headline(load_speed()[1])} times faster than adb. A snapshot line, @e10 switch Dark theme off, is tapped and the diff reads off to on.", c["page"])


def social():
    c = LIGHT
    W, H = 1280, 640
    b = ""
    b += lockup(c, 64, 64, 48, solid=True)[0]
    t, _ = text("Let your AI agent", 64, 256, INTER, 88, c["t1"], -0.04)
    b += t
    t, _ = text("use a phone.", 64, 346, INTER, 88, c["t1"], -0.04)
    b += t
    fx, fy, fw = 64, 400, 1152
    b += frame(fx, fy, fw, 176, c["line"], c["page"])
    b += rect(fx + 1, fy + 1, fw - 2, 46, c["panel"]) + rule(fx + 1, fy + 47, fw - 2, c["line"])
    t, _ = mono("phone-use against adb, p50", fx + 16, fy + 24, 16, c["t1"])
    b += t
    t, _ = mono("Android 17 emulator, median of 50 runs", fx + fw - 16, fy + 24, 14, c["t3"], anchor="end")
    b += t
    byid = {r["id"]: r for r in load_speed()[1]}
    cells = [(lab, ratio(byid[k]["vs_adb"]), k == "tree_list") for lab, k in
             (("Screenshot", "screenshot"), ("Snapshot, Settings", "tree_settings"),
              ("Snapshot, 1,000-row list", "tree_list"), ("Type 100 characters", "type100"), ("Tap, then what changed", "tap_diff"))]
    cw = (fw - 2) / 5
    for i, (lab, val, sel) in enumerate(cells):
        x = fx + 1 + i * cw
        if sel:
            b += rect(x, fy + 48, cw - (1 if i < 4 else 0), 127, c["selected"])
        if i < 4:
            b += vrule(x + cw - 1, fy + 48, 127, c["line"])
        t, _ = mono(lab, x + 16, fy + 48 + 28, 12, c["t2"], 0.05, caps=True)
        b += t
        t, w = text(val, x + 16, fy + 48 + 100, INTER, 56, c["t1"], -0.04)
        b += t
        t, _ = text("x", x + 16 + w + 2, fy + 48 + 100, INTER, 28, c["t2"], -0.02)
        b += t
    return svg(W, H, b, f"phone-use, {min(float(v.replace(',', '')) for _, v, _ in cells)} to {headline(load_speed()[1])} times faster than adb at p50", c["page"])


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
        pu, ph, adb = o["phone_use"], o["phone_harness"], o["adb"]
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
    return "Bar chart, log scale, p50 on an Android 17 emulator. phone-use against plain adb and phone-harness 0.3.3. " + "; ".join(parts) + "."


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
    for i, (name, col) in enumerate((("adb", "d3"), ("phone-harness", "d5"), ("phone-use", "d1"))):
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
        series = ((r["phone_use"], c["d1"], c["edge"], c["t1"], r.get("phone_use_p95")), (r["phone_harness"], c["d5"], None, c["t2"], r.get("phone_harness_p95")), (r["adb"], c["d3"], None, c["t2"], r.get("adb_p95")))
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
            t, _ = text("Only phone-use", px1 + 20, y + 58, INTER, 24, c["t1"], -0.03)
            b += t
            t, _ = mono("phone-harness: not supported", px1 + 20, y + 84, 12, c["t3"], 0.03)
            b += t
    return svg(W, H, b, speed_alt(rows), c["page"])


def sheet(tn):
    c = THEMES[tn]
    W, H = 1280, 736
    b = ""
    fx, fy, fw, fh = 24, 24, 1232, 688
    b += frame(fx, fy, fw, fh, c["line"], c["page"])
    b += rect(fx + 1, fy + 1, fw - 2, 45, c["panel"]) + rule(fx + 1, fy + 46, fw - 2, c["line"])
    t, _ = mono("phone-use brand", fx + 14, fy + 24, 16, c["t1"])
    b += t
    t, _ = mono("Sodium, light and dark", fx + fw - 14, fy + 24, 14, c["t3"], anchor="end")
    b += t
    y0 = fy + 47
    half = fw // 2
    b += vrule(fx + half, y0, 232, c["line"])
    b += lockup(c, fx + 48, y0 + 88, 96)[0]
    t, _ = mono("Lockup", fx + 14, y0 + 24, 13, c["t3"], 0.06, caps=True)
    b += t
    t, _ = mono("Mark at 16, 32, 64 and 128", fx + half + 14, y0 + 24, 13, c["t3"], 0.06, caps=True)
    b += t
    x = fx + half + 48
    for s in (16, 32, 64, 128):
        b += mark(s, c, x=x - s * 0.25, y=y0 + 56 + (128 - s))
        x += s * 0.5 + 56
    y1 = y0 + 232
    b += rule(fx + 1, y1, fw - 2, c["line"])
    t, _ = mono("Color", fx + 14, y1 + 24, 13, c["t3"], 0.06, caps=True)
    b += t
    sw = [("--page", "page"), ("--panel", "panel"), ("--well", "well"), ("--line", "line"), ("--text-1", "t1"),
          ("--text-2", "t2"), ("--selected", "selected"), ("--accent", "accent"), ("--accent-text", "atext"), ("--ok", "ok")]
    cw = (fw - 2) / 5
    for i, (nm, k) in enumerate(sw):
        col, row = i % 5, i // 5
        x = fx + 1 + col * cw
        y = y1 + 40 + row * 80
        b += frame(x + 14, y + 8, 40, 40, c["line"], c[k])
        t, _ = mono(nm, x + 66, y + 22, 13, c["t1"])
        b += t
        t, _ = mono(c[k], x + 66, y + 42, 13, c["t3"])
        b += t
    y2 = y1 + 40 + 160 + 8
    b += rule(fx + 1, y2, fw - 2, c["line"])
    b += vrule(fx + half, y2, fy + fh - 1 - y2, c["line"])
    t, _ = mono("Inter Display 400", fx + 14, y2 + 24, 13, c["t3"], 0.06, caps=True)
    b += t
    t, _ = mono("Commit Mono 400", fx + half + 14, y2 + 24, 13, c["t3"], 0.06, caps=True)
    b += t
    t, _ = text("Read the screen as text.", fx + 14, y2 + 92, INTER, 56, c["t1"], -0.04)
    b += t
    t, _ = text("Tap a ref. Get the diff.", fx + 14, y2 + 148, INTER, 32, c["t2"], -0.035)
    b += t
    t, _ = mono("One Rust binary", fx + half + 14, y2 + 72, 13, c["t2"], 0.05, caps=True)
    b += t
    b += runs(fx + half + 14, y2 + 104, 13, [("@e10 ", c["t3"]), ("switch ", c["t2"]), ('"Dark theme" ', c["t1"]), ("off", c["t1"])])[0]
    b += runs(fx + half + 14, y2 + 128, 13, [("~ ", c["atext"]), ("@e10 ", c["t3"]), ("switch ", c["t2"]), ('"Dark theme" ', c["t1"]), ("off → ", c["t2"]), ("on", c["ok"])])[0]
    b += runs(fx + half + 14, y2 + 152, 13, [("ok ", c["ok"]), ('tapped @e10 "Dark theme"', c["t1"]), (" · settled in 410ms", c["t2"])])[0]
    return svg(W, H, b, "phone-use brand sheet: the lockup, the mark at four sizes, ten Sodium colour tokens and the two typefaces Inter Display and Commit Mono.", c["page"])


def tile(size):
    c = DARK
    return svg(size, size, mark(size, c, ground="#161512", inset=True, ink="#f5f3ee"), "phone-use")


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
        write(ASSETS / f"banner-{tn}.svg", banner(tn))
        write(ASSETS / f"loop-{tn}.svg", loop(tn))
        write(ASSETS / f"speed-{tn}.svg", speed(tn))
        write(ASSETS / f"brand-{tn}.svg", sheet(tn))
    write(LOGO / "mark.svg", svg(16, 16, mark(16, LIGHT), "phone-use"))
    write(LOGO / "mark-dark.svg", svg(16, 16, mark(16, DARK), "phone-use"))
    write(LOGO / "mark-black.svg", svg(16, 16, mark(16, LIGHT, ink="#000000", solid=True), "phone-use"))
    write(LOGO / "mark-white.svg", svg(16, 16, mark(16, LIGHT, ink="#ffffff", solid=True), "phone-use"))
    for tn in THEMES:
        s, w = lockup(THEMES[tn], 4, 0, 32)
        write(LOGO / ("lockup.svg" if tn == "light" else "lockup-dark.svg"), svg(round(w) + 8, 32, s.replace('transform="translate(', 'transform="translate(', 1), "phone-use"))
    write(LOGO / "favicon.svg", tile(16))
    write(LOGO / "app-icon.svg", tile(512))
    soc = social()
    write(ASSETS / "social-preview.svg", soc)
    if len(sys.argv) > 1:
        scratch = pathlib.Path(sys.argv[1])
        shot(ASSETS / "social-preview.svg", ASSETS / "social-preview.png", 1280, 640)
        for n in (16, 32, 180, 512):
            write(scratch / f"tile-{n}.svg", tile(n))
            shot(scratch / f"tile-{n}.svg", LOGO / "png" / f"favicon-{n}.png", n, n)
    (ASSETS / "social-preview.svg").unlink()


if __name__ == "__main__":
    main()
