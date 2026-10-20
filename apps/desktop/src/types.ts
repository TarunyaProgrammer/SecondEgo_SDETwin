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

export type VoiceSnapshot = {
  enabled: boolean;
  provider: string;
  state: "DISABLED" | "IDLE" | "GENERATING" | "SPEAKING" | "UNAVAILABLE";
  queue_length: number;
  last_error: string | null;
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
  diff?: {
    format: string;
    patch: string;
    truncated: boolean;
    files: Array<{
      path: string;
      status: "added" | "modified" | "deleted" | "renamed" | "binary";
      additions: number;
      deletions: number;
      hunks: Array<{ header: string; lines: Array<{ kind: "add" | "delete" | "context"; content: string }> }>;
    }>;
  };
  resource_usage: Record<string, number>;
  verification: Verification;
  evidence: Array<Record<string, unknown>>;
  target_mutated?: boolean;
  lenses?: string[];
  findings?: DiscoveryFinding[];
  repository?: {
    files: number;
    symbols: number;
    tests: number;
    parser_failures: number;
  };
  rejected_signals?: number;
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
  api_version?: number;
  request_id: string;
  repository: string;
  issue: string;
  profile_id?: string | null;
  provider?: string;
  model: string;
  mode?: "task" | "discover";
  status: string;
  events: EngineEvent[];
  event_offset?: number;
  next_offset?: number;
  event_count?: number;
  has_more?: boolean;
  result: RunResult | null;
  error: string | null;
  voice?: VoiceSnapshot;
};

export type RunSummary = {
  request_id: string;
  status: string;
  profile_id?: string | null;
  workspace?: string | null;
  provider?: string | null;
  model?: string | null;
  engine_version?: string | null;
  created_at?: string | null;
  updated_at?: string | null;
};

export type GestureActionType =
  | "expand_notch"
  | "collapse_notch";

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
  serviceState: GestureServiceState;
  serviceMessage: string | null;
};

export type ProviderProfile = {
  id: string;
  provider: string;
  profile: string;
  model: string | null;
  keyPresent: boolean;
  envName: string;
  updatedAt: string;
};

export type ProviderProfileSnapshot = {
  active: string | null;
  entries: ProviderProfile[];
};

export type ProviderModel = {
  id: string;
  display_name: string | null;
};

export type GestureServiceState = "disabled" | "starting" | "active" | "unavailable" | "error";

export type GestureServiceStatus = {
  schema_version: 1;
  state: GestureServiceState;
  message: string | null;
  timestamp: string;
};

export type GatewayStatus = {
  schema_version: 1;
  state: "starting" | "ready" | "recovering" | "failed";
  message: string | null;
  timestamp: string;
};

export type LandmarkGestureSignal = {
  schema_version: 1;
  gesture: "open_palm" | "pinch";
  hand: "Left" | "Right";
  confidence: number;
  timestamp: string;
};

declare global {
  interface Window {
    secondEgoWindow?: {
      setExpanded: (expanded: boolean) => Promise<void>;
      onNotchCollapse?: (callback: () => void) => () => void;
      setGestureEnabled?: (enabled: boolean) => Promise<GestureServiceStatus>;
      onGesture?: (callback: (event: LandmarkGestureSignal) => void) => () => void;
      onGestureStatus?: (callback: (status: GestureServiceStatus) => void) => () => void;
      onGatewayStatus?: (callback: (status: GatewayStatus) => void) => () => void;
      listProviderProfiles?: () => Promise<ProviderProfileSnapshot>;
      saveProviderProfile?: (profile: { provider: string; profile: string; model?: string; apiKey: string }) => Promise<ProviderProfile>;
      setActiveProviderProfile?: (profile: { provider: string; profile: string }) => Promise<ProviderProfileSnapshot>;
      deleteProviderProfile?: (profile: { provider: string; profile: string }) => Promise<ProviderProfileSnapshot>;
    };
  }
}
