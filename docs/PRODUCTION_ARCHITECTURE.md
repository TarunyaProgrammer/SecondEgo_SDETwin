# SecondEgo production architecture

This document is the implementation contract for moving SecondEgo from a hackathon/evaluation harness to a downloadable local developer product.

It is intentionally explicit about what exists today, what is missing, and what must not be confused with a production guarantee.

## 1. Current architecture

```text
CLI / TUI --------------------------┐
                                    v
                         Rust runtime engine (default)
                         - state machine
                         - repository discovery/retrieval
                         - model provider enum
                         - bounded tools
                         - Git worktree isolation
                         - verification + recovery
                         - SQLite evidence
                                    ^
Electron + React -- loopback gateway

Python runtime: compatibility/reference path during the Rust migration.
Marketing website: static Vite/React application in web/.
```

The current source of truth is in `engine-rs/crates/secondego-runtime`. The Python implementation under `src/SecondEgo` remains useful for compatibility and tests, but new product behavior should be added to the Rust path first and then mirrored only when the compatibility contract requires it.

### Current strengths

- Explicit execution phases and terminal outcomes.
- Bounded repository retrieval and model context.
- Detached worktree execution with verified diff transfer.
- Local, rebuildable SQLite evidence storage.
- Renderer does not own shell access or core orchestration.
- Headless evaluation path remains independent of Electron.

### Current production gaps

1. Provider configuration is not a product-level registry. The required set is not present: DeepSeek, Gemini, OpenAI, Anthropic, and OpenRouter must all be first-class; Groq is currently present but is not one of the five required providers.
2. Credentials are modeled as one `AI_API_KEY` plus provider-specific exceptions. That does not support multiple saved provider profiles safely.
3. The desktop gateway is an observer API, not a versioned product API. Run payloads do not yet expose a complete diff model, replay metadata, or a durable profile/workspace contract.
4. The Electron shell has a visual notch/village concept but no release-grade settings, provider setup, diff review, update, or failure recovery experience.
5. Packaging is a development launcher. There is no signed/notarized release pipeline, update channel, checksums, or supported installer contract.
6. The terminal path is functional but not a stable, documented product interface comparable to modern coding-agent CLIs.
7. SQLite stores useful evidence but is not yet a migration-managed replay database with configuration snapshots and provider/model metadata.

### Increment delivered in this repository

The provider transport layer now has first-class Rust and Python compatibility
adapters for DeepSeek, Gemini, OpenAI, Anthropic, and OpenRouter, plus the
existing Groq adapter. The CLI/Makefile recognize named credentials, preserve
`AI_API_KEY` only as a migration fallback, and never print key values. This is
not yet the complete product profile system: OS keychain storage, authenticated
gateway settings, multi-profile selection, and renderer isolation are still
required before calling the desktop experience production-ready.

## 2. Target architecture

```text
                 terminal CLI/TUI       Electron desktop
                        |                      |
                        +------ local API -----+
                               (versioned)
                                  |
                  profile + workspace service
                  - OS keychain credentials
                  - provider/model selection
                  - workspace permissions
                                  |
                  Rust engine / orchestration core
                  - explicit state machine
                  - repository intelligence
                  - provider registry
                  - policy + tool router
                  - isolated attempts
                  - verification + diff service
                  - event journal
                                  |
               SQLite metadata + filesystem workspaces
```

The engine remains local-first. The UI and terminal are two control surfaces over the same local API; neither is allowed to implement a second orchestration loop.

### Ownership rules

| Concern | Owner | Rule |
| --- | --- | --- |
| Task lifecycle | Rust runtime | UI renders events; it does not infer state. |
| Provider calls | `secondego-model` | No provider HTTP code in CLI, gateway, or React. |
| Credentials | profile/credential service | UI never persists raw keys in React state, SQLite, logs, or events. |
| Workspace access | policy/tool router | Every path is canonicalized and confined to the selected workspace. |
| Diff generation | engine diff service | UI receives normalized file/hunk data, not only raw terminal text. |
| Verification | verification crate | Completion requires tool evidence, not model narration. |
| Persistence | storage crate | Schema migrations and bounded event/evidence records are explicit. |
| Presentation | terminal/Electron | Animation is optional; all essential state remains readable without it. |

## 3. Provider and API-key architecture

### Required provider contract

Introduce a provider registry with one typed configuration per provider:

```text
ProviderRegistry
  resolve(provider_id, model_id, credential_profile)
  capabilities(provider_id)
  validate_configuration(provider_id)
  generate(request) -> ProviderResponse
  stream(request) -> ProviderEvent
  estimate_tokens(request) -> TokenEstimate
```

