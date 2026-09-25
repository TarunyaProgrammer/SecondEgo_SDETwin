# Project Context

## Current status

This repository contains project guidance and an initial architecture proposal. Product/runtime implementation has not started.

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

- Exact model/provider API and token-counting implementation.
- Python package layout and persistence schema.
- Supported repository languages and parser coverage.
- Tool permission model and sandbox boundary.
- Evaluation tasks, benchmark fixtures, and telemetry format.

## Change protocol

When a design decision changes, update this file with the decision and rationale. Keep detailed exploration in separate design notes instead of turning this file into a transcript.
