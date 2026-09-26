Yes. This is where we should stop discussing the idea abstractly and freeze an engineering direction.

One important correction first: **we should not accidentally build “Second Self.”** The Devpost project is useful as a reference for agent architecture, memory, autonomy, and recovery, but the LCC × DevClub specification is explicitly evaluating an **autonomous coding-agent harness around a standardized foundation model**, not an end-user AI companion. The LCC document makes that distinction very explicit. 

The architecture therefore needs to optimize for the actual rubric: correctness, orchestration, context management, tools, efficiency, recovery, and technical quality. 

## Current implementation note (September 2026)

This document is the original architecture proposal and remains useful for
rationale, but its proposed Python implementation stack is no longer the final
engine direction. The production engine is being migrated to Rust under
`engine-rs/`; Python is the verified compatibility/reference path until parity
gates pass. The Rust design keeps the same contracts—state machine, provider
boundary, indexing, bounded context, safe tools, transactions, verification,
recovery, telemetry, and SQLite persistence—but makes Rust the authoritative
runtime after cutover. Prefer `uv` only for the temporary Python reference path;
use Cargo for the final engine.

# 1. Project identity

I suggest we call the repository:

## `SecondEgo`

**Full name:** `SecondEgo — Autonomous Coding Agent Harness`

The name is short, neutral, and describes what we're building: a system that takes a foundation model and SecondEgos it into a reliable software-engineering agent.

### GitHub description

> **An autonomous coding-agent harness that turns a foundation model into a reliable software engineer through intelligent repository exploration, structured planning, adaptive tool orchestration, context management, failure recovery, and test-driven verification.**

### Short tagline

> **A coding harness built for reliable autonomous software engineering.**

### Longer project description

> SecondEgo is an autonomous coding-agent harness designed to operate on existing software repositories and solve software-engineering issues without human intervention.
>
> Rather than relying on a single model call or a simple ReAct loop, SecondEgo coordinates repository intelligence, task planning, context retrieval, tool execution, verification, and failure recovery through a stateful orchestration engine.
>
> The harness dynamically determines what repository information is relevant, selects appropriate tools, maintains compact execution state, runs tests as feedback signals, diagnoses failures, and iterates until the task is verified or the system reaches a justified termination state.
>
> The architecture is designed around one principle: **give the model the right information, the right tool, and the right amount of context at the right time.**

That last sentence maps almost directly to what the evaluation document says about context management. 

---

# 2. What exactly are we building?

The fundamental pipeline is:

```text
                    ┌─────────────────────┐
                    │   USER ISSUE        │
                    └──────────┬──────────┘
                               │
                               ▼
                    ┌─────────────────────┐
                    │ TASK UNDERSTANDING  │
                    └──────────┬──────────┘
                               │
                               ▼
              ┌────────────────────────────────┐
              │     REPOSITORY INTELLIGENCE    │
              │                                │
              │  files / symbols / deps / git │
              │  tests / relevant code        │
              └───────────────┬────────────────┘
                              │
                              ▼
                    ┌─────────────────────┐
                    │       PLANNER       │
                    └──────────┬──────────┘
                               │
                               ▼
                ┌───────────────────────────┐
                │     EXECUTION ENGINE      │
                │                           │
                │ model ↔ tools ↔ state     │
                └─────────────┬─────────────┘
                              │
                 ┌────────────┼────────────┐
                 ▼            ▼            ▼
             filesystem     search       terminal
                 │            │            │
                 └────────────┼────────────┘
                              │
                              ▼
                    ┌─────────────────────┐
                    │      VERIFIER       │
                    │                     │
                    │ tests / lint / diff │
                    └──────────┬──────────┘
                               │
                    ┌──────────┴──────────┐
                    │                     │
                 SUCCESS               FAILURE
                    │                     │
                    ▼                     ▼
                 COMPLETE          DIAGNOSE + RECOVER
                                          │
                                          └──────► EXECUTION
```

This is much more defensible in front of judges than:

```text
prompt → LLM → tools → done
```

The LCC specification explicitly says judges will examine planning, state management, task decomposition, tool routing, verification, termination, adaptation, and failure handling. 

---

# 3. Our architectural thesis

I want us to build around **five major subsystems**.

