import pytest
import asyncio
import os
import subprocess
from pathlib import Path

from SecondEgo.context.assembler import ContextAssembler
from SecondEgo.context.ledger import EvidenceLedger
from SecondEgo.context.policy import ContextBudget, ContextPolicy, EvidenceRecord
from SecondEgo.core.resources import ResourceBudget, ResourceLimitExceeded, ResourceUsage
from SecondEgo.core.state import AcceptanceCriterion, ExecutionState, Phase, TerminalStatus
from SecondEgo.core.state_machine import InvalidTransition, StateMachine
from SecondEgo.tools.filesystem import FileTool
from SecondEgo.tools.git import GitTool
from SecondEgo.tools.policy import CommandPolicy, PolicyViolation, WorkspacePolicy
from SecondEgo.tools.router import ToolRouter
from SecondEgo.tools.runner import CommandRunner
from SecondEgo.tools.search import SearchTool
from SecondEgo.tools.transaction import GitAttemptTransaction
from SecondEgo.model.base import ActionProposal
from SecondEgo.model.gemini import GeminiProvider, ProviderConfigurationError
from SecondEgo.model.planner import ModelPlanner, PlanValidationError
from SecondEgo.model.scripted import ScriptedProvider
from SecondEgo.config import DEFAULT_MODEL, configured_model
from SecondEgo.config import PresentationMode, configured_presentation_mode
from SecondEgo.core.events import EngineEvent, EventLog
from SecondEgo.orchestration.engine import HarnessEngine
from SecondEgo.repository.scanner import RepositoryScanner
from SecondEgo.repository.index import RepositoryIndexer
from SecondEgo.repository.retrieval import RepositoryRetriever
from SecondEgo.repository.source import RepositorySourceError, resolve_repository
from SecondEgo.lifecycle import GcConfig, OwnedTempLease, collect_garbage
from SecondEgo.storage.sqlite import SQLiteRunStore
from SecondEgo.storage.redaction import redact_sensitive
from SecondEgo.verification.contracts import FailureClass, VerificationResult
from SecondEgo.verification.verifier import VerificationEngine
from SecondEgo.cli import _proposal
from SecondEgo.desktop.gateway import RunRecord, RunRegistry


def test_execution_state_snapshot_is_structured_and_bounded() -> None:
    state = ExecutionState(run_id="r1", task="fix bug", workspace="/repo")
    state.phase = Phase.VERIFY
    state.status = TerminalStatus.COMPLETE
    state.changed_paths.add("src/app.py")
    snapshot = state.snapshot()

    assert snapshot["phase"] == Phase.VERIFY
    assert snapshot["changed_paths"] == ["src/app.py"]
    assert "transcript" not in snapshot


def test_engine_event_serializes_for_ui_observers() -> None:
    event = EngineEvent(run_id="r1", event_type="state.changed", phase="PLAN")

    payload = event.to_dict()

    assert payload["schema_version"] == 1
    assert payload["run_id"] == "r1"
    assert payload["event_type"] == "state.changed"
    assert payload["timestamp"].endswith("+00:00")


def test_event_sink_is_observational_only() -> None:
    observed: list[str] = []
    log = EventLog(lambda event: observed.append(event.event_type))
    event = EngineEvent(run_id="r1", event_type="run.terminated", phase="VERIFY")

    log.append(event)

    assert tuple(log) == (event,)
    assert observed == ["run.terminated"]


def test_event_sink_failure_does_not_break_engine_history() -> None:
    log = EventLog(lambda event: (_ for _ in ()).throw(RuntimeError("ui offline")))
    event = EngineEvent(run_id="r1", event_type="tool.completed", phase="EXECUTE")

    log.append(event)

    assert log == [event]


def test_gateway_run_record_exposes_incremental_versioned_events() -> None:
    record = RunRecord(request_id="q1", repository="/repo", issue="fix", model="test-model")
    record.add_event(EngineEvent(run_id="r1", event_type="state.changed", phase="PLAN"))

    snapshot = record.snapshot(event_offset=0)

    assert snapshot["status"] == "RUNNING"
    assert snapshot["events"][0]["schema_version"] == 1


