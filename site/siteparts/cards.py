import hashlib
import html
import subprocess
import time
from pathlib import Path

T = """<!doctype html><meta charset=utf-8><style>
@font-face{{font-family:I;src:url(file://{fd}/InterDisplay-Regular.woff2)}}
@font-face{{font-family:M;src:url(file://{fd}/CommitMono-Regular.woff2)}}
*{{box-sizing:border-box;margin:0}}
body{{width:1200px;height:630px;background:#fff;color:#161512;font-family:I;position:relative;overflow:hidden}}
.top{{position:absolute;left:64px;top:52px;display:flex;align-items:center;gap:14px;font-size:30px}}
.t{{position:absolute;left:64px;right:64px;top:170px;font-size:{fs}px;line-height:1.04;letter-spacing:-.025em}}
.t span{{background:linear-gradient(#ffd400,#ffd400) 0 94%/100% 10px no-repeat}}
.s{{position:absolute;left:64px;right:64px;bottom:96px;font-family:M;font-size:26px;color:#55524a;line-height:1.35}}
.bar{{position:absolute;left:0;right:0;bottom:0;height:20px;background:#ffd400}}
.d{{position:absolute;left:64px;bottom:44px;font-family:M;font-size:20px;color:#6b675e;letter-spacing:.06em}}
</style>
<div class=top><svg width=40 height=40 viewBox="0 0 24 24"><rect x=6.5 y=2.5 width=11 height=19 fill=none stroke=#161512 stroke-width=1.5 /><path d="M9 7.5h6M9 17h6" stroke=#161512 stroke-width=1.5 /><rect x=8.5 y=10 width=7 height=4 fill=#ffd400 /></svg>phonedriver</div>
<div class=t><span>{title}</span></div>
<div class=s>{sub}</div>
<div class=d>{domain}</div><div class=bar></div>"""


def render(outdir, specs, fontdir, chrome, cache, domain="PHONEDRIVER.DEV"):
    cache.mkdir(exist_ok=True)
    for name, (title, sub) in specs.items():
        key = hashlib.sha1(("%s|%s|%s" % (title, sub, domain)).encode()).hexdigest()[:12]
        dst = outdir / (name + ".png")
        cached = cache / (name + "-" + key + ".png")
        if cached.exists():
            dst.write_bytes(cached.read_bytes())
            continue
        fs = 88 if len(title) < 34 else 72 if len(title) < 60 else 60
        h = T.format(fd=fontdir, title=html.escape(title), sub=html.escape(sub), fs=fs, domain=domain)
        src = cache / (name + ".html")
        src.write_text(h)
        p = subprocess.Popen([chrome, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1",
                              "--window-size=1200,630", "--screenshot=" + str(cached), "file://" + str(src)],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(300):
            if cached.exists() and cached.stat().st_size > 1000:
                time.sleep(0.3)
                break
            time.sleep(0.1)
        p.kill()
        if not cached.exists():
            raise SystemExit("card render failed: " + name)
        dst.write_bytes(cached.read_bytes())
