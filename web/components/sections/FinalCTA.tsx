import { Reveal } from "@/components/Reveal";

export function FinalCTA() {
  return (
    <section className="final-cta section-grid" data-tone="paper">
      <div className="final-cta__mesh" aria-hidden="true" />
      <Reveal className="final-cta__content wrap">
        <img src="/potential-logo.png" alt="SecondEgo Logo" width="94" height="52" loading="lazy" decoding="async" className="brand-logo brand-logo--large" />
        <p className="section-label">SECOND EGO / LOCAL BY DESIGN</p>
        <h2>Make the attempt<br /><span className="text-accent">provable.</span></h2>
        <p>Read the code, run the fixture, and decide whether the architecture earns your trust.</p>
        <a className="button button--primary" href="https://github.com/TarunyaProgrammer/SecondEgo_SDETwin" target="_blank" rel="noreferrer">Explore the repository <span aria-hidden="true">↗</span></a>
      </Reveal>
    </section>
  );
}
