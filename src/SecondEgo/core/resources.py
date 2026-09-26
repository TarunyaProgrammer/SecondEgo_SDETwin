from dataclasses import dataclass, field
from time import monotonic


class ResourceLimitExceeded(RuntimeError):
    """Raised before a run exceeds its declared resource budget."""


@dataclass(frozen=True)
class ResourceBudget:
    max_model_calls: int = 20
    max_tool_calls: int = 80
    max_retries: int = 6
    max_runtime_seconds: float = 900.0
    max_context_tokens_per_call: int = 24_000

    def __post_init__(self) -> None:
        values = (
            self.max_model_calls,
            self.max_tool_calls,
            self.max_retries,
            self.max_runtime_seconds,
            self.max_context_tokens_per_call,
        )
        if any(value <= 0 for value in values):
            raise ValueError("resource budget values must be positive")


@dataclass
class ResourceUsage:
    budget: ResourceBudget
    model_calls: int = 0
    tool_calls: int = 0
    retries: int = 0
    context_tokens: int = 0
    started_at: float = field(default_factory=monotonic)

    @property
    def elapsed_seconds(self) -> float:
        return monotonic() - self.started_at

    def record_model_call(self, context_tokens: int) -> None:
        self._ensure_runtime()
        if context_tokens > self.budget.max_context_tokens_per_call:
            raise ResourceLimitExceeded("context exceeds per-call token budget")
        if self.model_calls >= self.budget.max_model_calls:
            raise ResourceLimitExceeded("model-call budget exhausted")
        self.model_calls += 1
        self.context_tokens += context_tokens

    def record_tool_call(self) -> None:
        self._ensure_runtime()
        if self.tool_calls >= self.budget.max_tool_calls:
            raise ResourceLimitExceeded("tool-call budget exhausted")
        self.tool_calls += 1

    def record_retry(self) -> None:
        self._ensure_runtime()
        if self.retries >= self.budget.max_retries:
            raise ResourceLimitExceeded("retry budget exhausted")
        self.retries += 1

    def snapshot(self) -> dict[str, float | int]:
        return {
            "model_calls": self.model_calls,
            "tool_calls": self.tool_calls,
            "retries": self.retries,
            "context_tokens": self.context_tokens,
            "elapsed_seconds": round(self.elapsed_seconds, 3),
        }

    def _ensure_runtime(self) -> None:
        if self.elapsed_seconds > self.budget.max_runtime_seconds:
            raise ResourceLimitExceeded("runtime budget exhausted")