```text
┌──────────────────────────────────────────────────────────┐
│                      SecondEgo HARNESS                       │
│                                                          │
│  ┌────────────┐       ┌────────────────────────────┐    │
│  │ Task Layer │──────►│   ORCHESTRATION ENGINE      │    │
│  └────────────┘       │                            │    │
│                       │ Planner                    │    │
│                       │ State Machine              │    │
│                       │ Execution Controller       │    │
│                       │ Recovery Controller        │    │
│                       └─────────────┬──────────────┘    │
│                                     │                   │
│               ┌─────────────────────┼───────────────┐   │
│               ▼                     ▼               ▼   │
│       ┌──────────────┐      ┌──────────────┐ ┌────────┐│
│       │ Context      │      │ Tool Router  │ │Verifier││
│       │ Engine       │      │              │ │        ││
│       └──────┬───────┘      └──────┬───────┘ └───┬────┘│
│              │                     │              │     │
│              ▼                     ▼              ▼     │
│       Repository             Tool Registry       Tests  │
│       Intelligence           ├─ search          Diff   │
│       ├─ symbols             ├─ read            Lint   │
│       ├─ dependencies        ├─ edit            Build  │
│       ├─ tests               ├─ terminal               │
│       └─ git                 └─ git                    │
│                                                          │
│                     ┌────────────────┐                   │
│                     │ State + Memory │                   │
│                     └───────┬────────┘                   │
│                             │                            │
│                     ┌───────▼────────┐                   │
│                     │   Telemetry    │                   │
│                     └────────────────┘                   │
└──────────────────────────────────────────────────────────┘
```

---

# 4. The interesting part: Repository Intelligence

This is where I think we can differentiate substantially.

A naïve coding agent does:

```text
list files
↓
read random files
↓
ask model
```

We should instead construct a **Repository Intelligence Layer**.

It maintains a lightweight representation of:

```text
Repository
   │
   ├── Files
   │
   ├── Symbols
   │    ├── functions
   │    ├── classes
   │    ├── interfaces
   │    └── exports
   │
   ├── Dependencies
   │
   ├── Tests
   │
   ├── Configuration
   │
   └── Git history
```

Then a task like:

> "Fix authentication timeout handling"

doesn't require the model to blindly inspect 500 files.

The retrieval layer can discover:

```text
Issue
  ↓
authentication
  ↓
auth service
  ↓
request middleware
  ↓
timeout utility
  ↓
related tests
  ↓
configuration
```

and construct a focused context package.

That directly targets the rubric's emphasis on relevant repository information, redundant information, context growth, retention, and compression. 

---

# 5. Should we use a Graph Database?

## Short answer: **No — not initially.**

And this is important.

Don't add Neo4j simply because "graph database = sophisticated architecture."

That would probably make the system **worse** during a 24-hour hackathon.

### What we actually need is a graph.

Not necessarily a graph **database**.

We can build:

```text
Repository Knowledge Graph
```

in memory / SQLite.

For example:

```text
AuthController
      │
      ├──── calls ────► AuthService
      │                    │
      │                    └──── calls ────► TokenService
      │
      └──── tested by ───► auth.controller.spec.ts
```

This gives us graph-based reasoning without introducing a database server.

### Possible implementation

```text
Tree-sitter
     ↓
AST extraction
     ↓
Symbol extraction
     ↓
Dependency extraction
     ↓
Repository Graph
     ↓
Context Retrieval
```

Potentially:

```python
RepositoryGraph

nodes:
    File
    Class
    Function
    Test
    Config

edges:
    IMPORTS
    CALLS
    DEFINES
    TESTS
    EXTENDS
    REFERENCES
```

We can store the durable metadata in **SQLite**.

So:

```text
             ┌─────────────────────┐
             │ Repository Scanner  │
             └──────────┬──────────┘
                        ▼
                  Tree-sitter
                        │
                        ▼
              ┌──────────────────┐
              │ Knowledge Graph  │
              │                  │
              │ nodes + edges    │
              └────────┬─────────┘
                       │
                       ▼
               Context Retrieval
```

### Why this is better

We get the **architectural benefit of graph reasoning** without:

* Neo4j deployment
* network overhead
* another service
* another failure mode
* another dependency
* unnecessary persistence complexity

And if the judges ask:

> "Why didn't you use Neo4j?"

we have a very good engineering answer:

> "The problem required graph relationships, not necessarily a graph database. We represented repository relationships as a derived knowledge graph backed by SQLite because the evaluation workload is a single repository execution. This gives us structural retrieval without introducing distributed infrastructure that doesn't improve task completion."

That's a significantly better answer than "we used Neo4j because graphs are cool."

---

# 6. Technology stack

## Core language

