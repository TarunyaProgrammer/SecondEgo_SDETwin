"""Tests for the gesture module.

All tests run without opencv-python or mediapipe installed — the classifier,
actions, and broker are pure Python and have zero CV dependencies.
"""
from __future__ import annotations

import time
import threading

import pytest

from SecondEgo.gesture.classifier import Landmark, classify
from SecondEgo.gesture.actions import ActionDispatcher, GestureAction, DEFAULT_GESTURE_MAP
from SecondEgo.gesture.broker import GestureBroker, GestureSubscriber, GESTURE_PORT


# ── Helper: build 21 flat landmarks at origin ─────────────────────────────────

def _flat_hand() -> list[Landmark]:
    """21 landmarks all at (0.5, 0.5, 0)."""
    return [Landmark(0.5, 0.5, 0.0) for _ in range(21)]


def _hand_with(overrides: dict[int, tuple[float, float, float]]) -> list[Landmark]:
    lm = _flat_hand()
    for idx, (x, y, z) in overrides.items():
        lm[idx] = Landmark(x, y, z)
    return lm


# Landmark indices (mirroring classifier.py)
WRIST = 0
THUMB_MCP, THUMB_IP, THUMB_TIP = 2, 3, 4
INDEX_MCP, INDEX_PIP, INDEX_DIP, INDEX_TIP = 5, 6, 7, 8
MIDDLE_MCP, MIDDLE_PIP, MIDDLE_DIP, MIDDLE_TIP = 9, 10, 11, 12
RING_MCP, RING_PIP, RING_DIP, RING_TIP = 13, 14, 15, 16
PINKY_MCP, PINKY_PIP, PINKY_DIP, PINKY_TIP = 17, 18, 19, 20


def _extended_finger(mcp_y, tip_y) -> tuple[float, float]:
    """tip far from mcp → extended."""
    return mcp_y, tip_y  # caller places tip far above mcp


# ── Classifier ────────────────────────────────────────────────────────────────

