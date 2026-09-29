import type { EngineEvent } from "../types";
import villageBackdropUrl from "../assets/village-backdrop.png";

export type WorkerState = "idle" | "active" | "done" | "error";

export type PhaseGroup = {
  id: string;
  label: string;
  phases: string[];
};

export type Worker = {
  phase: string;
  name: string;
  role: string;
  position: string;
  tone: string;
  glyph: string;
};

type Facility = {
  id: string;
  label: string;
  detail: string;
  phases: string[];
  position: string;
  glyph: string;
};

export const phaseGroups: PhaseGroup[] = [
  { id: "UNDERSTAND", label: "Understand", phases: ["INITIALIZE", "UNDERSTAND"] },
  { id: "EXPLORE", label: "Explore", phases: ["EXPLORE"] },
  { id: "PLAN", label: "Plan", phases: ["PLAN"] },
  { id: "EXECUTE", label: "Execute", phases: ["EXECUTE"] },
  { id: "VERIFY", label: "Verify", phases: ["VERIFY"] },
  { id: "RECOVER", label: "Recover", phases: ["DIAGNOSE", "RECOVER"] },
];

export const workers: Worker[] = [
  { phase: "UNDERSTAND", name: "Mira", role: "Cartographer", position: "worker-understand", tone: "aqua", glyph: "⌁" },
  { phase: "EXPLORE", name: "Pip", role: "Scout", position: "worker-explore", tone: "blue", glyph: "⌕" },
  { phase: "PLAN", name: "Orin", role: "Architect", position: "worker-plan", tone: "lilac", glyph: "◇" },
  { phase: "EXECUTE", name: "Kade", role: "Builder", position: "worker-execute", tone: "coral", glyph: "✦" },
  { phase: "VERIFY", name: "Vela", role: "Inspector", position: "worker-verify", tone: "mint", glyph: "✓" },
  { phase: "RECOVER", name: "Sera", role: "Medic", position: "worker-recover", tone: "gold", glyph: "↻" },
];

const facilities: Facility[] = [
  { id: "index", label: "Repository index", detail: "understand · explore", phases: ["UNDERSTAND", "EXPLORE"], position: "place-index", glyph: "▤" },
  { id: "workshop", label: "Workshop", detail: "plan · execute", phases: ["PLAN", "EXECUTE"], position: "place-workshop", glyph: "⌘" },
  { id: "lab", label: "Test lab", detail: "verify", phases: ["VERIFY"], position: "place-lab", glyph: "∿" },
  { id: "archive", label: "Recovery bay", detail: "diagnose · recover", phases: ["RECOVER"], position: "place-archive", glyph: "↺" },
];

function normalizedPhase(phase: string): string {
  return phase.toUpperCase();
}

export function phaseGroupFor(phase: string): PhaseGroup | undefined {
  const normalized = normalizedPhase(phase);
  return phaseGroups.find((group) => group.phases.includes(normalized));
}

export function workerStateFor(
  phase: string,
  activePhase: string,
  events: EngineEvent[],
  complete: boolean,
  failed: boolean,
): WorkerState {
  const group = phaseGroupFor(phase);
  if (!group) return "idle";
  const activeGroup = phaseGroupFor(activePhase);
  if (activeGroup?.id === group.id && failed) return "error";
  if (activeGroup?.id === group.id) return complete ? "done" : "active";
  return events.some((event) => group.phases.includes(normalizedPhase(event.phase))) ? "done" : "idle";
}

function WorkerCard({ worker, state }: { worker: Worker; state: WorkerState }) {
  const stateLabel = state === "active" ? "working" : state === "done" ? "complete" : state === "error" ? "failed" : "queued";
  return (
    <div className={`worker ${worker.position} ${state}`} title={`${worker.name}, ${worker.role} — ${stateLabel}`}>
      <span className="worker-pulse" aria-hidden="true" />
      <span className={`worker-sprite ${worker.tone}`} aria-hidden="true"><span>{worker.glyph}</span><i /></span>
      <span className="worker-name"><strong>{worker.name}</strong><small>{worker.role}</small><em>{stateLabel}</em></span>
    </div>
  );
}

export function VillageScene({
  activePhase,
  events,
  complete,
  failed,
}: {
  activePhase: string;
  events: EngineEvent[];
  complete: boolean;
  failed: boolean;
}) {
  const activeGroup = phaseGroupFor(activePhase);
  const workerState = (phase: string) => workerStateFor(phase, activePhase, events, complete, failed);
  const facilityState = (facility: Facility): WorkerState => {
    const states = facility.phases.map(workerState);
    if (states.includes("error")) return "error";
    if (states.includes("active")) return "active";
    return states.includes("done") ? "done" : "idle";
  };

  return (
    <div className={`village-map ${activePhase ? "is-active" : "is-resting"}`} role="img" aria-label="SecondEgo village visualizing the engine phase activity">
      <img className="village-backdrop" src={villageBackdropUrl} alt="" aria-hidden="true" />
      <div className="village-decor" aria-hidden="true">
        <span className="village-vignette" />
      </div>

      <div className="phase-rail" aria-hidden="true">
        {phaseGroups.map((group, index) => {
          const state = workerState(group.id);
          return <span className={state} key={group.id}><b>{String(index + 1).padStart(2, "0")}</b><i /><small>{group.label}</small></span>;
        })}
      </div>

      <div className="map-link link-index-workshop" aria-hidden="true" />
      <div className="map-link link-workshop-lab" aria-hidden="true" />
      <div className="map-link link-workshop-archive" aria-hidden="true" />
      <div className={`coordinator-node ${activeGroup ? `phase-${activeGroup.id.toLowerCase()}` : ""}`} title="Presentation-only visualization of the coordinator">
        <span className="coordinator-core" aria-hidden="true">◇</span>
        <span><strong>Coordinator</strong><small>engine observer</small></span>
      </div>

      {facilities.map((facility) => <div className={`place ${facility.position} ${facilityState(facility)}`} key={facility.id} title={`${facility.label}, ${facility.detail}`}>
        <span className="place-icon" aria-hidden="true">{facility.glyph}</span>
        <span className="roof" />
        <span className="house"><i /><b /></span>
        <small>{facility.label}</small>
        <em>{facility.detail}</em>
      </div>)}

      {workers.map((worker) => <WorkerCard key={worker.phase} worker={worker} state={workerState(worker.phase)} />)}
      {activeGroup && <div className={`data-packet phase-${activeGroup.id.toLowerCase()}`} aria-hidden="true" />}
      <div className="map-key" aria-hidden="true"><span><i className="active-dot" />active</span><span><i className="done-dot" />complete</span><span><i className="idle-dot" />queued</span></div>
    </div>
  );
}
