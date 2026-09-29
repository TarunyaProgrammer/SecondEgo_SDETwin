import { Reveal } from "@/components/Reveal";
import { PipelineDiagram } from "@/components/PipelineDiagram";
import type { PipelineNode } from "@/components/PipelineDiagram";

const recoveryNodes: PipelineNode[] = [
  { label: "Pagination issue", detail: "separate fixture repo", state: "complete" },
  { label: "Structured plan", detail: "validated by runtime", state: "complete" },
  { label: "Verification fails", detail: "attempt discarded", state: "warning" },
  { label: "Repair plan", detail: "one fresh attempt", state: "active" },
  { label: "Verified diff", detail: "transfer on pass", state: "complete" },
];

export function ReferenceRunPanel() {
  return (
    <section className="reference-section wrap" id="reference-run" data-tone="ink">
      <Reveal className="reference-panel">
        <div className="reference-panel__glow" aria-hidden="true" />
        <div className="reference-panel__content">
          <div className="reference-panel__text">
            <p className="section-label"><span className="eyebrow__pulse" /> REFERENCE RUN / RECOVERY PATH</p>
            <h2>Failure is a<br />state, not a story.</h2>
            <p className="reference-panel__intro">The pagination fixture is a separate target repository with a deliberate off-by-one bug. This walkthrough maps the engine's documented recovery path: observe a failed check, discard the attempt, repair from a clean baseline, then transfer only after a passing verification.</p>
            <a className="button button--outline" href="https://github.com/TarunyaProgrammer/SecondEgo_SDETwin/blob/main/evaluation/README.md" target="_blank" rel="noreferrer">Open the evaluation walkthrough <span aria-hidden="true">↗</span></a>
            <p className="reference-panel__disclosure">Architecture walkthrough · not a published customer result</p>
          </div>
          <div className="reference-panel__visual">
            <div className="reference-panel__visual-head"><span className="mono">FIXTURE / PAGINATION</span><span className="reference-panel__live"><i /> REPLAYABLE</span></div>
            <PipelineDiagram nodes={recoveryNodes} label="Reference recovery path from pagination issue to verified diff" />
            <div className="reference-panel__result"><span className="mono">TERMINAL CONDITION</span><strong><i /> PASSING EVIDENCE REQUIRED</strong></div>
          </div>
        </div>
      </Reveal>
    </section>
  );
}
