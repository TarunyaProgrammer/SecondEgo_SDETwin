"use client";

import { useEffect, useState, useCallback } from "react";

// Each slide maps to a section id on the page
// Speaker notes = what you say out loud. Key points = bullet reminders.
export const SLIDES: {
  id: string;
  sectionId: string;
  title: string;
  subtitle: string;
  speakerNotes: string;
  keyPoints: string[];
  tone: "paper" | "ink";
}[] = [
  {
    id: "slide-hero",
    sectionId: "top",
    title: "SecondEgo",
    subtitle: "A local autonomous coding-agent harness",
    tone: "paper",
    speakerNotes:
      "SecondEgo is not a chat interface that can run shell commands. It is a control system that sits between a language model and your repository. The model reasons; the Rust engine acts, verifies, and decides the outcome.",
    keyPoints: [
      "Rust engine is the source of truth — not the model, not the UI",
      "Changes only reach your repo after verification passes",
      "Headless by default — GUI is optional, observability only",
      "MIT licensed, runs fully local, no cloud dependency",
    ],
  },
  {
    id: "slide-problem",
    sectionId: "problem",
    title: "The 5 problems LLMs have without a harness",
    subtitle: "Stale context · unchecked edits · self-reported success · silent failure · unbounded loops",
    tone: "ink",
    speakerNotes:
      "Foundation models suggest code — they cannot verify it. Without a harness, you get stale context (model guesses), unchecked edits (dirty repos), self-reported success (narration ≠ evidence), silent failure (no error classification), and unbounded retry loops.",
    keyPoints: [
      "Stale context → AST-aware index + ranked retrieval solves this",
      "Unchecked edits → detached Git worktree + transactional execution",
      "Self-reported success → completion requires real exit codes + diff",
      "Silent failure → every failure is classified (test/build/lint/env...)",
      "Unbounded loops → 3 recovery attempts + separate budget limits",
    ],
  },
  {
    id: "slide-arch",
    sectionId: "architecture",
    title: "Architecture",
    subtitle: "User → Gateway → Rust Engine → Detached Git Worktree",
    tone: "ink",
    speakerNotes:
      "The architecture is deliberately layered. The desktop is an observer — it reads engine events. The gateway is a loopback-only boundary. The Rust engine owns everything: state machine, repository index, tool routing, verification, and SQLite storage. The model is just one component behind a provider interface.",
    keyPoints: [
      "Desktop observer: reads events — cannot plan or call tools",
      "Gateway: 127.0.0.1 only, random auth token, request-size limits",
      "Engine owns: state machine, repo index, tool policy, verification",
      "Model is behind a provider interface — swappable (DeepSeek/Groq/Gemini)",
      "Worktree = clean Git branch per attempt — target repo untouched until pass",
    ],
  },
  {
    id: "slide-flow",
    sectionId: "user-flow",
    title: "A run is 12 deterministic steps",
    subtitle: "The model is only involved at steps 6–7 and recovery step 11",
    tone: "paper",
    speakerNotes:
      "Walk through the 12 steps on screen. Key insight for judges: steps 1–5 and 8–10 and 12 are entirely deterministic — no model involved. The model only sees a bounded evidence packet at planning time, and optionally at recovery.",
    keyPoints: [
      "Steps 1–5: validation, indexing, retrieval — no model call yet",
      "Step 6–7: model gets ranked evidence → returns structured ActionProposal",
      "Step 8–9: detached worktree, bounded tool router, shell=false",
      "Step 10: verification runs — exit codes, bounded output, diff required",
      "Step 11: failure → classify → repair plan → fresh verify (max 3×)",
      "Step 12: pass = diff transferred. fail = worktree discarded silently",
    ],
  },
  {
    id: "slide-lifecycle",
    sectionId: "lifecycle",
    title: "State machine: every transition is explicit",
    subtitle: "INIT → UNDERSTAND → EXPLORE → PLAN → EXECUTE → VERIFY → (DIAGNOSE → RECOVER)",
    tone: "ink",
    speakerNotes:
      "Unlike a ReAct loop, you can answer: what is the agent doing? which actions are legal? why did it retry? The state machine makes every transition auditable. Terminal statuses are COMPLETE, FAILED, BLOCKED, CANCELLED — not ambiguous.",
    keyPoints: [
      "Every phase has legal transitions — no freeform loops",
      "BLOCKED = deterministic precondition failure (vs FAILED = runtime error)",
      "Recovery = bounded: attempts + model calls + tool calls + time budget",
      "Cancel is checked at phase boundaries — no mid-subprocess kill",
      "94 Rust tests pass — state machine has unit + integration coverage",
    ],
  },
  {
    id: "slide-capabilities",
    sectionId: "capabilities",
    title: "Three engineering boundaries",
    subtitle: "Repository intelligence · Transactional execution · Bounded recovery",
    tone: "paper",
    speakerNotes:
      "These three boundaries are what make SecondEgo more than a wrapper. Repository intelligence: rank and budget context before asking the model. Transactional execution: detached worktree, failed = discard, passed = transfer. Bounded recovery: classify failure, one repair plan, fresh verify.",
    keyPoints: [
      "Boundary 1: ranked retrieval → model sees focused evidence, not the whole repo",
      "Boundary 2: worktree isolation → target repo only touched after verification",
      "Boundary 3: recovery is one bounded attempt — not unlimited retries",
      "Tool router: 8 allowlisted executables, shell=false, 120s cap",
      "Context slots: task · evidence · state · response reserve — all budgeted",
    ],
  },
  {
    id: "slide-map",
    sectionId: "judges-map",
    title: "Read the code — every claim is source-backed",
    subtitle: "6 architecture layers · trace the control flow · feature ledger · tech inventory",
    tone: "paper",
    speakerNotes:
      "This section is for judges who want to verify. Click any layer to see the source path. Click any feature in the ledger to see its evidence. The feature ledger distinguishes: implemented, optional, and not yet. We are honest about what is not yet done.",
    keyPoints: [
      "Layer 1: Observer shell (Electron/React) — shows events, owns nothing",
      "Layer 3: Rust runtime — click to see secondego-runtime crate",
      "Feature ledger: green = implemented, amber = optional, grey = not yet",
      "'Not yet' items: installer, broad language support, full benchmark suite",
      "Judge commands: make setup · make discover · make judge · make rust-test",
    ],
  },
  {
    id: "slide-proof",
    sectionId: "proof",
    title: "Don't trust the story. Trace the run.",
    subtitle: "Every claim maps to a state transition, allowlist, fixture, test, or termination rule",
    tone: "ink",
    speakerNotes:
      "The ProofConsole shows a real run trace. Point to the event sequence. Every entry corresponds to an actual engine event — not a narration. The run IDs, state transitions, and verification results are SQLite-backed.",
    keyPoints: [
      "94 Rust tests pass (unit + integration + fixture)",
      "Pagination fixture: off-by-one bug, recovery path, verified diff",
      "Python suite: 83 pass / 5 fail (gesture socket — env permission, not logic)",
      "make rust-test runs all 94 in under 60s from clean checkout",
      "Desktop production build: clean TSC + Vite",
    ],
  },
  {
    id: "slide-honest",
    sectionId: "honest-boundaries",
    title: "What we do and don't claim",
    subtitle: "Security · Model reliability · Verification authority · Scope limits",
    tone: "paper",
    speakerNotes:
      "We lead with honesty here because judges respect it. Use the filter to show security boundaries. Confirmed (green) = fully implemented. Amber = real gap with a documented next step. We don't call it a sandbox — it is workspace-bounded, not OS-level.",
    keyPoints: [
      "✓ Filesystem escape prevented: canonical paths, sensitive filename blocklist",
      "✓ Original repo protected: Git + clean worktree required, worktree discarded on fail",
      "✓ Electron renderer unprivileged: nodeIntegration: false, contextIsolation: true",
      "⚠ Not an OS sandbox: allowed interpreters can access absolute paths",
      "⚠ Subprocess environment: provider keys inherited — fix = minimal env",
      "⚠ Test tampering: prompt-based today, diff-level enforcement is P0 next step",
    ],
  },
  {
    id: "slide-qa",
    sectionId: "judge-qa",
    title: "Every hard question. Honest answers.",
    subtitle: "Architecture · Execution · Rate limits · Security · Difficult judge questions",
    tone: "ink",
    speakerNotes:
      "This section is the full Q&A bank. 30+ questions with written answers. During the presentation, flip to 'Difficult judge questions' to pre-empt the hardest ones. The best defensive answers are already here — you don't need to improvise.",
    keyPoints: [
      "Q: How are you different from an IDE + LLM? → The harness, not the visual shell",
      "Q: Can the model game verification? → Real gap, documented fix: diff-level test protection",
      "Q: What stops python -c reading /home? → Not enough today — needs OS sandbox",
      "Q: Why trust automatic recovery? → Bounded, evidence-based, verified again before applying",
      "Q: Biggest technical debt? → Subprocess isolation, test integrity enforcement, benchmarks",
      "Q: Biggest design success? → Verified-diff boundary: failed attempts are disposable",
    ],
  },
  {
    id: "slide-reference",
    sectionId: "reference-run",
    title: "Reference run: failure is a state, not a story",
    subtitle: "Pagination bug · plan fails verification · repair plan · verified diff transferred",
    tone: "ink",
    speakerNotes:
      "Walk through the pagination fixture. This is a separate target repository with a deliberate off-by-one bug. The first verification fails. The engine classifies the failure, requests a repair plan, and the second attempt passes verification. The diff is transferred.",
    keyPoints: [
      "Target: separate fixture repo — not the SecondEgo repo itself",
      "Bug: off-by-one in pagination logic (deliberate)",
      "First attempt: verification fails (test fails) → worktree discarded",
      "Recovery: failure evidence → repair plan → fresh worktree → verify",
      "Terminal: COMPLETE with passing verification + transferred diff",
      "Reproducible: make judge runs this from clean checkout",
    ],
  },
  {
    id: "slide-closing",
    sectionId: "top",
    title: "30-second closing answer",
    subtitle: "What SecondEgo is, why it matters, what comes next",
    tone: "paper",
    speakerNotes:
      "SecondEgo turns a text model into a bounded software-engineering system. It indexes the repository, assembles focused context, validates a structured plan, executes changes in an isolated Git worktree, verifies them using real commands, and transfers only a verified diff. Failures enter an evidence-based, bounded recovery loop, and every run terminates with explicit status and evidence. The desktop village makes that execution observable, but the Rust engine remains the sole source of truth. Our next hardening priorities are OS-level subprocess isolation, deterministic protection of existing tests, and broader benchmark coverage.",
    keyPoints: [
      "Lead with: 'The model proposes. The engine disposes.'",
      "The harness is the product — not the visual shell",
      "Verified-diff boundary is the strongest reliability property",
      "Next priorities: OS sandbox · test integrity · broader benchmarks",
      "94 Rust tests pass. Production build is clean. Fixture is replayable.",
    ],
  },
];