### Python 3.12+

I strongly recommend Python.

Why:

* excellent subprocess support
* excellent filesystem tooling
* mature AST ecosystem
* Tree-sitter bindings
* async support
* Pydantic
* fast iteration
* excellent LLM SDK ecosystem
* easy telemetry
* easy testing

This is an agent infrastructure project, not a frontend application.

---

# 7. Model layer

The competition specification currently identifies **Gemini 3.8 High** as the foundation model and says the final rules regarding external models are still subject to organizer decisions.  

Therefore:

```text
ModelProvider
      │
      └── GeminiProvider
```

Do **not** hard-code Gemini calls throughout the architecture.

Instead:

```python
class ModelProvider(Protocol):
    async def generate(...)
    async def stream(...)
    async def count_tokens(...)
```

Then:

```text
ModelProvider
      │
      └── GeminiProvider
```

This gives us clean separation.

If the organizers change the interface, one adapter changes instead of the entire harness.

---

# 8. Orchestration engine

This is the heart of the project.

I don't want a pure ReAct loop.

I also don't want an absurd 12-agent multi-agent system.

The sweet spot is:

## Hierarchical state-machine + adaptive execution loop

Something like:

```text
                TASK
                  │
                  ▼
             UNDERSTAND
                  │
                  ▼
              EXPLORE
                  │
                  ▼
               PLAN
                  │
                  ▼
             EXECUTE
                  │
                  ▼
              VERIFY
              /     \
          PASS       FAIL
           │           │
           ▼           ▼
        COMPLETE    DIAGNOSE
                       │
                       ▼
                    RECOVER
                       │
                       └──────► EXECUTE
```

Each state has explicit entry/exit conditions.

This is much easier to reason about than an unconstrained agent loop.

---

# 9. But the model still controls execution

The state machine should **not** rigidly dictate every action.

Instead:

```text
State Machine
      │
      ▼
Model
      │
      ▼
Action Proposal
      │
      ▼
Policy / Tool Router
      │
      ▼
Tool
      │
      ▼
Observation
      │
      ▼
State Update
```

So the model has autonomy inside controlled boundaries.

---

# 10. Tool architecture

Don't expose 30 tiny tools.

The LCC document explicitly says that more tools do not automatically mean a better harness; tool granularity, usefulness, reliability, selection, sequencing, and error handling matter. 

I'd start with approximately **8 high-value tools**:

```text
1. repository_tree
2. search_code
3. read_file
4. inspect_symbol
5. edit_file
6. run_command
7. run_tests
8. git_diff
```

Potentially:

```text
9. git_log
10. find_tests
```

later.

---

# 11. Tool Router

The model shouldn't blindly call tools.

We'll have:

```text
Model
  │
  ▼
Tool Intent
  │
  ▼
Tool Router
  │
  ├── permission check
  ├── argument validation
  ├── cost estimation
  ├── duplicate detection
  └── execution
          │
          ▼
       Tool
          │
          ▼
       Result
          │
          ▼
     Observation
```

This allows us to track:

```text
tool success
tool failure
execution duration
duplicate calls
arguments
result size
```

That feeds telemetry.

---

# 12. Context Engine

This is one of the areas I want us to spend serious time on.

Instead of:

```text
entire repository → model
```

we do:

```text
Task
 │
 ▼
Retriever
 │
 ├── lexical search
 ├── symbol search
 ├── dependency graph
 ├── test mapping
 ├── git history
 └── previous observations
 │
 ▼
Context Builder
 │
 ▼
Context Budgeter
 │
 ▼
Model
```

And importantly:

## Context is stateful.

We maintain:

```text
Task State

goal
constraints
plan
current_step
known_files
known_symbols
observations
failed_attempts
test_results
decisions
```

Instead of repeatedly sending everything.

This directly addresses the LCC requirement that the objective isn't simply minimizing context size, but supplying the **right information at the right time**. 

---

# 13. Context compression

We should implement explicit compression.

For example:

```text
Raw observations
      ↓
Important facts
      ↓
Structured state
      ↓
Discard redundant raw output
```

Suppose the model executes:

```bash
pytest tests/auth/test_login.py
```

and gets 200 lines.

We don't want all 200 lines permanently sitting in context.

Convert:

```text
TestResult {
    passed: false,
    failures: [
       {
         test: "test_expired_token",
         error: "TimeoutError",
         location: "auth/service.py:84"
       }
    ]
}
```

Now the model gets the **information**, not the noise.

