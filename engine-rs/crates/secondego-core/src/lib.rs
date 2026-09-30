use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const EVENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationMode {
    #[serde(rename = "headless")]
    Headless,
    #[serde(rename = "events")]
    Events,
    #[serde(rename = "desktop")]
    Desktop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Phase {
    Initialize,
    Understand,
    Explore,
    Plan,
    Execute,
    Verify,
    Diagnose,
    Recover,
}

impl Phase {
    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Initialize, Self::Understand)
                | (Self::Understand, Self::Explore)
                | (Self::Understand, Self::Plan)
                | (Self::Explore, Self::Plan)
                | (Self::Plan, Self::Execute)
                | (Self::Execute, Self::Verify)
                // When a tool action itself fails during Execute (e.g. replace_text
                // targets a non-existent path), the runtime synthesises a failed
                // VerificationResult and skips the Verify step so the event log
                // accurately reflects that verification was never attempted.
                | (Self::Execute, Self::Diagnose)
                | (Self::Verify, Self::Diagnose)
                | (Self::Diagnose, Self::Recover)
                | (Self::Recover, Self::Execute)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TerminalStatus {
    Running,
    Complete,
    Failed,
    Blocked,
    Cancelled,
}

impl TerminalStatus {
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Running)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionState {
    pub run_id: Uuid,
    pub task: String,
    pub workspace: String,
    pub acceptance_criteria: Vec<String>,
    pub phase: Phase,
    pub status: TerminalStatus,
    pub current_step: Option<String>,
    pub changed_paths: BTreeSet<String>,
    pub facts: BTreeMap<String, String>,
    pub decisions: Vec<String>,
    pub open_questions: Vec<String>,
    pub failed_approaches: Vec<String>,
    pub retry_counts: BTreeMap<String, u32>,
    pub resource_usage: BTreeMap<String, u64>,
    pub evidence_refs: Vec<String>,
    pub termination_reason: Option<String>,
}

impl ExecutionState {
    pub fn new(task: impl Into<String>, workspace: impl Into<String>) -> Self {
        Self {
            run_id: Uuid::new_v4(),
            task: task.into(),
            workspace: workspace.into(),
            acceptance_criteria: Vec::new(),
            phase: Phase::Initialize,
            status: TerminalStatus::Running,
            current_step: None,
            changed_paths: BTreeSet::new(),
            facts: BTreeMap::new(),
            decisions: Vec::new(),
            open_questions: Vec::new(),
            failed_approaches: Vec::new(),
            retry_counts: BTreeMap::new(),
            resource_usage: BTreeMap::new(),
            evidence_refs: Vec::new(),
            termination_reason: None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineEvent {
    pub schema_version: u32,
    pub run_id: Uuid,
    pub event_type: String,
    pub phase: Phase,
    pub timestamp: DateTime<Utc>,
    pub status: TerminalStatus,
    pub evidence_ref: Option<String>,
    pub payload: serde_json::Value,
}

impl EngineEvent {
    pub fn new(state: &ExecutionState, event_type: impl Into<String>) -> Self {
        Self {
            schema_version: EVENT_SCHEMA_VERSION,
            run_id: state.run_id,
            event_type: event_type.into(),
            phase: state.phase,
            timestamp: Utc::now(),
            status: state.status,
            evidence_ref: None,
            payload: serde_json::json!({}),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBudget {
    pub max_model_calls: u32,
    pub max_tool_calls: u32,
    pub max_retries: u32,
    pub max_runtime: Duration,
    pub max_context_tokens_per_call: u64,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        let max_retries = std::env::var("SECONDEGO_MAX_RETRIES")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(12);
        Self {
            max_model_calls: 20,
            max_tool_calls: 80,
            max_retries,
            max_runtime: Duration::from_secs(900),
            max_context_tokens_per_call: 24_000,
        }
    }
}

#[derive(Debug)]
pub struct ResourceUsage {
    pub budget: ResourceBudget,
    pub model_calls: u32,
    pub tool_calls: u32,
    pub retries: u32,
    pub context_tokens: u64,
    pub provider_input_tokens: u64,
    pub provider_output_tokens: u64,
    started_at: Instant,
}

impl ResourceUsage {
    pub fn new(budget: ResourceBudget) -> Self {
        Self {
            budget,
            model_calls: 0,
            tool_calls: 0,
            retries: 0,
            context_tokens: 0,
            provider_input_tokens: 0,
            provider_output_tokens: 0,
            started_at: Instant::now(),
        }
    }

    pub fn record_model_call(&mut self, context_tokens: u64) -> Result<(), ResourceError> {
        self.ensure_runtime()?;
        if context_tokens > self.budget.max_context_tokens_per_call {
            return Err(ResourceError::ContextBudget);
        }
        if self.model_calls >= self.budget.max_model_calls {
            return Err(ResourceError::ModelCalls);
        }
        self.model_calls += 1;
        self.context_tokens += context_tokens;
        Ok(())
    }

    pub fn record_tool_call(&mut self) -> Result<(), ResourceError> {
        self.ensure_runtime()?;
        if self.tool_calls >= self.budget.max_tool_calls {
            return Err(ResourceError::ToolCalls);
        }
        self.tool_calls += 1;
        Ok(())
    }

    /// Store only provider-supplied aggregate usage. This is intentionally
    /// separate from the local prompt estimate: not every provider returns
    /// usage metadata, and neither value contains prompt or response text.
    pub fn record_provider_usage(&mut self, input_tokens: Option<u64>, output_tokens: Option<u64>) {
        self.provider_input_tokens += input_tokens.unwrap_or_default();
        self.provider_output_tokens += output_tokens.unwrap_or_default();
    }

    pub fn record_retry(&mut self) -> Result<(), ResourceError> {
        self.ensure_runtime()?;
        if self.retries >= self.budget.max_retries {
            return Err(ResourceError::Retries);
        }
        self.retries += 1;
        Ok(())
    }

    pub fn snapshot(&self) -> BTreeMap<String, u64> {
        BTreeMap::from([
            ("model_calls".into(), self.model_calls as u64),
            ("tool_calls".into(), self.tool_calls as u64),
            ("retries".into(), self.retries as u64),
            ("context_tokens".into(), self.context_tokens),
            ("provider_input_tokens".into(), self.provider_input_tokens),
            ("provider_output_tokens".into(), self.provider_output_tokens),
            (
                "elapsed_ms".into(),
                self.started_at.elapsed().as_millis() as u64,
            ),
        ])
    }

    fn ensure_runtime(&self) -> Result<(), ResourceError> {
        if self.started_at.elapsed() > self.budget.max_runtime {
            Err(ResourceError::Runtime)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResourceError {
    #[error("runtime budget exhausted")]
    Runtime,
    #[error("context budget exceeded")]
    ContextBudget,
    #[error("model-call budget exhausted")]
    ModelCalls,
    #[error("tool-call budget exhausted")]
    ToolCalls,
    #[error("retry budget exhausted")]
    Retries,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StateError {
    #[error("terminal execution cannot transition")]
    Terminal,
    #[error("invalid transition from {from:?} to {to:?}")]
    InvalidTransition { from: Phase, to: Phase },
    #[error("transition reason cannot be empty")]
    EmptyReason,
    #[error("termination status must be terminal")]
    NonTerminalStatus,
    #[error("termination reason cannot be empty")]
    EmptyTerminationReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionProposal {
    pub action: String,
    pub arguments: serde_json::Value,
    pub rationale: String,
}

pub struct StateMachine {
    pub state: ExecutionState,
    pub transitions: Vec<(Phase, Phase, String)>,
}

impl StateMachine {
    pub fn new(state: ExecutionState) -> Self {
        Self {
            state,
            transitions: Vec::new(),
        }
    }

    pub fn move_to(&mut self, next: Phase, reason: &str) -> Result<EngineEvent, StateError> {
        if self.state.is_terminal() {
            return Err(StateError::Terminal);
        }
        if !self.state.phase.can_transition_to(next) {
            return Err(StateError::InvalidTransition {
                from: self.state.phase,
                to: next,
            });
        }
        if reason.trim().is_empty() {
            return Err(StateError::EmptyReason);
        }
        let previous = self.state.phase;
        self.state.phase = next;
        self.transitions.push((previous, next, reason.to_owned()));
        let mut event = EngineEvent::new(&self.state, "state.changed");
        event.payload = serde_json::json!({
            "previous_phase": previous,
            "current_phase": next,
            "reason": reason,
        });
        Ok(event)
    }

    pub fn terminate(
        &mut self,
        status: TerminalStatus,
        reason: &str,
    ) -> Result<EngineEvent, StateError> {
        if !status.is_terminal() {
            return Err(StateError::NonTerminalStatus);
        }
        if self.state.is_terminal() {
            return Err(StateError::Terminal);
        }
        if reason.trim().is_empty() {
            return Err(StateError::EmptyTerminationReason);
        }
        self.state.status = status;
        self.state.termination_reason = Some(reason.to_owned());
        let mut event = EngineEvent::new(&self.state, "run.terminated");
        event.payload = serde_json::json!({ "reason": reason });
        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_machine_enforces_the_safe_phase_graph() {
        let mut machine = StateMachine::new(ExecutionState::new("fix", "/repo"));
        machine.move_to(Phase::Understand, "task accepted").unwrap();
        machine.move_to(Phase::Explore, "scan repository").unwrap();
        machine.move_to(Phase::Plan, "evidence prepared").unwrap();
        assert_eq!(machine.state.phase, Phase::Plan);
        assert!(matches!(
            machine.move_to(Phase::Verify, "skip execution"),
            Err(StateError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn resource_usage_rejects_budget_overrun_before_incrementing() {
        let mut usage = ResourceUsage::new(ResourceBudget {
            max_model_calls: 1,
            ..Default::default()
        });
        usage.record_model_call(10).unwrap();
        assert_eq!(usage.record_model_call(10), Err(ResourceError::ModelCalls));
        assert_eq!(usage.model_calls, 1);
    }

    #[test]
    fn events_are_versioned_and_json_serializable() {
        let state = ExecutionState::new("fix", "/repo");
        let event = EngineEvent::new(&state, "state.changed");
        let value = serde_json::to_value(event).unwrap();
        assert_eq!(value["schema_version"], EVENT_SCHEMA_VERSION);
        assert_eq!(value["event_type"], "state.changed");
    }
}
