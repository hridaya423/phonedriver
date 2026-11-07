import http.server, socketserver, subprocess, sys, threading, time
from pathlib import Path
PUB = Path(__file__).resolve().parent / "public"
OUT = Path(sys.argv[1])
OUT.mkdir(parents=True, exist_ok=True)
CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
PORT = 8765


class H(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k):
        super().__init__(*a, directory=str(PUB), **k)

    def log_message(self, *a):
        pass

    def do_GET(self):
        if self.path.startswith("/__m/"):
            _, _, theme, rest = self.path.split("/", 3)
            data = ('<body style="margin:0;background:#888"><iframe src="/__%s/%s" style="width:390px;height:12000px;border:0;display:block"></iframe>' % (theme, rest)).encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/html")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return
        theme = None
        for t in ("light", "dark"):
            if self.path.startswith("/__%s" % t):
                theme = t
                self.path = self.path[len(t) + 3:] or "/"
        p = self.translate_path(self.path)
        pp = Path(p)
        if pp.is_dir():
            pp = pp / "index.html"
        if theme and pp.suffix == ".html" and pp.exists():
            data = pp.read_text().replace('<html lang="en">', '<html lang="en" data-theme="%s">' % theme).replace('media="(prefers-color-scheme: dark)"', 'media="%s"' % ("all" if theme == "dark" else "not all")).encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/html")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return
        super().do_GET()


class S(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


srv = S(("127.0.0.1", PORT), H)
threading.Thread(target=srv.serve_forever, daemon=True).start()
pages = {"home": "/", "docs": "/docs/claude-code/", "compare": "/compare/adb/", "post": "/blog/how-phonedriver-reads-a-screen-98x-faster-than-adb/", "bench": "/benchmarks/"}
only = sys.argv[2:] or list(pages)
for name in only:
    path = pages[name]
    for w, h in ((1440, 9000), (390, 12000)):
        for scheme in ("light", "dark"):
            dst = OUT / ("%s-%d-%s.png" % (name, w, scheme))
            dst.unlink(missing_ok=True)
            args = [CHROME, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1", "--window-size=%d,%d" % (w, h),
                    "--screenshot=" + str(dst), ("http://127.0.0.1:%d/__m/%s%s" if w == 390 else "http://127.0.0.1:%d/__%s%s") % (PORT, scheme, path)]
            p = subprocess.Popen(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            for _ in range(300):
                if dst.exists() and dst.stat().st_size > 1000:
                    time.sleep(0.6)
                    break
                time.sleep(0.1)
            p.kill()
            if w == 390 and dst.exists():
                from PIL import Image
                Image.open(dst).crop((0, 0, 390, Image.open(dst).size[1])).save(dst)
            print(dst, dst.exists(), flush=True)
srv.shutdown()
