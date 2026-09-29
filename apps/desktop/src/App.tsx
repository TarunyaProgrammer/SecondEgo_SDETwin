import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import logoUrl from "./assets/secondego-logo.png";
import { GestureIndicator } from "./components/GestureIndicator";
import { phaseGroupFor, phaseGroups, VillageScene, workerStateFor, workers } from "./components/VillageScene";
import { useGestureControl } from "./hooks/useGestureControl";
import type { DiscoveryFinding, EngineEvent, GestureActionType, GestureEvent, RunView, VoiceSnapshot } from "./types";

const defaultGateway =
  new URLSearchParams(window.location.search).get("gateway") ||
  import.meta.env.VITE_GATEWAY_URL ||
  "http://127.0.0.1:8787";

const discoveryLenses = [
  { id: "error", label: "Error paths", detail: "swallowed failures" },
  { id: "test", label: "Test gaps", detail: "uncovered public code" },
  { id: "structural", label: "Structure", detail: "maintenance risks" },
];

function initialToken(): string {
  return new URLSearchParams(window.location.search).get("token") || "";
}

function label(event?: EngineEvent): string {
  const message = event?.payload?.message;
  const reason = event?.payload?.reason;
  if (typeof message === "string" && message.trim()) return message;
  if (typeof reason === "string" && reason.trim()) return reason;
  return event ? event.event_type.replace(/\./g, " / ") : "Waiting for a run";
}

function time(value: string): string {
  return new Date(value).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}

function voiceLabel(voice?: VoiceSnapshot): string {
  if (!voice || voice.state === "DISABLED") return "voice disabled";
  if (voice.state === "GENERATING") return "voice generating";
  if (voice.state === "SPEAKING") return "voice speaking";
  if (voice.state === "UNAVAILABLE") return "voice unavailable";
  return "voice idle";
}

function FindingRow({ finding }: { finding: DiscoveryFinding }) {
  const evidence = finding.evidence[0];
  return (
    <article className="finding-row">
      <div className="finding-meta"><span>{finding.kind.replace(/_/g, " ")}</span><b>{Math.round(finding.confidence * 100)}% confidence</b></div>
      <strong>{finding.title}</strong>
      <p>{finding.summary}</p>
      {evidence && <code>{evidence.path}:{evidence.line_start}–{evidence.line_end}</code>}
    </article>
  );
}

