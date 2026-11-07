"""Builds .github/assets/demo.mp4 and demo.gif from a run recorded by demo_record.py: Claude Code's terminal
recording (VHS) on the left and the emulator's own screen video on the right, aligned by wall-clock start times
(times.json). VHS cannot keep real time while it renders a long session (its video runs about 0.8 of wall time),
so times.json carries "term_map": t0, wall0, rate, which maps a time in the terminal video to wall seconds after
term_start; the terminal video is first put on the wall clock with it. Then one uniform speed-up covers the wall
window from START to END minus HOLD, and the last HOLD seconds play at real speed so the final state can be read.
Nothing else is cut or retimed.
The first frame of the GIF is the finished state, so a static preview shows the result.
Usage: python3 demo_compose.py <run dir> <start s> <end s> <speed> <hold s> <out dir>   (start and end in wall seconds after term_start)"""
import json
import subprocess
import sys
from pathlib import Path

from fontTools.ttLib import TTFont
from PIL import Image, ImageDraw, ImageFont

from sodium import DARK as C, HERE

RUN, START, END, SPEED, HOLD, OUT = Path(sys.argv[1]), float(sys.argv[2]), float(sys.argv[3]), float(sys.argv[4]), float(sys.argv[5]), Path(sys.argv[6])
t = json.loads((RUN / "times.json").read_text())


def duration(p):
    return float(subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(p)], capture_output=True, text=True, check=True).stdout)


M = t.get("term_map", {"t0": 0.0, "wall0": 0.0, "rate": 1.0})
bg = OUT / "bg.png"
tmp = OUT / "commit.ttf"
tt = TTFont(HERE / "fonts" / "CommitMono-Regular.woff2")
tt.flavor = None
tt.save(tmp)
img = Image.new("RGB", (960, 660), C["page"])
dr = ImageDraw.Draw(img)
f = ImageFont.truetype(str(tmp), 12)
for txt, x in (("CLAUDE CODE", 16), ("ANDROID 17 EMULATOR", 672)):
    dr.text((x, 12), txt, font=f, fill=C["t3"])
dr.rectangle((16, 36, 943, 36), fill=C["line"])
img.save(bg)
tmp.unlink()

inputs = ["-i", str(RUN / "term.mp4")]
chains = []
placed = 0.0
labels = []
for i, seg in enumerate(t["segments"]):
    inputs += ["-i", str(RUN / seg["file"])]
    d = duration(RUN / seg["file"])
    off = seg["start"] - t["term_start"]
    trim = 0.0
    pad = 0.0
    if off < placed:
        trim = placed - off
    else:
        pad = off - placed
    chains.append(f"[{i + 1}:v]fps=30,trim=start={trim:.3f},setpts=PTS-STARTPTS,tpad=start_duration={pad:.3f}:start_mode=clone,scale=268:-2:flags=lanczos[s{i}]")
    labels.append(f"[s{i}]")
    placed = max(placed, off) + (d - trim)
n = len(labels)
phone = "".join(labels) + f"concat=n={n}:v=1[pall]" if n > 1 else "[s0]null[pall]"
main_end = END - HOLD


def split(src, name, w, wall):
    pre = f"[{src}]scale={w}:-2:flags=lanczos,fps=30," if w else f"[{src}]fps=30,"
    pre += f"setpts={M['rate']}*PTS+{M['wall0'] - M['rate'] * M['t0']:.3f}/TB," if wall else ""
    return (
        pre + f"split[{name}a][{name}b];"
        f"[{name}a]trim=start={START:.3f}:end={main_end:.3f},setpts=(PTS-STARTPTS)/{SPEED}[{name}m];"
        f"[{name}b]trim=start={main_end:.3f}:end={END:.3f},setpts=PTS-STARTPTS[{name}h];"
        f"[{name}m][{name}h]concat=n=2:v=1[{name}]"
    )


fc = ";".join(chains + [phone, split("0:v", "t", 640, True), split("pall", "p", 0, False),
                         f"[{n + 1}:v][t]overlay=16:52:shortest=1[a];[a][p]overlay=672:52:shortest=1[v]"])
inputs += ["-loop", "1", "-framerate", "30", "-i", str(bg)]
mp4 = OUT / "demo.mp4"
subprocess.run(["ffmpeg", "-v", "error", "-y", *inputs, "-filter_complex", fc, "-map", "[v]", "-c:v", "libx264", "-crf", "20", "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(mp4)], check=True)
last = RUN / "last.png"
subprocess.run(["ffmpeg", "-v", "error", "-y", "-sseof", "-0.1", "-i", str(mp4), "-frames:v", "1", str(last)], check=True)
gf = "fps=10,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle"
subprocess.run(["ffmpeg", "-v", "error", "-y", "-loop", "1", "-framerate", "10", "-t", "0.2", "-i", str(last), "-i", str(mp4),
                "-filter_complex", f"[0:v]fps=10,setsar=1[f];[1:v]fps=10,setsar=1[m];[f][m]concat=n=2:v=1,{gf}", "-loop", "0", str(OUT / "demo.gif")], check=True)
