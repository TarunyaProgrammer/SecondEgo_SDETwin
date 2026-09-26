from dataclasses import dataclass, field
from typing import Any


@dataclass(frozen=True)
class ToolRequest:
    name: str
    arguments: dict[str, Any] = field(default_factory=dict)
    timeout_seconds: float = 30.0


@dataclass(frozen=True)
class ToolResult:
    tool: str
    success: bool
    exit_code: int | None = None
    stdout: str = ""
    stderr: str = ""
    changed_paths: tuple[str, ...] = ()
    duration_ms: int = 0
    evidence_ref: str | None = None
    truncated: bool = False

