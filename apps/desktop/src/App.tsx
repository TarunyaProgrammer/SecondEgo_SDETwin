import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import type { EngineEvent, RunView } from "./types";

const defaultGateway =
  new URLSearchParams(window.location.search).get("gateway") ||
  import.meta.env.VITE_GATEWAY_URL ||
  "http://127.0.0.1:8787";

type WorkerDefinition = {
  phase: string;
  name: string;
  role: string;
  mark: string;
  position: string;
};

const workers: WorkerDefinition[] = [
  { phase: "UNDERSTAND", name: "Mira", role: "Cartographer", mark: "map", position: "worker-understand" },
  { phase: "EXPLORE", name: "Pip", role: "Scout", mark: "scout", position: "worker-explore" },
  { phase: "PLAN", name: "Orin", role: "Architect", mark: "plan", position: "worker-plan" },
  { phase: "EXECUTE", name: "Kade", role: "Builder", mark: "build", position: "worker-execute" },
  { phase: "VERIFY", name: "Vela", role: "Inspector", mark: "verify", position: "worker-verify" },
  { phase: "RECOVER", name: "Sera", role: "Medic", mark: "recover", position: "worker-recover" },
];

function initialToken(): string {
  return new URLSearchParams(window.location.search).get("token") || "";
}

function eventLabel(event: EngineEvent | undefined): string {
  if (!event) return "No engine signal yet";
  return event.event_type.replace(/\./g, " / ");
}