def test_gateway_registry_validates_bounded_run_requests(tmp_path) -> None:
    registry = RunRegistry()

    with pytest.raises(ValueError, match="issue"):
        registry.submit(repository=str(tmp_path), issue=" ")
    with pytest.raises(ValueError, match="repository"):
        registry.submit(repository="file:///tmp/not-supported", issue="fix")


def test_repository_source_accepts_local_directory(tmp_path) -> None:
    resolved = resolve_repository(tmp_path)

    assert resolved.root == tmp_path.resolve()
    assert resolved.cloned is False


def test_repository_source_rejects_unsafe_remote_forms() -> None:
    with pytest.raises(RepositorySourceError, match="credentials"):
        resolve_repository("https://user:secret@github.com/org/repo")
    with pytest.raises(RepositorySourceError, match="only HTTPS GitHub"):
        resolve_repository("git@github.com:org/repo.git")


def test_repository_source_shallow_clones_github_url(monkeypatch) -> None:
    import SecondEgo.repository.source as source
    calls = []

    def fake_run(arguments, **kwargs):
        calls.append(arguments)
        target = Path(arguments[-1])
        (target / ".git").mkdir(parents=True)
        return subprocess.CompletedProcess(arguments, 0, "", "")

    monkeypatch.setattr(source.subprocess, "run", fake_run)
    resolved = resolve_repository("https://github.com/example/project")

    assert resolved.cloned is True
    assert resolved.root.name == "project"
    assert "--depth" in calls[0]
    assert "--no-tags" in calls[0]


def test_garbage_collector_reclaims_only_stale_owned_directories(tmp_path) -> None:
    stale = tmp_path / "secondego-remote-stale"
    stale.mkdir()
    (stale / ".secondego-owned").write_text("{}\n", encoding="utf-8")
    (stale / "payload").write_text("clone", encoding="utf-8")
    os.utime(stale, (0, 0))

    unowned = tmp_path / "secondego-remote-unowned"
    unowned.mkdir()
    os.utime(unowned, (0, 0))

    recent = tmp_path / "secondego-remote-recent"
    recent.mkdir()
    (recent / ".secondego-owned").write_text("{}\n", encoding="utf-8")

    report = collect_garbage(GcConfig(temp_root=tmp_path, retention_seconds=1))

    assert report.deleted == 1
    assert report.skipped_unowned == 1
    assert report.skipped_recent == 1
    assert not stale.exists()
    assert unowned.exists()
    assert recent.exists()


def test_garbage_collector_does_not_remove_an_active_lease(tmp_path) -> None:
    active = tmp_path / "secondego-attempt-active"
    active.mkdir()
    lease = OwnedTempLease(active, "attempt")
    os.utime(active, (0, 0))

    report = collect_garbage(GcConfig(temp_root=tmp_path, retention_seconds=0))

    assert report.deleted == 0
    assert report.skipped_active == 1
    assert active.exists()
    lease.cleanup()
    assert not active.exists()


def test_gateway_registry_accepts_valid_request_without_running_model(monkeypatch, tmp_path) -> None:
    monkeypatch.setattr(RunRegistry, "_run", lambda self, record: None)
    registry = RunRegistry()

    record = registry.submit(repository=str(tmp_path), issue="fix the issue", model="test-model")

    assert registry.get(record.request_id) is record
    assert record.status == "QUEUED"
    assert record.model == "test-model"


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
    assert packet.dropped_evidence == ("stale",)
    assert packet.slot_usage["response_reserved"] == 10
    assert "OMITTED_EVIDENCE" in packet.as_text()


def test_evidence_ledger_retains_source_linked_active_evidence() -> None:
    ledger = EvidenceLedger()
    ledger.record(EvidenceRecord("test-1", "one failure", "tests/auth.py:17", importance=3))
    ledger.record(EvidenceRecord("old", "stale output", "stdout", importance=1))
    ledger.mark_stale("old")

    assert [record.reference for record in ledger.active()] == ["test-1"]
    snapshot = {record["reference"]: record for record in ledger.snapshot()}
    assert snapshot["old"]["stale"] is True


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


def test_command_runner_reports_missing_allowlisted_executable(tmp_path) -> None:
    policy = CommandPolicy(frozenset({"definitely-missing"}))
    result = CommandRunner(WorkspacePolicy(tmp_path), policy).run(("definitely-missing",))

    assert result.success is False
    assert "could not start" in result.stderr


