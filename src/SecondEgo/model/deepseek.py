"""DeepSeek's OpenAI-compatible structured-planning adapter.

The implementation uses the standard library so DeepSeek remains the
evaluation default without adding an SDK dependency to the headless path.
"""

import asyncio
import json
import os
from dataclasses import dataclass
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from .base import ActionProposal
from .structured import ProviderConfigurationError, parse_action


DEFAULT_BASE_URL = "https://api.deepseek.com"


@dataclass(frozen=True)
class DeepSeekProvider:
    """OpenAI-compatible DeepSeek adapter with JSON-only output."""

    model: str = "deepseek-flash"
    api_key: str | None = None
    base_url: str | None = None
    timeout_seconds: float = 60.0

    async def generate(self, prompt: str, *, context: dict[str, Any]) -> ActionProposal:
        del context  # The bounded planner context is already included in the prompt.
        return await asyncio.to_thread(self._generate_sync, prompt)

    async def count_tokens(self, text: str) -> int:
        return max(1, (len(text) + 3) // 4)

    def _generate_sync(self, prompt: str) -> ActionProposal:
        key = self._resolve_api_key()
        if not key:
            raise ProviderConfigurationError("AI_API_KEY is required for DeepSeekProvider")
        base_url = self.base_url or os.environ.get("SECONDEGO_DEEPSEEK_BASE_URL", DEFAULT_BASE_URL)
        endpoint = f"{base_url.rstrip('/')}/chat/completions"
        payload = {
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": "Return only one JSON object with action, arguments, and rationale. Do not use markdown.",
                },
                {"role": "user", "content": prompt},
            ],
            "response_format": {"type": "json_object"},
            "thinking": {"type": "disabled"},
            "max_tokens": 4096,
        }
        request = Request(
            endpoint,
            data=json.dumps(payload).encode("utf-8"),
            headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
            method="POST",
        )
        try:
            with urlopen(request, timeout=self.timeout_seconds) as response:  # noqa: S310 - configured HTTPS API endpoint
                response_payload = json.load(response)
        except (HTTPError, URLError, OSError, TimeoutError) as exc:
            raise ProviderConfigurationError(f"DeepSeek request failed: {type(exc).__name__}") from exc
        try:
            content = response_payload["choices"][0]["message"]["content"]
            data = json.loads(content)
        except (KeyError, IndexError, TypeError, json.JSONDecodeError) as exc:
            raise RuntimeError("DeepSeek returned invalid structured action output") from exc
        return parse_action(data)

    def _resolve_api_key(self) -> str | None:
        return self.api_key or os.environ.get("AI_API_KEY")
