"use client";

import { useState } from "react";
import { Reveal } from "@/components/Reveal";
import { PipelineDiagram } from "@/components/PipelineDiagram";
import type { PipelineNode } from "@/components/PipelineDiagram";
import { PhaseTabs } from "@/components/PhaseTabs";
import type { PhaseTabItem } from "@/components/PhaseTabs";
import { CodeBlock } from "@/components/CodeBlock";

type Phase = PhaseTabItem & { description: string; nodes: PipelineNode[]; codeTitle: string; codeLanguage: string; codeLines: string[]; footnote: string };

const fullRun: PipelineNode[] = [
  { label: "ISSUE", state: "complete" },
  { label: "INDEX", state: "complete" },
  { label: "RETRIEVE", state: "complete" },
  { label: "VALIDATE", state: "complete" },
  { label: "ISOLATE", state: "complete" },
  { label: "EXECUTE", state: "active" },
  { label: "VERIFY" },
  { label: "DIAGNOSE" },
  { label: "REPAIR" },
  { label: "COMPLETE" },
];

const phases: Phase[] = [
  {
    id: "index", label: "Index", summary: "Map the target", description: "A bounded scan builds local repository metadata before planning. Python receives AST-aware indexing; other languages use structural and lexical fallback.",
    nodes: [{ label: "Target repo", detail: "separate checkout", state: "complete" }, { label: "Scan", detail: "bounded file walk", state: "complete" }, { label: "Index", detail: "symbols + tests", state: "active" }, { label: "Retrieve", detail: "ranked evidence" }],
    codeTitle: "repository.snapshot", codeLanguage: "INDEX", codeLines: ["files: bounded manifest", "symbols: Python AST", "tests: discovered + linked", "evidence: ranked, source-linked"], footnote: "Derived metadata is local and rebuildable.",
  },
  {
    id: "plan", label: "Plan", summary: "Propose actions", description: "The provider receives a source-linked evidence packet and returns one structured action list. The runtime validates its shape and policy before dispatch.",
    nodes: [{ label: "Retrieve", detail: "top evidence", state: "complete" }, { label: "Context", detail: "24K estimate", state: "complete" }, { label: "Model", detail: "structured proposal", state: "active" }, { label: "Validate", detail: "schema + policy" }],
    codeTitle: "plan.contract", codeLanguage: "JSON SHAPE", codeLines: ["submit_plan({", "  actions: [...],", "  verification_commands: [[...]]", "})", "// runtime validates before execution"], footnote: "The model proposes. Deterministic policy decides.",
  },
  {
    id: "isolate", label: "Isolate", summary: "Start from clean Git", description: "Production attempts require a clean Git repository with an initial commit. Each attempt runs in a detached worktree, leaving the target working tree untouched until a verified diff is transferred.",
    nodes: [{ label: "Preflight", detail: "clean Git + HEAD", state: "complete" }, { label: "Detach", detail: "fresh worktree", state: "active" }, { label: "Attempt", detail: "target unchanged" }],
    codeTitle: "attempt.boundary", codeLanguage: "GIT", codeLines: ["preflight(target)", "worktree = detached(HEAD)", "execute(cwd=worktree)", "// never mutate target directly"], footnote: "Dirty or non-Git targets are blocked.",
  },
  {
    id: "execute", label: "Execute", summary: "Dispatch safe actions", description: "The router checks action shape, paths, budgets, and executable policy. Commands use argv arrays and shell=false, with bounded timeout and captured output.",
    nodes: [{ label: "Proposal", detail: "action + args", state: "complete" }, { label: "Router", detail: "policy checks", state: "complete" }, { label: "Tools", detail: "bounded calls", state: "active" }, { label: "Evidence", detail: "results recorded" }],
    codeTitle: "command.policy", codeLanguage: "RUNTIME", codeLines: ["allow = {git, npm, pnpm,", "         pytest, python, python3,", "         ruff, uv}", "shell = false", "timeout <= 120s"], footnote: "Paths stay inside the attempt workspace.",
  },
  {
    id: "verify", label: "Verify", summary: "Require passing evidence", description: "Declared test, build, or lint commands run sequentially. Completion requires passing command evidence; narration cannot mark a run complete.",
    nodes: [{ label: "Actions", detail: "applied in worktree", state: "complete" }, { label: "Commands", detail: "tests / build / lint", state: "active" }, { label: "Evidence", detail: "bounded output" }, { label: "Decision", detail: "runtime status" }],
    codeTitle: "verification.result", codeLanguage: "EVIDENCE", codeLines: ["argv: [\"pytest\", \"-q\"]", "exit_code: checked", "output: capped", "complete: only after pass"], footnote: "A passing diff is transferred within configured size limits.",
  },
  {
    id: "recover", label: "Recover", summary: "Retry once, from clean state", description: "On failure, the attempt is discarded. Failure-specific evidence informs one repair plan, which runs in a fresh worktree and is verified again.",
    nodes: [{ label: "Failure", detail: "classified output", state: "warning" }, { label: "Diagnose", detail: "refresh retrieval", state: "active" }, { label: "Repair", detail: "one plan", state: "active" }, { label: "Fresh attempt", detail: "clean baseline" }],
    codeTitle: "recovery.policy", codeLanguage: "RETRY", codeLines: ["discard(failed_worktree)", "context = retrieve(failure)", "repair = request_one_plan(context)", "retry_in(fresh_worktree)", "verify_again()"], footnote: "Recovery is bounded to a single provider-backed retry.",
  },
];

export function LifecycleTabs() {
  const [activeId, setActiveId] = useState(phases[0].id);
  const active = phases.find((phase) => phase.id === activeId) ?? phases[0];

  return (
    <section className="lifecycle-section section-grid" id="lifecycle" data-tone="ink">
      <div className="wrap">
        <Reveal className="section-heading section-heading--split">
          <div><p className="section-label">A RUN HAS A SHAPE</p><h2>From issue to<br /><span className="text-accent">verified change.</span></h2></div>
          <p className="section-heading__aside">Six visible phases. One bounded attempt at a time. Follow the evidence from repository scan through verification and recovery.</p>
        </Reveal>
        <div className="pipeline-overview">
          <div className="pipeline-overview__label"><span className="mono">RUN SEQUENCE</span><span>Diagnosis and repair run only after a failed verification.</span></div>
          <PipelineDiagram nodes={fullRun} label="Issue, index, retrieve, validate, isolate, execute, verify, diagnose, repair, complete" className="pipeline-diagram--full" />
        </div>
        <div className="lifecycle-layout">
          <PhaseTabs items={phases} activeId={activeId} onChange={setActiveId} />
          <div className="phase-panel" id={`panel-${active.id}`} role="tabpanel" aria-labelledby={`tab-${active.id}`} key={active.id}>
            <div className="phase-panel__copy"><p className="phase-panel__kicker mono">PHASE / {String(phases.findIndex((phase) => phase.id === active.id) + 1).padStart(2, "0")}</p><h3>{active.label}<span>.</span></h3><p>{active.description}</p></div>
            <PipelineDiagram nodes={active.nodes} label={`${active.label} phase flow`} />
            <CodeBlock title={active.codeTitle} language={active.codeLanguage} lines={active.codeLines} />
            <p className="phase-panel__footnote"><span>↳</span> {active.footnote}</p>
          </div>
        </div>
      </div>
    </section>
  );
}
