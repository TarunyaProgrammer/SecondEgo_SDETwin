---
name: second-ego-verification
description: Plan and review SecondEgo verification, test feedback, diffs, telemetry, replayability, and explicit termination evidence.
---

# SecondEgo verification

Use this skill when implementing or reviewing tests, validation, run reports, telemetry, recovery outcomes, or completion criteria.

## Rules

- Define success criteria before execution and verify them with commands or other observable evidence.
- Capture test/lint/build commands, exit codes, duration, relevant output, changed paths, and final diff summary.
- Separate model claims from tool evidence. A generated explanation is not proof that code works.
- Classify failures before retrying: environment, tool contract, repository state, implementation, test, or model-planning failure.
- Bound retries and vary the recovery action; stop with a useful blocked/failure reason when progress is not justified.
- Make runs reproducible where possible: record configuration, provider/model identity, repository revision, and safe tool inputs.