type PresentationModeProps = {
  isActive: boolean;
  onExit: () => void;
};

export function PresentationModeOverlay({ isActive, onExit }: PresentationModeProps) {
  const [currentIndex, setCurrentIndex] = useState(0);
  const [showNotes, setShowNotes] = useState(true);
  const slide = SLIDES[currentIndex];

  const scrollToSection = useCallback((sectionId: string) => {
    const el = document.getElementById(sectionId);
    if (el) el.scrollIntoView({ behavior: "smooth" });
  }, []);

  const goTo = useCallback(
    (index: number) => {
      const next = Math.max(0, Math.min(SLIDES.length - 1, index));
      setCurrentIndex(next);
      scrollToSection(SLIDES[next].sectionId);
    },
    [scrollToSection],
  );

  useEffect(() => {
    if (!isActive) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") { onExit(); return; }
      if (e.key === "ArrowRight" || e.key === "ArrowDown" || e.key === " ") {
        e.preventDefault();
        goTo(currentIndex + 1);
      }
      if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
        e.preventDefault();
        goTo(currentIndex - 1);
      }
      if (e.key === "n" || e.key === "N") setShowNotes((v) => !v);
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [isActive, currentIndex, goTo, onExit]);

  // When entering, scroll to current slide section
  useEffect(() => {
    if (isActive) scrollToSection(SLIDES[currentIndex].sectionId);
  }, [isActive, currentIndex, scrollToSection]);

  if (!isActive) return null;

  return (
    <div className="pmode-overlay" role="dialog" aria-label="Presentation mode" aria-modal="true">
      {/* Top bar */}
      <div className="pmode-topbar">
        <div className="pmode-brand mono">
          <span className="pmode-brand-dot" />
          secondego / present
        </div>
        <div className="pmode-slide-counter mono">
          {currentIndex + 1} <span>/</span> {SLIDES.length}
        </div>
        <div className="pmode-topbar-actions">
          <button
            className={`pmode-notes-toggle${showNotes ? " is-active" : ""}`}
            type="button"
            onClick={() => setShowNotes((v) => !v)}
            title="Toggle speaker notes (N)"
          >
            notes
          </button>
          <button className="pmode-exit-btn" type="button" onClick={onExit} title="Exit (ESC)">
            ✕ exit
          </button>
        </div>
      </div>

      {/* Slide dot rail */}
      <div className="pmode-dot-rail" role="tablist" aria-label="Slides">
        {SLIDES.map((s, i) => (
          <button
            key={s.id}
            type="button"
            role="tab"
            aria-selected={i === currentIndex}
            className={`pmode-dot${i === currentIndex ? " is-active" : ""}${i < currentIndex ? " is-done" : ""}`}
            title={s.title}
            onClick={() => goTo(i)}
          />
        ))}
      </div>

      {/* Main content panel */}
      <div className={`pmode-panel${showNotes ? " has-notes" : ""}`}>
        {/* Slide info */}
        <div className={`pmode-slide pmode-slide--${slide.tone}`}>
          <div className="pmode-slide-meta">
            <span className="pmode-slide-num mono">{String(currentIndex + 1).padStart(2, "0")} / {String(SLIDES.length).padStart(2, "0")}</span>
            <span className="pmode-slide-divider" aria-hidden="true" />
            <span className="pmode-slide-section mono">{slide.sectionId.replace(/-/g, " ").toUpperCase()}</span>
          </div>
          <h2 className="pmode-slide-title">{slide.title}</h2>
          <p className="pmode-slide-subtitle">{slide.subtitle}</p>

          <div className="pmode-key-points">
            <p className="pmode-kp-heading mono">KEY POINTS</p>
            <ul>
              {slide.keyPoints.map((kp, i) => (
                <li key={i}>{kp}</li>
              ))}
            </ul>
          </div>

          {/* Navigation */}
          <div className="pmode-slide-nav">
            <button
              className="pmode-nav-btn"
              type="button"
              disabled={currentIndex === 0}
              onClick={() => goTo(currentIndex - 1)}
            >
              ← Prev
            </button>
            <span className="pmode-kbd-hint mono">← → Space · N=notes · ESC=exit</span>
            <button
              className="pmode-nav-btn pmode-nav-btn--primary"
              type="button"
              disabled={currentIndex === SLIDES.length - 1}
              onClick={() => goTo(currentIndex + 1)}
            >
              Next →
            </button>
          </div>
        </div>

        {/* Speaker notes */}
        {showNotes && (
          <div className="pmode-notes">
            <p className="pmode-notes-label mono">SPEAKER NOTES</p>
            <p className="pmode-notes-body">{slide.speakerNotes}</p>
            {currentIndex === SLIDES.length - 1 && (
              <div className="pmode-closing-quote">
                <p className="mono" style={{ fontSize: "9px", color: "rgba(243,242,238,.35)", marginBottom: "12px" }}>30-SECOND ANSWER</p>
                <blockquote>
                  "SecondEgo turns a text model into a bounded software-engineering system. It indexes
                  the repository, assembles focused context, validates a structured plan, executes changes
                  in an isolated Git worktree, verifies them using real commands, and transfers only a
                  verified diff. Failures enter an evidence-based, bounded recovery loop, and every run
                  terminates with explicit status and evidence. The desktop village makes that execution
                  observable, but the Rust engine remains the sole source of truth. Our next hardening
                  priorities are OS-level subprocess isolation, deterministic protection of existing tests,
                  and broader benchmark coverage."
                </blockquote>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

// Floating trigger button
type PresentButtonProps = { onClick: () => void };
export function PresentButton({ onClick }: PresentButtonProps) {
  return (
    <button className="pmode-trigger" type="button" onClick={onClick} aria-label="Enter presentation mode">
      <span className="pmode-trigger-icon" aria-hidden="true">▶</span>
      <span>Present</span>
    </button>
  );
}
