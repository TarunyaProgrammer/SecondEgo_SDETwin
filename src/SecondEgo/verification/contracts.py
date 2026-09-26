from dataclasses import dataclass, field
from enum import StrEnum


class FailureClass(StrEnum):
    NONE = "NONE"
    TEST_FAILURE = "TEST_FAILURE"
    BUILD_FAILURE = "BUILD_FAILURE"
    LINT_FAILURE = "LINT_FAILURE"
    TYPE_ERROR = "TYPE_ERROR"
    RUNTIME_ERROR = "RUNTIME_ERROR"
    TOOL_FAILURE = "TOOL_FAILURE"
    ENVIRONMENT_FAILURE = "ENVIRONMENT_FAILURE"
    MODEL_PLANNING_FAILURE = "MODEL_PLANNING_FAILURE"
    REGRESSION = "REGRESSION"
    UNKNOWN = "UNKNOWN"


@dataclass(frozen=True)
class FailureRecord:
    failure_class: FailureClass
    summary: str
    failing_tests: tuple[str, ...] = ()
    error_locations: tuple[str, ...] = ()
    fingerprint: str | None = None
    changed_paths: tuple[str, ...] = ()


@dataclass(frozen=True)
class VerificationResult:
    passed: bool
    commands: tuple[str, ...] = ()
    passed_tests: int = 0
    failed_tests: int = 0
    failure_class: FailureClass = FailureClass.NONE
    failure_summary: str | None = None
    failure_record: FailureRecord | None = None
    evidence_refs: tuple[str, ...] = ()


@dataclass(frozen=True)
class TerminationDecision:
    status: str
    reason: str
    evidence_refs: tuple[str, ...] = field(default_factory=tuple)
