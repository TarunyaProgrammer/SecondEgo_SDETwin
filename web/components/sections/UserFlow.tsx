"use client";

import { useState } from "react";
import { Reveal } from "@/components/Reveal";

const steps = [
  {
    id: "01",
    phase: "INPUT",
    title: "Select a repository",
    body: "Point SecondEgo at a local path or a public GitHub URL. For remote URLs, a shallow single-branch clone is performed, capped at 1 GB. Private repos, SSH, and branch selection are not yet supported.",
    detail: "Validated before any model call is made.",
    tone: "neutral",
  },
  {
    id: "02",
    phase: "INPUT",
    title: "Describe the issue",
    body: "Write one clear task: a bug to fix, a feature to add, or an area to scan. In Discovery mode, leave this blank—SecondEgo maps the repository for you.",
    detail: "Issue text becomes the primary task context slot.",
    tone: "neutral",
  },
  {
    id: "03",
    phase: "UNDERSTAND",
    title: "Validate the repository",
    body: "SecondEgo checks for a Git root, an initial commit, and a clean working tree. Dirty repositories are rejected—merging an agent diff with uncommitted changes would be unsafe.",
    detail: "No model call has happened yet.",
    tone: "neutral",
  },
  {
    id: "04",
    phase: "EXPLORE",
    title: "Index files, symbols & tests",
    body: "A bounded scan walks the repository: files, language detection, symbol extraction (Tree-sitter for Python, JS, TS, TSX, Rust), imports, test symbols, and source-to-test links. Generated and vendor directories are ignored.",
    detail: "Index is local and rebuildable—no cloud required.",
    tone: "active",
  },
  {
    id: "05",
    phase: "EXPLORE",
    title: "Retrieve ranked evidence",
    body: "Task terms score against symbols, paths, and dependency relationships. Only the highest-ranked, source-linked excerpts enter the context packet. Omitted evidence is recorded. The full repository is never sent to the model.",
    detail: "Context slots: task · evidence · state · response reserve.",
    tone: "active",
  },
  {
    id: "06",
    phase: "PLAN",
    title: "Model produces a structured plan",
    body: "The ranked evidence packet is sent to the configured provider (DeepSeek default, Groq, or Gemini). The model returns a typed ActionProposal—a list of structured actions with known types and arguments.",
    detail: "The model proposes. The runtime has not yet touched any file.",
    tone: "model",
  },
  {
    id: "07",
    phase: "PLAN",
    title: "Schema + policy validation",
    body: "The proposal is parsed against a strict schema. Every action is checked for type, path, and argument validity. Invalid or malformed plans are rejected before any execution begins.",
    detail: "If the model hallucinates a file, replace_text fails safely here.",
    tone: "model",
  },
  {
    id: "08",
    phase: "EXECUTE",
    title: "Detached Git worktree created",
    body: "A fresh worktree is branched from HEAD. All subsequent file writes, patches, and command runs happen inside this isolated workspace. The original working tree is untouched.",
    detail: "Failed attempts are discarded here—never in the target repo.",
    tone: "execute",
  },
  {
    id: "09",
    phase: "EXECUTE",
    title: "Actions dispatched through bounded router",
    body: "Each action is routed through policy checks: path canonicalization, workspace boundary enforcement, command allowlist (git, npm, pytest, python, ruff, uv), shell=false, 120 s timeout cap, and bounded output capture.",
    detail: "No shell expansion. No arbitrary command execution.",
    tone: "execute",
  },
  {
    id: "10",
    phase: "VERIFY",
    title: "Verification commands run",
    body: "The declared test, build, or lint commands run sequentially in the worktree. Exit codes and bounded command output are recorded. A passing exit code is required—model narration cannot substitute.",
    detail: "Completion = passing evidence + transferable diff. Not one without the other.",
    tone: "verify",
  },
  {
    id: "11",
    phase: "DIAGNOSE → RECOVER",
    title: "On failure: classify, repair, retry",
    body: "A failure is classified (test · build · lint · type · environment · tool · planning). The engine enters DIAGNOSE, assembles failure-specific evidence, requests one structured repair plan, and reruns verification in the same worktree. Up to 3 attempts.",
    detail: "Recovery stops at the attempt, model-call, or time budget—whichever is first.",
    tone: "recover",
  },
  {
    id: "12",
    phase: "TERMINAL",
    title: "Verified diff transferred — or worktree discarded",
    body: "On success: the verified patch and eligible new files are transferred to the original repository. On failure: the worktree is removed, the target repository is unchanged, and the run terminates with FAILED or BLOCKED.",
    detail: "Every run ends with COMPLETE · FAILED · BLOCKED · CANCELLED + evidence report.",
    tone: "terminal",
  },
];