function App() {
  const notchMode = new URLSearchParams(window.location.search).get("notch") === "1";
  const [gateway, setGateway] = useState(defaultGateway);
  const [token, setToken] = useState(initialToken);
  const [repository, setRepository] = useState("");
  const [issue, setIssue] = useState("");
  const [mode, setMode] = useState<"task" | "discover">("task");
  const [selectedLenses, setSelectedLenses] = useState<string[]>(discoveryLenses.map((lens) => lens.id));
  const [run, setRun] = useState<RunView | null>(null);
  const [busy, setBusy] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [error, setError] = useState("");
  const [notchOpen, setNotchOpen] = useState(!notchMode);
  const [showSettings, setShowSettings] = useState(false);
  const offsetRef = useRef(0);
  const api = useMemo(() => gateway.replace(/\/$/, ""), [gateway]);

  const handleGestureAction = useCallback((action: GestureActionType, _event: GestureEvent) => {
    switch (action) {
      case "expand_notch":
        setNotchOpen(true);
        break;
      case "collapse_notch":
        setNotchOpen(false);
        break;
    }
  }, []);

  const gestureControl = useGestureControl({
    notchMode,
    onAction: handleGestureAction,
  });

  useEffect(() => {
    document.body.classList.toggle("notch-mode", notchMode);
    if (notchMode) void window.secondEgoWindow?.setExpanded(notchOpen);
    return () => document.body.classList.remove("notch-mode");
  }, [notchMode, notchOpen]);

  useEffect(() => {
    if (!notchMode || !notchOpen || busy || run) return;
    const timer = window.setTimeout(() => document.getElementById("repository-input")?.focus(), 120);
    return () => window.clearTimeout(timer);
  }, [busy, notchMode, notchOpen, run]);

  useEffect(() => {
    if (!notchMode) return;
    if (!notchOpen) setShowSettings(false);
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && notchOpen && !busy) {
        setShowSettings(false);
        setNotchOpen(false);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [busy, notchMode, notchOpen]);

  useEffect(() => {
    if (!notchMode) return;
    const collapse = () => {
      setShowSettings(false);
      setNotchOpen(false);
    };
    const unsubscribe = window.secondEgoWindow?.onNotchCollapse?.(collapse);
    const onWindowBlur = () => collapse();
    const onVisibilityChange = () => {
      if (document.hidden) collapse();
    };
    window.addEventListener("blur", onWindowBlur);
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => {
      unsubscribe?.();
      window.removeEventListener("blur", onWindowBlur);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [notchMode]);

  useEffect(() => {
    const requestId = run?.request_id;
    if (!requestId || run?.result || run?.error || ["COMPLETE", "FAILED", "CANCELLED", "BLOCKED"].includes(run?.status || "")) return;
    let cancelled = false;
    const poll = async () => {
      try {
        const offset = offsetRef.current;
        const response = await fetch(`${api}/api/runs/${requestId}?offset=${offset}`, {
          headers: { "X-SecondEgo-Token": token },
        });
        const next = (await response.json()) as RunView & { error?: string };
        if (!response.ok) throw new Error(next.error || "Could not read this run.");
        if (cancelled) return;
        offsetRef.current += next.events.length;
        setRun((current) => current && current.request_id === requestId
          ? { ...current, ...next, events: [...current.events, ...next.events] }
          : current);
        if (next.result || next.error || ["COMPLETE", "FAILED", "CANCELLED", "BLOCKED"].includes(next.status)) {
          setBusy(false);
          setStopping(false);
        }
      } catch (reason) {
        if (cancelled) return;
        setError(reason instanceof Error ? reason.message : "Could not read this run.");
        setBusy(false);
      }
    };
    void poll();
    const timer = window.setInterval(poll, 750);
    return () => { cancelled = true; window.clearInterval(timer); };
  }, [api, run?.error, run?.request_id, run?.result, run?.status, token]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setStopping(false);
    setError("");
    setRun(null);
    offsetRef.current = 0;
    try {
      const response = await fetch(`${api}/api/runs`, {
        method: "POST",
        headers: { "Content-Type": "application/json", "X-SecondEgo-Token": token },
        body: JSON.stringify({
          repository,
          issue: mode === "discover" ? "Discover repository issues" : issue,
          mode,
          lenses: mode === "discover" ? selectedLenses : undefined,
          max_findings: 20,
        }),
      });
      const payload = (await response.json()) as { request_id?: string; status?: string; error?: string };
      if (!response.ok || !payload.request_id) throw new Error(payload.error || "Could not start the run.");
      setRun({ request_id: payload.request_id, repository, issue: mode === "discover" ? "Discover repository issues" : issue, model: "", mode, status: payload.status || "QUEUED", events: [], result: null, error: null });
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not start the run.");
      setBusy(false);
    }
  }

  async function cancelRun() {
    const requestId = run?.request_id;
    if (!requestId || !busy || stopping) return;
    setStopping(true);
    setError("");
    try {
      const response = await fetch(`${api}/api/runs/${requestId}/cancel`, {
        method: "POST",
        headers: { "X-SecondEgo-Token": token },
      });
      const payload = (await response.json()) as { error?: string };
      if (!response.ok) throw new Error(payload.error || "Could not stop the run.");
    } catch (reason) {
      setStopping(false);
      setError(reason instanceof Error ? reason.message : "Could not stop the run.");
    }
  }

  const latest = run?.events[run.events.length - 1];
  const activePhase = latest?.phase || run?.result?.phase || "";
  const activeGroup = phaseGroupFor(activePhase);
  const terminal = ["COMPLETE", "FAILED", "CANCELLED", "BLOCKED"].includes(run?.status || "");
  const complete = Boolean(run?.result || run?.error || terminal);
  const verification = run?.result?.verification;
  const findings = run?.result?.findings || [];
  const isDiscovery = run?.mode === "discover";
  const activeWorker = workers.find((worker) => worker.phase === activeGroup?.id);
  const failed = Boolean(run?.error || run?.status === "FAILED" || run?.status === "BLOCKED");
  const statusTone = run?.status === "CANCELLED" ? "idle" : run?.status === "BLOCKED" ? "warning" : failed ? "error" : verification?.passed ? "success" : busy ? "active" : "idle";
  const statusLabel = run?.status === "CANCEL_REQUESTED" ? "stopping" : run?.status === "CANCELLED" ? "cancelled" : run?.status === "BLOCKED" ? "blocked" : failed ? "failed" : verification?.passed ? "verified" : busy ? "running" : run?.status || "ready";
  const eventUsage = latest?.payload?.resource_usage;
  const resourceUsage = (eventUsage && typeof eventUsage === "object" ? eventUsage : run?.result?.resource_usage) as Record<string, number> | undefined;
  const usage = (key: string) => resourceUsage?.[key] ?? null;
  const activeOperation = typeof latest?.payload?.operation === "string" ? latest.payload.operation : "—";
  const eventPaths = stringArray(latest?.payload?.changed_paths);
  const changedPaths = run?.result?.changed_paths || eventPaths;
  const affectedPaths = changedPaths.length ? changedPaths : eventPaths;
  const repositorySummary = run?.result?.repository;
  const workerState = (phase: string) => workerStateFor(phase, activePhase, run?.events || [], complete, failed);

  return (
    <main
      className={`app-shell ${notchMode ? `notch-shell ${notchOpen ? "is-open" : "is-closed"}` : ""}`}
    >
      <header className="app-header">
        <div className="brand"><img src={logoUrl} alt="SecondEgo" /><span><strong>SecondEgo</strong><small>local coding harness</small></span></div>
        <div className="app-header-meta"><span className="workspace-badge"><i />local-first</span><div className={`status ${statusTone}`}><i />{run ? statusLabel : "ready"}</div></div>
      </header>

      <section className="command-surface">
        <button className="notch-trigger" type="button" onClick={() => {
          setNotchOpen((open) => {
            if (open) setShowSettings(false);
            return !open;
          });
        }} aria-expanded={notchOpen} aria-controls={notchMode ? "mission-control" : undefined}>
          <span className="trigger-mark" aria-hidden="true" /><span>{notchOpen ? "SecondEgo / Mission control" : "Open mission control"}</span><small className={statusTone}>{run ? statusLabel : "headless"}</small>
        </button>
        {notchMode && (
          <div className="notch-controls">
            <GestureIndicator
              enabled={gestureControl.enabled}
              lastGesture={gestureControl.lastGesture}
              cameraActive={gestureControl.cameraActive}
              error={gestureControl.error}
              serviceState={gestureControl.serviceState}
              onToggle={gestureControl.toggleEnabled}
              onTriggerManual={gestureControl.triggerManualGesture}
              onOpenNotch={() => setNotchOpen(true)}
            />
            <button className="notch-settings-button" type="button" aria-label="Open companion settings" aria-expanded={showSettings} aria-controls="notch-settings" onClick={() => {
              setNotchOpen(true);
              setShowSettings((open) => !open);
            }}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.12 2.12-.06-.06a1.7 1.7 0 0 0-1.88-.34 1.7 1.7 0 0 0-1.04 1.56V20.5h-3v-.28A1.7 1.7 0 0 0 10.66 18.66a1.7 1.7 0 0 0-1.88.34l-.06.06-2.12-2.12.06-.06A1.7 1.7 0 0 0 7 15a1.7 1.7 0 0 0-1.56-1.04h-.28v-3h.28A1.7 1.7 0 0 0 7 9.92a1.7 1.7 0 0 0-.34-1.88L6.6 7.98l2.12-2.12.06.06a1.7 1.7 0 0 0 1.88.34 1.7 1.7 0 0 0 1.04-1.56V4.42h3v.28a1.7 1.7 0 0 0 1.04 1.56 1.7 1.7 0 0 0 1.88-.34l.06-.06 2.12 2.12-.06.06a1.7 1.7 0 0 0-.34 1.88 1.7 1.7 0 0 0 1.56 1.04h.28v3h-.28A1.7 1.7 0 0 0 19.4 15Z" /></svg>
            </button>
          </div>
        )}
        {notchMode && notchOpen && showSettings && <aside className="notch-settings" id="notch-settings" role="dialog" aria-label="Companion settings">
          <header><span>Companion settings</span><button type="button" onClick={() => setShowSettings(false)} aria-label="Close companion settings">×</button></header>
          <section><strong>Camera gestures</strong><p>Optional local landmark tracking. Clicking the capsule always works without a camera.</p><button className={`settings-switch ${gestureControl.enabled ? "is-on" : ""}`} type="button" role="switch" aria-checked={gestureControl.enabled} onClick={gestureControl.toggleEnabled}><i aria-hidden="true" /><span>{gestureControl.serviceState === "unavailable" || gestureControl.serviceState === "error" ? "Retry" : gestureControl.enabled ? "On" : "Off"}</span></button><p className={`gesture-service-status ${gestureControl.serviceState}`}>{gestureControl.serviceMessage || (gestureControl.serviceState === "active" ? "Landmark tracker is live." : gestureControl.serviceState === "starting" ? "Starting local landmark tracker…" : gestureControl.serviceState === "disabled" ? "Camera tracking is off." : "Landmark tracker is unavailable.")}</p></section>
          <section><strong>Window controls</strong><p><b>Click the top capsule</b> opens or closes mission control. <b>Click outside</b> or press <b>Escape</b> to collapse it. Hover does not resize the window.</p></section>
          <section><strong>Gesture map</strong><div className="settings-gesture-map"><span><b>Thumbs up</b><small>Expand</small></span><span><b>Thumbs down</b><small>Collapse</small></span><span><b>Open palm</b><small>Expand fallback</small></span></div><p>Camera input never starts or cancels a run. Use the visible run controls for those actions.</p></section>
          <section><strong>Headless evaluator</strong><p>Run <code>make run</code> in Terminal for the evaluator path. The companion never changes engine permissions or execution state.</p></section>
        </aside>}
       {notchOpen && <form id="mission-control" className="mission-form" onSubmit={submit}>
          <div className="mission-copy"><div className="mode-switch" role="tablist" aria-label="Mission type"><button type="button" className={mode === "task" ? "selected" : ""} onClick={() => setMode("task")}>Fix a task</button><button type="button" className={mode === "discover" ? "selected" : ""} onClick={() => setMode("discover")}>Scan issues</button></div><h1>{mode === "discover" ? <>What needs<br /><em>attention?</em></> : <>What shall we<br /><em>build?</em></>}</h1><p>{mode === "discover" ? "Map suspicious paths, test gaps, and structural risks without touching the repository." : "Give SecondEgo a repository and a goal. Its plan, changes, and proof stay in view."}</p><div className="mission-proof"><span><i className="proof-bolt" />Autonomous</span><span><i className="proof-shield" />Evidence-led</span><span><i className="proof-orbit" />Local-first</span></div></div>
          <div className="mission-inputs">
            <label><span className="form-label"><i>↗</i>Repository</span><input id="repository-input" value={repository} onChange={(event) => setRepository(event.target.value)} placeholder="/path/to/repository or https://github.com/owner/repo" autoComplete="url" required /></label>
            <label><span className="form-label"><i>▤</i>{mode === "discover" ? "Scan focus" : "Task"}</span><textarea value={issue} onChange={(event) => setIssue(event.target.value)} placeholder={mode === "discover" ? "Optional: describe the area to inspect." : "Describe the change and how it should be verified."} required={mode === "task"} /></label>
            {mode === "discover" && <div className="lens-picker" aria-label="Discovery lenses">{discoveryLenses.map((lens) => <label key={lens.id}><input type="checkbox" checked={selectedLenses.includes(lens.id)} onChange={() => setSelectedLenses((current) => current.includes(lens.id) ? current.filter((item) => item !== lens.id) : [...current, lens.id])} /><span><strong>{lens.label}</strong><small>{lens.detail}</small></span></label>)}</div>}
          </div>
          <div className="mission-actions"><button className="primary-action" disabled={busy || (mode === "discover" && selectedLenses.length === 0)}>{busy ? <span className="button-spinner" aria-hidden="true" /> : <svg aria-hidden="true" viewBox="0 0 16 16"><path d="m5 3 7 5-7 5Z" /></svg>}<span>{busy ? "Initializing" : mode === "discover" ? "Scan repository" : "Start a run"}</span></button><span>{busy ? "Live engine signals will appear below." : mode === "discover" ? "Read-only · evidence-led" : "Autonomous · verified workspace"}</span>{error && <p role="alert">{error}</p>}</div>
          <details className="connection"><summary>Local connection</summary><label>Gateway URL<input value={gateway} onChange={(event) => setGateway(event.target.value)} /></label><label>Gateway token<input type="password" value={token} onChange={(event) => setToken(event.target.value)} required /></label></details>
        </form>}
      </section>

      <section className={`workspace ${run ? "has-run" : "is-welcome"}`}>
        <section className="village-panel">
          <header className="village-header"><div className="village-heading"><span className="section-icon" aria-hidden="true">⌘</span><div><span className="kicker">Execution map</span><h2>{activeGroup ? `${activeGroup.label} in progress` : "The village is ready"}</h2><p>{latest ? label(latest) : "Your workers will light up as the run unfolds."}</p></div></div><div className="village-state"><span className={`status-chip ${statusTone}`}><i />{run ? statusLabel : "ready to start"}</span><small>{activePhase ? `engine phase · ${activePhase.toLowerCase()}` : "awaiting a brief"}</small></div></header>
          <VillageScene activePhase={activePhase} events={run?.events || []} complete={complete} failed={failed} />
        </section>

        <aside className={`run-panel ${statusTone} ${run ? "has-run" : "is-welcome"}`}>
          {run ? <>
         <div className="run-task-card"><span className="kicker">Current task</span><strong title={run.issue}>{run.issue}</strong><small title={run.repository}>{run.repository}</small></div>
         <div className="run-heading"><div><span className="kicker">Run state</span><strong><i />{statusLabel}</strong></div><span className={`voice-state voice-${(run.voice?.state || "DISABLED").toLowerCase()}`} aria-label={`Voice ${voiceLabel(run.voice)}`}><i />{voiceLabel(run.voice)}</span>{busy && !complete && <button className="stop-action" type="button" onClick={() => void cancelRun()} disabled={stopping}>{stopping ? "Stopping…" : "Stop run"}</button>}</div>
         <div className="run-facts"><div><span>Active operation</span><b>{activeOperation}</b></div><div><span>Affected paths</span><b>{affectedPaths.length ? `${affectedPaths.length} path${affectedPaths.length === 1 ? "" : "s"}` : "none reported"}</b></div><div><span>Verification</span><b>{verification ? (verification.passed ? "passed" : verification.failure_class) : "pending"}</b></div></div>
         <dl className="run-metrics">
           <div><dt>Current phase</dt><dd>{activeGroup ? `${activeGroup.label} · ${activePhase}` : "—"}</dd></div>
           <div><dt>Current agent</dt><dd>{activeWorker ? activeWorker.name : "—"}</dd></div>
           <div><dt>Engine events</dt><dd>{run?.events.length ?? 0}</dd></div>
           <div><dt>{isDiscovery ? "Findings" : "Changed files"}</dt><dd>{run?.result ? (isDiscovery ? findings.length : changedPaths.length) : "—"}</dd></div>
           <div><dt>Tool calls</dt><dd>{usage("tool_calls") ?? "—"}</dd></div>
           <div><dt>Model calls</dt><dd>{usage("model_calls") ?? "—"}</dd></div>
         </dl>
         <div className="signal"><span>Latest signal</span><b>{latest ? `${time(latest.timestamp)} · ${label(latest)}` : "No signals yet — the village is ready."}</b></div>
         {run.error && <p className="run-error" role="alert">{run.error}</p>}
         <ol>{phaseGroups.map((group, index) => { const state = workerState(group.id); const worker = workers.find((item) => item.phase === group.id); return <li className={state} key={group.id}><i>{state === "done" ? "✓" : state === "active" ? "●" : state === "error" ? "!" : "○"}</i><span><b>{String(index + 1).padStart(2, "0")} {group.label}</b><small>{worker?.name} · {worker?.role}{activePhase && activeGroup?.id === group.id && activePhase !== group.id ? ` · ${activePhase.toLowerCase()}` : ""}</small></span></li>; })}</ol>
         <p className="run-note">Only engine-backed telemetry appears here.</p>
          </> : <section className="welcome-panel" aria-label="How to begin a SecondEgo run">
            <span className="welcome-mark" aria-hidden="true"><i /><b /></span>
            <h3>Your workshop is ready.</h3>
            <p>Give the village one clear brief. It will map the work, make the change, and leave the evidence behind.</p>
            <ol>
              <li><b>01</b><span><strong>Choose a repository</strong><small>Local path or public GitHub URL.</small></span></li>
              <li><b>02</b><span><strong>Choose a mission</strong><small>Fix a task or scan for evidence-backed issue candidates.</small></span></li>
              <li><b>03</b><span><strong>Watch the trail</strong><small>Live engine state replaces this guide automatically.</small></span></li>
            </ol>
          </section>}
       </aside>
      </section>

      <section className="details-grid">
        <section className="transcript"><div className="panel-heading"><span className="kicker">Run log</span><span className="live-tag"><i />{run ? "live" : "waiting"}</span></div>{run?.events.length ? <div>{run.events.map((event, index) => { const operation = typeof event.payload.operation === "string" ? event.payload.operation : event.event_type; return <p key={`${event.timestamp}-${index}`}><time>{time(event.timestamp)}</time><b>{event.phase}</b><span><strong>{label(event)}</strong><small>{operation}</small></span></p>; })}</div> : <p className="empty">The engine event stream will appear here.</p>}</section>
        <section className={`outcome ${isDiscovery ? "discovery-outcome" : ""}`}><div className="panel-heading"><span className="kicker">{isDiscovery ? "Discovery report" : "Verification"}</span><span className={`result-mark ${verification?.passed || (isDiscovery && run?.result?.target_mutated === false) ? "passed" : ""}`} aria-hidden="true">{verification?.passed || (isDiscovery && run?.result?.target_mutated === false) ? "✓" : "·"}</span></div><strong className={verification?.passed || (isDiscovery && run?.result?.target_mutated === false) ? "passed" : ""}>{isDiscovery ? `${findings.length} candidate${findings.length === 1 ? "" : "s"}` : verification ? (verification.passed ? "Passed" : verification.failure_class) : run?.status === "CANCELLED" ? "Cancelled" : run?.status === "BLOCKED" ? "Blocked" : run?.error ? "Failed" : "Pending"}</strong><p>{isDiscovery ? (run?.error || (run?.status === "CANCELLED" ? "Scan stopped safely before a report was produced." : run?.result?.target_mutated === false ? "Read-only scan complete. The target repository was not modified." : run?.result?.termination_reason || "Scan pending")) : run?.status === "CANCELLED" ? "Run stopped safely. Its isolated attempt was discarded." : run?.status === "BLOCKED" ? run.result?.termination_reason || "The engine stopped without applying a change." : run?.error || verification?.failure_summary || run?.result?.termination_reason || "Completion requires command evidence, not model narration."}</p>{isDiscovery && repositorySummary && <div className="discovery-summary"><span><b>{repositorySummary.files}</b> files</span><span><b>{repositorySummary.symbols}</b> symbols</span><span><b>{repositorySummary.tests}</b> tests</span><span><b>{repositorySummary.parser_failures}</b> parser issues</span></div>}{isDiscovery ? <div className="finding-list">{findings.length ? findings.map((finding) => <FindingRow key={finding.id} finding={finding} />) : <p className="empty">No candidate signals matched the selected lenses.</p>}</div> : <>{verification?.commands.length ? <ul className="command-list">{verification.commands.map((command) => <li key={command}><code>{command}</code></li>)}</ul> : null}{changedPaths.length ? <ul>{changedPaths.map((path) => <li key={path}>{path}</li>)}</ul> : null}</>}</section>
      </section>
    </main>
  );
}

export default App;
