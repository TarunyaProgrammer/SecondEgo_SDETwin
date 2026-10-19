import { useMemo, useState } from "react";
import type { KeyboardEvent, ReactNode } from "react";
import { Reveal } from "@/components/Reveal";
import { ScrollExperience } from "@/components/ScrollExperience";
import "./styles.css";

const repository = "https://github.com/TarunyaProgrammer/SecondEgo_SDETwin";

const providers = [
  { id: "openrouter", name: "OpenRouter", mark: "OR", tone: "violet", detail: "One key for a routed model catalog. Choose the model explicitly; the runtime still validates the response contract." },
  { id: "anthropic", name: "Anthropic", mark: "A", tone: "sand", detail: "Native Messages adapter with strict structured output normalization and provider-aware retry classification." },
  { id: "openai", name: "OpenAI", mark: "O", tone: "mint", detail: "First-class Responses/Chat adapter with streaming, cancellation, and a stable internal proposal type." },
  { id: "gemini", name: "Gemini", mark: "G", tone: "blue", detail: "Google GenAI adapter with schema-constrained planning and a key stored in the OS credential vault." },
  { id: "deepseek", name: "DeepSeek", mark: "D", tone: "orange", detail: "OpenAI-compatible adapter and the current evaluator default. Legacy AI_API_KEY remains a migration alias only." },
] as const;

const installModes = {
  mac: {
    label: "macOS app",
    title: "Download once. Keep the engine local.",
    body: "The macOS DMG installs SecondEgo into Applications, runs a local engine, and opens guided provider setup on first launch.",
    code: ["open SecondEgo-<version>-arm64.dmg", "drag SecondEgo → Applications", "open -a SecondEgo", "shasum -a 256 -c SHA256SUMS.txt"],
    note: "Current releases publish unsigned DMG candidates with checksums; signing, notarization, and automatic updates are the next release gate.",
  },
  terminal: {
    label: "Terminal",
    title: "A serious CLI for serious runs.",
    body: "Use the same engine from scripts, a shell, or CI. The terminal exposes phases, evidence, diffs, and a machine-readable JSON mode.",
    code: ["git clone github.com/TarunyaProgrammer/SecondEgo_SDETwin", "cd SecondEgo_SDETwin && make setup", "printf '%s\\n' \"$OPENAI_API_KEY\" | secondego provider add openai --key-stdin", "secondego provider test"],
    note: "Native profiles use macOS Keychain, Windows Credential Manager, or Linux Secret Service.",
  },
  desktop: {
    label: "Desktop",
    title: "Open the command surface, not a toy dashboard.",
    body: "The desktop is a presentation and control layer over the local engine: provider profiles, run history, diff review, and explicit failure states.",
    code: ["Settings → Providers", "Save encrypted profile → Test", "New task → Review diff → Apply"],
    note: "The notch stays a glanceable entry point; full run inspection lives in the expanded workspace.",
  },
} as const;

type ProviderId = (typeof providers)[number]["id"];
type InstallMode = keyof typeof installModes;

function ArrowIcon({ down = false }: { down?: boolean }) {
  return <svg className="icon" viewBox="0 0 16 16" aria-hidden="true">{down ? <path d="M8 2.5v10m-4-4 4 4 4-4" /> : <path d="M3.5 12.5 12.5 3.5m-5.5 0h5.5V9" />}</svg>;
}

function CheckIcon() {
  return <svg className="icon" viewBox="0 0 16 16" aria-hidden="true"><path d="m3.5 8.5 3 3 6-7" /></svg>;
}

function MenuIcon({ open }: { open: boolean }) {
  return <svg className="icon" viewBox="0 0 20 20" aria-hidden="true"><path d={open ? "M4 4 16 16M16 4 4 16" : "M3 5h14M3 10h14M3 15h14"} /></svg>;
}

function BranchIcon() {
  return <svg className="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M6 4v11a4 4 0 0 0 4 4h8m-4-4 4 4-4 4M6 4h.01" /></svg>;
}

function Logo() {
  return <a className="production-logo" href="#top" aria-label="SecondEgo home"><span className="production-logo__glyph" aria-hidden="true"><i /><i /><i /></span><span>second<span>ego</span></span></a>;
}

function SectionIntro({ eyebrow, title, body, dark = false }: { eyebrow: string; title: ReactNode; body: string; dark?: boolean }) {
  return <Reveal className={`production-intro${dark ? " production-intro--dark" : ""}`}><p className="production-eyebrow"><span />{eyebrow}</p><h2>{title}</h2><p className="production-intro__body">{body}</p></Reveal>;
}

