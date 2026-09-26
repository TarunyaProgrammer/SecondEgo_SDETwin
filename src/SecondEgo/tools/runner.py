import subprocess
import time
from pathlib import Path
from typing import Sequence

from .contracts import ToolResult
from .policy import CommandPolicy, WorkspacePolicy


class CommandRunner:
    def __init__(
        self,
        workspace: WorkspacePolicy,
        command_policy: CommandPolicy | None = None,
    ) -> None:
        self.workspace = workspace
        self.command_policy = command_policy or CommandPolicy.default()

    def run(
        self,
        argv: Sequence[str],
        *,
        cwd: str | Path = ".",
        timeout_seconds: float = 30.0,
    ) -> ToolResult:
        command = self.command_policy.validate(argv, timeout_seconds)
        working_directory = self.workspace.resolve_path(cwd)
        started = time.monotonic()
        try:
            completed = subprocess.run(
                command,
                cwd=working_directory,
                capture_output=True,
                text=True,
                timeout=timeout_seconds,
                check=False,
                shell=False,
            )
            stdout = completed.stdout
            stderr = completed.stderr
            success = completed.returncode == 0
            exit_code = completed.returncode
            truncated = False
        except subprocess.TimeoutExpired as exc:
            stdout = _decode_output(exc.stdout)
            stderr = _decode_output(exc.stderr)
            success = False
            exit_code = None
            truncated = False
        duration_ms = int((time.monotonic() - started) * 1000)
        stdout, stdout_truncated = _cap_output(stdout, self.command_policy.max_output_bytes)
        stderr, stderr_truncated = _cap_output(stderr, self.command_policy.max_output_bytes)
        return ToolResult(
            tool="run_command",
            success=success,
            exit_code=exit_code,
            stdout=stdout,
            stderr=stderr,
            duration_ms=duration_ms,
            truncated=truncated or stdout_truncated or stderr_truncated,
        )


def _decode_output(value: str | bytes | None) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode(errors="replace")
    return value


def _cap_output(value: str, limit: int) -> tuple[str, bool]:
    encoded = value.encode()
    if len(encoded) <= limit:
        return value, False
    return encoded[:limit].decode(errors="replace"), True

