"""Main gesture watcher — camera capture loop with live OpenCV overlay.

Requires:
    pip install opencv-python mediapipe

Run standalone:
    python -m SecondEgo.gesture.watcher [--camera 0] [--port 52381] [--headless]

Or programmatically:
    from SecondEgo.gesture.watcher import GestureWatcher
    watcher = GestureWatcher()
    watcher.start()          # non-blocking, runs in daemon thread
    ...
    watcher.stop()
"""
from __future__ import annotations

import argparse
import logging
import threading
import time
from collections import deque
from typing import TYPE_CHECKING

from SecondEgo.gesture.actions import ActionDispatcher, DEFAULT_GESTURE_MAP
from SecondEgo.gesture.broker import GestureBroker
from SecondEgo.gesture.classifier import Landmark, classify

if TYPE_CHECKING:
    pass  # keep import-time footprint minimal

logger = logging.getLogger(__name__)

# ── Tuneable constants ────────────────────────────────────────────────────────
DEBOUNCE_FRAMES = 8       # gesture must hold for N consecutive frames to fire
COOLDOWN_SECONDS = 1.2    # minimum gap between two identical action fires
DISPLAY_HUD_SECONDS = 2.0 # how long the action name stays on the overlay

# Gesture → emoji for the live overlay
GESTURE_EMOJI: dict[str, str] = {
    "thumbs_up":    "👍  APPROVE",
    "thumbs_down":  "👎  CANCEL",
    "open_palm":    "✋  PAUSE",
    "fist":         "✊  START RUN",
    "point":        "☝  NEXT",
    "two_fingers":  "✌  SCROLL UP",
    "three_fingers":"🤟  SCROLL DOWN",
    "four_fingers": "🖐  PREV",
    "pinch":        "🤌  ZOOM IN",
}


class _Debouncer:
    """Require a gesture to be stable for DEBOUNCE_FRAMES before confirming."""

    def __init__(self, frames: int = DEBOUNCE_FRAMES) -> None:
        self._frames = frames
        self._history: deque[str | None] = deque(maxlen=frames)

    def feed(self, gesture: str | None) -> str | None:
        """Feed one frame's gesture. Returns gesture name when debounce fires."""
        self._history.append(gesture)
        if len(self._history) < self._frames:
            return None
        candidates = [g for g in self._history if g is not None]
        if len(candidates) == self._frames and len(set(candidates)) == 1:
            return candidates[0]
        return None


class _Cooldown:
    """Prevent the same action from firing more than once per cooldown window."""

    def __init__(self, seconds: float = COOLDOWN_SECONDS) -> None:
        self._seconds = seconds
        self._last: dict[str, float] = {}

    def allow(self, gesture: str) -> bool:
        now = time.monotonic()
        if now - self._last.get(gesture, 0) >= self._seconds:
            self._last[gesture] = now
            return True
        return False


