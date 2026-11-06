"""Latency baseline for plain adb on the same emulator, screens and operations as phone-use-bench.

Standard library only. Run on the machine next to the emulator with the default local adb server:
  python run_adb.py --serial emulator-5554 --runs 50 --warmup 5 [--csv out.csv]

Each op is one adb client process per call, timed from spawn to exit with the output read.
`adb shell true` is measured too: it is the cost of reaching the device at all.
"""
import argparse
import math
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

PACKAGE = "dev.phoneuse.testbed"
ASCII_100 = ("The quick brown fox jumps over the lazy dog 0123456789. " * 3)[:100]
MIXED = "héllo 世界 مرحبا 👋🏽 abc 123 "
CLEAR = "input keycombination 113 29 && input keyevent 67"


def pct(values, p):
    v = sorted(values)
    return v[max(0, min(len(v) - 1, math.ceil(p * len(v)) - 1))]


class Rig:
    def __init__(self, serial, runs, warmup):
        self.serial, self.runs, self.warmup, self.rows = serial, runs, warmup, []

    def adb(self, *args, raw=False):
        r = subprocess.run(["adb", "-s", self.serial, *args], capture_output=True)
        return r.stdout if raw else r.stdout.decode(errors="replace")

    def tab(self, name):
        self.adb("shell", f"am start -n {PACKAGE}/.MainActivity --es tab {name}")
        time.sleep(1.5)

    def dump(self):
        raw = self.adb("exec-out", "uiautomator", "dump", "/dev/tty", raw=True)
        return raw.split(b"</hierarchy>")[0] + b"</hierarchy>"

    def measure(self, name, note, op, between=None):
        ms = []
        for i in range(self.warmup + self.runs):
            t = time.perf_counter()
            op()
            took = (time.perf_counter() - t) * 1000
            if i >= self.warmup:
                ms.append(took)
            if between:
                between()
        print(f"{name}: {len(ms)} runs done", file=sys.stderr)
        self.rows.append((name, note, ms))


def nodes(xml):
    out = set()
    for n in ET.fromstring(xml).iter("node"):
        a = n.attrib
        out.add((a["class"], a["resource-id"], a["text"], a["content-desc"], a["bounds"],
                 a["checked"], a["selected"], a["enabled"], a["focused"]))
    return out


def diff(before, after):
    b, a = nodes(before), nodes(after)
    return sorted(b - a), sorted(a - b)


def center(xml, pick):
    for n in ET.fromstring(xml).iter("node"):
        if pick(n.attrib):
            l, t, r, bt = map(int, n.attrib["bounds"].replace("][", ",").strip("[]").split(","))
            return (l + r) // 2, (t + bt) // 2
    raise SystemExit("node not found on screen")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--serial", required=True)
    ap.add_argument("--runs", type=int, default=50)
    ap.add_argument("--warmup", type=int, default=5)
    ap.add_argument("--csv")
    a = ap.parse_args()
    rig = Rig(a.serial, a.runs, a.warmup)
    density = int(rig.adb("shell", "wm density").split(":")[-1].split()[0]) / 160

    rig.tab("taps")
    rig.measure("adb floor (`shell true`)", "one adb process, nothing run on the device",
                lambda: rig.adb("shell", "true"))
    x, y = int(120 * density), int(300 * density)
    rig.measure("tap", "`adb shell input tap X Y`, returns before the app reacts",
                lambda: rig.adb("shell", f"input tap {x} {y}"))

    def shot():
        data = rig.adb("exec-out", "screencap", "-p", raw=True)
        with open("adb-shot.png", "wb") as f:
            f.write(data)

    rig.measure("screenshot (settled Taps tab)", "`exec-out screencap -p` to a file, 1080x2424 PNG", shot)

    tx, ty = center(rig.dump(), lambda n: n["text"] == "Dark theme")
    state = {}

    def tap_dump_diff(wait_for_quiet):
        def op():
            rig.adb("shell", f"input tap {tx} {ty}")
            after = rig.dump()
            if wait_for_quiet:
                while True:
                    again = rig.dump()
                    if again == after:
                        break
                    after = again
            gone, new = diff(state["before"], after)
            assert gone or new, "dump after the tap shows no change"
            state["before"] = after
        return op

    state["before"] = rig.dump()
    rig.measure("tap, then know what changed (one dump)",
                "tap, `uiautomator dump`, parse and diff in the caller; the dump itself waits for idle",
                tap_dump_diff(False), lambda: time.sleep(0.3))
    state["before"] = rig.dump()
    rig.measure("tap, then know what changed (dump until two agree)",
                "as above, repeating the dump until two in a row are equal",
                tap_dump_diff(True), lambda: time.sleep(0.3))

    rig.adb("shell", "am force-stop com.android.settings")
    rig.adb("shell", "am start -a android.settings.SETTINGS")
    time.sleep(2.5)
    rig.measure("UI tree, Settings main (exec-out)", "`exec-out uiautomator dump /dev/tty`, raw XML", rig.dump)

    def dump_pull():
        rig.adb("shell", "uiautomator dump /sdcard/pu.xml")
        rig.adb("pull", "/sdcard/pu.xml", "pu.xml")

    rig.measure("UI tree, Settings main (dump + pull)", "`shell uiautomator dump` then `adb pull`", dump_pull)
    rig.tab("list")
    rig.measure("UI tree, List tab (exec-out)", "same, 1,000-row RecyclerView (bound rows only)", rig.dump)

    rig.tab("text")
    fx, fy = center(rig.dump(), lambda n: n["class"].endswith("EditText"))
    rig.adb("shell", f"input tap {fx} {fy}")
    time.sleep(1.0)
    field = lambda: [n.attrib["text"] for n in ET.fromstring(rig.dump()).iter("node") if n.attrib["class"].endswith("EditText")]
    arg = "'" + ASCII_100.replace(" ", "%s") + "'"
    rig.adb("shell", f"input text {arg}")
    time.sleep(0.5)
    shown = field()
    assert any(ASCII_100 in t for t in shown), f"input text did not reach the field: {shown!r}"
    rig.adb("shell", CLEAR)
    rig.measure("type 100 ASCII chars", "`adb shell input text`, one process", lambda: rig.adb("shell", f"input text {arg}"),
                lambda: rig.adb("shell", CLEAR))

    rig.adb("shell", CLEAR)
    uni = "'" + MIXED.replace(" ", "%s") + "'"
    out = subprocess.run(["adb", "-s", rig.serial, "shell", f"input text {uni}"], capture_output=True)
    time.sleep(0.5)
    print(f"unicode input text: exit {out.returncode}, stderr {out.stderr.decode(errors='replace')[:200]!r}, field after {field()!r}",
          file=sys.stderr)
    rig.adb("shell", CLEAR)

    print("# plain adb latency\n")
    ver = rig.adb("shell", "getprop ro.build.version.sdk").strip()
    print(f"device {rig.serial} - API {ver} - {a.runs} runs per op after {a.warmup} warm-ups, one adb process per call.\n")
    print("| op | runs | p50 ms | p95 ms | max ms | what limits it |")
    print("|---|---|---|---|---|---|")
    for name, note, ms in rig.rows:
        print(f"| {name} | {len(ms)} | {pct(ms, .5):.1f} | {pct(ms, .95):.1f} | {pct(ms, 1):.1f} | {note} |")
    if a.csv:
        with open(a.csv, "w") as f:
            f.write("op,ms\n")
            for name, _, ms in rig.rows:
                for v in ms:
                    f.write(f'"{name}",{v:.3f}\n')


main()
