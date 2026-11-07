import io
import uharfbuzz as hb
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen

HERE = __import__("pathlib").Path(__file__).parent

LIGHT = dict(
    page="#ffffff", panel="#fafaf7", well="#f4f3ee", selected="#fff4bf",
    t1="#161512", t2="#55524a", t3="#6b675e", faint="#efede6", line="#e2dfd5",
    control="#8a8478", mark="#8a8478", dot="#dedad0", accent="#ffd400",
    edge="#c9a700", atext="#7a5d00", ok="#2c6a3a", fail="#b3261e",
    d1="#ffd400", d2="#8c7a3c", d3="#9a937c", d4="#5b4a1a", d5="#2e3a4a",
)
DARK = dict(
    page="#141412", panel="#1c1b19", well="#22211e", selected="#3a3107",
    t1="#f5f3ee", t2="#c4c0b6", t3="#a19c90", faint="#22211e", line="#2c2a25",
    control="#736d61", mark="#736d61", dot="#2a2824", accent="#ffd400",
    edge="#ffd400", atext="#ffd400", ok="#8cc79a", fail="#ff6b57",
    d1="#ffd400", d2="#ebdc9c", d3="#8f8a76", d4="#8a6d1f", d5="#5d7290",
)
THEMES = {"light": LIGHT, "dark": DARK}


class Font:
    def __init__(self, name):
        t = TTFont(HERE / "fonts" / name)
        t.flavor = None
        buf = io.BytesIO()
        t.save(buf)
        self.tt = TTFont(io.BytesIO(buf.getvalue()))
        self.upem = self.tt["head"].unitsPerEm
        self.gs = self.tt.getGlyphSet()
        self.order = self.tt.getGlyphOrder()
        self.hb = hb.Font(hb.Face(hb.Blob(buf.getvalue())))
        self.cap = self.tt["OS/2"].sCapHeight / self.upem

    def run(self, text, size, tracking=0.0):
        buf = hb.Buffer()
        buf.add_str(text)
        buf.guess_segment_properties()
        hb.shape(self.hb, buf, {"kern": True, "liga": True})
        s = size / self.upem
        x = 0.0
        out = []
        for info, pos in zip(buf.glyph_infos, buf.glyph_positions):
            pen = SVGPathPen(self.gs, ntos=lambda v: ("%.2f" % v).rstrip("0").rstrip("."))
            tp = TransformPen(pen, (s, 0, 0, -s, x + pos.x_offset * s, -pos.y_offset * s))
            self.gs[self.order[info.codepoint]].draw(tp)
            out.append(pen.getCommands())
            x += pos.x_advance * s + tracking * size
        return " ".join(c for c in out if c), x - tracking * size


INTER = Font("InterDisplay-Regular.woff2")
MONO = Font("CommitMono-Regular.woff2")


def text(s, x, y, font, size, fill, tracking=0.0, anchor="start", caps=False, extra=""):
    if caps:
        s = s.upper()
    d, w = font.run(s, size, tracking)
    if anchor == "end":
        x -= w
    elif anchor == "middle":
        x -= w / 2
    return f'<path transform="translate({x:g} {y:g})" fill="{fill}" d="{d}"{extra}/>', w


def width(s, font, size, tracking=0.0, caps=False):
    return font.run(s.upper() if caps else s, size, tracking)[1]


def rect(x, y, w, h, fill, extra=""):
    return f'<rect x="{x:g}" y="{y:g}" width="{w:g}" height="{h:g}" fill="{fill}"{extra}/>'


def frame(x, y, w, h, line, fill=None, t=1):
    f = rect(x, y, w, h, fill) if fill else ""
    return f + (
        rect(x, y, w, t, line) + rect(x, y + h - t, w, t, line)
        + rect(x, y, t, h, line) + rect(x + w - t, y, t, h, line)
    )


def dots(x, y, w, h, color, pitch=4, r=0.75):
    pid = f"dots{int(x)}_{int(y)}"
    return (
        f'<pattern id="{pid}" x="{x:g}" y="{y:g}" width="{pitch}" height="{pitch}" patternUnits="userSpaceOnUse">'
        f'<circle cx="{pitch/2:g}" cy="{pitch/2:g}" r="{r}" fill="{color}"/></pattern>'
        f'<rect x="{x:g}" y="{y:g}" width="{w:g}" height="{h:g}" fill="url(#{pid})"/>'
    )


def svg(w, h, body, label, bg=None):
    b = rect(0, 0, w, h, bg) if bg else ""
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" role="img" '
        f'aria-label="{label}">{b}{body}</svg>\n'
    )


def mark(size, c, x=0, y=0, ground=None, inset=False, solid=False, ink=None):
    """Phone frame 8 wide by 16 tall on a 16 grid, two rules, the middle row on the selection yellow.
    Rule weight is 1u below 48px and 0.5u from 48px. inset puts the frame at y 1 to 15 inside a ground tile.
    solid fills the middle row with ink for one-colour use."""
    k = size / 16
    st = 1 if size < 48 else 0.5
    ink = ink or c["t1"]
    out = []

    def R(a, b, w, h, fill):
        out.append(rect(round(x + a * k, 3), round(y + b * k, 3), round(w * k, 3), round(h * k, 3), fill))

    if ground:
        R(0, 0, 16, 16, ground)
    y0 = 1 if inset else 0
    h = 16 - 2 * y0
    R(4, y0, 8, st, ink); R(4, y0 + h - st, 8, st, ink)
    R(4, y0, st, h, ink); R(12 - st, y0, st, h, ink)
    R(5, 5 + st, 6, 5 - st, ink if solid else c["accent"])
    R(5, 5, 6, st, ink); R(5, 10, 6, st, ink)
    return "".join(out)


def lockup(c, x, y, ms, ink=None, solid=False):
    """Phone left edge at x, mark top at y. Returns (svg, width)."""
    fs = ms * 0.875
    out = mark(ms, c, x=x - ms * 0.25, y=y, ink=ink, solid=solid)
    xh = INTER.tt["OS/2"].sxHeight / INTER.upem * fs
    t, w = text("phonedriver", x + ms * 0.875, y + ms / 2 + xh / 2, INTER, fs, ink or c["t1"], -0.035)
    return out + t, ms * 0.875 + w
