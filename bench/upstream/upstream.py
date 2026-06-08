#!/usr/bin/env python3
"""Bench upstream echo + control server.

Stdlib only. Single file. Listens on 0.0.0.0:8080.

Endpoints:
    GET  /healthz                  -> 200 "ok\n"
    GET  /                         -> 200, body of N x's. ?bytes=N (default 200, cap 1 MiB).
    POST /__ctl/sleep?ms=N         -> sleeps N ms, then 200 with empty body.
    POST /__ctl/drop?code=502      -> arms a drop state. Subsequent requests return
                                      the armed status code with empty body.
    POST /__ctl/resume             -> clears the drop state. 200.
    POST /__ctl/shutdown           -> 200, then os._exit(0).
    anything else                  -> 404.

Concurrency model: ThreadingHTTPServer (one thread per connection). The
`__ctl/sleep` endpoint must not block other requests, so the handler thread
per request is required.

Drop state is a module-level bool + int guarded by a Lock, so the control
endpoints can mutate it atomically while request handlers read it.
"""

from __future__ import annotations

import os
import sys
import threading
import time
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT = 8080
MAX_BODY_BYTES = 1024 * 1024  # 1 MiB
DEFAULT_BODY_BYTES = 200
BODY_BYTE = b"x"

# --- module-level drop state (mutated by /__ctl/drop and /__ctl/resume) -----
_drop_state_lock = threading.Lock()
_drop_active: bool = False
_drop_code: int = 502


def _arm_drop(code: int) -> None:
    global _drop_active, _drop_code
    with _drop_state_lock:
        _drop_active = True
        _drop_code = code


def _clear_drop() -> None:
    global _drop_active
    with _drop_state_lock:
        _drop_active = False


def _drop_snapshot() -> tuple[bool, int]:
    with _drop_state_lock:
        return (_drop_active, _drop_code)


class _UpstreamHandler(BaseHTTPRequestHandler):
    server_version = "bench-upstream/1.0"
    # Quiet the default BaseHTTPRequestHandler access log; we log ourselves.
    def log_message(self, format: str, *args) -> None:  # noqa: A002
        return

    # ------------------------------------------------------------------
    # Request dispatch
    # ------------------------------------------------------------------
    def _dispatch(self) -> None:
        start_ns = time.monotonic_ns()
        method = self.command
        parsed = urllib.parse.urlsplit(self.path)
        path = parsed.path
        query = urllib.parse.parse_qs(parsed.query, keep_blank_values=True)

        status = 404
        try:
            # Drop state is checked FIRST, before sleeping or body gen.
            dropped, drop_code = _drop_snapshot()
            if dropped:
                self._send_empty(drop_code)
                status = drop_code
                return

            if method == "GET" and path == "/healthz":
                self._send_body(200, b"ok\n", "text/plain")
                status = 200
                return

            if method == "GET" and path == "/":
                size = self._parse_bytes(query)
                if size is None:
                    self._send_empty(400)
                    status = 400
                    return
                body = BODY_BYTE * size
                self._send_body(200, body, "application/octet-stream")
                status = 200
                return

            if method == "POST" and path == "/__ctl/sleep":
                ms = self._parse_ms(query)
                if ms is None:
                    self._send_empty(400)
                    status = 400
                    return
                # Re-check drop state after the sleep — a /__ctl/drop may have
                # arrived while we were waiting. If so, return the dropped code.
                time.sleep(ms / 1000.0)
                dropped, drop_code = _drop_snapshot()
                if dropped:
                    self._send_empty(drop_code)
                    status = drop_code
                    return
                self._send_empty(200, "application/octet-stream")
                status = 200
                return

            if method == "POST" and path == "/__ctl/drop":
                code = self._parse_code(query)
                if code is None:
                    self._send_empty(400)
                    status = 400
                    return
                _arm_drop(code)
                self._send_empty(200)
                status = 200
                return

            if method == "POST" and path == "/__ctl/resume":
                _clear_drop()
                self._send_empty(200)
                status = 200
                return

            if method == "POST" and path == "/__ctl/shutdown":
                self._send_empty(200)
                self.wfile.flush()
                status = 200
                # Respond, then exit. We log AFTER sending so the client sees
                # the 200 before the connection is torn down.
                self._log(method, path, status, start_ns)
                os._exit(0)

            # Fall-through: 404.
            self._send_empty(404)
            status = 404
        finally:
            self._log(method, path, status, start_ns)

    def do_GET(self) -> None:  # noqa: N802 (BaseHTTPRequestHandler API)
        self._dispatch()

    def do_POST(self) -> None:  # noqa: N802
        self._dispatch()

    # ------------------------------------------------------------------
    # Response helpers
    # ------------------------------------------------------------------
    def _send_body(self, status: int, body: bytes, content_type: str) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    def _send_empty(self, status: int, content_type: str = "text/plain") -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", "0")
        self.end_headers()

    # ------------------------------------------------------------------
    # Query-param parsing
    # ------------------------------------------------------------------
    @staticmethod
    def _parse_bytes(query: dict[str, list[str]]) -> int | None:
        raw = query.get("bytes", [str(DEFAULT_BODY_BYTES)])[0]
        try:
            n = int(raw)
        except (TypeError, ValueError):
            return None
        if n < 0 or n > MAX_BODY_BYTES:
            return None
        return n

    @staticmethod
    def _parse_ms(query: dict[str, list[str]]) -> int | None:
        raw = query.get("ms", ["0"])[0]
        try:
            n = int(raw)
        except (TypeError, ValueError):
            return None
        if n < 0 or n > 600_000:  # 10 min hard cap
            return None
        return n

    @staticmethod
    def _parse_code(query: dict[str, list[str]]) -> int | None:
        raw = query.get("code", ["502"])[0]
        try:
            n = int(raw)
        except (TypeError, ValueError):
            return None
        if n < 100 or n > 599:
            return None
        return n

    # ------------------------------------------------------------------
    # Logging
    # ------------------------------------------------------------------
    @staticmethod
    def _log(method: str, path: str, status: int, start_ns: int) -> None:
        ts = time.strftime("%H:%M:%S")
        dur_ms = (time.monotonic_ns() - start_ns) / 1_000_000.0
        sys.stderr.write(f"{ts} {method} {path} {status} {dur_ms:.2f}\n")
        sys.stderr.flush()


def main() -> None:
    server = ThreadingHTTPServer(("0.0.0.0", PORT), _UpstreamHandler)
    sys.stderr.write(f"bench-upstream listening on 0.0.0.0:{PORT}\n")
    sys.stderr.flush()
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
