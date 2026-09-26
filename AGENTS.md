# AGENTS.md

## Project purpose

This repository is **SecondEgo**: a local desktop coding-agent application whose core is an autonomous coding harness. The current design sources are [`CONTEXT-1.md`](docs/context/CONTEXT-1.md) and [`CONTEXT-2.md`](docs/context/CONTEXT-2.md). They describe the target architecture; they are not permission to implement product code or to assume that every proposed technology is final.

The working architectural direction is a Python-based, model-provider-agnostic engine behind a desktop shell with:

- repository intelligence and focused context retrieval;
- a hierarchical state machine for understand, explore, plan, execute, verify, diagnose, and recover;
- a small, high-value tool surface;
- explicit state, bounded context, telemetry, and test-driven verification.
- an Electron + React pixel-village interface that visualizes engine state rather than containing core agent logic;
- SQLite for local, rebuildable execution state and repository metadata;
- a local IPC/API boundary between the desktop UI and Python engine.

## Scope discipline

- Do not add product or runtime code unless the user explicitly asks for implementation.
- Treat `docs/context/CONTEXT-1.md` as a design proposal. Re-check assumptions before turning it into interfaces or dependencies.
- Treat `docs/context/CONTEXT-2.md` as the product-shell proposal. SecondEgo is the product name; do not rename it to Forge or another working title.
- Prefer the smallest architecture that satisfies the evaluation rubric. Do not add services such as Neo4j, queues, or multi-agent layers without a demonstrated need.
- Keep model access behind a provider interface. Never scatter provider-specific calls through orchestration code.
- Keep repository changes reviewable and avoid unrelated formatting or dependency changes.

## Working rules for agents

1. Inspect the repository and relevant task context before editing.
2. State assumptions when requirements are ambiguous; do not silently invent behavior.
3. Plan around observable outcomes: tool calls, state transitions, diffs, tests, and termination reasons.
4. Make narrow changes, then run the most relevant checks.
5. Never claim a task is verified when tests, lint, or required checks were not run.
6. Treat command execution, file edits, and model output as untrusted inputs that need validation and bounded permissions.
7. Preserve user changes. Do not reset, clean, or overwrite unrelated work.
8. Do not commit secrets, API keys, credentials, generated private data, or repository snapshots.

## Local agent skills

Project-specific Antigravity skills live under [`.agent/skills/`](.agent/skills/):

- [`second-ego-repository-intelligence`](.agent/skills/second-ego-repository-intelligence/SKILL.md): scanning, symbols, dependencies, tests, and retrieval.
- [`second-ego-context-management`](.agent/skills/second-ego-context-management/SKILL.md): context budgets, compression, retention, and evidence.
- [`second-ego-orchestration`](.agent/skills/second-ego-orchestration/SKILL.md): state-machine execution, tool routing, verification, and recovery.
- [`second-ego-desktop`](.agent/skills/second-ego-desktop/SKILL.md): desktop-shell boundaries and local IPC.
- [`second-ego-python-engine`](.agent/skills/second-ego-python-engine/SKILL.md): Python engine structure and provider abstraction.
- [`second-ego-ui-state`](.agent/skills/second-ego-ui-state/SKILL.md): pixel-village state visualization and UI evidence.
- [`second-ego-verification`](.agent/skills/second-ego-verification/SKILL.md): tests, diffs, telemetry, and termination evidence.
- [`second-ego-security`](.agent/skills/second-ego-security/SKILL.md): local-tool security, secrets, permissions, and sandboxing.

Use only the skill relevant to the task. These skills guide decisions; they do not authorize implementation beyond the user's request.

## Documentation map

- [`docs/context/CONTEXT.md`](docs/context/CONTEXT.md): concise, current project context and decision log.
- [`docs/context/CONTEXT-1.md`](docs/context/CONTEXT-1.md): initial architecture proposal and rationale.
- [`docs/context/CONTEXT-2.md`](docs/context/CONTEXT-2.md): SecondEgo desktop product-shell and end-to-end experience proposal.
- [`docs/context/ContextSubmission.md`](docs/context/ContextSubmission.md): evaluator interface and submission requirements.
- [`CONTRIBUTING.md`](CONTRIBUTING.md): contribution workflow and quality bar.
- [`SECURITY.md`](SECURITY.md): vulnerability reporting and security expectations.
- [`LICENSE`](LICENSE): MIT license.
