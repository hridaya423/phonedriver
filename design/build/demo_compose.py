"""Builds the launch demo from scenes recorded by demo_record.py. Each scene is Claude Code's terminal (tmux captures
drawn by demo_term.py) beside the emulator's own screen video, both on the wall clock of the recording. One uniform
speed-up covers a scene, from START to END minus TAIL; the last TAIL seconds play at real speed so the final state can
be read, and an opening FREEZE holds the first frame so the prompt can be read. Nothing else is cut or retimed.
The edit list (edit.json) names, per scene: run dir, start and end (seconds after the mark "enter"), speed, freeze,
tail, badge text and captions as [seconds after enter (wall), text]. Outputs go to the out dir:
  demo.mp4 (1280x720), demo.gif (600 wide), poster.png (1200x675), short.mp4 (1080x1920), contact.png (every second).
Usage: python3 demo_compose.py <edit.json> <out dir> [--only wide|short]"""
import io
import json
import math
import subprocess
import sys
from pathlib import Path

from fontTools.ttLib import TTFont
from PIL import Image, ImageDraw, ImageFont

import demo_term
from sodium import DARK as C, HERE, INTER, MONO, lockup, svg, text as stext, rect

EDIT, OUT = json.loads(Path(sys.argv[1]).read_text()), Path(sys.argv[2])
ONLY = sys.argv[sys.argv.index("--only") + 1] if "--only" in sys.argv else None
OUT.mkdir(parents=True, exist_ok=True)
FPS = 30
BORDER = "#2c2a25"


def rgb(h):
    return tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))


def ttf(name):
    p = OUT / (name + ".ttf")
    if not p.exists():
        t = TTFont(HERE / "fonts" / (name + ".woff2"))
        t.flavor = None
        t.save(p)
    return str(p)


def ff(*args, **kw):
    subprocess.run(["ffmpeg", "-v", "error", "-y", *args], check=True, **kw)


def duration(p):
    return float(subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(p)], capture_output=True, text=True, check=True).stdout)


