import asyncio
import json
import os
from dataclasses import dataclass
from typing import Any

from .base import ActionProposal
from .structured import ACTION_SCHEMA, ProviderConfigurationError, parse_action


@dataclass(frozen=True)
class GeminiProvider:
    """Optional Gemini adapter; SDK import and API-key access are delayed until use."""

    model: str = "gemini-3.8-flash"
    api_key: str | None = None

    async def generate(self, prompt: str, *, context: dict[str, Any]) -> ActionProposal:
        del context  # The planner includes bounded context in the prompt itself.
        return await asyncio.to_thread(self._generate_sync, prompt)

    async def count_tokens(self, text: str) -> int:
        # Deterministic estimate keeps preflight budgeting local and avoids a separate paid API call.
        return max(1, (len(text) + 3) // 4)

    def _generate_sync(self, prompt: str) -> ActionProposal:
        key = self._resolve_api_key()
        if not key:
            raise ProviderConfigurationError("AI_API_KEY is required for GeminiProvider")
        try:
            from google import genai
        except ImportError as exc:
            raise ProviderConfigurationError(
                "Gemini support requires the optional dependency: pip install '.[gemini]'"
            ) from exc

        try:
            client = genai.Client(api_key=key)
            response = client.models.generate_content(
                model=self.model,
                contents=prompt,
                config={
                    "response_mime_type": "application/json",
                    "response_schema": ACTION_SCHEMA,
                },
            )
        except Exception as exc:
            # Transport/library failures must become bounded run evidence; exception
            # text can contain URLs, request details, or provider-sensitive values.
            raise ProviderConfigurationError(
                f"Gemini request failed: {type(exc).__name__}"
            ) from exc
        try:
            data = json.loads(response.text)
        except (TypeError, json.JSONDecodeError) as exc:
            raise RuntimeError("Gemini returned invalid structured action output") from exc
        return parse_action(data)

    def _resolve_api_key(self) -> str | None:
        return self.api_key or os.environ.get("AI_API_KEY")
