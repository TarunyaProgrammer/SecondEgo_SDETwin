import json
from typing import Any

from SecondEgo.core.resources import ResourceLimitExceeded, ResourceUsage
from SecondEgo.model.base import ActionProposal

from .contracts import ToolResult
from .filesystem import FileTool
from .git import GitTool
from .runner import CommandRunner
from .search import SearchTool


class ToolRouter:
    """Maps validated model action proposals to the small approved tool surface."""

    def __init__(
        self,
        *,
        files: FileTool,
        search: SearchTool,
        runner: CommandRunner,
        resources: ResourceUsage,
        git: GitTool | None = None,
    ) -> None:
        self.files = files
        self.search = search
        self.runner = runner
        self.resources = resources
        self.git = git or GitTool(runner)

    def dispatch(self, proposal: ActionProposal) -> ToolResult:
        try:
            self.resources.record_tool_call()
            return self._dispatch(proposal.action, proposal.arguments)
        except (KeyError, TypeError, ValueError, ResourceLimitExceeded) as exc:
            return ToolResult(tool=proposal.action, success=False, stderr=str(exc))

    def _dispatch(self, action: str, arguments: dict[str, Any]) -> ToolResult:
        match action:
            case "read_file":
                return self.files.read(_required_string(arguments, "path"))
            case "edit_file":
                return self.files.write(
                    _required_string(arguments, "path"),
                    _required_string(arguments, "content"),
                )
            case "search_code":
                results = self.search.text(
                    _required_string(arguments, "query"),
                    path=arguments.get("path", "."),
                )
                return ToolResult(tool="search_code", success=True, stdout=json.dumps(results))
            case "run_command":
                argv = arguments.get("argv")
                if not isinstance(argv, list) or not all(isinstance(item, str) for item in argv):
                    raise ValueError("argv must be a list of strings")
                timeout = arguments.get("timeout_seconds", 30.0)
                if not isinstance(timeout, (int, float)):
                    raise ValueError("timeout_seconds must be numeric")
                return self.runner.run(
                    argv,
                    cwd=arguments.get("cwd", "."),
                    timeout_seconds=float(timeout),
                )
            case "git_diff":
                return self.git.diff()
            case "git_status":
                return self.git.status()
            case _:
                raise ValueError(f"unsupported tool action: {action}")


def _required_string(arguments: dict[str, Any], field: str) -> str:
    value = arguments.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{field} must be a non-empty string")
    return value
