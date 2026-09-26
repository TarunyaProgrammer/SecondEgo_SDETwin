"""Resolve local repository paths and bounded public GitHub sources."""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import urlparse


MAX_CLONE_SECONDS = 180
MAX_CLONE_BYTES = 1_000_000_000


class RepositorySourceError(ValueError):
    """Raised when a repository source cannot be safely acquired."""


@dataclass(frozen=True)
class ResolvedRepository:
    source: str
    root: Path
    cloned: bool = False


def resolve_repository(source: str | Path) -> ResolvedRepository:
    value = str(source).strip()
    if not value:
        raise RepositorySourceError("repository path or GitHub URL is required")
    if _is_github_url(value):
        return _clone_github_repository(value)
    if "://" in value or "github.com" in value:
        raise RepositorySourceError("only HTTPS GitHub repository URLs are supported")
    root = Path(value).expanduser().resolve()
    if not root.is_dir():
        raise RepositorySourceError(f"repository is not an existing directory: {root}")
    return ResolvedRepository(source=value, root=root)


def validate_repository_source(source: str | Path) -> None:
    """Validate a source without cloning a remote repository."""
    value = str(source).strip()
    if not value:
        raise RepositorySourceError("repository path or GitHub URL is required")
    if _is_github_url(value):
        _github_parts(value)
        return
    if "://" in value or "github.com" in value:
        raise RepositorySourceError("only HTTPS GitHub repository URLs are supported")
    root = Path(value).expanduser().resolve()
    if not root.is_dir():
        raise RepositorySourceError(f"repository is not an existing directory: {root}")


def _is_github_url(value: str) -> bool:
    parsed = urlparse(value)
    return parsed.scheme == "https" and parsed.hostname in {"github.com", "www.github.com"}


def _clone_github_repository(url: str) -> ResolvedRepository:
    owner, repository = _github_parts(url)

    git = shutil.which("git")
    if git is None:
        raise RepositorySourceError("git is required to clone a remote repository")
    clone_root = Path(tempfile.mkdtemp(prefix="secondego-remote-"))
    target = clone_root / repository
    clone_url = f"https://github.com/{owner}/{repository}.git"
    try:
        completed = subprocess.run(
            (
                git,
                "-c",
                "core.hooksPath=/dev/null",
                "clone",
                "--depth",
                "1",
                "--no-tags",
                "--single-branch",
                clone_url,
                str(target),
            ),
            capture_output=True,
            text=True,
            timeout=MAX_CLONE_SECONDS,
            check=False,
            env={**os.environ, "GIT_TERMINAL_PROMPT": "0"},
        )
    except subprocess.TimeoutExpired as exc:
        shutil.rmtree(clone_root, ignore_errors=True)
        raise RepositorySourceError("GitHub clone timed out after 180 seconds") from exc
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout).strip().splitlines()
        reason = detail[-1][:300] if detail else "git clone failed"
        shutil.rmtree(clone_root, ignore_errors=True)
        raise RepositorySourceError(f"GitHub clone failed: {reason}")
    if not target.is_dir() or not (target / ".git").exists():
        shutil.rmtree(clone_root, ignore_errors=True)
        raise RepositorySourceError("GitHub clone did not produce a Git repository")
    if _directory_size(target) > MAX_CLONE_BYTES:
        shutil.rmtree(clone_root, ignore_errors=True)
        raise RepositorySourceError("cloned repository exceeds the 1 GB safety limit")
    return ResolvedRepository(source=url, root=target.resolve(), cloned=True)


def _github_parts(url: str) -> tuple[str, str]:
    parsed = urlparse(url)
    if parsed.username or parsed.password or parsed.query or parsed.fragment or parsed.port:
        raise RepositorySourceError("GitHub URL must not contain credentials, query parameters, or a port")
    parts = [part for part in parsed.path.split("/") if part]
    if len(parts) != 2:
        raise RepositorySourceError("GitHub URL must have the form https://github.com/owner/repository")
    owner, repository = parts
    if repository.endswith(".git"):
        repository = repository[:-4]
    if not owner or not repository or any(part in {".", ".."} for part in (owner, repository)):
        raise RepositorySourceError("GitHub URL contains an invalid owner or repository")
    return owner, repository


def _directory_size(root: Path) -> int:
    total = 0
    for path in root.rglob("*"):
        if path.is_file() and not path.is_symlink():
            total += path.stat().st_size
            if total > MAX_CLONE_BYTES:
                return total
    return total
