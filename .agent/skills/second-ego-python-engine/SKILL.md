---
name: second-ego-python-engine
description: Design or implement the SecondEgo Python agent engine, model-provider adapter, local persistence, and deterministic process boundaries.
---

# SecondEgo Python engine

Use this skill for the core harness, Python package layout, model integration, subprocess execution, SQLite state, or engine APIs.

## Rules

- Separate domain state, orchestration, tools, repository intelligence, providers, persistence, and transport adapters.
- Hide vendor-specific model calls behind a provider protocol with generation, streaming, and token-budget capabilities where supported.
- Use typed state and explicit transitions. Persist enough information to resume, inspect, or explain a run without storing unbounded transcripts.
- Treat subprocesses and model output as untrusted. Enforce timeouts, working-directory boundaries, output limits, cancellation, and exit-status capture.
- Keep local SQLite metadata rebuildable and versioned. Do not make correctness depend on a remote database for a single-repository run.
- Prefer standard-library or narrowly justified dependencies during the hackathon; every dependency needs a concrete benefit and a failure story.

