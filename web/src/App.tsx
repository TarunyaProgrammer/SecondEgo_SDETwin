import { useEffect, useMemo, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import { Reveal } from "@/components/Reveal";
import { applyPageMeta } from "./seo";
import "./styles.css";

const repository = "https://github.com/TarunyaProgrammer/SecondEgo_SDETwin";
const releases = `${repository}/releases`;
type Platform = "macOS" | "Windows" | "Linux" | "Unknown";

const providers = [
  { name: "OpenAI", mark: "O", color: "rose", copy: "Responses and chat models behind one normalized runtime contract." },
  { name: "Anthropic", mark: "A", color: "amber", copy: "Native Messages support with explicit retries and structured proposals." },
  { name: "Gemini", mark: "G", color: "blue", copy: "Schema-aware planning through the Google GenAI adapter." },
  { name: "DeepSeek", mark: "D", color: "coral", copy: "OpenAI-compatible access with a clean migration path." },
  { name: "OpenRouter", mark: "R", color: "violet", copy: "A routed catalog when you need breadth without changing orchestration." },
] as const;

const journey = [
  { label: "Understand", title: "Start with the repository, not a guess.", copy: "SecondEgo indexes the workspace, finds the relevant surfaces, and keeps the context window focused on evidence that can change the answer." },
  { label: "Execute", title: "Work in a bounded, inspectable attempt.", copy: "The Rust engine owns permissions, tool budgets, isolated worktrees, and provider output normalization. The UI observes the state; it does not improvise authority." },
  { label: "Prove", title: "Finish with a diff you can defend.", copy: "Tests, changed paths, tool calls, and termination reasons remain visible. Review the diff, apply it, or discard the attempt." },
] as const;

const docsNavigation = [
  { label: "Introduction", href: "#introduction" },
  { label: "Install", href: "#install" },
  { label: "Provider keys", href: "#providers" },
  { label: "Desktop and terminal", href: "#surfaces" },
  { label: "Architecture", href: "#architecture" },
  { label: "Security", href: "#security" },
] as const;

function detectPlatform(): Platform {
  if (typeof navigator === "undefined") return "Unknown";
  const value = `${navigator.userAgent} ${navigator.platform}`.toLowerCase();
  if (value.includes("mac")) return "macOS";
  if (value.includes("win")) return "Windows";
  if (value.includes("linux")) return "Linux";
  return "Unknown";
}

function useRoute() {
  const [path, setPath] = useState(() => window.location.pathname.replace(/\/$/, "") || "/");
  useEffect(() => {
    const onPopState = () => setPath(window.location.pathname.replace(/\/$/, "") || "/");
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);
  const navigate = (to: string) => {
    const [pathname, hash] = to.split("#");
    const next = pathname || "/";
    if (next !== window.location.pathname) {
      window.history.pushState({}, "", `${next}${hash ? `#${hash}` : ""}`);
      setPath(next.replace(/\/$/, "") || "/");
    } else if (hash) window.history.pushState({}, "", `${next}#${hash}`);
    window.scrollTo({ top: 0, behavior: "smooth" });
    if (hash) window.setTimeout(() => document.getElementById(hash)?.scrollIntoView({ behavior: "smooth" }), 30);
  };
  return { path, navigate };
}

function Arrow({ direction = "right" }: { direction?: "right" | "down" }) {
  return <svg className="icon" viewBox="0 0 18 18" aria-hidden="true">{direction === "down" ? <path d="M9 3v11m-4-4 4 4 4-4" /> : <path d="M3 9h11m-4-4 4 4-4 4" />}</svg>;
}

function MenuIcon({ open }: { open: boolean }) {
  return <svg className="icon" viewBox="0 0 20 20" aria-hidden="true"><path d={open ? "M4 4 16 16M16 4 4 16" : "M3 5h14M3 10h14M3 15h14"} /></svg>;
}

function Logo({ navigate, light = false }: { navigate: (to: string) => void; light?: boolean }) {
  return <button className={`logo${light ? " logo--light" : ""}`} onClick={() => navigate("/")} aria-label="SecondEgo home"><span className="logo-mark" aria-hidden="true"><i /><i /><i /></span><span>second<span>ego</span></span></button>;
}

function LinkButton({ to, children, className = "", navigate, external = false, onClick }: { to: string; children: ReactNode; className?: string; navigate: (to: string) => void; external?: boolean; onClick?: () => void }) {
  if (external) return <a className={className} href={to} target="_blank" rel="noreferrer">{children}</a>;
  return <a className={className} href={to} onClick={(event) => { event.preventDefault(); onClick?.(); navigate(to); }}>{children}</a>;
}

function Navigation({ navigate }: { navigate: (to: string) => void }) {
  const [open, setOpen] = useState(false);
  const close = () => setOpen(false);
  return <header className={`site-nav${open ? " is-open" : ""}`}><div className="site-nav__inner"><Logo navigate={navigate} light /><nav className="site-nav__links" aria-label="Primary navigation"><LinkButton to="/docs" navigate={navigate} onClick={close}>Docs</LinkButton><LinkButton to="/download" navigate={navigate} onClick={close}>Download</LinkButton><LinkButton to="/about" navigate={navigate} onClick={close}>About</LinkButton><a href={repository} target="_blank" rel="noreferrer">GitHub <Arrow /></a></nav><div className="site-nav__actions"><LinkButton to="/download" navigate={navigate} className="button button--small button--red" onClick={close}>Get SecondEgo <Arrow /></LinkButton><button className="menu-button" type="button" aria-label={open ? "Close navigation menu" : "Open navigation menu"} aria-expanded={open} onClick={() => setOpen((value) => !value)}><MenuIcon open={open} /></button></div></div></header>;
}

function Eyebrow({ children, light = false }: { children: ReactNode; light?: boolean }) { return <p className={`eyebrow${light ? " eyebrow--light" : ""}`}><span />{children}</p>; }

function Hero({ navigate }: { navigate: (to: string) => void }) {
  const platform = detectPlatform();
  return <section className="hero" id="top"><div className="hero-wallpaper" aria-hidden="true"><span className="hero-wallpaper__orb hero-wallpaper__orb--one" /><span className="hero-wallpaper__orb hero-wallpaper__orb--two" /><span className="hero-wallpaper__beam hero-wallpaper__beam--one" /><span className="hero-wallpaper__beam hero-wallpaper__beam--two" /><span className="hero-wallpaper__rings" /><span className="hero-wallpaper__grain" /></div><div className="page-width hero__inner"><Reveal className="hero__copy"><Eyebrow light>LOCAL-FIRST / BUILT FOR PROOF</Eyebrow><h1>The coding agent<br /><em>with a conscience.</em></h1><p className="hero__lead">SecondEgo turns a software issue into a bounded attempt: retrieve the right context, make the change, run the checks, and show the evidence before you ship.</p><div className="hero__actions"><LinkButton to="/download" navigate={navigate} className="button button--red">Download for {platform === "Unknown" ? "your OS" : platform} <Arrow /></LinkButton><LinkButton to="/docs" navigate={navigate} className="button button--quiet">Read the architecture <Arrow /></LinkButton></div><div className="hero__proof"><span><b>01</b> local engine</span><span><b>02</b> five providers</span><span><b>03</b> reviewable diffs</span></div></Reveal><Reveal className="hero__product" delay={0.12}><div className="product-window"><div className="product-window__top"><span className="window-dots"><i /><i /><i /></span><span>mission control</span><span className="live-pill"><i /> live</span></div><div className="product-window__body"><aside className="product-window__rail"><span className="rail-active">⌁</span><span>◈</span><span>⌘</span><span>＋</span><span>⚙</span></aside><div className="run-view"><div className="run-view__header"><div><small>ACTIVE ATTEMPT</small><h3>Refine provider lifecycle</h3></div><span className="status-badge">EXECUTING</span></div><div className="run-progress"><span /><span /><span className="is-active" /><span /><span /></div><div className="run-steps"><div className="run-step is-done"><b>01</b><span><strong>Understand</strong><small>repository map refreshed</small></span><em>done</em></div><div className="run-step is-done"><b>02</b><span><strong>Plan</strong><small>proposal normalized</small></span><em>done</em></div><div className="run-step is-active"><b>03</b><span><strong>Execute</strong><small>isolated worktree / 2 paths</small></span><em>now</em></div><div className="run-step"><b>04</b><span><strong>Verify</strong><small>waiting on checks</small></span><em>next</em></div></div><div className="run-footer"><span>provider <strong>OpenAI</strong></span><span>workspace <strong>secondego</strong></span><span>budget <strong>64k</strong></span></div></div></div></div><div className="product-window__caption"><span>One surface for every run.</span><span>Bounded by the engine.</span></div></Reveal></div><a className="hero__scroll" href="#signal" aria-label="Scroll to learn more"><span />scroll to explore <Arrow direction="down" /></a></section>;
}

function ProviderBand() {
  return <section className="provider-band" id="signal"><div className="page-width"><div className="provider-band__intro"><Eyebrow>YOUR KEY. YOUR CONTROL.</Eyebrow><h2>Bring the model<br /><em>you already trust.</em></h2><p>Five provider adapters, one stable execution contract. Add multiple profiles, switch per workspace, and keep credentials in the operating system vault.</p></div><div className="provider-orbit">{providers.map((provider, index) => <div className={`provider-chip provider-chip--${provider.color}`} key={provider.name} style={{ "--chip-index": index } as CSSProperties}><span>{provider.mark}</span><strong>{provider.name}</strong></div>)}</div></div></section>;
}

function StorySection() {
  const [active, setActive] = useState(0);
  const item = journey[active];
  return <section className="story-section section-dark"><div className="page-width story-layout"><div><Eyebrow light>THE SECOND EGO LOOP</Eyebrow><h2>Less magic.<br /><em>More signal.</em></h2><p className="section-lead">Agent software earns trust in the parts it makes observable. SecondEgo keeps the loop legible from first context read to final diff.</p><div className="story-tabs" role="tablist" aria-label="SecondEgo workflow">{journey.map((step, index) => <button type="button" role="tab" aria-selected={active === index} className={active === index ? "is-active" : ""} key={step.label} onClick={() => setActive(index)}><span>0{index + 1}</span>{step.label}<Arrow /></button>)}</div></div><Reveal className="story-card"><div className="story-card__number">0{active + 1}</div><Eyebrow light>{item.label} / RUNTIME STATE</Eyebrow><h3>{item.title}</h3><p>{item.copy}</p><div className="story-card__signal"><span className="signal-line"><i /><i /><i /><i /><i /><i /></span><span>state transition visible</span></div></Reveal></div></section>;
}

function ArchitectureTeaser({ navigate }: { navigate: (to: string) => void }) {
  return <section className="architecture-teaser"><div className="page-width"><div className="section-heading"><div><Eyebrow>THE CONTROL PLANE</Eyebrow><h2>The model reasons.<br /><em>The engine decides.</em></h2></div><p>Electron and terminal are control surfaces. Rust owns policy, tools, worktrees, verification, and the evidence trail.</p></div><div className="architecture-grid"><div className="architecture-card architecture-card--surface"><span>01</span><strong>Desktop + terminal</strong><small>human control surfaces</small></div><div className="architecture-connector"><Arrow /></div><div className="architecture-card architecture-card--api"><span>02</span><strong>Authenticated loopback</strong><small>versioned local API</small></div><div className="architecture-connector"><Arrow /></div><div className="architecture-card architecture-card--engine"><span>03</span><strong>Rust engine</strong><small>state, policy, proof</small></div><div className="architecture-branches"><div><span>04A</span><strong>Provider registry</strong><small>five adapters / key handles</small></div><div><span>04B</span><strong>Bounded tools</strong><small>paths / commands / budgets</small></div><div><span>04C</span><strong>SQLite evidence</strong><small>runs / events / diffs</small></div></div></div><div className="section-footer"><p>Read the HLD, LLD, key lifecycle, and migration notes in the documentation.</p><LinkButton to="/docs#architecture" navigate={navigate} className="text-link">Explore the architecture <Arrow /></LinkButton></div></div></section>;
}

function HomePage({ navigate }: { navigate: (to: string) => void }) {
  return <><Hero navigate={navigate} /><ProviderBand /><StorySection /><ArchitectureTeaser navigate={navigate} /><section className="home-cta"><div className="page-width home-cta__inner"><Eyebrow light>READY WHEN YOU ARE</Eyebrow><h2>Make the next change<br /><em>worth reviewing.</em></h2><div><p>Download the app, bring one provider key, and keep the first run close to the evidence.</p><LinkButton to="/download" navigate={navigate} className="button button--red">Choose your platform <Arrow /></LinkButton></div></div></section></>;
}

function DocsPage({ navigate }: { navigate: (to: string) => void }) {
  return <div className="docs-page page-width"><div className="docs-page__top"><Eyebrow>DOCUMENTATION / VERSION 1</Eyebrow><h1>Build with<br /><em>the whole picture.</em></h1><p>Everything required to install SecondEgo, configure provider profiles, understand the local control plane, and debug a run without guessing.</p></div><div className="docs-layout"><aside className="docs-sidebar"><span className="docs-sidebar__label">On this page</span>{docsNavigation.map((item, index) => <a href={item.href} key={item.label}><b>0{index + 1}</b>{item.label}</a>)}<div className="docs-sidebar__card"><span>Need the app?</span><strong>Pick your platform and start local.</strong><LinkButton to="/download" navigate={navigate} className="text-link">Downloads <Arrow /></LinkButton></div></aside><main className="docs-content"><DocBlock id="introduction" eyebrow="01 / INTRODUCTION" title="What SecondEgo is"><p>SecondEgo is a local coding-agent harness for software work that should end in a reviewable, verifiable change. The provider can propose; the runtime owns the attempt.</p><div className="docs-callout"><span>Principle</span><strong>Every run should leave behind enough evidence for a human to understand what happened.</strong></div></DocBlock><DocBlock id="install" eyebrow="02 / INSTALL" title="Install the right surface"><p>Download the desktop app for a guided experience, or use the terminal for scripts, CI, and headless runs. Both surfaces talk to the same local engine boundary.</p><div className="install-steps"><Step number="01" title="Download" copy="Choose macOS, Windows, or Linux. The download page recommends the platform detected by your browser." /><Step number="02" title="Add a provider" copy="Create one or more profiles. The secret is stored in the OS credential vault; the database keeps only metadata." /><Step number="03" title="Run and review" copy="Start a task, watch the state transitions, inspect the diff, then apply or discard the attempt." /></div></DocBlock><DocBlock id="providers" eyebrow="03 / PROVIDER KEYS" title="One platform, five keys"><p>OpenAI, Anthropic, Gemini, DeepSeek, and OpenRouter are exposed through one provider registry. The orchestration layer never needs to know which vendor is active.</p><div className="provider-table">{providers.map((provider) => <div key={provider.name}><span className={`provider-dot provider-dot--${provider.color}`}>{provider.mark}</span><strong>{provider.name}</strong><span>{provider.copy}</span><b>native profile</b></div>)}</div></DocBlock><DocBlock id="surfaces" eyebrow="04 / SURFACES" title="Desktop, terminal, same engine"><p>The notch-style desktop entry is intentionally glanceable. Full runs, history, diff review, provider settings, and failure recovery live in the expanded workspace. The CLI exposes the same state with human-readable and JSON output.</p><div className="surface-grid"><div><strong>Desktop</strong><span>Visual state, provider profiles, history, diff review.</span></div><div><strong>Terminal</strong><span>Scriptable runs, preflight, provider status, machine-readable output.</span></div></div></DocBlock><DocBlock id="architecture" eyebrow="05 / ARCHITECTURE" title="HLD and LLD"><p>The control plane is deliberately small: an Electron shell, an authenticated loopback gateway, a Rust runtime, provider adapters, bounded tools, and SQLite evidence.</p><div className="diagram-mini"><div><span>UI</span><strong>Electron + React</strong></div><i>→</i><div><span>API</span><strong>loopback gateway</strong></div><i>→</i><div className="diagram-mini__accent"><span>CORE</span><strong>Rust engine</strong></div><div className="diagram-mini__bottom"><span>provider registry</span><span>tool policy</span><span>evidence store</span></div></div><div className="docs-links"><a href={`${repository}/blob/main/docs/diagrams/second-ego-hld.mmd`} target="_blank" rel="noreferrer">Open HLD source <Arrow /></a><a href={`${repository}/blob/main/docs/diagrams/provider-key-lld.mmd`} target="_blank" rel="noreferrer">Open key LLD source <Arrow /></a></div></DocBlock><DocBlock id="security" eyebrow="06 / SECURITY" title="Keys stay close to their owner"><p>Provider secrets are accepted through explicit configuration flows, stored in the native credential vault, and passed to the runtime only for the active session. They are not persisted in SQLite, renderer state, run events, or diffs.</p><div className="security-list"><span><b>01</b> OS vault storage</span><span><b>02</b> Authenticated local boundary</span><span><b>03</b> Redacted telemetry</span><span><b>04</b> Bounded workspace permissions</span></div></DocBlock></main></div></div>;
}

function DocBlock({ id, eyebrow, title, children }: { id: string; eyebrow: string; title: string; children: ReactNode }) { return <section className="doc-block" id={id}><Eyebrow>{eyebrow}</Eyebrow><h2>{title}</h2><div className="doc-block__body">{children}</div></section>; }
function Step({ number, title, copy }: { number: string; title: string; copy: string }) { return <div className="install-step"><span>{number}</span><div><strong>{title}</strong><p>{copy}</p></div></div>; }

function DownloadPage() {
  const detected = detectPlatform();
  const options = [{ name: "macOS", version: "Apple Silicon + Intel", icon: "⌘", copy: "DMG installer with a guided first launch.", status: "Release candidate", available: true }, { name: "Windows", version: "Windows 10 or newer · x64", icon: "⊞", copy: "NSIS installer with native credential vault integration.", status: "First release pending", available: false }, { name: "Linux", version: "x64", icon: "◒", copy: "AppImage and deb packages with a terminal-first workflow.", status: "First release pending", available: false }];
  return <div className="download-page page-width"><div className="download-hero"><div><Eyebrow>DOWNLOAD SECOND EGO</Eyebrow><h1>Your workspace.<br /><em>Your platform.</em></h1><p>Choose a native surface for your operating system. We detect your platform in the browser and keep the release status clear.</p></div><div className="detected-card"><span>Detected on this device</span><strong>{detected === "Unknown" ? "Choose a platform" : detected}</strong><small>{detected === "Unknown" ? "No guess made" : "Recommended download"}</small></div></div><div className="download-grid">{options.map((option) => { const recommended = option.name === detected; return <article className={`download-card${recommended ? " is-recommended" : ""}`} key={option.name}><div className="download-card__top"><span className="os-icon">{option.icon}</span>{recommended && <b className="recommendation">Recommended</b>}</div><h2>{option.name}</h2><span className="download-card__version">{option.version}</span><p>{option.copy}</p><div className="download-card__bottom"><span className={option.available ? "status-live" : "status-muted"}><i />{option.status}</span>{option.available ? <a className="button button--red button--small" href={releases} target="_blank" rel="noreferrer">View release <Arrow /></a> : <a className="text-link" href={repository} target="_blank" rel="noreferrer">Watch GitHub <Arrow /></a>}</div></article>; })}</div><section className="download-guide"><div><Eyebrow>FIRST LAUNCH</Eyebrow><h2>Two minutes to<br /><em>your first run.</em></h2></div><div className="download-guide__steps"><Step number="01" title="Install the app" copy="Open the package, move SecondEgo to Applications or your platform's app directory, then launch it." /><Step number="02" title="Choose a provider" copy="Paste your own key in Settings → Providers. It is validated, stored in the native vault, and never shown again." /><Step number="03" title="Run with proof" copy="Start a task, review the generated diff, and keep the evidence that explains the result." /></div></section><div className="release-note"><strong>Release note</strong><span>macOS candidates currently publish through GitHub Releases with checksums. Windows and Linux artifacts are marked honestly until their packaging pipelines are shipped.</span></div></div>;
}

function AboutPage({ navigate }: { navigate: (to: string) => void }) {
  return <div className="about-page page-width"><div className="about-hero"><Eyebrow>ABOUT SECOND EGO</Eyebrow><h1>Software that<br /><em>knows its edges.</em></h1><p>We are building a local-first coding agent for people who want leverage without surrendering the shape of the work.</p></div><div className="about-grid"><div className="about-statement"><span>Our thesis</span><h2>Useful autonomy is not a magic trick. It is a sequence of explicit boundaries.</h2></div><div className="principles"><div><b>01</b><strong>Local by default</strong><p>Your repository and provider keys stay close to the machine that owns them.</p></div><div><b>02</b><strong>Evidence over theatre</strong><p>A green animation is not proof. Diffs, checks, paths, and termination reasons are.</p></div><div><b>03</b><strong>Composable by design</strong><p>Provider adapters, surfaces, and tools stay behind contracts so the system can evolve without a rewrite.</p></div></div></div><section className="team-note"><Eyebrow light>OPEN SOURCE, IN THE OPEN</Eyebrow><h2>Read the code.<br /><em>Challenge the choices.</em></h2><p>SecondEgo is an evolving open-source project. The repository is the most accurate place to inspect implementation status, release work, and architectural decisions.</p><div><a className="button button--red" href={repository} target="_blank" rel="noreferrer">Visit GitHub <Arrow /></a><LinkButton to="/docs" navigate={navigate} className="button button--quiet">Read the docs <Arrow /></LinkButton></div></section></div>;
}

function TermsPage() {
  return <div className="legal-page page-width"><Eyebrow>TERMS AND CONDITIONS</Eyebrow><h1>Clear terms for<br /><em>a local tool.</em></h1><p className="legal-lead">Last updated October 20, 2026. This page is a product-facing summary; have counsel review any terms before using it as a binding commercial agreement.</p><div className="legal-content"><LegalBlock title="1. The software"><p>SecondEgo is open-source software provided under the license included in the repository. You are responsible for reviewing the license and complying with it when you use, modify, or redistribute the project.</p></LegalBlock><LegalBlock title="2. Your credentials"><p>You bring and control your own model-provider credentials. You are responsible for their accuracy, applicable provider terms, usage limits, and any costs created by your requests. Do not share keys with people or systems that should not have them.</p></LegalBlock><LegalBlock title="3. Your repository"><p>You remain responsible for the code, data, and credentials in the workspace where the app runs. Review generated changes before applying them. A local agent can make mistakes, and no generated patch should be treated as automatically safe.</p></LegalBlock><LegalBlock title="4. No warranty"><p>The project is provided on an as-is basis to the maximum extent allowed by law. We do not promise uninterrupted operation, model availability, release artifact availability, or that generated changes will be correct for your situation.</p></LegalBlock><LegalBlock title="5. Changes"><p>We may update this page as the product and release infrastructure evolve. The repository and release notes remain the source of truth for implementation and distribution status.</p></LegalBlock></div><a className="text-link" href={repository} target="_blank" rel="noreferrer">Read the license in GitHub <Arrow /></a></div>;
}
function LegalBlock({ title, children }: { title: string; children: ReactNode }) { return <section><h2>{title}</h2>{children}</section>; }

function Footer({ navigate }: { navigate: (to: string) => void }) {
  return <footer className="site-footer"><div className="page-width"><div className="site-footer__top"><div><Logo navigate={navigate} light /><p>A local coding agent built around bounded execution, provider choice, and verified change.</p></div><div className="site-footer__links"><div><span>Explore</span><LinkButton to="/docs" navigate={navigate}>Docs</LinkButton><LinkButton to="/download" navigate={navigate}>Download</LinkButton><LinkButton to="/about" navigate={navigate}>About</LinkButton></div><div><span>Project</span><a href={repository} target="_blank" rel="noreferrer">GitHub</a><LinkButton to="/terms" navigate={navigate}>Terms</LinkButton><a href={`${repository}/blob/main/SECURITY.md`} target="_blank" rel="noreferrer">Security</a></div></div></div><div className="site-footer__bottom"><span>© 2026 SecondEgo · MIT License</span><span>LOCAL FIRST / EVIDENCE REQUIRED</span></div></div></footer>;
}

export default function App() {
  const { path, navigate } = useRoute();
  const page = useMemo(() => { if (path === "/docs") return <DocsPage navigate={navigate} />; if (path === "/download" || path === "/downloads") return <DownloadPage />; if (path === "/about") return <AboutPage navigate={navigate} />; if (path === "/terms" || path === "/terms-and-conditions") return <TermsPage />; return <HomePage navigate={navigate} />; }, [navigate, path]);
  useEffect(() => { applyPageMeta(path); }, [path]);
  return <div className="site-shell"><Navigation navigate={navigate} /><main>{page}</main><Footer navigate={navigate} /></div>;
}
