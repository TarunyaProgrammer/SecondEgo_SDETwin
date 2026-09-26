# SecondEgo

SecondEgo is an autonomous coding-agent harness. It surrounds a text-only
foundation model with repository intelligence, structured planning, bounded
tools, verification, recovery, and auditable run state.

The evaluation-critical path is headless. The Electron/React interface is an
optional observer of engine events; it is not the source of truth for planning,
tool execution, permissions, or verification.

## Organizer quick start

From the repository root, the standard evaluation flow is:

```bash
export AI_API_KEY="<PROVIDED_API_KEY>"
make setup
make run
```

`make run` launches the Rust terminal harness, prompts for the repository, then
opens a multi-line mission editor. Paste or type the complete issue and finish
with a line containing `.done` (Ctrl-D also ends input). The harness then shows
live phase/tool evidence and a final machine-readable JSON report.

The evaluator-facing default is headless: `make run` does not start Electron,
the notch shell, or any browser window. If you want the entire local judge flow
in one command, use `make judge`.

Run the regression suite separately:

```bash
make test
```

The required Makefile interface is:

| Command | Purpose |
| --- | --- |
| `make setup` | Create the Python environment, install dependencies, and build the Rust CLI and gateway. |
| `make run` | Launch the default Rust evaluation harness. |
| `make judge` | Run setup and then launch the same headless harness in one command. |
| `make test` | Run the Python contract/evaluation tests. |
| `make clean` | Remove generated Python/build artifacts. |
| `make gc` | Collect stale SecondEgo-owned temp clones/worktrees using the bounded retention policy. |

Rust-specific checks are also available:

```bash
make rust-test
make rust-check
make rust-build-release
```

## What the evaluator should expect

The harness expects:

- Python 3.12 or newer;
- Rust/Cargo;
- Node.js/npm only when building the optional desktop shell;
- Git;
- a clean Git repository with an initial commit;
- the prescribed text-model credential in `AI_API_KEY`.

The official evaluation should use the organizer-provided repository, issue,
tests, model, and evaluation rules. A GitHub URL is supported for development
rehearsals, but external network access, private-repository credentials, and
additional model providers must not be assumed during the official evaluation.

The default model is `gemini-3.8-flash`. An organizer-prescribed model can be
selected with:

```bash
export SECONDEGO_MODEL="<PRESCRIBED_MODEL>"
```

The harness reads the API key only from `AI_API_KEY`. Credentials must not be
committed, placed in `.env`, included in prompts, or written to run evidence.

## Execution flow

The engine follows this bounded flow:

```text
issue
  -> understand
  -> explore repository
  -> plan structured actions
  -> execute in detached Git worktree
  -> run verification commands
  -> diagnose and perform bounded recovery when configured
  -> transfer only verified changes
  -> terminate with evidence
```

The terminal reports explicit terminal outcomes:

- `COMPLETE`: verification passed and the verified diff was transferred;
- `FAILED`: execution or verification failed;
- `BLOCKED`: a safety, environment, repository, or policy requirement prevented execution;
- `CANCELLED`: execution was cancelled.

The engine does not treat model narration as proof. Completion is based on
command results, test results, Git evidence, changed paths, and termination
state.

## Repository inputs

The CLI accepts either a local repository path or a public HTTPS GitHub URL:

```bash
PYTHONPATH=src .venv/bin/python -m SecondEgo.cli solve \
  --repo /path/to/evaluation-repository \
  --issue "Fix authentication timeout handling" \
  --gemini \
  --model "$SECONDEGO_MODEL"
```

For a development rehearsal:

```bash
make run
# Repository path or GitHub URL: https://github.com/owner/repository
# Mission / issue: paste a multi-line task
# .done
```

Remote sources are acquired as bounded shallow clones into a temporary local
Git repository. The normal clean-worktree transaction and verification path is
then used. Successful runs remove the clone; the crash-safe collector reclaims
stale leftovers. SecondEgo does not push changes back to GitHub.

