import json
import shutil
import subprocess
import sys
from pathlib import Path

from SecondEgo.app import build_engine, build_resources
from SecondEgo.core.state import AcceptanceCriterion
from SecondEgo.model.base import ActionProposal


def test_pagination_fixture_runs_through_transactional_engine(tmp_path) -> None:
    repository = tmp_path / "pagination-fixture"
    source = Path(__file__).parents[1] / "evaluation" / "fixture_repo"
    shutil.copytree(source, repository)
    _initialize_git(repository)

    plan = json.loads(
        (Path(__file__).parents[1] / "evaluation" / "fixture_plan.json").read_text(
            encoding="utf-8"
        )
    )
    engine = build_engine(repository, resources=build_resources())
    result = engine.run(
        task="Fix one-based pagination",
        acceptance_criteria=tuple(AcceptanceCriterion(item) for item in plan["acceptance_criteria"]),
        actions=tuple(
            ActionProposal(
                item["action"], item.get("arguments", {}), item.get("rationale", "")
            )
            for item in plan["actions"]
        ),
        verification_commands=((sys.executable, "-m", "pytest", "-q"),),
    )

    assert result.state.status.value == "COMPLETE"
    assert result.verification.passed is True
    assert "src/pagination.py" in result.state.changed_paths
    assert "start = (page - 1) * page_size" in (repository / "src/pagination.py").read_text()


def _initialize_git(repository: Path) -> None:
    subprocess.run(("git", "init", "-q"), cwd=repository, check=True)
    subprocess.run(("git", "config", "user.email", "test@example.com"), cwd=repository, check=True)
    subprocess.run(("git", "config", "user.name", "SecondEgo Test"), cwd=repository, check=True)
    subprocess.run(("git", "add", "."), cwd=repository, check=True)
    subprocess.run(
        ("git", "-c", "commit.gpgSign=false", "commit", "-qm", "baseline"),
        cwd=repository,
        check=True,
    )
