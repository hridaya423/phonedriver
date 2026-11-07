"""Turns DIR/samples.csv (from run_speed.py) into p50, p95 and bootstrap 95% confidence intervals on the p50 ratios.

  python stats.py DIR [--also DIR2]... [--json docs/bench/speed.json] [--md out.md]

With --also, the ops in DIR2 replace the same ops in DIR; the replaced rows stay in the tables as "(first pass)".

Ratios are adb / phonedriver and phone-harness / phonedriver of the p50s. Each tool's samples are resampled on their own,
10,000 times, with a fixed seed. The headline ratio of a row is its p50 ratio, unless the lower bound of the interval
is below 90% of it, in which case the lower bound is the headline.
"""
import argparse
import csv
import json
import math
import random
import statistics
from collections import defaultdict


def pct(v, p):
    v = sorted(v)
    return v[max(0, min(len(v) - 1, math.ceil(p * len(v)) - 1))]


def boot_ratio(num, den, n=10000, seed=7):
    rng = random.Random(seed)
    out = []
    for _ in range(n):
        a = pct([rng.choice(num) for _ in num], 0.5)
        b = pct([rng.choice(den) for _ in den], 0.5)
        out.append(a / b)
    out.sort()
    return out[int(0.025 * n)], out[int(0.975 * n)]


def load(d):
    data = defaultdict(lambda: defaultdict(list))
    with open(f"{d}/samples.csv") as f:
        for r in csv.DictReader(f):
            data[r["op"]][r["tool"]].append(float(r["ms"]))
    return data


CHART = [
    ("tap", "Tap, back to back", "tap"),
    ("tap, paced", "Tap, 1.5 s apart", "tap_paced"),
    ("screenshot", "Screenshot", "screenshot"),
    ("tree, Settings", "UI tree, Settings", "tree_settings"),
    ("tree, list", "UI tree, 1,000-row list", "tree_list"),
    ("type 100 characters", "Type 100 characters", "type100"),
    ("tap, then what changed", "Tap, then what changed", "tap_diff"),
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dir")
    ap.add_argument("--json")
    ap.add_argument("--md")
    ap.add_argument("--also", action="append", default=[])
    a = ap.parse_args()
    data = load(a.dir)
    for also in a.also:
        for op, tools in load(also).items():
            if op in data:
                data[op + " (first pass)"] = data[op]
            data[op] = tools
    meta = json.load(open(f"{a.dir}/meta.json"))
    lines = ["| op | tool | n | p50 ms | p95 ms |", "|---|---|---|---|---|"]
    ratios = ["| op | adb / phonedriver (p50) | 95% CI | phone-harness / phonedriver (p50) | 95% CI |", "|---|---|---|---|---|"]
    ops = []
    for op, tools in data.items():
        for tool, v in tools.items():
            lines.append(f"| {op} | {tool} | {len(v)} | {pct(v, .5):.1f} | {pct(v, .95):.1f} |")
        pu = tools.get("phonedriver")
        if not pu:
            continue
        cells = []
        for other in ("adb", "phone-harness"):
            o = tools.get(other)
            if not o:
                cells += ["not supported", ""]
                continue
            r = pct(o, .5) / pct(pu, .5)
            lo, hi = boot_ratio(o, pu)
            cells += [f"{r:.1f}x", f"{lo:.1f} to {hi:.1f}"]
        ratios.append(f"| {op} | " + " | ".join(cells) + " |")
    for op, label, key in CHART:
        t = data.get(op)
        if not t:
            continue
        row = {"id": key, "label": label}
        for tool, k in (("phonedriver", "phonedriver"), ("phone-harness", "phone_harness"), ("adb", "adb")):
            v = t.get(tool)
            row[k] = round(pct(v, .5), 1) if v else None
            row[k + "_p95"] = round(pct(v, .95), 1) if v else None
        r = pct(t["adb"], .5) / pct(t["phonedriver"], .5)
        lo, hi = boot_ratio(t["adb"], t["phonedriver"])
        row["adb_ratio_ci"] = [round(lo, 2), round(hi, 2)]
        row["headline_adb"] = round(lo if lo < 0.9 * r else r, 2)
        if t.get("phone-harness"):
            r2 = pct(t["phone-harness"], .5) / pct(t["phonedriver"], .5)
            lo2, hi2 = boot_ratio(t["phone-harness"], t["phonedriver"])
            row["phone_harness_ratio_ci"] = [round(lo2, 2), round(hi2, 2)]
        ops.append(row)
    md = "\n".join(lines) + "\n\n" + "\n".join(ratios) + "\n"
    if a.md:
        open(a.md, "w").write(md)
    print(md)
    if a.json:
        out = {
            "title": "phonedriver against adb and phone-harness",
            "subtitle": f"Android 17 emulator (API {meta['api']}), p50 of {meta['runs']} runs",
            "source": "docs/bench/speed-2026-11-07.md",
            "ops": ops,
        }
        json.dump(out, open(a.json, "w"), indent=1)
        print("wrote", a.json)


if __name__ == "__main__":
    main()