def test_telemetry_redacts_common_secret_assignments() -> None:
    value = redact_sensitive("AI_API_KEY=secret-value password: hunter2 normal")

    assert "secret-value" not in value
    assert "hunter2" not in value
    assert "[REDACTED]" in value


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


def test_repository_indexer_records_symbols_imports_and_parse_failures(tmp_path) -> None:
    files = FileTool(WorkspacePolicy(tmp_path))
    files.write("src/auth.py", "import time\n\nclass AuthService:\n    def refresh_token(self):\n        return time.time()\n")
    files.write("src/broken.py", "def incomplete(:\n")

    index = RepositoryIndexer(WorkspacePolicy(tmp_path)).build()

    assert [symbol.name for symbol in index.symbols] == ["AuthService", "refresh_token"]
    assert index.imports[0].module == "time"
    assert index.parser_failures[0].path == "src/broken.py"


def test_repository_indexer_builds_test_topology_from_python_imports(tmp_path) -> None:
    files = FileTool(WorkspacePolicy(tmp_path))
    files.write("src/auth.py", "def refresh_token():\n    return None\n")
    files.write(
        "tests/test_auth.py",
        "from src.auth import refresh_token\n\ndef test_refresh_token():\n    assert refresh_token() is None\n",
    )

    index = RepositoryIndexer(WorkspacePolicy(tmp_path)).build()

    assert [(item.path, item.name) for item in index.tests] == [
        ("tests/test_auth.py", "test_refresh_token")
    ]
    assert index.test_links[0].test_path == "tests/test_auth.py"
    assert index.test_links[0].target_path == "src/auth.py"
    assert index.test_links[0].confidence == 0.95


def test_repository_retriever_ranks_symbol_matches_above_path_matches(tmp_path) -> None:
    files = FileTool(WorkspacePolicy(tmp_path))
    files.write("src/auth.py", "def refresh_token():\n    return None\n")
    files.write("src/token_notes.py", "VALUE = 1\n")
    index = RepositoryIndexer(WorkspacePolicy(tmp_path)).build()

    ranked = RepositoryRetriever().rank(index, "refresh token bug")

    assert ranked[0].path == "src/auth.py"
    assert "symbol:refresh" in ranked[0].reasons


def test_repository_retriever_links_failure_test_to_implementation(tmp_path) -> None:
    files = FileTool(WorkspacePolicy(tmp_path))
    files.write("src/auth.py", "def refresh_token():\n    return None\n")
    files.write(
        "tests/test_auth.py",
        "from src.auth import refresh_token\n\ndef test_refresh_token():\n    assert refresh_token() is None\n",
    )
    index = RepositoryIndexer(WorkspacePolicy(tmp_path)).build()

    ranked = RepositoryRetriever().rank(
        index,
        "tests/test_auth.py::test_refresh_token failed",
        mode="failure",
    )

    assert ranked[0].path == "tests/test_auth.py"
    implementation = next(item for item in ranked if item.path == "src/auth.py")
    assert "failure-linked-test" in implementation.reasons
    assert implementation.confidence == 0.95


def test_engine_feeds_ranked_repository_evidence_into_run_ledger(tmp_path) -> None:
    files = FileTool(WorkspacePolicy(tmp_path))
    files.write("src/auth.py", "def refresh_token():\n    return None\n")
    files.write("tests/test_auth.py", "from src.auth import refresh_token\n")
    workspace = WorkspacePolicy(tmp_path)
    resources = ResourceUsage(ResourceBudget(max_tool_calls=10))
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
        task="fix refresh_token behavior",
        acceptance_criteria=(),
        actions=(),
        verification_commands=(("python3", "-c", "assert True"),),
    )

    retrieval = [item for item in result.evidence if item.reference.startswith("retrieval:")]
    assert retrieval
    assert any(item.source == "src/auth.py" for item in retrieval)
    assert any("def refresh_token" in item.summary for item in retrieval)


def test_verifier_returns_test_failure_evidence(tmp_path) -> None:
    runner = CommandRunner(WorkspacePolicy(tmp_path))
    result, evidence = VerificationEngine(runner).run(
        (("python3", "-c", "raise AssertionError('broken')"),)
    )

    assert result.passed is False
    assert result.failure_class is FailureClass.TEST_FAILURE
    assert evidence[0].success is False