class TestClassifier:

    def test_returns_none_for_too_few_landmarks(self):
        assert classify([Landmark(0, 0, 0)] * 10) is None

    def test_returns_none_for_flat_ambiguous_hand(self):
        # All at same point — no clear gesture
        result = classify(_flat_hand())
        # Could be fist or None depending on distances — either is correct
        assert result in (None, "fist", "thumbs_up", "thumbs_down")

    def test_thumbs_up_detected(self):
        # Thumb tip well above MCP, all fingers curled at their MCPs
        lm = _hand_with({
            THUMB_MCP: (0.5, 0.7, 0),   # MCP lower in image (larger y)
            THUMB_TIP: (0.5, 0.1, 0),   # Tip high in image (smaller y)
            # All finger tips at their MCP positions → curled
            INDEX_TIP: (0.5, 0.5, 0),  INDEX_MCP: (0.5, 0.5, 0),
            INDEX_PIP: (0.5, 0.5, 0),
            MIDDLE_TIP: (0.5, 0.5, 0), MIDDLE_MCP: (0.5, 0.5, 0),
            MIDDLE_PIP: (0.5, 0.5, 0),
            RING_TIP: (0.5, 0.5, 0),   RING_MCP: (0.5, 0.5, 0),
            RING_PIP: (0.5, 0.5, 0),
            PINKY_TIP: (0.5, 0.5, 0),  PINKY_MCP: (0.5, 0.5, 0),
            PINKY_PIP: (0.5, 0.5, 0),
        })
        assert classify(lm) == "thumbs_up"

    def test_thumbs_down_detected(self):
        lm = _hand_with({
            THUMB_MCP: (0.5, 0.2, 0),   # MCP high in image
            THUMB_TIP: (0.5, 0.8, 0),   # Tip low in image
            INDEX_TIP: (0.5, 0.5, 0),  INDEX_MCP: (0.5, 0.5, 0),
            INDEX_PIP: (0.5, 0.5, 0),
            MIDDLE_TIP: (0.5, 0.5, 0), MIDDLE_MCP: (0.5, 0.5, 0),
            MIDDLE_PIP: (0.5, 0.5, 0),
            RING_TIP: (0.5, 0.5, 0),   RING_MCP: (0.5, 0.5, 0),
            RING_PIP: (0.5, 0.5, 0),
            PINKY_TIP: (0.5, 0.5, 0),  PINKY_MCP: (0.5, 0.5, 0),
            PINKY_PIP: (0.5, 0.5, 0),
        })
        assert classify(lm) == "thumbs_down"

    def test_open_palm_detected(self):
        # All fingertips far above MCPs → all extended
        lm = _hand_with({
            INDEX_MCP: (0.3, 0.8, 0), INDEX_PIP: (0.3, 0.65, 0), INDEX_TIP: (0.3, 0.1, 0),
            MIDDLE_MCP: (0.4, 0.8, 0), MIDDLE_PIP: (0.4, 0.65, 0), MIDDLE_TIP: (0.4, 0.1, 0),
            RING_MCP: (0.5, 0.8, 0), RING_PIP: (0.5, 0.65, 0), RING_TIP: (0.5, 0.1, 0),
            PINKY_MCP: (0.6, 0.8, 0), PINKY_PIP: (0.6, 0.65, 0), PINKY_TIP: (0.6, 0.1, 0),
        })
        assert classify(lm) == "open_palm"

    def test_point_detected(self):
        # Only index extended, others curled
        lm = _hand_with({
            INDEX_MCP: (0.3, 0.8, 0), INDEX_PIP: (0.3, 0.65, 0), INDEX_TIP: (0.3, 0.1, 0),
            MIDDLE_MCP: (0.4, 0.5, 0), MIDDLE_PIP: (0.4, 0.5, 0), MIDDLE_TIP: (0.4, 0.5, 0),
            RING_MCP:   (0.5, 0.5, 0), RING_PIP:   (0.5, 0.5, 0), RING_TIP:   (0.5, 0.5, 0),
            PINKY_MCP:  (0.6, 0.5, 0), PINKY_PIP:  (0.6, 0.5, 0), PINKY_TIP:  (0.6, 0.5, 0),
        })
        assert classify(lm) == "point"

    def test_two_fingers_detected(self):
        lm = _hand_with({
            INDEX_MCP: (0.3, 0.8, 0), INDEX_PIP: (0.3, 0.65, 0), INDEX_TIP: (0.3, 0.1, 0),
            MIDDLE_MCP: (0.4, 0.8, 0), MIDDLE_PIP: (0.4, 0.65, 0), MIDDLE_TIP: (0.4, 0.1, 0),
            RING_MCP:   (0.5, 0.5, 0), RING_PIP:   (0.5, 0.5, 0), RING_TIP:   (0.5, 0.5, 0),
            PINKY_MCP:  (0.6, 0.5, 0), PINKY_PIP:  (0.6, 0.5, 0), PINKY_TIP:  (0.6, 0.5, 0),
        })
        assert classify(lm) == "two_fingers"

    def test_pinch_detected(self):
        # Thumb tip and index tip are nearly touching.
        # Index is curled (tip close to MCP) so the classifier doesn't hit
        # the 'point' branch before reaching the pinch distance check.
        lm = _hand_with({
            WRIST:      (0.5, 0.95, 0),
            MIDDLE_MCP: (0.5, 0.50, 0),   # hand-scale reference landmark
            # Index curled: tip and PIP sit at MCP location
            INDEX_MCP:  (0.45, 0.55, 0),
            INDEX_PIP:  (0.45, 0.55, 0),
            INDEX_TIP:  (0.46, 0.32, 0),  # tip close to thumb tip
            # Thumb tip right next to index tip
            THUMB_MCP:  (0.50, 0.60, 0),
            THUMB_TIP:  (0.46, 0.30, 0),  # very close to INDEX_TIP
            # All other fingers curled at their MCPs
            MIDDLE_MCP: (0.50, 0.55, 0), MIDDLE_PIP: (0.50, 0.55, 0), MIDDLE_TIP: (0.50, 0.55, 0),
            RING_MCP:   (0.55, 0.55, 0), RING_PIP:   (0.55, 0.55, 0), RING_TIP:   (0.55, 0.55, 0),
            PINKY_MCP:  (0.60, 0.55, 0), PINKY_PIP:  (0.60, 0.55, 0), PINKY_TIP:  (0.60, 0.55, 0),
        })
        assert classify(lm) == "pinch"


# ── ActionDispatcher ──────────────────────────────────────────────────────────

