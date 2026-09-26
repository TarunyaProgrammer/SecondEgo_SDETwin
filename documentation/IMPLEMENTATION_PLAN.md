# SecondEgo Implementation Plan

Status: active implementation plan; transaction milestone delivered

This plan reconciles `docs/context/CONTEXT.md`, `docs/context/CONTEXT-1.md`, `docs/context/CONTEXT-2.md`, the repository `AGENTS.md`, and the hackathon scoring rubric supplied by the user.

## 1. Product boundary

SecondEgo has three layers:

1. **Core harness** — the Python system that independently understands an issue, explores a repository, plans, edits, verifies, recovers, and terminates with evidence.
2. **Desktop shell** — Electron/React UI that visualizes real engine events and provides bounded control. It is not authoritative for execution.
3. **Local data** — SQLite for rebuildable repository metadata, execution state, telemetry, and reports.

The headless core is the competition-critical product. The shell is added only after the core vertical slice works.

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

## 6. Evaluation proof plan

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

## 7. Non-negotiable gates

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

## 8. Current delivery status

Milestones A, B, the first vertical slice, the transactional safety increment, and
the first diagnosis-informed recovery loop are implemented. The current test suite
has 36 passing contract tests. The remaining competition-critical work is
test-topology retrieval, phase-specific context packets, richer telemetry, and a
realistic local evaluation fixture.

## 9. Immediate next action

The next implementation task is the retrieval/context increment: connect the
failure record to targeted test and dependency retrieval, then enforce fixed
phase-specific context slots. No UI, voice, computer vision, graph database, or
packaging work should begin before that evidence packet passes fixture tests.
