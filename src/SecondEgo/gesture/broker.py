"""Gesture event broker — publishes GestureAction events over a TCP socket.

The watcher runs in a background thread. Any SecondEgo component (desktop
gateway, TUI, CLI) can subscribe by connecting to localhost:GESTURE_PORT and
reading newline-delimited JSON frames.

Frame schema
------------
{
  "schema_version": 1,
  "event_type": "gesture.action",
  "gesture": "<gesture_name>",
  "action": "<action_name>",
  "description": "<human description>",
  "hand": "Left" | "Right",
  "timestamp": "<ISO-8601>"
}
"""
from __future__ import annotations

import json
import logging
import socket
import threading
import time
from datetime import datetime, timezone
from typing import Any

from SecondEgo.gesture.actions import GestureAction

logger = logging.getLogger(__name__)

GESTURE_PORT = 52_381  # arbitrary high port, unlikely to conflict
GESTURE_SCHEMA_VERSION = 1


def _now_iso() -> str:
    return datetime.now(tz=timezone.utc).isoformat()


class GestureBroker:
    """TCP pub-sub broker for gesture events.

    Listens on ``localhost:GESTURE_PORT``. Each connected client receives
    every gesture event as a newline-delimited JSON string.
    """

    def __init__(self, port: int = GESTURE_PORT) -> None:
        self._port = port
        self._clients: list[socket.socket] = []
        self._lock = threading.Lock()
        self._server: socket.socket | None = None
        self._running = False
        self._accept_thread: threading.Thread | None = None

    # ── Lifecycle ─────────────────────────────────────────────────────────────

    def start(self) -> None:
        if self._running:
            return
        self._server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self._server.bind(("127.0.0.1", self._port))
        self._server.listen(8)
        self._server.settimeout(1.0)
        self._running = True
        self._accept_thread = threading.Thread(
            target=self._accept_loop, daemon=True, name="gesture-broker"
        )
        self._accept_thread.start()
        logger.info("GestureBroker listening on port %d", self._port)

    def stop(self) -> None:
        self._running = False
        if self._server:
            try:
                self._server.close()
            except OSError:
                pass
        with self._lock:
            for client in self._clients:
                try:
                    client.close()
                except OSError:
                    pass
            self._clients.clear()

    # ── Internal ──────────────────────────────────────────────────────────────

    def _accept_loop(self) -> None:
        assert self._server is not None
        while self._running:
            try:
                conn, addr = self._server.accept()
                logger.debug("Gesture subscriber connected from %s", addr)
                with self._lock:
                    self._clients.append(conn)
            except TimeoutError:
                continue
            except OSError:
                break

    def _remove_client(self, client: socket.socket) -> None:
        try:
            client.close()
        except OSError:
            pass
        with self._lock:
            self._clients = [c for c in self._clients if c is not client]

    # ── Publishing ────────────────────────────────────────────────────────────

    def publish(self, action: GestureAction, hand: str = "Right") -> None:
        """Broadcast a gesture action event to all connected subscribers."""
        frame = self._build_frame(action, hand)
        payload = (json.dumps(frame) + "\n").encode()
        with self._lock:
            dead: list[socket.socket] = []
            for client in self._clients:
                try:
                    client.sendall(payload)
                except OSError:
                    dead.append(client)
        for client in dead:
            self._remove_client(client)

    @staticmethod
    def _build_frame(action: GestureAction, hand: str) -> dict[str, Any]:
        return {
            "schema_version": GESTURE_SCHEMA_VERSION,
            "event_type": "gesture.action",
            "gesture": action.gesture,
            "action": action.name,
            "description": action.description,
            "hand": hand,
            "timestamp": _now_iso(),
        }


class GestureSubscriber:
    """Convenience client that reads gesture events from the broker.

    Usage::

        sub = GestureSubscriber()
        sub.connect()
        for event in sub:
            print(event["action"])
        sub.close()
    """

    def __init__(self, port: int = GESTURE_PORT) -> None:
        self._port = port
        self._sock: socket.socket | None = None
        self._buf = b""

    def connect(self, timeout: float = 5.0) -> None:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            try:
                self._sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                self._sock.connect(("127.0.0.1", self._port))
                return
            except ConnectionRefusedError:
                time.sleep(0.2)
        raise ConnectionRefusedError(
            f"GestureBroker not available on port {self._port}"
        )

    def close(self) -> None:
        if self._sock:
            try:
                self._sock.close()
            except OSError:
                pass

    def __iter__(self):  # type: ignore[override]
        assert self._sock is not None
        while True:
            try:
                chunk = self._sock.recv(4096)
            except OSError:
                break
            if not chunk:
                break
            self._buf += chunk
            while b"\n" in self._buf:
                line, self._buf = self._buf.split(b"\n", 1)
                if line.strip():
                    try:
                        yield json.loads(line)
                    except json.JSONDecodeError:
                        pass