---

# 14. Memory architecture

Don't build a giant vector-memory system.

For this competition, most useful memory is **structured execution memory**.

I'd use:

### SQLite

```text
execution.db

tasks
plans
steps
observations
tool_calls
test_runs
failures
context_snapshots
repository_symbols
repository_edges
```

And transient runtime state:

```text
Python objects / asyncio
```

So:

```text
                 ┌──────────────┐
                 │ Runtime State│
                 └──────┬───────┘
                        │
                        ▼
                    SQLite
                        │
              ┌─────────┼─────────┐
              ▼         ▼         ▼
           Task       Graph     Telemetry
           State      Data       Data
```

---

# 15. Verification engine

This is critical because **correctness is worth 30%**, the single largest category. 

Verification isn't:

```text
model says "done"
```

It is:

```text
implementation
      │
      ▼
git diff
      │
      ▼
tests
      │
      ▼
lint / typecheck if available
      │
      ▼
failure classification
      │
      ▼
verification verdict
```

The verifier should classify:

```text
PASS

TEST_FAILURE
BUILD_FAILURE
LINT_FAILURE
TYPE_ERROR
RUNTIME_ERROR
REGRESSION
TOOL_FAILURE
UNKNOWN
```

This classification becomes extremely valuable for recovery.

---

# 16. Recovery engine

This is where I expect we can make the system substantially better than a basic coding agent.

Instead of:

```text
test failed
↓
ask model again
```

we do:

```text
Failure
  │
  ▼
Failure Classifier
  │
  ├── syntax error
  ├── type error
  ├── test assertion
  ├── runtime failure
  ├── dependency issue
  ├── environment issue
  └── unknown
  │
  ▼
Recovery Strategy
  │
  ▼
Targeted Context Retrieval
  │
  ▼
Model
  │
  ▼
New Action
```

The competition explicitly evaluates whether the harness can identify failure, reason about it, adapt, and continue without intervention. 

---

# 17. Termination engine

This is another thing many agent implementations screw up.

We need explicit termination criteria.

```text
SUCCESS
    │
    ├── required tests pass
    ├── implementation exists
    ├── diff is valid
    └── issue requirements satisfied

FAILURE
    │
    ├── unrecoverable environment
    ├── budget exhausted
    ├── repeated identical failure
    └── maximum recovery attempts

CONTINUE
    │
    └── meaningful progress possible
```

Potentially:

```text
progress_score
```

to detect loops.

Example:

```text
Attempt 1 → test failure
Attempt 2 → different test failure
Attempt 3 → fewer failures
Attempt 4 → all tests pass
```

That's progress.

But:

```text
Attempt 1 → same failure
Attempt 2 → same failure
Attempt 3 → same failure
```

should trigger loop detection.

---

# 18. Telemetry should NOT be an afterthought

This is extremely important because the competition captures:

* model calls
* tokens
* context
* tools
* agent state
* transitions
* retries
* errors
* recovery
* tests
* timing

as standardized evidence. 

So our architecture should have:

```text
                 ┌──────────────────┐
                 │   Event Bus      │
                 └────────┬─────────┘
                          │
        ┌─────────────────┼──────────────────┐
        ▼                 ▼                  ▼
    Orchestrator       Tools              Model
        │                 │                  │
        └─────────────────┼──────────────────┘
                          ▼
                     Telemetry
                          │
                    JSONL / SQLite
```

Every significant operation emits an event.

Example:

```json
{
  "event": "tool.completed",
  "tool": "run_tests",
  "duration_ms": 1834,
  "success": false,
  "timestamp": "...",
  "execution_id": "..."
}
```

Then at the end:

```text
Telemetry
    ↓
Report Generator
    ↓
Standard Report
```

---

# 19. Initial technology stack

## Core

| Component          | Technology                       |
| ------------------ | -------------------------------- |
| Language           | Python 3.12+                     |
| Async              | asyncio                          |
| Validation         | Pydantic                         |
| Model              | Gemini adapter                   |
| Persistence        | SQLite                           |
| Repository parsing | Tree-sitter                      |
| Search             | ripgrep                          |
| Git                | Git CLI / GitPython if necessary |
| Testing            | pytest                           |
| HTTP               | httpx                            |
| CLI                | Typer or argparse                |
| Logging            | structlog / stdlib logging       |
| Serialization      | JSON                             |
| Package management | uv                               |

### I prefer `uv` over Poetry.

It's faster and simpler for a hackathon.

---

