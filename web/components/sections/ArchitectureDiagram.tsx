"use client";

import { Reveal } from "@/components/Reveal";

const layers = [
  {
    id: "user",
    label: "You",
    sub: "Terminal or Desktop",
    color: "var(--signal)",
    icon: "◎",
  },
  {
    id: "gateway",
    label: "Rust Gateway",
    sub: "127.0.0.1 · auth token · loopback only",
    color: "#dbc1b5",
    icon: "⬡",
  },
  {
    id: "engine",
    label: "Rust Engine",
    sub: "Source of truth",
    color: "#9bcea9",
    icon: "⬛",
    internals: [
      { label: "State machine", detail: "INIT → UNDERSTAND → EXPLORE → PLAN → EXECUTE → VERIFY → DIAGNOSE → RECOVER → TERMINAL" },
      { label: "Repository index", detail: "Tree-sitter · Python AST · ranked retrieval · context budgets" },
      { label: "Provider interface", detail: "DeepSeek · Groq · Gemini · schema-validated proposals" },
      { label: "Bounded tool router", detail: "allowlist · shell=false · 120s cap · workspace-confined paths" },
      { label: "Verification", detail: "exit codes · bounded output · diff evidence · explicit terminal status" },
      { label: "SQLite storage", detail: "run state · versioned events · evidence records" },
    ],
  },
  {
    id: "worktree",
    label: "Detached Git Worktree",
    sub: "Isolated attempt — target repo untouched",
    color: "#7cc191",
    icon: "⬡",
  },
];

const outcomes = [
  { label: "PASS", detail: "Verified diff transferred to target repo", color: "#7cc191" },
  { label: "FAIL", detail: "Worktree discarded — target unchanged", color: "#f26545" },
];

