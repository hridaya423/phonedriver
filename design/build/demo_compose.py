"""Builds .github/assets/demo.mp4 and demo.gif from a run recorded by demo_record.py: Claude Code's terminal
recording (VHS) on the left and the Simulator's own screen video on the right, aligned by wall-clock start times
(times.json). Nothing is re-rendered or retimed: real speed, cut only at the start and at the end.
The first frame of the GIF is the finished state, so a static preview shows the result.
Usage: python3 demo_compose.py <run dir> <end s of terminal video> <out dir>"""
import json
import subprocess
import sys
from pathlib import Path

from fontTools.ttLib import TTFont
from PIL import Image, ImageDraw, ImageFont

from sodium import DARK as C, HERE

RUN, END, OUT = Path(sys.argv[1]), float(sys.argv[2]), Path(sys.argv[3])
t = json.loads((RUN / "times.json").read_text())
off = t["term_start"] - t["sim_start"]
bg = OUT / "bg.png"
tmp = OUT / "commit.ttf"
tt = TTFont(HERE / "fonts" / "CommitMono-Regular.woff2")
tt.flavor = None
tt.save(tmp)
img = Image.new("RGB", (960, 660), C["page"])
dr = ImageDraw.Draw(img)
f = ImageFont.truetype(str(tmp), 12)
for txt, x in (("CLAUDE CODE", 16), ("IPHONE 17 \u00b7 IOS 26.5", 672)):
    dr.text((x, 12), txt, font=f, fill=C["t3"])
dr.rectangle((16, 36, 943, 36), fill=C["line"])
img.save(bg)
tmp.unlink()
fc = (
    "[0:v]scale=640:-2:flags=lanczos,setpts=PTS-STARTPTS[t];"
    f"[1:v]fps=30,trim=start={off}:duration={END},setpts=PTS-STARTPTS,scale=272:-2:flags=lanczos[p];"
    "[2:v][t]overlay=16:52:shortest=1[a];[a][p]overlay=672:52:shortest=1[v]"
)
mp4 = OUT / "demo.mp4"
subprocess.run(["ffmpeg", "-v", "error", "-y", "-t", str(END), "-i", str(RUN / "term.mp4"), "-i", str(RUN / "screen.mov"), "-loop", "1", "-framerate", "30", "-i", str(bg),
                "-filter_complex", fc, "-map", "[v]", "-t", str(END), "-c:v", "libx264", "-crf", "20", "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(mp4)], check=True)
last = RUN / "last.png"
subprocess.run(["ffmpeg", "-v", "error", "-y", "-sseof", "-0.1", "-i", str(mp4), "-frames:v", "1", str(last)], check=True)
gf = "fps=10,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle"
subprocess.run(["ffmpeg", "-v", "error", "-y", "-loop", "1", "-framerate", "10", "-t", "0.2", "-i", str(last), "-i", str(mp4),
                "-filter_complex", f"[0:v]fps=10,setsar=1[f];[1:v]fps=10,setsar=1[m];[f][m]concat=n=2:v=1,{gf}", "-loop", "0", str(OUT / "demo.gif")], check=True)
