from pathlib import Path

from .contracts import ToolResult
from .policy import WorkspacePolicy


class FileTool:
    def __init__(self, workspace: WorkspacePolicy, *, max_file_bytes: int = 512_000) -> None:
        self.workspace = workspace
        self.max_file_bytes = max_file_bytes

    def read(self, path: str | Path) -> ToolResult:
        resolved = self.workspace.resolve_path(path)
        if not resolved.is_file():
            return ToolResult(tool="read_file", success=False, stderr="file does not exist")
        if resolved.stat().st_size > self.max_file_bytes:
            return ToolResult(tool="read_file", success=False, stderr="file exceeds size limit")
        try:
            content = resolved.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            return ToolResult(tool="read_file", success=False, stderr="file is not UTF-8 text")
        return ToolResult(tool="read_file", success=True, stdout=content)

    def write(self, path: str | Path, content: str) -> ToolResult:
        resolved = self.workspace.resolve_path(path)
        encoded = content.encode("utf-8")
        if len(encoded) > self.max_file_bytes:
            return ToolResult(tool="edit_file", success=False, stderr="content exceeds size limit")
        resolved.parent.mkdir(parents=True, exist_ok=True)
        resolved.write_text(content, encoding="utf-8")
        relative = resolved.relative_to(self.workspace.root).as_posix()
        return ToolResult(
            tool="edit_file",
            success=True,
            changed_paths=(relative,),
        )

