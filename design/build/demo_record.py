"""Records one scene of the README demo: a real interactive Claude Code session in tmux driving an Android emulator
through the phonedriver MCP server, and the emulator's own screen (adb screenrecord, in segments of 170 s because
screenrecord stops at 180 s). tmux is polled every 40 ms and each change of the screen is logged with the wall clock
(term.jsonl), so the terminal needs no screen recording and cannot drift from the phone video. The phone clock is
fixed by pulling the notification shade open and shut before the clip starts (the frame it appears in is found in
demo_compose.py). phonedriver runs on the machine that has the emulator and the MCP server is its stdio over ssh.
Claude Code must run on the subscription: ANTHROPIC_API_KEY has to be unset.
Usage: python3 demo_record.py <scene 1|2|3> <out dir> <prompt file> [--no-phone]
The system prompt is <prompt file with .system>, when it exists.
Environment: PD_HOST (ssh host, win-ts), PD_BIN (phonedriver on it), PD_SERIAL (emulator-5554), PD_ADB (adb server
address for the Mac side, tcp:127.0.0.1:5038 -> an ssh forward to the host's adb server), PD_REMOTE_ADB (that
server as the host sees it, tcp:127.0.0.1:5039), PD_COLS, PD_ROWS."""
import json
import os
import re
import subprocess
import sys
import threading
import time
from pathlib import Path

SCENE, OUT, PROMPT_FILE = sys.argv[1], Path(sys.argv[2]).resolve(), sys.argv[3]
PHONE = "--no-phone" not in sys.argv
HOST = os.environ.get("PD_HOST", "win-ts")
BIN = os.environ.get("PD_BIN", "/d/phonedriver-demo/target/release/phonedriver.exe")
SERIAL = os.environ.get("PD_SERIAL", "emulator-5554")
ADB = os.environ.get("PD_ADB", "tcp:127.0.0.1:5038")
REMOTE_ADB = os.environ.get("PD_REMOTE_ADB", "tcp:127.0.0.1:5039")
COLS, ROWS = int(os.environ.get("PD_COLS", "56")), int(os.environ.get("PD_ROWS", "19"))
OUT.mkdir(parents=True, exist_ok=True)
assert "ANTHROPIC_API_KEY" not in os.environ, "ANTHROPIC_API_KEY is set; Claude Code would bill the API"
prompt = Path(PROMPT_FILE).read_text().strip()
SYSTEM = (Path(PROMPT_FILE).with_suffix(".system").read_text().strip() if Path(PROMPT_FILE).with_suffix(".system").exists()
          else "You operate a phone with the phonedriver MCP tools. Use only those tools.")
work = OUT.parent / "cwd"
work.mkdir(exist_ok=True)
SESSION = f"demo{SCENE}"
env = dict(os.environ, ADB_SERVER_SOCKET=ADB)


def adb(*args):
    return subprocess.run(["adb", "-s", SERIAL, *args], env=env, capture_output=True, text=True)


def pd(*args):
    cmd = f"ADB_SERVER_SOCKET={REMOTE_ADB} {BIN} --device {SERIAL} " + " ".join(f"'{a}'" for a in args)
    return subprocess.run(["ssh", "-T", HOST, cmd], capture_output=True, text=True).stdout


def tmux(*args):
    return subprocess.run(["tmux", *args], capture_output=True, text=True).stdout


def prepare_phone():
    if SCENE in ("1", "3"):
        pd("apps", "clear", "Chrome")
        pd("open", "https://example.com")
        time.sleep(2)
        pd("tap", "Stay signed out")
        time.sleep(2)
        pd("tap", "No thanks")
        time.sleep(1)
        pd("press", "home")
    else:
        subprocess.run(["ssh", "-T", HOST, f"adb -P 5039 -s {SERIAL} shell am start -n dev.phonedriver.testbed/.MainActivity --es tab checkout"], capture_output=True)
        time.sleep(2)
    pd("release")
    time.sleep(1.5)


def start_claude():
    tmux("kill-session", "-t", SESSION)
    mcp = {"mcpServers": {"phonedriver": {"command": "ssh", "args": ["-T", HOST, f"ADB_SERVER_SOCKET={REMOTE_ADB} PHONEDRIVER_DEVICE=android:{SERIAL} {BIN} --device {SERIAL} mcp"]}}}
    (OUT / "mcp.json").write_text(json.dumps(mcp))
    sh = OUT / "launch.sh"
    sh.write_text(
        "#!/bin/zsh\nunset ANTHROPIC_API_KEY TMUX TMUX_PANE TERM_PROGRAM TERM_PROGRAM_VERSION\nexport DISABLE_COST_WARNINGS=1 ENABLE_TOOL_SEARCH=false\n"
        f"cd {work}\nexec claude --model claude-opus-5-5 --mcp-config {OUT / 'mcp.json'} --strict-mcp-config "
        "--permission-mode default --settings '{\"spinnerTipsEnabled\":false}' --allowedTools 'mcp__phonedriver__*' --tools '' "
        f"--append-system-prompt '{SYSTEM}'\n")
    sh.chmod(0o755)
    clean = {k: v for k, v in os.environ.items() if not k.startswith(("CLAUDE_CODE", "CLAUDECODE", "TMUX"))}
    subprocess.run(["tmux", "new-session", "-d", "-s", SESSION, "-x", str(COLS), "-y", str(ROWS), str(sh)], env=clean, check=True)
    tmux("set-option", "-t", SESSION, "status", "off")


