import { Reveal } from "@/components/Reveal";
import { LogoMarquee } from "@/components/LogoMarquee";

const technologies = [
  { name: "Rust", mark: "◈" },
  { name: "Python", mark: "⌘" },
  { name: "Gemini", mark: "✳" },
  { name: "Tree-sitter", mark: "⌁" },
  { name: "Git", mark: "⑂" },
  { name: "SQLite", mark: "▤" },
];

export function LogoMarqueeSection() {
  return (
    <section className="tech-section" aria-label="Technology foundations" data-tone="paper">
      <div className="wrap">
        <Reveal className="tech-section__label"><span>BUILT AROUND</span><i /><span>PRIMITIVES YOU CAN INSPECT</span></Reveal>
      </div>
      <LogoMarquee items={technologies} label="Technology foundations: Rust, Python, Gemini, Tree-sitter, Git, and SQLite" />
    </section>
  );
}
