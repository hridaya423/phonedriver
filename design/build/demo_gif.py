"""Builds .github/assets/demo.gif from a run recorded by demo_agent.py: the Simulator video on the right,
the prompt and the phone-use tool calls Claude Code made, at their real times, in a Sodium dark terminal on
the left. Idle time between calls is trimmed. Long results are clipped and marked with a count of the lines left out.
The first frame is the finished state, so a static preview shows the result.
Usage: python3 demo_gif.py <demo dir with screen.mov and agent.json> <out.gif>"""
import json
import re
import subprocess
import textwrap
import sys
import tempfile

import numpy as np
from pathlib import Path

from fontTools.ttLib import TTFont
from PIL import Image, ImageDraw, ImageFont

from sodium import DARK as C, HERE

W, H = 960, 540
HEAD = 38
PHONE_W = 260
FPS_OUT, FPS_SRC = 10, 20
FS, LH = 14, 20


def font(name, size, tmp):
    p = tmp / (name + ".ttf")
    t = TTFont(HERE / "fonts" / (name + ".woff2"))
    t.flavor = None
    t.save(p)
    return ImageFont.truetype(str(p), size)


def rgb(h):
    return tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))


def tool_calls(data):
    uses, calls = {}, []
    for e in data["events"]:
        ev = e["event"]
        for c in ev.get("message", {}).get("content", []) if isinstance(ev.get("message"), dict) else []:
            if not isinstance(c, dict):
                continue
            if c.get("type") == "tool_use" and c["name"].startswith("mcp__phone-use__"):
                uses[c["id"]] = {"tool": c["name"].split("__")[-1], "input": c["input"], "start": e["t"]}
            elif c.get("type") == "tool_result" and c["tool_use_id"] in uses:
                x = c["content"]
                u = uses[c["tool_use_id"]]
                u["end"] = e["t"]
                u["out"] = x if isinstance(x, str) else "\n".join(i.get("text", "") for i in x)
                calls.append(u)
    return sorted(calls, key=lambda u: u["start"])


def label(u):
    args = u["input"]
    if set(args) == {"target"}:
        return u["tool"] + " " + args["target"]
    return u["tool"] + "".join(f' {k}={json.dumps(v)}' for k, v in args.items() if k != "device")


def video_map(src, n, calls):
    """simctl drops idle time from the video, so its clock runs behind the call log. Each burst of motion in the video is
    tied to the call that caused it (the calls that change the screen, in order); time in between is interpolated."""
    g = [np.asarray(Image.open(src / f"{i + 1:04d}.png").convert("L").resize((24, 52)), dtype=float) for i in range(n)]
    moving = [i + 1 for i in range(n - 1) if np.abs(g[i + 1] - g[i]).mean() > 1.0]
    starts, last = [], -99
    for i in moving:
        if i - last > 0.8 * FPS_SRC:
            starts.append(i / FPS_SRC)
        last = i
    changing = [u for u in calls if "new screen" in u["out"] or "swiped" in u["out"] or "scrolled" in u["out"] or "\n~ " in u["out"]]
    if len(starts) == len(changing) + 1 and starts[1] - starts[0] < 1.5 and changing[0]["tool"] == "launch":
        del starts[1]
    if len(starts) != len(changing):
        raise SystemExit(f"{len(starts)} bursts of motion in the video, {len(changing)} calls that change the screen")
    pts = [(u["start"] + 0.2, v) for u, v in zip(changing, starts)]

    def f(t):
        if t <= pts[0][0]:
            return pts[0][1] + (t - pts[0][0])
        for (a, x), (b, y) in zip(pts, pts[1:]):
            if t <= b:
                return x + (y - x) * (t - a) / (b - a)
        return pts[-1][1] + (t - pts[-1][0])
    return f


def segments(calls):
    raw = []
    for u in calls:
        if u["end"] - u["start"] > 1.4:
            raw += [(u["start"] - 0.1, u["start"] + 0.7), (u["end"] - 0.5, u["end"] + 0.2)]
        else:
            raw.append((u["start"] - 0.1, u["end"] + 0.2))
    segs = []
    for a, b in sorted(raw):
        if segs and a <= segs[-1][1]:
            segs[-1] = (segs[-1][0], max(segs[-1][1], b))
        else:
            segs.append((a, b))
    segs[0] = (calls[0]["start"] - 0.6, segs[0][1])
    segs[-1] = (segs[-1][0], calls[-1]["end"] + 2.0)
    return segs


