"use client";

import { useState } from "react";
import { Reveal } from "@/components/Reveal";

type Category = "security" | "model" | "verification" | "limits";

type Boundary = {
  id: string;
  category: Category;
  claim: string;
  honest: string;
  nextStep?: string;
  positive?: boolean;
};

const boundaries: Boundary[] = [
  // Security
  {
    id: "sb1",
    category: "security",
    claim: "Filesystem escape is prevented",
    honest: "Paths are canonicalized and required to stay under the selected workspace. Sensitive filenames (.env, credential files, private-key patterns) are blocked from model file access. Symlink resolution is checked where the path exists.",
    positive: true,
  },
  {
    id: "sb2",
    category: "security",
    claim: "The original repository is protected",
    honest: "Mutation requires a Git root, an initial commit, and a clean working tree. Execution runs in a detached worktree. Only a successfully verified patch is transferred. Dirty repositories are rejected.",
    positive: true,
  },
  {
    id: "sb3",
    category: "security",
    claim: "It is a secure sandbox",
    honest: "It is workspace- and policy-bounded, not an OS-level sandbox. Executable names are allowlisted and timeouts exist, but allowed interpreters like python can execute arbitrary code and subprocesses inherit the environment.",
    nextStep: "Production fix: minimal subprocess environment, OS-level sandboxing, and network isolation.",
  },
  {
    id: "sb4",
    category: "security",
    claim: "API keys cannot reach subprocesses",
    honest: "Model keys are not written into prompts, UI state, or SQLite. However, subprocesses currently inherit the engine's environment by default, so an adversarial verification command could inspect environment variables.",
    nextStep: "Fix: construct a minimal subprocess env and explicitly remove provider credential variables.",
  },
  {
    id: "sb5",
    category: "security",
    claim: "The Electron renderer is unprivileged",
    honest: "The renderer uses nodeIntegration: false, contextIsolation: true, Electron sandboxing, and a narrow preload bridge. The gateway binds only to 127.0.0.1 with a random per-process token.",
    positive: true,
  },
  {
    id: "sb6",
    category: "security",
    claim: "Tests cannot be weakened by the model",
    honest: "The planner prompt tells the model not to modify existing tests, and actions are constrained. But the transaction layer does not yet deterministically reject every diff that modifies an existing test or test configuration.",
    nextStep: "Fix: snapshot protected test paths, reject modifications to existing tests at diff level.",
  },

  // Model
  {
    id: "mb1",
    category: "model",
    claim: "Provider-neutral by design",
    honest: "The runtime depends on a ModelProvider contract. The current Rust implementation supports DeepSeek (evaluator default), Groq, Gemini, and deterministic scripted providers for tests. Provider HTTP formats stay in the model crate.",
    positive: true,
  },
  {
    id: "mb2",
    category: "model",
    claim: "Rate limits are handled gracefully",
    honest: "For a 429 response: reads Retry-After or provider token-reset headers, adds a reset grace period, enforces a minimum wait, refuses waits above the configurable maximum (120 s), and waits cooperatively so cancellation still works.",
    positive: true,
  },
  {
    id: "mb3",
    category: "model",
    claim: "Invalid JSON terminates safely",
    honest: "Provider output is parsed into a strict ActionProposal. Invalid or incomplete plans cannot begin execution. Bounded format-repair requests may be attempted; if they fail, the run terminates without opening a mutating transaction.",
    positive: true,
  },
  {
    id: "mb4",
    category: "model",
    claim: "It uses exact provider tokenizers",
    honest: "Not currently for preflight budgeting. The default estimate is approximately one token per four characters—deterministic and free, but conservative and imprecise. Provider-reported token counts are recorded afterward when available.",
    nextStep: "Future: integrate tiktoken or provider-specific estimators for tighter preflight budgets.",
  },

  // Verification
  {
    id: "vb1",
    category: "verification",
    claim: "Completion requires real evidence",
    honest: "Completion requires passing command output, changed paths, Git diff evidence, successful worktree transfer, and an explicit terminal state. A model narration cannot mark a run complete.",
    positive: true,
  },
  {
    id: "vb2",
    category: "verification",
    claim: "Verification commands are trusted",
    honest: "Currently the model proposes verification commands as part of the plan. The engine validates command shape and executable policy, but a model may choose an insufficient test. Baseline repository-discovered tests are not yet combined.",
    nextStep: "Fix: discover baseline commands from manifests and combine with model-proposed targeted tests.",
  },
  {
    id: "vb3",
    category: "verification",
    claim: "Test counts are accurately reported",
    honest: "Command-level success (exit codes) is reliable. The verification structure contains passed/failed counts, but the generic verifier does not reliably parse per-framework counts; successful runs can show zero passed tests.",
    nextStep: "Fix: add parsers for pytest, Cargo, npm/Vitest/Jest while preserving raw bounded evidence.",
  },
  {
    id: "vb4",
    category: "verification",
    claim: "Failure classification is perfect",
    honest: "The verifier recognizes test, build, lint, type, runtime, tool, environment, model-planning, regression, and unknown classes. Classification is largely output-pattern based—useful, but not perfect semantic analysis.",
    positive: false,
  },

  // Limits
  {
    id: "lb1",
    category: "limits",
    claim: "Supports all programming languages",
    honest: "Tree-sitter grammars are implemented for Python, JavaScript, TypeScript, TSX, and Rust. Other files may participate through lexical fallback, but full semantic support is not claimed for other languages.",
    positive: false,
  },
  {
    id: "lb2",
    category: "limits",
    claim: "Handles large repositories",
    honest: "Current controls: ignore generated/vendor dirs, ranked retrieval, excerpt limits, file-size limits, context budgets, and a 1 GB cap on remote clones. The index rebuilds per run; incremental caching is future work.",
    positive: false,
  },
  {
    id: "lb3",
    category: "limits",
    claim: "Supports remote private repositories",
    honest: "SecondEgo accepts public HTTPS GitHub URLs with shallow single-branch cloning. Private repositories, SSH URLs, branch selection, push, and pull-request creation are not supported.",
    positive: false,
  },
  {
    id: "lb4",
    category: "limits",
    claim: "SQLite is a full replay system",
    honest: "SQLite stores final run state and ordered versioned events. It does not store safe tool inputs, repository revision, model identity, or configuration snapshots needed for full replay.",
    nextStep: "Future: record safe config, repository revision, model identity, and schema version migrations.",
  },
];

