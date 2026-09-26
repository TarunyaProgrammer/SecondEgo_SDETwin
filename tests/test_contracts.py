import pytest

from SecondEgo.context.assembler import ContextAssembler
from SecondEgo.context.ledger import EvidenceLedger
from SecondEgo.context.policy import ContextBudget, ContextPolicy, EvidenceRecord
from SecondEgo.core.resources import ResourceBudget, ResourceLimitExceeded, ResourceUsage
from SecondEgo.core.state import AcceptanceCriterion, ExecutionState, Phase, TerminalStatus
from SecondEgo.core.state_machine import InvalidTransition, StateMachine
from SecondEgo.tools.filesystem import FileTool
from SecondEgo.tools.policy import CommandPolicy, PolicyViolation, WorkspacePolicy
from SecondEgo.tools.router import ToolRouter
from SecondEgo.tools.runner import CommandRunner
from SecondEgo.tools.search import SearchTool
from SecondEgo.model.base import ActionProposal
from SecondEgo.orchestration.engine import HarnessEngine
from SecondEgo.repository.scanner import RepositoryScanner
from SecondEgo.verification.contracts import FailureClass, VerificationResult
from SecondEgo.verification.verifier import VerificationEngine
from SecondEgo.cli import _proposal


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


def test_evidence_ledger_retains_source_linked_active_evidence() -> None:
    ledger = EvidenceLedger()
    ledger.record(EvidenceRecord("test-1", "one failure", "tests/auth.py:17", importance=3))
    ledger.record(EvidenceRecord("old", "stale output", "stdout", importance=1))
    ledger.mark_stale("old")

    assert [record.reference for record in ledger.active()] == ["test-1"]
    assert ledger.snapshot()[1]["stale"] is True


def test_resource_usage_rejects_budget_overrun_before_recording() -> None:
    usage = ResourceUsage(ResourceBudget(max_model_calls=1, max_tool_calls=1, max_retries=1))
    usage.record_model_call(10)
    usage.record_tool_call()
    usage.record_retry()

    with pytest.raises(ResourceLimitExceeded, match="model-call"):
        usage.record_model_call(10)
    with pytest.raises(ResourceLimitExceeded, match="tool-call"):
        usage.record_tool_call()
    with pytest.raises(ResourceLimitExceeded, match="retry"):
        usage.record_retry()


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


def test_workspace_policy_rejects_paths_outside_root(tmp_path) -> None:
    policy = WorkspacePolicy(tmp_path)

    with pytest.raises(PolicyViolation, match="escapes"):
        policy.resolve_path("../outside.txt")


def test_command_policy_rejects_unapproved_executable() -> None:
    policy = CommandPolicy.default()

    with pytest.raises(PolicyViolation, match="allowlisted"):
        policy.validate(("curl", "https://example.com"), 5)


def test_command_runner_is_bounded_and_workspace_scoped(tmp_path) -> None:
    runner = CommandRunner(WorkspacePolicy(tmp_path))
    result = runner.run(("python3", "-c", "print('ok')"), cwd=".")

    assert result.success is True
    assert result.stdout.strip() == "ok"


def test_file_tool_reads_and_writes_only_workspace_files(tmp_path) -> None:
    files = FileTool(WorkspacePolicy(tmp_path))
    written = files.write("src/example.py", "VALUE = 1\n")
    read = files.read("src/example.py")

    assert written.success is True
    assert written.changed_paths == ("src/example.py",)
    assert read.success is True
    assert read.stdout == "VALUE = 1\n"


def test_search_tool_returns_paths_and_line_numbers(tmp_path) -> None:
    files = FileTool(WorkspacePolicy(tmp_path))
    files.write("src/example.py", "VALUE = 1\nneedle = True\n")
    results = SearchTool(WorkspacePolicy(tmp_path)).text("needle")

    assert results == [{"path": "src/example.py", "line": 2, "text": "needle = True"}]


