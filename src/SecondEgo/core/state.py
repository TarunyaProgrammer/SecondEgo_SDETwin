from dataclasses import dataclass, field
from enum import StrEnum
class Phase(StrEnum):
    INITIALIZE = "INITIALIZE"
    UNDERSTAND = "UNDERSTAND"
    EXPLORE = "EXPLORE"
    PLAN = "PLAN"
    EXECUTE = "EXECUTE"
    VERIFY = "VERIFY"
    DIAGNOSE = "DIAGNOSE"
    RECOVER = "RECOVER"


class TerminalStatus(StrEnum):
    RUNNING = "RUNNING"
    COMPLETE = "COMPLETE"
    FAILED = "FAILED"
    BLOCKED = "BLOCKED"
    CANCELLED = "CANCELLED"


@dataclass(frozen=True)
class AcceptanceCriterion:
    description: str
    evidence_required: bool = True


@dataclass
class ExecutionState:
    run_id: str
    task: str
    workspace: str
    acceptance_criteria: list[AcceptanceCriterion] = field(default_factory=list)
    phase: Phase = Phase.INITIALIZE
    status: TerminalStatus = TerminalStatus.RUNNING
    current_step: str | None = None
    changed_paths: set[str] = field(default_factory=set)
    facts: dict[str, str] = field(default_factory=dict)
    decisions: list[str] = field(default_factory=list)
    open_questions: list[str] = field(default_factory=list)
    failed_approaches: list[str] = field(default_factory=list)
    retry_counts: dict[str, int] = field(default_factory=dict)
    resource_usage: dict[str, float | int] = field(default_factory=dict)
    evidence_refs: list[str] = field(default_factory=list)
    termination_reason: str | None = None

    @property
    def is_terminal(self) -> bool:
        return self.status is not TerminalStatus.RUNNING

    def snapshot(self) -> dict[str, object]:
        """Return bounded structured state; raw transcripts do not belong here."""
        return {
            "run_id": self.run_id,
            "task": self.task,
            "workspace": self.workspace,
            "phase": self.phase,
            "status": self.status,
            "current_step": self.current_step,
            "changed_paths": sorted(self.changed_paths),
            "facts": dict(self.facts),
            "decisions": list(self.decisions),
            "open_questions": list(self.open_questions),
            "failed_approaches": list(self.failed_approaches),
            "retry_counts": dict(self.retry_counts),
            "resource_usage": dict(self.resource_usage),
            "evidence_refs": list(self.evidence_refs),
            "termination_reason": self.termination_reason,
        }
