#!/usr/bin/env python3
"""Usage: summarize.py <label=run-dir>... > jobs.json

A run killed by the 10-minute watchdog has no result event; its tokens are then summed from the assistant messages
(`tokens_estimated`).

Reads each run dir's results.jsonl (one line per run, written by run-harness.sh) and prints per-run rows and, per
harness and task, the median tokens, tool calls and wall time over the labelled passes plus the number of passes that
met their independent check. `--md` prints the markdown tables instead.
"""

import json
import os
import statistics
import sys
from collections import defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import jobs


def estimated_tokens(path):
    seen, total = set(), 0
    for line in open(path):
        try:
            v = json.loads(line)
        except ValueError:
            continue
        if v.get("type") != "assistant":
            continue
        m = v["message"]
        if m.get("id") in seen:
            continue
        seen.add(m.get("id"))
        u = m.get("usage", {})
        total += sum(u.get(k, 0) for k in ("input_tokens", "output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"))
    return total


def rescored(path, d):
    """The bug-report jobs under the current rubric (sections as well as sentences); the log checks stand as run."""
    base = f"{path}/{d['harness']}-{d['task']}"
    try:
        report = open(f"{base}.answer.txt").read()
        log_failures = [l for l in open(f"{base}.check.txt").read().splitlines() if l.startswith("log:")]
    except OSError:
        return d["success"]
    ok = all(jobs.found_bugs(report, jobs.BUGS[d["task"]]).values()) and jobs.has_steps(report)
    return ok and not log_failures


def load(args):
    runs = []
    for arg in args:
        label, path = arg.split("=", 1)
        for line in open(f"{path}/results.jsonl"):
            d = json.loads(line)
            estimated = d["total_tokens"] == 0
            if estimated:
                d["total_tokens"] = estimated_tokens(f"{path}/{d['harness']}-{d['task']}.jsonl")
            runs.append(
                {
                    "pass": label,
                    "harness": d["harness"],
                    "task": d["task"],
                    "success": rescored(path, d) if d["task"] in jobs.BUGS else d["success"],
                    "success_as_run": d["success"],
                    "total_tokens": d["total_tokens"],
                    "tokens_estimated": estimated,
                    "output_tokens": d["output_tokens"],
                    "tool_calls": d["tool_calls"],
                    "wall_s": d["wall_s"],
                    "cost_usd": d.get("cost_usd"),
                }
            )
    return runs


def medians(runs):
    groups = defaultdict(list)
    for r in runs:
        groups[(r["harness"], r["task"])].append(r)
    out = []
    for (harness, task), rs in sorted(groups.items()):
        out.append(
            {
                "harness": harness,
                "task": task,
                "runs": len(rs),
                "passed": sum(r["success"] for r in rs),
                "median_tokens": statistics.median(r["total_tokens"] for r in rs),
                "median_tool_calls": statistics.median(r["tool_calls"] for r in rs),
                "median_wall_s": statistics.median(r["wall_s"] for r in rs),
            }
        )
    return out


def totals(med):
    by = defaultdict(lambda: [0, 0, 0, 0, 0])
    for m in med:
        t = by[m["harness"]]
        t[0] += m["passed"]
        t[1] += m["runs"]
        t[2] += m["median_tokens"]
        t[3] += m["median_tool_calls"]
        t[4] += m["median_wall_s"]
    return {
        h: {
            "passed": t[0],
            "runs": t[1],
            "sum_median_tokens": t[2],
            "sum_median_tool_calls": t[3],
            "sum_median_wall_s": t[4],
        }
        for h, t in by.items()
    }


def markdown(runs, med):
    lines = ["| Job | Harness | Passes | Median tokens | Median calls | Median wall s |", "|---|---|---|---|---|---|"]
    for m in med:
        lines.append(
            f"| {m['task']} | {m['harness']} | {m['passed']}/{m['runs']} | {m['median_tokens']:,.0f} | {m['median_tool_calls']:g} | {m['median_wall_s']:g} |"
        )
    return "\n".join(lines)


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if a != "--md"]
    runs = load(args)
    med = medians(runs)
    if "--md" in sys.argv:
        print(markdown(runs, med))
    else:
        print(json.dumps({"runs": runs, "medians": med, "totals": totals(med)}, indent=1))