Remote acquisition currently supports public HTTPS GitHub URLs only. It rejects
credentials, query strings, unsupported protocols, and malformed repository
paths. Private repositories, SSH remotes, branch selection, pull requests, and
automatic push-back are not supported.

## Deterministic rehearsal

The repository includes a deliberately buggy pagination target. It is separate
from the harness repository and can be used to rehearse the edit/test/diff path:

```bash
TARGET_DIR="$(mktemp -d)/pagination-fixture"
mkdir -p "$TARGET_DIR"
cp -R evaluation/fixture_repo/. "$TARGET_DIR/"
git -C "$TARGET_DIR" init -q
git -C "$TARGET_DIR" config user.email evaluation@example.com
git -C "$TARGET_DIR" config user.name "SecondEgo Evaluation"
git -C "$TARGET_DIR" add .
git -C "$TARGET_DIR" -c commit.gpgSign=false commit -qm baseline

PATH="$PWD/.venv/bin:$PATH" .venv/bin/secondego solve \
  --repo "$TARGET_DIR" \
  --issue "Fix pagination so page 1 returns the first page and page 2 returns the second page." \
  --plan evaluation/fixture_plan.json

git -C "$TARGET_DIR" diff
```

The target must be separate from SecondEgo. The transaction layer rejects dirty
repositories and repositories without an initial commit so that failed attempts
can be discarded safely.

## Test integrity and one-shot evaluation

The LCC evaluation rules require existing tests to remain protected. During an
official run, the harness must not:

- modify, delete, rename, skip, or disable tests;
- alter assertions or test configuration to bypass failures;
- manually intervene after the prompt is frozen;
- modify the harness or restart the evaluation;
- use unauthorized models or external services.

The harness may make multiple internal model calls, tool calls, test runs, and
bounded recovery attempts. “One-shot” applies to the team’s evaluation attempt,
not to the number of internal model calls.

## Evidence and persistence

Every run produces structured state and evidence for:

- user task and acceptance criteria;
- repository scan and ranked retrieval;
- state/phase transitions;
- model and tool usage counters;
- commands and verification results;
- failures, retries, and recovery;
- changed paths and Git diff evidence;
- final termination status and reason.

SQLite stores compact run state, events, and evidence. It is not a source-code
blob store: repository files and temporary clones remain on disk where tools can
operate on them. An optional database can be supplied to the Python CLI:

```bash
PYTHONPATH=src .venv/bin/python -m SecondEgo.cli solve \
  --repo /path/to/repository \
  --issue "Fix the issue" \
  --plan plan.json \
  --state-db /path/to/secondego-runs.db
```

The current repository contains internal versioned events and SQLite run
persistence. The organizer-supplied LCC telemetry protocol, telemetry schema,
reporting instructions, and standard report schema are not present in this
checkout yet; they must be added or mapped before claiming full protocol
compliance.

## Optional presentation modes

The default evaluation mode is headless. Compact engine events can be displayed
without changing model/tool budgets:

```bash
make run UI_MODE=events
```

The terminal interface is intentionally compact and state-led. It shows the
current execution phase, tool/verification events, and final outcome without
inventing progress percentages or agent activity.

The desktop UI is an explicit opt-in switch. It is off by default:

```bash
make ui                 # build and open the macOS notch companion
make run UI=on          # same opt-in through the standard launcher
make run UI_MODE=events # headless run with compact terminal events
```

`UI=off` is the default and is the mode the evaluator should use. The UI is an
observer only; turning it on never changes planning, tools, permissions, or
verification.

The optional browser observer is started with:

```bash
make desktop
```

The Electron/React observer is started with:

```bash
make desktop-electron
```

The renderer only submits bounded requests and displays engine events. It does
not hold model keys, execute shell commands, or write repository files directly.

## Architecture

