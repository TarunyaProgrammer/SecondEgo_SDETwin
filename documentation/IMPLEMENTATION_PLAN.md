# SecondEgo Implementation Plan

Status: active implementation plan; Rust migration vertical slice delivered; evaluator cutover gated by parity

This plan reconciles `docs/context/CONTEXT.md`, `docs/context/CONTEXT-1.md`, `docs/context/CONTEXT-2.md`, the repository `AGENTS.md`, and the hackathon scoring rubric supplied by the user.

## 1. Product boundary

SecondEgo has three layers:

1. **Core harness** — the Rust system that independently understands an issue, explores a repository, plans, edits, verifies, recovers, and terminates with evidence. Python remains a verified compatibility/reference path during migration.
2. **Desktop shell** — Electron/React UI that visualizes real engine events and provides bounded control. It is not authoritative for execution.
3. **Local data** — SQLite for rebuildable repository metadata, execution state, telemetry, and reports.

The headless Rust core is the final competition-critical product. The shell is an observer and is added only after the core event boundary works.

ElevenLabs, OpenCV, graph databases, vector databases, multi-agent swarms, and voice assistants are out of scope unless a later requirement makes them directly relevant. They do not improve the coding-agent problem by themselves.

## 2. Skill usage plan

Use the smallest relevant project skill at each phase. Skills guide decisions; they do not expand scope.

| Phase | Skill | Exact use | Required output |
|---|---|---|---|
| 0 | `second-ego-context-management` | Define context budgets, durable state, evidence retention, compression, and stale-context removal | Context policy and run-state contract |
| 1 | `second-ego-python-engine` | Define package boundaries, provider protocol, typed state, persistence, and subprocess limits | Headless engine skeleton and provider contract |
| 2 | `second-ego-security` | Define workspace, path, shell, secret, prompt-injection, and IPC boundaries | Security policy and negative tests |
| 3 | `second-ego-orchestration` | Implement explicit phases, action proposals, routing, verification transitions, recovery, and termination | Executable state machine |
| 4 | `second-ego-repository-intelligence` | Implement cheap scanning, symbols, tests, dependencies, retrieval ranking, and parser fallbacks | Rebuildable repository index and evidence package |
| 5 | `second-ego-verification` | Define success criteria, test evidence, diff evidence, failure classes, retry limits, and reports | Verification/report contract and tests |
| 6 | `second-ego-desktop` | Define the local API/event boundary after the core is usable | Versioned transport/event contract |
| 7 | `second-ego-ui-state` | Map real events to village states; expose evidence and failure states honestly | Minimal observability UI |

Do not use the desktop or UI skills to design the engine. Do not use repository-intelligence output as authoritative if it is only a model inference.

## 3. Context-management policy

### Context sources

The engine may assemble context from:

- task and acceptance criteria;
- repository tree, manifests, and configuration;
- ranked files, symbols, dependencies, and tests;
- current plan step;
- recent tool observations;
- verification evidence;
- classified failure and recovery state.

### Context budget

Reserve the model context in this order:

1. task, constraints, and acceptance criteria;
2. current action schema and plan step;
3. relevant repository evidence;
4. verification/recovery evidence;
5. compact execution state;
6. response allowance.

The exact token count is provider-dependent and must be configurable. The builder must refuse or compress before exceeding the budget; it must not silently truncate acceptance criteria or verification evidence.

### Durable state

Persist structured records for:

- facts with source paths/line ranges;
- decisions and their rationale;
- open questions;
- plan and current step;
- tool observations;
- changed paths and diff references;
- test results and failure classifications;
- failed approaches and retry counts;
- context snapshots with source references.

Model summaries are never authoritative without links back to source evidence.

### Retention rules

- Across one tool call: retain task, current step, relevant facts, and the tool result summary.
- Across a phase transition: retain the plan, changed paths, verification evidence, unresolved questions, and failure state.
- Across a retry: retain the prior failure and strategy; discard redundant raw output.
- Across a new task: retain repository index only if its revision/hash is still valid; discard task-specific assumptions.
- On truncation or timeout: retain the command, exit status, partial output marker, and recovery options.

