from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any


@dataclass(frozen=True)
class EngineEvent:
    run_id: str
    event_type: str
    phase: str
    timestamp: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    status: str | None = None
    evidence_ref: str | None = None
    payload: dict[str, Any] = field(default_factory=dict)

