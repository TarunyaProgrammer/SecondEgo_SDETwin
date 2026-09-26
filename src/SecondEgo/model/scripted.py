from collections import deque
from dataclasses import dataclass, field
from typing import Any

from .base import ActionProposal, ModelProvider


@dataclass
class ScriptedProvider(ModelProvider):
    """Deterministic provider for replayable harness tests."""

    proposals: deque[ActionProposal] = field(default_factory=deque)

    def __init__(self, proposals: list[ActionProposal]) -> None:
        self.proposals = deque(proposals)

    async def generate(self, prompt: str, *, context: dict[str, Any]) -> ActionProposal:
        if not self.proposals:
            raise RuntimeError("scripted provider has no remaining proposals")
        return self.proposals.popleft()

    async def count_tokens(self, text: str) -> int:
        return max(1, (len(text) + 3) // 4)

