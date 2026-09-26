from pathlib import Path

from .policy import WorkspacePolicy


class SearchTool:
    def __init__(self, workspace: WorkspacePolicy, *, max_results: int = 100) -> None:
        self.workspace = workspace
        self.max_results = max_results

    def text(self, query: str, *, path: str | Path = ".") -> list[dict[str, object]]:
        if not query:
            raise ValueError("search query cannot be empty")
        root = self.workspace.resolve_path(path)
        if not root.exists():
            return []
        candidates = [root] if root.is_file() else root.rglob("*")
        results: list[dict[str, object]] = []
        for candidate in candidates:
            if len(results) >= self.max_results:
                break
            if not candidate.is_file() or _ignored(candidate):
                continue
            try:
                lines = candidate.read_text(encoding="utf-8").splitlines()
            except (OSError, UnicodeDecodeError):
                continue
            for line_number, line in enumerate(lines, start=1):
                if query.casefold() in line.casefold():
                    results.append(
                        {
                            "path": candidate.relative_to(self.workspace.root).as_posix(),
                            "line": line_number,
                            "text": line[:500],
                        }
                    )
                    if len(results) >= self.max_results:
                        break
        return results


def _ignored(path: Path) -> bool:
    return any(part in {".git", "__pycache__", ".venv", "node_modules"} for part in path.parts)

