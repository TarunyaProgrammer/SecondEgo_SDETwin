import os
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


class PolicyViolation(ValueError):
    """Raised when a tool request exceeds the configured workspace policy."""


@dataclass(frozen=True)
class WorkspacePolicy:
    root: Path

    def __post_init__(self) -> None:
        object.__setattr__(self, "root", self.root.expanduser().resolve())
        if not self.root.is_dir():
            raise ValueError(f"workspace is not a directory: {self.root}")

    def resolve_path(self, requested: str | Path) -> Path:
        candidate = Path(requested)
        if not candidate.is_absolute():
            candidate = self.root / candidate
        resolved = candidate.expanduser().resolve(strict=False)
        try:
            resolved.relative_to(self.root)
        except ValueError as exc:
            raise PolicyViolation("path escapes the configured workspace") from exc
        return resolved


@dataclass(frozen=True)
class CommandPolicy:
    allowed_executables: frozenset[str]
    max_timeout_seconds: float = 120.0
    max_output_bytes: int = 256_000

    @classmethod
    def default(cls) -> "CommandPolicy":
        # Include versioned python3.x executables so sys.executable resolves
        # correctly on any Python 3 install (e.g. python3.12, python3.14).
        versioned_pythons = {f"python3.{minor}" for minor in range(9, 20)}
        return cls(
            allowed_executables=frozenset(
                {
                    "git",
                    "npm",
                    "pnpm",
                    "pytest",
                    "python",
                    "python3",
                    "ruff",
                    "uv",
                }
            )
            | versioned_pythons
        )

    def validate(self, argv: Iterable[str], timeout_seconds: float) -> tuple[str, ...]:
        normalized = tuple(argv)
        if not normalized or not normalized[0].strip():
            raise PolicyViolation("command must contain an executable")
        executable = os.path.basename(normalized[0])
        if executable not in self.allowed_executables:
            raise PolicyViolation(f"executable is not allowlisted: {executable}")
        if timeout_seconds <= 0 or timeout_seconds > self.max_timeout_seconds:
            raise PolicyViolation("command timeout exceeds policy")
        if any("\x00" in argument for argument in normalized):
            raise PolicyViolation("command arguments cannot contain NUL bytes")
        return normalized