Every adapter must normalize into the same internal request/response types:

- structured action/proposal response;
- streaming text and tool-progress events where supported;
- provider request id and safe rate-limit metadata;
- retry classification (`auth`, `rate_limit`, `timeout`, `overloaded`, `invalid_response`, `network`);
- cancellation and timeout support;
- no raw credential or prompt logging.

### Provider matrix

| Provider | Key | Transport | Default model strategy |
| --- | --- | --- | --- |
| DeepSeek | `DEEPSEEK_API_KEY` | OpenAI-compatible HTTPS | provider default, user override |
| Gemini | `GEMINI_API_KEY` | Google GenAI REST/SDK adapter | provider default, user override |
| OpenAI | `OPENAI_API_KEY` | OpenAI Responses/Chat adapter | provider default, user override |
| Anthropic | `ANTHROPIC_API_KEY` | Anthropic Messages adapter | provider default, user override |
| OpenRouter | `OPENROUTER_API_KEY` | OpenAI-compatible HTTPS with routing headers | required model id, user override |

Keep `AI_API_KEY` as a read-only migration alias for one release cycle. It must produce a warning and must never win over an explicitly named provider key. Do not silently map an Anthropic or Gemini key into an OpenAI-compatible adapter.

### Credential lifecycle

Terminal:

```text
secondego provider list
secondego provider add openai --key-stdin
secondego provider test openai
secondego provider use openai --model gpt-5
```

Desktop:

```text
Settings → Providers → Add provider → paste key → local validation → keychain write
```

The raw key is accepted only at the boundary, sent to the OS keychain (`Keychain` on macOS, Credential Manager on Windows, Secret Service/libsecret on Linux), and replaced with a profile id. The engine receives a short-lived in-memory credential handle. SQLite stores provider id, model id, keychain reference, created/updated timestamps, and last validation status—never the secret.

The “Test connection” action must make a minimal provider request with a fixed non-user prompt, return a classified result, and redact all error text before showing or persisting it.

## 4. Desktop and terminal product surfaces

### Desktop information architecture

The current notch should become an entry point, not the entire application:

1. Command surface: new task, recent runs, provider switch, open workspace.
2. Run view: phase timeline, active tool, affected files, verification, cancel/retry.
3. Diff review: file tree, unified/split diff, inline comments, copy patch, apply/reject.
4. Settings: providers, profiles, workspaces, permissions, updates, diagnostics.
5. Run history: searchable local runs with terminal status and evidence links.

The notch stays useful for glanceable state, but every action must expand into a full run view. Decorative village animation cannot be the only indication of failure, cancellation, or blocked permissions.

### Terminal product contract

The terminal should use a stable event renderer with:

- one-line phase header and run id;
- collapsible tool activity;
- live streaming model output only when it is useful, never as a source of truth;
- explicit `PLAN`, `EXECUTE`, `VERIFY`, `RECOVER`, and `DIFF` sections;
- keyboard commands: cancel, pause, expand evidence, open diff, retry, quit;
- final summary containing status, changed paths, verification commands, and next action;
- `--json` and `--no-color` modes for scripts and CI.

The Rust CLI should own rendering. The Python TUI should remain a compatibility surface until the migration is complete.

## 5. Diff correctness contract

The engine must emit a normalized diff payload instead of relying on a UI to parse raw `git diff` text:

```json
{
  "schema_version": 1,
  "files": [
    {
      "path": "src/example.py",
      "status": "modified",
      "additions": 2,
      "deletions": 1,
      "hunks": [
        {"old_start": 10, "old_lines": 3, "new_start": 10, "new_lines": 4, "lines": []}
      ]
    }
  ],
  "truncated": false
}
```

Raw diff remains available as bounded evidence, but it is not the primary UI contract. The gateway and Electron types must share the same versioned schema. Add fixture tests for additions, deletions, renames, binary files, empty diffs, large diffs, and malformed patches.

## 6. Packaging and downloads

The website should point only to signed GitHub Release artifacts or the eventual release CDN. Do not ship a “download” button that silently points at a development build.

Release pipeline:

1. CI builds Rust binaries and Electron bundles per target.
2. Electron Builder creates a macOS DMG, Windows installer, and Linux AppImage/deb.
3. macOS builds are code-signed and notarized; release checksums are published.
4. The DMG uses the standard macOS affordance: branded background, app icon, Applications shortcut, readable install instruction, and no destructive custom installer workflow.
5. First launch performs a preflight: engine binary, Git, writable data directory, keychain access, and optional camera/voice permissions.
6. In-app updates are opt-in, signed, resumable, and rollback-safe.

