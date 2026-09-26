"""Shared validation for provider-produced structured tool proposals."""

from .base import ActionProposal


class ProviderConfigurationError(RuntimeError):
    """Raised when an optional model provider is unavailable or misconfigured."""


ACTION_SCHEMA: dict[str, object] = {
    "type": "object",
    "properties": {
        "action": {"type": "string"},
        "arguments": {"type": "object"},
        "rationale": {"type": "string"},
    },
    "required": ["action", "arguments", "rationale"],
}


def parse_action(data: object) -> ActionProposal:
    if not isinstance(data, dict):
        raise RuntimeError("model action must be an object")
    action = data.get("action")
    arguments = data.get("arguments")
    rationale = data.get("rationale")
    if not isinstance(action, str) or not isinstance(arguments, dict) or not isinstance(rationale, str):
        raise RuntimeError("model action does not match the required schema")
    return ActionProposal(action=action, arguments=arguments, rationale=rationale)
