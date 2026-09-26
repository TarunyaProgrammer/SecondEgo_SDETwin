import { FormEvent, useEffect, useMemo, useState } from "react";
import type { EngineEvent, RunView } from "./types";

const defaultGateway = import.meta.env.VITE_GATEWAY_URL || "http://127.0.0.1:8787";

function initialToken(): string {
  return new URLSearchParams(window.location.search).get("token") || "";
}

function App() {
  const [gateway, setGateway] = useState(defaultGateway);
  const [token, setToken] = useState(initialToken);
  const [repository, setRepository] = useState("");
  const [issue, setIssue] = useState("");
  const [run, setRun] = useState<RunView | null>(null);
  const [offset, setOffset] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const api = useMemo(() => gateway.replace(/\/$/, ""), [gateway]);

  useEffect(() => {
    if (!run || run.result || run.error) return;
    const timer = window.setInterval(async () => {
      try {
        const response = await fetch(`${api}/api/runs/${run.request_id}?offset=${offset}`, {
          headers: { "X-SecondEgo-Token": token },
        });
        const next = (await response.json()) as RunView;
        if (!response.ok) throw new Error((next as unknown as { error?: string }).error || "poll failed");
        setRun((current) => current ? { ...current, ...next, events: [...current.events, ...next.events] } : next);
        setOffset((current) => current + next.events.length);
        if (next.result || next.error) setBusy(false);
      } catch (pollError) {
        setError(pollError instanceof Error ? pollError.message : "poll failed");
        setBusy(false);
      }
    }, 650);
    return () => window.clearInterval(timer);
  }, [api, offset, run, token]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError("");
    setRun(null);
    setOffset(0);
    try {
      const response = await fetch(`${api}/api/runs`, {
        method: "POST",
        headers: { "Content-Type": "application/json", "X-SecondEgo-Token": token },
        body: JSON.stringify({ repository, issue }),
      });
      const payload = (await response.json()) as { request_id?: string; status?: string; error?: string };
      if (!response.ok || !payload.request_id) throw new Error(payload.error || "could not start run");
      setRun({ request_id: payload.request_id, repository, issue, model: "", status: payload.status || "QUEUED", events: [], result: null, error: null });
    } catch (submitError) {
      setError(submitError instanceof Error ? submitError.message : "could not start run");
      setBusy(false);
    }
  }

  const latestEvent = run?.events[run.events.length - 1];
  const result = run?.result;

  return (
    <main className="shell">
      <header className="topbar">
        <div>
          <div className="eyebrow">LOCAL CODING HARNESS / OBSERVER</div>
          <h1>SecondEgo</h1>
          <p className="lede">A verified coding run, rendered from real engine evidence.</p>
        </div>
        <div className={`run-pill ${result?.verification.passed ? "good" : ""}`}>{run?.status || "IDLE"}</div>
      </header>

      <section className="hero-grid">
        <form className="panel intake" onSubmit={submit}>
          <div className="panel-label">START A RUN</div>
          <label>Gateway URL<input value={gateway} onChange={(event) => setGateway(event.target.value)} /></label>
          <label>Gateway token<input type="password" value={token} onChange={(event) => setToken(event.target.value)} placeholder="Printed by secondego-desktop" required /></label>
          <label>Target repository<input value={repository} onChange={(event) => setRepository(event.target.value)} placeholder="/path/to/repository" required /></label>
          <label>Issue<textarea value={issue} onChange={(event) => setIssue(event.target.value)} placeholder="Describe the change to make and verify." required /></label>
          <button disabled={busy}>{busy ? "RUNNING…" : "START VERIFIED RUN"}</button>
          {error && <div className="error">{error}</div>}
        </form>

        <section className="panel signal">
          <div className="panel-label">CURRENT SIGNAL</div>
          <div className="signal-phase">{latestEvent?.phase || "WAITING"}</div>
          <div className="signal-event">{latestEvent?.event_type || "No engine event yet"}</div>
          <div className="signal-note">The renderer never decides the phase, calls a model, or executes a command.</div>
          <div className="phase-rail">{["UNDERSTAND", "EXPLORE", "PLAN", "EXECUTE", "VERIFY", "RECOVER"].map((phase) => <span className={latestEvent?.phase === phase ? "active" : ""} key={phase}>{phase}</span>)}</div>
        </section>
      </section>

      <section className="content-grid">
        <section className="panel">
          <div className="panel-label">ENGINE TIMELINE</div>
          <div className="timeline">{run?.events.length ? run.events.map((event: EngineEvent, index) => <div className="timeline-row" key={`${event.timestamp}-${index}`}><span className="timeline-time">{new Date(event.timestamp).toLocaleTimeString()}</span><span className="timeline-phase">{event.phase}</span><span>{event.event_type}</span></div>) : <div className="empty">Events appear here as the engine produces them.</div>}</div>
        </section>
        <section className="panel result-panel">
          <div className="panel-label">VERIFICATION</div>
          <div className={`verification ${result?.verification.passed ? "pass" : ""}`}>{result ? (result.verification.passed ? "PASSED" : result.verification.failure_class) : "PENDING"}</div>
          <p>{result?.verification.failure_summary || result?.termination_reason || "Completion is determined by command evidence, not model narration."}</p>
          <div className="panel-label">CHANGED PATHS</div>
          <ul>{result?.changed_paths.length ? result.changed_paths.map((path) => <li key={path}>{path}</li>) : <li className="muted">No verified changes yet.</li>}</ul>
        </section>
      </section>

      <section className="panel evidence-panel">
        <div className="panel-label">RUN EVIDENCE</div>
        <pre>{result ? JSON.stringify(result.evidence, null, 2) : "Evidence is source-linked and appears after the run completes."}</pre>
      </section>
    </main>
  );
}

export default App;
