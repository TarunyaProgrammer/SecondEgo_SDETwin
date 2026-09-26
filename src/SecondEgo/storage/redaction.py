import re


_SECRET_PATTERN = re.compile(
    r"(?i)(\b(?:(?:[a-z0-9]+[_-])?api[_-]?key|access[_-]?token|password|secret|credential)\b\s*[:=]\s*)([^\s,;]+)"
)


def redact_sensitive(value: str) -> str:
    """Remove common credential assignments from bounded telemetry text."""
    return _SECRET_PATTERN.sub(r"\1[REDACTED]", value)
