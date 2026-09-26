import os


DEFAULT_MODEL = "gemini-3.8-flash"


def configured_model() -> str:
    """Return the model selected by evaluation configuration without reading secrets."""
    return os.environ.get("SECONDEGO_MODEL", DEFAULT_MODEL)
