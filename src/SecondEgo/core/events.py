from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, Callable


EventSink = Callable[["EngineEvent"], None]
EVENT_SCHEMA_VERSION = 1


@dataclass(frozen=True)
class EngineEvent:
    run_id: str
    event_type: str
    phase: str
    timestamp: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    status: str | None = None
    evidence_ref: str | None = None
    payload: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        """Return the versioned, JSON-safe event shape exposed to observers."""
        return {
            "schema_version": EVENT_SCHEMA_VERSION,
            "run_id": self.run_id,
            "event_type": self.event_type,
            "phase": self.phase,
            "timestamp": self.timestamp.isoformat(),
            "status": self.status,
            "evidence_ref": self.evidence_ref,
            "payload": self.payload,
        }


class EventLog(list[EngineEvent]):
    """In-memory event history with an optional non-authoritative observer."""

    def __init__(self, sink: EventSink | None = None) -> None:
        super().__init__()
        self.sink = sink

    def append(self, event: EngineEvent) -> None:
        super().append(event)
        if self.sink is None:
            return
        try:
            self.sink(event)
        except Exception:
            # A presentation/telemetry consumer must never change engine behavior.
            return