function Hero() {
  return <section className="production-hero" id="top" data-tone="ink"><div className="production-hero__grid" aria-hidden="true" /><div className="production-hero__glow production-hero__glow--one" aria-hidden="true" /><div className="production-hero__glow production-hero__glow--two" aria-hidden="true" /><div className="production-wrap production-hero__inner"><Reveal className="production-hero__copy"><p className="production-eyebrow production-eyebrow--bright"><span />LOCAL CODING AGENT / BUILT FOR PROOF</p><h1>Your second self<br /><em>ships with evidence.</em></h1><p className="production-hero__lead">SecondEgo turns an issue into a bounded attempt: retrieve the right context, propose a plan, edit in isolation, run real checks, and show the diff before anything reaches your repository.</p><div className="production-hero__actions"><a className="production-button production-button--signal" href="#install">Get SecondEgo <ArrowIcon /></a><a className="production-button production-button--ghost" href="#architecture">See how it works <ArrowIcon down /></a></div><div className="production-hero__proof"><span><i /> 5 provider keys</span><span><i /> local-first</span><span><i /> verified diffs</span></div></Reveal><Reveal className="production-hero__console" delay={0.12}><div className="console-top"><span className="console-lights"><i /><i /><i /></span><span>secondego / run-042</span><span className="console-live"><i /> LIVE</span></div><div className="console-body"><div className="console-rail"><span>01</span><span>02</span><span>03</span><span>04</span><span>05</span><span>06</span></div><div className="console-content"><p><b className="console-orange">PLAN</b><span>provider proposal normalized</span><strong><CheckIcon /></strong></p><p><b className="console-violet">POLICY</b><span>6 actions validated</span><strong><CheckIcon /></strong></p><p><b className="console-mint">WORKTREE</b><span>detached attempt created</span><strong><CheckIcon /></strong></p><p><b className="console-blue">VERIFY</b><span>pytest -q</span><strong>running</strong></p><div className="console-diff"><div><span className="diff-minus">−</span><code>return items[offset:end]</code></div><div><span className="diff-plus">+</span><code>return items[start:end]</code></div></div></div></div><div className="console-bottom"><span>DECISION OWNER</span><strong>RUNTIME</strong><span>04:18</span></div></Reveal><a className="production-scroll-cue" href="#providers"><span />scroll to inspect the stack <ArrowIcon down /></a></div></section>;
}

function moveTab(event: KeyboardEvent<HTMLButtonElement>, index: number, count: number, idPrefix: string, select: (index: number) => void) {
  const direction = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 0;
  if (!direction) return;
  event.preventDefault();
  const next = (index + direction + count) % count;
  select(next);
  requestAnimationFrame(() => document.getElementById(`${idPrefix}-${next}`)?.focus());
}

function ProviderSection() {
  const [selected, setSelected] = useState<ProviderId>("openrouter");
  const active = useMemo(() => providers.find((provider) => provider.id === selected) ?? providers[0], [selected]);
  const selectProvider = (index: number) => setSelected(providers[index].id);
  return <section className="provider-section production-section" id="providers" data-tone="paper"><div className="production-wrap"><SectionIntro eyebrow="PROVIDER-NEUTRAL BY DESIGN" title={<>Bring the key<br /><span>you already trust.</span></>} body="DeepSeek, Gemini, OpenAI, Anthropic, and OpenRouter are five independent profiles—not five code paths leaking into orchestration. Pick a provider per workspace, test it locally, and keep the secret in your operating system's credential vault." /><Reveal className="provider-layout" delay={0.08}><div className="provider-list" role="tablist" aria-label="Supported model providers">{providers.map((item, index) => <button key={item.id} id={`provider-tab-${index}`} type="button" role="tab" aria-selected={selected === item.id} aria-controls="provider-panel" tabIndex={selected === item.id ? 0 : -1} className={`provider-card provider-card--${item.tone}${selected === item.id ? " is-selected" : ""}`} onClick={() => setSelected(item.id)} onKeyDown={(event) => moveTab(event, index, providers.length, "provider-tab", selectProvider)}><span className="provider-card__mark">{item.mark}</span><span><strong>{item.name}</strong><small>{item.id === "openrouter" ? "routed catalog" : "native adapter"}</small></span><span className="provider-card__arrow"><ArrowIcon /></span></button>)}</div><div className="provider-detail" id="provider-panel" role="tabpanel" aria-labelledby={`provider-tab-${providers.findIndex((provider) => provider.id === active.id)}`}><div className="provider-detail__top"><span className={`provider-detail__mark provider-detail__mark--${active.tone}`}>{active.mark}</span><span className="production-status"><i /> READY TO CONFIGURE</span></div><p className="production-eyebrow">{active.name} / PROFILE</p><h3>{active.name}<span>.</span></h3><p>{active.detail}</p><div className="provider-detail__rows"><div><span>Secret storage</span><strong>OS keychain</strong></div><div><span>Runtime surface</span><strong>normalized proposal</strong></div><div><span>Failure modes</span><strong>classified + bounded</strong></div></div><a className="text-link" href="#docs">Read the provider contract <ArrowIcon /></a></div></Reveal></div></section>;
}

