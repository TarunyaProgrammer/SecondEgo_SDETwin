"use client";

import { useState } from "react";
import { Reveal } from "@/components/Reveal";

const repository = "https://github.com/TarunyaProgrammer/SecondEgo_SDETwin";

type Layer = {
  id: string;
  number: string;
  label: string;
  role: string;
  tech: string;
  source: string;
  sourceType: "tree" | "blob";
  summary: string;
  details: string[];
};

const layers: Layer[] = [
  {
    id: "observer",
    number: "01",
    label: "Observer shell",
    role: "Optional presentation layer",
    tech: "Electron · React · Vite",
    source: "apps/desktop",
    sourceType: "tree",
    summary: "Shows engine events without owning agent decisions.",
    details: [
      "The desktop surface is opt-in; headless terminal execution remains the judge-safe default.",
      "The renderer displays serialized run state, evidence, voice status, and gesture status. It does not plan, authorize tools, verify work, or decide termination.",
    ],
  },
  {
    id: "gateway",
    number: "02",
    label: "Local boundary",
    role: "Loopback observer gateway",
    tech: "Rust · JSON · 127.0.0.1",
    source: "engine-rs/crates/secondego-gateway",
    sourceType: "tree",
    summary: "Keeps the UI-to-engine boundary local and explicit.",
    details: [
      "The gateway exposes serialized run events and state over loopback for the optional observer.",
      "The boundary is not a second orchestration engine: the Rust runtime remains the source of truth for planning, permissions, tools, verification, recovery, and termination.",
    ],
  },
  {
    id: "runtime",
    number: "03",
    label: "Rust runtime",
    role: "Default execution path",
    tech: "Rust · Cargo workspace · serde",
    source: "engine-rs/crates/secondego-runtime",
    sourceType: "tree",
    summary: "Runs the explicit state machine from issue to terminal evidence.",
    details: [
      "The workspace separates contracts, orchestration, repository intelligence, context, model access, tools, verification, storage, gateway, and CLI concerns.",
      "The runtime owns budgets, state transitions, attempt isolation, structured actions, evidence, bounded recovery, and terminal reports.",
    ],
  },
  {
    id: "intelligence",
    number: "04",
    label: "Repository intelligence",
    role: "Context before action",
    tech: "Tree-sitter · Python AST · lexical fallback",
    source: "engine-rs/crates/secondego-repository",
    sourceType: "tree",
    summary: "Builds local, rebuildable metadata and retrieves focused evidence.",
    details: [
      "Rust indexing covers JavaScript, TypeScript, Python, and Rust with Tree-sitter grammars; the compatibility path adds Python AST indexing and lexical fallback.",
      "Context assembly is source-linked and budgeted. The system stores derived metadata, not a remote graph database or an unbounded transcript.",
    ],
  },
  {
    id: "tools",
    number: "05",
    label: "Bounded tools",
    role: "Small execution surface",
    tech: "Filesystem · search · argv commands · Git",
    source: "engine-rs/crates/secondego-tools",
    sourceType: "tree",
    summary: "Makes every side effect pass path, command, timeout, and output policy.",
    details: [
      "Workspace paths are canonicalized and prevented from escaping the active workspace.",
      "Commands use argv arrays with shell=false, an allowlist, a 120-second timeout ceiling, and capped captured output. Git operations stay inside the attempt boundary.",
    ],
  },
  {
    id: "evidence",
    number: "06",
    label: "Verification + storage",
    role: "Evidence is the finish line",
    tech: "SQLite · Git worktrees · exit codes",
    source: "engine-rs/crates/secondego-verification",
    sourceType: "tree",
    summary: "Only verified changes can become a successful run.",
    details: [
      "Declared tests, builds, and lint commands run sequentially. Exit codes, bounded output, changed paths, Git evidence, and termination reasons are recorded.",
      "Failed attempts are discarded before a bounded repair attempt. SQLite stores local run, event, and evidence state; source blobs are not copied into the database.",
    ],
  },
];

const trace = [
  ["01", "issue", "accept task + criteria"],
  ["02", "understand", "shape the contract"],
  ["03", "explore", "index + retrieve"],
  ["04", "plan", "structured actions"],
  ["05", "execute", "fresh Git worktree"],
  ["06", "verify", "run declared checks"],
  ["07", "diagnose", "classify failure"],
  ["08", "recover", "one bounded retry"],
  ["09", "terminal", "complete / failed / blocked"],
] as const;

type FeatureStatus = "implemented" | "optional" | "not yet";