const categoryLabels: Record<Category, string> = {
  security: "Security",
  model: "Model & Rate Limits",
  verification: "Verification",
  limits: "Scope & Limits",
};

export function HonestBoundaries() {
  const [activeCategory, setActiveCategory] = useState<Category | "all">("all");

  const categories = (["all", "security", "model", "verification", "limits"] as const);
  const filtered = activeCategory === "all" ? boundaries : boundaries.filter((b) => b.category === activeCategory);

  return (
    <section className="honest-section section-grid" id="honest-boundaries" data-tone="paper">
      <div className="wrap">
        <Reveal className="section-heading section-heading--split">
          <div>
            <p className="section-label">WHAT WE DO AND DON'T CLAIM</p>
            <h2>Honest about<br /><span className="text-accent">every boundary.</span></h2>
          </div>
          <p className="section-heading__aside">
            Security, model reliability, verification authority, and scope limits—all documented
            before the demo. Click through each boundary to see what's implemented, what's honest,
            and what the next hardening step is.
          </p>
        </Reveal>

        <div className="honest-filter-bar">
          {categories.map((cat) => (
            <button
              key={cat}
              type="button"
              className={`honest-filter-pill${activeCategory === cat ? " is-active" : ""}`}
              onClick={() => setActiveCategory(cat)}
            >
              {cat === "all" ? "All boundaries" : categoryLabels[cat]}
            </button>
          ))}
        </div>

        <div className="honest-grid">
          {filtered.map((boundary, index) => (
            <Reveal className={`honest-card${boundary.positive === true ? " is-confirmed" : boundary.nextStep ? " is-gap" : " is-noted"}`} key={boundary.id} delay={index * 0.04}>
              <div className="honest-card__head">
                <span className={`honest-status-dot${boundary.positive === true ? " is-green" : boundary.nextStep ? " is-amber" : " is-noted"}`} />
                <span className="honest-card__cat mono">{categoryLabels[boundary.category]}</span>
              </div>
              <h3 className="honest-card__claim">{boundary.claim}</h3>
              <p className="honest-card__body">{boundary.honest}</p>
              {boundary.nextStep && (
                <div className="honest-card__next">
                  <span className="mono">NEXT STEP</span>
                  <p>{boundary.nextStep}</p>
                </div>
              )}
            </Reveal>
          ))}
        </div>
      </div>
    </section>
  );
}