def screen():
    return tmux("capture-pane", "-p", "-e", "-t", SESSION)


def plain():
    return tmux("capture-pane", "-p", "-t", SESSION)


def wait_for(rx, timeout, source=plain):
    end = time.time() + timeout
    while time.time() < end:
        s = source()
        if re.search(rx, s):
            return s
        time.sleep(0.25)
    raise SystemExit(f"timed out waiting for {rx!r}; screen:\n{plain()}")


log = []
stop = threading.Event()


def capture():
    last = None
    while not stop.is_set():
        t0 = time.time()
        s = screen()
        t1 = time.time()
        if s != last:
            log.append({"t": (t0 + t1) / 2, "screen": s})
            last = s
        time.sleep(0.03)


segments = []
recording = threading.Event()


def record():
    i = 0
    while not stop.is_set():
        path = f"/sdcard/pd-demo-{SCENE}-{i}.mp4"
        segments.append({"device": path, "start": time.time()})
        recording.set()
        adb("shell", "screenrecord", "--size", "720x1616", "--bit-rate", "8000000", "--time-limit", "170", path)
        i += 1


prepare_phone()
start_claude()
try:
    wait_for(r"Yes, I trust", 4)
    tmux("send-keys", "-t", SESSION, "Down", "Enter")
except SystemExit:
    pass
wait_for(r"/effort", 30)
time.sleep(1.5)

cap = threading.Thread(target=capture)
cap.start()
rec = threading.Thread(target=record)
if PHONE:
    rec.start()
    recording.wait()
    time.sleep(1.5)
    adb("shell", "cmd", "statusbar", "expand-notifications")
    flash = time.time()
    time.sleep(1.2)
    adb("shell", "cmd", "statusbar", "collapse")
    time.sleep(1.5)
else:
    flash = None

tmux("set-buffer", "-b", "pdprompt", prompt)
paste = time.time()
tmux("paste-buffer", "-p", "-b", "pdprompt", "-t", SESSION)
time.sleep(0.6)
pasted = plain()
(OUT / "after-paste.txt").write_text(pasted)
HOLD = float(os.environ.get("PD_HOLD", "3"))
time.sleep(HOLD)
enter = time.time()
tmux("send-keys", "-t", SESSION, "Enter")

marks = {"paste": paste, "enter": enter, "flash": flash}
if SCENE in ("2", "3"):
    s = wait_for(r"requests your input", int(os.environ.get("PD_MAX", "900")))
    marks["prompt_shown"] = time.time()
    time.sleep(float(os.environ.get("PD_READ", "2.5")))
    tmux("send-keys", "-t", SESSION, "Down")
    marks["down"] = time.time()
    time.sleep(0.8)
    tmux("send-keys", "-t", SESSION, "Right")
    time.sleep(0.8)
    tmux("send-keys", "-t", SESSION, "Enter")
    marks["decline"] = time.time()
    time.sleep(1)
wait_for(r"for [\dhms ]+ · done", int(os.environ.get("PD_MAX", "900")))
marks["done"] = time.time()
time.sleep(float(os.environ.get("PD_TAIL", "3.5")))
marks["end"] = time.time()
final = plain()
stop.set()
cap.join()
(OUT / "final.txt").write_text(final)
with (OUT / "term.jsonl").open("w") as f:
    for e in log:
        f.write(json.dumps(e) + "\n")
if PHONE:
    while rec.is_alive():
        adb("shell", "pkill", "-INT", "screenrecord")
        rec.join(3)
    time.sleep(1.5)
    for i, s in enumerate(segments):
        s["file"] = f"screen-{i}.mp4"
        subprocess.run(["adb", "-s", SERIAL, "pull", s["device"], str(OUT / s["file"])], env=env, check=True, capture_output=True)
        adb("shell", "rm", "-f", s["device"])
(OUT / "times.json").write_text(json.dumps({"scene": SCENE, "cols": COLS, "rows": ROWS, "marks": marks, "segments": segments}, indent=1))
tmux("kill-session", "-t", SESSION)
if SCENE in ("1", "3"):
    pd("apps", "clear", "Chrome")
pd("press", "home")
pd("release")
print(final)
