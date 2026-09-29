import { Reveal } from "@/components/Reveal";

export function FinalCTA() {
  return (
    <section className="final-cta section-grid" data-tone="paper">
      <div className="final-cta__mesh" aria-hidden="true" />
      <Reveal className="final-cta__content wrap">
        <div className="brand-mark brand-mark--large" aria-hidden="true"><i /><i /><i /></div>
        <p className="section-label">SECOND EGO / LOCAL BY DESIGN</p>
        <h2>Make the attempt<br /><span className="text-accent">provable.</span></h2>
        <p>Read the code, run the fixture, and decide whether the architecture earns your trust.</p>
        <a className="button button--primary" href="https://github.com/TarunyaProgrammer/SecondEgo_SDETwin" target="_blank" rel="noreferrer">Explore the repository <span aria-hidden="true">↗</span></a>
      </Reveal>
    </section>
  );
}