```text
                 ┌──────────────────────────────┐
task/repository  │ CLI / TUI / Electron observer │
        ────────>└──────────────┬───────────────┘
                                │ local boundary
                    ┌───────────▼───────────┐
                    │ Rust orchestration    │
                    │ state + planning      │
                    └──────┬───────┬────────┘
                           │       │
              ┌────────────▼─┐   ┌─▼─────────────┐
              │ repository   │   │ bounded tools │
              │ index/context│   │ + Git attempt │
              └──────────────┘   └──────┬────────┘
                                        │
                              ┌─────────▼─────────┐
                              │ verify / recover  │
                              └─────────┬─────────┘
                                        │
                              ┌─────────▼─────────┐
                              │ SQLite evidence   │
                              └───────────────────┘
```

### Main components

- `secondego-core`: phases, terminal statuses, events, and resource budgets;
- `secondego-repository`: bounded scanning, Tree-sitter Python indexing, test/import links, and retrieval;
- `secondego-context`: source-linked evidence ledger and context budgets;
- `secondego-model`: structured provider boundary with scripted and Gemini providers;
- `secondego-tools`: path safety, command policy, file/search tools, Git evidence, and detached worktrees;
- `secondego-verification`: command execution evidence and failure classification;
- `secondego-runtime`: Rust orchestration loop and bounded recovery;
- `secondego-storage`: local SQLite run/event persistence;
- `secondego-cli`: evaluator-facing Rust entry point;
- `src/SecondEgo`: Python reference/compatibility runtime;
- `apps/desktop`: optional Electron/React event observer.

## Design decisions

1. Rust is the default evaluator-facing engine; Python remains a compatibility/reference path.
2. Model access is behind a provider interface; orchestration does not depend on provider-specific calls.
3. Repository intelligence is derived and rebuildable rather than stored in an external graph service.
4. The tool surface is small and policy-controlled instead of exposing unrestricted shell/filesystem access.
5. Each coding attempt runs in a detached Git worktree. Failed attempts are discarded; only verified diffs are transferred.
6. Context is ranked, source-linked, bounded, and retained by evidence rather than by an unbounded transcript.
7. The desktop shell observes engine truth; it does not contain agent logic.
8. SQLite stores compact execution evidence, not repository contents or secrets.
9. Correctness and objective evidence take priority over model-generated claims or decorative UI.

## Known limitations and evaluation gaps

The current implementation is an executable vertical slice, not a complete
production coding platform. Known gaps are:

- the official LCC telemetry/reporting packages are not yet included;
- provider-native token counts are not yet captured consistently;
- telemetry and report schemas are internal rather than organizer-standardized;
- broad language parsing is incomplete; Python has the strongest indexing support;
- the benchmark fixture set is small;
- the Electron shell is an observer, not a packaged installer;
- remote GitHub support is for development rehearsal and public HTTPS sources;
- private repository credentials and automatic GitHub publishing are intentionally absent;
- full clean-checkout, cross-process, and real-provider evaluation rehearsal should be performed in the organizer environment.

These limitations should be resolved or explicitly accepted before an official
judging run. The repository should not claim compliance with organizer-provided
telemetry or reporting schemas until those files and validation checks are
present.

## Repository guidance

- [`AGENTS.md`](AGENTS.md): instructions for coding agents working in this repository.
- [`docs/context/CONTEXT.md`](docs/context/CONTEXT.md): current decisions and open questions.
- [`docs/context/CONTEXT-1.md`](docs/context/CONTEXT-1.md): initial harness architecture proposal.
- [`docs/context/CONTEXT-2.md`](docs/context/CONTEXT-2.md): desktop product-shell proposal.
- [`docs/context/LCC_ARCHITECTURE.md`](docs/context/LCC_ARCHITECTURE.md): evaluation-oriented architecture decisions.
- [`evaluation/README.md`](evaluation/README.md): deterministic target-repository rehearsal.
- [`CONTRIBUTING.md`](CONTRIBUTING.md): contribution workflow.
- [`SECURITY.md`](SECURITY.md): security expectations and reporting.
- [`docs/context/GARBAGE_COLLECTION.md`](docs/context/GARBAGE_COLLECTION.md): temporary-state lifecycle policy.

## License

Released under the MIT License. See [`LICENSE`](LICENSE).
