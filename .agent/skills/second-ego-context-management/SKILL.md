---
name: SecondEgo-context-management
description: Plan and review bounded context assembly, compression, retention, and evidence handling for autonomous coding-agent runs.
---

# SecondEgo context management

Use this skill when designing prompts, execution state, memory, retrieval, summaries, or token budgets.

## Required behavior

- Define a context budget before retrieval and reserve space for the current task, tool results, plan, verification evidence, and recovery state.
- Prefer compact structured state over repeated prose. Store durable facts, decisions, open questions, and failed approaches separately.
- Compress only after preserving actionable evidence: file paths, symbols, commands, errors, diffs, test results, and confidence.
- Remove stale or redundant context instead of allowing unbounded growth.
- Make retention rules explicit: what survives a step, a retry, a phase transition, and a new task.
- Never treat a model-generated summary as authoritative without retaining a path back to the source evidence.

## Review questions

- Is the supplied context relevant to the current action?
- What information was dropped, and could that omission change the decision?
- Can the agent recover after a failed command or truncated response?
- Is the termination decision based on evidence rather than prompt length or iteration count alone?

