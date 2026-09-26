import json
from dataclasses import dataclass
from typing import Any

from SecondEgo.context.assembler import ContextAssembler
from SecondEgo.context.policy import EvidenceRecord
from SecondEgo.core.resources import ResourceUsage
from SecondEgo.core.state import ExecutionState

from .base import ActionProposal, ModelProvider


class PlanValidationError(ValueError):
    """Raised when untrusted provider output does not form a safe action plan."""


@dataclass(frozen=True)
class ActionPlan:
    actions: tuple[ActionProposal, ...]
    verification_commands: tuple[tuple[str, ...], ...]
    recovery_actions: tuple[ActionProposal, ...] = ()


class ModelPlanner:
    """Converts one bounded, structured model response into a validated execution plan."""

    def __init__(
        self,
        *,
        provider: ModelProvider,
        assembler: ContextAssembler,
        resources: ResourceUsage,
    ) -> None:
        self.provider = provider
        self.assembler = assembler
        self.resources = resources

    async def create_plan(
        self,
        state: ExecutionState,
        evidence: list[EvidenceRecord],
    ) -> ActionPlan:
        packet = self.assembler.assemble(
            task=state.task,
            action=_PLAN_INSTRUCTION,
            evidence=evidence,
            state=json.dumps(state.snapshot(), separators=(",", ":"), default=str),
        )
        self.resources.record_model_call(packet.estimated_tokens)
        proposal = await self.provider.generate(
            packet.as_text(),
            context={"run_id": state.run_id, "phase": state.phase.value},
        )
        return _parse_plan(proposal)


_PLAN_INSTRUCTION = """Return exactly one action named submit_plan. Its arguments must contain:
actions: a list of {action, arguments, rationale} tool actions;
verification_commands: a list of argv arrays for test, lint, build, or focused verification commands;
recovery_actions: an optional list of corrective tool actions.
Use only workspace-safe actions: read_file, search_code, edit_file, run_command, git_diff, git_status.
Do not include shell strings; every command must be an argv array."""


def _parse_plan(proposal: ActionProposal) -> ActionPlan:
    if proposal.action != "submit_plan":
        raise PlanValidationError("planner must return a submit_plan action")
    arguments = proposal.arguments
    actions = _parse_actions(arguments.get("actions"), field="actions")
    recovery_actions = _parse_actions(arguments.get("recovery_actions", []), field="recovery_actions")
    commands_raw = arguments.get("verification_commands")
    if not isinstance(commands_raw, list) or not commands_raw:
        raise PlanValidationError("verification_commands must be a non-empty list")
    commands: list[tuple[str, ...]] = []
    for command in commands_raw:
        if not isinstance(command, list) or not command or not all(isinstance(item, str) and item for item in command):
            raise PlanValidationError("each verification command must be a non-empty argv array")
        commands.append(tuple(command))
    return ActionPlan(
        actions=tuple(actions),
        verification_commands=tuple(commands),
        recovery_actions=tuple(recovery_actions),
    )


def _parse_actions(value: object, *, field: str) -> list[ActionProposal]:
    if not isinstance(value, list):
        raise PlanValidationError(f"{field} must be a list")
    parsed: list[ActionProposal] = []
    for item in value:
        if not isinstance(item, dict):
            raise PlanValidationError(f"{field} entries must be objects")
        action = item.get("action")
        arguments = item.get("arguments")
        rationale = item.get("rationale", "")
        if not isinstance(action, str) or not isinstance(arguments, dict) or not isinstance(rationale, str):
            raise PlanValidationError(f"{field} entries require action, arguments, and rationale")
        parsed.append(ActionProposal(action=action, arguments=arguments, rationale=rationale))
    return parsed