The DMG itself should stay conventional. Custom animations inside a disk image do not improve trust and can make installation feel suspicious. Put motion into first-run onboarding and the app’s command surface instead.

## 7. Scale and maintainability

### Local multi-profile support (first production target)

- profile id is the root of provider credentials, workspace permissions, and UI preferences;
- every run carries profile id, workspace id, repository revision, provider id, model id, and engine version;
- profile data is isolated in the OS app data directory;
- no network account is required;
- export/import is encrypted or explicitly redacted.

### True multi-user service (separate product)

Do not call the local profile model “multi-tenant”. A hosted service would additionally require authenticated tenants, server-side secret management, durable job queues, worker isolation, repository credential handling, audit logs, quotas, billing, and a remote execution threat model. That should be a later architecture, not hidden inside the desktop gateway.

### Required engineering hygiene

- version every public event and API payload;
- add migration tests for SQLite schema changes;
- centralize provider and model defaults;
- add contract tests for each provider with recorded/redacted fixtures;
- cap all model, tool, output, diff, and event sizes;
- record safe configuration snapshots for replay;
- add crash recovery and cancellation tests;
- keep repository indexes rebuildable and incremental rather than introducing a graph service prematurely.

## 8. File-level implementation plan

### Phase 0 — contracts and docs

- `docs/PRODUCTION_ARCHITECTURE.md` — this contract.
- `docs/diagrams/second-ego-hld.mmd` — system-level diagram.
- `docs/diagrams/provider-key-lld.mmd` — credential/provider sequence.
- `web/src/App.tsx`, `web/src/styles.css` — user-facing installation/docs site.
- `README.md` — link production quick start and supported providers.

### Phase 1 — provider registry and credentials

- `engine-rs/crates/secondego-model/src/lib.rs` — split provider enum/configuration from transport adapters; add OpenAI, Anthropic, and OpenRouter adapters.
- `engine-rs/crates/secondego-cli/src/main.rs` — provider/profile subcommands and non-interactive key input.
- `engine-rs/crates/secondego-storage/src/lib.rs` — profile metadata and migration-managed credential references.
- `engine-rs/crates/secondego-gateway/src/main.rs` — authenticated provider/profile endpoints with no secret return path.
- `src/SecondEgo/config.py`, `src/SecondEgo/model/*` — compatibility registry and named-key resolution.
- `Makefile` — replace the single-key dotenv loader with named provider keys while retaining a warned migration alias.

### Phase 2 — API/event/diff contracts

- `engine-rs/crates/secondego-core/src/lib.rs` — versioned event payloads and diff types.
- `engine-rs/crates/secondego-runtime/src/lib.rs` — emit normalized diff and replay metadata.
- `engine-rs/crates/secondego-gateway/src/main.rs` — run history, events, diff, and provider status endpoints.
- `apps/desktop/src/types.ts` — generated/manual mirror of the versioned contract.
- `apps/desktop/src/App.tsx` — run timeline, diff review, settings, and explicit failure states.

### Phase 3 — terminal and desktop UX

- `engine-rs/crates/secondego-cli/src/main.rs` — stable renderer modes and keyboard command contract.
- `src/SecondEgo/tui.py` — compatibility renderer only; no new engine authority.
- `apps/desktop/src/styles.css`, `apps/desktop/src/components/*` — command surface, run history, diff viewer, provider settings.
- `apps/desktop/electron/main.cjs`, `preload.cjs` — keychain bridge, lifecycle, deep links, update hooks.

### Phase 4 — packaging and operations

- `apps/desktop/package.json` — Electron Builder configuration and release scripts.
- `.github/workflows/release.yml` — matrix builds, signing, notarization, checksums.
- `apps/desktop/native/Info.plist` — bundle identifiers, permissions, update metadata.
- `README.md`, `SECURITY.md`, `CONTRIBUTING.md` — install, support, incident, and release policy.

## 9. Definition of production readiness

The app is not production-ready when it merely launches. The minimum bar is:

- all five required providers can be configured independently from terminal and UI;
- keys never appear in renderer state, logs, SQLite, diffs, prompts, or subprocess environments;
- a clean install can run a provider preflight and a deterministic fixture without repository mutation;
- every run can be resumed or explained from versioned events, safe config metadata, diff data, and verification evidence;
- failed, blocked, cancelled, and partial states are first-class in both terminal and desktop surfaces;
- signed release artifacts install cleanly on supported platforms;
- tests cover provider contracts, key redaction, path policy, diff rendering, cancellation, recovery, migrations, and packaging smoke checks.