### Compression rules

Raw output is summarized into typed evidence, preserving:

- command/tool;
- exit status;
- duration;
- paths and symbols;
- error/test names;
- relevant line numbers;
- confidence;
- source reference.

Never compress away an acceptance criterion, changed path, failure reason, or verification result.

## 4. Engine contracts

The first implementation must establish these contracts before real model integration:

```text
Task
  -> ExecutionState
  -> StateTransition
  -> ActionProposal
  -> PolicyDecision
  -> ToolResult
  -> Observation
  -> VerificationResult
  -> RecoveryDecision / Termination
```

Required phases:

```text
INITIALIZE → UNDERSTAND → EXPLORE → PLAN → EXECUTE → VERIFY
                                      ↑       ↓
                                      └ DIAGNOSE ← RECOVER
```

Terminal states are `COMPLETE`, `FAILED`, `BLOCKED`, and `CANCELLED`.

## 5. Delivery sequence

### Milestone A — Contracts and safety

- typed state and event schemas;
- provider protocol plus deterministic mock;
- workspace/path policy;
- bounded subprocess policy;
- tool result schema;
- unit tests for invalid inputs.

Exit condition: the core can execute a mocked state transition sequence without an LLM or UI.

### Milestone B — Safe tools and verification

- tree/search/read/edit/command/tests/diff;
- exit status and output limits;
- test discovery;
- diff inspection;
- verification verdicts;
- failure classification.

Exit condition: a fixture repository can be inspected, edited, tested, and reported safely.

### Milestone B.5 — Transactional attempts (delivered)

- clean Git-root preflight;
- detached linked worktree per attempt;
- discard failed attempts;
- transfer only verified tracked and new-file changes;
- explicit transaction evidence and blocked behavior.

Exit condition: contract tests prove failure rollback, passing transfer, and recovery from a clean baseline.

### Milestone C — First vertical slice

```text
issue → explore → plan → edit → test → diff → report
```

Use one resettable fixture and one narrow task. Add recovery only after the success path is observable.

Exit condition: deterministic mock-provider run produces the expected files, tests, events, and final report.

### Milestone D — Context and repository intelligence

- repository index;
- symbol/test/dependency evidence;
- ranked retrieval;
- context budget;
- compression and retention;
- source references.

Exit condition: the model receives a focused evidence package rather than a repository dump.

### Milestone E — Recovery and termination

- failure diagnosis;
- targeted context refresh;
- varied recovery strategies;
- retry and loop bounds;
- explicit blocked/failed outcomes.

Exit condition: a controlled failing test demonstrates diagnosis, corrective action, rerun, and success—or a justified block.

### Milestone F — Telemetry and reporting

- event stream;
- model/tool/state timing;
- context and token measurements;
- retry records;
- changed paths;
- final evidence report.

Exit condition: a run can be inspected without relying on model narration.

### Milestone G — Desktop shell

- local engine gateway;
- versioned event contract;
- task/repository submission;
- phase timeline;
- village visualization;
- evidence, diff, tests, and failure panels.

Exit condition: the shell renders the same run truthfully and cannot bypass engine policy.

### Milestone H — Presentation modes and desktop boundary (in progress)

The first UI increment must not change the evaluator path or consume model/tool
budget. The engine remains authoritative and exposes an observational event
boundary.

#### H1 — Headless default

- `make run` uses `UI_MODE=headless` by default;
- no presentation observer is attached;
- model calls, tool calls, context budgets, verification, and termination remain unchanged;
- `--ui` / `UI_MODE=events` is opt-in for compact event display only.

#### H2 — Versioned event boundary

- serialize `EngineEvent` into a JSON-safe contract;
- allow a non-authoritative event sink;
- swallow observer failures so a closed UI cannot fail a coding run;
- never include secrets or unbounded tool/model output in display events.

#### H3 — Local desktop gateway

- add a localhost-only gateway around the existing engine;
- accept a task/repository request;
- return a run ID;
- stream the existing event contract;
- expose final evidence, verification, and diff records;
- enforce local ownership/authentication and bounded request sizes.

