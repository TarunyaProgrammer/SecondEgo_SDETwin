"use client";

import { useState } from "react";
import { Reveal } from "@/components/Reveal";

type QAItem = {
  q: string;
  a: string;
  tag: string;
};

const qaGroups: { label: string; id: string; items: QAItem[] }[] = [
  {
    label: "Architecture",
    id: "arch",
    items: [
      {
        tag: "CORE",
        q: "What is SecondEgo?",
        a: "A local autonomous coding-agent harness. A text model supplies reasoning and structured change proposals, while the Rust engine supplies repository understanding, constrained tools, isolated execution, verification, recovery, and auditable termination. It is not merely an LLM chat interface that can run shell commands.",
      },
      {
        tag: "CORE",
        q: "What is the main architectural principle?",
        a: "The model proposes; the engine disposes. The model cannot directly declare success or mutate the production workspace. Every proposal passes through schema validation, policy enforcement, isolated execution, and verification.",
      },
      {
        tag: "ARCH",
        q: "Why did you choose Rust?",
        a: "Rust fits the final engine because the harness owns sensitive boundaries: filesystem operations, subprocesses, state transitions, cancellation, local networking, and resource limits. It gives stronger types, safer resource handling, and produces a self-contained evaluator binary. The main trade-off is slower iteration and more implementation complexity than Python.",
      },
      {
        tag: "ARCH",
        q: "Why does Python still exist?",
        a: "Python is the verified reference and compatibility implementation used during staged migration. Rust is now the default engine, but Python remains useful for behavioral comparison and rollback until full parity and packaging are complete. This is not two production engines—it is a controlled migration with a verified reference path.",
      },
      {
        tag: "ARCH",
        q: "Why is the desktop called an observer?",
        a: "Because the engine—not React—owns task state, model calls, tools, permissions, verification, recovery, and termination. The UI consumes versioned events and sends bounded commands such as start or cancel. This prevents presentation code from becoming a second orchestration system.",
      },
      {
        tag: "ARCH",
        q: "What is the pixel village for?",
        a: "It visualizes real engine activity in a more legible and memorable way. Each visible phase maps to an actual engine event. The village improves observability; it does not increase agent authority. The animated characters are not separate autonomous agents—there is no multi-agent runtime here.",
      },
    ],
  },
  {
    label: "Execution & Reliability",
    id: "exec",
    items: [
      {
        tag: "STATE MACHINE",
        q: "What does the state machine look like?",
        a: "The primary path: INITIALIZE → UNDERSTAND → EXPLORE → PLAN → EXECUTE → VERIFY. If verification fails: VERIFY → DIAGNOSE → RECOVER → EXECUTE → VERIFY. Terminal statuses are COMPLETE, FAILED, BLOCKED, and CANCELLED. Currently many runtime errors map to FAILED rather than BLOCKED—perfect semantic classification is a stated next step.",
      },
      {
        tag: "STATE MACHINE",
        q: "Why use an explicit state machine instead of a ReAct loop?",
        a: "An unconstrained loop makes it difficult to answer: what the agent is doing, which actions are legal, why it retried, whether verification happened, and why execution stopped. An explicit state machine gives legal transitions, bounded recovery, observable phase events, and testable termination behavior.",
      },
      {
        tag: "RECOVERY",
        q: "How many recovery attempts are allowed?",
        a: "The current default is three execution-recovery attempts, configurable through SECONDEGO_MAX_RECOVERY_ATTEMPTS. Separate budgets also cover model calls, tool calls, total retries, context size, and total runtime. Provider retries and code-recovery attempts are different failure classes with independent budgets.",
      },
      {
        tag: "RECOVERY",
        q: "What happens if the model returns invalid JSON?",
        a: "The provider output is parsed into a strict ActionProposal. Invalid or incomplete plans cannot begin execution. The runtime may perform bounded format-repair requests. If it still cannot obtain a valid plan, the run terminates without opening a mutating transaction.",
      },
      {
        tag: "RECOVERY",
        q: "What if tests pass but the patch is malicious?",
        a: "Passing tests alone do not prove safety. Current controls include constrained actions, bounded paths, diff evidence, and no automatic remote push. Stronger guarantees require semantic diff policies, secret scanning, dependency-policy checks, protected-path enforcement, and user approval for high-impact changes.",
      },
      {
        tag: "RECOVERY",
        q: "What if verification passes but no files changed?",
        a: "The engine does not mark the task complete. Completion requires both passing verification AND a transferable repository change. This prevents a weak or irrelevant test command from producing a false success with an empty diff.",
      },
    ],
  },
  {
    label: "Rate Limits & Providers",
    id: "rate",
    items: [
      {
        tag: "RATE LIMITS",
        q: "How do you solve rate limiting?",
        a: "For a 429 response: (1) read Retry-After or the provider's token-reset header, (2) add a small reset grace period, (3) enforce a minimum wait to avoid immediate retry bursts, (4) refuse waits above the configurable maximum (120 s), (5) wait cooperatively so cancellation still works, (6) retry only within both rate-limit and global retry budgets, (7) record safe aggregate usage and rate-limit metadata. The defaults allow up to six server-directed retries.",
      },
      {
        tag: "RATE LIMITS",
        q: "What happens when the provider gives no retry delay?",
        a: "SecondEgo does not guess and hammer the API. A rate-limit response without usable server timing is returned as a terminal provider failure. This is intentional—blind retries worsen a rate-limit situation.",
      },
      {
        tag: "PROVIDERS",
        q: "Are you locked to one model provider?",
        a: "No. The runtime depends on a ModelProvider contract. Current Rust implementation supports DeepSeek (evaluator default), Groq, Gemini, and deterministic scripted providers for tests. Provider-specific HTTP formats and usage headers stay in the model crate, not in orchestration code.",
      },
      {
        tag: "PROVIDERS",
        q: "Why use structured outputs?",
        a: "Structured output changes the model from an executor into a proposal generator. Plans must contain known actions and typed arguments, which the engine can reject before mutation. It reduces ambiguity but does not make model output trustworthy—semantic and policy validation are still required.",
      },
      {
        tag: "PROVIDERS",
        q: "How do you reduce the chance of hitting rate limits?",
        a: "Preventive controls: task-focused retrieval instead of sending the whole repository, limited initial evidence files, bounded excerpts, token-budgeted context slots, direct structured planning for tighter provider profiles, no model call for deterministic execution or verification, and aggregate usage tracking.",
      },
    ],
  },
  {
    label: "Security & Safety",
    id: "sec",
    items: [
      {
        tag: "SECURITY",
        q: "Is the tool runner sandboxed?",
        a: "The honest answer: it is workspace- and policy-bounded, but not an OS-level sandbox. Executable names are allowlisted, timeouts and output caps exist, and normal file tools are workspace-confined. But allowed interpreters like python can execute arbitrary code, subprocesses inherit the environment, and the current policy does not provide network or system-call isolation. Do not call it a 'secure sandbox.'",
      },
      {
        tag: "SECURITY",
        q: "What stops python -c from reading your home directory?",
        a: "Currently, not enough. The command starts in a bounded workspace, but cwd restriction is not filesystem isolation. Python code can reference absolute paths. The architecture needs an OS-level sandbox or a much narrower verification command contract before it should execute untrusted code against sensitive machines.",
      },
      {
        tag: "SECURITY",
        q: "Can the model modify tests to make them pass?",
        a: "Yes, that is a real risk. The current system reduces it through existing-test guidance and command policy, but does not eliminate it. Test integrity is an explicit policy and evaluation rule, but it still needs deterministic diff-level enforcement—snapshotting protected test paths before execution and rejecting modifications.",
      },
      {
        tag: "SECURITY",
        q: "Can tools access API keys?",
        a: "This is a current risk. Model keys are not written into prompts, UI state, or SQLite, but subprocesses inherit the engine environment by default. The correct production fix is to construct a minimal subprocess environment and explicitly remove provider credentials.",
      },
      {
        tag: "SECURITY",
        q: "How is the desktop gateway protected?",
        a: "The gateway binds only to 127.0.0.1, uses a random per-process token, requires the token in X-SecondEgo-Token, imposes request-size and field-length limits, exposes a small API, and keeps model credentials out of the renderer. One weakness is Access-Control-Allow-Origin: *, which is broader than a local desktop gateway requires.",
      },
    ],
  },
  {
    label: "Difficult Judge Questions",
    id: "judge",
    items: [
      {
        tag: "JUDGES",
        q: "How are you different from an IDE wrapper around an LLM?",
        a: "The defensible differences: explicit state transitions, indexed and budgeted context, schema-validated action proposals, detached transactional execution, evidence-based verification, failure classification and bounded recovery, explicit terminal outcomes, and headless reproducibility. The differentiation is the harness, not the visual shell.",
      },
      {
        tag: "JUDGES",
        q: "If the model chooses the edit and the test, can it game verification?",
        a: "Yes, that is a real risk. The current system reduces it through existing-test guidance and command policy, but does not eliminate it. The stronger next step: run deterministic baseline checks discovered before planning, model-proposed targeted tests, protected existing tests after modification, and diff-level enforcement preventing weakened tests or configurations.",
      },
      {
        tag: "JUDGES",
        q: "Why should we trust automatic recovery?",
        a: "Recovery is not trusted automatically. It receives actual failure evidence, produces another structured proposal, executes in the same disposable worktree, and must pass verification. Recovery is also bounded—it cannot loop indefinitely. The combination of recovery, model-call, tool-call, retry, context, and runtime budgets is what provides the constraint.",
      },
      {
        tag: "JUDGES",
        q: "What is your biggest technical debt?",
        a: "Deterministic security and verification policies need to catch up with the transactional architecture. Specifically: subprocess environment isolation, OS-level command sandboxing, enforced test/config protection, deterministic baseline test selection, broader benchmark coverage, and more complete persistence and replay.",
      },
      {
        tag: "JUDGES",
        q: "What is your biggest design success?",
        a: "The verified-diff boundary: failed attempts are disposable; only changes that pass verification are eligible to reach the target repository. That is a concrete reliability property, not a UI claim.",
      },
    ],
  },
];

