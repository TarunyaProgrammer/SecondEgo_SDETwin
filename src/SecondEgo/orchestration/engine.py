from dataclasses import dataclass
from pathlib import Path
from uuid import uuid4

from SecondEgo.context.ledger import EvidenceLedger
from SecondEgo.context.policy import EvidenceRecord
from SecondEgo.core.resources import ResourceLimitExceeded, ResourceUsage
from SecondEgo.core.state import AcceptanceCriterion, ExecutionState, Phase, TerminalStatus
from SecondEgo.core.state_machine import StateMachine
from SecondEgo.model.base import ActionProposal
from SecondEgo.repository.scanner import RepositoryScanner
from SecondEgo.tools.router import ToolRouter
from SecondEgo.verification.contracts import FailureClass, VerificationResult
from SecondEgo.verification.verifier import VerificationEngine


@dataclass(frozen=True)
class EngineResult:
    state: ExecutionState
    verification: VerificationResult
    events: tuple[object, ...]
    evidence: tuple[EvidenceRecord, ...]


class HarnessEngine:
    """Headless, evidence-first agent execution skeleton with bounded recovery."""

    def __init__(
        self,
        *,
        scanner: RepositoryScanner,
        router: ToolRouter,
        verifier: VerificationEngine,
        resources: ResourceUsage,
    ) -> None:
        self.scanner = scanner
        self.router = router
        self.verifier = verifier
        self.resources = resources

    def run(
        self,
        *,
        task: str,
        acceptance_criteria: tuple[AcceptanceCriterion, ...],
        actions: tuple[ActionProposal, ...],
        verification_commands: tuple[tuple[str, ...], ...],
        recovery_actions: tuple[ActionProposal, ...] = (),
    ) -> EngineResult:
        state = ExecutionState(
            run_id=str(uuid4()),
            task=task,
            workspace=str(self.scanner.workspace.root),
            acceptance_criteria=list(acceptance_criteria),
        )
        machine = StateMachine(state)
        ledger = EvidenceLedger()
        events: list[object] = []
        try:
            events.append(machine.move(Phase.UNDERSTAND, reason="task accepted")[1])
            events.append(machine.move(Phase.EXPLORE, reason="structural repository scan")[1])
            snapshot = self.scanner.scan()
            ledger.record(
                EvidenceRecord(
                    reference="repository:scan",
                    summary=(
                        f"files={len(snapshot.files)} manifests={list(snapshot.manifests)} "
                        f"tests={list(snapshot.test_files)}"
                    ),
                    source=snapshot.root,
                    importance=3,
                )
            )
            events.append(machine.move(Phase.PLAN, reason="actions supplied by planner")[1])
            events.append(machine.move(Phase.EXECUTE, reason="dispatch planned actions")[1])
            self._execute_actions(actions, ledger, state)
            events.append(machine.move(Phase.VERIFY, reason="run verification commands")[1])
            verification, _ = self._verify(verification_commands, ledger)
            if verification.passed:
                events.append(machine.terminate(
                    TerminalStatus.COMPLETE,
                    reason="verification passed",
                    evidence_refs=verification.evidence_refs,
                ))
            elif recovery_actions:
                events.append(machine.move(Phase.DIAGNOSE, reason=verification.failure_summary or "verification failed")[1])
                self.resources.record_retry()
                events.append(machine.move(Phase.RECOVER, reason="recovery actions supplied")[1])
                events.append(machine.move(Phase.EXECUTE, reason="dispatch recovery actions")[1])
                self._execute_actions(recovery_actions, ledger, state)
                events.append(machine.move(Phase.VERIFY, reason="rerun verification commands")[1])
                verification, _ = self._verify(verification_commands, ledger)
                if verification.passed:
                    events.append(machine.terminate(
                        TerminalStatus.COMPLETE,
                        reason="verification passed after recovery",
                        evidence_refs=verification.evidence_refs,
                    ))
                else:
                    events.append(machine.terminate(
                        TerminalStatus.FAILED,
                        reason=verification.failure_summary or "verification failed after recovery",
                    ))
            else:
                events.append(machine.terminate(
                    TerminalStatus.FAILED,
                    reason=verification.failure_summary or "verification failed",
                ))
        except ResourceLimitExceeded as exc:
            events.append(machine.terminate(TerminalStatus.BLOCKED, reason=str(exc)))
            verification = VerificationResult(
                passed=False,
                failure_class=FailureClass.ENVIRONMENT_FAILURE,
                failure_summary=str(exc),
            )
        except RuntimeError as exc:
            events.append(machine.terminate(TerminalStatus.FAILED, reason=str(exc)))
            verification = VerificationResult(
                passed=False,
                failure_class=FailureClass.TOOL_FAILURE,
                failure_summary=str(exc),
            )
        state.resource_usage = self.resources.snapshot()
        return EngineResult(
            state=state,
            verification=verification,
            events=tuple(events),
            evidence=tuple(ledger.active()),
        )

    def _execute_actions(
        self,
        actions: tuple[ActionProposal, ...],
        ledger: EvidenceLedger,
        state: ExecutionState,
    ) -> None:
        for index, proposal in enumerate(actions):
            result = self.router.dispatch(proposal)
            reference = f"tool:{index}:{proposal.action}"
            summary = result.stderr if not result.success else result.stdout[:1_000]
            ledger.record(
                EvidenceRecord(
                    reference=reference,
                    summary=summary or f"{proposal.action} completed",
                    source=proposal.action,
                    importance=3 if not result.success else 2,
                )
            )
            state.changed_paths.update(result.changed_paths)
            state.evidence_refs.append(reference)
            if not result.success:
                raise RuntimeError(f"tool action failed: {proposal.action}: {result.stderr}")

    def _verify(
        self,
        commands: tuple[tuple[str, ...], ...],
        ledger: EvidenceLedger,
    ) -> tuple[VerificationResult, tuple[object, ...]]:
        result, evidence = self.verifier.run(commands)
        for index, item in enumerate(evidence):
            ledger.record(
                EvidenceRecord(
                    reference=f"verification:{index}",
                    summary=item.output or "verification command passed",
                    source=" ".join(item.command),
                    importance=4 if not item.success else 3,
                )
            )
        return result, evidence