def main():
    demo, out = Path(sys.argv[1]), Path(sys.argv[2])
    data = json.loads((demo / "agent.json").read_text())
    calls = tool_calls(data)
    segs = segments(calls)
    tmp = Path(tempfile.mkdtemp())
    mono = font("CommitMono-Regular", FS, tmp)
    mono_s = font("CommitMono-Regular", 13, tmp)
    ph = H - HEAD - 2 - 2 * 18
    pw = round(ph * 1206 / 2622)
    (tmp / "src").mkdir()
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(demo / "screen.mov"), "-vf", f"fps={FPS_SRC},scale={pw}:{ph}:flags=lanczos",
                    str(tmp / "src" / "%04d.png")], check=True)
    nsrc = len(list((tmp / "src").glob("*.png")))
    vmap = video_map(tmp / "src", nsrc, calls)

    blocks = []
    for u in calls:
        lines = u["out"].rstrip("\n").split("\n")
        clip = 99 if any(l.startswith("~ ") for l in lines) else 3
        if len(lines) > clip:
            lines = lines[:clip] + [f"… {len(lines) - clip} more lines"]
        blocks.append((u["start"], u["end"], label(u), lines))
    prompt = textwrap.wrap(data["prompt"], 66)

    def buffer_at(t):
        rows = [("prompt", (">" if i == 0 else " ") + " " + l, False) for i, l in enumerate(prompt)]
        for s, e, cmd, lines in blocks:
            if t >= s:
                rows.append(("cmd", cmd, t < e))
            if t >= e:
                rows += [("out", l, False) for l in lines]
        return rows

    def paint(d, x, y, s, fill, maxx):
        for part, col in colorize(s, fill):
            d.text((x, y), part, font=mono, fill=col)
            x += mono.getlength(part)
        return x

    def colorize(s, base):
        t1, t2, t3, ok = rgb(C["t1"]), rgb(C["t2"]), rgb(C["t3"]), rgb(C["ok"])
        if s.startswith("… "):
            return [(s, t3)]
        parts, pos = [], 0
        for m in re.finditer(r'@e\d+|"[^"]*"|\bon\b$|^ok\b|^[-+~] ', s):
            if m.start() > pos:
                parts.append((s[pos:m.start()], t2))
            g = m.group()
            col = t3 if g.startswith("@") else t1 if g.startswith('"') else ok if g in ("on", "ok") else t3 if g.strip() in "-+" else t2
            parts.append((g, col))
            pos = m.end()
        if pos < len(s):
            parts.append((s[pos:], t2))
        return parts

    def frame(t_src, k):
        im = Image.new("RGB", (W, H), rgb(C["page"]))
        d = ImageDraw.Draw(im)
        line, panel, well = rgb(C["line"]), rgb(C["panel"]), rgb(C["well"])
        tw = W - PHONE_W
        d.rectangle([0, 0, W - 1, HEAD], fill=panel)
        d.rectangle([0, 0, tw, H - 1], fill=well)
        d.rectangle([0, 0, tw - 1, HEAD - 1], fill=panel)
        d.rectangle([0, HEAD, W - 1, HEAD], fill=line)
        d.rectangle([tw, 0, tw, H - 1], fill=line)
        d.rectangle([0, 0, W - 1, H - 1], outline=line)
        d.text((16, HEAD // 2), "CLAUDE CODE · PHONE-USE MCP", font=mono_s, fill=rgb(C["t3"]), anchor="lm")
        d.text((tw + 16, HEAD // 2), "IPHONE 17 · IOS 26.5", font=mono_s, fill=rgb(C["t3"]), anchor="lm")
        rows = buffer_at(t_src)
        vis = (H - HEAD - 24) // LH
        np = len(prompt)
        rows = rows[:np] + [("rule", "", False)] + rows[np:][-(vis - np - 1):]
        y = HEAD + 14
        for kind, s, running in rows:
            if kind == "out" and s.startswith("~ "):
                d.rectangle([1, y - 3, tw - 1, y + LH - 4], fill=rgb(C["selected"]))
            x = 16
            if kind == "rule":
                d.rectangle([1, y + 2, tw - 1, y + 2], fill=rgb(C["line"]))
            elif kind == "prompt":
                d.text((x, y), s, font=mono, fill=rgb(C["t1"]))
            elif kind == "cmd":
                d.text((x, y), "mcp", font=mono, fill=rgb(C["t3"]))
                x += mono.getlength("mcp ")
                d.text((x, y), s, font=mono, fill=rgb(C["t1"]))
                x += mono.getlength(s)
                if running:
                    d.rectangle([x + 6, y + 2, x + 13, y + 15], fill=rgb(C["t2"]))
            else:
                while mono.getlength(s) > tw - 32:
                    s = s[:-2]
                    if not s.endswith("…"):
                        s = s[:-1] + "…"
                paint(d, x, y, s, None, tw - 16)
            y += LH
        idx = min(max(round(vmap(t_src) * FPS_SRC), 0), nsrc - 1) + 1
        phone = Image.open(tmp / "src" / f"{idx:04d}.png").convert("RGB")
        px = tw + (PHONE_W - pw) // 2
        py = HEAD + 1 + 18
        im.paste(phone, (px, py))
        d.rectangle([px - 1, py - 1, px + pw, py + ph], outline=line)
        return im

    fr = tmp / "out"
    fr.mkdir()
    k = 0
    final = segs[-1][1] - 0.05
    for _ in range(3):
        frame(final, k).save(fr / f"{k:04d}.png")
        k += 1
    for a, b in segs:
        t = a
        while t < b:
            frame(t, k).save(fr / f"{k:04d}.png")
            k += 1
            t += 1 / FPS_OUT
    print(f"{k} frames, {k / FPS_OUT:.1f} s")
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-framerate", str(FPS_OUT), "-i", str(fr / "%04d.png"), "-vf",
                    "split[a][b];[a]palettegen=max_colors=256:stats_mode=diff[p];[b][p]paletteuse=dither=sierra2_4a",
                    "-loop", "0", str(out)], check=True)
    for n, lab in ((0, "first"), (k // 2, "middle"), (k - 1, "last")):
        (out.parent / f"{out.stem}-{lab}.png").write_bytes((fr / f"{n:04d}.png").read_bytes())
    print(out, out.stat().st_size)


if __name__ == "__main__":
    main()
