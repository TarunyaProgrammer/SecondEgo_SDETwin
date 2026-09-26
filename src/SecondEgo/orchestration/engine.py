from dataclasses import dataclass
from uuid import uuid4

from SecondEgo.context.ledger import EvidenceLedger
from SecondEgo.context.policy import EvidenceRecord
from SecondEgo.core.resources import ResourceLimitExceeded, ResourceUsage
from SecondEgo.core.state import AcceptanceCriterion, ExecutionState, Phase, TerminalStatus
from SecondEgo.core.events import EngineEvent, EventLog, EventSink
from SecondEgo.core.state_machine import StateMachine
from SecondEgo.model.base import ActionProposal
from SecondEgo.model.planner import ModelCallTelemetry, ModelPlanner, PlanValidationError
from SecondEgo.repository.index import RepositoryIndexer
from SecondEgo.repository.index import RepositoryIndex
from SecondEgo.repository.retrieval import RepositoryRetriever
from SecondEgo.repository.scanner import RepositoryScanner
from SecondEgo.storage.sqlite import SQLiteRunStore
from SecondEgo.tools.router import ToolRouter
from SecondEgo.tools.git import GitTool
from SecondEgo.tools.filesystem import FileTool
from SecondEgo.tools.policy import WorkspacePolicy
from SecondEgo.tools.runner import CommandRunner
from SecondEgo.tools.search import SearchTool
from SecondEgo.tools.transaction import GitAttemptTransaction, TransactionBlocked
from SecondEgo.storage.redaction import redact_sensitive
from SecondEgo.verification.contracts import FailureClass, VerificationResult
from SecondEgo.verification.verifier import VerificationEngine


@dataclass(frozen=True)
class EngineResult:
    state: ExecutionState
    verification: VerificationResult
    events: tuple[EngineEvent, ...]
    evidence: tuple[EvidenceRecord, ...]


@dataclass
class _PreparedRun:
    state: ExecutionState
    machine: StateMachine
    ledger: EvidenceLedger
    events: list[EngineEvent]
    index: RepositoryIndex


