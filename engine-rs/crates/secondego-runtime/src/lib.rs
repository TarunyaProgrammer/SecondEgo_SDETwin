use secondego_context::{ContextAssembler, ContextBudget, EvidenceLedger, EvidenceRecord};
use secondego_core::{
    EngineEvent, ExecutionState, Phase, ResourceBudget, ResourceUsage, StateMachine, TerminalStatus,
};
use secondego_model::{ModelProvider, ProviderContext, ProviderError};
use secondego_repository::RepositoryIndexer;
use secondego_tools::{
    CommandPolicy, CommandRunner, FileTool, GitAttemptTransaction, PolicyError, SearchTool,
    ToolResult, ToolRouter, WorkspacePolicy,
};
use secondego_verification::{VerificationEngine, VerificationResult};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
pub enum RuntimeError {
    Repository(String),
    Context(String),
    Provider(ProviderError),
    Plan(String),
    Tool(PolicyError),
    State(String),
    Transaction(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RuntimeError {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActionPlan {
    pub actions: Vec<secondego_core::ActionProposal>,
    pub verification_commands: Vec<Vec<String>>,
    #[serde(default)]
    pub recovery_actions: Vec<secondego_core::ActionProposal>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunReport {
    pub state: ExecutionState,
    pub events: Vec<EngineEvent>,
    pub verification: VerificationResult,
    pub evidence: Vec<EvidenceRecord>,
    pub resource_usage: std::collections::BTreeMap<String, u64>,
    pub verification_passed: bool,
    pub changed_paths: Vec<String>,
    pub tool_results: Vec<ToolResult>,
    pub index_files: usize,
    pub index_symbols: usize,
    pub index_parser_failures: usize,
}

pub struct RustEngine<P: ModelProvider> {
    pub provider: P,
    pub context: ContextAssembler,
    pub resources: ResourceUsage,
    pub max_actions: usize,
    pub event_sink: Option<Arc<dyn Fn(&EngineEvent) + Send + Sync>>,
}

impl<P: ModelProvider> RustEngine<P> {
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            context: ContextAssembler::new(ContextBudget {
                total_tokens: 24_000,
                task_tokens: 2_000,
                action_tokens: 2_000,
                evidence_tokens: 16_000,
                state_tokens: 2_000,
                response_tokens: 2_000,
            })
            .expect("static context budget is valid"),
            resources: ResourceUsage::new(ResourceBudget::default()),
            max_actions: 32,
            event_sink: None,
        }
    }

    pub fn with_event_sink(mut self, sink: impl Fn(&EngineEvent) + Send + Sync + 'static) -> Self {
        self.event_sink = Some(Arc::new(sink));
        self
    }

    pub fn run(
        &mut self,
        task: impl Into<String>,
        workspace: impl AsRef<Path>,
    ) -> Result<RunReport, RuntimeError> {
        let workspace = workspace
            .as_ref()
            .canonicalize()
            .map_err(|error| RuntimeError::Repository(error.to_string()))?;
        let mut machine = StateMachine::new(ExecutionState::new(task, workspace.to_string_lossy()));
        let mut events = Vec::new();
        let mut ledger = EvidenceLedger::default();
        let mut tools_used = Vec::new();

        self.append_event(
            &mut events,
            machine
                .move_to(Phase::Understand, "initialize bounded execution")
                .map_err(|error| RuntimeError::State(error.to_string()))?,
        );
        let index = RepositoryIndexer::new(&workspace)
            .build()
            .map_err(|error| RuntimeError::Repository(error.to_string()))?;
        ledger.record(EvidenceRecord::new(
            "repository:index",
            format!(
                "{} files, {} symbols, {} tests, {} parser failures",
                index.snapshot.files.len(),
                index.symbols.len(),
                index.tests.len(),
                index.parser_failures.len()
            ),
            "repository index",
            5,
        ));
        self.append_event(
            &mut events,
            machine
                .move_to(Phase::Explore, "build repository index before planning")
                .map_err(|error| RuntimeError::State(error.to_string()))?,
        );
        let ranked = index.rank(&machine.state.task, 12, false);
        if !ranked.is_empty() {
            ledger.record(EvidenceRecord::new(
                "repository:retrieval",
                serde_json::to_string(&ranked).unwrap_or_default(),
                "explainable repository retrieval",
                4,
            ));
        }
        self.append_event(
            &mut events,
            machine
                .move_to(Phase::Plan, "assemble focused indexed context")
                .map_err(|error| RuntimeError::State(error.to_string()))?,
        );
        let state_json = serde_json::to_string(&machine.state)
            .map_err(|error| RuntimeError::Context(error.to_string()))?;
        let packet = self
            .context
            .assemble(
                &machine.state.task,
                PLAN_INSTRUCTION,
                state_json,
                &ledger.active(),
            )
            .map_err(|error| RuntimeError::Context(format!("{error:?}")))?;
        self.resources
            .record_model_call(packet.estimated_tokens as u64)
            .map_err(|error| RuntimeError::Context(error.to_string()))?;
        let proposal = self
            .provider
            .generate(
                &packet.as_text(),
                &ProviderContext {
                    run_id: machine.state.run_id.to_string(),
                    phase: "PLAN".into(),
                },
            )
            .map_err(RuntimeError::Provider)?;
        let plan = parse_plan(proposal)?;
        if plan.actions.len() > self.max_actions {
            return self.terminate_error(
                &mut machine,
                &mut events,
                RuntimeError::Plan("action count exceeds runtime limit".into()),
            );
        }

        self.append_event(
            &mut events,
            machine
                .move_to(
                    Phase::Execute,
                    "execute validated actions in detached worktree",
                )
                .map_err(|error| RuntimeError::State(error.to_string()))?,
        );
        let target_policy = WorkspacePolicy::new(&workspace).map_err(RuntimeError::Tool)?;
        let mut transaction = GitAttemptTransaction::new(target_policy);
        let attempt_policy = transaction
            .begin()
            .map_err(|error| RuntimeError::Transaction(error.to_string()))?;
        let command_policy = CommandPolicy::default();
        let runner = CommandRunner {
            workspace: attempt_policy.clone(),
            policy: command_policy,
        };
        let router = ToolRouter {
            files: FileTool {
                workspace: attempt_policy.clone(),
                max_file_bytes: 512 * 1024,
            },
            search: SearchTool {
                workspace: attempt_policy.clone(),
                max_results: 200,
            },
            runner,
        };
        if let Err(error) =
            execute_actions(&mut self.resources, &router, &plan.actions, &mut tools_used)
        {
            let _ = transaction.finish(false);
            return self.terminate_error(&mut machine, &mut events, error);
        }
        self.append_event(
            &mut events,
            machine
                .move_to(Phase::Verify, "run bounded verification commands")
                .map_err(|error| RuntimeError::State(error.to_string()))?,
        );
        let verifier = VerificationEngine {
            runner: &router.runner,
        };
        for _ in &plan.verification_commands {
            self.resources.record_tool_call().map_err(|error| {
                RuntimeError::Tool(PolicyError::UnsupportedAction(error.to_string()))
            })?;
        }
        let mut verification = verifier.run(&plan.verification_commands);
        if !verification.passed && !plan.recovery_actions.is_empty() {
            self.append_event(
                &mut events,
                machine
                    .move_to(
                        Phase::Diagnose,
                        "classify verification failure before recovery",
                    )
                    .map_err(|error| RuntimeError::State(error.to_string()))?,
            );
            self.resources.record_retry().map_err(|error| {
                RuntimeError::Tool(PolicyError::UnsupportedAction(error.to_string()))
            })?;
            self.append_event(
                &mut events,
                machine
                    .move_to(Phase::Recover, "apply one bounded, model-proposed repair")
                    .map_err(|error| RuntimeError::State(error.to_string()))?,
            );
            if let Err(error) = execute_actions(
                &mut self.resources,
                &router,
                &plan.recovery_actions,
                &mut tools_used,
            ) {
                let _ = transaction.finish(false);
                return self.terminate_error(&mut machine, &mut events, error);
            }
            self.append_event(
                &mut events,
                machine
                    .move_to(Phase::Execute, "re-run verification after recovery")
                    .map_err(|error| RuntimeError::State(error.to_string()))?,
            );
            self.append_event(
                &mut events,
                machine
                    .move_to(Phase::Verify, "confirm recovery result")
                    .map_err(|error| RuntimeError::State(error.to_string()))?,
            );
            for _ in &plan.verification_commands {
                self.resources.record_tool_call().map_err(|error| {
                    RuntimeError::Tool(PolicyError::UnsupportedAction(error.to_string()))
                })?;
            }
            verification = verifier.run(&plan.verification_commands);
        }
        let passed = verification.passed;
        let transfer = transaction
            .finish(passed)
            .map_err(|error| RuntimeError::Transaction(error.to_string()))?;
        if !passed {
            let _ = machine
                .terminate(
                    TerminalStatus::Failed,
                    verification
                        .failure_summary
                        .as_deref()
                        .unwrap_or("verification failed"),
                )
                .map(|event| self.append_event(&mut events, event));
            return Ok(RunReport {
                state: machine.state,
                events,
                verification: verification.clone(),
                evidence: ledger.snapshot(),
                resource_usage: self.resources.snapshot(),
                verification_passed: false,
                changed_paths: transfer.changed_paths,
                tool_results: tools_used,
                index_files: index.snapshot.files.len(),
                index_symbols: index.symbols.len(),
                index_parser_failures: index.parser_failures.len(),
            });
        }
        self.append_event(
            &mut events,
            machine
                .terminate(
                    TerminalStatus::Complete,
                    "verification passed and diff transferred",
                )
                .map_err(|error| RuntimeError::State(error.to_string()))?,
        );
        Ok(RunReport {
            state: machine.state,
            events,
            verification: verification.clone(),
            evidence: ledger.snapshot(),
            resource_usage: self.resources.snapshot(),
            verification_passed: true,
            changed_paths: transfer.changed_paths,
            tool_results: tools_used,
            index_files: index.snapshot.files.len(),
            index_symbols: index.symbols.len(),
            index_parser_failures: index.parser_failures.len(),
        })
    }

    fn terminate_error<T>(
        &self,
        machine: &mut StateMachine,
        events: &mut Vec<EngineEvent>,
        error: RuntimeError,
    ) -> Result<T, RuntimeError> {
        let _ = machine
            .terminate(TerminalStatus::Failed, &error.to_string())
            .map(|event| self.append_event(events, event));
        Err(error)
    }

    fn append_event(&self, events: &mut Vec<EngineEvent>, event: EngineEvent) {
        if let Some(sink) = &self.event_sink {
            sink(&event);
        }
        events.push(event);
    }
}

fn execute_actions(
    resources: &mut ResourceUsage,
    router: &ToolRouter,
    actions: &[secondego_core::ActionProposal],
    tool_results: &mut Vec<ToolResult>,
) -> Result<(), RuntimeError> {
    for action in actions {
        resources.record_tool_call().map_err(|error| {
            RuntimeError::Tool(PolicyError::UnsupportedAction(error.to_string()))
        })?;
        let result = router.dispatch(action).map_err(RuntimeError::Tool)?;
        if !result.success {
            return Err(RuntimeError::Tool(PolicyError::UnsupportedAction(
                result.stderr,
            )));
        }
        tool_results.push(result);
    }
    Ok(())
}

pub fn parse_plan(proposal: secondego_core::ActionProposal) -> Result<ActionPlan, RuntimeError> {
    if proposal.action != "submit_plan" {
        return Err(RuntimeError::Plan(
            "provider must return submit_plan".into(),
        ));
    }
    let arguments = proposal
        .arguments
        .as_object()
        .ok_or_else(|| RuntimeError::Plan("plan arguments must be an object".into()))?;
    let actions = parse_actions(arguments.get("actions"), "actions")?;
    let verification_commands = arguments
        .get("verification_commands")
        .and_then(|value| value.as_array())
        .ok_or_else(|| RuntimeError::Plan("verification_commands must be a non-empty list".into()))?
        .iter()
        .map(parse_command)
        .collect::<Result<Vec<_>, _>>()?;
    if verification_commands.is_empty() {
        return Err(RuntimeError::Plan(
            "verification_commands must be a non-empty list".into(),
        ));
    }
    let recovery_actions = match arguments.get("recovery_actions") {
        Some(value) => parse_actions(Some(value), "recovery_actions")?,
        None => Vec::new(),
    };
    Ok(ActionPlan {
        actions,
        verification_commands,
        recovery_actions,
    })
}

fn parse_actions(
    value: Option<&serde_json::Value>,
    field: &str,
) -> Result<Vec<secondego_core::ActionProposal>, RuntimeError> {
    let items = value
        .and_then(|value| value.as_array())
        .ok_or_else(|| RuntimeError::Plan(format!("{field} must be a list")))?;
    items
        .iter()
        .map(|item| {
            let object = item
                .as_object()
                .ok_or_else(|| RuntimeError::Plan(format!("{field} entries must be objects")))?;
            let action = object
                .get("action")
                .and_then(|value| value.as_str())
                .ok_or_else(|| RuntimeError::Plan(format!("{field} action must be a string")))?;
            let arguments = object
                .get("arguments")
                .and_then(|value| value.as_object())
                .ok_or_else(|| {
                    RuntimeError::Plan(format!("{field} arguments must be an object"))
                })?;
            let rationale = object
                .get("rationale")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            Ok(secondego_core::ActionProposal {
                action: action.into(),
                arguments: serde_json::Value::Object(arguments.clone()),
                rationale: rationale.into(),
            })
        })
        .collect()
}

fn parse_command(value: &serde_json::Value) -> Result<Vec<String>, RuntimeError> {
    let command = value
        .as_array()
        .ok_or_else(|| RuntimeError::Plan("verification command must be argv".into()))?;
    let command = command
        .iter()
        .map(|item| {
            item.as_str()
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    RuntimeError::Plan("verification argv entries must be non-empty strings".into())
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if command.is_empty() {
        return Err(RuntimeError::Plan(
            "verification command cannot be empty".into(),
        ));
    }
    Ok(command)
}

const PLAN_INSTRUCTION: &str = "Return exactly one submit_plan action with actions [{action,arguments,rationale}] and non-empty verification_commands as argv arrays. Use only read_file, search_code, edit_file, run_command, git_diff, git_status. Never return shell strings.";

#[cfg(test)]
mod tests {
    use super::*;
    use secondego_model::ScriptedProvider;
    use std::process::Command;
    use std::sync::{Arc, Mutex};

    #[test]
    fn scripted_plan_executes_in_worktree_and_transfers_only_after_verification() {
        let root = tempfile::tempdir().unwrap();
        git(root.path(), &["init", "-q"]);
        git(root.path(), &["config", "user.email", "test@example.com"]);
        git(root.path(), &["config", "user.name", "SecondEgo Test"]);
        git(root.path(), &["config", "commit.gpgsign", "false"]);
        std::fs::write(root.path().join("value.py"), "VALUE = 1\n").unwrap();
        git(root.path(), &["add", "value.py"]);
        git(root.path(), &["commit", "-qm", "initial"]);
        let proposal = secondego_core::ActionProposal {
            action: "submit_plan".into(),
            arguments: serde_json::json!({"actions":[{"action":"edit_file","arguments":{"path":"value.py","content":"VALUE = 2\n"},"rationale":"update value"}],"verification_commands":[["python3","-c","from pathlib import Path; assert Path('value.py').read_text() == 'VALUE = 2\\n'"]] }),
            rationale: "plan".into(),
        };
        let observed = Arc::new(Mutex::new(Vec::new()));
        let observed_for_sink = observed.clone();
        let mut engine =
            RustEngine::new(ScriptedProvider::new(vec![proposal])).with_event_sink(move |event| {
                observed_for_sink
                    .lock()
                    .unwrap()
                    .push(event.event_type.clone());
            });
        let report = engine.run("update value", root.path()).unwrap();
        assert!(report.verification_passed);
        assert_eq!(
            std::fs::read_to_string(root.path().join("value.py")).unwrap(),
            "VALUE = 2\n"
        );
        assert_eq!(report.state.status, TerminalStatus::Complete);
        assert_eq!(observed.lock().unwrap().len(), report.events.len());
    }

    #[test]
    fn failed_verification_uses_one_bounded_recovery_action() {
        let root = tempfile::tempdir().unwrap();
        git(root.path(), &["init", "-q"]);
        git(root.path(), &["config", "user.email", "test@example.com"]);
        git(root.path(), &["config", "user.name", "SecondEgo Test"]);
        git(root.path(), &["config", "commit.gpgsign", "false"]);
        std::fs::write(root.path().join("value.py"), "VALUE = 1\n").unwrap();
        git(root.path(), &["add", "value.py"]);
        git(root.path(), &["commit", "-qm", "initial"]);
        let proposal = secondego_core::ActionProposal {
            action: "submit_plan".into(),
            arguments: serde_json::json!({"actions":[{"action":"edit_file","arguments":{"path":"value.py","content":"VALUE = 3\n"},"rationale":"first attempt"}],"verification_commands":[["python3","-c","from pathlib import Path; assert Path('value.py').read_text() == 'VALUE = 2\\n'"]],"recovery_actions":[{"action":"edit_file","arguments":{"path":"value.py","content":"VALUE = 2\n"},"rationale":"correct the failed value"}]}),
            rationale: "plan".into(),
        };
        let mut engine = RustEngine::new(ScriptedProvider::new(vec![proposal]));
        let report = engine.run("repair value", root.path()).unwrap();
        assert!(report.verification_passed);
        assert_eq!(
            std::fs::read_to_string(root.path().join("value.py")).unwrap(),
            "VALUE = 2\n"
        );
        assert!(report.events.iter().any(|event| {
            event.event_type == "state.changed"
                && event
                    .payload
                    .get("current_phase")
                    .and_then(|value| value.as_str())
                    == Some("RECOVER")
        }));
    }

    fn git(cwd: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