export function JudgeQA() {
  const [activeGroup, setActiveGroup] = useState(qaGroups[0].id);
  const [openItems, setOpenItems] = useState<Set<string>>(new Set());

  const group = qaGroups.find((g) => g.id === activeGroup) ?? qaGroups[0];

  const toggle = (id: string) => {
    setOpenItems((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  return (
    <section className="judge-qa-section section-grid" id="judge-qa" data-tone="ink">
      <div className="wrap">
        <Reveal className="section-heading section-heading--split">
          <div>
            <p className="section-label">JUDGE Q&amp;A / HONEST ANSWERS ONLY</p>
            <h2>Every hard question.<br /><span className="text-accent">No hand-waving.</span></h2>
          </div>
          <p className="section-heading__aside">
            Drawn from a full pre-presentation audit. Answers are intentionally honest about what
            is implemented, what is limited, and what the next engineering step is.
          </p>
        </Reveal>

        <div className="judge-qa-layout">
          <nav className="judge-qa-nav" aria-label="Question categories">
            {qaGroups.map((g) => (
              <button
                key={g.id}
                type="button"
                className={`judge-qa-nav-btn${activeGroup === g.id ? " is-active" : ""}`}
                onClick={() => { setActiveGroup(g.id); setOpenItems(new Set()); }}
              >
                <span>{g.label}</span>
                <span className="judge-qa-nav-count">{g.items.length}</span>
              </button>
            ))}
          </nav>

          <div className="judge-qa-panel">
            {group.items.map((item, index) => {
              const itemId = `${activeGroup}-${index}`;
              const isOpen = openItems.has(itemId);
              return (
                <Reveal className={`judge-qa-item${isOpen ? " is-open" : ""}`} key={itemId} delay={index * 0.04}>
                  <button
                    className="judge-qa-question"
                    type="button"
                    aria-expanded={isOpen}
                    onClick={() => toggle(itemId)}
                  >
                    <div className="judge-qa-question__inner">
                      <span className="judge-qa-tag mono">{item.tag}</span>
                      <span className="judge-qa-q">{item.q}</span>
                    </div>
                    <span className="judge-qa-toggle" aria-hidden="true">{isOpen ? "−" : "+"}</span>
                  </button>
                  {isOpen && (
                    <div className="judge-qa-answer">
                      <p>{item.a}</p>
                    </div>
                  )}
                </Reveal>
              );
            })}
          </div>
        </div>
      </div>
    </section>
  );
}
