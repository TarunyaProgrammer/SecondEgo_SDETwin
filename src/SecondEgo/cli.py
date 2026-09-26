import argparse
import asyncio
import json
from pathlib import Path
from typing import Any

from SecondEgo.context.assembler import ContextAssembler
from SecondEgo.context.policy import ContextBudget
from SecondEgo.core.resources import ResourceBudget, ResourceUsage
from SecondEgo.core.state import AcceptanceCriterion
from SecondEgo.model.base import ActionProposal
from SecondEgo.model.gemini import GeminiProvider
from SecondEgo.model.planner import ModelPlanner
from SecondEgo.orchestration.engine import HarnessEngine
from SecondEgo.repository.scanner import RepositoryScanner
from SecondEgo.tools.filesystem import FileTool
from SecondEgo.tools.git import GitTool
from SecondEgo.tools.policy import WorkspacePolicy
from SecondEgo.tools.router import ToolRouter
from SecondEgo.tools.runner import CommandRunner
from SecondEgo.tools.search import SearchTool
from SecondEgo.verification.verifier import VerificationEngine
from SecondEgo.storage.sqlite import SQLiteRunStore


def main() -> int:
    parser = argparse.ArgumentParser(description="Run SecondEgo with an auditable action plan.")
    parser.add_argument("solve", help="literal command: solve")
    parser.add_argument("--repo", required=True, type=Path, help="target repository root")
    parser.add_argument("--issue", required=True, help="issue description")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--plan", type=Path, help="JSON plan with actions and verification_commands")
    source.add_argument("--gemini", action="store_true", help="request a structured plan from Gemini")
    parser.add_argument("--model", default="gemini-3.8-flash", help="Gemini model identifier for --gemini mode")
    parser.add_argument("--max-model-calls", type=int, default=20)
    parser.add_argument("--max-tool-calls", type=int, default=80)
    parser.add_argument("--max-retries", type=int, default=6)
    parser.add_argument("--state-db", type=Path, help="optional SQLite path for run state and evidence")
    arguments = parser.parse_args()
    if arguments.solve != "solve":
        parser.error("the first argument must be 'solve'")

    workspace = WorkspacePolicy(arguments.repo)
    resources = ResourceUsage(
        ResourceBudget(
            max_model_calls=arguments.max_model_calls,
            max_tool_calls=arguments.max_tool_calls,
            max_retries=arguments.max_retries,
        )
    )
    runner = CommandRunner(workspace)
    git = GitTool(runner)
    engine = HarnessEngine(
        scanner=RepositoryScanner(workspace),
        router=ToolRouter(
            files=FileTool(workspace),
            search=SearchTool(workspace),
            runner=runner,
            resources=resources,
            git=git,
        ),
        verifier=VerificationEngine(runner),
        resources=resources,
        store=SQLiteRunStore(arguments.state_db) if arguments.state_db else None,
        git=git,
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
        planner = ModelPlanner(
            provider=GeminiProvider(model=arguments.model),
            assembler=ContextAssembler(ContextBudget(24_000, 4_000, 2_000, 12_000, 3_000, 3_000)),
            resources=resources,
        )
        result = asyncio.run(
            engine.run_with_planner(
                task=arguments.issue,
                acceptance_criteria=(),
                planner=planner,
            )
        )
    print(json.dumps(_report(result), indent=2, default=str))
    return 0 if result.verification.passed else 1


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
        },
        "events": [
            {
                "event_type": event.event_type,
                "phase": event.phase,
                "status": event.status,
                "timestamp": event.timestamp.isoformat(),
                "evidence_ref": event.evidence_ref,
                "payload": event.payload,
            }
            for event in result.events
        ],
        "evidence": [
            {
                "reference": item.reference,
                "summary": item.summary,
                "source": item.source,
                "importance": item.importance,
            }
            for item in result.evidence
        ],
    }


if __name__ == "__main__":
    raise SystemExit(main())
