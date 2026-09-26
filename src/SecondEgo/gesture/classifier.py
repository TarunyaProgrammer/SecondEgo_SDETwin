"""Gesture classifier: maps raw MediaPipe landmark data to named gestures.

Supported gestures
------------------
thumbs_up      – thumb extended upward, other fingers curled
thumbs_down    – thumb extended downward, other fingers curled
open_palm      – all five fingers extended
fist           – all fingers curled into fist
point          – only index finger extended, others curled
two_fingers    – index + middle extended (victory / scroll-up)
three_fingers  – index + middle + ring extended
four_fingers   – all except thumb extended
pinch          – thumb and index tips close together
swipe_left     – palm moving left (tracked externally)
swipe_right    – palm moving right (tracked externally)
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import NamedTuple


class Landmark(NamedTuple):
    x: float
    y: float
    z: float


# MediaPipe hand landmark indices
_WRIST = 0
_THUMB_CMC, _THUMB_MCP, _THUMB_IP, _THUMB_TIP = 1, 2, 3, 4
_INDEX_MCP, _INDEX_PIP, _INDEX_DIP, _INDEX_TIP = 5, 6, 7, 8
_MIDDLE_MCP, _MIDDLE_PIP, _MIDDLE_DIP, _MIDDLE_TIP = 9, 10, 11, 12
_RING_MCP, _RING_PIP, _RING_DIP, _RING_TIP = 13, 14, 15, 16
_PINKY_MCP, _PINKY_PIP, _PINKY_DIP, _PINKY_TIP = 17, 18, 19, 20


def _dist(a: Landmark, b: Landmark) -> float:
    return ((a.x - b.x) ** 2 + (a.y - b.y) ** 2) ** 0.5


def _finger_extended(tip: Landmark, pip: Landmark, mcp: Landmark) -> bool:
    """Return True when the finger tip is further from the MCP than the PIP.

    Works in normalised landmark space (y increases downward).
    """
    return _dist(tip, mcp) > _dist(pip, mcp) * 1.1


def _thumb_extended_up(lm: list[Landmark]) -> bool:
    """Thumb tip clearly above its MCP joint in image space (y decreases upward)."""
    return lm[_THUMB_TIP].y < lm[_THUMB_MCP].y - 0.05


def _thumb_extended_down(lm: list[Landmark]) -> bool:
    """Thumb tip clearly below its MCP joint."""
    return lm[_THUMB_TIP].y > lm[_THUMB_MCP].y + 0.05


def _fingers_up(lm: list[Landmark]) -> list[bool]:
    """Return [index, middle, ring, pinky] extended booleans."""
    return [
        _finger_extended(lm[_INDEX_TIP], lm[_INDEX_PIP], lm[_INDEX_MCP]),
        _finger_extended(lm[_MIDDLE_TIP], lm[_MIDDLE_PIP], lm[_MIDDLE_MCP]),
        _finger_extended(lm[_RING_TIP], lm[_RING_PIP], lm[_RING_MCP]),
        _finger_extended(lm[_PINKY_TIP], lm[_PINKY_PIP], lm[_PINKY_MCP]),
    ]


def _all_curled(fingers: list[bool]) -> bool:
    return not any(fingers)


def classify(landmarks: list[Landmark]) -> str | None:
    """Classify a list of 21 normalised hand landmarks into a gesture name.

    Returns None when the hand shape does not match any known gesture.
    """
    if len(landmarks) < 21:
        return None

    lm = landmarks
    fingers = _fingers_up(lm)
    thumb_up = _thumb_extended_up(lm)
    thumb_down = _thumb_extended_down(lm)
    all_curled = _all_curled(fingers)

    # Pinch: thumb and index tips very close — check FIRST (most specific)
    pinch_dist = _dist(lm[_THUMB_TIP], lm[_INDEX_TIP])
    hand_scale = _dist(lm[_WRIST], lm[_MIDDLE_MCP])
    if hand_scale > 0 and pinch_dist / hand_scale < 0.2:
        return "pinch"

    # Thumbs up: thumb high, all other fingers curled
    if thumb_up and all_curled:
        return "thumbs_up"

    # Thumbs down: thumb low, all other fingers curled
    if thumb_down and all_curled:
        return "thumbs_down"

    # Fist: all fingers curled (thumb not extended either direction)
    if all_curled and not thumb_up and not thumb_down:
        return "fist"

    # Open palm: all fingers extended
    if all(fingers):
        return "open_palm"

    # Point: only index extended
    if fingers[0] and not fingers[1] and not fingers[2] and not fingers[3]:
        return "point"

    # Two fingers (victory): index + middle
    if fingers[0] and fingers[1] and not fingers[2] and not fingers[3]:
        return "two_fingers"

    # Three fingers: index + middle + ring
    if fingers[0] and fingers[1] and fingers[2] and not fingers[3]:
        return "three_fingers"

    # Four fingers: all except thumb
    if all(fingers) and not thumb_up and not thumb_down:
        return "four_fingers"

    return None


@dataclass(frozen=True)
class GestureEvent:
    """A confirmed, debounced gesture event ready to be acted on."""
    gesture: str
    confidence: float  # 0.0–1.0, currently always 1.0 (rule-based)
    hand_label: str    # "Left" or "Right"
