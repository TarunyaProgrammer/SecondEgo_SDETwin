# SecondEgo — Current Architecture

**Status:** implementation reference, refreshed for the current build on 2026-09-27.

This file describes what is actually wired today. [`CONTEXT-1.md`](CONTEXT-1.md)
and [`CONTEXT-2.md`](CONTEXT-2.md) remain design proposals; they do not override
this implementation record.

## 1. Product boundary

SecondEgo is a local coding harness. It receives an issue and a **separate target
repository**, derives bounded repository context, asks a model for a structured
plan, executes only allowed actions in an isolated Git worktree, verifies the
result, and returns evidence-backed terminal state.

```text
issue + target repository
          |
          v
  understand -> explore -> plan -> execute -> verify
                                      |          |
                                      +-> diagnose/recover (bounded)
                                                 |
                                                 v
                                      verified diff or safe failure
```

The model proposes; the runtime controls permissions, budgets, worktrees,
verification, evidence, transfer, and terminal status. The optional desktop
surface only submits a bounded run request and visualizes engine events. It
does not contain model keys, shell execution, planning rules, or write access.

## 2. Execution paths

| Path | Entry point | Role |
| --- | --- | --- |
| Rust engine | `make run` | Default evaluation path and final runtime direction |
| Rust gateway | `secondego-gateway` | Loopback-only event/run API for the optional UI |
| Python engine | `ENGINE=python make run` | Verified compatibility/reference implementation |
| Python CLI | `secondego solve` | Explicit deterministic plan or provider-backed run |
| Desktop observer | `make ui` or `make run UI=on` | Optional macOS notch companion; never needed for judging |

The root Makefile remains the evaluator contract:

```bash
export AI_API_KEY="<organizer-provided-key>"
make setup
make run
make test
```

`make run` is headless by default. It starts no Electron process, browser, or
notch window. It prompts for a local target path or a public HTTPS GitHub URL,
then collects a multi-line mission. `make ui` is deliberately opt-in.

## 3. Provider architecture

Provider code is confined to `secondego-model` (Rust) and `src/SecondEgo/model`
(Python). The runtime and UI depend only on the structured proposal contract.

```text
SECONDEGO_PROVIDER + SECONDEGO_MODEL + AI_API_KEY
                         |
                         v
          ConfiguredProvider / build_model_planner
                         |
              +----------+----------+
              |                     |
              v                     v
     DeepSeekProvider        GeminiProvider (opt-in)
              |                     |
              +----------+----------+
                         |
                         v
        ActionProposal { action, arguments, rationale }
```

### Current provider policy

| Setting | Default | Purpose |
| --- | --- | --- |
| `SECONDEGO_PROVIDER` | `deepseek` | Selects the adapter; valid values are `deepseek`, `gemini` |
| `SECONDEGO_MODEL` | `deepseek-flash` for DeepSeek | Optional organizer/developer model override |
| `AI_API_KEY` | none | The only credential environment variable read by either adapter |
| `SECONDEGO_DEEPSEEK_BASE_URL` | `https://api.deepseek.com` | Development/test endpoint override for DeepSeek only |

DeepSeek is the evaluation default and uses its OpenAI-compatible
`/chat/completions` endpoint. The adapter requests JSON-only output and disables
thinking so the runtime receives a bounded `action`, `arguments`, `rationale`
object. Gemini remains available for development through
`SECONDEGO_PROVIDER=gemini`; the Python compatibility path requires the optional
`.[gemini]` package in that mode. Provider transport failures become bounded run
errors; raw provider responses and credentials are not stored in evidence.

Examples:

```bash
# Evaluation/default route
export SECONDEGO_PROVIDER=deepseek
export AI_API_KEY="<provided-key>"
make run

# Explicit provider selection from the Python CLI
PYTHONPATH=src .venv/bin/python -m SecondEgo.cli solve \
  --repo /path/to/target --issue "Fix pagination" --provider deepseek

# Optional Gemini compatibility route
export SECONDEGO_PROVIDER=gemini
.venv/bin/pip install '.[gemini]'
```

## 4. Runtime lifecycle and safety

### State and resource boundaries

The engine has explicit phase, status, resource, event, and termination data.
The lifecycle is:

1. **Initialize / understand** — validate the target source and task.
2. **Explore** — scan the target and build/rebuild the repository index.
3. **Plan** — assemble a bounded, source-linked context packet and validate the
   structured provider response.
4. **Execute** — dispatch only allowed file, search, Git, and command tools.
5. **Verify** — run evidence-producing checks and classify failure.
6. **Diagnose / recover** — at most the configured bounded recovery loop, using
   failure-specific retrieval rather than an unbounded transcript.
7. **Terminate** — transfer only a verified diff, or retain explicit failure,
   blocked, or cancelled evidence.

`ResourceUsage` bounds model calls, tool calls, retries, context, and elapsed
time. `ContextAssembler` ranks and caps evidence. `ToolRouter` and
`WorkspacePolicy` keep file and command work inside the target boundary. The
model never receives direct, unrestricted shell or filesystem control.

