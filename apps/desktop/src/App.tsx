import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import logoUrl from "./assets/secondego-logo.png";
import type { EngineEvent, RunView } from "./types";

const defaultGateway =
  new URLSearchParams(window.location.search).get("gateway") ||
  import.meta.env.VITE_GATEWAY_URL ||
  "http://127.0.0.1:8787";

type Worker = { phase: string; name: string; role: string; position: string; tone: string };
type WorkerState = "idle" | "active" | "done" | "error";
type Facility = { id: string; label: string; detail: string; phases: string[]; position: string };

const workers: Worker[] = [
  { phase: "UNDERSTAND", name: "Mira", role: "Cartographer", position: "worker-understand", tone: "aqua" },
  { phase: "EXPLORE", name: "Pip", role: "Scout", position: "worker-explore", tone: "blue" },
  { phase: "PLAN", name: "Orin", role: "Architect", position: "worker-plan", tone: "lilac" },
  { phase: "EXECUTE", name: "Kade", role: "Builder", position: "worker-execute", tone: "coral" },
  { phase: "VERIFY", name: "Vela", role: "Inspector", position: "worker-verify", tone: "mint" },
  { phase: "RECOVER", name: "Sera", role: "Medic", position: "worker-recover", tone: "gold" },
];

const facilities: Facility[] = [
  { id: "index", label: "Repository index", detail: "understand · explore", phases: ["UNDERSTAND", "EXPLORE"], position: "place-index" },
  { id: "workshop", label: "Workshop", detail: "plan · execute", phases: ["PLAN", "EXECUTE"], position: "place-workshop" },
  { id: "lab", label: "Test lab", detail: "verify", phases: ["VERIFY"], position: "place-lab" },
  { id: "archive", label: "Recovery bay", detail: "recover", phases: ["RECOVER"], position: "place-archive" },
];

function initialToken(): string {
  return new URLSearchParams(window.location.search).get("token") || "";
}

function label(event?: EngineEvent): string {
  return event ? event.event_type.replace(/\./g, " / ") : "Waiting for a run";
}

