import { Reveal } from "@/components/Reveal";

export function FinalCTA() {
  return (
    <section className="final-cta section-grid">
      <div className="final-cta__mesh" aria-hidden="true" />
      <Reveal className="final-cta__content wrap">
        <div className="brand-mark brand-mark--large" aria-hidden="true"><i /><i /><i /></div>
        <p className="eyebrow">SECOND EGO / LOCAL BY DESIGN</p>
        <h2>Make the attempt<br /><span className="text-accent">provable.</span></h2>
        <p>Explore the runtime that puts boundaries, evidence, and verification around model-proposed code changes.</p>
        <a className="button button--primary" href="https://github.com/riyaagarwal5040/SecondEgo_SDETwin" target="_blank" rel="noreferrer">Explore the repository <span aria-hidden="true">↗</span></a>
      </Reveal>
    </section>
  );
}
