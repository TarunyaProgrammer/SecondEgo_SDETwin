"use client";

import { useState } from "react";

const links = [
  { label: "Architecture", href: "#architecture" },
  { label: "User flow", href: "#user-flow" },
  { label: "Judge's map", href: "#judges-map" },
  { label: "Judge Q&A", href: "#judge-qa" },
  { label: "Team", href: "#team" },
];

const repository = "https://github.com/TarunyaProgrammer/SecondEgo_SDETwin";

type NavProps = { onPresent?: () => void };

export function Nav({ onPresent }: NavProps) {
  const [open, setOpen] = useState(false);

  return (
    <header className="site-nav">
      <div className="site-nav__inner wrap">
        <a className="skip-link" href="#main-content">Skip to content</a>
        <a className="brand" href="#top" aria-label="SecondEgo home">
          <img src="/potential-logo.png" alt="SecondEgo Logo" width="47" height="26" className="brand-logo" />
          <span>second<span className="brand__ego">ego</span></span>
        </a>
        <nav className="site-nav__links" aria-label="Main navigation">
          {links.map((link) => <a key={link.href} href={link.href}>{link.label}</a>)}
        </nav>
        <div className="site-nav__actions">
          <a className="button button--quiet nav-github" href={repository} target="_blank" rel="noreferrer">GitHub <span aria-hidden="true">↗</span></a>
          {onPresent && (
            <button className="button button--present" type="button" onClick={onPresent} id="nav-present-btn">
              <span className="button-present-icon" aria-hidden="true">▶</span>
              Present
            </button>
          )}
          <a className="button button--small" href="#proof">See the proof <span aria-hidden="true">↘</span></a>
        </div>
        <button className={`menu-toggle${open ? " is-open" : ""}`} type="button" aria-label={open ? "Close menu" : "Open menu"} aria-expanded={open} onClick={() => setOpen(!open)}>
          <span /><span />
        </button>
      </div>
      <div className={`mobile-menu${open ? " is-open" : ""}`} aria-hidden={!open}>
        <nav aria-label="Mobile navigation">
          {links.map((link) => <a key={link.href} href={link.href} tabIndex={open ? 0 : -1} onClick={() => setOpen(false)}>{link.label}<span aria-hidden="true">↗</span></a>)}
          <a href={repository} target="_blank" rel="noreferrer" tabIndex={open ? 0 : -1}>GitHub<span aria-hidden="true">↗</span></a>
          {onPresent && (
            <button className="mobile-menu__cta" type="button" tabIndex={open ? 0 : -1} onClick={() => { setOpen(false); onPresent(); }}>
              ▶ Present
            </button>
          )}
        </nav>
      </div>
    </header>
  );
}
