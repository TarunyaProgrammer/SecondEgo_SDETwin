import os
from enum import StrEnum


class ProviderKind(StrEnum):
    """Supported structured-planning providers selected outside orchestration."""

    DEEPSEEK = "deepseek"
    GEMINI = "gemini"
    OPENAI = "openai"
    ANTHROPIC = "anthropic"
    OPENROUTER = "openrouter"
    GROQ = "groq"


DEFAULT_PROVIDER = ProviderKind.DEEPSEEK
DEFAULT_MODELS = {
    ProviderKind.DEEPSEEK: "deepseek-flash",
    ProviderKind.GEMINI: "gemini-3.8-flash",
    ProviderKind.OPENAI: "gpt-5-mini",
    ProviderKind.ANTHROPIC: "claude-sonnet-4-6",
    ProviderKind.OPENROUTER: "openai/gpt-5-mini",
    ProviderKind.GROQ: "qwen/qwen3.8-27b",
}

PROVIDER_KEY_NAMES = {
    ProviderKind.DEEPSEEK: "DEEPSEEK_API_KEY",
    ProviderKind.GEMINI: "GEMINI_API_KEY",
    ProviderKind.OPENAI: "OPENAI_API_KEY",
    ProviderKind.ANTHROPIC: "ANTHROPIC_API_KEY",
    ProviderKind.OPENROUTER: "OPENROUTER_API_KEY",
    ProviderKind.GROQ: "GROQ_API_KEY",
}
# Compatibility for callers that read the default without selecting a provider.
DEFAULT_MODEL = DEFAULT_MODELS[DEFAULT_PROVIDER]


class PresentationMode(StrEnum):
    """Controls presentation only; it never changes engine policy or model calls."""

    HEADLESS = "headless"
    EVENTS = "events"


def configured_provider() -> ProviderKind:
    """Read the provider choice without reading a credential."""
    value = os.environ.get("SECONDEGO_PROVIDER", DEFAULT_PROVIDER.value).strip().lower()
    try:
        return ProviderKind(value or DEFAULT_PROVIDER.value)
    except ValueError as exc:
        allowed = ", ".join(provider.value for provider in ProviderKind)
        raise ValueError(f"SECONDEGO_PROVIDER must be one of: {allowed}") from exc


def configured_model(provider: ProviderKind | None = None) -> str:
    """Return a model override or the selected provider's safe evaluator default."""
    return os.environ.get("SECONDEGO_MODEL", "").strip() or DEFAULT_MODELS[provider or configured_provider()]


def configured_presentation_mode() -> PresentationMode:
    """Read the optional event-display mode without reading any secret."""
    value = os.environ.get("SECONDEGO_UI_MODE", PresentationMode.HEADLESS.value)
    try:
        return PresentationMode(value.strip().lower())
    except ValueError as exc:
        allowed = ", ".join(mode.value for mode in PresentationMode)
        raise ValueError(f"SECONDEGO_UI_MODE must be one of: {allowed}") from exc
