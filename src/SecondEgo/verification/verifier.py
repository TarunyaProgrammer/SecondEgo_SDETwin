from dataclasses import dataclass
from typing import Sequence

from SecondEgo.tools.runner import CommandRunner

from .contracts import FailureClass, VerificationResult


@dataclass(frozen=True)
class VerificationEvidence:
    command: tuple[str, ...]
    success: bool
    exit_code: int | None
    duration_ms: int
    output: str


class VerificationEngine:
    def __init__(self, runner: CommandRunner) -> None:
        self.runner = runner

    def run(self, commands: Sequence[Sequence[str]]) -> tuple[VerificationResult, tuple[VerificationEvidence, ...]]:
        if not commands:
            return (
                VerificationResult(
                    passed=False,
                    failure_class=FailureClass.ENVIRONMENT_FAILURE,
                    failure_summary="no verification commands configured",
                ),
                (),
            )
        evidence: list[VerificationEvidence] = []
        for command in commands:
            result = self.runner.run(command)
            output = "\n".join(part for part in (result.stdout, result.stderr) if part)
            evidence.append(
                VerificationEvidence(
                    command=tuple(command),
                    success=result.success,
                    exit_code=result.exit_code,
                    duration_ms=result.duration_ms,
                    output=output[:4_000],
                )
            )
            if not result.success:
                return (
                    VerificationResult(
                        passed=False,
                        commands=tuple(" ".join(item.command) for item in evidence),
                        failure_class=_classify_failure(output),
                        failure_summary=output[:1_000] or "verification command failed",
                    ),
                    tuple(evidence),
                )
        return (
            VerificationResult(
                passed=True,
                commands=tuple(" ".join(item.command) for item in evidence),
                evidence_refs=tuple(f"verification:{index}" for index in range(len(evidence))),
            ),
            tuple(evidence),
        )


def _classify_failure(output: str) -> FailureClass:
    normalized = output.casefold()
    if "timed out" in normalized or "not found" in normalized or "no module named" in normalized:
        return FailureClass.ENVIRONMENT_FAILURE
    if "syntaxerror" in normalized or "syntax error" in normalized:
        return FailureClass.BUILD_FAILURE
    if "mypy" in normalized or "type error" in normalized:
        return FailureClass.TYPE_ERROR
    if "lint" in normalized or "ruff" in normalized:
        return FailureClass.LINT_FAILURE
    if "failed" in normalized or "assert" in normalized:
        return FailureClass.TEST_FAILURE
    return FailureClass.UNKNOWN

