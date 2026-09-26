# LCC Architecture — Deterministic Coding Runtime

**Status:** approved implementation target for the next harness slice
**Scope:** the text-only, Makefile-evaluated harness. The Electron product shell is out of scope until this runtime is reliable.

## Critical assessment

The current implementation is a credible vertical slice: explicit phases, bounded tools, Python AST indexing, a context ledger, verification, one recovery route, SQLite run state, and a provider boundary. It is not yet a competitive coding harness.

The main flaw is architectural: the model produces one broad plan before execution. A test failure then consumes a pre-written recovery action list, rather than causing an evidence-driven diagnosis and a new repair decision. Retrieval is lexical/Python-symbol based, not test- or failure-aware. Failed attempts can alter the target workspace. Those are reliability failures, not missing polish.

SecondEgo should be a deterministic runtime that uses a model for bounded reasoning—not a chat loop that happens to run commands.

## Evaluation constraints

- The evaluator runs `make setup`, exports `AI_API_KEY`, then runs `make run`.
- The model input/output is text-only; voice, vision, and desktop UI do not improve the scored path.
- The model, repository, issue, tests, and time budget may be standardized. The advantage comes from state, evidence, recovery, and verification.
- Repository text, issue text, and tool output are untrusted. They can contain prompt injection, secrets, malformed files, or hostile paths.
- A 24-hour build should not add a vector database, Neo4j, a generic agent framework, multi-agent orchestration, or always-on services.

## Target state machine

```text
issue + repository
       |
       v
PREPARE -> INDEX -> RETRIEVE -> PLAN -> BEGIN_ATTEMPT -> EXECUTE -> VERIFY
                                                               |       |
                                                               | pass  +--> APPLY -> FINALIZE -> COMPLETE
                                                               |
                                                              fail
                                                               |
                                                               v
                                                          DIAGNOSE
                                                               |
                                                               v
                                                    RETRIEVE_FOR_FAILURE
                                                               |
                                                               v
                                                         REPAIR_PLAN
                                                               |
                                       retry allowed? -- no --> FINALIZE -> FAILED / BLOCKED
                                            |
                                           yes
                                            v
                                      ROLLBACK -> BEGIN_ATTEMPT
```

Every transition records: reason, evidence references, resource consumption, and a terminal-safe state. `COMPLETE` is reachable only with passing verification evidence, never from a model claim.

## Model boundary

The model makes only three structured, validated decisions:

1. **Plan:** smallest change and focused verification.
2. **Diagnose:** rank hypotheses from cited failure and repository evidence.
3. **Repair plan:** a materially different strategy after diagnosis.

It does not decide its own permissions, context limits, retries, workspace root, success state, or rollback. The runtime owns tools, validation, state transitions, budgets, transactions, and final status.

## 1. Repository Digital Twin

Keep it local and rebuildable in SQLite. This is metadata, not a product graph service.

| Layer | Required data | Current gap |
| --- | --- | --- |
| Structure | paths, ignore/generated flags, manifests, test dirs, revision, Git status | add structured revision/status |
| Symbols | definitions, imports, exports, line ranges, parser confidence | current Python support is the base; label fallback extraction |
| Test topology | test node IDs, imports, named symbols, command mapping | missing; highest-value next index |
| Impact graph | direct/inverse imports and weighted test-to-code links | imports exist; inverse and test links missing |
| Change evidence | baseline, attempt diffs, changed paths | final diff exists; attempt scope missing |

Hard facts retain source path, line range, parser, and confidence. Heuristic links such as a test-name match are labeled as inferences, never shown to the model as certain call graph facts.

## 2. Retrieval cascade

Do not use one generic retrieval strategy. Match retrieval to the question:

```text
issue / test failure / current diff
    -> exact paths, symbols, test IDs, error locations
    -> lexical file and symbol matches
    -> imports and inverse dependents
    -> related tests and test-to-code candidates
    -> changed-file impact and focused test candidates
    -> Git history only when weaker signals fail and budget remains
    -> diversity-aware evidence packet or explicit abstention
```

Each result needs `score`, `reasons`, `source`, and `confidence`. Reserve evidence space for both implementation and test files when possible; do not spend the entire packet on near-duplicate paths. If evidence is weak, report that explicitly.

Version-one prefetch is a cache of one-hop candidates after selection. It is not background workers or a full-repository read.

## 3. Virtual Context Manager

The existing `ContextBudget` and ledger are the base. Replace a broad active-evidence handoff with fixed phase-specific slots:

| Slot | Retention rule | Contents |
| --- | --- | --- |
| Task contract | whole run | issue, acceptance criteria, workspace, permissions |
| Runtime state | whole run | phase, attempt, remaining budgets, provider/model |
| Current evidence | current phase/retry | excerpts, symbols, topology, diff with source refs |
| Verification | through recovery | commands, exit codes, failing IDs, bounded output |
| Decisions | whole run | plan/hypothesis/rejection with confidence |
| Response reserve | one model call | never consumed by retrieval |

Reserve task, state, verification, and response slots before retrieval. On a phase boundary, retain small durable facts and source IDs; drop redundant raw output; retrieve again from the index. Deterministic elision happens before LLM summarization. Model summaries are advisory and must link to source evidence.

## 4. Failure graph

`VerificationResult` must grow from a failure class plus text into a structured evidence record.

```text
verification command -> failing test / error location -> changed paths
       |                    |                         |
       +--------------------+--- related symbols ------+--- import / test edges
                                                           -> ranked repair candidates
```

