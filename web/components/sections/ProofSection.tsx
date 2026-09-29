import { Reveal } from "@/components/Reveal";
import { ProofConsole } from "@/components/ProofConsole";

export function ProofSection() {
  return (
    <section className="proof-section" id="proof" data-tone="ink">
      <div className="wrap proof-section__layout">
        <Reveal className="proof-section__copy">
          <p className="section-label">A RUN YOU CAN INSPECT</p>
          <h2>Don’t trust the story.<br /><span>Trace the run.</span></h2>
          <p>Every useful claim on this page has a corresponding place in the repository: a state transition, an allowlist, a fixture, a test, or a termination rule.</p>
          <div className="proof-links">
            <a className="text-link" href="https://github.com/TarunyaProgrammer/SecondEgo_SDETwin/tree/main/engine-rs" target="_blank" rel="noreferrer">Browse the Rust engine <span aria-hidden="true">↗</span></a>
            <a className="text-link" href="https://github.com/TarunyaProgrammer/SecondEgo_SDETwin/tree/main/tests" target="_blank" rel="noreferrer">Inspect the tests <span aria-hidden="true">↗</span></a>
          </div>
        </Reveal>
        <Reveal className="proof-section__visual" delay={0.1}><ProofConsole /></Reveal>
      </div>
    </section>
  );
}
