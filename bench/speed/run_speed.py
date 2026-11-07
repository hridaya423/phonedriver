"""Interleaved speed benchmark: phonedriver, phone-harness 0.3.3 and plain adb against one emulator, on the machine next to it.

Run on the host that owns the emulator, with the default local adb server (never through an ssh tunnel):
  python run_speed.py --phonedriver PATH\\phonedriver.exe --apk PATH\\app-debug.apk --serial emulator-5554 --out DIR

phone-harness must be importable (run with the Python of its venv). Every op is sampled in turn by every tool,
sample by sample, with the start order rotating, so drift on the host reaches all tools equally. The first
`--warmup` rounds of each op are discarded. Raw samples go to DIR/samples.csv, setup facts to DIR/meta.json.
"""
import argparse
import csv
import json
import os
import statistics
import subprocess
import sys
import tempfile
import threading
import time
import xml.etree.ElementTree as ET

PACKAGE = "dev.phonedriver.testbed"
ASCII_100 = ("The quick brown fox jumps over the lazy dog 0123456789. " * 3)[:100]
MIXED_100 = ("héllo 世界 مرحبا 👋🏽 abc 123 " * 10)[:100]
CLEAR = "input keycombination 113 29 && input keyevent 67"


def ms_now():
    return time.perf_counter() * 1000


class Pipe:
    def __init__(self, path):
        self.f = open(path, "r+b", buffering=0)
        self.buf = b""
        self.next = 1
        self.call("hello", {"version": VERSION[0], "client": "bench", "progress": False}, session=False)

    def call(self, method, params, session=True):
        if session:
            params = dict(params, session="speed")
        req = {"jsonrpc": "2.0", "id": self.next, "method": method, "params": params}
        self.next += 1
        self.f.write((json.dumps(req) + "\n").encode())
        while b"\n" not in self.buf:
            chunk = self.f.read(1 << 16)
            if not chunk:
                raise RuntimeError("daemon closed the pipe")
            self.buf += chunk
        line, self.buf = self.buf.split(b"\n", 1)
        reply = json.loads(line)
        if "error" in reply:
            raise RuntimeError(f"{method}: {reply['error']}")
        return reply["result"]


VERSION = [None]


