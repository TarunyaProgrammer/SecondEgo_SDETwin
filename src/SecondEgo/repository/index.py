import ast
from dataclasses import dataclass
from pathlib import Path

from SecondEgo.tools.policy import WorkspacePolicy

from .scanner import RepositoryScanner, RepositorySnapshot


@dataclass(frozen=True)
class Symbol:
    name: str
    kind: str
    path: str
    line_start: int
    line_end: int


@dataclass(frozen=True)
class ImportEdge:
    source_path: str
    module: str
    line: int


@dataclass(frozen=True)
class ParserFailure:
    path: str
    message: str
    line: int | None


@dataclass(frozen=True)
class RepositoryIndex:
    snapshot: RepositorySnapshot
    symbols: tuple[Symbol, ...]
    imports: tuple[ImportEdge, ...]
    parser_failures: tuple[ParserFailure, ...]


class RepositoryIndexer:
    """Derived Python metadata; unsupported languages remain searchable via file paths."""

    def __init__(self, workspace: WorkspacePolicy, *, scanner: RepositoryScanner | None = None) -> None:
        self.workspace = workspace
        self.scanner = scanner or RepositoryScanner(workspace)

    def build(self) -> RepositoryIndex:
        snapshot = self.scanner.scan()
        symbols: list[Symbol] = []
        imports: list[ImportEdge] = []
        failures: list[ParserFailure] = []
        for path in snapshot.files:
            if not path.endswith(".py"):
                continue
            absolute_path = self.workspace.resolve_path(path)
            try:
                tree = ast.parse(absolute_path.read_text(encoding="utf-8"), filename=path)
            except (OSError, UnicodeDecodeError, SyntaxError) as exc:
                failures.append(
                    ParserFailure(
                        path=path,
                        message=str(exc),
                        line=getattr(exc, "lineno", None),
                    )
                )
                continue
            for node in ast.walk(tree):
                if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    symbols.append(
                        Symbol(
                            name=node.name,
                            kind="function",
                            path=path,
                            line_start=node.lineno,
                            line_end=getattr(node, "end_lineno", node.lineno),
                        )
                    )
                elif isinstance(node, ast.ClassDef):
                    symbols.append(
                        Symbol(
                            name=node.name,
                            kind="class",
                            path=path,
                            line_start=node.lineno,
                            line_end=getattr(node, "end_lineno", node.lineno),
                        )
                    )
                elif isinstance(node, ast.Import):
                    for alias in node.names:
                        imports.append(ImportEdge(path, alias.name, node.lineno))
                elif isinstance(node, ast.ImportFrom):
                    imports.append(ImportEdge(path, node.module or "", node.lineno))
        return RepositoryIndex(
            snapshot=snapshot,
            symbols=tuple(sorted(symbols, key=lambda item: (item.path, item.line_start, item.name))),
            imports=tuple(sorted(imports, key=lambda item: (item.source_path, item.line, item.module))),
            parser_failures=tuple(sorted(failures, key=lambda item: item.path)),
        )