def test_verifier_extracts_structured_failure_record(tmp_path) -> None:
    runner = CommandRunner(WorkspacePolicy(tmp_path))
    result, _ = VerificationEngine(runner).run(
        ((
            "python3",
            "-c",
            "print('FAILED tests/test_auth.py::test_login - AssertionError at tests/auth.py:17'); raise SystemExit(1)",
        ),)
    )

    assert result.failure_record is not None
    assert result.failure_record.failing_tests == ("tests/test_auth.py::test_login",)
    assert result.failure_record.error_locations == ("tests/auth.py:17",)


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
    assert any(event.event_type == "run.resources" for event in result.events)
    repository_evidence = next(item for item in result.evidence if item.reference == "repository:scan")
    assert "symbols=" in repository_evidence.summary
    assert [event.event_type for event in result.events][-3:] == [
        "git.diff_collected",
        "run.resources",
        "run.terminated",
    ]
    assert any(record.reference == "diff:final" for record in result.evidence)


def test_git_tool_reports_status_from_workspace(tmp_path) -> None:
    workspace = WorkspacePolicy(tmp_path)
    runner = CommandRunner(workspace)
    assert runner.run(("git", "init")).success is True
    FileTool(workspace).write("new.py", "VALUE = 1\n")

    result = GitTool(runner).status()

    assert result.success is True
    assert "?? new.py" in result.stdout


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


def _git_fixture(tmp_path):
    root = tmp_path / "evaluation-repo"
    root.mkdir()
    subprocess.run(("git", "init", "-q"), cwd=root, check=True)
    subprocess.run(("git", "config", "user.email", "test@example.com"), cwd=root, check=True)
    subprocess.run(("git", "config", "user.name", "SecondEgo Test"), cwd=root, check=True)
    FileTool(WorkspacePolicy(root)).write("value.txt", "baseline\n")
    subprocess.run(("git", "add", "value.txt"), cwd=root, check=True)
    subprocess.run(("git", "-c", "commit.gpgSign=false", "commit", "-qm", "baseline"), cwd=root, check=True)
    return root


def test_git_transaction_discards_failed_attempt_without_touching_target(tmp_path) -> None:
    root = _git_fixture(tmp_path)
    base = WorkspacePolicy(root)
    transaction = GitAttemptTransaction(base)

    attempt = transaction.begin()
    FileTool(attempt).write("value.txt", "failed\n")
    result = transaction.finish(passed=False)

    assert result.transferred is False
    assert (root / "value.txt").read_text() == "baseline\n"
    assert transaction.active is False


def test_git_transaction_transfers_verified_tracked_and_new_files(tmp_path) -> None:
    root = _git_fixture(tmp_path)
    base = WorkspacePolicy(root)
    transaction = GitAttemptTransaction(base)

    attempt = transaction.begin()
    FileTool(attempt).write("value.txt", "verified\n")
    FileTool(attempt).write("src/new.py", "VALUE = 1\n")
    result = transaction.finish(passed=True)

    assert result.transferred is True
    assert set(result.changed_paths) == {"src/new.py", "value.txt"}
    assert (root / "value.txt").read_text() == "verified\n"
    assert (root / "src/new.py").read_text() == "VALUE = 1\n"


def test_transactional_engine_discards_failed_attempt_before_recovery(tmp_path) -> None:
    root = _git_fixture(tmp_path)
    workspace = WorkspacePolicy(root)
    resources = ResourceUsage(ResourceBudget(max_tool_calls=20, max_retries=1))
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
        transaction=GitAttemptTransaction(workspace),
    )

    result = engine.run(
        task="write the verified value",
        acceptance_criteria=(AcceptanceCriterion("value is verified"),),
        actions=(ActionProposal("edit_file", {"path": "value.txt", "content": "wrong\n"}),),
        verification_commands=(("python3", "-c", "from pathlib import Path; assert Path('value.txt').read_text() == 'right\\n'"),),
        recovery_actions=(ActionProposal("edit_file", {"path": "value.txt", "content": "right\n"}),),
    )

    assert result.state.status is TerminalStatus.COMPLETE
    assert (root / "value.txt").read_text() == "right\n"
    transaction_events = [event for event in result.events if event.event_type == "transaction.finished"]
    assert [event.payload["transferred"] for event in transaction_events] == [False, True]


