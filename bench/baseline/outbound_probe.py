"""Runs phone-harness helper calls for 12 s and prints every network event Python itself raised.

`sys.addaudithook` sees each socket connect and DNS lookup made in this process, and each child it starts,
so a telemetry send would show here even if it were too short to sample from outside.
"""
import collections
import sys
import time

events = collections.Counter()


def hook(name, args):
    if name == "socket.connect":
        events[f"socket.connect {args[1]}"] += 1
    elif name in ("socket.getaddrinfo", "socket.gethostbyname"):
        events[f"{name} {args[0]}"] += 1
    elif name == "subprocess.Popen":
        events[f"subprocess.Popen {args[1][0] if isinstance(args[1], (list, tuple)) else args[1]} {args[1][1:3] if isinstance(args[1], (list, tuple)) else ''}"] += 1


sys.addaudithook(hook)
from phone_harness import helpers as h
from phone_harness import telemetry, update

print(f"telemetry.is_enabled() = {telemetry.is_enabled()}")
end = time.time() + 12
calls = 0
while time.time() < end:
    h.screen()
    h.tap(300, 800)
    h.screenshot()
    calls += 3
print(f"{calls} helper calls (screen, tap, screenshot)")
net = {k: v for k, v in events.items() if not k.startswith("subprocess")}
print(f"network events raised in this process: {net if net else 'none'}")
for k, v in sorted(events.items()):
    if k.startswith("subprocess"):
        print(f"{v:4d} x {k}")