class GestureWatcher:
    """Background camera loop: captures frames, detects hands, fires actions.

    Decoupled from OpenCV / MediaPipe at import time — those are imported
    lazily inside ``start()`` so the rest of SecondEgo can import this module
    without requiring the optional CV packages.
    """

    def __init__(
        self,
        camera_index: int = 0,
        port: int = 52381,
        headless: bool = False,
        debounce_frames: int = DEBOUNCE_FRAMES,
        cooldown_seconds: float = COOLDOWN_SECONDS,
    ) -> None:
        self._camera_index = camera_index
        self._headless = headless
        self._broker = GestureBroker(port=port)
        self._dispatcher = ActionDispatcher(DEFAULT_GESTURE_MAP)
        # Wire broker as the primary action handler
        self._dispatcher.register(
            lambda action: self._broker.publish(action, hand=self._last_hand)
        )
        self._debouncer = _Debouncer(frames=debounce_frames)
        self._cooldown = _Cooldown(seconds=cooldown_seconds)
        self._last_hand = "Right"
        self._running = False
        self._thread: threading.Thread | None = None
        self._last_action_label: str = ""
        self._last_action_time: float = 0.0

    # ── Public API ────────────────────────────────────────────────────────────

    def start(self) -> None:
        """Start watcher in a daemon thread (non-blocking)."""
        if self._running:
            return
        self._broker.start()
        self._running = True
        self._broker.publish_status("starting")
        self._thread = threading.Thread(
            target=self._camera_loop, daemon=True, name="gesture-watcher"
        )
        self._thread.start()
        logger.info("GestureWatcher started (camera %d)", self._camera_index)

    def stop(self) -> None:
        """Stop the watcher gracefully."""
        self._running = False
        if self._thread:
            self._thread.join(timeout=3.0)
        self._broker.stop()
        logger.info("GestureWatcher stopped")

    def add_handler(self, handler) -> None:  # type: ignore[type-arg]
        """Register an additional action handler beyond the socket broker."""
        self._dispatcher.register(handler)

    # ── Main loop ─────────────────────────────────────────────────────────────

    # ── MediaPipe model management ─────────────────────────────────────────────

    @staticmethod
    def _ensure_hand_model() -> str:
        """Download the MediaPipe hand-landmarker .task file on first use.

        Returns the local filesystem path to the model.
        Raises RuntimeError if the download fails.
        """
        import os
        import urllib.request

        cache_dir = os.path.join(os.path.expanduser("~"), ".cache", "secondego")
        model_path = os.path.join(cache_dir, "hand_landmarker.task")
        if not os.path.exists(model_path):
            os.makedirs(cache_dir, exist_ok=True)
            url = (
                "https://storage.googleapis.com/mediapipe-models/"
                "hand_landmarker/hand_landmarker/float16/1/hand_landmarker.task"
            )
            logger.info("Downloading MediaPipe hand-landmarker model …")
            try:
                import ssl
                context = ssl._create_unverified_context()
                with urllib.request.urlopen(url, context=context) as response:
                    with open(model_path, "wb") as f:
                        f.write(response.read())
                logger.info("Model saved to %s", model_path)
            except Exception as exc:  # noqa: BLE001
                # Clean up partial file so next run re-tries
                if os.path.exists(model_path):
                    os.remove(model_path)
                raise RuntimeError(
                    f"Could not download hand_landmarker.task: {exc}"
                ) from exc
        return model_path

    def _camera_loop(self) -> None:
        cap = None
        try:
            import cv2  # type: ignore[import-not-found]
            import mediapipe as mp  # type: ignore[import-not-found]
        except ImportError as exc:
            logger.error(
                "Gesture detection requires 'opencv-python' and 'mediapipe': %s", exc
            )
            self._broker.publish_status(
                "unavailable",
                "Install the optional landmark-tracking dependencies.",
            )
            self._running = False
            return

        # ── Select implementation path ────────────────────────────────────────
        # MediaPipe ≥0.10 removed mp.solutions.hands; use the Tasks API instead.
        # Fall back to the legacy solutions API if still available (0.9.x).
        use_tasks_api = not hasattr(mp, "solutions") or not hasattr(
            getattr(mp, "solutions", None), "hands"
        )

        try:
            if use_tasks_api:
                self._run_tasks_api(cv2, mp)
            else:
                self._run_solutions_api(cv2, mp)
        except Exception:  # noqa: BLE001
            logger.exception("Landmark gesture tracking stopped unexpectedly")
            self._broker.publish_status(
                "error",
                "Landmark gesture tracking stopped unexpectedly.",
            )
        finally:
            self._running = False

    # ── Tasks API (MediaPipe ≥0.10) ───────────────────────────────────────────

    def _run_tasks_api(self, cv2, mp) -> None:  # type: ignore[type-arg]
        """Camera loop using mp.tasks.vision.HandLandmarker (MediaPipe ≥0.10)."""
        vision = mp.tasks.vision
        tasks = mp.tasks

        try:
            model_path = self._ensure_hand_model()
        except RuntimeError as exc:
            logger.error("Cannot start gesture detection: %s", exc)
            self._broker.publish_status("unavailable", str(exc))
            self._running = False
            return

        options = vision.HandLandmarkerOptions(
            base_options=tasks.BaseOptions(model_asset_path=model_path),
            running_mode=vision.RunningMode.IMAGE,
            num_hands=2,
            min_hand_detection_confidence=0.7,
            min_hand_presence_confidence=0.5,
            min_tracking_confidence=0.5,
        )

        cap = cv2.VideoCapture(self._camera_index)
        if not cap.isOpened():
            logger.error("Cannot open camera index %d", self._camera_index)
            self._broker.publish_status(
                "unavailable",
                "The selected camera could not be opened.",
            )
            self._running = False
            return

        try:
            with vision.HandLandmarker.create_from_options(options) as landmarker:
                self._broker.publish_status("active")
                while self._running:
                    ok, frame = cap.read()
                    if not ok:
                        logger.warning("Camera read failed — retrying")
                        time.sleep(0.05)
                        continue

                    frame = cv2.flip(frame, 1)
                    rgb = cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)
                    mp_image = mp.Image(
                        image_format=mp.ImageFormat.SRGB, data=rgb
                    )
                    result = landmarker.detect(mp_image)

                    detected_gesture: str | None = None

                    if result.hand_landmarks:
                        for idx, hand_lm_list in enumerate(result.hand_landmarks):
                            # Handedness: "Left"/"Right" from result.handedness
                            if result.handedness and idx < len(result.handedness):
                                label = result.handedness[idx][0].display_name
                                self._last_hand = label

                            lm = [
                                Landmark(lm.x, lm.y, lm.z)
                                for lm in hand_lm_list
                            ]
                            gesture = classify(lm)
                            if gesture:
                                detected_gesture = gesture

                            if not self._headless:
                                # Draw landmarks manually using new utils
                                self._draw_hand_landmarks_tasks(
                                    cv2, frame, hand_lm_list
                                )

                    confirmed = self._debouncer.feed(detected_gesture)
                    if confirmed and self._cooldown.allow(confirmed):
                        action = self._dispatcher.dispatch(confirmed)
                        if action:
                            self._last_action_label = (
                                GESTURE_EMOJI.get(confirmed, confirmed.upper())
                            )
                            self._last_action_time = time.monotonic()
                            logger.info(
                                "Gesture fired: %s → %s", confirmed, action.name
                            )

                    if not self._headless:
                        self._draw_hud(frame, detected_gesture)
                        cv2.imshow("SecondEgo Gesture Control", frame)
                        if cv2.waitKey(1) & 0xFF == ord("q"):
                            self._running = False
        finally:
            self._running = False
            cap.release()
            if not self._headless:
                try:
                    import cv2 as _cv2  # type: ignore[import-not-found]
                    _cv2.destroyAllWindows()
                except Exception:  # noqa: BLE001
                    pass

    def _draw_hand_landmarks_tasks(self, cv2, frame, hand_lm_list) -> None:  # type: ignore[type-arg]
        """Draw landmarks from the Tasks API result onto the frame."""
        h, w = frame.shape[:2]
        # Draw dots at each landmark
        for lm in hand_lm_list:
            cx, cy = int(lm.x * w), int(lm.y * h)
            cv2.circle(frame, (cx, cy), 4, (0, 220, 100), -1)

    # ── Solutions API (MediaPipe 0.9.x legacy) ────────────────────────────────

    def _run_solutions_api(self, cv2, mp) -> None:  # type: ignore[type-arg]
        """Camera loop using the legacy mp.solutions.hands (MediaPipe <0.10)."""
        cap = None
        try:
            mp_hands = mp.solutions.hands
            mp_draw = mp.solutions.drawing_utils

            cap = cv2.VideoCapture(self._camera_index)
            if not cap.isOpened():
                logger.error("Cannot open camera index %d", self._camera_index)
                self._broker.publish_status(
                    "unavailable",
                    "The selected camera could not be opened.",
                )
                return

            with mp_hands.Hands(
                static_image_mode=False,
                max_num_hands=2,
                min_detection_confidence=0.7,
                min_tracking_confidence=0.5,
            ) as hands:
                self._broker.publish_status("active")
                while self._running:
                    ok, frame = cap.read()
                    if not ok:
                        logger.warning("Camera read failed — retrying")
                        time.sleep(0.05)
                        continue

                    frame = cv2.flip(frame, 1)
                    rgb = cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)
                    result = hands.process(rgb)

                    detected_gesture: str | None = None

                    if result.multi_hand_landmarks:
                        for hand_landmarks, handedness in zip(
                            result.multi_hand_landmarks,
                            result.multi_handedness,
                        ):
                            label = handedness.classification[0].label
                            self._last_hand = label

                            lm = [
                                Landmark(pt.x, pt.y, pt.z)
                                for pt in hand_landmarks.landmark
                            ]
                            gesture = classify(lm)
                            if gesture:
                                detected_gesture = gesture

                            if not self._headless:
                                mp_draw.draw_landmarks(
                                    frame,
                                    hand_landmarks,
                                    mp_hands.HAND_CONNECTIONS,
                                )

                    confirmed = self._debouncer.feed(detected_gesture)
                    if confirmed and self._cooldown.allow(confirmed):
                        action = self._dispatcher.dispatch(confirmed)
                        if action:
                            self._last_action_label = (
                                GESTURE_EMOJI.get(confirmed, confirmed.upper())
                            )
                            self._last_action_time = time.monotonic()
                            logger.info(
                                "Gesture fired: %s → %s", confirmed, action.name
                            )

                    if not self._headless:
                        self._draw_hud(frame, detected_gesture)
                        cv2.imshow("SecondEgo Gesture Control", frame)
                        if cv2.waitKey(1) & 0xFF == ord("q"):
                            self._running = False
        finally:
            self._running = False
            if cap is not None:
                cap.release()
            if not self._headless:
                try:
                    cv2.destroyAllWindows()
                except Exception:  # noqa: BLE001
                    pass

    def _draw_hud(self, frame, detected: str | None) -> None:  # type: ignore[type-arg]
        """Draw semi-transparent HUD overlay on the camera frame."""
        try:
            import cv2  # type: ignore[import-not-found]
            import numpy as np  # type: ignore[import-not-found]
        except ImportError:
            return

        h, w = frame.shape[:2]

        # Top banner: current detected (raw, pre-debounce)
        banner = f"Detecting: {detected or 'none'}"
        overlay = frame.copy()
        cv2.rectangle(overlay, (0, 0), (w, 48), (20, 20, 20), -1)
        cv2.addWeighted(overlay, 0.6, frame, 0.4, 0, frame)
        cv2.putText(
            frame, banner, (12, 32),
            cv2.FONT_HERSHEY_SIMPLEX, 0.8, (200, 230, 255), 2
        )

        # Bottom banner: last confirmed action (shown for DISPLAY_HUD_SECONDS)
        if self._last_action_label and (
            time.monotonic() - self._last_action_time < DISPLAY_HUD_SECONDS
        ):
            action_overlay = frame.copy()
            cv2.rectangle(action_overlay, (0, h - 60), (w, h), (0, 160, 80), -1)
            cv2.addWeighted(action_overlay, 0.7, frame, 0.3, 0, frame)
            cv2.putText(
                frame,
                f"  {self._last_action_label}",
                (12, h - 18),
                cv2.FONT_HERSHEY_SIMPLEX, 1.0, (255, 255, 255), 2,
            )

        # Corner hint
        hint = "Q to quit"
        cv2.putText(
            frame, hint, (w - 110, h - 10),
            cv2.FONT_HERSHEY_SIMPLEX, 0.45, (150, 150, 150), 1
        )


