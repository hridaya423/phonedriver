#!/usr/bin/env python3
"""Live tests for PhoneUseRunner against the iOS testbed on a booted simulator.

Run on the Mac that hosts the simulator:

    PHONE_USE_TEST_UDID=<udid> python3 ios-runner/tests/test_runner.py -v

Builds the runner if needed, starts it with `xcodebuild test-without-building` unless one
already answers `hello` on PHONE_USE_PORT (default 27753), and checks every op against the
testbed's event log, not against the runner's own replies.
"""

import base64
import glob
import json
import os
import socket
import struct
import subprocess
import sys
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
RUNNER_DIR = os.path.dirname(HERE)
REPO = os.path.dirname(RUNNER_DIR)
sys.path.insert(0, os.path.join(REPO, "testbeds", "ios"))
import testbed  # noqa: E402

PORT = int(os.environ.get("PHONE_USE_PORT", "27753"))
TESTBED = testbed.BUNDLE_ID


def booted_udid():
    if "PHONE_USE_TEST_UDID" in os.environ:
        return os.environ["PHONE_USE_TEST_UDID"]
    devices = json.loads(subprocess.run(
        ["xcrun", "simctl", "list", "devices", "booted", "-j"],
        check=True, capture_output=True, text=True).stdout)["devices"]
    return next(d["udid"] for ds in devices.values() for d in ds)


class Runner:
    def __init__(self, port):
        self.sock = socket.create_connection(("127.0.0.1", port), timeout=60)
        self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.next_id = 1

    def _read(self, n):
        buf = b""
        while len(buf) < n:
            chunk = self.sock.recv(n - len(buf))
            if not chunk:
                raise ConnectionError("runner closed the connection")
            buf += chunk
        return buf

    def raw(self, request):
        body = json.dumps(request).encode()
        self.sock.sendall(struct.pack(">I", len(body)) + body)
        (length,) = struct.unpack(">I", self._read(4))
        return json.loads(self._read(length))

    def call(self, op, **fields):
        rid = self.next_id
        self.next_id += 1
        reply = self.raw({"id": rid, "op": op, **fields})
        assert reply.get("id") == rid, reply
        return reply

    def ok(self, op, **fields):
        reply = self.call(op, **fields)
        assert reply.get("ok") is True, reply
        return reply

    def close(self):
        self.sock.close()


def try_hello():
    try:
        r = Runner(PORT)
    except OSError:
        return False
    try:
        r.sock.settimeout(2)
        r.ok("hello", version="0.1.0")
        return True
    except (OSError, AssertionError, ConnectionError):
        return False
    finally:
        r.close()


