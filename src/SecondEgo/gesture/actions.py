"""Gesture-to-action mapping for SecondEgo.

Each gesture maps to a GestureAction with a name and human-readable
description that is broadcast to UI subscribers.

Action catalogue
----------------
start_run       – trigger a new agent run (requires an issue to be queued)
approve_run     – approve / confirm the current pending action
cancel_run      – cancel / abort the active run
scroll_up       – scroll the event feed upward
scroll_down     – scroll the event feed downward
pause           – pause / hold (open palm = "stop")
navigate_next   – move to the next run / tab
navigate_prev   – move to the previous run / tab
zoom_in         – increase terminal font size
no_action       – explicitly bound but intentionally a no-op
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Callable


@dataclass(frozen=True)
class GestureAction:
    name: str
    description: str
    gesture: str


# ── Default binding table ─────────────────────────────────────────────────────
# Users can override this mapping at runtime via the gesture watcher config.
DEFAULT_GESTURE_MAP: dict[str, GestureAction] = {
    "thumbs_up": GestureAction(
        name="approve_run",
        description="Approve / accept the current agent run",
        gesture="thumbs_up",
    ),
    "thumbs_down": GestureAction(
        name="cancel_run",
        description="Cancel / abort the active agent run",
        gesture="thumbs_down",
    ),
    "open_palm": GestureAction(
        name="pause",
        description="Pause and hold — stop processing new requests",
        gesture="open_palm",
    ),
    "fist": GestureAction(
        name="start_run",
        description="Trigger a new agent run from the pending issue queue",
        gesture="fist",
    ),
    "point": GestureAction(
        name="navigate_next",
        description="Move to the next run or tab",
        gesture="point",
    ),
    "two_fingers": GestureAction(
        name="scroll_up",
        description="Scroll the event feed upward",
        gesture="two_fingers",
    ),
    "three_fingers": GestureAction(
        name="scroll_down",
        description="Scroll the event feed downward",
        gesture="three_fingers",
    ),
    "four_fingers": GestureAction(
        name="navigate_prev",
        description="Move to the previous run or tab",
        gesture="four_fingers",
    ),
    "pinch": GestureAction(
        name="zoom_in",
        description="Zoom in / increase terminal font size",
        gesture="pinch",
    ),
}


class ActionDispatcher:
    """Resolves a gesture name to a GestureAction and calls registered handlers."""

    def __init__(
        self,
        gesture_map: dict[str, GestureAction] | None = None,
    ) -> None:
        self._map = gesture_map if gesture_map is not None else DEFAULT_GESTURE_MAP
        self._handlers: list[Callable[[GestureAction], None]] = []

    def register(self, handler: Callable[[GestureAction], None]) -> None:
        """Register a callable that will be called with each GestureAction."""
        self._handlers.append(handler)

    def dispatch(self, gesture: str) -> GestureAction | None:
        """Look up and dispatch a gesture. Returns the action or None if unknown."""
        action = self._map.get(gesture)
        if action is None:
            return None
        for handler in self._handlers:
            try:
                handler(action)
            except Exception:  # noqa: BLE001
                pass  # never let a UI handler crash the gesture loop
        return action

    def all_bindings(self) -> list[tuple[str, GestureAction]]:
        """Return all current gesture → action bindings for display."""
        return sorted(self._map.items())
