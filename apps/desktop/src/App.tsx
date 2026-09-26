import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import logoUrl from "./assets/secondego-logo.png";
import type { EngineEvent, RunView } from "./types";

const defaultGateway =
  new URLSearchParams(window.location.search).get("gateway") ||
  import.meta.env.VITE_GATEWAY_URL ||
  "http://127.0.0.1:8787";

type Worker = { phase: string; name: string; role: string; position: string; tone: string };

const workers: Worker[] = [
  { phase: "UNDERSTAND", name: "Mira", role: "Cartographer", position: "worker-understand", tone: "aqua" },
  { phase: "EXPLORE", name: "Pip", role: "Scout", position: "worker-explore", tone: "blue" },
  { phase: "PLAN", name: "Orin", role: "Architect", position: "worker-plan", tone: "lilac" },
  { phase: "EXECUTE", name: "Kade", role: "Builder", position: "worker-execute", tone: "coral" },
  { phase: "VERIFY", name: "Vela", role: "Inspector", position: "worker-verify", tone: "mint" },
  { phase: "RECOVER", name: "Sera", role: "Medic", position: "worker-recover", tone: "gold" },
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

function WorkerSprite({ worker, state }: { worker: Worker; state: "idle" | "active" | "done" }) {
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

  function workerState(phase: string): "idle" | "active" | "done" {
    if (activePhase === phase) return complete ? "done" : "active";
    return run?.events.some((event) => event.phase === phase) ? "done" : "idle";
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
          <span className="trigger-mark" aria-hidden="true" /><span>{notchOpen ? "Close mission control" : "Open mission control"}</span><small>{run ? `run ${run.request_id.slice(0, 6)}` : "headless by default"}</small>
        </button>
        {notchOpen && <form className="mission-form" onSubmit={submit}>
          <div className="mission-copy"><span className="kicker">Mission control</span><h1>Give your second self<br />one clear task.</h1><p>SecondEgo plans, changes, and verifies the target repository in a bounded worktree.</p></div>
          <div className="mission-inputs">
            <label>Repository<input id="repository-input" value={repository} onChange={(event) => setRepository(event.target.value)} placeholder="/path/to/repository or https://github.com/owner/repo" autoComplete="url" required /></label>
            <label>Task<textarea value={issue} onChange={(event) => setIssue(event.target.value)} placeholder="Describe the change and how it should be verified." required /></label>
          </div>
          <div className="mission-actions"><button className="primary-action" disabled={busy}>{busy ? "Working…" : "Start verified run"}</button><span>{busy ? "The village is reporting live." : "No hidden agent actions."}</span>{error && <p role="alert">{error}</p>}</div>
          <details className="connection"><summary>Local connection</summary><label>Gateway URL<input value={gateway} onChange={(event) => setGateway(event.target.value)} /></label><label>Gateway token<input type="password" value={token} onChange={(event) => setToken(event.target.value)} required /></label></details>
        </form>}
      </section>

      <section className="workspace">
        <section className="village-panel">
          <header><span className="kicker">The working village</span><h2>{activePhase || "Ready when you are"}</h2><p>{label(latest)}</p></header>
          <div className="village-map" aria-label="Visualized engine phase activity">
            <div className="map-path path-a" /><div className="map-path path-b" /><div className="map-gate" aria-hidden="true" />
            <div className="place place-index"><i />Index house</div><div className="place place-workshop"><i />Workshop</div><div className="place place-lab"><i />Test lab</div><div className="place place-archive"><i />Archive</div>
            {workers.map((worker) => <WorkerSprite key={worker.phase} worker={worker} state={workerState(worker.phase)} />)}
            <div className="map-key"><span><i className="active-dot" />active</span><span><i className="done-dot" />visited</span><span><i className="idle-dot" />idle</span></div>
          </div>
        </section>

        <aside className="run-panel">
          <span className="kicker">Run state</span><strong className={verification?.passed ? "passed" : ""}>{run?.status || "standby"}</strong>
          <p className="run-task">{run?.issue || "Open mission control to submit a repository task."}</p>
          <div className="signal"><span>Latest signal</span><b>{latest ? `${time(latest.timestamp)} · ${label(latest)}` : "No engine signal"}</b></div>
          <ol>{workers.map((worker) => <li className={workerState(worker.phase)} key={worker.phase}><i /> <span>{worker.phase}</span><small>{worker.name} · {worker.role}</small></li>)}</ol>
          <p className="run-note">This view is evidence-led: it never invents activity or completion.</p>
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