function InstallSection() {
  const [mode, setMode] = useState<InstallMode>("mac");
  const active = installModes[mode];
  const modes = Object.keys(installModes) as InstallMode[];
  const selectMode = (index: number) => setMode(modes[index]);
  return <section className="install-section production-section" id="install" data-tone="ink"><div className="production-wrap"><SectionIntro dark eyebrow="INSTALL / CONFIGURE / RUN" title={<>One product.<br /><span>Three entry points.</span></>} body="Download the local app when you want a command surface. Use the terminal when you want speed or automation. Both talk to the same local engine, provider profiles, and evidence store." /><Reveal className="install-panel" delay={0.08}><div className="install-tabs" role="tablist" aria-label="Installation modes">{modes.map((key, index) => <button key={key} id={`install-tab-${index}`} type="button" role="tab" aria-selected={mode === key} aria-controls="install-panel" tabIndex={mode === key ? 0 : -1} className={mode === key ? "is-active" : ""} onClick={() => setMode(key)} onKeyDown={(event) => moveTab(event, index, modes.length, "install-tab", selectMode)}>{installModes[key].label}</button>)}</div><div className="install-panel__content" id="install-panel" role="tabpanel" aria-labelledby={`install-tab-${modes.indexOf(mode)}`}><div><p className="production-eyebrow production-eyebrow--bright">PATH / {mode.toUpperCase()}</p><h3>{active.title}</h3><p>{active.body}</p><a className="production-button production-button--signal" href={mode === "mac" ? `${repository}/releases` : repository} target="_blank" rel="noreferrer">{mode === "mac" ? "View release artifacts" : "Open setup docs"} <ArrowIcon /></a></div><div className="install-code"><div className="install-code__head"><span>FIRST RUN</span><span>SAFE BY DEFAULT</span></div>{active.code.map((line) => <code key={line}><b>$</b> {line}</code>)}<p><i />{active.note}</p></div></div></Reveal></div></section>;
}

function ArchitectureSection() {
  return <section className="architecture-section production-section" id="architecture" data-tone="paper"><div className="production-wrap"><SectionIntro eyebrow="HLD / LOCAL CONTROL PLANE" title={<>The model can reason.<br /><span>The engine owns authority.</span></>} body="The desktop and terminal are observers and control surfaces. The Rust runtime remains the only component that can validate actions, mutate an isolated worktree, run verification, transfer a diff, or declare a terminal outcome." /><Reveal className="hld-diagram" delay={0.08}><div className="hld-node hld-node--surface"><span>01</span><strong>Terminal + Desktop</strong><small>command surface / observer</small></div><div className="hld-arrow"><ArrowIcon /></div><div className="hld-node hld-node--boundary"><span>02</span><strong>Versioned local API</strong><small>token-authenticated loopback</small></div><div className="hld-arrow"><ArrowIcon /></div><div className="hld-node hld-node--engine"><span>03</span><strong>Rust runtime</strong><small>state machine / policy / proof</small></div><div className="hld-branch"><div className="hld-node hld-node--provider"><span>04A</span><strong>Provider registry</strong><small>five adapters / key handles</small></div><div className="hld-node hld-node--tools"><span>04B</span><strong>Bounded tools</strong><small>paths / commands / budgets</small></div><div className="hld-node hld-node--evidence"><span>04C</span><strong>Evidence store</strong><small>SQLite / events / diff</small></div></div></Reveal><div className="architecture-lower"><Reveal className="ll-panel" delay={0.12}><p className="production-eyebrow">LLD / KEY LIFECYCLE</p><h3>A key enters once.<br /><span>It never becomes UI state.</span></h3><ol><li><b>01</b><span>Provider profile validates the input and writes the secret to the OS keychain.</span></li><li><b>02</b><span>SQLite stores only provider id, model id, reference, and validation status.</span></li><li><b>03</b><span>Electron main resolves the selected key and sends it once over authenticated loopback to the Rust gateway, which keeps it in memory only.</span></li><li><b>04</b><span>Events, subprocesses, diffs, and logs receive redacted metadata only.</span></li></ol><a className="text-link" href={`${repository}/blob/main/docs/diagrams/provider-key-lld.mmd`} target="_blank" rel="noreferrer">Open the LLD source <ArrowIcon /></a></Reveal><Reveal className="state-panel" delay={0.16}><div className="state-panel__top"><span>RUN STATE</span><span className="production-status"><i /> OBSERVABLE</span></div><div className="state-track"><span className="is-done">UNDERSTAND</span><span className="is-done">EXPLORE</span><span className="is-done">PLAN</span><span className="is-active">EXECUTE</span><span>VERIFY</span><span>TERMINAL</span></div><div className="state-evidence"><div><span>ACTIVE TOOL</span><strong>git_diff</strong></div><div><span>AFFECTED PATHS</span><strong>2 files</strong></div><div><span>AUTHORITY</span><strong>runtime</strong></div></div><div className="state-log"><code><b>14:18:03</b> proposal accepted</code><code><b>14:18:04</b> worktree detached</code><code><b>14:18:05</b> diff requested</code></div></Reveal></div></div></section>;
}

