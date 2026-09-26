# Run State and Resource Policy

SecondEgo treats context and resources as run-scoped state, not as an unbounded chat transcript.

## Retained evidence

Each retained record must have a stable reference, a compact summary, and a source. Source evidence includes repository paths and lines, command invocations, test names, errors, diffs, and tool-result identifiers.

The engine retains active evidence across steps and retries. It marks superseded observations stale rather than repeatedly resending them. A model-written summary is useful only when its underlying evidence record remains available.

## Budgets

Each run declares caps for model calls, tool calls, retries, wall-clock runtime, and per-call context. The engine must reject the next operation before it exceeds a cap and terminate with a resource-evidence reason.

## Context assembly order

1. task and acceptance criteria;
2. current action and plan step;
3. verification and recovery evidence;
4. ranked repository evidence;
5. compact execution state;
6. response reserve.

The context assembler cannot silently truncate required task or verification evidence. It either selects bounded evidence or raises an explicit budget error for the orchestrator to handle.
