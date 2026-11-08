"""Draws a tmux screen capture (capture-pane -p -e) as an image in the Sodium dark theme with Commit Mono.
Glyphs Commit Mono lacks (Claude Code's bullets and spinners) come from Menlo, Apple Symbols or Arial Unicode.
Colours are exactly the ones Claude Code sent; only the page background is the brand's."""
import re
import tempfile
from pathlib import Path

from fontTools.ttLib import TTFont
from PIL import Image, ImageDraw, ImageFont

from sodium import DARK as C, HERE

FALLBACKS = ["/System/Library/Fonts/Menlo.ttc", "/System/Library/Fonts/Apple Symbols.ttf", "/Library/Fonts/Arial Unicode.ttf",
             "/System/Library/Fonts/Supplemental/Arial Unicode.ttf", "/System/Library/Fonts/LucidaGrande.ttc"]
SGR = re.compile(r"\x1b\[([0-9;]*)m")
OTHER = re.compile(r"\x1b\[[0-9;?]*[A-Za-ln-z]")
BASE16 = ["#141412", "#ff6b57", "#8cc79a", "#ffd400", "#7f9fc4", "#c49fc4", "#7fc4bf", "#c4c0b6",
          "#736d61", "#ff8a79", "#a8dcb3", "#ffe14d", "#a3bfe0", "#dcbfdc", "#a3dcd7", "#f5f3ee"]
SUBSTITUTE = {"\u23fa": "\u25cf", "\u23f5": "\u25b6", "\u23f8": "\u2016"}


def xterm(n):
    if n < 16:
        return BASE16[n]
    if n < 232:
        n -= 16
        r, g, b = (n // 36) % 6, (n // 6) % 6, n % 6
        return "#%02x%02x%02x" % tuple(0 if v == 0 else 55 + 40 * v for v in (r, g, b))
    v = 8 + 10 * (n - 232)
    return "#%02x%02x%02x" % (v, v, v)


def color(n):
    return xterm(n)


def hexrgb(h):
    return tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))


def parse(screen):
    """Returns rows of (char, fg, bg, bold, dim, inverse) cells."""
    rows = []
    fg = bg = None
    bold = dim = inv = False
    for line in screen.split("\n"):
        cells = []
        i = 0
        while i < len(line):
            m = SGR.match(line, i)
            if m:
                p = [int(x) if x else 0 for x in m.group(1).split(";")] or [0]
                k = 0
                while k < len(p):
                    c = p[k]
                    if c == 0:
                        fg = bg = None
                        bold = dim = inv = False
                    elif c == 1:
                        bold = True
                    elif c == 2:
                        dim = True
                    elif c == 22:
                        bold = dim = False
                    elif c == 7:
                        inv = True
                    elif c == 27:
                        inv = False
                    elif c == 39:
                        fg = None
                    elif c == 49:
                        bg = None
                    elif 30 <= c <= 37:
                        fg = BASE16[c - 30]
                    elif 90 <= c <= 97:
                        fg = BASE16[c - 90 + 8]
                    elif 40 <= c <= 47:
                        bg = BASE16[c - 40]
                    elif c in (38, 48) and k + 1 < len(p):
                        if p[k + 1] == 5 and k + 2 < len(p):
                            v = color(p[k + 2])
                            k += 2
                        elif p[k + 1] == 2 and k + 4 < len(p):
                            v = "#%02x%02x%02x" % tuple(p[k + 2:k + 5])
                            k += 4
                        else:
                            v = None
                        if c == 38:
                            fg = v
                        else:
                            bg = v
                    k += 1
                i = m.end()
                continue
            m = OTHER.match(line, i)
            if m:
                i = m.end()
                continue
            cells.append((line[i], fg, bg, bold, dim, inv))
            i += 1
        rows.append(cells)
    return rows


class Term:
    def __init__(self, font_px, cols, pad=14, lh=1.2):
        tmp = Path(tempfile.gettempdir()) / "commit-mono-demo.ttf"
        t = TTFont(HERE / "fonts" / "CommitMono-Regular.woff2")
        t.flavor = None
        t.save(tmp)
        self.px, self.cols, self.pad = font_px, cols, pad
        self.cw = font_px * 0.6
        self.lh = round(font_px * lh)
        self.main = ImageFont.truetype(str(tmp), font_px)
        self.cmap = TTFont(tmp).getBestCmap()
        self.fallbacks = []
        for f in FALLBACKS:
            if Path(f).exists():
                try:
                    ft = ImageFont.truetype(f, font_px)
                    self.fallbacks.append((ft, self._cmap(f)))
                except OSError:
                    pass
        self.cache = {}
        self.missing = set()

    def _cmap(self, f):
        try:
            return TTFont(f, fontNumber=0).getBestCmap()
        except Exception:
            return None

    def font_for(self, ch):
        if ord(ch) in self.cmap:
            return self.main
        for ft, cm in self.fallbacks:
            if cm and ord(ch) in cm:
                return ft
        self.missing.add(ch)
        return self.main

    def size(self, rows):
        w, h = round(self.cols * self.cw + 2 * self.pad), rows * self.lh + 2 * self.pad
        return w + w % 2, h + h % 2

    def render(self, screen, row_from=0, row_to=None):
        key = (screen, row_from, row_to)
        if key in self.cache:
            return self.cache[key]
        cells = parse(screen.rstrip("\n"))
        head = ["".join(c[0] for c in row) for row in cells[:4]]
        if any(re.search(r"Claude Code v|Opus|/cwd|[\u2590\u259b\u259c\u259d\u2598\u2588]|^\W*$", h) and h.strip() for h in head):
            cells = [[] if i < 4 else row for i, row in enumerate(cells)]
        cells = [[] if "weekly limit" in "".join(c[0] for c in row) else row for row in cells][row_from:row_to]
        w, h = self.size(len(cells))
        img = Image.new("RGB", (w, h), hexrgb(C["page"]))
        d = ImageDraw.Draw(img)
        asc = self.main.getmetrics()[0]
        off = (self.lh - (asc + self.main.getmetrics()[1])) / 2
        for r, row in enumerate(cells):
            y = self.pad + r * self.lh
            for c, (ch, fg, bg, bold, dim, inv) in enumerate(row[: self.cols]):
                x = self.pad + c * self.cw
                f, b = fg or C["t1"], bg
                if inv:
                    f, b = b or C["page"], fg or C["t1"]
                if b:
                    d.rectangle((x, y, x + self.cw - 1, y + self.lh - 1), fill=hexrgb(b))
                if ch != " ":
                    ch = SUBSTITUTE.get(ch, ch)
                    ft = self.font_for(ch)
                    col = hexrgb(f)
                    d.text((x, y + off), ch, font=ft, fill=col)
                    if bold:
                        d.text((x + 0.8, y + off), ch, font=ft, fill=col)
        self.cache[key] = img
        return img
