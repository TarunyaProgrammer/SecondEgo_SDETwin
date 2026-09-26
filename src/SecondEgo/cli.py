import argparse
import asyncio
import json
import sys
from pathlib import Path
from typing import Any

from SecondEgo.app import build_engine, build_model_planner, build_resources
from SecondEgo.config import ProviderKind, configured_model
from SecondEgo.core.events import EngineEvent
from SecondEgo.core.state import AcceptanceCriterion
from SecondEgo.model.base import ActionProposal
from SecondEgo.lifecycle import collect_garbage
from SecondEgo.orchestration.engine import HarnessEngine
from SecondEgo.repository.source import RepositorySourceError, resolve_repository
from SecondEgo.storage.redaction import redact_sensitive
from SecondEgo.verification.contracts import FailureRecord


def main() -> int:
    gc = collect_garbage()
    if gc.deleted:
        print(f"SecondEgo garbage collector reclaimed {gc.deleted} stale temp directories", file=sys.stderr)
    parser = argparse.ArgumentParser(description="Run SecondEgo with an auditable action plan.")
    parser.add_argument("solve", help="literal command: solve")
    parser.add_argument("--repo", required=True, help="target repository path or HTTPS GitHub URL")
    parser.add_argument("--issue", required=True, help="issue description")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--plan", type=Path, help="JSON plan with actions and verification_commands")
    source.add_argument("--provider", choices=[provider.value for provider in ProviderKind], help="request a structured plan from this provider")
    source.add_argument("--gemini", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--model", help="provider model identifier (defaults to the selected provider's evaluation model)")
    parser.add_argument("--max-model-calls", type=int, default=20)
    parser.add_argument("--max-tool-calls", type=int, default=80)
    parser.add_argument("--max-retries", type=int, default=6)
    parser.add_argument("--state-db", type=Path, help="optional SQLite path for run state and evidence")
    parser.add_argument(
        "--ui",
        action="store_true",
        help="stream compact engine events; does not add model calls or tools",
    )
    arguments = parser.parse_args()
    if arguments.solve != "solve":
        parser.error("the first argument must be 'solve'")

    try:
        repository = resolve_repository(arguments.repo)
    except RepositorySourceError as exc:
        parser.error(str(exc))
    if repository.cloned:
        print(f"SecondEgo cloned repository to: {repository.root}", file=sys.stderr)

    try:
        resources = build_resources(
            max_model_calls=arguments.max_model_calls,
            max_tool_calls=arguments.max_tool_calls,
            max_retries=arguments.max_retries,
        )
        engine = build_engine(
            repository.root,
            resources=resources,
            state_db=arguments.state_db,
            event_sink=_print_event if arguments.ui else None,
        )
        if arguments.plan:
            plan = _load_plan(arguments.plan)
            result = engine.run(
                task=arguments.issue,
                acceptance_criteria=tuple(
                    AcceptanceCriterion(description=item) for item in plan.get("acceptance_criteria", [])
                ),
                actions=tuple(_proposal(item) for item in plan.get("actions", [])),
                verification_commands=tuple(
                    tuple(command) for command in plan.get("verification_commands", [])
                ),
                recovery_actions=tuple(_proposal(item) for item in plan.get("recovery_actions", [])),
            )
        else:
            provider_kind = ProviderKind("gemini" if arguments.gemini else arguments.provider)
            planner = build_model_planner(resources, arguments.model or configured_model(provider_kind), provider_kind)
            result = asyncio.run(
                engine.run_with_planner(
                    task=arguments.issue,
                    acceptance_criteria=(),
                    planner=planner,
                )
            )
        print(json.dumps(_report(result), indent=2, default=str))
        return 0 if result.verification.passed else 1
    finally:
        repository.cleanup()


def _load_plan(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise SystemExit(f"invalid plan file: {exc}") from exc
    if not isinstance(data, dict):
        raise SystemExit("plan root must be a JSON object")
    return data


def _proposal(value: object) -> ActionProposal:
    if not isinstance(value, dict):
        raise SystemExit("each action must be an object")
    action = value.get("action")
    arguments = value.get("arguments", {})
    rationale = value.get("rationale", "")
    if not isinstance(action, str) or not isinstance(arguments, dict) or not isinstance(rationale, str):
        raise SystemExit("action requires string action/rationale and object arguments")
    return ActionProposal(action=action, arguments=arguments, rationale=rationale)


def _report(result: object) -> dict[str, object]:
    state = result.state
    return {
        "run_id": state.run_id,
        "status": state.status.value,
        "phase": state.phase.value,
        "termination_reason": state.termination_reason,
        "changed_paths": sorted(state.changed_paths),
        "resource_usage": state.resource_usage,
        "verification": {
            "passed": result.verification.passed,
            "failure_class": result.verification.failure_class.value,
            "failure_summary": result.verification.failure_summary,
            "commands": list(result.verification.commands),
            "failure_record": _failure_report(result.verification.failure_record),
        },
        "events": [event.to_dict() for event in result.events],
        "evidence": [
            {
                "reference": item.reference,
                "summary": redact_sensitive(item.summary),
                "source": item.source,
                "importance": item.importance,
            }
            for item in result.evidence
        ],
    }


def _failure_report(record: FailureRecord | None) -> dict[str, object] | None:
    if record is None:
        return None
    return {
        "failure_class": record.failure_class.value,
        "summary": redact_sensitive(record.summary),
        "failing_tests": list(record.failing_tests),
        "error_locations": list(record.error_locations),
        "fingerprint": redact_sensitive(record.fingerprint or ""),
    }


def _print_event(event: EngineEvent) -> None:
    """Render safe, compact progress without exposing model/tool output."""
    status = f" status={event.status}" if event.status else ""
    print(f"[{event.phase}] {event.event_type}{status}", flush=True)


if __name__ == "__main__":
    raise SystemExit(main())
