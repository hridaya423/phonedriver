"""Main display against hidden display on one device: snapshot, tap and screenshot latency through one daemon.

  PHONEDRIVER_HOME=DIR python run_display.py --phonedriver PATH --device emulator-5558 --out DIR [--runs 20] [--rounds 2]

A mode switch restarts the agent (several seconds), so the modes run in blocks, main then hidden, `--rounds` times, and
each op is sampled `--runs` times per block after `--warmup` discarded runs. Every call is timed from the client side of
the daemon socket, so the numbers include the adb path to the device, like the other speed runs.
"""
import argparse
import json
import os
import socket
import statistics
import subprocess
import sys
import time

TAP = {"kind": "tap", "target": {"point": {"x": 640.0, "y": 600.0}}}


class Daemon:
    def __init__(self, exe, sock, device):
        self.device = device
        self.proc = subprocess.Popen([exe, "daemon"], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        assert self.proc.stdout.readline().startswith("ready")
        self.s = socket.socket(socket.AF_UNIX)
        self.s.connect(sock)
        self.f = self.s.makefile("rwb", buffering=0)
        self.next = 1
        version = subprocess.run([exe, "version"], capture_output=True, text=True).stdout.split()[-1]
        self.call("hello", {"version": version, "client": "bench", "progress": False}, device=False)

    def call(self, method, params, device=True):
        params = dict(params, session="display")
        if device:
            params["device"] = self.device
        self.f.write((json.dumps({"jsonrpc": "2.0", "id": self.next, "method": method, "params": params}) + "\n").encode())
        self.next += 1
        reply = json.loads(self.f.readline())
        if "error" in reply:
            raise RuntimeError(f"{method}: {reply['error']}")
        return reply["result"]

    def stop(self):
        try:
            self.call("shutdown", {}, device=False)
        except Exception:
            pass
        self.proc.wait(timeout=10)


def percentile(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, round(p / 100 * (len(xs) - 1)))]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--phonedriver", required=True)
    ap.add_argument("--device", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--runs", type=int, default=20)
    ap.add_argument("--warmup", type=int, default=5)
    ap.add_argument("--rounds", type=int, default=2)
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    sock = os.environ.get("PHONEDRIVER_SOCKET") or os.path.join(os.environ["PHONEDRIVER_HOME"], "run", "daemon.sock")
    d = Daemon(a.phonedriver, sock, a.device)
    ops = {
        "snapshot": lambda mode: d.call("snapshot", {"no_settle": True, **mode}),
        "tap": lambda mode: d.call("act", {"action": TAP, "settle": False, **mode}),
        "screenshot": lambda mode: d.call("screenshot", {**mode}),
    }
    samples = {(m, op): [] for m in ("main", "hidden") for op in ops}
    try:
        for r in range(a.rounds):
            for m in ("main", "hidden"):
                mode = {"display": "hidden"} if m == "hidden" else {}
                d.call("act", {"action": {"kind": "launch", "app": "dev.phonedriver.testbed"}, **mode})
                time.sleep(1.0)
                for op, fn in ops.items():
                    for i in range(a.warmup + a.runs):
                        t = time.perf_counter()
                        fn(mode)
                        ms = (time.perf_counter() - t) * 1000
                        if i >= a.warmup:
                            samples[(m, op)].append(ms)
                        time.sleep(0.3)
                d.call("release", {})
                print(f"round {r + 1} {m} done", file=sys.stderr)
    finally:
        d.stop()
    rows = ["op,mode,n,p50_ms,p95_ms,mean_ms"]
    for op in ops:
        for m in ("main", "hidden"):
            xs = samples[(m, op)]
            rows.append(f"{op},{m},{len(xs)},{statistics.median(xs):.1f},{percentile(xs, 95):.1f},{statistics.mean(xs):.1f}")
    open(os.path.join(a.out, "summary.csv"), "w").write("\n".join(rows) + "\n")
    with open(os.path.join(a.out, "samples.csv"), "w") as f:
        f.write("op,mode,ms\n")
        for (m, op), xs in samples.items():
            f.writelines(f"{op},{m},{x:.2f}\n" for x in xs)
    print("\n".join(rows))


main()
