---
name: second-ego-desktop
description: Design or implement the SecondEgo Electron/React desktop shell, its local engine boundary, and safe event-driven UI integration.
---

# SecondEgo desktop shell

Use this skill for desktop packaging, Electron/React structure, IPC or local HTTP/WebSocket communication, and shell-level UX.

## Rules

- Keep the Python engine authoritative for task state, tool execution, verification, and termination.
- Treat the desktop as a presentation and control surface: submit tasks, display events, inspect evidence, and request safe user actions.
- Define a versioned event contract before wiring screens to engine internals. Events should include run ID, phase, timestamp, status, and useful evidence references.
- Prefer localhost IPC or a local API with explicit authentication/ownership checks over arbitrary renderer-to-shell access.
- Keep the evaluation harness runnable headlessly; the UI is not a dependency of correctness.
- Do not put model keys, shell privileges, or direct unrestricted filesystem APIs in renderer code.

