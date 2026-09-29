"use client";

import { useEffect, useState } from "react";
import { useReducedMotion } from "framer-motion";

type ProofStep = {
  phase: string;
  label: string;
  detail: string;
  lines: string[];
};

const steps: ProofStep[] = [
  {
    phase: "01",
    label: "Understand",
    detail: "Turn the issue into explicit acceptance criteria.",
    lines: ["issue        / pagination returns the wrong page", "criteria     / page 1 → first slice", "signal       / target fixture located"],
  },
  {
    phase: "02",
    label: "Explore",
    detail: "Retrieve source-linked evidence before a model call.",
    lines: ["index        / 14 files · 21 symbols", "retrieve     / src/pagination.py:1–18", "context      / bounded evidence packet"],
  },
  {
    phase: "03",
    label: "Execute",
    detail: "Apply the structured proposal inside a detached worktree.",
    lines: ["preflight    / clean Git baseline", "worktree     / .secondego/attempt-01", "router       / argv validated · shell false"],
  },
  {
    phase: "04",
    label: "Verify",
    detail: "Let command evidence decide whether the diff can leave the attempt.",
    lines: ["command      / pytest -q", "result       / 4 passed", "terminal     / COMPLETE · diff transferred"],
  },
];

export function ProofConsole() {
  const [active, setActive] = useState(0);
  const [running, setRunning] = useState(false);
  const reduceMotion = useReducedMotion();
  const current = steps[active];

  useEffect(() => {
    if (!running || reduceMotion) return;
    const timer = window.setInterval(() => {
      setActive((value) => {
        if (value === steps.length - 1) {
          setRunning(false);
          return value;
        }
        return value + 1;
      });
    }, 850);
    return () => window.clearInterval(timer);
  }, [reduceMotion, running]);

  function replay() {
    setActive(0);
    setRunning(true);
  }

  return (
    <div className="proof-console" aria-label="Interactive proof run">
      <div className="proof-console__top">
        <div className="console-dots" aria-hidden="true"><i /><i /><i /></div>
        <span className="mono">secondego / reference-run</span>
        <span className="proof-console__status"><i /> LOCAL</span>
      </div>
      <div className="proof-console__body">
        <div className="proof-console__rail" aria-label="Proof run phases">
          {steps.map((step, index) => (
            <button
              type="button"
              key={step.phase}
              className={`proof-console__phase${index === active ? " is-active" : ""}`}
              aria-label={`Show ${step.label} phase`}
              aria-pressed={index === active}
              onClick={() => { setActive(index); setRunning(false); }}
            >
              <span>{step.phase}</span><i aria-hidden="true" />
            </button>
          ))}
        </div>
        <div className="proof-console__content" aria-live="polite">
          <p className="proof-console__kicker mono">PHASE / {current.phase}</p>
          <h3>{current.label}<span>.</span></h3>
          <p className="proof-console__detail">{current.detail}</p>
          <div className="proof-console__lines">
            {current.lines.map((line) => <code key={line}>{line}</code>)}
          </div>
        </div>
      </div>
      <div className="proof-console__bottom">
        <span className="mono">EVIDENCE FIRST / NARRATION SECOND</span>
        <button type="button" className="console-replay" onClick={replay} disabled={running}>
          {running ? "Running…" : "Replay the run"}<span aria-hidden="true">↗</span>
        </button>
      </div>
    </div>
  );
}