def test_repository_scanner_discovers_manifests_and_tests(tmp_path) -> None:
    files = FileTool(WorkspacePolicy(tmp_path))
    files.write("pyproject.toml", "[project]\nname = 'fixture'\n")
    files.write("tests/test_feature.py", "def test_feature(): pass\n")
    files.write("src/app.py", "VALUE = 1\n")

    snapshot = RepositoryScanner(WorkspacePolicy(tmp_path)).scan()

    assert snapshot.manifests == ("pyproject.toml",)
    assert snapshot.test_files == ("tests/test_feature.py",)
    assert "src/app.py" in snapshot.files


def test_repository_scanner_ignores_non_code_test_fixtures(tmp_path) -> None:
    files = FileTool(WorkspacePolicy(tmp_path))
    files.write("tests/plan.json", "{}")

    snapshot = RepositoryScanner(WorkspacePolicy(tmp_path)).scan()

    assert snapshot.test_files == ()


def test_verifier_returns_test_failure_evidence(tmp_path) -> None:
    runner = CommandRunner(WorkspacePolicy(tmp_path))
    result, evidence = VerificationEngine(runner).run(
        (("python3", "-c", "raise AssertionError('broken')"),)
    )

    assert result.passed is False
    assert result.failure_class is FailureClass.TEST_FAILURE
    assert evidence[0].success is False


def test_harness_engine_runs_edit_to_verified_completion(tmp_path) -> None:
    workspace = WorkspacePolicy(tmp_path)
    resources = ResourceUsage(ResourceBudget(max_tool_calls=10, max_retries=1))
    runner = CommandRunner(workspace)
    engine = HarnessEngine(
        scanner=RepositoryScanner(workspace),
        router=ToolRouter(
            files=FileTool(workspace),
            search=SearchTool(workspace),
            runner=runner,
            resources=resources,
        ),
        verifier=VerificationEngine(runner),
        resources=resources,
    )

    result = engine.run(
        task="create a version file",
        acceptance_criteria=(AcceptanceCriterion("version file exists"),),
        actions=(ActionProposal("edit_file", {"path": "src/version.py", "content": "VERSION = '1'\n"}),),
        verification_commands=(("python3", "-c", "from pathlib import Path; assert Path('src/version.py').exists()"),),
    )

    assert result.state.status is TerminalStatus.COMPLETE
    assert result.state.phase is Phase.VERIFY
    assert result.verification.passed is True
    assert "src/version.py" in result.state.changed_paths
    assert [event.event_type for event in result.events][-2:] == [
        "verification.completed",
        "run.terminated",
    ]


def test_harness_engine_recovers_from_failed_verification(tmp_path) -> None:
    workspace = WorkspacePolicy(tmp_path)
    resources = ResourceUsage(ResourceBudget(max_tool_calls=10, max_retries=1))
    runner = CommandRunner(workspace)
    engine = HarnessEngine(
        scanner=RepositoryScanner(workspace),
        router=ToolRouter(
            files=FileTool(workspace),
            search=SearchTool(workspace),
            runner=runner,
            resources=resources,
        ),
        verifier=VerificationEngine(runner),
        resources=resources,
    )

    result = engine.run(
        task="write the expected value",
        acceptance_criteria=(AcceptanceCriterion("file contains correct value"),),
        actions=(ActionProposal("edit_file", {"path": "value.txt", "content": "wrong\n"}),),
        verification_commands=(
            ("python3", "-c", "from pathlib import Path; assert Path('value.txt').read_text() == 'right\\n'"),
        ),
        recovery_actions=(ActionProposal("edit_file", {"path": "value.txt", "content": "right\n"}),),
    )

    assert result.state.status is TerminalStatus.COMPLETE
    assert result.verification.passed is True
    assert result.state.resource_usage["retries"] == 1


def test_cli_action_parser_rejects_malformed_plan_actions() -> None:
    with pytest.raises(SystemExit, match="action requires"):
        _proposal({"action": "edit_file", "arguments": []})
