---
name: SecondEgo-orchestration
description: Design or implement the SecondEgo state machine, tool routing, execution loop, verification, failure diagnosis, and bounded recovery.
---

# SecondEgo orchestration

Use this skill when work involves the agent loop or the transitions between understanding, exploration, planning, execution, verification, and recovery.

## Required behavior

- Model phases and transitions explicitly; do not hide the entire system inside an unconstrained ReAct loop.
- Allow model autonomy through validated action proposals, policy checks, tool routing, and observations.
- Keep tools small, composable, observable, and permission-bounded. Start with repository tree, search, read, symbol inspection, edit, command execution, tests, and diff inspection.
- Validate tool arguments and outputs. Capture exit status, stdout/stderr, changed paths, and timing.
- Treat tests, lint, builds, and diffs as verification evidence, not as optional narration.
- On failure, classify the failure before retrying. Bound retries, vary the recovery strategy, and terminate with a useful reason when progress is not justified.
- Make idempotence, cancellation, timeouts, and partial changes explicit for every mutating tool.

## Minimum observable run

`task → understand → explore → plan → execute → verify → complete`

Failure path:

`verify → diagnose → recover → execute`, with a bounded retry policy and an explicit blocked/failure outcome.