def test_sqlite_store_persists_run_events_and_evidence(tmp_path) -> None:
    root = tmp_path / "repo"
    root.mkdir()
    workspace = WorkspacePolicy(root)
    resources = ResourceUsage(ResourceBudget(max_tool_calls=10))
    runner = CommandRunner(workspace)
    store = SQLiteRunStore(tmp_path / "state" / "runs.db")
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
        store=store,
    )

    result = engine.run(
        task="persist evidence",
        acceptance_criteria=(),
        actions=(),
        verification_commands=(("python3", "-c", "assert True"),),
    )
    loaded = store.load_run(result.state.run_id)

    assert loaded is not None
    assert loaded["status"] == "COMPLETE"
    assert loaded["events"][-1]["event_type"] == "run.terminated"
    assert "repository:scan" in {record["reference"] for record in loaded["evidence"]}


def test_model_planner_validates_bounded_structured_plan() -> None:
    state = ExecutionState(run_id="model-run", task="write a version file", workspace="/repo")
    provider = ScriptedProvider(
        [
            ActionProposal(
                "submit_plan",
                {
                    "actions": [
                        {
                            "action": "edit_file",
                            "arguments": {"path": "src/version.py", "content": "VERSION = '1'\n"},
                            "rationale": "create requested file",
                        }
                    ],
                    "verification_commands": [["python3", "-c", "assert True"]],
                    "recovery_actions": [],
                },
            )
        ]
    )
    resources = ResourceUsage(ResourceBudget(max_model_calls=1))
    planner = ModelPlanner(
        provider=provider,
        assembler=ContextAssembler(ContextBudget(24_000, 4_000, 2_000, 12_000, 3_000, 3_000)),
        resources=resources,
    )

    plan = asyncio.run(
        planner.create_plan(state, [EvidenceRecord("e1", "target file", "src/version.py", importance=3)])
    )

    assert plan.actions[0].action == "edit_file"
    assert plan.verification_commands == (("python3", "-c", "assert True"),)
    assert resources.model_calls == 1


def test_model_planner_rejects_non_plan_action() -> None:
    with pytest.raises(PlanValidationError, match="submit_plan"):
        from SecondEgo.model.planner import _parse_plan

        _parse_plan(ActionProposal("edit_file", {"path": "x"}))


def test_harness_engine_executes_validated_provider_plan(tmp_path) -> None:
    workspace = WorkspacePolicy(tmp_path)
    resources = ResourceUsage(ResourceBudget(max_model_calls=1, max_tool_calls=10))
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
    planner = ModelPlanner(
        provider=ScriptedProvider(
            [
                ActionProposal(
                    "submit_plan",
                    {
                        "actions": [
                            {
                                "action": "edit_file",
                                "arguments": {"path": "provider.txt", "content": "planned\n"},
                                "rationale": "create the requested file",
                            }
                        ],
                        "verification_commands": [
                            ["python3", "-c", "from pathlib import Path; assert Path('provider.txt').exists()"]
                        ],
                    },
                )
            ]
        ),
        assembler=ContextAssembler(ContextBudget(24_000, 4_000, 2_000, 12_000, 3_000, 3_000)),
        resources=resources,
    )

    result = asyncio.run(
        engine.run_with_planner(
            task="create provider file",
            acceptance_criteria=(AcceptanceCriterion("provider file exists"),),
            planner=planner,
        )
    )

    assert result.state.status is TerminalStatus.COMPLETE
    assert result.state.resource_usage["model_calls"] == 1
    assert "provider.txt" in result.state.changed_paths


def test_harness_engine_converts_provider_transport_error_to_terminal_evidence(tmp_path) -> None:
    class FailingProvider:
        async def generate(self, prompt: str, *, context: dict[str, object]) -> ActionProposal:
            raise ConnectionError("network details must not escape the run")

        async def count_tokens(self, text: str) -> int:
            return 1

    workspace = WorkspacePolicy(tmp_path)
    resources = ResourceUsage(ResourceBudget(max_model_calls=1, max_tool_calls=5))
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
    planner = ModelPlanner(
        provider=FailingProvider(),
        assembler=ContextAssembler(ContextBudget(24_000, 4_000, 2_000, 12_000, 3_000, 3_000)),
        resources=resources,
    )

    result = asyncio.run(
        engine.run_with_planner(
            task="provider failure test",
            acceptance_criteria=(),
            planner=planner,
        )
    )

    assert result.state.status is TerminalStatus.FAILED
    assert result.verification.failure_class is FailureClass.MODEL_PLANNING_FAILURE
    assert "ConnectionError" in (result.verification.failure_summary or "")


