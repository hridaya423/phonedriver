#!/usr/bin/env python3
import json
import socket
import statistics
import struct
import subprocess
import sys
import time
import urllib.request

SCREENS = [
    ("testbed list", ["dev.phoneuse.testbed", "-tab", "list"]),
    ("testbed taps", ["dev.phoneuse.testbed", "-tab", "taps"]),
    ("settings", ["com.apple.Preferences"]),
]


def pct(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(round(p / 100 * (len(xs) - 1))))]


def row(**fields):
    print(json.dumps(fields), flush=True)


class Runner:
    def __init__(self):
        self.s = socket.create_connection(("127.0.0.1", 27753))
        self.s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)

    def call(self, msg):
        b = json.dumps(msg).encode()
        self.s.sendall(struct.pack(">I", len(b)) + b)
        n = struct.unpack(">I", self._read(4))[0]
        return json.loads(self._read(n))

    def _read(self, n):
        buf = b""
        while len(buf) < n:
            buf += self.s.recv(n - len(buf))
        return buf


def wda(method, path, body=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request("http://127.0.0.1:8100" + path, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=120) as r:
        return json.loads(r.read())


def timed(fn, runs):
    for _ in range(2):
        fn()
    times = []
    for _ in range(runs):
        t0 = time.perf_counter()
        out = fn()
        times.append((time.perf_counter() - t0) * 1000)
    return times, out


def main():
    udid, which, runs = sys.argv[1], sys.argv[2], int(sys.argv[3])
    if which == "wda":
        sid = wda("POST", "/session", {"capabilities": {"alwaysMatch": {}}})["value"]["sessionId"]
    else:
        r = Runner()
    for name, launch in SCREENS:
        subprocess.run(["xcrun", "simctl", "launch", "--terminate-running-process", udid, *launch],
                       check=True, capture_output=True)
        time.sleep(3)
        if which == "wda":
            variants = [
                ("depth 30", {"snapshotMaxDepth": 30}, "/source"),
                ("depth 30, excluded attributes", {"snapshotMaxDepth": 30},
                 "/source?excluded_attributes=visible,accessible,hittable"),
                ("depth 30, excluded attributes, idle waits off",
                 {"snapshotMaxDepth": 30, "waitForIdleTimeout": 0, "animationCoolOffTimeout": 0},
                 "/source?excluded_attributes=visible,accessible,hittable"),
            ]
            for label, settings, path in variants:
                wda("POST", f"/session/{sid}/appium/settings", {"settings": settings})
                times, out = timed(lambda: wda("GET", f"/session/{sid}{path}")["value"], runs)
                row(screen=name, who="wda", variant=label, runs=runs,
                    p50=round(statistics.median(times), 1), p95=round(pct(times, 95), 1),
                    elements=out.count("<XCUIElementType"))
        else:
            times, out = timed(lambda: r.call({"id": 1, "op": "snapshot", "max_depth": 30}), runs)
            row(screen=name, who="runner", variant="snapshot max_depth 30", runs=runs,
                p50=round(statistics.median(times), 1), p95=round(pct(times, 95), 1),
                elements=len(out["nodes"]))
            for fmt, q in (("jpeg", 80), ("jpeg", 30), ("png", 100)):
                times, out = timed(lambda: r.call({"id": 2, "op": "screenshot", "format": fmt, "quality": q}), runs)
                row(screen=name, who="runner", variant=f"screenshot {fmt} q{q}", runs=runs,
                    p50=round(statistics.median(times), 1), p95=round(pct(times, 95), 1),
                    bytes=len(out["data_b64"]) * 3 // 4)


main()
