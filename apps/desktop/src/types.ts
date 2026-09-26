export type EngineEvent = {
  schema_version: number;
  run_id: string;
  event_type: string;
  phase: string;
  timestamp: string;
  status: string | null;
  evidence_ref: string | null;
  payload: Record<string, unknown>;
};

export type Verification = {
  passed: boolean;
  failure_class: string;
  failure_summary: string | null;
  commands: string[];
  failure_record: Record<string, unknown> | null;
};

export type RunResult = {
  run_id: string;
  status: string;
  phase: string;
  termination_reason: string;
  changed_paths: string[];
  resource_usage: Record<string, number>;
  verification: Verification;
  evidence: Array<Record<string, unknown>>;
};

export type RunView = {
  request_id: string;
  repository: string;
  issue: string;
  model: string;
  status: string;
  events: EngineEvent[];
  result: RunResult | null;
  error: string | null;
};

declare global {
  interface Window {
    secondEgoWindow?: {
      setExpanded: (expanded: boolean) => Promise<void>;
    };
  }
}
