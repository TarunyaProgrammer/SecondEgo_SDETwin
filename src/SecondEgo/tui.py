import asyncio
import json
import os
import sys

from SecondEgo.app import build_engine, build_model_planner, build_resources
from SecondEgo.config import configured_model, configured_presentation_mode
from SecondEgo.core.state import AcceptanceCriterion
from SecondEgo.core.events import EngineEvent
from SecondEgo.repository.source import RepositorySourceError, resolve_repository
from SecondEgo.lifecycle import collect_garbage


def _print_event(event: EngineEvent) -> None:
    icon = {"state.changed": "·", "tool.completed": "→", "verification.completed": "✓", "run.terminated": "■"}.get(event.event_type, "·")
    print(f"  {icon} {event.phase:<9} {event.event_type}", flush=True)


def _paint(code: str, value: str) -> str:
    if sys.stdout.isatty() and os.environ.get("NO_COLOR") is None:
        return f"\033[{code}m{value}\033[0m"
    return value


def _banner() -> None:
    print()
    for line in (
        " ███████╗███████╗ ██████╗ ██████╗ ███╗   ██╗██████╗     ███████╗ ██████╗  ██████╗ ",
        " ██╔════╝██╔════╝██╔════╝██╔═══██╗████╗  ██║██╔══██╗    ██╔════╝██╔════╝ ██╔═══██╗",
        " ███████╗█████╗  ██║     ██║   ██║██╔██╗ ██║██║  ██║    █████╗  ██║  ███╗██║   ██║",
        " ╚════██║██╔══╝  ██║     ██║   ██║██║╚██╗██║██║  ██║    ██╔══╝  ██║   ██║██║   ██║",
        " ███████║███████╗╚██████╗╚██████╔╝██║ ╚████║██████╔╝    ███████╗╚██████╔╝╚██████╔╝",
        " ╚══════╝╚══════╝ ╚═════╝ ╚═════╝ ╚═╝  ╚═══╝╚═════╝     ╚══════╝ ╚═════╝  ╚═════╝ ",
    ):
        print(_paint("33", line))
    print()
    print(_paint("33", "╭─ SECOND EGO · VERIFIED CODING HARNESS ───────────────────────────────╮"))
    print(_paint("33", "│  understand  ›  explore  ›  plan  ›  execute  ›  verify              │"))
    print(_paint("33", "│  local-first repository intelligence with bounded, inspectable runs   │"))
    print(_paint("33", "╰───────────────────────────────────────────────────────────────────────╯"))
    print(f"  {_paint('90', 'PYTHON COMPATIBILITY / INTERACTIVE')}  Type Ctrl-C to cancel.\n")


def _read_multiline_issue() -> str:
    print(_paint("33", "Mission / issue"))
    print(_paint("90", "  Paste or type the complete task. Finish with a line containing .done (Ctrl-D also works)."))
    lines: list[str] = []
    while True:
        try:
            line = input(f"  {_paint('33', '│')} ")
        except EOFError:
            break
        if line.strip() == ".done":
            break
        lines.append(line)
    return "\n".join(lines).strip()


def main() -> int:
    gc = collect_garbage()
    if gc.deleted:
        print(f"  {_paint('90', f'garbage collector reclaimed {gc.deleted} stale temp directories')}")
    presentation_mode = configured_presentation_mode()
    _banner()
    repository_text = input(_paint("36", "Repository path or GitHub URL › ")).strip() or "."
    issue = _read_multiline_issue()
    if not issue:
        print("No issue supplied; exiting without an agent run.")
        return 0
    try:
        repository = resolve_repository(repository_text)
        if repository.cloned:
            print(f"  {_paint('32', '✓')} cloned repository to {repository.root}")
        engine = build_engine(
            repository.root,
            resources=build_resources(),
            event_sink=_print_event if presentation_mode.value == "events" else None,
        )
    except (RepositorySourceError, ValueError) as exc:
        if "repository" in locals():
            repository.cleanup()
        print(f"Invalid repository: {exc}")
        return 2
    try:
        result = asyncio.run(
            engine.run_with_planner(
                task=issue,
                acceptance_criteria=(AcceptanceCriterion("Implement and verify the supplied issue"),),
                planner=build_model_planner(engine.resources, configured_model()),
            )
        )
        print(f"\n  {_paint('90', 'RUN COMPLETE')}  {result.state.status.value}  ·  {result.state.phase.value}")
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
    finally:
        repository.cleanup()


if __name__ == "__main__":
    raise SystemExit(main())