function time(value: string): string {
  return new Date(value).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function WorkerSprite({ worker, state }: { worker: Worker; state: WorkerState }) {
  return (
    <div className={`worker ${worker.position} ${state}`}>
      <span className="worker-pulse" aria-hidden="true" />
      <span className={`worker-sprite ${worker.tone}`} aria-hidden="true"><i /><b /><em /></span>
      <span className="worker-name"><strong>{worker.name}</strong><small>{worker.role}</small></span>
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
      window.setTimeout(() => document.getElementById("repository-input")?.focus(), 120);
    }
    return () => document.body.classList.remove("notch-mode");
  }, [busy, notchMode, notchOpen, run]);

  useEffect(() => {
    const requestId = run?.request_id;
    if (!requestId || run.result || run.error) return;
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
        if (next.result || next.error) setBusy(false);
      } catch (reason) {
        if (cancelled) return;
        setError(reason instanceof Error ? reason.message : "Could not read this run.");
        setBusy(false);
      }
    };
    void poll();
    const timer = window.setInterval(poll, 750);
    return () => { cancelled = true; window.clearInterval(timer); };
  }, [api, run?.error, run?.request_id, run?.result, token]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError("");
    setRun(null);
    offsetRef.current = 0;
    try {
      const response = await fetch(`${api}/api/runs`, {
        method: "POST",
        headers: { "Content-Type": "application/json", "X-SecondEgo-Token": token },
        body: JSON.stringify({ repository, issue }),
      });
      const payload = (await response.json()) as { request_id?: string; status?: string; error?: string };
      if (!response.ok || !payload.request_id) throw new Error(payload.error || "Could not start the run.");
      setRun({ request_id: payload.request_id, repository, issue, model: "", status: payload.status || "QUEUED", events: [], result: null, error: null });
      setNotchOpen(false);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not start the run.");
      setBusy(false);
    }
  }

  const latest = run?.events[run.events.length - 1];
  const activePhase = latest?.phase || "";
  const complete = Boolean(run?.result || run?.error);
  const verification = run?.result?.verification;
  const activeWorker = workers.find((worker) => worker.phase === activePhase);
  const statusTone = run?.error ? "error" : verification?.passed ? "success" : busy ? "active" : "idle";
  const statusLabel = run?.error ? "needs recovery" : verification?.passed ? "verified" : busy ? "running" : run?.status || "ready";
  const resourceUsage = run?.result?.resource_usage;
  const usage = (key: string) => resourceUsage?.[key] ?? null;

  function workerState(phase: string): WorkerState {
    if (activePhase === phase && run?.error) return "error";
    if (activePhase === phase) return complete ? "done" : "active";
    return run?.events.some((event) => event.phase === phase) ? "done" : "idle";
  }

  function facilityState(facility: Facility): WorkerState {
    const states = facility.phases.map(workerState);
    if (states.includes("error")) return "error";
    if (states.includes("active")) return "active";
    return states.includes("done") ? "done" : "idle";
  }

  return (
    <main
      className={`app-shell ${notchMode ? `notch-shell ${notchOpen ? "is-open" : "is-closed"}` : ""}`}
      onMouseEnter={() => notchMode && setNotchOpen(true)}
      onMouseLeave={() => notchMode && !busy && setNotchOpen(false)}
    >
      <header className="app-header">
        <div className="brand"><img src={logoUrl} alt="SecondEgo" /><span><strong>SecondEgo</strong><small>local coding harness</small></span></div>
        <div className={`status ${verification?.passed ? "verified" : ""}`}><i />{run?.status || "ready"}</div>
      </header>

      <section className="command-surface">
        <button className="notch-trigger" type="button" onClick={() => setNotchOpen((open) => !open)} aria-expanded={notchOpen}>
          <span className="trigger-mark" aria-hidden="true" /><span>{notchOpen ? "SecondEgo / Mission control" : "Open mission control"}</span><small className={statusTone}>{run ? statusLabel : "headless"}</small>
        </button>
       {notchOpen && <form className="mission-form" onSubmit={submit}>
          <div className="mission-copy"><h1>What would you<br />like to make?</h1><p>Give SecondEgo a repository and a goal. Its plan, changes, and proof stay in view.</p></div>
          <div className="mission-inputs">
            <label>Repository<input id="repository-input" value={repository} onChange={(event) => setRepository(event.target.value)} placeholder="/path/to/repository or https://github.com/owner/repo" autoComplete="url" required /></label>
            <label>Task<textarea value={issue} onChange={(event) => setIssue(event.target.value)} placeholder="Describe the change and how it should be verified." required /></label>
          </div>
          <div className="mission-actions"><button className="primary-action" disabled={busy}>{busy ? <span className="button-spinner" aria-hidden="true" /> : <svg aria-hidden="true" viewBox="0 0 16 16"><path d="m5 3 7 5-7 5Z" /></svg>}<span>{busy ? "Initializing" : "Start a run"}</span></button><span>{busy ? "Live engine signals will appear below." : "Autonomous · verified workspace"}</span>{error && <p role="alert">{error}</p>}</div>
          <details className="connection"><summary>Local connection</summary><label>Gateway URL<input value={gateway} onChange={(event) => setGateway(event.target.value)} /></label><label>Gateway token<input type="password" value={token} onChange={(event) => setToken(event.target.value)} required /></label></details>
        </form>}
      </section>

      <section className="workspace">
        <section className="village-panel">
          <header><h2>{activePhase ? `${activePhase.toLowerCase()} in progress` : "The village is ready"}</h2><p>{latest ? label(latest) : "Your workers will light up as the run unfolds."}</p></header>
          <div className={`village-map ${activePhase ? "is-active" : "is-resting"}`} aria-label="Visualized engine phase activity">
            <div className="phase-rail" aria-label="Execution pipeline">
              {workers.map((worker, index) => <span className={workerState(worker.phase)} key={worker.phase}><b>{String(index + 1).padStart(2, "0")}</b><i /><small>{worker.phase}</small></span>)}
            </div>
            <div className="map-link link-index-workshop" aria-hidden="true" /><div className="map-link link-workshop-lab" aria-hidden="true" /><div className="map-link link-workshop-archive" aria-hidden="true" />
            {facilities.map((facility) => <div className={`place ${facility.position} ${facilityState(facility)}`} key={facility.id}><span className="roof" /><span className="house"><i /><b /></span><small>{facility.label}</small><em>{facility.detail}</em></div>)}
            {workers.map((worker) => <WorkerSprite key={worker.phase} worker={worker} state={workerState(worker.phase)} />)}
            {activePhase && <div className="data-packet" aria-label={`${activePhase.toLowerCase()} data moving through the execution map`} />}
            <div className="map-key"><span><i className="active-dot" />active</span><span><i className="done-dot" />complete</span><span><i className="idle-dot" />queued</span></div>
          </div>
        </section>

        <aside className={`run-panel ${statusTone}`}>
          <div className="run-heading"><span className="kicker">Run state</span><strong><i />{statusLabel}</strong></div>
          <dl className="run-metrics">
            <div><dt>Current phase</dt><dd>{activePhase || "—"}</dd></div>
            <div><dt>Current agent</dt><dd>{activeWorker ? activeWorker.name : "—"}</dd></div>
            <div><dt>Engine events</dt><dd>{run?.events.length ?? 0}</dd></div>
            <div><dt>Changed files</dt><dd>{run?.result ? run.result.changed_paths.length : "—"}</dd></div>
            <div><dt>Tool calls</dt><dd>{usage("tool_calls") ?? "—"}</dd></div>
            <div><dt>Model calls</dt><dd>{usage("model_calls") ?? "—"}</dd></div>
          </dl>
          <div className="signal"><span>Latest signal</span><b>{latest ? `${time(latest.timestamp)} · ${label(latest)}` : "No signals yet — the village is ready."}</b></div>
          <ol>{workers.map((worker, index) => <li className={workerState(worker.phase)} key={worker.phase}><i>{workerState(worker.phase) === "done" ? "✓" : workerState(worker.phase) === "active" ? "●" : workerState(worker.phase) === "error" ? "!" : "○"}</i><span><b>{String(index + 1).padStart(2, "0")} {worker.phase}</b><small>{worker.name} · {worker.role}</small></span></li>)}</ol>
          <p className="run-note">Only engine-backed telemetry appears here.</p>
        </aside>
      </section>

      <section className="details-grid">
        <section className="transcript"><span className="kicker">Live transcript</span>{run?.events.length ? <div>{run.events.map((event, index) => <p key={`${event.timestamp}-${index}`}><time>{time(event.timestamp)}</time><b>{event.phase}</b>{label(event)}</p>)}</div> : <p className="empty">The engine event stream will appear here.</p>}</section>
        <section className="outcome"><span className="kicker">Verification</span><strong className={verification?.passed ? "passed" : ""}>{verification ? (verification.passed ? "Passed" : verification.failure_class) : "Pending"}</strong><p>{verification?.failure_summary || run?.result?.termination_reason || "Completion requires command evidence, not model narration."}</p>{run?.result?.changed_paths.length ? <ul>{run.result.changed_paths.map((path) => <li key={path}>{path}</li>)}</ul> : null}</section>
      </section>
    </main>
  );
}

export default App;
