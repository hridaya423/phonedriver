"""Records the README demo: Claude Code's real interactive terminal UI (captured headlessly with VHS) driving
the iOS Simulator through the phone-use MCP server, and the Simulator's own screen video.
The Simulator and the phone-use daemon are on another machine (ssh host); the daemon socket is forwarded and
claude starts `phone-use mcp` with PHONE_USE_SOCKET set. Wall-clock times of both recordings go to times.json.
Claude Code must run on the subscription: ANTHROPIC_API_KEY has to be unset.
Usage: python3 demo_record.py <ssh host> <udid> <local phone-use> <host phone-use> <out dir> <prompt> [wait s]"""
import json
import os
import subprocess
import sys
import time
from pathlib import Path

HOST, UDID, LOCAL_BIN, HOST_BIN, OUT, PROMPT = sys.argv[1:7]
WAIT = int(sys.argv[7]) if len(sys.argv) > 7 else 70
OUT = Path(OUT).resolve()
OUT.mkdir(parents=True, exist_ok=True)
assert "ANTHROPIC_API_KEY" not in os.environ, "ANTHROPIC_API_KEY is set; Claude Code would bill the API"
SOCK = "/tmp/pu-demo.sock"
VIDEO = "/tmp/demo-real.mov"
work = OUT / "cwd"
work.mkdir(exist_ok=True)


def ssh(cmd):
    return subprocess.run(["ssh", HOST, cmd], capture_output=True, text=True)


user = ssh("whoami").stdout.strip()
Path(SOCK).unlink(missing_ok=True)
fwd = subprocess.Popen(["ssh", "-N", "-o", "ExitOnForwardFailure=yes", "-L", f"{SOCK}:/Users/{user}/pu-take/run/daemon.sock", HOST])
time.sleep(1.5)
(OUT / "mcp.json").write_text(json.dumps({"mcpServers": {"phone-use": {"command": LOCAL_BIN, "args": ["mcp"], "env": {"PHONE_USE_SOCKET": SOCK}}}}))
tape = (Path(__file__).parent / "demo.tape").read_text()
for k, v in {"OUT": str(OUT), "PROMPT": PROMPT, "WAIT": str(WAIT)}.items():
    tape = tape.replace("{{" + k + "}}", v)
(OUT / "run.tape").write_text(tape)

ssh(f"xcrun simctl terminate {UDID} com.apple.mobilesafari; rm -f ~/Library/Developer/CoreSimulator/Devices/{UDID}/data/Library/Safari/SafariTabs.db* ~/Library/Developer/CoreSimulator/Devices/{UDID}/data/Library/Safari/BrowserState.db*; xcrun simctl ui {UDID} appearance light; {HOST_BIN} press home; {HOST_BIN} release")
for row in ssh(f"{HOST_BIN} devices").stdout.splitlines()[1:]:
    dev, lease = row.split()[0], row.split()[-1]
    if lease != "-":
        ssh(f"{HOST_BIN} --device {dev} --session {lease} release")
time.sleep(1.5)
rec = subprocess.Popen(["ssh", HOST, f"xcrun simctl io {UDID} recordVideo --codec=h264 --force {VIDEO}"], stderr=subprocess.PIPE, stdout=subprocess.DEVNULL, text=True)
sim_t0 = None
for line in rec.stderr:
    if "Recording started" in line:
        sim_t0 = time.time()
        break
assert sim_t0, "simulator recording did not start"
clean = {k: v for k, v in os.environ.items() if not k.startswith(("CLAUDE_CODE", "CLAUDECODE"))}
subprocess.run(["vhs", str(OUT / "run.tape")], cwd=str(work), check=True, env=clean)
mark = float((OUT / "term-start.txt").read_text())
time.sleep(1.0)
ssh("pkill -INT -f 'simctl io.*recordVideo'")
rec.wait()
subprocess.run(["scp", "-q", f"{HOST}:{VIDEO}", str(OUT / "screen.mov")], check=True)
ssh(f"rm -f {VIDEO}; xcrun simctl ui {UDID} appearance light")
fwd.terminate()
(OUT / "times.json").write_text(json.dumps({"prompt": PROMPT, "sim_start": sim_t0, "term_start": mark}))
print("sim_start", sim_t0, "term_start", mark, "term minus sim", round(mark - sim_t0, 2))