#### H4 — React/Electron observer

- build the shell after the gateway contract is stable;
- render task, phase, tools, changed paths, verification, failure, recovery, and final diff;
- label unknown/indeterminate progress honestly;
- keep renderer code free of API keys, shell access, and direct filesystem writes.

#### H5 — Mode acceptance gates

- headless evaluator run works when no UI dependencies are installed;
- event display adds zero model calls and zero tool calls;
- observer disconnect does not alter terminal status;
- UI state is derived only from engine events;
- the same fixture result is produced in headless and presentation modes.

## 6. Rust migration and cutover plan

The final engine is Rust. Migration is staged because changing the evaluator
default before parity would make failures harder to diagnose and would remove the
known-good rollback path.

```text
Rust core contracts
  -> repository index and explainable retrieval
  -> bounded evidence context
  -> provider boundary (scripted + Gemini)
  -> safe tools and detached worktree transaction
  -> verification and one bounded recovery cycle
  -> SQLite report persistence
  -> Rust CLI and loopback gateway
  -> root Makefile cutover
```

Current Rust crates:

- `secondego-core`: state machine, terminal statuses, versioned events, budgets;
- `secondego-repository`: bounded scanner, Tree-sitter Python index, tests/imports/links, ranked retrieval;
- `secondego-context`: source-linked ledger, stale evidence, packet budgets and omissions;
- `secondego-model`: deterministic scripted provider and bounded Gemini REST adapter;
- `secondego-tools`: path/command/file/search/Git policy and detached worktree transaction;
- `secondego-verification`: command evidence and failure classification;
- `secondego-runtime`: indexed plan/execute/verify/recover orchestration;
- `secondego-storage`: local SQLite run/event persistence;
- `secondego-cli`: replayable fixture and Gemini-backed Rust entry point.

The Rust engine is exercised with `make rust-run` and `make rust-test`. The root
`make run` remains Python until the parity gates pass: recovery fixture, dirty/non-
Git/path escape/timeout safety cases, report/evidence parity, gateway integration,
and clean-checkout evaluator commands.

## 7. Evaluation proof plan

### Problem and user value — 20%

Demonstrate a developer submitting a real repository issue and receiving a verified change with minimal setup.

### AI/technical innovation — 25%

Show focused retrieval, structured planning, bounded context, action validation, failure classification, and recovery. A single API call with shell access is insufficient.

### Product execution — 25%

Measure first useful action, total latency, successful completion rate on fixtures, test evidence, and clean reset behavior.

### Demo impact — 20%

Use the sequence: issue → repository map → plan → edit → failure → diagnosis → recovery → passing tests → diff/report.

### Feasibility and scalability — 10%

Show provider abstraction, mock mode, bounded API cost, SQLite, local execution, redacted telemetry, and headless operation.

## 8. Non-negotiable gates

The project is not ready for a judging demo until:

- the headless engine works without Electron;
- the mock-provider run is deterministic;
- all filesystem changes stay inside the workspace;
- commands have timeouts and output limits;
- tests actually execute;
- verification is based on tool evidence;
- retries are bounded and strategy-aware;
- blocked and failed outcomes are visible;
- telemetry and a final report are generated;
- the fixture repository can be reset;
- the UI has no invented progress or fake agent activity.

## 9. Current delivery status

Milestones A, B, the first vertical slice, the transactional safety increment, and
the first diagnosis-informed recovery loop are implemented. The presentation
boundary is now in progress with 51 Python tests plus the Rust workspace suite.
The localhost gateway, browser fallback, Electron/React observer shell, Rust CLI,
Rust gateway, and Rust recovery fixture now exist; remaining competition-critical
work is cross-process integration testing, richer telemetry, packaging, broader
language indexing, and a broader realistic evaluation fixture.

## 10. Immediate next action

The next implementation task is gateway integration testing: verify bounded
requests, authentication, observer disconnects, and headless equivalence. After
that passes, harden the React/Electron packaging and preserve the same event
contract. No voice, computer vision, graph database, or always-on service should
be added.