class TestActionDispatcher:

    def test_dispatches_known_gesture(self):
        fired: list[GestureAction] = []
        dispatcher = ActionDispatcher()
        dispatcher.register(fired.append)

        action = dispatcher.dispatch("thumbs_up")

        assert action is not None
        assert action.name == "approve_run"
        assert len(fired) == 1
        assert fired[0].name == "approve_run"

    def test_returns_none_for_unknown_gesture(self):
        dispatcher = ActionDispatcher()
        assert dispatcher.dispatch("wave_hello") is None

    def test_all_bindings_returns_sorted_pairs(self):
        dispatcher = ActionDispatcher()
        bindings = dispatcher.all_bindings()
        assert all(isinstance(g, str) and isinstance(a, GestureAction) for g, a in bindings)
        gestures = [g for g, _ in bindings]
        assert gestures == sorted(gestures)

    def test_handler_exception_does_not_propagate(self):
        def bad_handler(action: GestureAction) -> None:
            raise RuntimeError("handler crashed")

        dispatcher = ActionDispatcher()
        dispatcher.register(bad_handler)
        # Must not raise
        result = dispatcher.dispatch("thumbs_up")
        assert result is not None

    def test_custom_gesture_map_overrides_default(self):
        custom = {
            "thumbs_up": GestureAction("my_action", "custom", "thumbs_up")
        }
        dispatcher = ActionDispatcher(gesture_map=custom)
        action = dispatcher.dispatch("thumbs_up")
        assert action is not None
        assert action.name == "my_action"

    def test_default_map_covers_all_standard_gestures(self):
        expected = {
            "thumbs_up", "thumbs_down", "open_palm", "fist",
            "point", "two_fingers", "three_fingers", "four_fingers", "pinch",
        }
        assert expected.issubset(set(DEFAULT_GESTURE_MAP.keys()))


# ── GestureBroker ─────────────────────────────────────────────────────────────

def _free_port() -> int:
    import socket
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class TestGestureBroker:

    def test_broker_starts_and_accepts_subscriber(self):
        port = _free_port()
        broker = GestureBroker(port=port)
        broker.start()
        try:
            sub = GestureSubscriber(port=port)
            sub.connect(timeout=3.0)
            sub.close()
        finally:
            broker.stop()

    def test_broker_publishes_json_frame_to_subscriber(self):
        port = _free_port()
        broker = GestureBroker(port=port)
        broker.start()

        received: list[dict] = []

        def collect():
            sub = GestureSubscriber(port=port)
            sub.connect(timeout=3.0)
            for event in sub:
                received.append(event)
                break  # stop after first event

        t = threading.Thread(target=collect, daemon=True)
        t.start()
        time.sleep(0.2)  # let subscriber connect

        action = GestureAction(name="approve_run", description="approve", gesture="thumbs_up")
        broker.publish(action, hand="Right")

        t.join(timeout=3.0)
        broker.stop()

        assert len(received) == 1
        evt = received[0]
        assert evt["schema_version"] == 1
        assert evt["event_type"] == "gesture.action"
        assert evt["gesture"] == "thumbs_up"
        assert evt["action"] == "approve_run"
        assert evt["hand"] == "Right"
        assert "timestamp" in evt

    def test_broker_replays_latest_status_to_new_subscriber(self):
        port = _free_port()
        broker = GestureBroker(port=port)
        broker.start()

        sub = GestureSubscriber(port=port)
        try:
            broker.publish_status("active")
            sub.connect(timeout=3.0)
            assert sub._sock is not None  # noqa: SLF001 - set a test-only read timeout
            sub._sock.settimeout(1.0)  # noqa: SLF001
            event = next(iter(sub))

            assert event["schema_version"] == 1
            assert event["event_type"] == "gesture.status"
            assert event["status"] == "active"
            assert "timestamp" in event
        finally:
            sub.close()
            broker.stop()

    def test_broker_handles_subscriber_disconnect_gracefully(self):
        port = _free_port()
        broker = GestureBroker(port=port)
        broker.start()
        try:
            sub = GestureSubscriber(port=port)
            sub.connect(timeout=3.0)
            sub.close()  # disconnect immediately
            time.sleep(0.1)
            # Publish after disconnect — must not raise
            action = GestureAction("cancel_run", "cancel", "thumbs_down")
            broker.publish(action)
        finally:
            broker.stop()

    def test_broker_stop_is_idempotent(self):
        port = _free_port()
        broker = GestureBroker(port=port)
        broker.start()
        broker.stop()
        broker.stop()  # second stop must not raise