export function ArchitectureDiagram() {
  return (
    <section className="arch-section section-grid" id="architecture" data-tone="ink">
      <div className="wrap">
        <Reveal className="section-heading section-heading--split">
          <div>
            <p className="section-label">SYSTEM ARCHITECTURE</p>
            <h2>One engine.<br /><span className="text-accent">Every boundary explicit.</span></h2>
          </div>
          <p className="section-heading__aside">
            The model is one component behind a provider interface. It cannot directly call tools,
            mutate files, or declare success. The Rust engine owns every boundary that matters.
          </p>
        </Reveal>

        <Reveal className="arch-diagram" delay={0.08}>
          {/* Top: User inputs */}
          <div className="arch-inputs">
            <div className="arch-input-box">
              <span className="arch-badge arch-badge--signal">USER INPUT</span>
              <div className="arch-input-pair">
                <div className="arch-input-item">
                  <span className="arch-input-icon">⬛</span>
                  <div>
                    <strong>Headless CLI / TUI</strong>
                    <small>Default evaluation path</small>
                  </div>
                </div>
                <div className="arch-input-divider">or</div>
                <div className="arch-input-item">
                  <span className="arch-input-icon">▣</span>
                  <div>
                    <strong>Electron + React Desktop</strong>
                    <small>Observer only · optional</small>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div className="arch-arrow arch-arrow--down">
            <span className="arch-arrow-label mono">authenticated loopback HTTP · 127.0.0.1</span>
            <span className="arch-arrow-line" aria-hidden="true">↓</span>
          </div>

          {/* Gateway */}
          <div className="arch-gateway">
            <span className="arch-badge arch-badge--muted">BOUNDARY LAYER</span>
            <div className="arch-gateway-inner">
              <span className="arch-gw-icon">⬡</span>
              <div>
                <strong>Rust Gateway</strong>
                <small>Random per-process token · loopback only · request-size limits · no model access</small>
              </div>
            </div>
          </div>

          <div className="arch-arrow arch-arrow--down">
            <span className="arch-arrow-line" aria-hidden="true">↓</span>
          </div>

          {/* Engine box */}
          <div className="arch-engine">
            <div className="arch-engine-header">
              <span className="arch-badge arch-badge--green">RUST ENGINE — SOURCE OF TRUTH</span>
            </div>
            <div className="arch-engine-internals">
              <div className="arch-state-machine">
                <p className="mono arch-sm-label">STATE MACHINE</p>
                <div className="arch-sm-flow">
                  {["INIT", "UNDERSTAND", "EXPLORE", "PLAN", "EXECUTE", "VERIFY"].map((s, i) => (
                    <span key={s} className="arch-sm-state">
                      {s}
                      {i < 5 && <span className="arch-sm-arrow">→</span>}
                    </span>
                  ))}
                </div>
                <div className="arch-sm-recovery">
                  <span className="arch-sm-recover-path mono">on failure: VERIFY → DIAGNOSE → RECOVER → EXECUTE → VERIFY</span>
                </div>
              </div>
              <div className="arch-engine-grid">
                <div className="arch-engine-cell">
                  <span className="arch-cell-dot" style={{ background: "#ff8970" }} />
                  <div>
                    <strong>Repository index</strong>
                    <small>Tree-sitter · AST · ranked retrieval · context budgets</small>
                  </div>
                </div>
                <div className="arch-engine-cell">
                  <span className="arch-cell-dot" style={{ background: "#dbc1b5" }} />
                  <div>
                    <strong>Provider interface</strong>
                    <small>DeepSeek · Groq · Gemini · schema-validated proposals</small>
                  </div>
                </div>
                <div className="arch-engine-cell">
                  <span className="arch-cell-dot" style={{ background: "#9bcea9" }} />
                  <div>
                    <strong>Bounded tool router</strong>
                    <small>allowlist · shell=false · 120s cap · workspace paths only</small>
                  </div>
                </div>
                <div className="arch-engine-cell">
                  <span className="arch-cell-dot" style={{ background: "#7cc191" }} />
                  <div>
                    <strong>Verification + SQLite</strong>
                    <small>exit codes · diff evidence · run state · versioned events</small>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div className="arch-arrow arch-arrow--down">
            <span className="arch-arrow-line" aria-hidden="true">↓</span>
          </div>

          {/* Worktree */}
          <div className="arch-worktree">
            <span className="arch-badge arch-badge--teal">ISOLATED EXECUTION</span>
            <div className="arch-worktree-inner">
              <span className="arch-wt-icon">⬡</span>
              <div>
                <strong>Detached Git Worktree</strong>
                <small>Target repository working tree is untouched until a verified diff is transferred</small>
              </div>
            </div>
          </div>

          {/* Outcomes */}
          <div className="arch-outcomes">
            <div className="arch-outcome arch-outcome--pass">
              <span className="arch-outcome-icon">✓</span>
              <div>
                <strong>Verification passes</strong>
                <small>Verified diff transferred to target repository</small>
              </div>
              <span className="arch-outcome-status mono">COMPLETE</span>
            </div>
            <div className="arch-outcome-or" aria-hidden="true">or</div>
            <div className="arch-outcome arch-outcome--fail">
              <span className="arch-outcome-icon">✕</span>
              <div>
                <strong>Verification fails (after recovery)</strong>
                <small>Worktree discarded · target repository unchanged</small>
              </div>
              <span className="arch-outcome-status mono">FAILED</span>
            </div>
          </div>
        </Reveal>

        {/* Key principle callout */}
        <Reveal className="arch-principle" delay={0.15}>
          <div className="arch-principle__inner">
            <span className="arch-principle__number">↯</span>
            <div>
              <p className="mono" style={{ color: "rgba(243,242,238,.4)", fontSize: "9px", letterSpacing: ".12em", marginBottom: "10px" }}>THE CORE CONSTRAINT</p>
              <blockquote>
                The model supplies reasoning.<br />
                <strong>The engine supplies authority.</strong>
              </blockquote>
              <p>
                The model cannot call tools, declare success, write files, or modify the target repository.
                It sends a structured proposal. The Rust engine validates, executes in isolation, verifies with
                real commands, and decides the terminal outcome.
              </p>
            </div>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
