"""OpenAI-compatible providers used by the Python compatibility engine."""

import asyncio
import json
import os
import warnings
from dataclasses import dataclass
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from .base import ActionProposal
from .structured import ProviderConfigurationError, parse_action


DEFAULT_OPENAI_BASE_URL = "https://api.openai.com/v1"
DEFAULT_OPENROUTER_BASE_URL = "https://openrouter.ai/api/v1"


def _resolve_key(explicit: str | None, variable: str) -> str | None:
    """Prefer the provider-specific key, retaining AI_API_KEY migration support."""
    key = explicit or os.environ.get(variable)
    if key:
        return key
    legacy = os.environ.get("AI_API_KEY")
    if legacy:
        warnings.warn(f"AI_API_KEY is deprecated; use {variable}", DeprecationWarning, stacklevel=2)
    return legacy


def _request_action(
    *,
    provider_name: str,
    model: str,
    prompt: str,
    key: str,
    endpoint: str,
    timeout: float,
    extra_headers: dict[str, str] | None = None,
) -> ActionProposal:
    payload = {
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "Return only one JSON object with action, arguments, and rationale. Do not use markdown.",
            },
            {"role": "user", "content": prompt},
        ],
        "response_format": {"type": "json_object"},
        "temperature": 0.0,
        "max_tokens": 4096,
    }
    headers = {"Authorization": f"Bearer {key}", "Content-Type": "application/json"}
    if extra_headers:
        headers.update(extra_headers)
    request = Request(
        endpoint,
        data=json.dumps(payload).encode("utf-8"),
        headers=headers,
        method="POST",
    )
    try:
        with urlopen(request, timeout=timeout) as response:  # noqa: S310 - configured HTTPS provider endpoint
            response_payload = json.load(response)
    except HTTPError as exc:
        raise ProviderConfigurationError(
            f"{provider_name} request failed: HTTP {exc.code}"
        ) from exc
    except (URLError, OSError, TimeoutError) as exc:
        raise ProviderConfigurationError(
            f"{provider_name} request failed: {type(exc).__name__}"
        ) from exc
    try:
        content = response_payload["choices"][0]["message"]["content"]
        data = json.loads(content)
    except (KeyError, IndexError, TypeError, json.JSONDecodeError) as exc:
        raise RuntimeError(f"{provider_name} returned invalid structured action output") from exc
    return parse_action(data)


@dataclass(frozen=True)
class OpenAIProvider:
    """Direct OpenAI Chat Completions adapter."""

    model: str = "gpt-5-mini"
    api_key: str | None = None
    base_url: str | None = None
    timeout_seconds: float = 60.0

    async def generate(self, prompt: str, *, context: dict[str, Any]) -> ActionProposal:
        del context
        key = _resolve_key(self.api_key, "OPENAI_API_KEY")
        if not key:
            raise ProviderConfigurationError("OPENAI_API_KEY is required for OpenAIProvider")
        base_url = self.base_url or os.environ.get("SECONDEGO_OPENAI_BASE_URL", DEFAULT_OPENAI_BASE_URL)
        return await asyncio.to_thread(
            _request_action,
            provider_name="OpenAI",
            model=self.model,
            prompt=prompt,
            key=key,
            endpoint=f"{base_url.rstrip('/')}/chat/completions",
            timeout=self.timeout_seconds,
        )

    async def count_tokens(self, text: str) -> int:
        return max(1, (len(text) + 3) // 4)

    def _resolve_api_key(self) -> str | None:
        return _resolve_key(self.api_key, "OPENAI_API_KEY")


@dataclass(frozen=True)
class OpenRouterProvider:
    """OpenRouter adapter using its OpenAI-compatible endpoint and model slugs."""

    model: str = "openai/gpt-5-mini"
    api_key: str | None = None
    base_url: str | None = None
    timeout_seconds: float = 60.0

    async def generate(self, prompt: str, *, context: dict[str, Any]) -> ActionProposal:
        del context
        key = _resolve_key(self.api_key, "OPENROUTER_API_KEY")
        if not key:
            raise ProviderConfigurationError("OPENROUTER_API_KEY is required for OpenRouterProvider")
        base_url = self.base_url or os.environ.get(
            "SECONDEGO_OPENROUTER_BASE_URL", DEFAULT_OPENROUTER_BASE_URL
        )
        extra_headers = {}
        if referer := os.environ.get("SECONDEGO_OPENROUTER_REFERER", "").strip():
            extra_headers["HTTP-Referer"] = referer
        if title := os.environ.get("SECONDEGO_OPENROUTER_TITLE", "").strip():
            extra_headers["X-Title"] = title
        return await asyncio.to_thread(
            _request_action,
            provider_name="OpenRouter",
            model=self.model,
            prompt=prompt,
            key=key,
            endpoint=f"{base_url.rstrip('/')}/chat/completions",
            timeout=self.timeout_seconds,
            extra_headers=extra_headers,
        )

    async def count_tokens(self, text: str) -> int:
        return max(1, (len(text) + 3) // 4)

    def _resolve_api_key(self) -> str | None:
        return _resolve_key(self.api_key, "OPENROUTER_API_KEY")
