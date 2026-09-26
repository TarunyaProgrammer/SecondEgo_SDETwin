# SecondEgo — Autonomous Coding Agent Application

> A coding harness built for reliable autonomous software engineering.

SecondEgo is a local coding-agent application whose Rust engine turns a foundation model into a reliable software engineer through repository indexing, structured planning, bounded tool orchestration, context management, failure recovery, and test-driven verification. The Electron + React pixel-village shell visualizes the engine; it does not replace it. Python remains a verified compatibility/reference runtime during migration.

## Status

The headless Rust harness now has an executable indexed vertical slice: explicit state machine, Tree-sitter repository index, explainable retrieval, safe workspace tools, isolated Git attempts, bounded context packets, structured provider output, verification, one recovery cycle, SQLite persistence, and a CLI. Python remains the compatibility path until evaluator parity is proven. An Electron/React observer shell builds against the local gateway; presentation mode is optional and observes engine events without adding model calls or tools.

[`CONTEXT-1.md`](docs/context/CONTEXT-1.md) contains the harness proposal, [`CONTEXT-2.md`](docs/context/CONTEXT-2.md) contains the desktop product-shell proposal, and [`CONTEXT.md`](docs/context/CONTEXT.md) records current decisions and open questions.

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

The Rust migration can be exercised with `make rust-test` and `make rust-run`. A
replayable Rust run accepts `TASK=... SCRIPT=...`; `--state-db` persists its final
state and versioned events to SQLite. The root evaluator commands remain unchanged
until Rust passes the documented cutover gates.

Presentation is opt-in. The evaluator-safe default is `make run`. To display
compact live engine events without changing model or tool budgets, run
`make run UI_MODE=events`. Direct CLI runs support the equivalent `--ui` flag.

The optional local browser observer can be started separately with `make desktop`.
It binds to loopback, displays a per-process token, and submits work through the
local engine gateway. The Electron/React shell consumes the same event boundary
while the Rust gateway cutover is completed.

For the Rust-backed Electron shell, use `make desktop-electron`; it builds the
Rust gateway first and the Electron process prefers that binary automatically.

[`evaluation/README.md`](evaluation/README.md) documents a separate target-repository
rehearsal and includes a deliberately buggy pagination fixture.

## Design direction

The SecondEgo architecture uses a model-provider adapter, a repository intelligence layer, a stateful Rust execution engine, local SQLite state, and a desktop shell:

```text
task → understand → explore → plan → execute → verify
                                      ↑          │
                                      └ diagnose ←┘
```

The design favors a derived repository knowledge graph backed by lightweight local storage over premature distributed infrastructure. It also favors a small set of reliable tools over a large catalog of weakly validated tools.

## Current scope

- Python AST symbol/import extraction is implemented; other languages currently use structural and lexical fallback only.
- Clean Git targets run in detached worktrees; failed attempts are discarded and only verified diffs are transferred.
- Python test nodes and import-backed test-to-code links feed failure-aware retrieval.
- SQLite schema version 1 persists compact final run state, events, and evidence.
- The model is required to return a structured plan, which is validated before tools execute it.
- Desktop packaging, the full benchmark suite, and richer multi-language parsing remain planned work; the first Electron/React observer shell is now present.

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
