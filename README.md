# SecondEgo — Autonomous Coding Agent Application

> A coding harness built for reliable autonomous software engineering.

SecondEgo is a local coding-agent application whose Python engine turns a foundation model into a reliable software engineer through repository intelligence, structured planning, adaptive tool orchestration, bounded context management, failure recovery, and test-driven verification. The planned Electron + React pixel-village shell will visualize the engine; it will not replace it.

## Status

The headless Python harness is implemented as an early vertical slice. It has an explicit state machine, safe workspace tools, context and resource budgets, Python repository indexing, structured plan validation, verification/recovery, SQLite evidence persistence, and a CLI. The Electron/React shell is not implemented yet.

[`CONTEXT-1.md`](CONTEXT-1.md) contains the harness proposal, [`CONTEXT-2.md`](CONTEXT-2.md) contains the desktop product-shell proposal, and [`CONTEXT.md`](CONTEXT.md) records current decisions and open questions.

## Run the harness

Create an isolated environment and install the deterministic/test path:

```bash
python3 -m venv .venv
.venv/bin/pip install -e '.[dev]'
.venv/bin/python -m pytest -q
```

Run a replayable JSON plan:

```bash
PYTHONPATH=src python3 -m SecondEgo.cli solve \
  --repo /path/to/repository \
  --issue "Fix authentication timeout handling" \
  --plan plan.json \
  --state-db /path/to/secondego-runs.db
```

For provider-backed planning, install the optional Gemini dependency, set the evaluation-required `AI_API_KEY` in the environment, and select a model explicitly if the evaluation requires one:

```bash
.venv/bin/pip install -e '.[dev,gemini]'
AI_API_KEY=... PYTHONPATH=src python3 -m SecondEgo.cli solve \
  --repo /path/to/repository \
  --issue "Fix authentication timeout handling" \
  --gemini \
  --model gemini-3.8-flash
```

The engine does not send an API request when `AI_API_KEY` is missing. It terminates with explicit model-planning evidence instead. The root `Makefile` provides the evaluator interface: `make setup`, `make run`, `make test`, and `make clean`.

## Design direction

The proposed SecondEgo architecture uses a model-provider adapter, a repository intelligence layer, a stateful Python execution engine, local SQLite state, and a desktop shell:

```text
task → understand → explore → plan → execute → verify
                                      ↑          │
                                      └ diagnose ←┘
```

The design favors a derived repository knowledge graph backed by lightweight local storage over premature distributed infrastructure. It also favors a small set of reliable tools over a large catalog of weakly validated tools.

## Current scope

- Python AST symbol/import extraction is implemented; other languages currently use structural and lexical fallback only.
- SQLite schema version 1 persists compact final run state, events, and evidence.
- The model is required to return a structured plan, which is validated before tools execute it.
- The desktop shell, package distribution, full benchmark suite, and richer multi-language parsing remain planned work.

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
