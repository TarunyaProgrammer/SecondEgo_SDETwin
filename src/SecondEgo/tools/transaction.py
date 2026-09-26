from __future__ import annotations

import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

from .policy import WorkspacePolicy


class TransactionBlocked(RuntimeError):
    """Raised when an attempt cannot be isolated without risking repository state."""


@dataclass(frozen=True)
class TransactionResult:
    passed: bool
    transferred: bool
    changed_paths: tuple[str, ...] = ()
    reason: str = ""


class GitAttemptTransaction:
    """Run one coding attempt in a detached worktree and transfer only a passing diff."""

    def __init__(
        self,
        workspace: WorkspacePolicy,
        *,
        max_patch_bytes: int = 2_000_000,
        max_file_bytes: int = 512_000,
    ) -> None:
        self.workspace = workspace
        self.max_patch_bytes = max_patch_bytes
        self.max_file_bytes = max_file_bytes
        self._state_dir: Path | None = None
        self._attempt_root: Path | None = None

    @property
    def active(self) -> bool:
        return self._attempt_root is not None

    def begin(self) -> WorkspacePolicy:
        if self.active:
            raise TransactionBlocked("an attempt transaction is already active")
        self._require_git_repository()
        status = _run_git(
            ("status", "--porcelain", "--untracked-files=all"),
            self.workspace.root,
        )
        if status.stdout:
            raise TransactionBlocked("transaction requires a clean target repository")
        head = _run_git(("rev-parse", "--verify", "HEAD"), self.workspace.root)
        if not head.stdout.strip():
            raise TransactionBlocked("transaction requires a repository with an initial commit")

        state_dir = Path(tempfile.mkdtemp(prefix="secondego-attempt-"))
        attempt_root = state_dir / "workspace"
        try:
            _run_git(
                ("worktree", "add", "--detach", str(attempt_root), "HEAD"),
                self.workspace.root,
            )
        except Exception:
            shutil.rmtree(state_dir, ignore_errors=True)
            raise
        self._state_dir = state_dir
        self._attempt_root = attempt_root
        return WorkspacePolicy(attempt_root)

    def finish(self, *, passed: bool) -> TransactionResult:
        attempt_root = self._attempt_root
        state_dir = self._state_dir
        if attempt_root is None or state_dir is None:
            raise TransactionBlocked("no active attempt transaction")
        try:
            changed_paths = self._changed_paths(attempt_root)
            if not passed:
                return TransactionResult(
                    passed=False,
                    transferred=False,
                    changed_paths=changed_paths,
                    reason="failed attempt discarded",
                )
            patch = _run_git(
                ("diff", "--binary", "--no-ext-diff", "HEAD"),
                attempt_root,
            ).stdout
            if len(patch.encode("utf-8")) > self.max_patch_bytes:
                raise TransactionBlocked("attempt diff exceeds transfer limit")
            if patch:
                _apply_patch(self.workspace.root, patch)
            for relative in _untracked_paths(attempt_root):
                self._transfer_new_file(attempt_root, relative)
            return TransactionResult(
                passed=True,
                transferred=True,
                changed_paths=changed_paths,
                reason="verified attempt transferred to target workspace",
            )
        finally:
            self._cleanup(attempt_root, state_dir)

    def abort(self) -> None:
        if self._attempt_root is not None and self._state_dir is not None:
            self._cleanup(self._attempt_root, self._state_dir)

    def _require_git_repository(self) -> None:
        result = subprocess.run(
            ("git", "rev-parse", "--show-toplevel"),
            cwd=self.workspace.root,
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        if result.returncode != 0:
            raise TransactionBlocked("target workspace is not a Git repository")
        repository_root = Path(result.stdout.strip()).resolve()
        if repository_root != self.workspace.root:
            raise TransactionBlocked("workspace must be the Git repository root")

    def _changed_paths(self, attempt_root: Path) -> tuple[str, ...]:
        return tuple(sorted(set(_status_paths(attempt_root))))

    def _transfer_new_file(self, attempt_root: Path, relative: str) -> None:
        source = _safe_child(attempt_root, relative)
        destination = _safe_child(self.workspace.root, relative)
        if source.is_symlink() or not source.is_file():
            raise TransactionBlocked(f"cannot transfer non-regular new path: {relative}")
        if source.stat().st_size > self.max_file_bytes:
            raise TransactionBlocked(f"new file exceeds transfer limit: {relative}")
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)

    def _cleanup(self, attempt_root: Path, state_dir: Path) -> None:
        try:
            _run_git(("worktree", "remove", "--force", str(attempt_root)), self.workspace.root)
        finally:
            shutil.rmtree(state_dir, ignore_errors=True)
            self._attempt_root = None
            self._state_dir = None


def _run_git(argv: tuple[str, ...], cwd: Path) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        ("git", *argv),
        cwd=cwd,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    if result.returncode != 0:
        message = result.stderr.strip() or result.stdout.strip() or "git command failed"
        raise TransactionBlocked(message)
    return result


def _apply_patch(workspace: Path, patch: str) -> None:
    result = subprocess.run(
        ("git", "apply", "--binary", "--whitespace=nowarn", "-"),
        cwd=workspace,
        input=patch,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    if result.returncode != 0:
        message = result.stderr.strip() or "verified diff could not be applied"
        raise TransactionBlocked(message)


def _status_paths(workspace: Path) -> list[str]:
    result = _run_git(
        ("status", "--porcelain", "--untracked-files=all"),
        workspace,
    )
    paths: list[str] = []
    for line in result.stdout.splitlines():
        if len(line) < 4:
            continue
        value = line[3:]
        if " -> " in value:
            value = value.rsplit(" -> ", 1)[1]
        paths.append(value)
    return paths


def _untracked_paths(workspace: Path) -> tuple[str, ...]:
    result = _run_git(
        ("ls-files", "--others", "--exclude-standard", "-z"),
        workspace,
    )
    return tuple(item for item in result.stdout.split("\0") if item)


def _safe_child(root: Path, relative: str) -> Path:
    candidate = (root / relative).resolve(strict=False)
    try:
        candidate.relative_to(root.resolve())
    except ValueError as exc:
        raise TransactionBlocked(f"transaction path escapes workspace: {relative}") from exc
    return candidate