# 20. Architecture libraries

Potentially:

```text
tree-sitter
tree-sitter-language-pack
pydantic
pydantic-settings
httpx
pytest
pytest-asyncio
rich
typer
```

We should **avoid adding libraries until we actually need them**.

The LCC document explicitly considers unnecessary dependencies as part of repository/code quality. 

---

# 21. Repository structure

This is what I want us to start with:

```text
SecondEgo/
│
├── README.md
├── pyproject.toml
├── uv.lock
├── .gitignore
│
├── src/
│   └── SecondEgo/
│       │
│       ├── core/
│       │   ├── engine.py
│       │   ├── state.py
│       │   ├── events.py
│       │   └── policies.py
│       │
│       ├── model/
│       │   ├── base.py
│       │   ├── gemini.py
│       │   └── schemas.py
│       │
│       ├── orchestration/
│       │   ├── state_machine.py
│       │   ├── planner.py
│       │   ├── executor.py
│       │   ├── recovery.py
│       │   └── termination.py
│       │
│       ├── repository/
│       │   ├── scanner.py
│       │   ├── parser.py
│       │   ├── symbols.py
│       │   ├── graph.py
│       │   ├── search.py
│       │   └── index.py
│       │
│       ├── context/
│       │   ├── retriever.py
│       │   ├── builder.py
│       │   ├── budget.py
│       │   ├── compressor.py
│       │   └── state.py
│       │
│       ├── tools/
│       │   ├── registry.py
│       │   ├── router.py
│       │   ├── filesystem.py
│       │   ├── search.py
│       │   ├── terminal.py
│       │   ├── testing.py
│       │   └── git.py
│       │
│       ├── verification/
│       │   ├── verifier.py
│       │   ├── failures.py
│       │   └── regression.py
│       │
│       ├── telemetry/
│       │   ├── collector.py
│       │   ├── events.py
│       │   └── writer.py
│       │
│       ├── storage/
│       │   ├── database.py
│       │   ├── models.py
│       │   └── repositories.py
│       │
│       └── cli.py
│
├── tests/
│   ├── unit/
│   ├── integration/
│   └── fixtures/
│
├── telemetry/
│   ├── telemetry.md
│   ├── telemetryAgent.md
│   └── telemetry.schema.json
│
├── reporting/
│   ├── reportCreating.md
│   ├── reportCreatorAgent.md
│   └── report.schema.json
│
├── configuration/
│   └── default.toml
│
└── documentation/
    ├── architecture.md
    ├── decisions/
    └── execution.md
```

The official specification itself expects the submission to contain `harness/`, `telemetry/`, `reporting/`, `README.md`, `configuration/`, and `documentation/`, although it says the exact structure may be finalized later. 

So **we should adapt our internal `src/SecondEgo` architecture to the required external submission structure later**, rather than blindly copying their proposed tree.

---

# 22. One thing I want to change from this structure

Don't prematurely create 50 files.

For the first implementation, we need only:

```text
SecondEgo/
├── src/SecondEgo/
│   ├── core/
│   ├── model/
│   ├── orchestration/
│   ├── repository/
│   ├── context/
│   ├── tools/
│   ├── verification/
│   └── telemetry/
│
├── tests/
├── README.md
├── pyproject.toml
└── uv.lock
```

Then create files when functionality actually appears.

Architecture should emerge from working boundaries, not empty directories.

---

# 23. What we are NOT building

This is just as important.

### No:

* React frontend
* Next.js
* Electron
* ElevenLabs
* OpenCV
* BeautifulSoup
* Auth0
* Firebase
* MongoDB Atlas
* Neo4j
* Redis
* vector database
* multi-agent swarm
* fancy UI
* voice assistant
* browser automation

Those technologies appeared in the **Second Self** project, which used macOS sessions, VNC, PyAutoGUI, browser-use, Claude agents, ElevenLabs, FastAPI, Firebase, etc. 

They are irrelevant to the core LCC competition unless the final rules introduce a reason to use them.

---

# 24. Our actual "advanced architecture"

The impressive part isn't the number of technologies.

It's this:

