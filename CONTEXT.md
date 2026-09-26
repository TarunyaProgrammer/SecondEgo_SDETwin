# Project Context

## Current status

The repository now contains an early, headless implementation of the core harness. It is not yet the full desktop product.

Implemented vertical slice:

- typed state machine with verification, bounded recovery, and explicit termination;
- workspace-bounded files, search, commands, Git evidence, and tool routing;
- context budgets, source-linked evidence ledger, and run-level resource budgets;
- Python AST repository indexing with lexical fallback retrieval;
- deterministic and optional Gemini-backed structured planning;
- SQLite schema version 1 for final run state, events, and evidence;
- CLI execution and automated contract tests.

Still unimplemented: Electron/React shell, packaging, broad language parsing, benchmark fixtures, and full telemetry/report schemas.

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
7. Keep the Electron/React pixel-village shell separate from the Python engine and keep the core runnable headlessly.
8. Use SQLite for local execution state and rebuildable repository metadata before considering external infrastructure.

## Decisions still open

- Evaluation-required Gemini model identifier and API configuration. The adapter defaults to `gemini-3.8-flash` but is configurable; model calls remain behind `ModelProvider`.
- Provider-native token counting. The current preflight budget uses a deterministic local estimate to avoid a separate paid request.
- Supported repository languages beyond Python AST extraction; unsupported languages fall back to structural/lexical retrieval.
- Exact evaluation tool-permission policy and sandbox boundary.
- Evaluation tasks, benchmark fixtures, and final telemetry/report schemas.
- Desktop transport contract and Electron/React shell implementation.

## Change protocol

When a design decision changes, update this file with the decision and rationale. Keep detailed exploration in separate design notes instead of turning this file into a transcript.