const toneColors: Record<string, string> = {
  neutral: "#aaa9a0",
  active: "#ff8970",
  model: "#dbc1b5",
  execute: "#9bcea9",
  verify: "#7cc191",
  recover: "#dfb877",
  terminal: "#f26545",
};

export function UserFlow() {
  const [activeStep, setActiveStep] = useState(0);
  const step = steps[activeStep];

  return (
    <section className="userflow-section section-grid" id="user-flow" data-tone="paper">
      <div className="wrap">
        <Reveal className="section-heading section-heading--split">
          <div>
            <p className="section-label">HOW ONE RUN WORKS</p>
            <h2>12 steps from<br /><span className="text-accent">issue to verified diff.</span></h2>
          </div>
          <p className="section-heading__aside">
            Walk through every decision the engine makes. Each step is observable, bounded, and evidence-backed—
            the model is only involved at steps 6–7 and recovery step 11.
          </p>
        </Reveal>

        <div className="userflow-layout">
          {/* Step rail */}
          <ol className="userflow-rail" aria-label="Run steps">
            {steps.map((s, index) => (
              <li key={s.id}>
                <button
                  type="button"
                  className={`userflow-step-btn${activeStep === index ? " is-active" : ""} tone-${s.tone}`}
                  onClick={() => setActiveStep(index)}
                  aria-current={activeStep === index}
                >
                  <span className="userflow-step-num">{s.id}</span>
                  <span className="userflow-step-label">
                    <b>{s.title}</b>
                    <small className="mono">{s.phase}</small>
                  </span>
                  {index < steps.length - 1 && (
                    <span className="userflow-connector" aria-hidden="true" />
                  )}
                </button>
              </li>
            ))}
          </ol>

          {/* Detail panel */}
          <div className="userflow-panel" key={activeStep}>
            <div className="userflow-panel__badge" style={{ "--tone-color": toneColors[step.tone] } as React.CSSProperties}>
              <span className="mono">STEP / {step.id}</span>
              <span className="userflow-phase-tag">{step.phase}</span>
            </div>
            <h3 className="userflow-panel__title">{step.title}</h3>
            <p className="userflow-panel__body">{step.body}</p>
            <div className="userflow-panel__detail">
              <span className="userflow-detail-marker" aria-hidden="true">↳</span>
              <p>{step.detail}</p>
            </div>

            {/* Progress bar */}
            <div className="userflow-progress" role="progressbar" aria-valuenow={activeStep + 1} aria-valuemin={1} aria-valuemax={steps.length}>
              <div className="userflow-progress__fill" style={{ width: `${((activeStep + 1) / steps.length) * 100}%`, background: toneColors[step.tone] }} />
            </div>
            <div className="userflow-nav-btns">
              <button
                type="button"
                className="userflow-nav-btn"
                disabled={activeStep === 0}
                onClick={() => setActiveStep((p) => Math.max(0, p - 1))}
              >
                ← Previous
              </button>
              <span className="mono userflow-counter">{activeStep + 1} / {steps.length}</span>
              <button
                type="button"
                className="userflow-nav-btn"
                disabled={activeStep === steps.length - 1}
                onClick={() => setActiveStep((p) => Math.min(steps.length - 1, p + 1))}
              >
                Next →
              </button>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