class Scene:
    def __init__(self, spec):
        self.s = spec
        self.run = Path(spec["run"])
        self.times = json.loads((self.run / "times.json").read_text())
        self.mark = self.times["marks"]["enter"]
        self.t0 = self.mark + spec["start"]
        self.t1 = self.mark + spec["end"]
        self.log = [json.loads(l) for l in (self.run / "term.jsonl").read_text().splitlines()]
        self.speed = spec["speed"]
        self.freeze = spec.get("freeze", 0.0)
        self.tail = spec.get("tail", 0.0)
        self.tm = self.t1 - self.tail

    def out_time(self, wall):
        """Seconds into the scene's output for a wall time (absolute)."""
        w = wall
        if w <= self.tm:
            return self.freeze + max(0.0, w - self.t0) / self.speed
        return self.freeze + (self.tm - self.t0) / self.speed + (w - self.tm)

    @property
    def length(self):
        return self.out_time(self.t1)

    def screen_at(self, t):
        cur = self.log[0]["screen"]
        for e in self.log:
            if e["t"] <= t:
                cur = e["screen"]
            else:
                break
        return cur

    def rows_for_short(self, screen, n):
        """The last n conversation rows: above the input box, or the whole form when Claude Code asks for input."""
        lines = screen.rstrip("\n").split("\n")
        plain = [__import__("re").sub(r"\x1b\[[0-9;]*m", "", l) for l in lines]
        rules = [i for i, l in enumerate(plain) if l.startswith("──")]
        if any("requests your input" in l for l in plain) and rules:
            top = rules[0] + 1
            body = [i for i in range(top, len(plain)) if plain[i].strip() and not plain[i].lstrip().startswith(("Esc to", "Backspace", "→"))]
            return body[0], body[0] + n
        end = rules[-2] if len(rules) >= 2 else len(plain)
        body = [i for i in range(0, end) if plain[i].strip()]
        last = body[-1] + 1 if body else end
        start = max(0, last - n)
        return start, start + n

    def term_video(self, layout, tag):
        d = OUT / f"{self.s['name']}-{tag}-term"
        d.mkdir(exist_ok=True)
        term = layout["term"]
        entries = [e for e in self.log if e["t"] < self.t1]
        entries = [e for e in entries if e["t"] >= self.t0 - 60]
        lines = []
        seen = {}
        pieces = []
        for i, e in enumerate(entries):
            a = max(e["t"], self.t0)
            b = entries[i + 1]["t"] if i + 1 < len(entries) else self.t1
            b = min(b, self.t1)
            if b > a:
                pieces.append((a, b, e["screen"]))
        if not pieces:
            pieces = [(self.t0, self.t1, self.screen_at(self.t0))]
        pieces[0] = (self.t0, pieces[0][1], pieces[0][2])
        for i, (a, b, s) in enumerate(pieces):
            rows = layout.get("rows")
            r = self.rows_for_short(s, rows) if rows else (0, None)
            key = (s, r)
            if key not in seen:
                p = d / f"{len(seen):05d}.png"
                term.render(s, *r).save(p)
                seen[key] = p
            lines.append(f"file '{seen[key].resolve()}'\nduration {b - a:.4f}")
        lines.append(f"file '{seen[key].resolve()}'")
        (d / "list.txt").write_text("\n".join(lines) + "\n")
        out = OUT / f"{self.s['name']}-{tag}-term.mp4"
        ff("-f", "concat", "-safe", "0", "-i", str(d / "list.txt"), "-vf", f"fps={FPS},format=yuv420p", "-c:v", "libx264", "-crf", "12", "-preset", "veryfast", str(out))
        return out

    def phone_offset(self):
        seg0 = self.times["segments"][0]
        f = self.run / seg0["file"]
        raw = subprocess.run(["ffmpeg", "-v", "error", "-i", str(f), "-vf", "fps=20,scale=32:72,format=gray", "-f", "rawvideo", "-"], capture_output=True, check=True).stdout
        n = 32 * 72
        frames = [raw[i:i + n] for i in range(0, len(raw) - n + 1, n)]
        base = frames[0]
        ft = None
        for i, fr in enumerate(frames):
            if sum(abs(a - b) for a, b in zip(fr, base)) / n > 6:
                ft = i / 20
                break
        if ft is None:
            raise SystemExit(f"{f}: no shade flash found")
        flash = self.times["marks"]["flash"]
        return flash - ft - 0.15 - seg0["start"], ft

    def phone_video(self, size, tag):
        delta, ft = self.phone_offset()
        inputs, chains, labels = [], [], []
        placed = self.t0
        for i, seg in enumerate(self.times["segments"]):
            inputs += ["-i", str(self.run / seg["file"])]
            start = seg["start"] + delta
            d = duration(self.run / seg["file"])
            end = start + d
            if end <= placed:
                continue
            trim = max(0.0, placed - start)
            pad = max(0.0, start - placed)
            chains.append(f"[{i}:v]fps={FPS},trim=start={trim:.3f},setpts=PTS-STARTPTS,tpad=start_duration={pad:.3f}:start_mode=clone,scale={size[0]}:{size[1]}:flags=lanczos[s{i}]")
            labels.append(f"[s{i}]")
            placed = end
        fc = ";".join(chains) + ";" + ("".join(labels) + f"concat=n={len(labels)}:v=1[o]" if len(labels) > 1 else f"{labels[0]}null[o]")
        out = OUT / f"{self.s['name']}-{tag}-phone.mp4"
        ff(*inputs, "-filter_complex", fc, "-map", "[o]", "-t", f"{self.t1 - self.t0:.3f}", "-c:v", "libx264", "-crf", "14", "-preset", "veryfast", "-pix_fmt", "yuv420p", str(out))
        return out

    def retime(self, src, label):
        """Filter chain text: freeze the first frame, speed the middle, keep the tail real."""
        tm, tl = self.tm - self.t0, self.t1 - self.t0
        n = 1 + (self.freeze > 0) + (self.tail > 0)
        outs = [f"[{label}{i}]" for i in range(n)]
        parts = [f"[{src}]fps={FPS},split={n}{''.join(outs)}"]
        segs = []
        i = 0
        if self.freeze > 0:
            parts.append(f"{outs[i]}trim=end_frame=1,setpts=PTS-STARTPTS,tpad=stop_duration={self.freeze:.3f}:stop_mode=clone[{label}f]")
            segs.append(f"[{label}f]")
            i += 1
        parts.append(f"{outs[i]}trim=start=0:end={tm:.3f},setpts=(PTS-STARTPTS)/{self.speed}[{label}m]")
        segs.append(f"[{label}m]")
        i += 1
        if self.tail > 0:
            parts.append(f"{outs[i]}trim=start={tm:.3f}:end={tl:.3f},setpts=PTS-STARTPTS[{label}t]")
            segs.append(f"[{label}t]")
        parts.append("".join(segs) + f"concat=n={len(segs)}:v=1[{label}]")
        return ";".join(parts), None


