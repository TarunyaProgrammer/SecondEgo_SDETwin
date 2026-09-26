import { Reveal } from "@/components/Reveal";

export function Hero() {
  return (
    <section className="hero section-grid" id="top">
      <div className="hero__mesh" aria-hidden="true"><div className="hero__orb hero__orb--one" /><div className="hero__orb hero__orb--two" /><div className="hero__grid" /></div>
      <div className="wrap hero__content">
        <Reveal>
          <div className="eyebrow"><span className="eyebrow__pulse" /> LOCAL-FIRST ENGINEERING HARNESS</div>
          <h1>Issue in.<br /><strong>Verified diff out.</strong></h1>
          <p className="hero__subhead">SecondEgo turns a software issue into an isolated, verified diff—with runtime policy in control at every step.</p>
          <div className="hero__actions">
            <a className="button button--primary" href="#lifecycle">Explore the architecture <span aria-hidden="true">↓</span></a>
            <a className="button button--outline" href="https://github.com/riyaagarwal5040/SecondEgo_SDETwin" target="_blank" rel="noreferrer">View source <span aria-hidden="true">↗</span></a>
          </div>
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
          <div className="hero__artifact-bottom"><span>DECISION OWNERSHIP</span><span className="hero__artifact-rule" /><strong>DETERMINISTIC</strong></div>
        </Reveal>
        <div className="hero__scroll-note mono"><span /> SCROLL TO TRACE THE RUN</div>
      </div>
    </section>
  );
}
