import asyncio
import json
from pathlib import Path

from SecondEgo.app import build_engine, build_gemini_planner, build_resources
from SecondEgo.config import configured_model
from SecondEgo.core.state import AcceptanceCriterion


def main() -> int:
    print("SecondEgo evaluation mode")
    print("Text-only autonomous coding harness. Type Ctrl-C to cancel before execution.")
    repository_text = input("Repository path [.]: ").strip() or "."
    issue = input("Issue: ").strip()
    if not issue:
        print("No issue supplied; exiting without an agent run.")
        return 0
    try:
        engine = build_engine(Path(repository_text), resources=build_resources())
    except ValueError as exc:
        print(f"Invalid repository: {exc}")
        return 2
    result = asyncio.run(
        engine.run_with_planner(
            task=issue,
            acceptance_criteria=(AcceptanceCriterion("Implement and verify the supplied issue"),),
            planner=build_gemini_planner(engine.resources, configured_model()),
        )
    )
    print(
        json.dumps(
            {
                "run_id": result.state.run_id,
                "status": result.state.status.value,
                "termination_reason": result.state.termination_reason,
                "changed_paths": sorted(result.state.changed_paths),
                "verification": {
                    "passed": result.verification.passed,
                    "failure_class": result.verification.failure_class.value,
                },
            },
            indent=2,
        )
    )
    return 0 if result.verification.passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