def caption_png(textv, layout, path):
    w, h = layout["caption_size"]
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    f = ImageFont.truetype(ttf("InterDisplay-Regular"), layout["caption_px"])
    tw = d.textlength(textv, font=f)
    box = layout["caption_px"] * 0.34
    gap = layout["caption_px"] * 0.5
    total = box + gap + tw
    pad = layout.get("caption_pad", 0)
    x0 = (w - total) / 2
    if pad:
        d.rectangle((x0 - pad, 0, x0 + total + pad, h - 1), fill=rgb(C["page"]) + (235,))
        d.rectangle((x0 - pad, 0, x0 - pad + 5, h - 1), fill=rgb(C["accent"]))
    cy = h / 2
    d.rectangle((x0, cy - box / 2, x0 + box, cy + box / 2), fill=rgb(C["accent"]))
    asc, desc = f.getmetrics()
    d.text((x0 + box + gap, cy - (asc + desc) / 2 + 1), textv, font=f, fill=rgb(C["t1"]))
    img.save(path)


def badge_png(label, px, path):
    f = ImageFont.truetype(ttf("CommitMono-Regular"), px)
    d0 = ImageDraw.Draw(Image.new("RGB", (1, 1)))
    tw = d0.textlength(label, font=f)
    pad = px * 0.5
    w, h = int(tw + 2 * pad), int(px * 1.9)
    img = Image.new("RGBA", (w, h), rgb(C["page"]) + (255,))
    d = ImageDraw.Draw(img)
    d.rectangle((0, 0, w - 1, h - 1), outline=rgb(C["accent"]), width=2)
    asc, desc = f.getmetrics()
    d.text((pad, h / 2 - (asc + desc) / 2), label, font=f, fill=rgb(C["accent"]))
    img.save(path)
    return w, h


