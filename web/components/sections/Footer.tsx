import { NewsletterSignup } from "@/components/NewsletterSignup";

const repository = "https://github.com/TarunyaProgrammer/SecondEgo_SDETwin";

const columns = [
  { title: "Products", links: [{ label: "Rust engine", href: `${repository}/tree/main/engine-rs` }, { label: "Python compatibility runtime", href: `${repository}/tree/main/src/SecondEgo` }, { label: "CLI", href: `${repository}/blob/main/README.md` }, { label: "TUI", href: `${repository}/blob/main/README.md` }] },
  { title: "Resources", links: [{ label: "Docs", href: `${repository}/blob/main/README.md` }, { label: "Judge's map", href: "#judges-map" }, { label: "Evaluation fixture", href: `${repository}/tree/main/evaluation` }, { label: "Changelog / commits", href: `${repository}/commits/main` }] },
  { title: "Project", links: [{ label: "About SecondEgo", href: "#top" }, { label: "Team", href: "#team" }, { label: "GitHub", href: repository }, { label: "MIT license", href: `${repository}/blob/main/LICENSE` }] },
];

export function Footer() {
  return (
    <footer className="site-footer" data-tone="paper">
      <div className="wrap footer-stage-shell">
        <div className="footer-stage">
          <div className="footer-stage__copy">
            <div className="footer-stage__actions">
              <a className="footer-pill" href={repository} target="_blank" rel="noreferrer"><span className="footer-pill__dot" aria-hidden="true" /> Open GitHub <span aria-hidden="true">↗</span></a>
              <a className="footer-pill" href={`${repository}/blob/main/LICENSE`} target="_blank" rel="noreferrer">MIT licensed <span aria-hidden="true">↗</span></a>
            </div>
            <p>SecondEgo is an open-source local coding-agent harness. The Rust engine is the default execution path; the Python runtime remains the compatibility and reference path during migration.</p>
          </div>
          <div className="footer-stage__scene" aria-hidden="true">
            <span className="footer-stage__light" />
            <span className="footer-stage__platform" />
            <span className="footer-stage__tower footer-stage__tower--left" />
            <span className="footer-stage__tower footer-stage__tower--center" />
            <span className="footer-stage__tower footer-stage__tower--right" />
            <span className="footer-stage__ladder" />
            <span className="footer-stage__beam" />
            <span className="footer-stage__node"><i /></span>
          </div>
          <div className="footer-stage__wordmark" aria-label="SecondEgo">secondego</div>
        </div>
      </div>
      <div className="wrap footer-details">
        <div className="footer-main">
          <div className="footer-brand">
            <a className="brand" href="#top"><span className="brand-mark" aria-hidden="true"><i /><i /><i /></span><span>second<span className="brand__ego">ego</span></span></a>
            <p>A local coding-agent harness built around bounded execution and verified change.</p>
            <a className="footer-github" href={repository} target="_blank" rel="noreferrer">Open source on GitHub <span aria-hidden="true">↗</span></a>
          </div>
          {columns.map((column) => (
            <div className="footer-column" key={column.title}>
              <h3>{column.title}</h3>
              {column.links.map((link) => <a key={link.label} href={link.href} target={link.href.startsWith("http") ? "_blank" : undefined} rel={link.href.startsWith("http") ? "noreferrer" : undefined}>{link.label}</a>)}
            </div>
          ))}
          <NewsletterSignup />
        </div>
        <div className="footer-bottom">
          <span>© 2026 SecondEgo · MIT License</span>
          <span className="footer-bottom__status"><i /> LOCAL-FIRST · OBSERVER SHELL OPTIONAL</span>
          <a href={`${repository}/blob/main/SECURITY.md`} target="_blank" rel="noreferrer">Security <span aria-hidden="true">↗</span></a>
        </div>
      </div>
    </footer>
  );
}