class PU:
    """The phonedriver daemon. Its on-device agent holds the UiAutomation connection, and `uiautomator dump` (adb, phone-harness) cannot run beside it,
    so the daemon is down while those tools are sampled."""

    def __init__(self, exe, home, serial):
        self.exe, self.home, self.serial = exe, home, serial
        self.pipe = rf"\\.\pipe\phonedriver-speed-{os.getpid()}"
        self.env = dict(os.environ, PHONEDRIVER_HOME=home, PHONEDRIVER_SOCKET=self.pipe)
        self.proc = self.conn = None

    def start(self):
        if self.proc:
            return
        self.proc = subprocess.Popen([self.exe, "daemon"], env=self.env, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        assert self.proc.stdout.readline().startswith("ready")
        self.conn = Pipe(self.pipe)
        self.conn.call("snapshot", {"device": self.serial})

    def stop(self):
        if not self.proc:
            return
        self.conn.f.close()
        self.proc.terminate()
        self.proc.wait()
        subprocess.run([self.exe, "daemon", "stop"], env=self.env, capture_output=True)
        subprocess.run(["adb", "-s", self.serial, "shell", "pkill -f 'dev.phonedriver.agent.[M]ain'"], capture_output=True)
        time.sleep(1)
        self.proc = self.conn = None

    def call(self, method, params):
        return self.conn.call(method, params)


class Rig:
    def __init__(self, a):
        self.a = a
        self.serial = a.serial
        self.rows = []
        self.meta = {"load": {}}
        self.density = int(self.adb("shell", "wm density").split(":")[-1].split()[0]) / 160
        self.tmp = tempfile.mkdtemp(prefix="speed-")

    def adb(self, *args, raw=False):
        r = subprocess.run(["adb", "-s", self.serial, *args], capture_output=True)
        return r.stdout if raw else r.stdout.decode(errors="replace")

    def tab(self, name):
        self.adb("shell", f"am start -n {PACKAGE}/.MainActivity --es tab {name}")
        time.sleep(1.5)

    def dump(self):
        raw = self.adb("exec-out", "uiautomator", "dump", "/dev/tty", raw=True)
        return raw.split(b"</hierarchy>")[0] + b"</hierarchy>"

    def load(self, label):
        out = subprocess.run(
            ["powershell", "-NoProfile", "-Command",
             "(Get-Counter '\\Processor(_Total)\\% Processor Time' -SampleInterval 1 -MaxSamples 5).CounterSamples | % { [math]::Round($_.CookedValue,1) }"],
            capture_output=True, text=True).stdout.split()
        vals = [float(v) for v in out]
        self.meta["load"][label] = vals
        print(f"load {label}: {vals}", file=sys.stderr)

    def sample(self, op, tools, warmup, runs, between=None, blocks=0):
        """tools: list of (name, callable[, enter, exit]). Without `blocks`, each round runs every tool once with the start rotating.
        With `blocks`, tools that cannot run together (see PU) take turns in blocks of runs/blocks samples, round-robin, `warmup` discarded runs first."""
        self.load(f"before {op}")
        n = len(tools)
        out = {t[0]: [] for t in tools}
        if not blocks:
            for i in range(warmup + runs):
                for k in range(n):
                    name, fn = tools[(i + k) % n][:2]
                    t = ms_now()
                    fn()
                    took = ms_now() - t
                    if i >= warmup:
                        out[name].append(took)
                    if between:
                        between(name)
        else:
            per = runs // blocks
            for r in range(blocks):
                for k in range(n):
                    name, fn, enter, leave = tools[(r + k) % n]
                    enter()
                    for i in range((warmup if r == 0 else 3) + per):
                        t = ms_now()
                        fn()
                        took = ms_now() - t
                        if i >= (warmup if r == 0 else 3):
                            out[name].append(took)
                        if between:
                            between(name)
                    leave()
        for name, ms in out.items():
            for j, v in enumerate(ms):
                self.rows.append((op, name, j, f"{v:.3f}"))
        self.load(f"after {op}")
        print(f"{op}: done", file=sys.stderr)


def nodes(xml):
    out = set()
    for n in ET.fromstring(xml).iter("node"):
        a = n.attrib
        out.add((a["class"], a["resource-id"], a["text"], a["content-desc"], a["bounds"],
                 a["checked"], a["selected"], a["enabled"], a["focused"]))
    return out


def center(xml, pick):
    for n in ET.fromstring(xml).iter("node"):
        if pick(n.attrib):
            l, t, r, b = map(int, n.attrib["bounds"].replace("][", ",").strip("[]").split(","))
            return (l + r) // 2, (t + b) // 2
    raise SystemExit("node not found on screen")


class Clock:
    """Offset between the emulator's wall clock and the host's, from a persistent adb shell (no process start per read)."""

    def __init__(self, serial):
        self.p = subprocess.Popen(["adb", "-s", serial, "shell"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)

    def offset(self, n=60):
        best = None
        for _ in range(n):
            t0 = time.time()
            self.p.stdin.write(b"date +%s%N\n")
            self.p.stdin.flush()
            line = self.p.stdout.readline().strip()
            t1 = time.time()
            if not line.isdigit():
                continue
            dev = int(line) / 1e9
            rtt = t1 - t0
            if best is None or rtt < best[0]:
                best = (rtt, dev - (t0 + t1) / 2)
        return best


def device_side(rig, pu, ph, density, n, warmup):
    """Host time of sending a tap to the time the testbed's touch listener logged ACTION_DOWN, on the device clock."""
    rig.tab("gestures")
    downs = []
    lc = subprocess.Popen(["adb", "-s", rig.serial, "logcat", "-v", "epoch", "-s", "PUTB:I"], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)

    def reader():
        for raw in lc.stdout:
            line = raw.decode(errors="replace")
            if '"ACTION_DOWN"' in line and '"touch"' in line:
                downs.append(float(line.split()[0]))

    threading.Thread(target=reader, daemon=True).start()
    time.sleep(1)
    clock = Clock(rig.serial)
    rtt0, off0 = clock.offset()
    x, y = 120, 300
    tools = [
        ("phonedriver", lambda: pu.call("act", {"device": rig.serial, "action": {"kind": "tap", "target": {"point": {"x": x, "y": y}}}, "settle": False})),
        ("phone-harness", lambda: ph.tap(int(x * density), int(y * density))),
        ("adb", lambda: rig.adb("shell", f"input tap {int(x * density)} {int(y * density)}")),
    ]
    out = {name: [] for name, _ in tools}
    for i in range(warmup + n):
        for k in range(3):
            name, fn = tools[(i + k) % 3]
            before = len(downs)
            sent = time.time()
            fn()
            for _ in range(200):
                if len(downs) > before:
                    break
                time.sleep(0.005)
            if len(downs) > before and i >= warmup:
                out[name].append((downs[before] - off0 - sent) * 1000)
            time.sleep(0.15)
    rtt1, off1 = clock.offset()
    lc.kill()
    clock.p.kill()
    rig.meta["clock_offset_s"] = {"before": off0, "after": off1, "rtt_before_ms": rtt0 * 1000, "rtt_after_ms": rtt1 * 1000}
    for name, ms in out.items():
        for j, v in enumerate(ms):
            rig.rows.append(("tap, device side", name, j, f"{v:.3f}"))


def cold(rig, ph_python, exe, rounds):
    """Fresh tool to first screen read. phonedriver: daemon stopped, then one CLI snapshot (starts the daemon, deploys and boots the agent).
    phone-harness: a new Python process, import, first screen(). adb: one dump (the adb server is already running, as for any adb user)."""
    env = dict(os.environ, PHONEDRIVER_HOME=os.path.join(rig.tmp, "cold-home"))
    env.pop("PHONEDRIVER_SOCKET", None)
    penv = dict(os.environ, DO_NOT_TRACK="1", PHONE_HARNESS_TELEMETRY="0", PHONE_HARNESS_NO_UPDATE_CHECK="1", PHONE_HARNESS_PLATFORM="android", ANDROID_SERIAL=rig.serial)
    rig.adb("shell", "am start -a android.settings.SETTINGS")
    time.sleep(2.5)
    rig.load("before cold start")
    out = {"phonedriver": [], "phone-harness": [], "adb": []}
    for i in range(rounds + 1):
        for name in (["phonedriver", "phone-harness", "adb"] if i % 2 == 0 else ["adb", "phone-harness", "phonedriver"]):
            if name == "phonedriver":
                subprocess.run([exe, "daemon", "stop"], env=env, capture_output=True)
                time.sleep(1)
                t = ms_now()
                r = subprocess.run([exe, "--device", rig.serial, "snapshot"], env=env, capture_output=True)
                took = ms_now() - t
                assert r.returncode == 0, r.stderr[-300:]
                subprocess.run([exe, "daemon", "stop"], env=env, capture_output=True)
                rig.adb("shell", "pkill -f 'dev.phonedriver.agent.[M]ain'")
            elif name == "phone-harness":
                t = ms_now()
                r = subprocess.run([ph_python, "-c", "from phone_harness import helpers as h; h.screen()"], env=penv, capture_output=True)
                took = ms_now() - t
                assert r.returncode == 0, r.stderr[-300:]
            else:
                t = ms_now()
                rig.dump()
                took = ms_now() - t
            if i > 0:
                out[name].append(took)
            time.sleep(1)
    subprocess.run([exe, "daemon", "stop"], env=env, capture_output=True)
    for name, ms in out.items():
        for j, v in enumerate(ms):
            rig.rows.append(("cold start, first screen read", name, j, f"{v:.3f}"))
    rig.load("after cold start")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--phonedriver", required=True)
    ap.add_argument("--apk", required=True)
    ap.add_argument("--serial", default="emulator-5554")
    ap.add_argument("--out", required=True)
    ap.add_argument("--runs", type=int, default=50)
    ap.add_argument("--warmup", type=int, default=20)
    ap.add_argument("--cold-rounds", type=int, default=10)
    ap.add_argument("--ph-python", default=sys.executable)
    ap.add_argument("--only", default="")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    only = [s for s in a.only.split(";") if s]
    want = lambda name: not only or any((o[1:] == name) if o.startswith("=") else (o in name) for o in only)

    os.environ.update(DO_NOT_TRACK="1", PHONE_HARNESS_TELEMETRY="0", PHONE_HARNESS_NO_UPDATE_CHECK="1", PHONE_HARNESS_PLATFORM="android", ANDROID_SERIAL=a.serial)
    os.environ.pop("ADB_SERVER_SOCKET", None)
    from phone_harness import helpers as ph

    rig = Rig(a)
    rig.adb("shell", "input keyevent KEYCODE_WAKEUP")
    rig.adb("install", "-r", a.apk)
    ver = subprocess.run([a.phonedriver, "--version"], capture_output=True, text=True).stdout.strip()
    VERSION[0] = ver.split()[-1]
    home = os.path.join(rig.tmp, "home")
    os.makedirs(home)
    S = rig.serial
    d = rig.density
    W, R = a.warmup, a.runs
    puh = PU(a.phonedriver, home, S)
    BL = 5

    def pu_tool(fn):
        return ("phonedriver", fn, puh.start, puh.stop)

    def other(name, fn):
        return (name, fn, lambda: None, lambda: None)

    rig.meta.update({
        "date": time.strftime("%Y-%m-%d %H:%M:%S"),
        "phonedriver_version": ver,
        "adb_version": rig.adb("version").splitlines()[:2],
        "serial": S,
        "model": rig.adb("shell", "getprop ro.product.model").strip(),
        "api": rig.adb("shell", "getprop ro.build.version.sdk").strip(),
        "fingerprint": rig.adb("shell", "getprop ro.build.fingerprint").strip(),
        "wm_size": rig.adb("shell", "wm size").strip(),
        "wm_density": rig.adb("shell", "wm density").strip(),
        "devices": rig.adb("devices").splitlines()[1:],
        "runs": R, "warmup": W, "blocks": BL,
    })

    try:
        ph.screen()
        x, y = 120, 300
        rig.tab("taps")
        puh.start()
        if want("tap"):
            rig.sample("tap", [
                ("phonedriver", lambda: puh.call("act", {"device": S, "action": {"kind": "tap", "target": {"point": {"x": x, "y": y}}}, "settle": False})),
                ("phone-harness", lambda: ph.tap(int(x * d), int(y * d))),
                ("adb", lambda: rig.adb("shell", f"input tap {int(x * d)} {int(y * d)}")),
            ], W, R)
        if want("tap, paced"):
            rig.tab("taps")
            puh.start()
            rig.sample("tap, paced", [
                ("phonedriver", lambda: puh.call("act", {"device": S, "action": {"kind": "tap", "target": {"point": {"x": x, "y": y}}}, "settle": False})),
                ("phone-harness", lambda: ph.tap(int(x * d), int(y * d))),
                ("adb", lambda: rig.adb("shell", f"input tap {int(x * d)} {int(y * d)}")),
            ], W, R, lambda name: time.sleep(1.5))
            puh.stop()
        if want("screenshot"):
            def shot():
                with open(os.path.join(rig.tmp, "adb.png"), "wb") as f:
                    f.write(rig.adb("exec-out", "screencap", "-p", raw=True))
            rig.sample("screenshot", [
                ("phonedriver", lambda: puh.call("screenshot", {"device": S})),
                ("phone-harness", lambda: ph.screenshot()),
                ("adb", shot),
            ], W, R)
        if want("device side"):
            device_side(rig, puh, ph, d, R, W)
            rig.tab("taps")
        if want("tap, then"):
            el = puh.call("snapshot", {"device": S, "no_settle": True, "elements": True})["elements"]
            sw = next(e for e in el if e["role"] == "switch" and e["label"] == "Dark theme")["bounds"]
            px, py = sw["x"] + sw["w"] / 2, sw["y"] + sw["h"] / 2
            puh.stop()
            tx, ty = center(rig.dump(), lambda n: n["text"] == "Dark theme")
            state = {}

            def adb_enter():
                state["before"] = rig.dump()

            def adb_tap_diff():
                rig.adb("shell", f"input tap {tx} {ty}")
                after = rig.dump()
                assert nodes(state["before"]) != nodes(after), "dump after the tap shows no change"

            def pu_tap_diff():
                out = puh.call("act", {"device": S, "action": {"kind": "tap", "target": {"point": {"x": px, "y": py}}}, "settle": True})
                assert out["changed"] is True, out

            def between(name):
                time.sleep(0.3)
                if name == "adb":
                    state["before"] = rig.dump()

            rig.sample("tap, then what changed", [pu_tool(pu_tap_diff), ("adb", adb_tap_diff, adb_enter, lambda: None)], W, R, between, blocks=BL)
        puh.stop()

        def tree_tools():
            return [
                pu_tool(lambda: puh.call("snapshot", {"device": S, "no_settle": True})),
                other("phone-harness", lambda: ph.screen()),
                other("adb", lambda: rig.dump()),
                ("phonedriver, settled", lambda: puh.call("snapshot", {"device": S}), puh.start, puh.stop),
            ]

        if want("tree, Settings"):
            rig.adb("shell", "am force-stop com.android.settings")
            rig.adb("shell", "am start -a android.settings.SETTINGS")
            time.sleep(2.5)
            rig.sample("tree, Settings", tree_tools(), W, R, blocks=BL)
        if want("tree, list"):
            rig.tab("list")
            rig.sample("tree, list", tree_tools(), W, R, blocks=BL)
        if want("type"):
            rig.tab("text")
            fx, fy = center(rig.dump(), lambda n: n["class"].endswith("EditText"))
            rig.adb("shell", f"input tap {fx} {fy}")
            time.sleep(1.0)
            arg = "'" + ASCII_100.replace(" ", "%s") + "'"
            puh.start()
            rig.sample("type 100 characters", [
                ("phonedriver", lambda: puh.call("act", {"device": S, "action": {"kind": "type", "text": ASCII_100}, "settle": False})),
                ("phone-harness", lambda: ph.type_text(ASCII_100)),
                ("adb", lambda: rig.adb("shell", f"input text {arg}")),
                ("phonedriver, mixed Unicode", lambda: puh.call("act", {"device": S, "action": {"kind": "type", "text": MIXED_100}, "settle": False})),
            ], W, R, lambda name: rig.adb("shell", CLEAR))
            rig.adb("shell", CLEAR)
            puh.stop()
        try:
            if want("cold"):
                cold(rig, a.ph_python, a.phonedriver, a.cold_rounds)
        finally:
            pass
    finally:
        puh.stop()
        with open(os.path.join(a.out, "samples.csv"), "w", newline="") as f:
            w = csv.writer(f)
            w.writerow(["op", "tool", "i", "ms"])
            w.writerows(rig.rows)
        with open(os.path.join(a.out, "meta.json"), "w") as f:
            json.dump(rig.meta, f, indent=1)
    print("done", file=sys.stderr)


if __name__ == "__main__":
    main()
