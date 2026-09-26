"""Crash-safe collection of SecondEgo-owned temporary directories."""

from __future__ import annotations

import json
import os
import shutil
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path

try:
    import fcntl
except ImportError:  # pragma: no cover - the supported desktop platforms have fcntl
    fcntl = None


PREFIXES = ("secondego-remote-", "secondego-attempt-")
MARKER = ".secondego-owned"
LOCK = ".secondego-lock"


@dataclass(frozen=True)
class GcConfig:
    temp_root: Path = Path(tempfile.gettempdir())
    retention_seconds: int = 24 * 60 * 60
    max_scan_entries: int = 256
    max_size_bytes: int = 2_000_000_000

    @classmethod
    def from_environment(cls) -> "GcConfig":
        def number(name: str, default: int, low: int, high: int) -> int:
            try:
                return min(max(int(os.environ.get(name, default)), low), high)
            except (TypeError, ValueError):
                return default

        return cls(
            retention_seconds=number("SECONDEGO_GC_RETENTION_SECONDS", 86_400, 0, 30 * 86_400),
            max_scan_entries=number("SECONDEGO_GC_MAX_ENTRIES", 256, 1, 10_000),
            max_size_bytes=number("SECONDEGO_GC_MAX_BYTES", 2_000_000_000, 1_000_000, 100_000_000_000),
        )


@dataclass
class GcReport:
    scanned: int = 0
    deleted: int = 0
    skipped_recent: int = 0
    skipped_active: int = 0
    skipped_unowned: int = 0
    errors: int = 0
    bytes_reclaimed: int = 0


class OwnedTempLease:
    """Hold an OS lock while a temp directory is active."""

    def __init__(self, root: Path, kind: str) -> None:
        self.root = root
        self._handle = None
        root.mkdir(parents=True, exist_ok=True)
        marker = root / MARKER
        marker.write_text(json.dumps({"kind": kind, "pid": os.getpid()}) + "\n", encoding="utf-8")
        handle = (root / LOCK).open("a+")
        if fcntl is not None:
            fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        self._handle = handle

    def cleanup(self, *, remove: bool = True) -> None:
        handle, self._handle = self._handle, None
        if handle is not None:
            if fcntl is not None:
                fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
            handle.close()
        if remove:
            shutil.rmtree(self.root, ignore_errors=True)

    def __del__(self) -> None:  # pragma: no cover - interpreter cleanup fallback
        try:
            self.cleanup()
        except Exception:
            pass


def collect_garbage(config: GcConfig | None = None) -> GcReport:
    config = config or GcConfig.from_environment()
    report = GcReport()
    candidates: list[tuple[Path, float]] = []
    try:
        entries = list(config.temp_root.iterdir())
    except OSError:
        return report
    for path in entries:
        if not path.name.startswith(PREFIXES):
            continue
        try:
            if not path.is_dir() or path.is_symlink():
                continue
            candidates.append((path, path.stat().st_mtime))
        except OSError:
            report.errors += 1
    candidates.sort(key=lambda item: item[1])
    for path, modified in candidates[: config.max_scan_entries]:
        report.scanned += 1
        if time.time() - modified < config.retention_seconds:
            report.skipped_recent += 1
            continue
        if not (path / MARKER).is_file():
            report.skipped_unowned += 1
            continue
        try:
            handle = (path / LOCK).open("a+")
            try:
                if fcntl is not None:
                    try:
                        fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                    except BlockingIOError:
                        report.skipped_active += 1
                        continue
                reclaimed = _directory_size(path, config.max_size_bytes)
                shutil.rmtree(path)
                report.deleted += 1
                report.bytes_reclaimed += reclaimed
            finally:
                if fcntl is not None:
                    fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
                handle.close()
        except OSError:
            report.errors += 1
    return report


def _directory_size(root: Path, limit: int) -> int:
    total = 0
    for path in root.rglob("*"):
        try:
            if path.is_file() and not path.is_symlink():
                total += path.stat().st_size
                if total >= limit:
                    return total
        except OSError:
            continue
    return total
