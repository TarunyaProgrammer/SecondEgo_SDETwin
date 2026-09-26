import re
from dataclasses import dataclass

from .index import RepositoryIndex


@dataclass(frozen=True)
class RankedFile:
    path: str
    score: int
    reasons: tuple[str, ...]
    confidence: float = 1.0


class RepositoryRetriever:
    """Explainable lexical/symbol retrieval; graph ranking can be added without changing callers."""

    def rank(
        self,
        index: RepositoryIndex,
        query: str,
        *,
        limit: int = 12,
        mode: str = "issue",
    ) -> tuple[RankedFile, ...]:
        terms = tuple(term.casefold() for term in re.findall(r"[A-Za-z_][A-Za-z0-9_]*", query))
        scores: dict[str, int] = {}
        reasons: dict[str, list[str]] = {}
        symbols_by_path: dict[str, list[str]] = {}
        for symbol in index.symbols:
            symbols_by_path.setdefault(symbol.path, []).append(symbol.name.casefold())

        for path in index.snapshot.files:
            lowered_path = path.casefold()
            for term in terms:
                if term in lowered_path:
                    scores[path] = scores.get(path, 0) + 3
                    reasons.setdefault(path, []).append(f"path:{term}")
                if any(term in symbol for symbol in symbols_by_path.get(path, [])):
                    scores[path] = scores.get(path, 0) + 5
                    reasons.setdefault(path, []).append(f"symbol:{term}")
            if path in index.snapshot.test_files and any(term in {"test", "bug", "fix", "regression"} for term in terms):
                scores[path] = scores.get(path, 0) + 1
                reasons.setdefault(path, []).append("test-relevance")

        links_by_target: dict[str, list[str]] = {}
        links_by_test: dict[str, list[str]] = {}
        for link in index.test_links:
            links_by_target.setdefault(link.target_path, []).append(link.test_path)
            links_by_test.setdefault(link.test_path, []).append(link.target_path)
        for target_path, test_paths in links_by_target.items():
            if any(any(term in test_path.casefold() for term in terms) for test_path in test_paths):
                scores[target_path] = scores.get(target_path, 0) + 7
                reasons.setdefault(target_path, []).append("failure-linked-test")
            if mode == "failure" and any(term in {"failure", "failed", "error", "assert"} for term in terms):
                scores[target_path] = scores.get(target_path, 0) + 2
                reasons.setdefault(target_path, []).append("test-topology")
        for test_path, target_paths in links_by_test.items():
            if any(term in test_path.casefold() for term in terms):
                scores[test_path] = scores.get(test_path, 0) + 8
                reasons.setdefault(test_path, []).append("failure-test")
                for target_path in target_paths:
                    reasons.setdefault(target_path, []).append("linked-from-failure-test")

        # Multi-file dependency & blast radius propagation across import graph
        module_to_file: dict[str, str] = {}
        for path in index.snapshot.files:
            if path.endswith(".py"):
                module = path[:-3].replace("/", ".").removesuffix(".__init__")
                module_to_file[module] = path
                base = module.split(".")[-1]
                module_to_file.setdefault(base, path)

        scored_paths = [p for p, s in scores.items() if s >= 3]
        for scored_path in scored_paths:
            for edge in index.imports:
                target_file = module_to_file.get(edge.module)
                if target_file == scored_path and edge.source_path != scored_path:
                    scores[edge.source_path] = scores.get(edge.source_path, 0) + 4
                    reasons.setdefault(edge.source_path, []).append(f"importer-of:{scored_path}")
                elif edge.source_path == scored_path and target_file and target_file != scored_path:
                    scores[target_file] = scores.get(target_file, 0) + 3
                    reasons.setdefault(target_file, []).append(f"dependency-of:{scored_path}")

        ranked = [
            RankedFile(
                path=path,
                score=score,
                reasons=tuple(dict.fromkeys(reasons[path])),
                confidence=0.95 if any("test" in reason for reason in reasons[path]) else 0.6,
            )
            for path, score in scores.items()
            if score > 0
        ]
        return tuple(sorted(ranked, key=lambda item: (-item.score, item.path))[:limit])
