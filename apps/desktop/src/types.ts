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
  target_mutated?: boolean;
  lenses?: string[];
  findings?: DiscoveryFinding[];
};

export type DiscoveryFinding = {
  id: string;
  kind: string;
  status: string;
  title: string;
  summary: string;
  severity: string;
  confidence: number;
  affected_paths: string[];
  evidence: Array<{ reference: string; path: string; line_start: number; line_end: number; summary: string; confidence: number }>;
  verification_plan: string[];
};

export type RunView = {
  request_id: string;
  repository: string;
  issue: string;
  model: string;
  mode?: "task" | "discover";
  status: string;
  events: EngineEvent[];
  result: RunResult | null;
  error: string | null;
};

export type GestureActionType =
  | "expand_notch"
  | "collapse_notch"
  | "start_run"
  | "cancel_run"
  | "focus_input"
  | "scroll_up"
  | "scroll_down";

export type GestureEvent = {
  gesture: string;
  action: GestureActionType;
  confidence: number;
  label: string;
  hand?: string;
  timestamp: string;
};

export type GestureState = {
  enabled: boolean;
  active: boolean;
  lastGesture: GestureEvent | null;
  cameraActive: boolean;
  error: string | null;
};

declare global {
  interface Window {
    secondEgoWindow?: {
      setExpanded: (expanded: boolean) => Promise<void>;
      onGesture?: (callback: (event: GestureEvent) => void) => () => void;
    };
  }
}

