# Contributing

Thanks for considering a contribution to SecondEgo. The project is early-stage, so design clarity and scope discipline matter more than adding surface area.

## Before opening a change

1. Read [`AGENTS.md`](AGENTS.md) and [`CONTEXT.md`](docs/context/CONTEXT.md).
2. Check existing issues and open work to avoid duplicating effort.
3. For architectural changes, explain the problem, alternatives considered, and the evidence that justifies the added complexity.
4. Do not include secrets, credentials, generated private data, or unrelated formatting changes.

## Pull requests

A pull request should state:

- what changed and why;
- what was intentionally not changed;
- how the change was verified;
- any remaining risks or follow-up work.

Keep commits focused. New runtime behavior must include appropriate tests and observable failure handling. Documentation-only changes should not invent implementation details.

## Design expectations

Prefer provider abstraction, bounded context, explicit state transitions, narrow tools, local/rebuildable metadata, and evidence-backed termination. Challenge complexity that does not improve task completion or evaluation outcomes.

## Code of conduct

Participation is governed by [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
