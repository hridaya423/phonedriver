import io
from fontTools import subset
from fontTools.ttLib import TTFont

RANGES = "U+0020-007E,U+00A0-00FF,U+2010-2015,U+2018-201F,U+2022,U+2026,U+2190-2193,U+2212,U+00D7,U+2011,U+2248,U+2264,U+2265"
ARIAL = {"xavg": 904 / 2048, "asc": 0.905, "desc": 0.212}


def sub(src, dst):
    opts = subset.Options()
    opts.flavor = "woff2"
    opts.layout_features = ["kern", "liga", "calt", "tnum", "case", "ccmp", "locl", "mark", "mkmk"]
    opts.name_IDs = [1, 2]
    opts.notdef_outline = True
    f = TTFont(src)
    s = subset.Subsetter(opts)
    unis = []
    for part in RANGES.split(","):
        a = part[2:].split("-")
        lo = int(a[0], 16)
        hi = int(a[-1], 16)
        unis += list(range(lo, hi + 1))
    s.populate(unicodes=unis)
    s.subset(f)
    f.flavor = "woff2"
    f.save(dst)
    return f


def metrics(path, mono=False):
    f = TTFont(path)
    upem = f["head"].unitsPerEm
    hh = f["hhea"]
    cmap = f.getBestCmap()
    hm = f["hmtx"]
    ws = [hm[cmap[c]][0] for c in range(97, 123) if c in cmap]
    avg = sum(ws) / len(ws) / upem
    ref = 0.5 if mono else 0.46
    return {"avg": avg, "asc": hh.ascent / upem, "desc": -hh.descent / upem}


def build(outdir, srcdir):
    i = srcdir / "InterDisplay-Regular.woff2"
    m = srcdir / "CommitMono-Regular.woff2"
    sub(i, outdir / "inter-display.woff2")
    sub(m, outdir / "commit-mono.woff2")
    mi = metrics(i)
    mm = metrics(m, True)
    arial_avg = 0.4614
    menlo_avg = 0.602
    isa = mi["avg"] / arial_avg
    msa = mm["avg"] / menlo_avg
    return {
        "@@ISA@@": "%.1f" % (isa * 100),
        "@@IAS@@": "%.1f" % (mi["asc"] / isa * 100),
        "@@IDE@@": "%.1f" % (mi["desc"] / isa * 100),
        "@@MSA@@": "%.1f" % (msa * 100),
        "@@MAS@@": "%.1f" % (mm["asc"] / msa * 100),
        "@@MDE@@": "%.1f" % (mm["desc"] / msa * 100),
    }
