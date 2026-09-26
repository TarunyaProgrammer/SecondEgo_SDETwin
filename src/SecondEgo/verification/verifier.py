from dataclasses import dataclass
import re
from typing import Sequence

from SecondEgo.tools.runner import CommandRunner

from .contracts import FailureClass, FailureRecord, VerificationResult


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
                failure_class = _classify_failure(output)
                return (
                    VerificationResult(
                        passed=False,
                        commands=tuple(" ".join(item.command) for item in evidence),
                        failure_class=failure_class,
                        failure_summary=output[:1_000] or "verification command failed",
                        failure_record=_build_failure_record(failure_class, output),
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
    if (
        "timed out" in normalized
        or "not found" in normalized
        or "no module named" in normalized
        or "could not start" in normalized
        or "no such file or directory" in normalized
    ):
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


def _build_failure_record(failure_class: FailureClass, output: str) -> FailureRecord:
    failing_tests = tuple(
        dict.fromkeys(match.group(1) for match in re.finditer(r"(?:FAILED|ERROR)\s+([^\s]+)", output))
    )
    locations = tuple(
        dict.fromkeys(
            f"{match.group(1)}:{match.group(2)}"
            for match in re.finditer(r"([A-Za-z0-9_./-]+\.(?:py|js|ts|tsx|jsx)):(\d+)", output)
        )
    )
    first_line = next((line.strip() for line in output.splitlines() if line.strip()), None)
    return FailureRecord(
        failure_class=failure_class,
        summary=output[:1_000] or "verification command failed",
        failing_tests=failing_tests[:20],
        error_locations=locations[:20],
        fingerprint=first_line[:240] if first_line else None,
    )
