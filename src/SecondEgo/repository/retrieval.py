import re
from dataclasses import dataclass

from .index import RepositoryIndex


@dataclass(frozen=True)
class RankedFile:
    path: str
    score: int
    reasons: tuple[str, ...]


class RepositoryRetriever:
    """Explainable lexical/symbol retrieval; graph ranking can be added without changing callers."""

    def rank(self, index: RepositoryIndex, query: str, *, limit: int = 12) -> tuple[RankedFile, ...]:
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

        ranked = [
            RankedFile(path=path, score=score, reasons=tuple(dict.fromkeys(reasons[path])))
            for path, score in scores.items()
            if score > 0
        ]
        return tuple(sorted(ranked, key=lambda item: (-item.score, item.path))[:limit])

