# SecondEgo — Autonomous Coding Agent Application

> A coding harness built for reliable autonomous software engineering.

SecondEgo is a local desktop application whose Python engine turns a foundation model into a reliable software engineer through repository intelligence, structured planning, adaptive tool orchestration, bounded context management, failure recovery, and test-driven verification. An Electron + React pixel-village shell visualizes the engine; it does not replace it.

## Status

This repository is currently in the architecture and project-scaffolding phase. The implementation has not started. [`CONTEXT-1.md`](CONTEXT-1.md) contains the harness proposal, [`CONTEXT-2.md`](CONTEXT-2.md) contains the desktop product-shell proposal, and [`CONTEXT.md`](CONTEXT.md) records current decisions and open questions.

## Design direction

The proposed SecondEgo architecture uses a model-provider adapter, a repository intelligence layer, a stateful Python execution engine, local SQLite state, and a desktop shell:

```text
task → understand → explore → plan → execute → verify
                                      ↑          │
                                      └ diagnose ←┘
```

The design favors a derived repository knowledge graph backed by lightweight local storage over premature distributed infrastructure. It also favors a small set of reliable tools over a large catalog of weakly validated tools.

## Repository guidance

- [`AGENTS.md`](AGENTS.md) — instructions for coding agents working in this repository.
- [`.agent/skills/`](.agent/skills/) — Antigravity-discoverable project skills.
- [`.agent/rules/second-ego.md`](.agent/rules/second-ego.md) — project-specific Antigravity rules.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — contribution workflow and quality bar.
- [`SECURITY.md`](SECURITY.md) — vulnerability reporting and security expectations.
- [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) — community standards.
- [`LICENSE`](LICENSE) — MIT license, allowing reuse and modification with attribution and warranty disclaimer.

## License

Released under the MIT License. See [`LICENSE`](LICENSE).
