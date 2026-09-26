from dataclasses import dataclass, field

from .policy import ContextBudget, EvidenceRecord


@dataclass(frozen=True)
class ContextPacket:
    task: str
    action: str
    evidence: tuple[EvidenceRecord, ...]
    state: str
    estimated_tokens: int
    dropped_evidence: tuple[str, ...] = ()
    slot_usage: dict[str, int] = field(default_factory=dict)

    def as_text(self) -> str:
        evidence_text = "\n".join(
            f"[{item.reference}] {item.summary} (source: {item.source})"
            for item in self.evidence
        )
        return (
            f"TASK:\n{self.task}\n\n"
            f"ACTION:\n{self.action}\n\n"
            f"EVIDENCE:\n{evidence_text}\n\n"
            f"OMITTED_EVIDENCE:\n{list(self.dropped_evidence)}\n\n"
            f"STATE:\n{self.state}"
        )


class ContextAssembler:
    """Build context in priority order without silently truncating required data."""

    def __init__(self, budget: ContextBudget) -> None:
        self.budget = budget

    @staticmethod
    def estimate_tokens(value: str) -> int:
        # Conservative deterministic estimate until provider token counting is available.
        return max(1, (len(value) + 3) // 4)

    def assemble(
        self,
        *,
        task: str,
        action: str,
        evidence: list[EvidenceRecord],
        state: str,
    ) -> ContextPacket:
        task_tokens = self.estimate_tokens(task)
        action_tokens = self.estimate_tokens(action)
        state_tokens = self.estimate_tokens(state)
        if task_tokens > self.budget.task_tokens:
            raise ValueError("task exceeds its reserved context budget")
        if action_tokens > self.budget.action_tokens:
            raise ValueError("action exceeds its reserved context budget")
        if state_tokens > self.budget.state_tokens:
            raise ValueError("state exceeds its reserved context budget")

        selected: list[EvidenceRecord] = []
        dropped: list[str] = []
        used_evidence_tokens = 0
        for item in sorted(evidence, key=lambda value: value.importance, reverse=True):
            if item.stale:
                dropped.append(item.reference)
                continue
            item_tokens = self.estimate_tokens(item.summary)
            if used_evidence_tokens + item_tokens > self.budget.evidence_tokens:
                dropped.append(item.reference)
                continue
            selected.append(item)
            used_evidence_tokens += item_tokens

        estimated = task_tokens + action_tokens + state_tokens + used_evidence_tokens
        if estimated > self.budget.total_tokens - self.budget.response_tokens:
            raise ValueError("context leaves insufficient response budget")
        return ContextPacket(
            task=task,
            action=action,
            evidence=tuple(selected),
            state=state,
            estimated_tokens=estimated,
            dropped_evidence=tuple(dropped),
            slot_usage={
                "task": task_tokens,
                "action": action_tokens,
                "evidence": used_evidence_tokens,
                "state": state_tokens,
                "response_reserved": self.budget.response_tokens,
            },
        )
