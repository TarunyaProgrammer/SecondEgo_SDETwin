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
class TestNode:
    path: str
    name: str
    line: int


@dataclass(frozen=True)
class TestLink:
    test_path: str
    target_path: str
    reason: str
    confidence: float


@dataclass(frozen=True)
class RepositoryIndex:
    snapshot: RepositorySnapshot
    symbols: tuple[Symbol, ...]
    imports: tuple[ImportEdge, ...]
    parser_failures: tuple[ParserFailure, ...]
    tests: tuple[TestNode, ...] = ()
    test_links: tuple[TestLink, ...] = ()


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
        tests: list[TestNode] = []
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
                if (
                    path in snapshot.test_files
                    and isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
                    and node.name.startswith("test")
                ):
                    tests.append(TestNode(path=path, name=node.name, line=node.lineno))
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
        test_links = _link_tests(snapshot, imports)
        return RepositoryIndex(
            snapshot=snapshot,
            symbols=tuple(sorted(symbols, key=lambda item: (item.path, item.line_start, item.name))),
            imports=tuple(sorted(imports, key=lambda item: (item.source_path, item.line, item.module))),
            parser_failures=tuple(sorted(failures, key=lambda item: item.path)),
            tests=tuple(sorted(tests, key=lambda item: (item.path, item.line, item.name))),
            test_links=tuple(
                sorted(
                    test_links,
                    key=lambda item: (item.test_path, item.target_path, item.reason),
                )
            ),
        )


def _link_tests(snapshot: RepositorySnapshot, imports: list[ImportEdge]) -> list[TestLink]:
    module_paths: dict[str, str] = {}
    for path in snapshot.files:
        if not path.endswith(".py"):
            continue
        without_suffix = path[:-3].replace("/", ".")
        if without_suffix.endswith(".__init__"):
            without_suffix = without_suffix[:-9]
        module_paths[without_suffix] = path

    links: list[TestLink] = []
    for edge in imports:
        if edge.source_path not in snapshot.test_files:
            continue
        target = module_paths.get(edge.module)
        if target is None:
            candidates = [
                path
                for module, path in module_paths.items()
                if module.endswith(f".{edge.module}") or edge.module.endswith(f".{module}")
            ]
            if len(candidates) == 1:
                target = candidates[0]
        if target is not None and target not in snapshot.test_files:
            links.append(
                TestLink(
                    test_path=edge.source_path,
                    target_path=target,
                    reason="test_import",
                    confidence=0.95,
                )
            )
    return list({(item.test_path, item.target_path, item.reason): item for item in links}.values())