function DocsSection() {
  const docs = [{ title: "Production architecture", body: "Provider registry, key lifecycle, desktop boundaries, packaging, and the migration sequence.", href: `${repository}/blob/main/docs/PRODUCTION_ARCHITECTURE.md`, tag: "HLD + LLD" }, { title: "Run the engine", body: "Headless setup, terminal commands, provider selection, fixture rehearsal, and verification.", href: `${repository}/blob/main/README.md`, tag: "QUICK START" }, { title: "Security model", body: "Workspace confinement, subprocess trust boundaries, secrets, prompt injection, and reporting.", href: `${repository}/blob/main/SECURITY.md`, tag: "SECURITY" }];
  return <section className="docs-section production-section" id="docs" data-tone="ink"><div className="production-wrap"><SectionIntro dark eyebrow="DOCUMENTATION / NO HAND-WAVING" title={<>Everything you need<br /><span>before the first run.</span></>} body="The docs are organized around decisions a new user actually needs to make: which surface to use, where keys live, what a run can touch, and what the app can honestly prove." /><div className="docs-grid">{docs.map((doc, index) => <Reveal key={doc.title} className={`doc-card doc-card--${index + 1}`} delay={index * 0.06}><span className="doc-card__tag">{doc.tag}</span><h3>{doc.title}</h3><p>{doc.body}</p><a className="text-link" href={doc.href} target="_blank" rel="noreferrer">Read the doc <ArrowIcon /></a></Reveal>)}</div><Reveal className="docs-callout" delay={0.16}><span className="docs-callout__mark"><BranchIcon /></span><div><p className="production-eyebrow production-eyebrow--bright">THE PRODUCTION PROMISE</p><h3>Local by default. Explicit at every boundary.</h3><p>SecondEgo is not a cloud service wearing a local UI. It is a local execution system that can use your provider keys without handing your repository to a hidden orchestration layer.</p></div><a className="production-button production-button--ghost" href={repository} target="_blank" rel="noreferrer">Inspect the repository <ArrowIcon /></a></Reveal></div></section>;
}

function Footer() {
  return <footer className="production-footer"><div className="production-wrap production-footer__inner"><div><Logo /><p>A local coding agent built around bounded execution and verified change.</p></div><div className="production-footer__links"><a href="#providers">Providers</a><a href="#install">Install</a><a href="#architecture">Architecture</a><a href="#docs">Docs</a><a href={repository} target="_blank" rel="noreferrer">GitHub <ArrowIcon /></a></div><div className="production-footer__bottom"><span>© 2026 SecondEgo · MIT License</span><span>RUST ENGINE / LOCAL FIRST / EVIDENCE REQUIRED</span></div></div></footer>;
}

export default function App() {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const closeMobileMenu = () => setMobileMenuOpen(false);
  return <ScrollExperience><header className={`production-nav${mobileMenuOpen ? " is-menu-open" : ""}`}><div className="production-wrap production-nav__inner"><Logo /><nav id="primary-navigation" aria-label="Primary navigation"><a href="#providers" onClick={closeMobileMenu}>Providers</a><a href="#install" onClick={closeMobileMenu}>Install</a><a href="#architecture" onClick={closeMobileMenu}>Architecture</a><a href="#docs" onClick={closeMobileMenu}>Docs</a></nav><div className="production-nav__actions"><a className="production-nav__cta" href="#install" onClick={closeMobileMenu}>Download <ArrowIcon /></a><button className="production-nav__menu" type="button" aria-label={mobileMenuOpen ? "Close navigation menu" : "Open navigation menu"} aria-expanded={mobileMenuOpen} aria-controls="primary-navigation" onClick={() => setMobileMenuOpen((open) => !open)}><MenuIcon open={mobileMenuOpen} /></button></div></div></header><main><Hero /><div className="production-marquee" aria-hidden="true"><span>LOCAL-FIRST</span><i /><span>BOUNDED EXECUTION</span><i /><span>FIVE PROVIDERS</span><i /><span>VERIFIED DIFFS</span><i /><span>LOCAL-FIRST</span><i /><span>BOUNDED EXECUTION</span><i /></div><ProviderSection /><InstallSection /><ArchitectureSection /><DocsSection /></main><Footer /></ScrollExperience>;
}
