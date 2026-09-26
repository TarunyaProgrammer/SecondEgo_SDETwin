# SecondEgo Rust Engine Migration Plan

Status: migration active; the Rust engine has an executable indexed vertical slice
beside the verified Python reference runtime.

## Why this is a staged migration

Replacing the entire engine in one edit would destroy the current evaluator
path before the Rust implementation has parity. The current Python runtime is
therefore the reference implementation during migration. It remains runnable
until Rust passes the same fixture, safety, event, and termination gates.

The final architecture is:

```text
Electron + React observer
          |
          | loopback HTTP / event contract
          v
Rust SecondEgo engine
  orchestration
  repository index/retrieval
  bounded context
  Gemini provider
  tools / transactions
  verification / recovery
  SQLite persistence
          |
          v
target repository
```

## Rust workspace layout

```text
engine-rs/
└── crates/
    ├── secondego-core         state, events, budgets, action contracts
    ├── secondego-repository   scanner, Tree-sitter index, retrieval
    ├── secondego-context      evidence ledger and context packets
    ├── secondego-model        provider trait and Gemini adapter
    ├── secondego-tools        path, command, file, search, Git, transaction
    ├── secondego-verification tests, failure classes, diff evidence
    ├── secondego-runtime      orchestration and recovery
    ├── secondego-gateway      loopback API for the desktop shell
    └── secondego-cli          evaluator-compatible binary
```

The contracts, indexing, context, model, tools/transaction, verification, runtime,
storage, and CLI crates now exist. The Rust gateway and final evaluator cutover
remain gated on parity tests rather than being assumed complete.

## Migration order

1. **Contracts:** Rust state machine, terminal statuses, events, budgets, and
   action proposals. Match Python behavior with unit tests.
2. **Indexing first:** scanner, ignored paths, manifests, symbols, imports,
   inverse dependencies, test nodes, test links, parser failures, stable source
   references, and explainable retrieval. This is the highest-value technical
   differentiator and must not be replaced with a repository dump.
3. **Context:** source-linked evidence ledger, stale evidence, deterministic
   token estimates, phase-specific packets, dropped-evidence telemetry.
4. **Model:** provider trait, Gemini REST adapter, structured JSON validation,
   bounded model calls, and a scripted provider for deterministic tests.
5. **Tools and transactions:** canonical paths, allowlisted argv, timeouts,
   output caps, Git worktrees, verified diff transfer, and rollback.
6. **Verification and recovery:** command evidence, failure classification,
   diagnosis retrieval, one varied repair attempt, explicit terminal states.
7. **Gateway and UI:** move the current loopback API to Rust while keeping the
   React/Electron renderer unchanged.
8. **Cutover:** Rust passes all Python fixture cases and safety gates; update
   `make run`, `make test`, and `make setup` to use Rust by default.
9. **Removal:** only after a full cutover commit, remove Python runtime code and
   keep a migration note explaining the compatibility history.

## Rust technology choices

| Concern | Rust implementation | Reason |
| --- | --- | --- |
| Async/runtime | Tokio | bounded concurrent gateway and model I/O |
| Types/JSON | serde + serde_json | explicit contracts and reports |
| Errors | thiserror | typed failure boundaries |
| IDs/time | uuid + chrono | stable run IDs and UTC events |
| Repository parsing | Tree-sitter | incremental, multi-language syntax/indexing |
| Search | ignore/walkdir + regex | repository-aware bounded traversal/search |
| Model HTTP | reqwest | provider adapter without vendor lock-in |
| SQLite | rusqlite | local rebuildable persistence |
| CLI | clap | evaluator and developer commands |
| Git | controlled `git` subprocesses | preserve existing Git semantics and policy |
| Telemetry | tracing + JSON events | bounded, inspectable run evidence |

## Cutover gates

Rust is the repository's default local `make run` engine. It should not be called
the organizer-evaluator default until all of these clean-checkout gates are true:

- the same fixture produces the same verified diff;
- failed attempts leave the target untouched;
- dirty/non-Git/path-escape/timeout cases are blocked safely;
- model output is schema-validated and provider failures are bounded;
- indexing reports symbols, imports, tests, parser failures, and confidence;
- context budgets and dropped evidence are observable;
- UI-off mode starts no UI process and makes no extra model/tool calls;
- `make setup`, `make run`, `make test`, and `make clean` work from a clean checkout;
- the Rust report preserves run ID, events, evidence, resources, verification,
  changed paths, and terminal reason.

Until then, Python remains an explicit reference/rollback path; Rust remains the
final engine direction and the default local execution path.

## Current implementation checkpoint

`cargo test --manifest-path engine-rs/Cargo.toml` covers the Rust workspace,
including an end-to-end scripted plan that proves indexed context, isolated edit,
verification, and verified diff transfer. `make rust-run` exposes the CLI. The
remaining hardening blockers are a full Electron run against the Rust gateway,
broader language indexing, explicit report/evidence parity with the Python
reference, and clean-checkout validation of the Rust-default `make setup`/`make
run` path.