### Git transaction policy

Real mutations occur in a detached worktree via `GitAttemptTransaction`:

```text
clean target repository -> detached attempt -> edits/tests
                                      |             |
                              verified diff     failed attempt
                                      |             |
                            transfer to target  discard
```

The current transactional path intentionally blocks dirty repositories and
repositories without an initial commit. This is safer than attempting to merge
unknown local work. Test changes are guarded by policy and verification is
command evidence, never model narration.

## 5. Repository sources, persistence, and cleanup

### Target inputs

The runtime accepts either:

- a local repository path; or
- a public credential-free HTTPS GitHub repository URL.

Remote sources are shallow-cloned into an owned temporary directory. Private
repositories, embedded credentials, SSH URLs, branch selection, pull requests,
and automatic push-back are intentionally out of scope. Successful runs clean
their temporary clone; failures leave only owned, leased state for recovery.

### SQLite and garbage collection

SQLite records compact run state, events, and evidence. It does **not** persist
repository contents or credentials. The database is rebuildable support state,
not a source-of-truth code store.

Temporary clones and detached worktrees are marked as SecondEgo-owned and
lease-protected. `collect_garbage()` runs on entry and `make gc` can trigger it
explicitly. The collector only reclaims stale, inactive, owned directories; it
does not scan or delete arbitrary user paths. The detailed policy is in
[`GARBAGE_COLLECTION.md`](GARBAGE_COLLECTION.md).

## 6. Desktop/UI architecture

The visual layer is optional and observation-only:

```text
React renderer -> local token -> Rust/Python loopback gateway -> engine events
       |                                                          |
       +---- no model key / no shell / no direct file mutation ---+
```

`apps/desktop` provides a React/Vite renderer and two local shells:

- a macOS native `WKWebView` notch companion, preferred by `make ui` on macOS;
- an Electron fallback shell for non-macOS development.

The native window is a transparent, focused borderless strip at the top of the
screen. It opens on hover or click from `260 × 46` to a bounded `1024 × 720`
workspace, constrained to the current display. The renderer asks the native
shell only to resize; it never owns native policy.

The refreshed interface contains:

- an original coral/parchment SecondEgo mark inspired by, but not copied from,
  `Potential Logo.png`;
- an accessible mission form with autofocus after opening, visible focus state,
  and configurable loopback connection details;
- a spaced working-village map where each worker is driven by a real engine
  phase event;
- a run state, live transcript, verification outcome, and changed-path evidence;
- reduced-motion behavior and responsive fallbacks.

No view invents progress. A worker is active only when the latest engine event
names that phase, and it becomes visited only after an event exists for it.

## 7. Component map

| Location | Responsibility |
| --- | --- |
| `engine-rs/crates/secondego-core` | phases, terminal statuses, events, budgets |
| `engine-rs/crates/secondego-repository` | bounded scanning/indexing/retrieval |
| `engine-rs/crates/secondego-context` | evidence ledger and context budgets |
| `engine-rs/crates/secondego-model` | scripted, DeepSeek, and Gemini structured adapters |
| `engine-rs/crates/secondego-tools` | workspace policy, tools, worktrees, Git evidence |
| `engine-rs/crates/secondego-verification` | command evidence and failure classification |
| `engine-rs/crates/secondego-runtime` | orchestration, recovery, source resolution, cleanup |
| `engine-rs/crates/secondego-storage` | local SQLite run/event persistence |
| `engine-rs/crates/secondego-gateway` | token-protected loopback API and static UI serving |
| `src/SecondEgo` | Python reference/compatibility implementation |
| `apps/desktop` | optional React/Electron/native event observer |

## 8. Verification and current limitations

Relevant checks:

```bash
make test
make rust-test
make rust-check
npm --prefix apps/desktop run build
make ui          # macOS visual smoke check; optional
```

Known limits that should be stated rather than hidden:

- broad language-aware indexing remains incomplete; Python is the strongest
  parser path and other languages use structural/lexical fallback;
- provider-native token accounting is not yet uniform; preflight budgets use a
  deterministic local estimate;
- the desktop shell is an un-packaged development companion, not a signed
  distributable installer;
- only public HTTPS GitHub rehearsal sources are supported;
- organizer telemetry/reporting schemas must be added before claiming protocol
  compliance beyond the current internal evidence and SQLite contracts.

## 9. Change record

- **2026-09-27:** made DeepSeek the default provider in Rust and Python while
  retaining Gemini as an explicit adapter; both read only `AI_API_KEY`.
- **2026-09-27:** refreshed the optional notch UI, enlarged the bounded open
  workspace, and replaced the logo with an original mark plus prompt provenance.
- **2026-09-27:** confirmed the headless Makefile flow remains the primary
  evaluation path and the UI cannot alter engine budgets or policy.

For high-level decisions, see [`CONTEXT.md`](CONTEXT.md). For evaluator
boundaries, see [`LCC_EVALUATION_MODEL.md`](LCC_EVALUATION_MODEL.md). For the
original architecture rationale, see [`CONTEXT-1.md`](CONTEXT-1.md).