def test_planner_requests_repair_after_observed_failure(tmp_path) -> None:
    root = _git_fixture(tmp_path)
    workspace = WorkspacePolicy(root)
    resources = ResourceUsage(
        ResourceBudget(max_model_calls=2, max_tool_calls=20, max_retries=1)
    )
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
        transaction=GitAttemptTransaction(workspace),
    )
    planner = ModelPlanner(
        provider=ScriptedProvider(
            [
                ActionProposal(
                    "submit_plan",
                    {
                        "actions": [
                            {
                                "action": "edit_file",
                                "arguments": {"path": "value.txt", "content": "wrong\n"},
                                "rationale": "initial hypothesis",
                            }
                        ],
                        "verification_commands": [
                            [
                                "python3",
                                "-c",
                                "from pathlib import Path; assert Path('value.txt').read_text() == 'right\\n'",
                            ]
                        ],
                    },
                ),
                ActionProposal(
                    "submit_repair_plan",
                    {
                        "actions": [
                            {
                                "action": "edit_file",
                                "arguments": {"path": "value.txt", "content": "right\n"},
                                "rationale": "repair the value identified by the failed assertion",
                            }
                        ]
                    },
                ),
            ]
        ),
        assembler=ContextAssembler(ContextBudget(24_000, 4_000, 2_000, 12_000, 3_000, 3_000)),
        resources=resources,
    )

    result = asyncio.run(
        engine.run_with_planner(
            task="write the verified value",
            acceptance_criteria=(AcceptanceCriterion("value is right"),),
            planner=planner,
        )
    )

    assert result.state.status is TerminalStatus.COMPLETE
    assert result.state.resource_usage["model_calls"] == 2
    assert "diagnosis:failure" in {record.reference for record in result.evidence}
    assert [
        event.payload["call_type"]
        for event in result.events
        if event.event_type == "model.context_prepared"
    ] == ["plan", "recovery"]
    assert (root / "value.txt").read_text() == "right\n"
    assert any(
        event.event_type == "state.changed" and event.payload.get("reason") == "apply diagnosis-informed repair plan"
        for event in result.events
    )


def test_gemini_provider_requires_explicit_key_before_sdk_import(monkeypatch) -> None:
    monkeypatch.delenv("AI_API_KEY", raising=False)
    provider = GeminiProvider(api_key="")

    with pytest.raises(ProviderConfigurationError, match="AI_API_KEY"):
        asyncio.run(provider.generate("plan", context={}))


def test_gemini_provider_reads_evaluator_credential_name(monkeypatch) -> None:
    monkeypatch.setenv("AI_API_KEY", "evaluation-key")

    assert GeminiProvider()._resolve_api_key() == "evaluation-key"


def test_model_name_can_be_overridden_for_prescribed_evaluation_model(monkeypatch) -> None:
    monkeypatch.delenv("SECONDEGO_MODEL", raising=False)
    assert configured_model() == DEFAULT_MODEL
    monkeypatch.setenv("SECONDEGO_MODEL", "prescribed-text-model")
    assert configured_model() == "prescribed-text-model"


def test_presentation_mode_defaults_to_headless(monkeypatch) -> None:
    monkeypatch.delenv("SECONDEGO_UI_MODE", raising=False)

    assert configured_presentation_mode() is PresentationMode.HEADLESS


def test_presentation_mode_can_enable_event_display(monkeypatch) -> None:
    monkeypatch.setenv("SECONDEGO_UI_MODE", "events")

    assert configured_presentation_mode() is PresentationMode.EVENTS


def test_cli_action_parser_rejects_malformed_plan_actions() -> None:
    with pytest.raises(SystemExit, match="action requires"):
        _proposal({"action": "edit_file", "arguments": []})
