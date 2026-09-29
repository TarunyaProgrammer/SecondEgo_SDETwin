import { Reveal } from "@/components/Reveal";

export function Hero() {
  return (
    <section className="hero section-grid" id="top" data-tone="paper">
      <div className="hero__mesh" aria-hidden="true"><div className="hero__grid" /></div>
      <div className="wrap hero__content">
        <Reveal>
          <p className="section-label">SECOND EGO / LOCAL CODING HARNESS</p>
          <h1>Make code changes<br /><strong>you can defend.</strong></h1>
          <p className="hero__subhead">SecondEgo turns a software issue into an isolated, verified diff. The model can propose. The runtime still has to prove it.</p>
          <div className="hero__actions">
            <a className="button button--primary" href="#proof">Trace one run <span aria-hidden="true">↓</span></a>
            <a className="button button--outline" href="https://github.com/TarunyaProgrammer/SecondEgo_SDETwin" target="_blank" rel="noreferrer">Open the repository <span aria-hidden="true">↗</span></a>
          </div>
          <div className="hero__assurance"><span><i /> local-first</span><span><i /> MIT licensed</span><span><i /> Rust default</span></div>
        </Reveal>
        <Reveal className="hero__artifact" delay={0.12}>
          <div className="hero__artifact-top"><span className="status-pill"><i /> RUN BOUNDARY</span><span className="mono muted-text">LOCAL / TRANSACTIONAL</span></div>
          <div className="hero__artifact-body">
            <div className="hero__artifact-rail" aria-hidden="true"><i /><i /><i /><i /><i /></div>
            <div className="hero__artifact-copy">
              <p><span className="syntax-blue">MODEL</span> <span className="muted-text">/ structured proposal</span></p>
              <p><span className="syntax-violet">RUNTIME</span> <span className="muted-text">/ validates every action</span></p>
              <p><span className="syntax-green">WORKTREE</span> <span className="muted-text">/ detached Git attempt</span></p>
              <p><span className="syntax-amber">VERIFY</span> <span className="muted-text">/ evidence decides completion</span></p>
            </div>
          </div>
          <div className="hero__artifact-bottom"><span>DECISION OWNERSHIP</span><span className="hero__artifact-rule" /><strong>RUNTIME</strong></div>
        </Reveal>
        <a className="hero__scroll-note mono" href="#proof"><span /> SCROLL TO TRACE THE RUN</a>
      </div>
    </section>
  );
}