Nodes: command, exit status, failing test ID where parseable, error path/line, error fingerprint, changed path, symbol, test candidate. Edges: `reported_by`, `located_in`, `changed_in_attempt`, `imports`, `imported_by`, `tests`, and `heuristic_test_match`.

The model receives raw failure evidence and ranked candidates. It may choose a hypothesis; the runtime must not falsely claim it knows root cause.

## 5. Transaction runtime

Failed attempts must not contaminate the evaluation workspace.

Preferred design:

1. Preflight Git availability, baseline revision/status, a harness-owned temporary state directory, and allowed commands.
2. Create one detached linked Git worktree outside the target repository tree.
3. Route all reads, writes, commands, and tests for an attempt to that worktree.
4. Record attempt diff, changed paths, tests, and tool evidence.
5. On pass, transfer only the reviewed/validated diff (including explicitly tracked new files) to the original workspace. On fail, remove the worktree and retain evidence only.

Fallback for non-Git repositories: a bounded file journal captures pre-write content/hash and created files before mutations. If neither transaction method is safe, block mutation rather than risk user files.

Do not do parallel speculation. A second attempt is allowed only after diagnosis or a deterministic ambiguity trigger. Model confidence alone is not reliable enough to branch on.

## 6. Tool surface

| Tool | Return contract |
| --- | --- |
| `inspect_target(query, mode)` | ranked files/symbols/tests, score reasons, evidence refs |
| `inspect_symbol(path, symbol)` | definition, importers/dependents, related tests, source excerpt |
| `diagnose_failure(verification_id)` | classified failure graph, raw refs, candidates |
| `apply_patch(patch)` | validated paths, before/after hashes, transaction ID |
| `run_verification(command_id)` | exit code, duration, bounded output, parsed failures |
| `inspect_diff()` | attempt-only diff summary and references |
| `run_command(...)` | narrow allowlisted fallback with cwd and timeout |

Existing file/search/Git/command tools remain the primitives. Every argument is validated, paths are canonicalized within the active attempt workspace, output is capped/redacted, and repository content never becomes instruction.

## 7. Recovery budget

Use one primary attempt and at most one diagnosis-informed recovery attempt.

| Failure class | First action | Retry |
| --- | --- | --- |
| Environment/dependency | capture command evidence; avoid code edits | no, unless declared setup repair is permitted |
| Tool contract/path policy | precise blocked result | no blind retry |
| Repository/transaction | restore baseline and report | no mutation until preflight passes |
| Test or implementation | build failure graph and retrieve code/tests | one varied repair |
| Model planning/schema | constrained re-request if budget remains | one, no tools |

A retry needs new diagnosis evidence and a repair plan that states what changed. Re-running the same plan is wasted time and model budget.

## 8. Telemetry and replay

SQLite stores safe, bounded metadata: run/revision/provider configuration, phase/attempt/time/resource use; retrieved and dropped candidates; context-slot estimates; tool timing/status/truncation/changed paths; verification/failure/transaction/final diff/terminal reason.

Never store API keys, `.env` contents, unredacted secrets, or raw unbounded command output. The run report must answer: **what evidence caused this edit, and why did the run stop?**

## Explicit non-goals

- Vector embeddings or remote vector services until benchmark evidence proves lexical/symbol/test topology insufficient.
- Neo4j, queues, and persistent graph infrastructure.
- Multi-agent debates, generic agent frameworks, or autonomous parallel branches.
- Electron/React pixel-village work before reliability passes.
- Voice, vision, or any multimodal feature—the evaluator is text-only.

## Implementation order

1. Define contracts for attempt, transaction, failure graph, context packet, retrieval mode, and transition tests.
2. Build transaction preflight and isolated worktree attempt; test pass transfer, failure rollback, new file handling, dirty and non-Git behavior.
3. Add Python test topology, inverse imports, and explainable retrieval modes with lexical fallback.
4. Add failure parsing and the structured diagnose/repair loop with one varied retry.
5. Assemble fixed context packets, deterministic elision, source links, and per-slot telemetry.
6. Add fixture repos for success, failure, malformed model output, timeout, hostile path, dirty repo, and rollback; rerun clean `make setup`, `make run`, and `make test`.
7. Then polish the TUI around visible phases, evidence, transaction status, verification, and final diff.

## Architecture acceptance gates

- A failed attempt leaves no target-workspace change.
- A passing attempt transfers only the approved diff.
- Every model call has bounded, source-linked evidence.
- Every retry follows classified failure and changes strategy.
- Final status has command evidence, final diff evidence, and explicit terminal reason.
- `make setup`, `make run`, and `make test` remain the complete evaluator-facing interface.

## Research basis

Source-code analysis of coding-agent harnesses supports hand-built deterministic loops and retrieval over generic agent frameworks or vector dependence: [Harness Engineering](https://arxiv.org/abs/2609.00006). Empirical harness work reports that deterministic elision before LLM summarization helps under constrained context, and that planning is chiefly a cost/reliability control: [Designing Coding-Agent Harnesses](https://arxiv.org/abs/2609.20804). Retrieval should stay task-aware because no one approach wins across code-to-test, trace-to-code, and edit-impact work: [Agent Retrieval Bench](https://arxiv.org/abs/2607.24882). Linked worktrees provide isolated working trees sharing Git metadata, which makes them a suitable transaction substrate when preflight passes: [Git worktree](https://git-scm.com/docs/git-worktree).
