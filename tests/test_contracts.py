import pytest

from SecondEgo.context.assembler import ContextAssembler
from SecondEgo.context.policy import ContextBudget, ContextPolicy, EvidenceRecord
from SecondEgo.core.state import ExecutionState, Phase, TerminalStatus
from SecondEgo.core.state_machine import InvalidTransition, StateMachine
from SecondEgo.verification.contracts import FailureClass, VerificationResult


def test_execution_state_snapshot_is_structured_and_bounded() -> None:
    state = ExecutionState(run_id="r1", task="fix bug", workspace="/repo")
    state.phase = Phase.VERIFY
    state.status = TerminalStatus.COMPLETE
    state.changed_paths.add("src/app.py")
    snapshot = state.snapshot()

    assert snapshot["phase"] == Phase.VERIFY
    assert snapshot["changed_paths"] == ["src/app.py"]
    assert "transcript" not in snapshot


def test_context_budget_rejects_overflow() -> None:
    budget = ContextBudget(100, 20, 10, 40, 10, 20)
    policy = ContextPolicy(budget)

    with pytest.raises(ValueError, match="exceeds"):
        policy.reject_if_over_budget(101)


def test_retry_retains_current_evidence_and_drops_stale() -> None:
    policy = ContextPolicy(ContextBudget(100, 20, 10, 40, 10, 20))
    evidence = [
        EvidenceRecord("e1", "failure", "test.py:10", importance=3),
        EvidenceRecord("e2", "old noise", "stdout", stale=True),
    ]

    retained = policy.retain_for_retry(evidence)
    assert [item.reference for item in retained] == ["e1"]


def test_context_assembler_prioritizes_evidence_and_skips_stale() -> None:
    budget = ContextBudget(100, 10, 10, 20, 10, 10)
    assembler = ContextAssembler(budget)
    packet = assembler.assemble(
        task="fix bug",
        action="inspect file",
        state="phase=EXPLORE",
        evidence=[
            EvidenceRecord("low", "low priority", "a.py", importance=1),
            EvidenceRecord("high", "high priority", "b.py", importance=3),
            EvidenceRecord("stale", "old output", "stdout", importance=5, stale=True),
        ],
    )

    assert [item.reference for item in packet.evidence] == ["high", "low"]
    assert packet.estimated_tokens <= 90


def test_failed_verification_has_explicit_failure_class() -> None:
    result = VerificationResult(
        passed=False,
        failed_tests=1,
        failure_class=FailureClass.TEST_FAILURE,
        failure_summary="assertion failed",
    )

    assert result.failure_class is FailureClass.TEST_FAILURE


def test_state_machine_emits_transition_event() -> None:
    machine = StateMachine(ExecutionState(run_id="r1", task="fix", workspace="/repo"))

    transition, event = machine.move(Phase.UNDERSTAND, reason="task accepted")

    assert transition.current is Phase.UNDERSTAND
    assert event.event_type == "state.changed"
    assert event.payload["previous_phase"] == "INITIALIZE"


def test_state_machine_rejects_invalid_transition() -> None:
    machine = StateMachine(ExecutionState(run_id="r1", task="fix", workspace="/repo"))

    with pytest.raises(InvalidTransition):
        machine.move(Phase.EXECUTE, reason="skip required phases")


def test_state_machine_protects_terminal_state() -> None:
    machine = StateMachine(ExecutionState(run_id="r1", task="fix", workspace="/repo"))
    machine.terminate(TerminalStatus.BLOCKED, reason="missing test command")

    with pytest.raises(InvalidTransition):
        machine.move(Phase.UNDERSTAND, reason="should not run")
