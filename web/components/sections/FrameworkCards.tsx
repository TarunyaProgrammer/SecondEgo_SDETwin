import { Reveal } from "@/components/Reveal";
import { CodeBlock } from "@/components/CodeBlock";

const cards = [
  {
    number: "01", title: "Repository intelligence", tag: "FIND THE RELEVANT CONTEXT", description: "Scan, index, and rank source-linked evidence before a planning call.", icon: "⌕", href: "#lifecycle", link: "Trace the index phase",
    codeTitle: "retrieval.path", language: "EVIDENCE", codeLines: ["issue terms → ranked paths", "reasons + confidence", "bounded source excerpts", "omitted evidence tracked"],
  },
  {
    number: "02", title: "Transactional execution", tag: "KEEP ATTEMPTS ISOLATED", description: "Run proposed edits in a detached worktree. Transfer a bounded diff only after verification passes.", icon: "⌘", href: "#lifecycle", link: "Trace the attempt boundary",
    codeTitle: "transaction.state", language: "GIT", codeLines: ["clean target → worktree", "failed attempt → discard", "passed attempt → diff", "bounded transfer → target"],
  },
  {
    number: "03", title: "Bounded recovery", tag: "RESPOND TO OBSERVED FAILURE", description: "Classify verification output, refresh failure-specific evidence, and ask for one repair plan.", icon: "↻", href: "#reference-run", link: "See the recovery path",
    codeTitle: "recovery.state", language: "VERIFY", codeLines: ["failure → diagnosis", "retrieve relevant evidence", "request one repair plan", "fresh attempt → verify"],
  },
];

export function FrameworkCards() {
  return (
    <section className="capabilities-section wrap" id="capabilities">
      <Reveal className="section-heading section-heading--center">
        <p className="eyebrow">THREE ENGINEERING BOUNDARIES</p>
        <h2>Small surface area.<br /><span className="text-accent">Clear ownership.</span></h2>
        <p className="section-heading__center-copy">Each layer has a job. The model supplies proposals; the local engine owns policy, state, and proof.</p>
      </Reveal>
      <div className="capability-grid">
        {cards.map((card, index) => (
          <Reveal className="capability-card" key={card.number} delay={index * 0.07}>
            <div className="capability-card__top"><span className="capability-card__icon" aria-hidden="true">{card.icon}</span><span className="mono">{card.number}</span></div>
            <p className="capability-card__tag">{card.tag}</p>
            <h3>{card.title}</h3>
            <p className="capability-card__description">{card.description}</p>
            <a href={card.href} className="text-link">{card.link}<span aria-hidden="true"> ↗</span></a>
            <CodeBlock title={card.codeTitle} language={card.language} lines={card.codeLines} className="capability-card__code" />
          </Reveal>
        ))}
      </div>
    </section>
  );
}
