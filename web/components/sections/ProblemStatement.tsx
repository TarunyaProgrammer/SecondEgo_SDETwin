"use client";

import { Reveal } from "@/components/Reveal";

const problems = [
  {
    id: "01",
    title: "Stale context",
    body: "LLMs suggest code based on what they were shown—not what the repository actually contains. Without focused retrieval, the model hallucinates file paths, function signatures, and APIs.",
    fix: "Bounded AST-aware index → ranked retrieval → source-linked evidence packet. Only the top signals enter the context window.",
  },
  {
    id: "02",
    title: "Unchecked edits",
    body: "A chat interface that can write files has no notion of \"transaction.\" A partial edit or a crashed run leaves the working tree in an unknown state.",
    fix: "Every attempt runs in a detached Git worktree. The target working tree is untouched until a verified diff is transferred.",
  },
  {
    id: "03",
    title: "Self-reported success",
    body: "Models narrate what they did. A narration cannot substitute for an exit code, a passing test, or a clean diff. \"Done\" is not evidence.",
    fix: "Completion requires passing command output, changed paths, Git evidence, and an explicit terminal status. Narration has no authority.",
  },
  {
    id: "04",
    title: "Silent failure",
    body: "When an LLM-driven tool fails, the user often sees a generic error or nothing at all. The failure mode is hidden and the repository is left dirty.",
    fix: "Every failure is classified—test, build, lint, type, environment, tool, planning—and produces a bounded evidence record before recovery.",
  },
  {
    id: "05",
    title: "Unbounded loops",
    body: "An unconstrained retry loop burns tokens, time, and money. Worse, it can spin forever on a failure it cannot fix, making the same mistake on each iteration.",
    fix: "Recovery is bounded to three execution attempts. Separate budgets cap model calls, tool calls, retries, context size, and total runtime.",
  },
];

export function ProblemStatement() {
  return (
    <section className="problem-section section-grid" id="problem" data-tone="ink">
      <div className="wrap problem-wrap">
        <Reveal className="section-heading section-heading--split">
          <div>
            <p className="section-label">THE PROBLEM WITH LLM CODING TOOLS</p>
            <h2>The model can suggest.<br /><span className="text-accent">It cannot verify.</span></h2>
          </div>
          <p className="section-heading__aside">
            Foundation models make plausible suggestions. They do not index your repository, isolate their
            changes, run your tests, or prove the result. SecondEgo is the control system that bridges the gap.
          </p>
        </Reveal>

        <div className="problem-grid">
          {problems.map((problem, index) => (
            <Reveal className="problem-card" key={problem.id} delay={index * 0.06}>
              <div className="problem-card__top">
                <span className="mono problem-card__id">{problem.id}</span>
                <span className="problem-card__tag">WITHOUT A HARNESS</span>
              </div>
              <h3 className="problem-card__title">{problem.title}</h3>
              <p className="problem-card__body">{problem.body}</p>
              <div className="problem-card__fix">
                <span className="problem-card__fix-label mono">SecondEgo fix</span>
                <p>{problem.fix}</p>
              </div>
            </Reveal>
          ))}
        </div>

        <Reveal className="problem-principle" delay={0.15}>
          <div className="problem-principle__inner">
            <span className="mono">CORE PRINCIPLE</span>
            <blockquote>
              "The model proposes.<br /><strong>The engine disposes.</strong>"
            </blockquote>
            <p>
              The model cannot directly declare success, mutate the production workspace, or choose which
              tests count as verification. Every proposal passes through schema validation, policy
              enforcement, isolated execution, and real command evidence before the runtime accepts it.
            </p>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
