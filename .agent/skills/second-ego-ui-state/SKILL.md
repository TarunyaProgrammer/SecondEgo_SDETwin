---
name: second-ego-ui-state
description: Design the SecondEgo pixel-village interface as an honest visualization of agent phases, tools, progress, and verification evidence.
---

# SecondEgo UI state visualization

Use this skill for the pixel-village UI, agent/building interactions, progress views, run timelines, or visual telemetry.

## Rules

- Every visible state must map to a real engine event or clearly be labeled as presentation-only.
- Do not invent progress percentages. Use phase status, completed actions, test counts, or explicit indeterminate states.
- Show the current task, phase, active tool, affected paths, test result, failure reason, and recovery status before decorative animation.
- Make failure, cancellation, blocked, and partial-change states as visible as success.
- Keep the village a visualization, not a second orchestration system. UI interactions should dispatch bounded commands to the engine.
- Ensure the core run remains understandable without animation, color, hover, or audio.

