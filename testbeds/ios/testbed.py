#!/usr/bin/env python3
"""Build, install and read the iOS testbed on a simulator (run on the Mac that hosts it).

    python3 testbed.py install <udid>
    python3 testbed.py launch <udid> [tab]
    python3 testbed.py clear <udid>
    python3 testbed.py log <udid>
    python3 testbed.py shot <udid> <tab> <out.png>
"""

import json
import os
import subprocess
import sys
import time

BUNDLE_ID = "dev.phoneuse.testbed"
HERE = os.path.dirname(os.path.abspath(__file__))
BUILD = os.path.join(HERE, "build")


def run(*args, cwd=None):
    return subprocess.run(args, cwd=cwd, check=True, capture_output=True, text=True).stdout


def install(udid):
    run("xcodegen", "generate", "--quiet", cwd=HERE)
    run(
        "xcodebuild", "-project", "Testbed.xcodeproj", "-scheme", "Testbed",
        "-destination", f"id={udid}", "-derivedDataPath", BUILD, "-quiet", "build",
        cwd=HERE,
    )
    app = os.path.join(BUILD, "Build", "Products", "Debug-iphonesimulator", "Testbed.app")
    run("xcrun", "simctl", "install", udid, app)


def launch(udid, tab=None):
    if running(udid):
        run("xcrun", "simctl", "terminate", udid, BUNDLE_ID)
    run("xcrun", "simctl", "launch", udid, BUNDLE_ID, *(["-tab", tab] if tab else []))


def running(udid):
    out = subprocess.run(
        ["xcrun", "simctl", "spawn", udid, "launchctl", "list"], capture_output=True, text=True
    ).stdout
    return BUNDLE_ID in out


def log_path(udid):
    data = run("xcrun", "simctl", "get_app_container", udid, BUNDLE_ID, "data").strip()
    return os.path.join(data, "Documents", "putb.log")


def clear_log(udid):
    path = log_path(udid)
    if os.path.exists(path):
        os.truncate(path, 0)


def read_log(udid):
    path = log_path(udid)
    if not os.path.exists(path):
        return []
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def wait_for(udid, predicate, timeout=5.0):
    deadline = time.monotonic() + timeout
    while True:
        events = read_log(udid)
        if any(predicate(e) for e in events):
            return events
        if time.monotonic() > deadline:
            raise AssertionError(f"no matching event within {timeout}s; log tail: {events[-10:]}")
        time.sleep(0.05)


def screenshot(udid, out):
    run("xcrun", "simctl", "io", udid, "screenshot", out)


def main(argv):
    cmd, udid, *rest = argv
    if cmd == "install":
        install(udid)
    elif cmd == "launch":
        launch(udid, *rest)
    elif cmd == "clear":
        clear_log(udid)
    elif cmd == "log":
        for e in read_log(udid):
            print(json.dumps(e, sort_keys=True))
    elif cmd == "shot":
        tab, out = rest
        launch(udid, tab)
        time.sleep(2)
        screenshot(udid, out)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
