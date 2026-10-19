"""Native Anthropic Messages API adapter for structured planning."""

import asyncio
import json
import os
from dataclasses import dataclass
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from .base import ActionProposal
from .structured import ProviderConfigurationError, parse_action


DEFAULT_BASE_URL = "https://api.anthropic.com/v1"
ANTHROPIC_API_VERSION = "2023-06-01"


@dataclass(frozen=True)
class AnthropicProvider:
    """Anthropic Messages adapter with a JSON-only system contract."""

    model: str = "claude-sonnet-4-6"
    api_key: str | None = None
    base_url: str | None = None
    timeout_seconds: float = 60.0

    async def generate(self, prompt: str, *, context: dict[str, Any]) -> ActionProposal:
        del context
        return await asyncio.to_thread(self._generate_sync, prompt)

    async def count_tokens(self, text: str) -> int:
        return max(1, (len(text) + 3) // 4)

    def _generate_sync(self, prompt: str) -> ActionProposal:
        key = self._resolve_api_key()
        if not key:
            raise ProviderConfigurationError("ANTHROPIC_API_KEY is required for AnthropicProvider")
        base_url = self.base_url or os.environ.get("SECONDEGO_ANTHROPIC_BASE_URL", DEFAULT_BASE_URL)
        payload = {
            "model": self.model,
            "system": "Return only one JSON object with action, arguments, and rationale. Do not use markdown.",
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.0,
            "max_tokens": 4096,
        }
        request = Request(
            f"{base_url.rstrip('/')}/messages",
            data=json.dumps(payload).encode("utf-8"),
            headers={
                "x-api-key": key,
                "anthropic-version": ANTHROPIC_API_VERSION,
                "Content-Type": "application/json",
            },
            method="POST",
        )
        try:
            with urlopen(request, timeout=self.timeout_seconds) as response:  # noqa: S310 - configured HTTPS provider endpoint
                response_payload = json.load(response)
        except HTTPError as exc:
            raise ProviderConfigurationError(
                f"Anthropic request failed: HTTP {exc.code}"
            ) from exc
        except (URLError, OSError, TimeoutError) as exc:
            raise ProviderConfigurationError(
                f"Anthropic request failed: {type(exc).__name__}"
            ) from exc
        try:
            text = "".join(
                block["text"]
                for block in response_payload["content"]
                if block.get("type") == "text"
            )
            data = json.loads(text)
        except (KeyError, TypeError, json.JSONDecodeError) as exc:
            raise RuntimeError("Anthropic returned invalid structured action output") from exc
        return parse_action(data)

    def _resolve_api_key(self) -> str | None:
        return self.api_key or os.environ.get("ANTHROPIC_API_KEY") or os.environ.get("AI_API_KEY")