def device_frame(screen_w, screen_h, pad_x, pad_top, pad_bot):
    w, h = screen_w + 2 * pad_x, screen_h + pad_top + pad_bot
    img = Image.new("RGB", (w, h), rgb("#0b0b0a"))
    d = ImageDraw.Draw(img)
    d.rectangle((0, 0, w - 1, h - 1), outline=rgb("#3a372f"), width=2)
    d.rectangle((pad_x - 1, pad_top - 1, pad_x + screen_w, pad_top + screen_h), outline=rgb("#2c2a25"))
    cx = w // 2
    d.rectangle((cx - 22, pad_top // 2 - 1, cx + 22, pad_top // 2 + 1), fill=rgb("#2c2a25"))
    d.rectangle((cx - 40, h - pad_bot // 2 - 1, cx + 40, h - pad_bot // 2), fill=rgb("#2c2a25"))
    return img


def layouts():
    wide_term = demo_term.Term(25, 56, pad=12)
    tw, th = wide_term.size(19)
    sw, sh = 308, 692
    frame = device_frame(sw, sh, 8, 10, 10)
    short_term = demo_term.Term(31, 56, pad=8)
    sw2, sh2 = 372, 836
    frame2 = device_frame(sw2, sh2, 14, 26, 26)
    return {
        "wide": dict(size=(1280, 720), term=wide_term, term_xy=(20, 24), phone=(sw, sh), frame=frame,
                     frame_xy=(1280 - 20 - frame.size[0], (720 - frame.size[1]) // 2), pad=(8, 10),
                     caption_xy=(20, 660), caption_size=(tw, 48), caption_px=28, badge_px=22, badge_anchor=("right", 20 + tw, 660 + 5),
                     rows=None, term_h=th, term_w=tw),
        "short": dict(size=(1080, 1920), term=short_term, term_xy=((1080 - short_term.size(10)[0]) // 2, 1030), phone=(sw2, sh2), frame=frame2,
                      frame_xy=((1080 - frame2.size[0]) // 2, 120), pad=(14, 26),
                      caption_xy=(0, 1434), caption_size=(1080, 64), caption_px=40, caption_pad=22, badge_px=30,
                      badge_anchor=("right", 1080 - 150, 120 - 60), rows=10, term_h=short_term.size(10)[1], term_w=short_term.size(10)[0]),
    }


def bg_png(L, path):
    W, H = L["size"]
    img = Image.new("RGB", (W, H), rgb(C["page"]))
    d = ImageDraw.Draw(img)
    x, y = L["term_xy"]
    th = L["term_h"] if L["rows"] is None else L["term_h"]
    if L["rows"] is None:
        d.rectangle((x - 1, y - 1, x + L["term_w"], y + th), outline=rgb(BORDER))
    img.paste(L["frame"], L["frame_xy"])
    img.save(path)


def render_scene(sc, L, tag):
    term_v = sc.term_video(L, tag)
    phone_v = sc.phone_video(L["phone"], tag)
    bg = OUT / f"bg-{tag}.png"
    bg_png(L, bg)
    inputs = ["-loop", "1", "-framerate", str(FPS), "-i", str(bg), "-i", str(term_v), "-i", str(phone_v)]
    fc, _ = sc.retime("1:v", "t")
    fp, _ = sc.retime("2:v", "p")
    ox, oy = L["frame_xy"][0] + L["pad"][0], L["frame_xy"][1] + L["pad"][1]
    parts = [fc, fp, f"[0:v][t]overlay={L['term_xy'][0]}:{L['term_xy'][1]}:shortest=0[a];[a][p]overlay={ox}:{oy}[v0]"]
    last = "v0"
    idx = 3
    bw, bh = badge_png(sc.s["badge"], L["badge_px"], OUT / f"{sc.s['name']}-{tag}-badge.png")
    inputs += ["-i", str(OUT / f"{sc.s['name']}-{tag}-badge.png")]
    if L["badge_anchor"][0] == "right":
        bx, by = L["badge_anchor"][1] - bw, L["badge_anchor"][2]
    parts.append(f"[{last}][{idx}:v]overlay={bx}:{by}[b{idx}]")
    last = f"b{idx}"
    idx += 1
    for k, (at, textv, dur) in enumerate(sc.s["captions"]):
        p = OUT / f"{sc.s['name']}-{tag}-cap{k}.png"
        caption_png(textv, L, p)
        a = sc.out_time(sc.mark + at)
        inputs += ["-i", str(p)]
        parts.append(f"[{last}][{idx}:v]overlay={L['caption_xy'][0]}:{L['caption_xy'][1]}:enable='between(t,{a:.3f},{a + dur:.3f})'[c{idx}]")
        last = f"c{idx}"
        idx += 1
    out = OUT / f"{sc.s['name']}-{tag}.mp4"
    ff(*inputs, "-filter_complex", ";".join(parts), "-map", f"[{last}]", "-t", f"{sc.length:.3f}", "-r", str(FPS), "-c:v", "libx264", "-crf", "14", "-preset", "veryfast", "-pix_fmt", "yuv420p", str(out))
    return out


def end_card(L, tag, secs):
    W, H = L["size"]
    ink = C["t1"]
    body = ""
    ms = 112 if W > H else 120
    lk, lw = lockup(C, 0, 0, ms)
    if W > H:
        x0 = (W - lw) / 2
        y0 = 150
        body += f'<g transform="translate({x0:g} {y0:g})">{lk}</g>'
        t1, w1 = stext("Your agent can use your phone.", W / 2, 360, INTER, 44, C["t1"], -0.01, anchor="middle")
        t2, w2 = stext("It can’t spend your money without you.", W / 2, 420, INTER, 44, C["accent"], -0.01, anchor="middle")
        body += t1 + t2
        cmd = "claude mcp add phonedriver -- npx -y phonedriver mcp"
        cs = 28
        cw = MONO.run(cmd, cs)[1]
        bx, by, bh = (W - cw) / 2 - 28, 500, 64
        body += rect(bx, by, cw + 56, bh, C["panel"]) + rect(bx, by, cw + 56, 1, BORDER) + rect(bx, by + bh - 1, cw + 56, 1, BORDER) + rect(bx, by, 1, bh, BORDER) + rect(bx + cw + 55, by, 1, bh, BORDER)
        t3, _ = stext(cmd, W / 2, by + bh / 2 + 9, MONO, cs, C["t1"], anchor="middle")
        body += t3
    else:
        x0 = (W - lw) / 2
        body += f'<g transform="translate({x0:g} 520)">{lk}</g>'
        t1, _ = stext("Your agent can use your phone.", W / 2, 800, INTER, 60, C["t1"], -0.01, anchor="middle")
        t2, _ = stext("It can’t spend your money", W / 2, 880, INTER, 60, C["accent"], -0.01, anchor="middle")
        t2b, _ = stext("without you.", W / 2, 960, INTER, 60, C["accent"], -0.01, anchor="middle")
        body += t1 + t2 + t2b
        cmd = "claude mcp add phonedriver -- npx -y phonedriver mcp"
        cs = 26
        cw = MONO.run(cmd, cs)[1]
        by, bh = 1060, 70
        bx = (W - cw) / 2 - 28
        body += rect(bx, by, cw + 56, bh, C["panel"]) + rect(bx, by, cw + 56, 1, BORDER) + rect(bx, by + bh - 1, cw + 56, 1, BORDER) + rect(bx, by, 1, bh, BORDER) + rect(bx + cw + 55, by, 1, bh, BORDER)
        t3, _ = stext(cmd, W / 2, by + bh / 2 + 9, MONO, cs, C["t1"], anchor="middle")
        body += t3
    s = svg(W, H, body, "phonedriver end card", bg=C["page"])
    sp = OUT / f"card-{tag}.svg"
    sp.write_text(s)
    png = OUT / f"card-{tag}.png"
    from build import shot
    shot(sp.resolve(), png, W, H)
    out = OUT / f"card-{tag}.mp4"
    ff("-loop", "1", "-framerate", str(FPS), "-t", f"{secs}", "-i", str(png), "-vf", "format=yuv420p", "-c:v", "libx264", "-crf", "14", str(out))
    return out


def join(clips, fade, out, crf):
    inputs = []
    for c in clips:
        inputs += ["-i", str(c)]
    ds = [duration(c) for c in clips]
    fc, last, acc = "", "0:v", ds[0]
    for i in range(1, len(clips)):
        off = acc - fade
        fc += f"[{last}][{i}:v]xfade=transition=fade:duration={fade}:offset={off:.3f}[x{i}];"
        last = f"x{i}"
        acc = off + ds[i]
    ff(*inputs, "-filter_complex", fc.rstrip(";"), "-map", f"[{last}]", "-r", str(FPS), "-c:v", "libx264", "-crf", str(crf), "-preset", "slow", "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(out))


def main():
    ls = layouts()
    for tag in ("wide", "short"):
        if ONLY and ONLY != tag:
            continue
        L = ls[tag]
        clips = [render_scene(Scene(s), L, tag) for s in EDIT["scenes"]]
        clips.append(end_card(L, tag, EDIT.get("card", 3.4)))
        out = OUT / ("demo.mp4" if tag == "wide" else "short.mp4")
        join(clips, EDIT.get("fade", 0.4), out, EDIT.get("crf", 22) if tag == "wide" else 20)
        print(tag, out, round(duration(out), 2), "s", out.stat().st_size, "bytes")


if __name__ == "__main__":
    main()
