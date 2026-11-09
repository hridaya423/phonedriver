#!/usr/bin/env python3
"""Usage: compare.py <jobs.json> --base p4,p5 --new p6,p7,p8 [--harness phonedriver]

Compares two groups of passes of the same jobs, read from the `runs` of a summarize.py output. For each job it prints
the median tokens and wall time of each group and a verdict: `lower` or `higher` when no run of one group falls inside
the range of the other, `noise` when the ranges overlap. Two or three passes per group cannot show a small change;
they show whether a change is larger than the run-to-run spread.
"""

import json
import statistics
import sys
from collections import defaultdict


def verdict(base, new):
    if max(new) < min(base):
        return "lower"
    if min(new) > max(base):
        return "higher"
    return "noise"


def rows(runs, base, new, harness):
    groups = defaultdict(lambda: {"base": [], "new": []})
    for r in runs:
        if r["harness"] != harness:
            continue
        side = "base" if r["pass"] in base else "new" if r["pass"] in new else None
        if side:
            groups[r["task"]][side].append(r)
    out = []
    for task, g in sorted(groups.items()):
        if not g["base"] or not g["new"]:
            continue
        row = {"task": task}
        for side in ("base", "new"):
            row[f"{side}_passed"] = f"{sum(r['success'] for r in g[side])}/{len(g[side])}"
        for key, name in (("total_tokens", "tokens"), ("wall_s", "wall_s")):
            b = [r[key] for r in g["base"]]
            n = [r[key] for r in g["new"]]
            row[f"base_{name}"] = statistics.median(b)
            row[f"new_{name}"] = statistics.median(n)
            row[f"{name}_change_pct"] = round(100 * (statistics.median(n) - statistics.median(b)) / statistics.median(b), 1)
            row[f"{name}_verdict"] = verdict(b, n)
        out.append(row)
    return out


def markdown(table):
    lines = [
        "| Job | Passed before | Passed now | Tokens before | Tokens now | Change | Verdict | Wall s before | Wall s now | Verdict |",
        "|---|---|---|---|---|---|---|---|---|---|",
    ]
    for r in table:
        lines.append(
            f"| {r['task']} | {r['base_passed']} | {r['new_passed']} | {r['base_tokens']:,.0f} | {r['new_tokens']:,.0f} | "
            f"{r['tokens_change_pct']:+.1f}% | {r['tokens_verdict']} | {r['base_wall_s']:g} | {r['new_wall_s']:g} | {r['wall_s_verdict']} |"
        )
    return "\n".join(lines)


if __name__ == "__main__":
    args = sys.argv[1:]
    opt = lambda name, default=None: args[args.index(name) + 1] if name in args else default
    data = json.load(open(args[0]))
    print(markdown(rows(data["runs"], opt("--base").split(","), opt("--new").split(","), opt("--harness", "phonedriver"))))
