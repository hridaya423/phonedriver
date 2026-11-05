#!/usr/bin/env python3
# Spike I4 (throwaway): time WebDriverAgent GET /source on the booted simulator.
# usage: time_wda.py <udid> <runs>
import json
import re
import statistics
import subprocess
import sys
import time
import urllib.request

WDA = "http://127.0.0.1:8100"


def call(method, path, body=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(WDA + path, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=120) as r:
        return json.loads(r.read())


def depth_and_count(xml):
    depth = best = count = 0
    for m in re.finditer(r"<(/?)XCUIElementType\w+[^>]*?(/?)>", xml):
        closing, selfclose = m.group(1), m.group(2)
        if closing:
            depth -= 1
            continue
        count += 1
        best = max(best, depth + 1)
        if not selfclose:
            depth += 1
    return best, count


def pct(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(round(p / 100 * (len(xs) - 1))))]


def main():
    udid, runs = sys.argv[1], int(sys.argv[2])
    sid = call("POST", "/session", {"capabilities": {"alwaysMatch": {}}})["value"]["sessionId"]
    screens = [
        ("testbed list", ["xcrun", "simctl", "launch", "--terminate-running-process", udid,
                          "dev.phoneuse.testbed", "-tab", "list"]),
        ("testbed taps", ["xcrun", "simctl", "launch", "--terminate-running-process", udid,
                          "dev.phoneuse.testbed", "-tab", "taps"]),
        ("settings", ["xcrun", "simctl", "launch", "--terminate-running-process", udid,
                      "com.apple.Preferences"]),
    ]
    for name, launch in screens:
        subprocess.run(launch, check=True, capture_output=True)
        time.sleep(3)
        variants = [
            ("default (maxDepth 50)", {"snapshotMaxDepth": 50}, "/source"),
            ("maxDepth 30", {"snapshotMaxDepth": 30}, "/source"),
            ("maxDepth 30, no idle waits, excluded visible/accessible/hittable",
             {"snapshotMaxDepth": 30, "waitForIdleTimeout": 0, "animationCoolOffTimeout": 0},
             "/source?excluded_attributes=visible,accessible,hittable"),
            ("same, format=json",
             {"snapshotMaxDepth": 30, "waitForIdleTimeout": 0, "animationCoolOffTimeout": 0},
             "/source?format=json&excluded_attributes=visible,accessible,hittable"),
        ]
        for label, settings, path in variants:
            call("POST", f"/session/{sid}/appium/settings", {"settings": settings})
            for _ in range(2):
                call("GET", f"/session/{sid}{path}")
            times = []
            for _ in range(runs):
                t0 = time.perf_counter()
                value = call("GET", f"/session/{sid}{path}")["value"]
                times.append((time.perf_counter() - t0) * 1000)
            d, n = depth_and_count(value) if isinstance(value, str) else (None, json.dumps(value).count('"type"'))
            print(json.dumps({"screen": name, "wda": label, "runs": runs,
                              "p50_ms": round(statistics.median(times), 1),
                              "p95_ms": round(pct(times, 95), 1),
                              "min_ms": round(min(times), 1),
                              "tree_depth": d, "elements": n}), flush=True)


main()
