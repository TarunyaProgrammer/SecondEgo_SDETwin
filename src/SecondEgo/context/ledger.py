from dataclasses import replace

from .policy import EvidenceRecord


class EvidenceLedger:
    """Compact, source-linked execution evidence retained across agent phases."""

    def __init__(self) -> None:
        self._records: dict[str, EvidenceRecord] = {}

    def record(self, evidence: EvidenceRecord) -> None:
        if not evidence.reference.strip() or not evidence.source.strip():
            raise ValueError("evidence requires a non-empty reference and source")
        current = self._records.get(evidence.reference)
        if current is None or evidence.importance >= current.importance:
            self._records[evidence.reference] = evidence

    def mark_stale(self, reference: str) -> None:
        if reference in self._records:
            self._records[reference] = replace(self._records[reference], stale=True)

    def active(self) -> list[EvidenceRecord]:
        return sorted(
            (record for record in self._records.values() if not record.stale),
            key=lambda record: (-record.importance, record.reference),
        )

    def snapshot(self) -> list[dict[str, object]]:
        return [
            {
                "reference": record.reference,
                "summary": record.summary,
                "source": record.source,
                "importance": record.importance,
                "stale": record.stale,
            }
            for record in sorted(self._records.values(), key=lambda item: item.reference)
        ]

