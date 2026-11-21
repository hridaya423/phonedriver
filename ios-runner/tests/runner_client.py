"""A client for PhonedriverRunner's protocol (docs/protocol.md section 3), shared by the runner's tests.

The first frame on a connection is a `hello` carrying the host's proof of the runner's token over a fresh nonce; the
reply carries the runner's proof over the same nonce (decision D106). The token itself never crosses the connection.
"""

import hashlib
import hmac
import json
import secrets
import socket
import struct


def proof(token, side, nonce):
    return hmac.new(token.encode(), f"{side}:{nonce}".encode(), hashlib.sha256).hexdigest()


class Runner:
    def __init__(self, port, token, host="127.0.0.1", family=socket.AF_INET, timeout=60):
        self.sock = socket.socket(family, socket.SOCK_STREAM)
        self.sock.settimeout(timeout)
        self.sock.connect((host, port))
        self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.next_id = 1
        self.first = None
        if token is not None:
            nonce = secrets.token_hex(16)
            self.first = self.call("hello", version="0.2.0", nonce=nonce, proof=proof(token, "host", nonce))
            self.proven = self.first.get("proof") == proof(token, "runner", nonce)

    def _read(self, n):
        buf = b""
        while len(buf) < n:
            chunk = self.sock.recv(n - len(buf))
            if not chunk:
                raise ConnectionError("runner closed the connection")
            buf += chunk
        return buf

    def send(self, request):
        body = json.dumps(request).encode()
        self.sock.sendall(struct.pack(">I", len(body)) + body)

    def receive(self):
        (length,) = struct.unpack(">I", self._read(4))
        return json.loads(self._read(length))

    def raw(self, request):
        self.send(request)
        return self.receive()

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

    def closed(self):
        try:
            self.sock.settimeout(2)
            return self.sock.recv(1) == b""
        except ConnectionResetError:
            return True

    def close(self):
        self.sock.close()