function formatTime(value: string): string {
  return new Date(value).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function PixelWorker({ worker, state }: { worker: WorkerDefinition; state: "idle" | "active" | "done" }) {
  return (
    <div className={`worker ${worker.position} worker-${state}`}>
      <div className="worker-bubble" aria-hidden="true">{state === "active" ? "WORKING" : state === "done" ? "DONE" : "STANDBY"}</div>
      <div className={`pixel-worker pixel-${worker.mark}`} aria-hidden="true">
        <span className="pixel-shadow" /><span className="pixel-head" /><span className="pixel-body" />
        <span className="pixel-arm pixel-arm-left" /><span className="pixel-arm pixel-arm-right" />
      </div>
      <div className="worker-copy"><strong>{worker.name}</strong><span>{worker.role}</span><small>{worker.phase}</small></div>
    </div>
  );
}

function App() {
  const notchMode = new URLSearchParams(window.location.search).get("notch") === "1";
  const [gateway, setGateway] = useState(defaultGateway);
  const [token, setToken] = useState(initialToken);
  const [repository, setRepository] = useState("");
  const [issue, setIssue] = useState("");
  const [run, setRun] = useState<RunView | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notchOpen, setNotchOpen] = useState(!notchMode);
  const offsetRef = useRef(0);

  const api = useMemo(() => gateway.replace(/\/$/, ""), [gateway]);

  useEffect(() => {
    document.body.classList.toggle("notch-mode", notchMode);
    void window.secondEgoWindow?.setExpanded(notchOpen);
    if (notchMode && notchOpen && !busy && !run) {
      window.setTimeout(() => document.getElementById("repository-input")?.focus(), 80);
    }
    return () => document.body.classList.remove("notch-mode");
  }, [busy, notchMode, notchOpen, run]);

  useEffect(() => {
    const requestId = run?.request_id;
    if (!requestId || run?.result || run?.error) return;
    let cancelled = false;
    const poll = async () => {
      try {
        const currentOffset = offsetRef.current;
        const response = await fetch(`${api}/api/runs/${requestId}?offset=${currentOffset}`, { headers: { "X-SecondEgo-Token": token } });
        const next = (await response.json()) as RunView;
        if (!response.ok) throw new Error((next as unknown as { error?: string }).error || "poll failed");
        if (cancelled) return;
        offsetRef.current = currentOffset + next.events.length;
        setRun((current) => current && current.request_id === requestId
          ? { ...current, ...next, events: [...current.events, ...next.events] }
          : current);
        if (next.result || next.error) setBusy(false);
      } catch (pollError) {
        if (cancelled) return;
        setError(pollError instanceof Error ? pollError.message : "poll failed");
        setBusy(false);
      }
    };
    void poll();
    const timer = window.setInterval(poll, 650);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [api, run?.error, run?.request_id, run?.result, token]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true); setError(""); setRun(null); offsetRef.current = 0;
    try {
      const response = await fetch(`${api}/api/runs`, {
        method: "POST",
        headers: { "Content-Type": "application/json", "X-SecondEgo-Token": token },
        body: JSON.stringify({ repository, issue }),
      });
      const payload = (await response.json()) as { request_id?: string; status?: string; error?: string };
      if (!response.ok || !payload.request_id) throw new Error(payload.error || "could not start run");
      setRun({ request_id: payload.request_id, repository, issue, model: "", status: payload.status || "QUEUED", events: [], result: null, error: null });
      setNotchOpen(false);
    } catch (submitError) {
      setError(submitError instanceof Error ? submitError.message : "could not start run");
      setBusy(false);
    }
  }

  const latestEvent = run?.events[run.events.length - 1];
  const result = run?.result;
  const activePhase = latestEvent?.phase || "";
  const terminal = Boolean(result || run?.error);

  function workerState(phase: string): "idle" | "active" | "done" {
    if (activePhase === phase) return terminal ? "done" : "active";
    if (terminal && run?.events.some((event) => event.phase === phase)) return "done";
    return "idle";
  }

  return (
    <main
      className={`shell ${notchMode ? `notch-shell ${notchOpen ? "notch-open" : "notch-closed"}` : ""}`}
      onMouseEnter={() => notchMode && setNotchOpen(true)}
      onMouseLeave={() => notchMode && !busy && setNotchOpen(false)}
    >
      <header className="topbar">
        <div className="brand-lockup"><div className="brand-seal" aria-hidden="true"><span /></div><div><div className="eyebrow">SECOND EGO / LOCAL AGENT VILLAGE</div><h1>Watch the work happen.</h1><p className="lede">A pixel map of real repository intelligence, tools, and verification.</p></div></div>
        <div className={`run-pill ${result?.verification.passed ? "good" : ""}`}><span className="status-dot" />{run?.status || "STANDBY"}</div>
      </header>

      <section className={`command-notch ${notchOpen ? "notch-open" : "notch-closed"}`}>
        <button className="notch-handle" type="button" onClick={() => setNotchOpen((open) => !open)} aria-expanded={notchOpen}><span className="notch-grip" aria-hidden="true" /><span>{notchOpen ? "CLOSE COMMAND NOTCH" : "OPEN COMMAND NOTCH"}</span><span className="notch-state">{run ? `RUN ${run.request_id.slice(0, 8)}` : "NO RUN"}</span></button>
        {notchOpen && <form className="notch-drawer" onSubmit={submit}>
          <div className="notch-intro"><span className="panel-label">MISSION CONTROL</span><h2>Give the village one repository task.</h2><p>The engine owns the plan. The village only shows its evidence.</p></div>
          <div className="notch-fields"><label>Repository path or public GitHub URL<input id="repository-input" value={repository} onChange={(event) => setRepository(event.target.value)} placeholder="/path/to/repository or https://github.com/owner/repo" autoComplete="url" required /></label><label>Issue<textarea id="issue-input" value={issue} onChange={(event) => setIssue(event.target.value)} placeholder="Describe the change to make and verify." required /></label></div>
          <details className="advanced-fields"><summary>Connection details</summary><label>Gateway URL<input value={gateway} onChange={(event) => setGateway(event.target.value)} /></label><label>Gateway token<input type="password" value={token} onChange={(event) => setToken(event.target.value)} placeholder="Printed by the local gateway" required /></label></details>
          <div className="notch-actions"><button className="start-button" disabled={busy}>{busy ? "VILLAGE WORKING..." : "START VERIFIED RUN"}</button>{error && <div className="error" role="alert">{error}</div>}</div>
        </form>}
      </section>

      <section className="village-layout">
        <section className="village-stage" aria-label="Agent village visualization">
          <div className="stage-header"><div><span className="panel-label">THE WORKING VILLAGE</span><h2>{activePhase || "Awaiting a mission"}</h2></div><div className="stage-coordinate">{latestEvent ? `${formatTime(latestEvent.timestamp)} / ${eventLabel(latestEvent)}` : "MAP IS QUIET"}</div></div>
          <div className="village-map">
            <div className="map-sun" aria-hidden="true" /><div className="map-road road-horizontal" aria-hidden="true" /><div className="map-road road-vertical" aria-hidden="true" />
            <div className="building building-index"><span className="building-roof" /><span className="building-window" /><small>INDEX HOUSE</small></div><div className="building building-workshop"><span className="building-roof" /><span className="building-window" /><small>WORKSHOP</small></div><div className="building building-lab"><span className="building-roof" /><span className="building-window" /><small>TEST LAB</small></div><div className="building building-archive"><span className="building-roof" /><span className="building-window" /><small>ARCHIVE</small></div>
            <div className="map-tree tree-one" aria-hidden="true" /><div className="map-tree tree-two" aria-hidden="true" />
            {workers.map((worker) => <PixelWorker key={worker.phase} worker={worker} state={workerState(worker.phase)} />)}
            <div className="map-legend"><span><i className="legend-dot active-dot" />active worker</span><span><i className="legend-dot done-dot" />completed phase</span><span><i className="legend-dot idle-dot" />standby</span></div>
          </div>
        </section>

        <aside className="mission-panel"><div className="panel-label">CURRENT MISSION</div><div className="mission-status">{run?.status || "STANDBY"}</div><p className="mission-issue">{run?.issue || "Open the command notch to send the village its first task."}</p><div className="signal-strip"><span>LAST SIGNAL</span><strong>{eventLabel(latestEvent)}</strong></div><div className="phase-list">{workers.map((worker) => { const state = workerState(worker.phase); return <div className={`phase-row ${state}`} key={worker.phase}><span className="phase-mark">{state === "done" ? "✓" : state === "active" ? "·" : "—"}</span><span>{worker.phase}</span><small>{worker.name} / {worker.role}</small></div>; })}</div><div className="mission-note">No invented progress. Every worker state comes from the engine event stream.</div></aside>
      </section>

      <section className="evidence-grid"><section className="panel timeline-panel"><div className="panel-label">ENGINE TRANSCRIPT</div><div className="timeline">{run?.events.length ? run.events.map((event: EngineEvent, index) => <div className="timeline-row" key={`${event.timestamp}-${index}`}><span className="timeline-time">{formatTime(event.timestamp)}</span><span className="timeline-phase">{event.phase}</span><span>{eventLabel(event)}</span></div>) : <div className="empty">Events appear here as the village works.</div>}</div></section><section className="panel result-panel"><div className="panel-label">VERIFICATION</div><div className={`verification ${result?.verification.passed ? "pass" : ""}`}>{result ? (result.verification.passed ? "PASSED" : result.verification.failure_class) : "PENDING"}</div><p>{result?.verification.failure_summary || result?.termination_reason || "Completion is determined by command evidence, not model narration."}</p><div className="panel-label">CHANGED PATHS</div><ul>{result?.changed_paths.length ? result.changed_paths.map((path) => <li key={path}>{path}</li>) : <li className="muted">No verified changes yet.</li>}</ul></section></section>
      <section className="panel evidence-panel"><div className="panel-label">SOURCE-LINKED EVIDENCE</div><pre>{result ? JSON.stringify(result.evidence, null, 2) : "Evidence is source-linked and appears after the run completes."}</pre></section>
    </main>
  );
}

export default App;
