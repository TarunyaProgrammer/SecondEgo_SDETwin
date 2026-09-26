from dataclasses import dataclass, field
from typing import Any, Protocol


@dataclass(frozen=True)
class ActionProposal:
    action: str
    arguments: dict[str, Any] = field(default_factory=dict)
    rationale: str = ""


class ModelProvider(Protocol):
    async def generate(self, prompt: str, *, context: dict[str, Any]) -> ActionProposal:
        ...

    async def count_tokens(self, text: str) -> int:
        ...

