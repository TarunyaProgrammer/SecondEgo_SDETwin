from dataclasses import dataclass

from .events import EngineEvent
from .state import ExecutionState, Phase, TerminalStatus


class InvalidTransition(ValueError):
    """Raised when the engine attempts an unsupported state transition."""


_ALLOWED_TRANSITIONS: dict[Phase, frozenset[Phase]] = {
    Phase.INITIALIZE: frozenset({Phase.UNDERSTAND}),
    Phase.UNDERSTAND: frozenset({Phase.EXPLORE, Phase.PLAN}),
    Phase.EXPLORE: frozenset({Phase.PLAN}),
    Phase.PLAN: frozenset({Phase.EXECUTE}),
    Phase.EXECUTE: frozenset({Phase.VERIFY}),
    Phase.VERIFY: frozenset({Phase.DIAGNOSE}),
    Phase.DIAGNOSE: frozenset({Phase.RECOVER}),
    Phase.RECOVER: frozenset({Phase.EXECUTE}),
}


@dataclass(frozen=True)
class Transition:
    previous: Phase
    current: Phase
    reason: str


class StateMachine:
    def __init__(self, state: ExecutionState) -> None:
        self.state = state
        self.transitions: list[Transition] = []

    def move(self, phase: Phase, *, reason: str) -> tuple[Transition, EngineEvent]:
        if self.state.is_terminal:
            raise InvalidTransition("terminal execution cannot transition")
        if phase not in _ALLOWED_TRANSITIONS.get(self.state.phase, frozenset()):
            raise InvalidTransition(
                f"cannot transition from {self.state.phase} to {phase}"
            )
        if not reason.strip():
            raise ValueError("transition reason cannot be empty")

        previous = self.state.phase
        self.state.phase = phase
        transition = Transition(previous=previous, current=phase, reason=reason)
        self.transitions.append(transition)
        event = EngineEvent(
            run_id=self.state.run_id,
            event_type="state.changed",
            phase=phase.value,
            status=self.state.status.value,
            payload={
                "previous_phase": previous.value,
                "current_phase": phase.value,
                "reason": reason,
            },
        )
        return transition, event

    def terminate(
        self,
        status: TerminalStatus,
        *,
        reason: str,
        evidence_refs: tuple[str, ...] = (),
    ) -> EngineEvent:
        if status is TerminalStatus.RUNNING:
            raise ValueError("termination status must be terminal")
        if self.state.is_terminal:
            raise InvalidTransition("execution is already terminal")
        if not reason.strip():
            raise ValueError("termination reason cannot be empty")

        self.state.status = status
        self.state.termination_reason = reason
        self.state.evidence_refs.extend(evidence_refs)
        return EngineEvent(
            run_id=self.state.run_id,
            event_type="run.terminated",
            phase=self.state.phase.value,
            status=status.value,
            payload={
                "reason": reason,
                "evidence_refs": list(evidence_refs),
            },
        )

