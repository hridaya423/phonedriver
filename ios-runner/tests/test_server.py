#!/usr/bin/env python3
"""Who the runner serves (Runner/Server.m, Runner/Auth.m, decision D106), on a Mac with no Simulator.

Builds a small macOS program from the runner's own Server.m, Auth.m and Json.m with a handler that echoes the op,
then connects to it over loopback the way a host and an outsider would:

    python3 ios-runner/tests/test_server.py -v

On macOS the device branch of Server.m is compiled (TARGET_OS_SIMULATOR is 0), so the listener is the device's.
"""

import os
import secrets
import socket
import subprocess
import sys
import tempfile
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
RUNNER = os.path.join(os.path.dirname(HERE), "Runner")
sys.path.insert(0, HERE)
from runner_client import Runner, proof  # noqa: E402

TOKEN = secrets.token_hex(32)


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def build(out):
    sources = [os.path.join(RUNNER, f) for f in ("Server.m", "Auth.m", "Json.m")]
    subprocess.run(
        ["xcrun", "clang", "-fobjc-arc", "-Wall", "-Werror", "-I", RUNNER, *sources,
         os.path.join(HERE, "server_harness.m"), "-framework", "Foundation", "-framework", "Network",
         "-framework", "CoreGraphics", "-o", out],
        check=True)


class Server:
    def __init__(self, binary, token):
        self.port = free_port()
        self.child = subprocess.Popen([binary, str(self.port), token], stdout=subprocess.PIPE, text=True)
        assert self.child.stdout.readline().strip() == "ready"

    def stop(self):
        self.child.terminate()
        self.child.wait()


class ServerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = tempfile.TemporaryDirectory()
        cls.binary = os.path.join(cls.dir.name, "server_harness")
        build(cls.binary)

    @classmethod
    def tearDownClass(cls):
        cls.dir.cleanup()

    def setUp(self):
        self.server = Server(self.binary, TOKEN)
        self.addCleanup(self.server.stop)

    def connect(self, token=TOKEN, **kw):
        r = Runner(self.server.port, token, timeout=5, **kw)
        self.addCleanup(r.close)
        return r

    def test_a_host_that_proves_the_token_is_served_and_the_runner_proves_it_back(self):
        host = self.connect()
        self.assertTrue(host.first["ok"], host.first)
        self.assertTrue(host.proven, host.first)
        self.assertEqual(host.ok("snapshot")["op"], "snapshot")

    def test_a_connection_with_another_token_is_refused_and_the_current_one_keeps_working(self):
        host = self.connect()
        outsider = self.connect(token=secrets.token_hex(32))
        self.assertEqual(outsider.first["error"]["code"], "unauthorized", outsider.first)
        self.assertNotIn("proof", outsider.first)
        self.assertTrue(outsider.closed())
        self.assertEqual(host.ok("type", text="still mine")["op"], "type")

    def test_an_operation_before_the_hello_is_refused_and_never_served(self):
        host = self.connect()
        outsider = self.connect(token=None)
        reply = outsider.call("snapshot")
        self.assertEqual(reply["error"]["code"], "unauthorized", reply)
        self.assertNotIn("op", reply)
        self.assertTrue(outsider.closed())
        self.assertEqual(host.ok("snapshot")["op"], "snapshot")

    def test_a_hello_with_only_the_public_values_is_refused(self):
        outsider = self.connect(token=None)
        reply = outsider.call("hello", version="0.2.0", udid="00008110-001E04900239801E", build="0123456789abcdef")
        self.assertEqual(reply["error"]["code"], "unauthorized", reply)

    def test_a_first_frame_read_off_the_network_does_not_open_a_second_connection(self):
        nonce = secrets.token_hex(16)
        frame = {"id": 1, "op": "hello", "version": "0.2.0", "nonce": nonce, "proof": proof(TOKEN, "host", nonce)}
        host = self.connect(token=None)
        self.assertTrue(host.raw(frame)["ok"])
        replay = self.connect(token=None)
        self.assertEqual(replay.raw(frame)["error"]["code"], "unauthorized")
        self.assertEqual(host.ok("snapshot")["op"], "snapshot")

    def test_a_large_first_frame_is_dropped_unread(self):
        host = self.connect()
        outsider = self.connect(token=None)
        outsider.send({"op": "hello", "pad": "x" * 8000})
        self.assertTrue(outsider.closed())
        self.assertEqual(host.ok("snapshot")["op"], "snapshot")

    def test_a_new_host_connection_that_proves_the_token_replaces_the_current_one(self):
        first = self.connect()
        second = self.connect()
        self.assertTrue(second.proven)
        self.assertTrue(first.closed())
        self.assertEqual(second.ok("snapshot")["op"], "snapshot")

    def test_the_listener_takes_ipv4_and_ipv6(self):
        v4 = self.connect()
        self.assertTrue(v4.proven)
        v6 = self.connect(host="::1", family=socket.AF_INET6)
        self.assertTrue(v6.proven)

    def test_a_runner_started_without_a_token_serves_nobody(self):
        bare = Server(self.binary, "")
        self.addCleanup(bare.stop)
        r = Runner(bare.port, "", timeout=5)
        self.addCleanup(r.close)
        self.assertEqual(r.first["error"]["code"], "unauthorized", r.first)
        self.assertTrue(r.closed())


if __name__ == "__main__":
    unittest.main()
