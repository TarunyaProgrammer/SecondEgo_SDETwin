from pathlib import Path

from SecondEgo.core.resources import ResourceBudget, ResourceUsage
from SecondEgo.model.gemini import GeminiProvider
from SecondEgo.model.planner import ModelPlanner
from SecondEgo.orchestration.engine import HarnessEngine
from SecondEgo.repository.scanner import RepositoryScanner
from SecondEgo.storage.sqlite import SQLiteRunStore
from SecondEgo.tools.filesystem import FileTool
from SecondEgo.tools.git import GitTool
from SecondEgo.tools.policy import WorkspacePolicy
from SecondEgo.tools.router import ToolRouter
from SecondEgo.tools.runner import CommandRunner
from SecondEgo.tools.search import SearchTool
from SecondEgo.verification.verifier import VerificationEngine
from SecondEgo.context.assembler import ContextAssembler
from SecondEgo.context.policy import ContextBudget


def build_engine(
    repository: Path,
    *,
    resources: ResourceUsage,
    state_db: Path | None = None,
) -> HarnessEngine:
    workspace = WorkspacePolicy(repository)
    runner = CommandRunner(workspace)
    git = GitTool(runner)
    return HarnessEngine(
        scanner=RepositoryScanner(workspace),
        router=ToolRouter(
            files=FileTool(workspace),
            search=SearchTool(workspace),
            runner=runner,
            resources=resources,
            git=git,
        ),
        verifier=VerificationEngine(runner),
        resources=resources,
        store=SQLiteRunStore(state_db) if state_db else None,
        git=git,
    )


def build_gemini_planner(resources: ResourceUsage, model: str) -> ModelPlanner:
    return ModelPlanner(
        provider=GeminiProvider(model=model),
        assembler=ContextAssembler(ContextBudget(24_000, 4_000, 2_000, 12_000, 3_000, 3_000)),
        resources=resources,
    )


def build_resources(*, max_model_calls: int = 20, max_tool_calls: int = 80, max_retries: int = 6) -> ResourceUsage:
    return ResourceUsage(
        ResourceBudget(
            max_model_calls=max_model_calls,
            max_tool_calls=max_tool_calls,
            max_retries=max_retries,
        )
    )
