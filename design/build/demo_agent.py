"""Records a real Claude Code run against the iOS Simulator. Run it on a machine with claude installed.
The Simulator and the phone-use daemon are on another machine (ssh host); the daemon socket is forwarded
and claude starts `phone-use mcp` with PHONE_USE_SOCKET set. The Simulator video is recorded on the host.
Usage: python3 demo_agent.py <ssh host> <udid> <local phone-use> <host phone-use> <out dir> <prompt> [timeout s]"""
import json
import subprocess
import sys
import time
from pathlib import Path

HOST, UDID, LOCAL_BIN, HOST_BIN, OUT, PROMPT = sys.argv[1:7]
TIMEOUT = float(sys.argv[7]) if len(sys.argv) > 7 else 150
OUT = Path(OUT)
OUT.mkdir(parents=True, exist_ok=True)
SOCK = "/tmp/pu-demo.sock"
REMOTE = f"/Users/{subprocess.run(['ssh', HOST, 'whoami'], capture_output=True, text=True).stdout.strip()}/.phone-use/run/daemon.sock"
VIDEO = "/tmp/demo-agent.mov"


def ssh(cmd):
    return subprocess.run(["ssh", HOST, cmd], capture_output=True, text=True)


Path(SOCK).unlink(missing_ok=True)
fwd = subprocess.Popen(["ssh", "-N", "-o", "ExitOnForwardFailure=yes", "-L", f"{SOCK}:{REMOTE}", HOST])
time.sleep(1.5)
cfg = OUT / "mcp.json"
cfg.write_text(json.dumps({"mcpServers": {"phone-use": {"command": LOCAL_BIN, "args": ["mcp"], "env": {"PHONE_USE_SOCKET": SOCK}}}}))

ssh(f"xcrun simctl ui {UDID} appearance light; xcrun simctl terminate {UDID} com.apple.Preferences; {HOST_BIN} press home; {HOST_BIN} release")
for row in ssh(f"{HOST_BIN} devices").stdout.splitlines()[1:]:
    lease = row.split()[-1]
    if lease != "-":
        ssh(f"{HOST_BIN} --session {lease} release")
time.sleep(1.5)
rec = subprocess.Popen(["ssh", HOST, f"xcrun simctl io {UDID} recordVideo --codec=h264 --force {VIDEO}"], stderr=subprocess.DEVNULL, stdout=subprocess.DEVNULL)
t0 = time.time()
time.sleep(1.5)
events = []
start = time.time() - t0
proc = subprocess.Popen(
    ["claude", "-p", PROMPT, "--mcp-config", str(cfg), "--strict-mcp-config", "--allowedTools", "mcp__phone-use__*", "--tools", "ToolSearch", "--append-system-prompt",
     "You operate a phone with the phone-use MCP tools. Use only those tools.",
     "--output-format", "stream-json", "--verbose", "--no-session-persistence"],
    stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, cwd=str(OUT))
deadline = time.time() + TIMEOUT
import select
while proc.poll() is None and time.time() < deadline:
    r, _, _ = select.select([proc.stdout], [], [], 0.2)
    if r:
        line = proc.stdout.readline()
        if line.strip():
            events.append({"t": time.time() - t0, "event": json.loads(line)})
for line in proc.stdout:
    if line.strip():
        events.append({"t": time.time() - t0, "event": json.loads(line)})
timed_out = proc.poll() is None
if timed_out:
    proc.kill()
end = time.time() - t0
time.sleep(2.0)
ssh("pkill -INT -f 'simctl io.*recordVideo'")
rec.wait()
subprocess.run(["scp", "-q", f"{HOST}:{VIDEO}", str(OUT / "screen.mov")], check=True)
ssh(f"xcrun simctl ui {UDID} appearance light")
fwd.terminate()
(OUT / "agent.json").write_text(json.dumps({"prompt": PROMPT, "start": start, "end": end, "timed_out": timed_out, "events": events}, indent=1))
print("events", len(events), "end", round(end, 1), "timed_out", timed_out)
