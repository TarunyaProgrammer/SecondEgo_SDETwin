import { Reveal } from "@/components/Reveal";

const stats = [
  { value: "8", label: "allowlisted executables", note: "Commands run without a shell" },
  { value: "24K", label: "estimated tokens per call", note: "A bounded context packet" },
  { value: "120s", label: "maximum command timeout", note: "A hard cap for each command" },
];

export function Stats() {
  return (
    <section className="stats-section wrap" aria-label="Runtime limits" data-tone="paper">
      <Reveal className="stats-heading"><p className="section-label">BOUNDS THAT ARE EXPLICIT</p><p className="stats-heading__copy">Resource limits live in the runtime,<br />where they can be enforced.</p></Reveal>
      <div className="stats-grid">
        {stats.map((stat, index) => (
          <Reveal className="stat-card" key={stat.value} delay={index * 0.08}>
            <div className="stat-card__top"><span className="mono">LIMIT / 0{index + 1}</span><span aria-hidden="true">↗</span></div>
            <strong>{stat.value}</strong>
            <span className="stat-card__label">{stat.label}</span>
            <span className="stat-card__note">{stat.note}</span>
          </Reveal>
        ))}
      </div>
    </section>
  );
}
