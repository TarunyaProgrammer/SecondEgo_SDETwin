from dataclasses import dataclass
from pathlib import Path

from SecondEgo.tools.policy import WorkspacePolicy


_IGNORED_DIRECTORIES = frozenset({".git", ".venv", "node_modules", "__pycache__", "dist", "build"})
_MANIFESTS = frozenset({"pyproject.toml", "package.json", "pytest.ini", "tox.ini", "setup.cfg"})


@dataclass(frozen=True)
class RepositorySnapshot:
    root: str
    files: tuple[str, ...]
    manifests: tuple[str, ...]
    test_files: tuple[str, ...]


class RepositoryScanner:
    """Cheap structural evidence; parsing and graph extraction are later layers."""

    def __init__(self, workspace: WorkspacePolicy, *, max_files: int = 20_000) -> None:
        self.workspace = workspace
        self.max_files = max_files

    def scan(self) -> RepositorySnapshot:
        files: list[str] = []
        manifests: list[str] = []
        test_files: list[str] = []
        for path in self.workspace.root.rglob("*"):
            if len(files) >= self.max_files:
                break
            relative = path.relative_to(self.workspace.root)
            if any(part in _IGNORED_DIRECTORIES for part in relative.parts):
                continue
            if not path.is_file():
                continue
            normalized = relative.as_posix()
            files.append(normalized)
            if path.name in _MANIFESTS:
                manifests.append(normalized)
            if _is_test_file(relative):
                test_files.append(normalized)
        return RepositorySnapshot(
            root=str(self.workspace.root),
            files=tuple(sorted(files)),
            manifests=tuple(sorted(manifests)),
            test_files=tuple(sorted(test_files)),
        )


def _is_test_file(path: Path) -> bool:
    name = path.name.lower()
    supported_suffixes = {".py", ".js", ".jsx", ".ts", ".tsx"}
    if path.suffix.lower() not in supported_suffixes:
        return False
    return (
        name.startswith("test_")
        or name.endswith("_test.py")
        or ".test." in name
        or ".spec." in name
        or "tests" in {part.lower() for part in path.parts}
    )
