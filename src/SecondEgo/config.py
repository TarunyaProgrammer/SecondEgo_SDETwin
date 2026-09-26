import os
from enum import StrEnum


DEFAULT_MODEL = "gemini-3.8-flash"


class PresentationMode(StrEnum):
    """Controls presentation only; it never changes engine policy or model calls."""

    HEADLESS = "headless"
    EVENTS = "events"


def configured_model() -> str:
    """Return the model selected by evaluation configuration without reading secrets."""
    return os.environ.get("SECONDEGO_MODEL", DEFAULT_MODEL)


def configured_presentation_mode() -> PresentationMode:
    """Read the optional event-display mode without reading any secret."""
    value = os.environ.get("SECONDEGO_UI_MODE", PresentationMode.HEADLESS.value)
    try:
        return PresentationMode(value.strip().lower())
    except ValueError as exc:
        allowed = ", ".join(mode.value for mode in PresentationMode)
        raise ValueError(f"SECONDEGO_UI_MODE must be one of: {allowed}") from exc
