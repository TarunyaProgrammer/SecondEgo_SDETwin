from .contracts import ToolResult
from .runner import CommandRunner


class GitTool:
    """Small Git evidence surface used for verification and reporting."""

    def __init__(self, runner: CommandRunner) -> None:
        self.runner = runner

    def diff(self) -> ToolResult:
        return self._rename(
            self.runner.run(("git", "diff", "--no-ext-diff", "--unified=3")),
            "git_diff",
        )

    def status(self) -> ToolResult:
        return self._rename(self.runner.run(("git", "status", "--short")), "git_status")

    @staticmethod
    def _rename(result: ToolResult, tool: str) -> ToolResult:
        return ToolResult(
            tool=tool,
            success=result.success,
            exit_code=result.exit_code,
            stdout=result.stdout,
            stderr=result.stderr,
            changed_paths=result.changed_paths,
            duration_ms=result.duration_ms,
            evidence_ref=result.evidence_ref,
            truncated=result.truncated,
        )