class HarnessEngine:
    """Headless, evidence-first agent execution skeleton with bounded recovery."""

    def __init__(
        self,
        *,
        scanner: RepositoryScanner,
        indexer: RepositoryIndexer | None = None,
        retriever: RepositoryRetriever | None = None,
        router: ToolRouter,
        verifier: VerificationEngine,
        resources: ResourceUsage,
        store: SQLiteRunStore | None = None,
        git: GitTool | None = None,
        transaction: GitAttemptTransaction | None = None,
        event_sink: EventSink | None = None,
    ) -> None:
        self.scanner = scanner
        self.indexer = indexer or RepositoryIndexer(scanner.workspace, scanner=scanner)
        self.retriever = retriever or RepositoryRetriever()
        self.router = router
        self.verifier = verifier
        self.resources = resources
        self.store = store
        self.git = git or GitTool(router.runner)
        self.transaction = transaction
        self.event_sink = event_sink

    def run(
        self,
        *,
        task: str,
        acceptance_criteria: tuple[AcceptanceCriterion, ...],
        actions: tuple[ActionProposal, ...],
        verification_commands: tuple[tuple[str, ...], ...],
        recovery_actions: tuple[ActionProposal, ...] = (),
    ) -> EngineResult:
        prepared = self._prepare_run(task, acceptance_criteria)
        prepared.events.append(
            prepared.machine.move(Phase.PLAN, reason="actions supplied by planner")[1]
        )
        return self._execute_plan(
            prepared,
            actions=actions,
            verification_commands=verification_commands,
            recovery_actions=recovery_actions,
        )

    async def run_with_planner(
        self,
        *,
        task: str,
        acceptance_criteria: tuple[AcceptanceCriterion, ...],
        planner: ModelPlanner,
    ) -> EngineResult:
        prepared = self._prepare_run(task, acceptance_criteria)
        prepared.events.append(
            prepared.machine.move(Phase.PLAN, reason="request structured model plan")[1]
        )
        try:
            plan = await planner.create_plan(prepared.state, prepared.ledger.active())
        except ResourceLimitExceeded as exc:
            verification = VerificationResult(
                passed=False,
                failure_class=FailureClass.ENVIRONMENT_FAILURE,
                failure_summary=str(exc),
            )
            return self._finalize(
                prepared,
                verification=verification,
                terminal_status=TerminalStatus.BLOCKED,
                terminal_reason=str(exc),
            )
        except Exception as exc:
            verification = VerificationResult(
                passed=False,
                failure_class=FailureClass.MODEL_PLANNING_FAILURE,
                failure_summary=f"{type(exc).__name__}: {str(exc)[:500]}",
            )
            return self._finalize(
                prepared,
                verification=verification,
                terminal_status=TerminalStatus.FAILED,
                terminal_reason=str(exc),
            )
        finally:
            self._record_model_context(planner, prepared)
        return await self._execute_planned_with_dynamic_recovery(
            prepared,
            actions=plan.actions,
            verification_commands=plan.verification_commands,
            planner=planner,
        )

    def _prepare_run(
        self,
        task: str,
        acceptance_criteria: tuple[AcceptanceCriterion, ...],
    ) -> _PreparedRun:
        state = ExecutionState(
            run_id=str(uuid4()),
            task=task,
            workspace=str(self.scanner.workspace.root),
            acceptance_criteria=list(acceptance_criteria),
        )
        machine = StateMachine(state)
        ledger = EvidenceLedger()
        events = EventLog(self.event_sink)
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
        self._record_retrieval(
            task,
            index,
            ledger,
            state,
            prefix="retrieval",
            mode="issue",
        )
        return _PreparedRun(
            state=state,
            machine=machine,
            ledger=ledger,
            events=events,
            index=index,
        )

    def _record_retrieval(
        self,
        query: str,
        index: RepositoryIndex,
        ledger: EvidenceLedger,
        state: ExecutionState,
        *,
        prefix: str,
        mode: str,
    ) -> None:
        ranked = self.retriever.rank(index, query, limit=8, mode=mode)
        for number, item in enumerate(ranked):
            summary = (
                f"path={item.path} score={item.score} confidence={item.confidence:.2f} "
                f"reasons={list(item.reasons)}"
            )
            if _is_safe_context_path(item.path):
                excerpt = self.router.files.read(item.path)
                if excerpt.success and excerpt.stdout:
                    summary += f" excerpt={redact_sensitive(excerpt.stdout[:2_500])}"
            reference = f"{prefix}:{number}"
            ledger.record(
                EvidenceRecord(
                    reference=reference,
                    summary=summary,
                    source=item.path,
                    importance=4 if mode == "failure" else 3,
                )
            )
            state.evidence_refs.append(reference)

    async def _execute_planned_with_dynamic_recovery(
        self,
        prepared: _PreparedRun,
        *,
        actions: tuple[ActionProposal, ...],
        verification_commands: tuple[tuple[str, ...], ...],
        planner: ModelPlanner,
    ) -> EngineResult:
        """Execute a model plan, then request repair only after observing failure evidence."""
        state = prepared.state
        machine = prepared.machine
        ledger = prepared.ledger
        events = prepared.events
        terminal_evidence_refs: tuple[str, ...] = ()
        attempt_active = False
        verification = VerificationResult(
            passed=False,
            failure_class=FailureClass.TOOL_FAILURE,
            failure_summary="execution did not reach verification",
        )
        try:
            active_router, active_verifier = self._begin_attempt(state, events)
            attempt_active = self.transaction is not None
            events.append(machine.move(Phase.EXECUTE, reason="dispatch planned actions")[1])
            self._execute_actions(
                actions,
                ledger,
                state,
                events,
                router=active_router,
                track_changed_paths=self.transaction is None,
            )
            events.append(machine.move(Phase.VERIFY, reason="run verification commands")[1])
            verification, _ = self._verify(
                verification_commands,
                ledger,
                state,
                events,
                verifier=active_verifier,
            )
            if verification.passed:
                self._finish_attempt(
                    passed=True,
                    ledger=ledger,
                    state=state,
                    events=events,
                )
                attempt_active = False
                return self._finalize(
                    prepared,
                    verification=verification,
                    terminal_status=TerminalStatus.COMPLETE,
                    terminal_reason="verification passed",
                    terminal_evidence_refs=verification.evidence_refs,
                )

            self._finish_attempt(
                passed=False,
                ledger=ledger,
                state=state,
                events=events,
            )
            attempt_active = False
            events.append(
                machine.move(
                    Phase.DIAGNOSE,
                    reason=verification.failure_summary or "verification failed",
                )[1]
            )
            ledger.record(
                EvidenceRecord(
                    reference="diagnosis:failure",
                    summary=verification.failure_summary or "verification failed",
                    source="verification failure record",
                    importance=5,
                )
            )
            state.evidence_refs.append("diagnosis:failure")
            failure_record = verification.failure_record
            diagnosis_query = " ".join(
                (list(failure_record.failing_tests) if failure_record is not None else [])
                + (list(failure_record.error_locations) if failure_record is not None else [])
                + [(verification.failure_summary or "")[:500]]
            )
            self._record_retrieval(
                diagnosis_query,
                prepared.index,
                ledger,
                state,
                prefix="diagnosis:retrieval",
                mode="failure",
            )
            self.resources.record_retry()
            try:
                recovery_actions = await planner.create_recovery_actions(
                    state,
                    ledger.active(),
                    verification,
                )
            except ResourceLimitExceeded as exc:
                return self._finalize(
                    prepared,
                    verification=VerificationResult(
                        passed=False,
                        failure_class=FailureClass.ENVIRONMENT_FAILURE,
                        failure_summary=str(exc),
                    ),
                    terminal_status=TerminalStatus.BLOCKED,
                    terminal_reason=str(exc),
                )
            except (PlanValidationError, RuntimeError) as exc:
                return self._finalize(
                    prepared,
                    verification=VerificationResult(
                        passed=False,
                        failure_class=FailureClass.MODEL_PLANNING_FAILURE,
                        failure_summary=str(exc),
                    ),
                    terminal_status=TerminalStatus.FAILED,
                    terminal_reason=str(exc),
                )
            finally:
                self._record_model_context(planner, prepared)

            events.append(machine.move(Phase.RECOVER, reason="apply diagnosis-informed repair plan")[1])
            active_router, active_verifier = self._begin_attempt(state, events)
            attempt_active = self.transaction is not None
            events.append(machine.move(Phase.EXECUTE, reason="dispatch repair actions")[1])
            self._execute_actions(
                recovery_actions,
                ledger,
                state,
                events,
                router=active_router,
                track_changed_paths=self.transaction is None,
            )
            events.append(machine.move(Phase.VERIFY, reason="verify repair plan")[1])
            verification, _ = self._verify(
                verification_commands,
                ledger,
                state,
                events,
                verifier=active_verifier,
            )
            if verification.passed:
                terminal_status = TerminalStatus.COMPLETE
                terminal_reason = "verification passed after diagnosis-informed recovery"
                terminal_evidence_refs = verification.evidence_refs
            else:
                terminal_status = TerminalStatus.FAILED
                terminal_reason = verification.failure_summary or "verification failed after recovery"
            self._finish_attempt(
                passed=verification.passed,
                ledger=ledger,
                state=state,
                events=events,
            )
            attempt_active = False
        except ResourceLimitExceeded as exc:
            if attempt_active and self.transaction is not None:
                self.transaction.abort()
            terminal_status = TerminalStatus.BLOCKED
            terminal_reason = str(exc)
            verification = VerificationResult(
                passed=False,
                failure_class=FailureClass.ENVIRONMENT_FAILURE,
                failure_summary=str(exc),
            )
        except TransactionBlocked as exc:
            if attempt_active and self.transaction is not None:
                self.transaction.abort()
            terminal_status = TerminalStatus.BLOCKED
            terminal_reason = str(exc)
            verification = VerificationResult(
                passed=False,
                failure_class=FailureClass.ENVIRONMENT_FAILURE,
                failure_summary=str(exc),
            )
        except RuntimeError as exc:
            if attempt_active and self.transaction is not None:
                self.transaction.abort()
            terminal_status = TerminalStatus.FAILED
            terminal_reason = str(exc)
            verification = VerificationResult(
                passed=False,
                failure_class=FailureClass.TOOL_FAILURE,
                failure_summary=str(exc),
            )
        return self._finalize(
            prepared,
            verification=verification,
            terminal_status=terminal_status,
            terminal_reason=terminal_reason,
            terminal_evidence_refs=terminal_evidence_refs,
        )

    def _execute_plan(
        self,
        prepared: _PreparedRun,
        *,
        actions: tuple[ActionProposal, ...],
        verification_commands: tuple[tuple[str, ...], ...],
        recovery_actions: tuple[ActionProposal, ...],
    ) -> EngineResult:
        state = prepared.state
        machine = prepared.machine
        ledger = prepared.ledger
        events = prepared.events
        terminal_status: TerminalStatus
        terminal_reason: str
        terminal_evidence_refs: tuple[str, ...] = ()
        attempt_active = False
        active_router = self.router
        active_verifier = self.verifier
        try:
            active_router, active_verifier = self._begin_attempt(state, events)
            attempt_active = self.transaction is not None
            events.append(machine.move(Phase.EXECUTE, reason="dispatch planned actions")[1])
            self._execute_actions(
                actions,
                ledger,
                state,
                events,
                router=active_router,
                track_changed_paths=self.transaction is None,
            )
            events.append(machine.move(Phase.VERIFY, reason="run verification commands")[1])
            verification, _ = self._verify(
                verification_commands,
                ledger,
                state,
                events,
                verifier=active_verifier,
            )
            if verification.passed:
                terminal_status = TerminalStatus.COMPLETE
                terminal_reason = "verification passed"
                terminal_evidence_refs = verification.evidence_refs
                self._finish_attempt(
                    passed=True,
                    ledger=ledger,
                    state=state,
                    events=events,
                )
                attempt_active = False
            elif recovery_actions:
                self._finish_attempt(
                    passed=False,
                    ledger=ledger,
                    state=state,
                    events=events,
                )
                attempt_active = False
                events.append(machine.move(Phase.DIAGNOSE, reason=verification.failure_summary or "verification failed")[1])
                self.resources.record_retry()
                events.append(machine.move(Phase.RECOVER, reason="recovery actions supplied")[1])
                active_router, active_verifier = self._begin_attempt(state, events)
                attempt_active = self.transaction is not None
                events.append(machine.move(Phase.EXECUTE, reason="dispatch recovery actions")[1])
                self._execute_actions(
                    recovery_actions,
                    ledger,
                    state,
                    events,
                    router=active_router,
                    track_changed_paths=self.transaction is None,
                )
                events.append(machine.move(Phase.VERIFY, reason="rerun verification commands")[1])
                verification, _ = self._verify(
                    verification_commands,
                    ledger,
                    state,
                    events,
                    verifier=active_verifier,
                )
                if verification.passed:
                    terminal_status = TerminalStatus.COMPLETE
                    terminal_reason = "verification passed after recovery"
                    terminal_evidence_refs = verification.evidence_refs
                    self._finish_attempt(
                        passed=True,
                        ledger=ledger,
                        state=state,
                        events=events,
                    )
                    attempt_active = False
                else:
                    terminal_status = TerminalStatus.FAILED
                    terminal_reason = verification.failure_summary or "verification failed after recovery"
                    self._finish_attempt(
                        passed=False,
                        ledger=ledger,
                        state=state,
                        events=events,
                    )
                    attempt_active = False
            else:
                terminal_status = TerminalStatus.FAILED
                terminal_reason = verification.failure_summary or "verification failed"
                self._finish_attempt(
                    passed=False,
                    ledger=ledger,
                    state=state,
                    events=events,
                )
                attempt_active = False
        except ResourceLimitExceeded as exc:
            if attempt_active and self.transaction is not None:
                self.transaction.abort()
            terminal_status = TerminalStatus.BLOCKED
            terminal_reason = str(exc)
            verification = VerificationResult(
                passed=False,
                failure_class=FailureClass.ENVIRONMENT_FAILURE,
                failure_summary=str(exc),
            )
        except TransactionBlocked as exc:
            if attempt_active and self.transaction is not None:
                self.transaction.abort()
            terminal_status = TerminalStatus.BLOCKED
            terminal_reason = str(exc)
            verification = VerificationResult(
                passed=False,
                failure_class=FailureClass.ENVIRONMENT_FAILURE,
                failure_summary=str(exc),
            )
        except RuntimeError as exc:
            if attempt_active and self.transaction is not None:
                self.transaction.abort()
            terminal_status = TerminalStatus.FAILED
            terminal_reason = str(exc)
            verification = VerificationResult(
                passed=False,
                failure_class=FailureClass.TOOL_FAILURE,
                failure_summary=str(exc),
            )
        return self._finalize(
            prepared,
            verification=verification,
            terminal_status=terminal_status,
            terminal_reason=terminal_reason,
            terminal_evidence_refs=terminal_evidence_refs,
        )

    def _begin_attempt(
        self,
        state: ExecutionState,
        events: list[EngineEvent],
    ) -> tuple[ToolRouter, VerificationEngine]:
        if self.transaction is None:
            return self.router, self.verifier
        attempt_workspace = self.transaction.begin()
        router, verifier = self._runtime_for(attempt_workspace)
        events.append(
            EngineEvent(
                run_id=state.run_id,
                event_type="transaction.started",
                phase=state.phase.value,
                status=state.status.value,
                payload={"isolated": True},
            )
        )
        return router, verifier

    def _finish_attempt(
        self,
        *,
        passed: bool,
        ledger: EvidenceLedger,
        state: ExecutionState,
        events: list[EngineEvent],
    ) -> None:
        if self.transaction is None:
            return
        result = self.transaction.finish(passed=passed)
        if result.transferred:
            state.changed_paths.update(result.changed_paths)
        reference = f"transaction:{'passed' if passed else 'discarded'}"
        ledger.record(
            EvidenceRecord(
                reference=reference,
                summary=result.reason,
                source="git worktree transaction",
                importance=4,
            )
        )
        state.evidence_refs.append(reference)
        events.append(
            EngineEvent(
                run_id=state.run_id,
                event_type="transaction.finished",
                phase=state.phase.value,
                status=state.status.value,
                evidence_ref=reference,
                payload={
                    "passed": result.passed,
                    "transferred": result.transferred,
                    "changed_paths": list(result.changed_paths),
                },
            )
        )

    def _runtime_for(
        self,
        workspace: WorkspacePolicy,
    ) -> tuple[ToolRouter, VerificationEngine]:
        runner = CommandRunner(workspace, self.router.runner.command_policy)
        router = ToolRouter(
            files=FileTool(workspace, max_file_bytes=self.router.files.max_file_bytes),
            search=SearchTool(workspace, max_results=self.router.search.max_results),
            runner=runner,
            resources=self.resources,
            git=GitTool(runner),
        )
        return router, VerificationEngine(runner)

    def _finalize(
        self,
        prepared: _PreparedRun,
        *,
        verification: VerificationResult,
        terminal_status: TerminalStatus,
        terminal_reason: str,
        terminal_evidence_refs: tuple[str, ...] = (),
    ) -> EngineResult:
        state = prepared.state
        machine = prepared.machine
        ledger = prepared.ledger
        events = prepared.events
        self._collect_final_diff(ledger, state, events)
        state.resource_usage = self.resources.snapshot()
        events.append(
            EngineEvent(
                run_id=state.run_id,
                event_type="run.resources",
                phase=state.phase.value,
                status=state.status.value,
                payload=state.resource_usage,
            )
        )
        events.append(
            machine.terminate(
                terminal_status,
                reason=terminal_reason,
                evidence_refs=terminal_evidence_refs,
            )
        )
        result = EngineResult(
            state=state,
            verification=verification,
            events=tuple(events),
            evidence=tuple(ledger.active()),
        )
        if self.store is not None:
            self.store.save_run(result.state, result.events, result.evidence)
        return result

    def _record_model_context(
        self,
        planner: ModelPlanner,
        prepared: _PreparedRun,
    ) -> None:
        telemetry = planner.last_call
        if telemetry is None:
            return
        reference = f"model:context:{telemetry.call_type}"
        prepared.ledger.record(
            EvidenceRecord(
                reference=reference,
                summary=(
                    f"call_type={telemetry.call_type} estimated_tokens={telemetry.estimated_tokens} "
                    f"slot_usage={telemetry.slot_usage} dropped_evidence={list(telemetry.dropped_evidence)}"
                ),
                source="context assembler",
                importance=3,
            )
        )
        prepared.state.evidence_refs.append(reference)
        prepared.events.append(
            EngineEvent(
                run_id=prepared.state.run_id,
                event_type="model.context_prepared",
                phase=prepared.state.phase.value,
                status=prepared.state.status.value,
                evidence_ref=reference,
                payload={
                    "call_type": telemetry.call_type,
                    "estimated_tokens": telemetry.estimated_tokens,
                    "slot_usage": telemetry.slot_usage,
                    "dropped_evidence": list(telemetry.dropped_evidence),
                },
            )
        )

    def _execute_actions(
        self,
        actions: tuple[ActionProposal, ...],
        ledger: EvidenceLedger,
        state: ExecutionState,
        events: list[EngineEvent],
        *,
        router: ToolRouter,
        track_changed_paths: bool,
    ) -> None:
        for index, proposal in enumerate(actions):
            result = router.dispatch(proposal)
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
            if track_changed_paths:
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
        *,
        verifier: VerificationEngine,
    ) -> tuple[VerificationResult, tuple[object, ...]]:
        result, evidence = verifier.run(commands)
        for index, item in enumerate(evidence):
            ledger.record(
                EvidenceRecord(
                    reference=f"verification:{index}",
                    summary=item.output or "verification command passed",
                    source=" ".join(item.command),
                    importance=4 if not item.success else 3,
                )
            )
        if result.failure_record is not None:
            failure = result.failure_record
            ledger.record(
                EvidenceRecord(
                    reference="verification:failure",
                    summary=(
                        f"class={failure.failure_class.value} "
                        f"tests={list(failure.failing_tests)} "
                        f"locations={list(failure.error_locations)} "
                        f"fingerprint={failure.fingerprint or 'none'}"
                    ),
                    source="verification failure parser",
                    importance=5,
                )
            )
            state.evidence_refs.append("verification:failure")
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


def _is_safe_context_path(path: str) -> bool:
    name = path.casefold().rsplit("/", 1)[-1]
    return name not in {".env", ".env.local", ".env.production", "credentials.json"}
