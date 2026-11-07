"""Records the README demo: Claude Code's real interactive terminal UI (captured headlessly with VHS) driving an
Android emulator through the phonedriver MCP server, and the emulator's own screen (adb screenrecord, in segments of
170 s because screenrecord stops at 180 s). The emulator is on another machine. phonedriver runs there and the MCP
server is its stdio over ssh, so no adb round trip crosses the network; adb (screenrecord and the pulls) uses an ssh
forward given as ADB_SERVER_SOCKET. Wall-clock times of both recordings go to times.json.
Claude Code must run on the subscription: ANTHROPIC_API_KEY has to be unset.
Claude Code asks once whether to trust the working folder (<out dir>/../cwd): run claude there by hand once first.
Usage: python3 demo_record.py <adb socket, tcp:127.0.0.1:5038> <serial> <ssh host> <phonedriver on that host> <out dir> <prompt file> [run s]"""
import json
import os
import subprocess
import sys
import threading
import time
from pathlib import Path

SOCKET, SERIAL, HOST, BIN, OUT, PROMPT_FILE = sys.argv[1:7]
WAIT = int(sys.argv[7]) if len(sys.argv) > 7 else 600
OUT = Path(OUT).resolve()
OUT.mkdir(parents=True, exist_ok=True)
assert "ANTHROPIC_API_KEY" not in os.environ, "ANTHROPIC_API_KEY is set; Claude Code would bill the API"
work = OUT.parent / "cwd"
work.mkdir(exist_ok=True)
env = dict(os.environ, ADB_SERVER_SOCKET=SOCKET)


def adb(*args, **kw):
    return subprocess.run(["adb", "-s", SERIAL, *args], env=env, capture_output=True, text=True, **kw)


def pu(*args):
    return subprocess.run(["ssh", "-T", HOST, f"{BIN} --device {SERIAL} " + " ".join(f"'{a}'" for a in args)], capture_output=True, text=True).stdout


pu("apps", "clear", "Chrome")
pu("open", "https://example.com")
time.sleep(2)
pu("tap", "Stay signed out")
time.sleep(2)
pu("tap", "No thanks")
time.sleep(1)
pu("press", "home")
pu("release")
time.sleep(1.5)

(OUT / "mcp.json").write_text(json.dumps({"mcpServers": {"phonedriver": {"command": "ssh", "args": ["-T", HOST, f"{BIN} --device {SERIAL} mcp"]}}}))
tape = (Path(__file__).parent / "demo.tape").read_text()
for k, v in {"OUT": str(OUT), "CWD": str(work), "PROMPT": Path(PROMPT_FILE).read_text().strip(), "WAIT": str(WAIT)}.items():
    tape = tape.replace("{{" + k + "}}", v)
(OUT / "run.tape").write_text(tape)

segments = []
stop = threading.Event()


def record():
    i = 0
    while not stop.is_set():
        path = f"/sdcard/pu-demo-{i}.mp4"
        segments.append({"device": path, "start": time.time()})
        adb("shell", "screenrecord", "--size", "540x1212", "--bit-rate", "4000000", "--time-limit", "170", path)
        i += 1


rec = threading.Thread(target=record)
rec.start()
time.sleep(2)
clean = {k: v for k, v in os.environ.items() if not k.startswith(("CLAUDE_CODE", "CLAUDECODE"))}
subprocess.run(["vhs", str(OUT / "run.tape")], cwd=str(work), check=True, env=clean)
mark = float((OUT / "term-start.txt").read_text())
time.sleep(1.0)
stop.set()
while rec.is_alive():
    adb("shell", "pkill", "-INT", "screenrecord")
    rec.join(3)
time.sleep(1.5)
for i, s in enumerate(segments):
    s["file"] = f"screen-{i}.mp4"
    subprocess.run(["adb", "-s", SERIAL, "pull", s["device"], str(OUT / s["file"])], env=env, check=True, capture_output=True)
    adb("shell", "rm", "-f", s["device"])
(OUT / "times.json").write_text(json.dumps({"term_start": mark, "segments": segments}))
pu("apps", "clear", "Chrome")
pu("press", "home")
pu("release")
