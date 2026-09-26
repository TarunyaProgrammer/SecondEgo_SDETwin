import json
from dataclasses import dataclass
from typing import Any

from SecondEgo.context.assembler import ContextAssembler
from SecondEgo.context.policy import EvidenceRecord
from SecondEgo.core.resources import ResourceUsage
from SecondEgo.core.state import ExecutionState
from SecondEgo.verification.contracts import VerificationResult

from .base import ActionProposal, ModelProvider


class PlanValidationError(ValueError):
    """Raised when untrusted provider output does not form a safe action plan."""


@dataclass(frozen=True)
class ActionPlan:
    actions: tuple[ActionProposal, ...]
    verification_commands: tuple[tuple[str, ...], ...]
    recovery_actions: tuple[ActionProposal, ...] = ()


@dataclass(frozen=True)
class ModelCallTelemetry:
    call_type: str
    estimated_tokens: int
    slot_usage: dict[str, int]
    dropped_evidence: tuple[str, ...]


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
        self.last_call: ModelCallTelemetry | None = None

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
        self._record_context("plan", packet)
        self.resources.record_model_call(packet.estimated_tokens)
        proposal = await self.provider.generate(
            packet.as_text(),
            context={"run_id": state.run_id, "phase": state.phase.value},
        )
        return _parse_plan(proposal)

    async def create_recovery_actions(
        self,
        state: ExecutionState,
        evidence: list[EvidenceRecord],
        verification: VerificationResult,
    ) -> tuple[ActionProposal, ...]:
        failure = verification.failure_record
        failure_summary = verification.failure_summary or "verification failed"
        if failure is not None:
            failure_summary = (
                f"class={failure.failure_class.value}; "
                f"tests={list(failure.failing_tests)}; "
                f"locations={list(failure.error_locations)}; "
                f"fingerprint={failure.fingerprint or 'none'}; "
                f"summary={failure.summary}"
            )
        recovery_evidence = list(evidence)
        recovery_evidence.append(
            EvidenceRecord(
                reference="diagnosis:failure",
                summary=failure_summary,
                source="verification failure record",
                importance=5,
            )
        )
        packet = self.assembler.assemble(
            task=state.task,
            action=_RECOVERY_INSTRUCTION,
            evidence=recovery_evidence,
            state=json.dumps(state.snapshot(), separators=(",", ":"), default=str),
        )
        self._record_context("recovery", packet)
        self.resources.record_model_call(packet.estimated_tokens)
        proposal = await self.provider.generate(
            packet.as_text(),
            context={"run_id": state.run_id, "phase": "DIAGNOSE"},
        )
        return _parse_recovery_actions(proposal)

    def _record_context(self, call_type: str, packet: object) -> None:
        # Keep this data structured and bounded; raw prompts are never retained.
        self.last_call = ModelCallTelemetry(
            call_type=call_type,
            estimated_tokens=packet.estimated_tokens,
            slot_usage=dict(packet.slot_usage),
            dropped_evidence=tuple(packet.dropped_evidence),
        )


_PLAN_INSTRUCTION = """Return exactly one action named submit_plan. Its arguments must contain:
actions: a list of {action, arguments, rationale} tool actions;
verification_commands: a list of argv arrays for test, lint, build, or focused verification commands;
recovery_actions: an optional list of corrective tool actions.
Use only workspace-safe actions: read_file, search_code, edit_file, run_command, git_diff, git_status.
Do not include shell strings; every command must be an argv array."""

_RECOVERY_INSTRUCTION = """Return exactly one action named submit_repair_plan. Its arguments must contain:
actions: a non-empty list of corrective {action, arguments, rationale} tool actions.
Use the verification failure record and cited repository evidence to choose a different repair strategy.
Use only workspace-safe actions: read_file, search_code, edit_file, run_command, git_diff, git_status.
Do not include shell strings; commands must be argv arrays. Do not modify tests unless the task explicitly requires it."""


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


def _parse_recovery_actions(proposal: ActionProposal) -> tuple[ActionProposal, ...]:
    if proposal.action != "submit_repair_plan":
        raise PlanValidationError("recovery planner must return a submit_repair_plan action")
    actions = _parse_actions(proposal.arguments.get("actions"), field="recovery actions")
    if not actions:
        raise PlanValidationError("recovery actions must not be empty")
    return tuple(actions)