def start_runner(udid):
    build = os.path.join(RUNNER_DIR, "build")
    subprocess.run(["xcodegen", "generate", "--quiet"], cwd=RUNNER_DIR, check=True)
    subprocess.run(
        ["xcodebuild", "-project", "PhoneUseRunner.xcodeproj", "-scheme", "PhoneUseRunner",
         "-destination", f"id={udid}", "-derivedDataPath", build, "-quiet", "build-for-testing"],
        cwd=RUNNER_DIR, check=True)
    xctestrun = glob.glob(os.path.join(build, "Build", "Products", "*.xctestrun"))[0]
    env = dict(os.environ, TEST_RUNNER_PHONE_USE_PORT=str(PORT))
    log = open(RUNNER_LOG, "w")
    child = subprocess.Popen(
        ["xcodebuild", "test-without-building", "-xctestrun", xctestrun,
         "-destination", f"id={udid}", "-test-timeouts-enabled", "NO"],
        env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    deadline = time.monotonic() + 120
    while not try_hello():
        if child.poll() is not None or time.monotonic() > deadline:
            raise RuntimeError(f"runner did not answer hello; see {log.name}")
        time.sleep(0.1)
    return child


UDID = None
CHILD = None
RUNNER_LOG = os.path.join(RUNNER_DIR, "build", "runner.log")


def setUpModule():
    global UDID, CHILD
    UDID = booted_udid()
    if not try_hello():
        CHILD = start_runner(UDID)


def tearDownModule():
    if CHILD is not None:
        os.killpg(CHILD.pid, 15)


def find(nodes, ident):
    matches = [n for n in nodes if n.get("id") == ident]
    assert matches, f"no node with id {ident!r}"
    return matches[0]


def center(node):
    l, t, r, b = node["b"]
    return (l + r) / 2, (t + b) / 2


def tap_path(x, y, ms=60):
    return [{"path": [{"x": x, "y": y, "t_ms": 0}, {"x": x, "y": y, "t_ms": ms}]}]


def wait_log(condition, timeout=5.0):
    deadline = time.monotonic() + timeout
    while True:
        events = testbed.read_log(UDID)
        if condition(events):
            return events
        if time.monotonic() > deadline:
            raise AssertionError(f"log condition not met within {timeout}s; tail: {events[-10:]}")
        time.sleep(0.05)


def touches(events):
    return [e for e in events if e["ev"] == "touch"]


class Base(unittest.TestCase):
    tab = "taps"

    def setUp(self):
        testbed.launch(UDID, self.tab)
        time.sleep(1.0)
        testbed.clear_log(UDID)
        self.r = Runner(PORT)

    def tearDown(self):
        self.r.close()

    def snapshot(self):
        return self.r.ok("snapshot", max_depth=30)


class RunnerTests(Base):
    def test_hello_reports_screen_in_points(self):
        reply = self.r.ok("hello", version="0.1.0")
        shot = self.r.ok("screenshot", format="png", quality=100)
        screen = reply["screen"]
        self.assertEqual(round(screen["w"] * screen["scale"]), shot["w"])
        self.assertEqual(round(screen["h"] * screen["scale"]), shot["h"])
        self.assertTrue(reply["ios"])
        self.assertTrue(reply["model"])
        self.assertEqual(reply["version"], "0.1.0")

    def test_snapshot_has_testbed_identifiers_and_visible_flags(self):
        reply = self.snapshot()
        self.assertEqual(reply["app"]["bundle_id"], TESTBED)
        self.assertGreater(reply["app"]["pid"], 0)
        nodes = reply["nodes"]
        self.assertEqual(nodes[0]["p"], -1)
        for i, n in enumerate(nodes[1:], start=1):
            self.assertLess(n["p"], i)
        b17 = find(nodes, "taps.b17")
        self.assertEqual(b17["cls"], "XCUIElementTypeButton")
        self.assertEqual(b17["text"], "B17")
        self.assertIn("V", b17["f"])
        self.assertIn("v", b17["f"])
        self.assertIn("e", b17["f"])
        self.assertIn("tap", b17["a"])

    def test_gesture_tap_lands_on_the_element_center(self):
        x, y = center(find(self.snapshot()["nodes"], "taps.b17"))
        self.r.ok("gesture", pointers=tap_path(x, y))
        events = testbed.wait_for(UDID, lambda e: e["ev"] == "tap")
        self.assertEqual([e["id"] for e in events if e["ev"] == "tap"], ["b17"])
        t = touches(events)
        self.assertEqual([e["phase"] for e in t][0], "began")
        self.assertEqual([e["phase"] for e in t][-1], "ended")
        for e in t:
            self.assertAlmostEqual(e["x"], x, delta=0.5)
            self.assertAlmostEqual(e["y"], y, delta=0.5)

    def test_unknown_op_is_an_error_not_a_crash(self):
        reply = self.r.call("fly")
        self.assertIs(reply["ok"], False)
        self.assertEqual(reply["error"]["code"], "bad_request")
        self.r.ok("hello", version="0.1.0")

    def test_screenshot_jpeg(self):
        reply = self.r.ok("screenshot", format="jpeg", quality=50)
        data = base64.b64decode(reply["data_b64"])
        self.assertEqual(data[:2], b"\xff\xd8")
        self.assertGreater(reply["w"], 0)
        self.assertGreater(reply["h"], 0)

    def test_button_home_then_activate(self):
        self.r.ok("button", name="home")
        testbed.wait_for(UDID, lambda e: e.get("state") == "background")
        testbed.clear_log(UDID)
        self.r.ok("activate", bundle_id=TESTBED)
        testbed.wait_for(UDID, lambda e: e.get("state") == "active")
        self.assertEqual(self.snapshot()["app"]["bundle_id"], TESTBED)

    def test_volume_is_unsupported_on_the_simulator(self):
        reply = self.r.call("button", name="volume_up")
        self.assertIs(reply["ok"], False)
        self.assertEqual(reply["error"]["code"], "unsupported")

    def test_lock_backgrounds_the_app(self):
        self.r.ok("button", name="lock")
        testbed.wait_for(UDID, lambda e: e.get("state") == "background")
        self.r.ok("button", name="home")
        self.r.ok("gesture", pointers=[{"path": [
            {"x": 200, "y": 860, "t_ms": 0}, {"x": 200, "y": 300, "t_ms": 250}]}])
        self.r.ok("activate", bundle_id=TESTBED)
        testbed.wait_for(UDID, lambda e: e.get("state") == "active")

    def test_open_url_reaches_the_testbed(self):
        self.r.ok("open_url", url="putb://tab/nav")
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if any(e["ev"] == "url" for e in testbed.read_log(UDID)):
                break
            nodes = self.snapshot()["nodes"]
            opens = [n for n in nodes if n.get("text") == "Open" and n["cls"] == "XCUIElementTypeButton"]
            if opens:
                self.r.ok("gesture", pointers=tap_path(*center(opens[0])))
            time.sleep(0.2)
        events = testbed.wait_for(UDID, lambda e: e["ev"] == "tab" and e["name"] == "nav")
        self.assertIn("putb://tab/nav", [e.get("url") for e in events])


class TextTests(Base):
    tab = "text"

    def test_type_unicode_into_the_focused_field(self):
        field = find(self.snapshot()["nodes"], "text.plain")
        self.assertIn("E", field["f"])
        self.r.ok("gesture", pointers=tap_path(*center(field)))
        time.sleep(0.5)
        text = "héllo wörld 👋 日本"
        self.r.ok("type", text=text)
        events = testbed.wait_for(UDID, lambda e: e.get("field") == "plain" and e.get("value") == text)
        self.assertEqual([e for e in events if e.get("field") == "plain"][-1]["value"], text)
        field = find(self.snapshot()["nodes"], "text.plain")
        self.assertEqual(field["sd"], text)
        self.assertIn("F", field["f"])


    def test_typed_text_stays_out_of_xcodebuild_output(self):
        if CHILD is None:
            self.skipTest("runner was not started by this test run, so its output is not visible")
        field = find(self.snapshot()["nodes"], "text.plain")
        self.r.ok("gesture", pointers=tap_path(*center(field)))
        time.sleep(0.5)
        canary = "canary-" + str(time.time_ns())
        self.r.ok("type", text=canary)
        testbed.wait_for(UDID, lambda e: e.get("value") == canary)
        with open(RUNNER_LOG, errors="replace") as f:
            self.assertNotIn(canary, f.read())


class GestureTests(Base):
    tab = "gestures"

    def test_two_finger_pinch_paths_and_timing(self):
        pad = find(self.snapshot()["nodes"], "gestures.pad")
        cx, cy = center(pad)
        pointers = [
            {"path": [{"x": cx - 30, "y": cy, "t_ms": 0}, {"x": cx - 120, "y": cy, "t_ms": 400}]},
            {"path": [{"x": cx + 30, "y": cy, "t_ms": 0}, {"x": cx + 120, "y": cy, "t_ms": 400}]},
        ]
        self.r.ok("gesture", pointers=pointers)
        events = wait_log(lambda log: sum(e["phase"] == "ended" for e in touches(log)) >= 2)
        t = touches(events)
        by_finger = {}
        for e in t:
            by_finger.setdefault(e["finger"], []).append(e)
        self.assertEqual(len(by_finger), 2, t)
        starts = sorted(round(f[0]["x"]) for f in by_finger.values())
        ends = sorted(round(f[-1]["x"]) for f in by_finger.values())
        self.assertEqual(starts, [round(cx - 30), round(cx + 30)])
        self.assertEqual(ends, [round(cx - 120), round(cx + 120)])
        for f in by_finger.values():
            self.assertEqual(f[0]["phase"], "began")
            self.assertEqual(f[-1]["phase"], "ended")
            self.assertTrue(any(e["phase"] == "moved" for e in f))
            duration = f[-1]["uptime"] - f[0]["uptime"]
            self.assertAlmostEqual(duration, 0.4, delta=0.1)


class ListTests(Base):
    tab = "list"

    def test_drag_scrolls_the_list(self):
        table = find(self.snapshot()["nodes"], "list.table")
        x, y = center(table)
        self.r.ok("gesture", pointers=[{"path": [
            {"x": x, "y": y + 200, "t_ms": 0}, {"x": x, "y": y - 200, "t_ms": 300},
            {"x": x, "y": y - 200, "t_ms": 400}]}])
        events = testbed.wait_for(UDID, lambda e: e["ev"] == "scroll" and e["first"] > 0)
        nodes = self.snapshot()["nodes"]
        first = [e for e in events if e["ev"] == "scroll"][-1]["first"]
        self.assertTrue(any(n.get("id") == f"list.row.{first + 1}" for n in nodes))


if __name__ == "__main__":
    unittest.main()
