# Project Context

## Current status

The repository contains a verified headless Python reference runtime and an
executable Rust engine. Rust is the default local execution path and final engine
direction; Python remains the explicit compatibility/reference path while clean
organizer-environment parity is hardened.

Implemented vertical slice:

- typed state machine with verification, bounded recovery, and explicit termination;
- workspace-bounded files, search, commands, Git evidence, and tool routing;
- context budgets, source-linked evidence ledger, and run-level resource budgets;
- Python AST repository indexing with lexical fallback retrieval;
- deterministic and optional Gemini-backed structured planning;
- isolated Git worktree attempts that discard failures and transfer only verified diffs;
- structured verification failure records and diagnosis-informed model recovery;
- ranked task and failure retrieval with bounded, redacted source excerpts in planner context;
- provider transport failures converted into bounded terminal evidence instead of TUI crashes;
- optional loopback-only observer gateway and browser UI that consume serialized engine events without adding model/tool calls;
- redacted bounded telemetry and generated-file filtering at the transaction boundary;
- SQLite schema version 1 for final run state, events, and evidence;
- Rust crates for contracts, indexing, bounded context, model providers, tools/transactions, verification, runtime, storage, and CLI;
- CLI execution and automated contract tests.

Still unimplemented: desktop packaging/installer, broad language parsing, a broader benchmark suite, and full telemetry/report schemas. The first Electron/React observer shell and loopback gateway are now present.

The approved Rust migration and cutover sequence is documented in
[`LCC_ARCHITECTURE.md`](LCC_ARCHITECTURE.md). It prioritizes transactional attempts,
test- and failure-aware retrieval, bounded phase-specific context packets, and one
diagnosis-informed recovery over UI work or additional infrastructure.

## Working identity

- Product name: **SecondEgo — Autonomous Coding Agent Application**
- Repository name: `SecondEgo_SDETwin`
- Goal: turn a foundation model into a reliable software-engineering agent for existing repositories.
- Primary design sources: [`CONTEXT-1.md`](CONTEXT-1.md) and [`CONTEXT-2.md`](CONTEXT-2.md)

## Architectural commitments

1. Use a provider interface so the harness is not coupled to one model vendor.
2. Use a stateful orchestration loop with explicit verification and recovery.
3. Build repository intelligence as a derived knowledge graph backed by lightweight local storage before considering a graph database.
4. Prefer a small set of reliable tools over a large tool catalog.
5. Treat context selection, compression, retention, and evidence as first-class concerns.
6. Make termination explicit: success, justified failure, or blocked state.
7. Keep the Electron/React pixel-village shell separate from the Rust engine and keep the core runnable headlessly.
8. Use SQLite for local execution state and rebuildable repository metadata before considering external infrastructure.
9. Meet the standard evaluation contract through a root `Makefile` exposing `setup`, `run`, `test`, and `clean`; `run` now defaults to Rust and `ENGINE=python` is an explicit compatibility switch.
10. Read the evaluator-supplied credential only from `AI_API_KEY`; never persist or log it.
11. Keep the evaluation path text-only. `make run` launches the terminal UI, which collects a repository path and issue before starting one autonomous run.
12. Keep presentation optional. `make run` defaults to `UI_MODE=headless`; `UI_MODE=events` or CLI `--ui` only observes compact engine events and must not add model calls, tool calls, or engine decisions.

## Evaluation interface

- `make setup` creates `.venv`, installs the Python compatibility dependencies and project, and builds the Rust CLI/gateway in release mode.
- `make run` builds the Rust release CLI if needed, prompts for the repository path and issue, and uses `AI_API_KEY` for Gemini-backed planning. `ENGINE=python make run` explicitly selects the compatibility TUI.
- `make test` runs the contract suite.
- `SECONDEGO_MODEL` overrides the default configured Gemini model when the organizers prescribe a different text model.
- No committed file contains an API credential; `.env.example` contains empty placeholders only.

## Decisions still open

- Evaluation-required Gemini model identifier and API configuration. The adapter defaults to `gemini-3.8-flash` but is configurable; model calls remain behind `ModelProvider`.
- Provider-native token counting. The current preflight budget uses a deterministic local estimate to avoid a separate paid request.
- Supported repository languages beyond Python AST extraction; unsupported languages fall back to structural/lexical retrieval.
- Exact evaluation tool-permission policy and sandbox boundary.
- Evaluation tasks, benchmark fixtures, and final telemetry/report schemas.
- The transactional runtime currently requires a clean Git repository root with an initial commit; non-Git and dirty targets intentionally block mutation until a bounded journal fallback exists.
- Desktop transport contract and Electron/React shell implementation.
- Rust migration and cutover are tracked in [`RUST_MIGRATION_PLAN.md`](RUST_MIGRATION_PLAN.md); `make run` now exercises Rust by default and `make rust-run` remains the explicit development entry point.

## Change protocol

When a design decision changes, update this file with the decision and rationale. Keep detailed exploration in separate design notes instead of turning this file into a transcript.

## Context map

- [`CONTEXT-1.md`](CONTEXT-1.md): initial architecture proposal and rationale.
- [`CONTEXT-2.md`](CONTEXT-2.md): desktop product-shell and end-to-end experience proposal.
- [`LCC_ARCHITECTURE.md`](LCC_ARCHITECTURE.md): LCC evaluation runtime decision and implementation order.
- [`LCC_EVALUATION_MODEL.md`](LCC_EVALUATION_MODEL.md): evaluator/repository separation and local evaluation workflow notes.
- [`ContextSubmission.md`](ContextSubmission.md): evaluator-provided submission requirements; do not commit credentials.
