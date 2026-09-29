import { Reveal } from "@/components/Reveal";

const repository = "https://github.com/TarunyaProgrammer/SecondEgo_SDETwin";

const contributors = [
  { name: "Tarunya K", handle: "TarunyaProgrammer", href: "https://github.com/TarunyaProgrammer", note: "Runtime and repository owner" },
  { name: "Yashika Gupta", handle: "yashikagupta", href: "https://github.com/yashikagupta", note: "Website contributor" },
  { name: "Riya Agarwal", handle: "riyaagarwal5040", href: "https://github.com/riyaagarwal5040", note: "Project contributor" },
  { name: "Mansha Agarwal", handle: "ManshaAgarwal716", href: "https://github.com/ManshaAgarwal716", note: "Code owner" },
];

export function TeamSection() {
  return (
    <section className="team-section wrap" id="team" data-tone="paper">
      <Reveal className="section-heading section-heading--split">
        <div><h2>Built in the open.<br /><span className="text-accent">Held to proof.</span></h2></div>
        <p className="section-heading__aside">SecondEgo is a small team project. The repository is the source of truth for the people, decisions, licenses, and work behind the product.</p>
      </Reveal>
      <div className="team-layout">
        <div className="team-statement">
          <p className="team-statement__mark" aria-hidden="true">SE</p>
          <p>We are building the harness we wanted before trusting one with a real codebase: local by default, inspectable at every boundary, and honest about what has and has not been verified.</p>
          <div className="team-statement__links">
            <a className="text-link" href={`${repository}/blob/main/CONTRIBUTING.md`} target="_blank" rel="noreferrer">Read how we work <span aria-hidden="true">↗</span></a>
            <a className="text-link" href={`${repository}/blob/main/LICENSE`} target="_blank" rel="noreferrer">MIT licensed <span aria-hidden="true">↗</span></a>
          </div>
        </div>
        <div className="contributors-list" aria-label="SecondEgo contributors">
          {contributors.map((person) => (
            <a className="contributor" key={person.handle} href={person.href} target="_blank" rel="noreferrer">
              <span className="contributor__avatar" aria-hidden="true">{person.name.split(" ").map((part) => part[0]).join("")}</span>
              <span className="contributor__copy"><strong>{person.name}</strong><span>@{person.handle}</span></span>
              <span className="contributor__note">{person.note}</span>
              <span aria-hidden="true">↗</span>
            </a>
          ))}
        </div>
      </div>
    </section>
  );
}