const features: { title: string; description: string; source: string; status: FeatureStatus }[] = [
  { title: "Explicit lifecycle", description: "State machine with initialize, understand, explore, plan, execute, verify, diagnose, recover, and terminal outcomes.", source: "src/SecondEgo/core/state_machine.py", status: "implemented" },
  { title: "Provider boundary", description: "Structured provider interface with DeepSeek as the evaluator default and Gemini as an explicit optional adapter.", source: "engine-rs/crates/secondego-model", status: "implemented" },
  { title: "Source-linked context", description: "Bounded retrieval, context budgets, evidence references, and compact state snapshots without raw transcripts.", source: "engine-rs/crates/secondego-context", status: "implemented" },
  { title: "Workspace safety", description: "Canonical path checks, command allowlist, shell=false, timeout caps, and bounded output capture.", source: "src/SecondEgo/tools/policy.py", status: "implemented" },
  { title: "Isolated attempts", description: "Clean Git preflight, detached worktree execution, discard-on-failure, and verified diff transfer.", source: "engine-rs/crates/secondego-runtime", status: "implemented" },
  { title: "Local evidence store", description: "SQLite persistence for run state, events, and evidence with crash-safe cleanup paths.", source: "engine-rs/crates/secondego-storage", status: "implemented" },
  { title: "Observer surfaces", description: "Terminal first, optional browser observer, and optional Electron/React shell consuming engine events.", source: "README.md · apps/desktop/README.md", status: "optional" },
  { title: "Voice + gestures", description: "Optional Gemini TTS narration and camera gesture shortcuts for the desktop/notch surface; never part of coding authority.", source: "engine-rs/crates/secondego-runtime/src/voice.rs · src/SecondEgo/gesture", status: "optional" },
  { title: "Desktop packaging", description: "The observer exists, but a polished installer and release packaging are deliberately outside the current shipped scope.", source: "docs/context/CONTEXT.md", status: "not yet" },
  { title: "Broader language parsing", description: "The current repository intelligence is intentionally bounded; broad multi-language parsing is future work.", source: "docs/context/CONTEXT.md", status: "not yet" },
  { title: "Benchmark + telemetry breadth", description: "The evaluation path is present, while broader benchmark coverage and full telemetry/report schemas remain future work.", source: "docs/context/CONTEXT.md", status: "not yet" },
];

const stackGroups = [
  { label: "Engine", items: ["Rust", "Cargo workspace", "serde / serde_json", "reqwest + rustls", "uuid + chrono"] },
  { label: "Repository + state", items: ["Tree-sitter", "Python AST", "SQLite / rusqlite", "Git worktrees", "bounded context"] },
  { label: "Compatibility path", items: ["Python 3.12+", "pytest", "DeepSeek adapter", "optional Gemini adapter", "CLI / TUI"] },
  { label: "Observer + site", items: ["Electron", "React", "Vite", "TypeScript", "Framer Motion"] },
];

function sourceHref(source: string, sourceType: "tree" | "blob") {
  const path = source.split(" · ")[0];
  return `${repository}/${sourceType}/main/${path}`;
}

function featureSourceType(source: string): "tree" | "blob" {
  return source.split(" · ")[0].includes(".") ? "blob" : "tree";
}

