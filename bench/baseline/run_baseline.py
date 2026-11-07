"""Latency baseline for phone-harness 0.3.3 on the same emulator and operations as phonedriver-bench.

Run inside the isolated venv with telemetry off:
  DO_NOT_TRACK=1 PHONE_HARNESS_TELEMETRY=0 PHONE_HARNESS_NO_UPDATE_CHECK=1 PHONE_HARNESS_PLATFORM=android \
  ANDROID_SERIAL=emulator-5554 python run_baseline.py --runs 50 --warmup 5 [--csv out.csv]
"""
import argparse
import math
import os
import subprocess
import sys
import time

PACKAGE = "dev.phonedriver.testbed"
MIXED = "héllo 世界 مرحبا 👋🏽 abc 123 "
ASCII_100 = ("The quick brown fox jumps over the lazy dog 0123456789. " * 3)[:100]


def adb(*args):
    r = subprocess.run(["adb", "-s", os.environ["ANDROID_SERIAL"], *args], capture_output=True)
    return r.stdout.decode(errors="replace")


def tab(name):
    adb("shell", f"am start -n {PACKAGE}/.MainActivity --es tab {name}")
    time.sleep(1.5)


def pct(values, p):
    v = sorted(values)
    return v[max(0, min(len(v) - 1, math.ceil(p * len(v)) - 1))]


def measure(rows, name, note, op, runs, warmup, between=None):
    ms = []
    for i in range(warmup + runs):
        t = time.perf_counter()
        op()
        took = (time.perf_counter() - t) * 1000
        if i >= warmup:
            ms.append(took)
        if between:
            between()
    print(f"{name}: {len(ms)} runs done", file=sys.stderr)
    rows.append((name, note, ms))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=50)
    ap.add_argument("--warmup", type=int, default=5)
    ap.add_argument("--csv")
    ap.add_argument("--unicode-check", action="store_true")
    a = ap.parse_args()

    from phone_harness import helpers as h
    from phone_harness import telemetry

    print(f"telemetry.is_enabled() = {telemetry.is_enabled()}", file=sys.stderr)
    density = int(adb("shell", "wm density").split(":")[-1].split()[0]) / 160

    rows = []
    tab("taps")
    h.screen()
    x, y = int(120 * density), int(300 * density)
    measure(rows, "tap", "`input tap` per call (adb CLI process), gate cached 2 s", lambda: h.tap(x, y), a.runs, a.warmup)
    measure(rows, "screenshot (settled Taps tab)", "`exec-out screencap -p` to a temp file", lambda: h.screenshot(), a.runs, a.warmup)

    adb("shell", "am force-stop com.android.settings")
    adb("shell", "am start -a android.settings.SETTINGS")
    time.sleep(2.5)
    measure(rows, "screen() (Settings main)", "`uiautomator dump` parsed to text boxes", h.screen, a.runs, a.warmup)
    tab("list")
    measure(rows, "screen() (List tab, 1000 rows)", "`uiautomator dump` parsed to text boxes", h.screen, a.runs, a.warmup)

    tab("text")
    field = [n for n in h.screen() if n["text"] == "plain" or "plain" in n["text"]]
    nodes = h.ui()
    edit = [n for n in nodes if n["class"].endswith("EditText")][0]
    h.tap(edit["x"], edit["y"])
    time.sleep(1.0)
    h.type_text("probe")
    time.sleep(0.5)
    shown = [n["text"] for n in h.ui() if n["class"].endswith("EditText")]
    assert any("probe" in t for t in shown), f"type_text did not reach the field: {shown!r}"
    adb("shell", "input keycombination 113 29 && input keyevent 67")

    def clear():
        adb("shell", "input keycombination 113 29 && input keyevent 67")

    measure(rows, "type_text 100 ASCII chars", "`input text` through adb CLI, one process per chunk",
            lambda: h.type_text(ASCII_100), a.runs, a.warmup, clear)

    if a.unicode_check:
        clear()
        out = "no exception"
        try:
            h.type_text(MIXED)
        except Exception as e:
            out = f"{type(e).__name__}: {e}"
        time.sleep(0.5)
        typed = [n["text"] for n in h.ui() if n["class"].endswith("EditText")]
        print(f"unicode type_text result: {out}", file=sys.stderr)
        print(f"unicode field contents after: {typed!r}", file=sys.stderr)
        clear()

    print("# phone-harness 0.3.3 latency\n")
    print(f"device {os.environ['ANDROID_SERIAL']} - adb server {os.environ.get('ADB_SERVER_SOCKET', '127.0.0.1:5037')}\n")
    print(f"{a.runs} runs per op after {a.warmup} warm-ups, in-process helper calls.\n")
    print("| op | runs | p50 ms | p95 ms | max ms | what limits it |")
    print("|---|---|---|---|---|---|")
    for name, note, ms in rows:
        print(f"| {name} | {len(ms)} | {pct(ms, .5):.1f} | {pct(ms, .95):.1f} | {pct(ms, 1):.1f} | {note} |")
    if a.csv:
        with open(a.csv, "w") as f:
            f.write("op,ms\n")
            for name, _, ms in rows:
                for v in ms:
                    f.write(f'"{name}",{v:.3f}\n')


main()