# ── CLI entry point ───────────────────────────────────────────────────────────

def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="secondego-gestures",
        description="SecondEgo gesture control — runs OpenCV hand detection and "
                    "publishes actions to localhost:52381",
    )
    parser.add_argument("--camera", type=int, default=0, metavar="N",
                        help="Camera index (default: 0)")
    parser.add_argument("--port", type=int, default=52381,
                        help="Broker TCP port (default: 52381)")
    parser.add_argument("--headless", action="store_true",
                        help="Disable the OpenCV preview window")
    parser.add_argument("--debug", action="store_true",
                        help="Enable DEBUG logging")
    return parser.parse_args()


def main() -> None:
    args = _parse_args()
    logging.basicConfig(
        level=logging.DEBUG if args.debug else logging.INFO,
        format="%(asctime)s  %(name)s  %(levelname)s  %(message)s",
    )
    watcher = GestureWatcher(
        camera_index=args.camera,
        port=args.port,
        headless=args.headless,
    )
    print(
        f"\n{'─'*54}\n"
        f"  SecondEgo Gesture Control\n"
        f"  Camera : {args.camera}\n"
        f"  Broker : localhost:{args.port}\n"
        f"  Mode   : {'headless' if args.headless else 'live preview'}\n"
        f"{'─'*54}\n"
        "  Gesture bindings:\n"
    )
    for gesture, action in ActionDispatcher().all_bindings():
        emoji = GESTURE_EMOJI.get(gesture, "  ")
        print(f"    {emoji:<22}  →  {action.name}")
    print(f"\n  Press Ctrl-C to stop.\n{'─'*54}\n")

    # Blocking run: camera loop runs in main thread when not headless,
    # in daemon thread when headless so Ctrl-C still works.
    if args.headless:
        watcher.start()
        try:
            while watcher._running:  # noqa: SLF001 - CLI owns this watcher
                time.sleep(1)
        except KeyboardInterrupt:
            pass
        finally:
            watcher.stop()
    else:
        # Run camera loop in main thread (required by OpenCV on macOS)
        watcher._broker.start()  # noqa: SLF001
        watcher._running = True  # noqa: SLF001
        watcher._broker.publish_status("starting")  # noqa: SLF001
        try:
            watcher._camera_loop()  # noqa: SLF001
        except KeyboardInterrupt:
            pass
        finally:
            watcher.stop()


if __name__ == "__main__":
    main()