export function JudgesMap() {
  const [activeId, setActiveId] = useState(layers[2].id);
  const active = layers.find((layer) => layer.id === activeId) ?? layers[2];

  const moveLayer = (direction: 1 | -1) => {
    const index = layers.findIndex((layer) => layer.id === activeId);
    const next = (index + direction + layers.length) % layers.length;
    setActiveId(layers[next].id);
  };

  return (
    <section className="judges-section section-grid" id="judges-map" data-tone="paper">
      <div className="wrap">
        <Reveal className="section-heading section-heading--split judges-section__heading">
          <div>
            <p className="section-label">JUDGE’S MAP / READ THE REPOSITORY</p>
            <h2>One system.<br /><span className="text-accent">Every boundary visible.</span></h2>
          </div>
          <p className="section-heading__aside">This is the compressed README: click through the layers, follow one run, inspect the evidence paths, then separate what ships today from what is intentionally still open.</p>
        </Reveal>

        <div className="judges-architecture">
          <div className="judges-architecture__rail" role="tablist" aria-label="SecondEgo architecture layers">
            {layers.map((layer) => (
              <button
                className={`architecture-tab${active.id === layer.id ? " is-active" : ""}`}
                key={layer.id}
                id={`layer-tab-${layer.id}`}
                role="tab"
                aria-selected={active.id === layer.id}
                aria-controls={`layer-panel-${layer.id}`}
                tabIndex={active.id === layer.id ? 0 : -1}
                onClick={() => setActiveId(layer.id)}
                onKeyDown={(event) => {
                  if (event.key === "ArrowDown" || event.key === "ArrowRight") { event.preventDefault(); moveLayer(1); }
                  if (event.key === "ArrowUp" || event.key === "ArrowLeft") { event.preventDefault(); moveLayer(-1); }
                  if (event.key === "Home") { event.preventDefault(); setActiveId(layers[0].id); }
                  if (event.key === "End") { event.preventDefault(); setActiveId(layers[layers.length - 1].id); }
                }}
              >
                <span className="architecture-tab__number">{layer.number}</span>
                <span><strong>{layer.label}</strong><small>{layer.role}</small></span>
                <span className="architecture-tab__arrow" aria-hidden="true">↗</span>
              </button>
            ))}
          </div>
          <div className="judges-architecture__panel" id={`layer-panel-${active.id}`} role="tabpanel" aria-labelledby={`layer-tab-${active.id}`}>
            <div className="architecture-panel__top">
              <div><p className="mono">LAYER / {active.number}</p><h3>{active.label}<span>.</span></h3><p>{active.summary}</p></div>
              <span className="architecture-panel__signal"><i /> source-backed</span>
            </div>
            <div className="architecture-panel__details">
              {active.details.map((detail) => <p key={detail}><span aria-hidden="true">↳</span>{detail}</p>)}
            </div>
            <div className="architecture-panel__source">
              <span className="mono">TECH / {active.tech}</span>
              <a href={sourceHref(active.source, active.sourceType)} target="_blank" rel="noreferrer">{active.source}<span aria-hidden="true">↗</span></a>
            </div>
          </div>
        </div>

        <Reveal className="judges-trace" delay={0.08}>
          <div className="judges-subheading"><div><p className="section-label">TRACE THE CONTROL FLOW</p><h3>A run is a chain of decisions,<br /><span>not a chat transcript.</span></h3></div><p>Recovery is conditional. Completion is earned by command results and Git evidence. The model proposes; the runtime validates.</p></div>
          <ol className="judges-trace__list">
            {trace.map(([number, label, detail], index) => <li key={label} className={label === "terminal" ? "is-terminal" : ""}><span className="judges-trace__number">{number}</span><strong>{label}</strong><small>{detail}</small>{index < trace.length - 1 && <span className="judges-trace__connector" aria-hidden="true">→</span>}</li>)}
          </ol>
        </Reveal>

        <Reveal className="judges-ledger" delay={0.1}>
          <div className="judges-subheading"><div><p className="section-label">FEATURE LEDGER</p><h3>What exists, where it lives,<br /><span>and what is still open.</span></h3></div><div className="status-legend" aria-label="Feature status legend"><span><i className="status-dot status-dot--implemented" /> implemented</span><span><i className="status-dot status-dot--optional" /> optional</span><span><i className="status-dot status-dot--future" /> not yet</span></div></div>
          <div className="feature-ledger" role="list">
            {features.map((feature) => <details className="feature-row" key={feature.title} open={feature.status === "implemented" && feature.title === "Explicit lifecycle"}>
              <summary><span className={`status-dot status-dot--${feature.status.replace(" ", "-")}`} /><strong>{feature.title}</strong><span className="feature-row__summary">{feature.description}</span><span className="feature-row__toggle" aria-hidden="true">+</span></summary>
              <div className="feature-row__body"><p>{feature.description}</p><a href={sourceHref(feature.source, featureSourceType(feature.source))} target="_blank" rel="noreferrer"><span className="mono">EVIDENCE</span>{feature.source}<span aria-hidden="true">↗</span></a></div>
            </details>)}
          </div>
        </Reveal>

        <Reveal className="judges-stack" delay={0.12}>
          <div className="judges-subheading"><div><p className="section-label">TEXT TECH INVENTORY</p><h3>The stack behind<br /><span>the behavior.</span></h3></div><a className="text-link" href={`${repository}/blob/main/README.md`} target="_blank" rel="noreferrer">Read the full README <span aria-hidden="true">↗</span></a></div>
          <div className="stack-inventory">{stackGroups.map((group) => <div className="stack-group" key={group.label}><p className="mono">{group.label}</p><ul>{group.items.map((item) => <li key={item}>{item}</li>)}</ul></div>)}</div>
        </Reveal>

        <Reveal className="judges-checklist" delay={0.14}>
          <div><p className="section-label">REPRODUCIBLE CHECK</p><h3>Give the judges<br /><span>something to run.</span></h3><p>From a clean checkout, the documented interface exposes setup, run, test, discovery, and judge paths. Secrets stay in the environment; credentials are not part of the repository.</p></div>
          <div className="judge-commands"><code><span>$</span> make setup</code><code><span>$</span> make discover</code><code><span>$</span> make judge</code><code><span>$</span> make rust-test</code><a href={`${repository}/blob/main/README.md#quick-start`} target="_blank" rel="noreferrer">Open setup + evaluation docs <span aria-hidden="true">↗</span></a></div>
        </Reveal>
      </div>
    </section>
  );
}
