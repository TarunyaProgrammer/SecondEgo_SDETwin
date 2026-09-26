from dataclasses import dataclass


@dataclass(frozen=True)
class ContextBudget:
    total_tokens: int
    task_tokens: int
    action_tokens: int
    evidence_tokens: int
    state_tokens: int
    response_tokens: int

    def __post_init__(self) -> None:
        values = (
            self.total_tokens,
            self.task_tokens,
            self.action_tokens,
            self.evidence_tokens,
            self.state_tokens,
            self.response_tokens,
        )
        if any(value < 0 for value in values):
            raise ValueError("context budget values cannot be negative")
        reserved = sum(values[1:])
        if reserved > self.total_tokens:
            raise ValueError("reserved context budget exceeds total budget")


@dataclass(frozen=True)
class EvidenceRecord:
    reference: str
    summary: str
    source: str
    importance: int = 1
    stale: bool = False


class ContextPolicy:
    """Retention rules for bounded context; raw transcripts are intentionally excluded."""

    def __init__(self, budget: ContextBudget) -> None:
        self.budget = budget

    def retain_for_retry(self, evidence: list[EvidenceRecord]) -> list[EvidenceRecord]:
        return [item for item in evidence if not item.stale and item.importance > 0]

    def reject_if_over_budget(self, estimated_tokens: int) -> None:
        if estimated_tokens > self.budget.total_tokens:
            raise ValueError("context exceeds configured budget; compress before sending")

