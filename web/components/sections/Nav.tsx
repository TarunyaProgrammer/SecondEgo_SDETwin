"use client";

import { useState } from "react";

const links = [
  { label: "Architecture", href: "#lifecycle" },
  { label: "Capabilities", href: "#capabilities" },
  { label: "Reference run", href: "#reference-run" },
];

export function Nav() {
  const [open, setOpen] = useState(false);

  return (
    <header className="site-nav">
      <div className="site-nav__inner wrap">
        <a className="brand" href="#top" aria-label="SecondEgo home">
          <span className="brand-mark" aria-hidden="true"><i /><i /><i /></span>
          <span>second<span className="brand__ego">ego</span></span>
        </a>
        <nav className="site-nav__links" aria-label="Main navigation">
          {links.map((link) => <a key={link.href} href={link.href}>{link.label}</a>)}
        </nav>
        <div className="site-nav__actions">
          <a className="button button--quiet nav-github" href="https://github.com/riyaagarwal5040/SecondEgo_SDETwin" target="_blank" rel="noreferrer">GitHub <span aria-hidden="true">↗</span></a>
          <a className="button button--small" href="#reference-run">See the flow <span aria-hidden="true">↘</span></a>
        </div>
        <button className={`menu-toggle${open ? " is-open" : ""}`} type="button" aria-label={open ? "Close menu" : "Open menu"} aria-expanded={open} onClick={() => setOpen(!open)}>
          <span /><span />
        </button>
      </div>
      <div className={`mobile-menu${open ? " is-open" : ""}`} aria-hidden={!open}>
        <nav aria-label="Mobile navigation">
          {links.map((link) => <a key={link.href} href={link.href} tabIndex={open ? 0 : -1} onClick={() => setOpen(false)}>{link.label}<span aria-hidden="true">↗</span></a>)}
          <a href="https://github.com/riyaagarwal5040/SecondEgo_SDETwin" target="_blank" rel="noreferrer" tabIndex={open ? 0 : -1}>GitHub<span aria-hidden="true">↗</span></a>
          <a className="mobile-menu__cta" href="#reference-run" tabIndex={open ? 0 : -1} onClick={() => setOpen(false)}>See the flow<span aria-hidden="true">↘</span></a>
        </nav>
      </div>
    </header>
  );
}