```text
                 ┌─────────────────────────┐
                 │       TASK INPUT        │
                 └────────────┬────────────┘
                              ▼
                 ┌─────────────────────────┐
                 │    TASK INTERPRETER     │
                 └────────────┬────────────┘
                              ▼
                 ┌─────────────────────────┐
                 │ REPOSITORY INTELLIGENCE │
                 │                         │
                 │ search + symbols +      │
                 │ dependency graph + tests│
                 └────────────┬────────────┘
                              ▼
                 ┌─────────────────────────┐
                 │    CONTEXT ENGINE       │
                 │                         │
                 │ retrieve → rank →       │
                 │ compress → budget       │
                 └────────────┬────────────┘
                              ▼
                 ┌─────────────────────────┐
                 │      PLANNER            │
                 └────────────┬────────────┘
                              ▼
                 ┌─────────────────────────┐
                 │   EXECUTION CONTROLLER  │◄────┐
                 └────────────┬────────────┘     │
                              ▼                   │
                 ┌─────────────────────────┐     │
                 │       TOOL ROUTER       │     │
                 └────────────┬────────────┘     │
                              ▼                   │
                 ┌─────────────────────────┐     │
                 │       TOOL EXECUTION    │     │
                 └────────────┬────────────┘     │
                              ▼                   │
                 ┌─────────────────────────┐     │
                 │       OBSERVATION       │     │
                 └────────────┬────────────┘     │
                              ▼                   │
                 ┌─────────────────────────┐     │
                 │       VERIFIER          │     │
                 └────────────┬────────────┘     │
                              │                   │
                    ┌─────────┴──────────┐        │
                    ▼                    ▼        │
                  PASS                 FAIL       │
                    │                    │        │
                    ▼                    ▼        │
                COMPLETE             RECOVERY ────┘
```

And **every transition emits telemetry**.

That's the architecture I would defend to the technical panel.

---

# 25. Our development priorities

Given the rubric, our priority order should be:

| Priority | System                     | Why                                              |
| -------- | -------------------------- | ------------------------------------------------ |
| **P0**   | Reliable execution loop    | Without correctness, architecture doesn't matter |
| **P0**   | Verification               | 30% correctness                                  |
| **P0**   | Repository exploration     | Fundamental coding-agent capability              |
| **P0**   | Tool execution             | Required to modify repository                    |
| **P1**   | Context engine             | 15% evaluation category                          |
| **P1**   | Recovery                   | 10% + affects correctness                        |
| **P1**   | Orchestration              | 15%                                              |
| **P1**   | Telemetry                  | Required evidence                                |
| **P2**   | Repository knowledge graph | Improves retrieval                               |
| **P2**   | Context compression        | Efficiency                                       |
| **P2**   | Token budgeting            | Efficiency                                       |
| **P3**   | Advanced optimizations     | Only after baseline works                        |

The weighting is why I don't want us spending six hours on a graph database before we have a reliable edit → test → recover loop. 

---

# 26. The first milestone

Our first working version should be brutally simple:

```text
SecondEgo solve "issue description"
```

and internally:

```text
Task
 ↓
Explore repository
 ↓
Plan
 ↓
Read relevant files
 ↓
Edit
 ↓
Run tests
 ↓
Analyze result
 ↓
Fix if necessary
 ↓
Run tests again
 ↓
Verify
 ↓
Finish
```

**That needs to work before anything clever gets added.**

Then we progressively replace pieces:

```text
V0
basic agent loop

        ↓

V1
state machine

        ↓

V2
repository intelligence

        ↓

V3
context budget + compression

        ↓

V4
failure classification + recovery

        ↓

V5
knowledge graph

        ↓

V6
telemetry + standardized reporting

        ↓

V7
optimization / evaluation harness
```

This gives us a working system at every stage instead of gambling the entire hackathon on an ambitious architecture.

---

## The repository we should create now

```bash
mkdir SecondEgo
cd SecondEgo

git init

uv init
uv add pydantic pydantic-settings httpx rich typer
uv add --dev pytest pytest-asyncio

mkdir -p src/SecondEgo
mkdir -p tests
```

Then our **first actual coding task** should be the core state model and execution engine — not the LLM call.

I want the first implementation to establish:

```text
Task
  ↓
ExecutionState
  ↓
StateMachine
  ↓
ModelProvider
  ↓
ToolRegistry
  ↓
ToolResult
  ↓
VerificationResult
  ↓
RecoveryDecision
```

Once those contracts are correct, plugging Gemini into the system becomes an implementation detail rather than the architecture.

And that's important because the LCC evaluation is explicitly about the **harness surrounding the model**, including orchestration, context, tools, recovery, telemetry, and state—not prompt engineering alone. 

**Next step: we should now write the actual `pyproject.toml`, package structure, Pydantic state schemas, event system, and first `StateMachine` implementation.**
