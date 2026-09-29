# SecondEgo evaluator instructions

SecondEgo follows the required Makefile workflow. From a clean checkout, use
only the organiser-provided credential and the commands below:

```bash
export AI_API_KEY="<PROVIDED_API_KEY>"
make setup
make run
```

`make run` launches the headless interactive Rust harness. It asks for the
repository path or public GitHub URL, then for the complete text issue. Finish
the multi-line issue editor with a line containing `.done` (or Ctrl-D).

The launcher may show a read-only preflight dashboard first. The default
judge-safe profile keeps the desktop UI, voice narration, camera, and gestures
off. It never prints credentials. `NONINTERACTIVE=1 PROFILE=judge` skips the
profile prompt for scripted local rehearsals.

Do not create or edit `.env`; the evaluator credential is read directly from
`AI_API_KEY`. The default path does not launch Electron, a browser, a camera,
or any multimodal model.

## What `make setup` does

The target validates Git and Cargo, then builds the evaluator CLI and local
gateway from the committed `engine-rs/Cargo.lock` using `--locked`. Cargo
downloads all Rust crates required by the evaluation path automatically.

The prescribed evaluator host must provide:

- Git;
- Rust/Cargo 1.85 or newer (Edition 2024);
- network access for the first Cargo dependency download.

No Node, npm, Python package, desktop UI, database server, Docker daemon, or
manually-entered project configuration is required for `make setup` or
`make run`. Node/npm are bootstrapped automatically only by optional Electron
commands; Python test packages are bootstrapped automatically only by
`make test`.

## Model and credential contract

The standard model boundary is text-only. The Rust evaluator runtime defaults
to DeepSeek / `deepseek-flash` when no provider/model is exported. If organisers
prescribe a different DeepSeek model, they can provide `SECONDEGO_MODEL` as an
environment variable without modifying the submission. The application reads the
credential only from `AI_API_KEY`; no credential is committed, logged, or
required in an `.env` file.

## Commands

| Command | Evaluator purpose |
| --- | --- |
| `make setup` | Install locked evaluation-path dependencies and build the release harness. |
| `make run` | Launch the required interactive, headless harness. |
| `make test` | Install test-only Python dependencies, run Python contracts, then run the Rust suite. |
| `make clean` | Remove generated Python and test artefacts. |

`make config` is a local diagnostic only. It reports the resolved provider,
model, and whether a key is present, but never prints a credential.

The harness reports terminal outcomes as `COMPLETE`, `FAILED`, `BLOCKED`, or
`CANCELLED`, with structured event evidence and a final JSON report.

## Submission checklist

- Root `Makefile` exposes `setup`, `run`, `test`, and `clean`.
- `make setup` and `make run` are sufficient for the evaluation path.
- API credentials remain external in `AI_API_KEY`.
- The default harness is text-only and configured through environment
  variables, not source edits.
- Runtime dependencies are declared in `Cargo.toml` and resolved by the
  committed `Cargo.lock`.
