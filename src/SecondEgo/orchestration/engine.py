from dataclasses import dataclass
from pathlib import Path
from uuid import uuid4

from SecondEgo.context.ledger import EvidenceLedger
from SecondEgo.context.policy import EvidenceRecord
from SecondEgo.core.resources import ResourceLimitExceeded, ResourceUsage
from SecondEgo.core.state import AcceptanceCriterion, ExecutionState, Phase, TerminalStatus
from SecondEgo.core.events import EngineEvent
from SecondEgo.core.state_machine import StateMachine
from SecondEgo.model.base import ActionProposal
from SecondEgo.repository.index import RepositoryIndexer
from SecondEgo.repository.scanner import RepositoryScanner
from SecondEgo.storage.sqlite import SQLiteRunStore
from SecondEgo.tools.router import ToolRouter
from SecondEgo.tools.git import GitTool
from SecondEgo.verification.contracts import FailureClass, VerificationResult
from SecondEgo.verification.verifier import VerificationEngine


@dataclass(frozen=True)
class EngineResult:
    state: ExecutionState
    verification: VerificationResult
    events: tuple[EngineEvent, ...]
    evidence: tuple[EvidenceRecord, ...]


class HarnessEngine:
    """Headless, evidence-first agent execution skeleton with bounded recovery."""

    def __init__(
        self,
        *,
        scanner: RepositoryScanner,
        indexer: RepositoryIndexer | None = None,
        router: ToolRouter,
        verifier: VerificationEngine,
        resources: ResourceUsage,
        store: SQLiteRunStore | None = None,
        git: GitTool | None = None,
    ) -> None:
        self.scanner = scanner
        self.indexer = indexer or RepositoryIndexer(scanner.workspace, scanner=scanner)
        self.router = router
        self.verifier = verifier
        self.resources = resources
        self.store = store
        self.git = git or GitTool(router.runner)

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
        events: list[EngineEvent] = []
        terminal_status: TerminalStatus
        terminal_reason: str
        terminal_evidence_refs: tuple[str, ...] = ()
        try:
            events.append(machine.move(Phase.UNDERSTAND, reason="task accepted")[1])
            events.append(machine.move(Phase.EXPLORE, reason="structural repository scan")[1])
            index = self.indexer.build()
            snapshot = index.snapshot
            ledger.record(
                EvidenceRecord(
                    reference="repository:scan",
                    summary=(
                        f"files={len(snapshot.files)} manifests={list(snapshot.manifests)} "
                        f"tests={list(snapshot.test_files)} symbols={len(index.symbols)} "
                        f"imports={len(index.imports)} parser_failures={len(index.parser_failures)}"
                    ),
                    source=snapshot.root,
                    importance=3,
                )
            )
            events.append(machine.move(Phase.PLAN, reason="actions supplied by planner")[1])
            events.append(machine.move(Phase.EXECUTE, reason="dispatch planned actions")[1])
            self._execute_actions(actions, ledger, state, events)
            events.append(machine.move(Phase.VERIFY, reason="run verification commands")[1])
            verification, _ = self._verify(verification_commands, ledger, state, events)
            if verification.passed:
                terminal_status = TerminalStatus.COMPLETE
                terminal_reason = "verification passed"
                terminal_evidence_refs = verification.evidence_refs
            elif recovery_actions:
                events.append(machine.move(Phase.DIAGNOSE, reason=verification.failure_summary or "verification failed")[1])
                self.resources.record_retry()
                events.append(machine.move(Phase.RECOVER, reason="recovery actions supplied")[1])
                events.append(machine.move(Phase.EXECUTE, reason="dispatch recovery actions")[1])
                self._execute_actions(recovery_actions, ledger, state, events)
                events.append(machine.move(Phase.VERIFY, reason="rerun verification commands")[1])
                verification, _ = self._verify(verification_commands, ledger, state, events)
                if verification.passed:
                    terminal_status = TerminalStatus.COMPLETE
                    terminal_reason = "verification passed after recovery"
                    terminal_evidence_refs = verification.evidence_refs
                else:
                    terminal_status = TerminalStatus.FAILED
                    terminal_reason = verification.failure_summary or "verification failed after recovery"
            else:
                terminal_status = TerminalStatus.FAILED
                terminal_reason = verification.failure_summary or "verification failed"
        except ResourceLimitExceeded as exc:
            terminal_status = TerminalStatus.BLOCKED
            terminal_reason = str(exc)
            verification = VerificationResult(
                passed=False,
                failure_class=FailureClass.ENVIRONMENT_FAILURE,
                failure_summary=str(exc),
            )
        except RuntimeError as exc:
            terminal_status = TerminalStatus.FAILED
            terminal_reason = str(exc)
            verification = VerificationResult(
                passed=False,
                failure_class=FailureClass.TOOL_FAILURE,
                failure_summary=str(exc),
            )
        self._collect_final_diff(ledger, state, events)
        events.append(
            machine.terminate(
                terminal_status,
                reason=terminal_reason,
                evidence_refs=terminal_evidence_refs,
            )
        )
        state.resource_usage = self.resources.snapshot()
        result = EngineResult(
            state=state,
            verification=verification,
            events=tuple(events),
            evidence=tuple(ledger.active()),
        )
        if self.store is not None:
            self.store.save_run(result.state, result.events, result.evidence)
        return result

    def _execute_actions(
        self,
        actions: tuple[ActionProposal, ...],
        ledger: EvidenceLedger,
        state: ExecutionState,
        events: list[EngineEvent],
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
            events.append(
                EngineEvent(
                    run_id=state.run_id,
                    event_type="tool.completed",
                    phase=state.phase.value,
                    status=state.status.value,
                    evidence_ref=reference,
                    payload={
                        "tool": proposal.action,
                        "success": result.success,
                        "exit_code": result.exit_code,
                        "changed_paths": list(result.changed_paths),
                        "duration_ms": result.duration_ms,
                    },
                )
            )
            if not result.success:
                raise RuntimeError(f"tool action failed: {proposal.action}: {result.stderr}")

    def _verify(
        self,
        commands: tuple[tuple[str, ...], ...],
        ledger: EvidenceLedger,
        state: ExecutionState,
        events: list[EngineEvent],
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
        events.append(
            EngineEvent(
                run_id=state.run_id,
                event_type="verification.completed",
                phase=state.phase.value,
                status=state.status.value,
                payload={
                    "passed": result.passed,
                    "failure_class": result.failure_class.value,
                    "commands": list(result.commands),
                },
            )
        )
        return result, evidence

    def _collect_final_diff(
        self,
        ledger: EvidenceLedger,
        state: ExecutionState,
        events: list[EngineEvent],
    ) -> None:
        try:
            self.resources.record_tool_call()
        except ResourceLimitExceeded as exc:
            ledger.record(
                EvidenceRecord(
                    reference="diff:final",
                    summary=f"final diff unavailable: {exc}",
                    source="git diff",
                    importance=3,
                )
            )
            return
        result = self.git.diff()
        summary = result.stdout[:4_000] if result.success else result.stderr[:1_000]
        ledger.record(
            EvidenceRecord(
                reference="diff:final",
                summary=summary or "no tracked diff",
                source="git diff --no-ext-diff --unified=3",
                importance=4,
            )
        )
        events.append(
            EngineEvent(
                run_id=state.run_id,
                event_type="git.diff_collected",
                phase=state.phase.value,
                status=state.status.value,
                evidence_ref="diff:final",
                payload={
                    "success": result.success,
                    "exit_code": result.exit_code,
                    "duration_ms": result.duration_ms,
                    "truncated": result.truncated,
                },
            )
        )
