# SecondEgo workspace rules

- The product is named **SecondEgo**. Use `SecondEgo` in user-facing text, documentation, telemetry labels, and UI copy. Do not use `Forge` as an alias.
- Keep the Python agent engine independent from the Electron/React desktop shell. UI code observes engine events; it must not become the orchestration authority.
- Keep the evaluation path runnable without the desktop shell. The core harness, tests, transcript, telemetry, and final diff are the primary correctness surface.
- Prefer local, rebuildable state in SQLite over introducing a graph database or distributed service prematurely.
- Do not add product/runtime code while the task is limited to scaffolding, rules, skills, or design documentation.
- Never expose secrets, raw credentials, unrestricted shell access, or unbounded filesystem access through the agent or UI.

