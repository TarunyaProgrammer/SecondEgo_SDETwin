use secondego_context::{
    ContextAssembler, ContextBudget, ContextPacket, EvidenceLedger, EvidenceRecord,
};
use secondego_core::{
    EngineEvent, ExecutionState, Phase, ResourceBudget, ResourceUsage, StateMachine, TerminalStatus,
};
use secondego_model::{ModelProvider, ProviderContext, ProviderError};
use secondego_repository::RepositoryIndexer;
use secondego_tools::{
    CommandPolicy, CommandRunner, FileTool, GitAttemptTransaction, PolicyError, SearchTool,
    ToolResult, ToolRouter, WorkspacePolicy, gc::OwnedTempLease,
};
use secondego_verification::{VerificationEngine, VerificationResult};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub mod discovery;
pub mod voice;

/// Cooperative cancellation shared by the gateway, terminal, and engine.
/// Long-running provider/tool calls remain bounded by their own timeouts, while
/// phase boundaries stop immediately and preserve a terminal cancellation event.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

const MAX_CLONE_BYTES: u64 = 1_000_000_000;

#[derive(Debug)]
pub struct ResolvedRepository {
    pub source: String,
    pub root: std::path::PathBuf,
    pub cloned: bool,
    _lease: Option<OwnedTempLease>,
}

/// Reclaim stale SecondEgo-owned temp directories before a new run starts.
pub fn collect_garbage() -> secondego_tools::gc::GcReport {
    secondego_tools::gc::collect_garbage()
}

/// Resolve a local path or acquire a bounded shallow clone of a public GitHub repository.
pub fn resolve_repository(source: impl AsRef<str>) -> Result<ResolvedRepository, RuntimeError> {
    resolve_repository_with_cancellation(source, None)
}

pub fn resolve_repository_with_cancellation(
    source: impl AsRef<str>,
    cancellation: Option<&CancellationToken>,
) -> Result<ResolvedRepository, RuntimeError> {
    let value = source.as_ref().trim();
    if value.is_empty() {
        return Err(RuntimeError::Repository(
            "repository path or GitHub URL is required".into(),
        ));
    }
    if let Some(path) = github_path(value)? {
        let target = unique_clone_target(&path.1)?;
        let clone_root = target
            .parent()
            .map(std::path::Path::to_path_buf)
            .ok_or_else(|| RuntimeError::Repository("invalid clone target".into()))?;
        let lease = OwnedTempLease::create(&clone_root, "remote").map_err(|error| {
            RuntimeError::Repository(format!("could not reserve clone directory: {error}"))
        })?;
        let clone_url = format!("https://github.com/{}/{}.git", path.0, path.1);
        let mut child = Command::new("git")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "clone",
                "--depth",
                "1",
                "--no-tags",
                "--single-branch",
                &clone_url,
            ])
            .arg(&target)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                RuntimeError::Repository(format!("could not start git clone: {error}"))
            })?;
        let deadline = Instant::now() + Duration::from_secs(180);
        loop {
            if cancellation.is_some_and(CancellationToken::is_cancelled) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RuntimeError::Cancelled);
            }
            if let Some(status) = child
                .try_wait()
                .map_err(|error| RuntimeError::Repository(format!("git clone failed: {error}")))?
            {
                if !status.success() {
                    let output = child.wait_with_output().map_err(|error| {
                        RuntimeError::Repository(format!("git clone failed: {error}"))
                    })?;
                    let detail = String::from_utf8_lossy(&output.stderr)
                        .lines()
                        .last()
                        .unwrap_or("git clone failed")
                        .chars()
                        .take(300)
                        .collect::<String>();
                    return Err(RuntimeError::Repository(format!(
                        "GitHub clone failed: {detail}"
                    )));
                }
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RuntimeError::Repository(
                    "GitHub clone timed out after 180 seconds".into(),
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if !target.join(".git").is_dir() {
            return Err(RuntimeError::Repository(
                "GitHub clone did not produce a Git repository".into(),
            ));
        }
        if directory_size(&target)? > MAX_CLONE_BYTES {
            return Err(RuntimeError::Repository(
                "cloned repository exceeds the 1 GB safety limit".into(),
            ));
        }
        return Ok(ResolvedRepository {
            source: value.into(),
            root: target,
            cloned: true,
            _lease: Some(lease),
        });
    }
    if value.contains("://") {
        return Err(RuntimeError::Repository(
            "only HTTPS GitHub repository URLs are supported".into(),
        ));
    }
    let root = std::path::PathBuf::from(value)
        .canonicalize()
        .map_err(|error| RuntimeError::Repository(error.to_string()))?;
    if !root.is_dir() {
        return Err(RuntimeError::Repository(
            "repository is not an existing directory".into(),
        ));
    }
    Ok(ResolvedRepository {
        source: value.into(),
        root,
        cloned: false,
        _lease: None,
    })
}

/// Validate a repository input without acquiring a remote clone.
pub fn validate_repository_source(source: impl AsRef<str>) -> Result<(), RuntimeError> {
    let value = source.as_ref().trim();
    if value.is_empty() {
        return Err(RuntimeError::Repository(
            "repository path or GitHub URL is required".into(),
        ));
    }
    if github_path(value)?.is_some() {
        return Ok(());
    }
    if value.contains("://") {
        return Err(RuntimeError::Repository(
            "only HTTPS GitHub repository URLs are supported".into(),
        ));
    }
    let root = std::path::PathBuf::from(value)
        .canonicalize()
        .map_err(|error| RuntimeError::Repository(error.to_string()))?;
    if !root.is_dir() {
        return Err(RuntimeError::Repository(
            "repository is not an existing directory".into(),
        ));
    }
    Ok(())
}

fn github_path(value: &str) -> Result<Option<(String, String)>, RuntimeError> {
    let Some(path) = value
        .strip_prefix("https://github.com/")
        .or_else(|| value.strip_prefix("https://www.github.com/"))
    else {
        return Ok(None);
    };
    if value.contains('@') || value.contains('?') || value.contains('#') {
        return Err(RuntimeError::Repository(
            "GitHub URL must not contain credentials or query parameters".into(),
        ));
    }
    let parts: Vec<_> = path.trim_matches('/').split('/').collect();
    if parts.len() != 2
        || parts
            .iter()
            .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return Err(RuntimeError::Repository(
            "GitHub URL must have the form https://github.com/owner/repository".into(),
        ));
    }
    let repository = parts[1].strip_suffix(".git").unwrap_or(parts[1]);
    if repository.is_empty() {
        return Err(RuntimeError::Repository(
            "GitHub URL contains an invalid repository".into(),
        ));
    }
    Ok(Some((parts[0].into(), repository.into())))
}

fn unique_clone_target(repository: &str) -> Result<std::path::PathBuf, RuntimeError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| RuntimeError::Repository(error.to_string()))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "secondego-remote-{}-{timestamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).map_err(|error| {
        RuntimeError::Repository(format!("could not create clone directory: {error}"))
    })?;
    Ok(root.join(repository))
}

fn directory_size(root: &Path) -> Result<u64, RuntimeError> {
    let mut total = 0_u64;
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory)
            .map_err(|error| RuntimeError::Repository(error.to_string()))?
        {
            let entry = entry.map_err(|error| RuntimeError::Repository(error.to_string()))?;
            let file_type = entry
                .file_type()
                .map_err(|error| RuntimeError::Repository(error.to_string()))?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                total = total.saturating_add(
                    entry
                        .metadata()
                        .map_err(|error| RuntimeError::Repository(error.to_string()))?
                        .len(),
                );
                if total > MAX_CLONE_BYTES {
                    return Ok(total);
                }
            }
        }
    }
    Ok(total)
}

#[derive(Debug)]
pub enum RuntimeError {
    Cancelled,
    Repository(String),
    Context(String),
    Resource(String),
    Provider(ProviderError),
    Plan(String),
    Tool(PolicyError),
    State(String),
    Transaction(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("run cancelled by user"),
            Self::Repository(message)
            | Self::Context(message)
            | Self::Resource(message)
            | Self::Plan(message)
            | Self::State(message)
            | Self::Transaction(message) => formatter.write_str(message),
            Self::Provider(error) => error.fmt(formatter),
            Self::Tool(error) => error.fmt(formatter),
        }
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

#[derive(Debug, Clone)]
struct RecoveryPlan {
    actions: Vec<secondego_core::ActionProposal>,
    additional_verification_commands: Vec<Vec<String>>,
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
    pub diff_transferred: bool,
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
    pub voice: Option<voice::VoiceService>,
    pub cancellation: Option<CancellationToken>,
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
            voice: None,
            cancellation: None,
        }
    }

    pub fn with_event_sink(mut self, sink: impl Fn(&EngineEvent) + Send + Sync + 'static) -> Self {
        self.event_sink = Some(Arc::new(sink));
        self
    }

    pub fn with_voice(mut self, voice: voice::VoiceService) -> Self {
        self.voice = Some(voice);
        self
    }

    pub fn with_cancellation(mut self, cancellation: CancellationToken) -> Self {
        self.cancellation = Some(cancellation);
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

        self.check_cancel(&mut machine, &mut events)?;
        self.activity(
            &mut events,
            &machine.state,
            "activity.started",
            "run",
            "Starting bounded repository run",
        );

        self.append_event(
            &mut events,
            machine
                .move_to(Phase::Understand, "initialize bounded execution")
                .map_err(|error| RuntimeError::State(error.to_string()))?,
        );
        self.check_cancel(&mut machine, &mut events)?;
        self.activity(
            &mut events,
            &machine.state,
            "activity.started",
            "repository.index",
            "Indexing repository",
        );
        let index = match RepositoryIndexer::new(&workspace).build() {
            Ok(index) => index,
            Err(error) => {
                let error = RuntimeError::Repository(error.to_string());
                self.activity(
                    &mut events,
                    &machine.state,
                    "activity.failed",
                    "repository.index",
                    &error.to_string(),
                );
                return self.terminate_error(&mut machine, &mut events, error);
            }
        };
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
        self.activity(
            &mut events,
            &machine.state,
            "activity.completed",
            "repository.index",
            &format!(
                "Indexed {} files and {} symbols",
                index.snapshot.files.len(),
                index.symbols.len()
            ),
        );
        self.append_event(
            &mut events,
            machine
                .move_to(Phase::Explore, "build repository index before planning")
                .map_err(|error| RuntimeError::State(error.to_string()))?,
        );
        self.check_cancel(&mut machine, &mut events)?;
        self.activity(
            &mut events,
            &machine.state,
            "activity.started",
            "repository.retrieve",
            "Selecting focused repository evidence",
        );
        let ranked = index.rank(&machine.state.task, 12, false);
        if !ranked.is_empty() {
            ledger.record(EvidenceRecord::new(
                "repository:retrieval",
                serde_json::to_string(&ranked).unwrap_or_default(),
                "explainable repository retrieval",
                4,
            ));
            for item in ranked.iter().take(INITIAL_EVIDENCE_FILES) {
                if !safe_initial_evidence_path(&item.path) {
                    continue;
                }
                let file_path = workspace.join(&item.path);
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    let excerpt: String = content.chars().take(2_500).collect();
                    ledger.record(EvidenceRecord::new(
                        format!("source:{}", item.path),
                        format!("path: {}\n```\n{}\n```", item.path, excerpt),
                        item.path.clone(),
                        3,
                    ));
                }
            }
        }
        self.activity(
            &mut events,
            &machine.state,
            "activity.completed",
            "repository.retrieve",
            &format!("Selected {} ranked repository candidates", ranked.len()),
        );
        self.append_event(
            &mut events,
            machine
                .move_to(Phase::Plan, "assemble focused indexed context")
                .map_err(|error| RuntimeError::State(error.to_string()))?,
        );
        let plan = self.collect_plan(
            &mut machine,
            &mut events,
            &mut ledger,
            &workspace,
            &mut tools_used,
        )?;
        self.activity(
            &mut events,
            &machine.state,
            "activity.completed",
            "plan.validate",
            &format!(
                "Validated {} actions and {} verification commands",
                plan.actions.len(),
                plan.verification_commands.len()
            ),
        );
        if plan.actions.len() > self.max_actions {
            return self.terminate_error(
                &mut machine,
                &mut events,
                RuntimeError::Plan("action count exceeds runtime limit".into()),
            );
        }

        self.check_cancel(&mut machine, &mut events)?;
        self.append_event(
            &mut events,
            machine
                .move_to(
                    Phase::Execute,
                    "execute validated actions in detached worktree",
                )
                .map_err(|error| RuntimeError::State(error.to_string()))?,
        );
        self.activity(
            &mut events,
            &machine.state,
            "activity.started",
            "execute.actions",
            &format!(
                "Executing {} validated actions in an isolated worktree",
                plan.actions.len()
            ),
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
        if let Err(error) = execute_actions(
            &mut self.resources,
            &router,
            &plan.actions,
            self.cancellation.as_ref(),
            &mut tools_used,
        ) {
            return self.abort_transaction_error(
                &mut transaction,
                &mut machine,
                &mut events,
                "execute.actions",
                error,
            );
        }
        self.activity(
            &mut events,
            &machine.state,
            "activity.completed",
            "execute.actions",
            "Validated actions completed",
        );
        if let Err(error) = self.check_cancel(&mut machine, &mut events) {
            transaction.abort();
            return Err(error);
        }
        self.append_event(
            &mut events,
            machine
                .move_to(Phase::Verify, "run bounded verification commands")
                .map_err(|error| RuntimeError::State(error.to_string()))?,
        );
        self.activity(
            &mut events,
            &machine.state,
            "activity.started",
            "verification.run",
            &format!(
                "Running {} verification command(s)",
                plan.verification_commands.len()
            ),
        );
        let verifier = VerificationEngine {
            runner: &router.runner,
        };
        for _ in &plan.verification_commands {
            if let Err(resource_error) = self.resources.record_tool_call() {
                return self.abort_transaction_error(
                    &mut transaction,
                    &mut machine,
                    &mut events,
                    "resource.tool",
                    RuntimeError::Resource(resource_error.to_string()),
                );
            }
        }
        let mut verification = verifier.run(&plan.verification_commands);
        self.activity(
            &mut events,
            &machine.state,
            "activity.completed",
            "verification.run",
            if verification.passed {
                "Verification passed"
            } else {
                "Verification produced a failure record"
            },
        );
        if !verification.passed {
            if let Err(error) = self.check_cancel(&mut machine, &mut events) {
                transaction.abort();
                return Err(error);
            }
            self.append_event(
                &mut events,
                machine
                    .move_to(
                        Phase::Diagnose,
                        "classify verification failure before recovery",
                    )
                    .map_err(|error| RuntimeError::State(error.to_string()))?,
            );
            let recovery = self.plan_adaptive_recovery(
                &mut machine,
                &mut events,
                &plan,
                &verification,
                &router,
                &mut tools_used,
            );
            let recovery = match recovery {
                Ok(recovery) => Some((
                    recovery.actions,
                    merge_verification_commands(
                        &plan.verification_commands,
                        &recovery.additional_verification_commands,
                    ),
                    "adaptive evidence-based recovery",
                )),
                Err(error)
                    if matches!(error, RuntimeError::Cancelled | RuntimeError::Resource(_)) =>
                {
                    return self.abort_transaction_error(
                        &mut transaction,
                        &mut machine,
                        &mut events,
                        "recovery.plan",
                        error,
                    );
                }
                Err(error) if !plan.recovery_actions.is_empty() => {
                    self.activity(
                        &mut events,
                        &machine.state,
                        "activity.retrying",
                        "recovery.fallback",
                        &format!(
                            "Adaptive recovery was unavailable ({error}); using the validated fallback actions from the original plan"
                        ),
                    );
                    Some((
                        plan.recovery_actions.clone(),
                        plan.verification_commands.clone(),
                        "pre-planned fallback recovery",
                    ))
                }
                Err(error) => {
                    self.activity(
                        &mut events,
                        &machine.state,
                        "activity.failed",
                        "recovery.plan",
                        &format!("No safe adaptive recovery was available: {error}"),
                    );
                    None
                }
            };
            if let Some((actions, verification_commands, strategy)) = recovery {
                if let Err(resource_error) = self.resources.record_retry() {
                    return self.abort_transaction_error(
                        &mut transaction,
                        &mut machine,
                        &mut events,
                        "resource.retry",
                        RuntimeError::Resource(resource_error.to_string()),
                    );
                }
                self.append_event(
                    &mut events,
                    machine
                        .move_to(Phase::Recover, strategy)
                        .map_err(|error| RuntimeError::State(error.to_string()))?,
                );
                self.activity(
                    &mut events,
                    &machine.state,
                    "activity.started",
                    "recovery.apply",
                    &format!("Applying {strategy}"),
                );
                if let Err(error) = execute_actions(
                    &mut self.resources,
                    &router,
                    &actions,
                    self.cancellation.as_ref(),
                    &mut tools_used,
                ) {
                    return self.abort_transaction_error(
                        &mut transaction,
                        &mut machine,
                        &mut events,
                        "recovery.apply",
                        error,
                    );
                }
                self.activity(
                    &mut events,
                    &machine.state,
                    "activity.completed",
                    "recovery.apply",
                    "Recovery actions completed",
                );
                if let Err(error) = self.check_cancel(&mut machine, &mut events) {
                    transaction.abort();
                    return Err(error);
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
                for _ in &verification_commands {
                    if let Err(resource_error) = self.resources.record_tool_call() {
                        return self.abort_transaction_error(
                            &mut transaction,
                            &mut machine,
                            &mut events,
                            "resource.tool",
                            RuntimeError::Resource(resource_error.to_string()),
                        );
                    }
                }
                verification = verifier.run(&verification_commands);
                self.activity(
                    &mut events,
                    &machine.state,
                    "activity.completed",
                    "verification.recheck",
                    if verification.passed {
                        "Recovery verification passed"
                    } else {
                        "Recovery verification still failed"
                    },
                );
            }
        }
        let passed = verification.passed;
        let transfer = match transaction.finish(passed) {
            Ok(transfer) => transfer,
            Err(error) => {
                let error = RuntimeError::Transaction(error.to_string());
                self.activity(
                    &mut events,
                    &machine.state,
                    "activity.failed",
                    "transaction.finish",
                    &error.to_string(),
                );
                return self.terminate_error(&mut machine, &mut events, error);
            }
        };
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
                diff_transferred: transfer.transferred,
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
            diff_transferred: transfer.transferred,
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
        let status = if matches!(error, RuntimeError::Cancelled) {
            TerminalStatus::Cancelled
        } else {
            TerminalStatus::Failed
        };
        let _ = machine
            .terminate(status, &error.to_string())
            .map(|event| self.append_event(events, event));
        Err(error)
    }

    fn abort_transaction_error<T>(
        &self,
        transaction: &mut GitAttemptTransaction,
        machine: &mut StateMachine,
        events: &mut Vec<EngineEvent>,
        operation: &str,
        error: RuntimeError,
    ) -> Result<T, RuntimeError> {
        transaction.abort();
        self.activity(
            events,
            &machine.state,
            "activity.failed",
            operation,
            &error.to_string(),
        );
        self.terminate_error(machine, events, error)
    }

    fn check_cancel(
        &self,
        machine: &mut StateMachine,
        events: &mut Vec<EngineEvent>,
    ) -> Result<(), RuntimeError> {
        if self
            .cancellation
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            return self.terminate_error(machine, events, RuntimeError::Cancelled);
        }
        Ok(())
    }

    fn activity(
        &self,
        events: &mut Vec<EngineEvent>,
        state: &ExecutionState,
        event_type: &str,
        operation: &str,
        message: &str,
    ) {
        let mut event = EngineEvent::new(state, event_type);
        event.payload = serde_json::json!({
            "operation": operation,
            "message": message,
            "resource_usage": self.resources.snapshot(),
        });
        self.append_event(events, event);
    }

    fn append_event(&self, events: &mut Vec<EngineEvent>, event: EngineEvent) {
        if let Some(voice) = &self.voice {
            voice.observe(&event);
        }
        if let Some(sink) = &self.event_sink {
            sink(&event);
        }
        events.push(event);
    }

    fn assemble_planning_packet(
        &self,
        machine: &StateMachine,
        ledger: &EvidenceLedger,
        inspections: usize,
        force_submission: bool,
    ) -> Result<ContextPacket, RuntimeError> {
        let state = serde_json::to_string(&machine.state)
            .map_err(|error| RuntimeError::Context(error.to_string()))?;
        let state = format!(
            "{state}\nPLANNING_INSPECTIONS={inspections}\nPLANNING_SUBMISSION_REQUIRED={force_submission}"
        );
        self.context
            .assemble(
                &machine.state.task,
                PLAN_INSTRUCTION,
                state,
                &ledger.active(),
            )
            .map_err(|error| RuntimeError::Context(format!("{error:?}")))
    }

    fn collect_plan(
        &mut self,
        machine: &mut StateMachine,
        events: &mut Vec<EngineEvent>,
        ledger: &mut EvidenceLedger,
        workspace: &Path,
        tool_results: &mut Vec<ToolResult>,
    ) -> Result<ActionPlan, RuntimeError> {
        self.activity(
            events,
            &machine.state,
            "activity.started",
            "context.assemble",
            "Assembling bounded planning context",
        );
        let mut inspections = 0;
        let mut force_submission = false;
        let mut packet =
            match self.assemble_planning_packet(machine, ledger, inspections, force_submission) {
                Ok(packet) => packet,
                Err(error) => {
                    self.activity(
                        events,
                        &machine.state,
                        "activity.failed",
                        "context.assemble",
                        &error.to_string(),
                    );
                    return self.terminate_error(machine, events, error);
                }
            };
        self.activity(
            events,
            &machine.state,
            "activity.completed",
            "context.assemble",
            &format!(
                "Prepared {} evidence records ({} estimated tokens)",
                packet.evidence.len(),
                packet.estimated_tokens
            ),
        );
        let planning_policy = match WorkspacePolicy::new(workspace) {
            Ok(policy) => policy,
            Err(error) => return self.terminate_error(machine, events, RuntimeError::Tool(error)),
        };
        let planning_router = ToolRouter {
            files: FileTool {
                workspace: planning_policy.clone(),
                max_file_bytes: 512 * 1024,
            },
            search: SearchTool {
                workspace: planning_policy.clone(),
                max_results: 80,
            },
            runner: CommandRunner {
                workspace: planning_policy,
                policy: CommandPolicy::default(),
            },
        };
        let mut format_attempt = 0;
        loop {
            self.check_cancel(machine, events)?;
            let mut prompt = packet.as_text();
            prompt.push_str(&format!(
                "\n\nPLANNING_TURN: {}/{}. You have used {inspections} read-only inspection(s).",
                inspections + 1,
                MAX_PLANNING_INSPECTIONS + 1
            ));
            if force_submission {
                prompt.push_str("\nNo further inspection requests are allowed. Submit the implementation plan now.");
            }
            if format_attempt > 0 {
                prompt.push_str("\n\n");
                prompt.push_str(PLAN_FORMAT_REPAIR_INSTRUCTION);
            }
            let operation = if format_attempt == 0 {
                "model.plan"
            } else {
                "model.plan.repair"
            };
            let message = if format_attempt == 0 {
                "Requesting a structured implementation plan or one read-only inspection"
            } else {
                "Requesting one corrected structured planning response"
            };
            let proposal =
                match self.request_proposal(machine, events, &prompt, "PLAN", operation, message) {
                    Ok(proposal) => proposal,
                    Err(error) => {
                        let retry = match self.schedule_protocol_repair(
                            machine,
                            events,
                            &error,
                            &mut format_attempt,
                            "model.plan.repair",
                        ) {
                            Ok(retry) => retry,
                            Err(error) => return self.terminate_error(machine, events, error),
                        };
                        if retry {
                            continue;
                        }
                        return self.terminate_error(machine, events, error);
                    }
                };
            if proposal.action == "submit_plan" {
                match parse_plan(proposal) {
                    Ok(plan) => {
                        self.activity(
                            events,
                            &machine.state,
                            "activity.completed",
                            operation,
                            "Structured plan received and validated",
                        );
                        return Ok(plan);
                    }
                    Err(error) => {
                        let retry = match self.schedule_protocol_repair(
                            machine,
                            events,
                            &error,
                            &mut format_attempt,
                            "model.plan.repair",
                        ) {
                            Ok(retry) => retry,
                            Err(error) => return self.terminate_error(machine, events, error),
                        };
                        if retry {
                            continue;
                        }
                        return self.terminate_error(machine, events, error);
                    }
                }
            }
            let proposal_error = validate_planning_proposal(&proposal, force_submission);
            if let Err(error) = proposal_error {
                let retry = match self.schedule_protocol_repair(
                    machine,
                    events,
                    &error,
                    &mut format_attempt,
                    "model.plan.repair",
                ) {
                    Ok(retry) => retry,
                    Err(error) => return self.terminate_error(machine, events, error),
                };
                if retry {
                    continue;
                }
                return self.terminate_error(machine, events, error);
            }
            if let Err(resource_error) = self.resources.record_tool_call() {
                let error = RuntimeError::Resource(resource_error.to_string());
                self.activity(
                    events,
                    &machine.state,
                    "activity.failed",
                    "resource.tool",
                    &error.to_string(),
                );
                return self.terminate_error(machine, events, error);
            }
            self.activity(
                events,
                &machine.state,
                "activity.started",
                "planning.inspect",
                &format!("Running read-only planning action {}", proposal.action),
            );
            let result = match planning_router.dispatch(&proposal) {
                Ok(result) => result,
                Err(error) => {
                    let error = RuntimeError::Tool(error);
                    let retry = match self.schedule_protocol_repair(
                        machine,
                        events,
                        &error,
                        &mut format_attempt,
                        "model.plan.repair",
                    ) {
                        Ok(retry) => retry,
                        Err(error) => return self.terminate_error(machine, events, error),
                    };
                    if retry {
                        continue;
                    }
                    return self.terminate_error(machine, events, error);
                }
            };
            self.activity(
                events,
                &machine.state,
                if result.success {
                    "activity.completed"
                } else {
                    "activity.failed"
                },
                "planning.inspect",
                &format!(
                    "{} planning inspection {}",
                    proposal.action,
                    if result.success {
                        "completed"
                    } else {
                        "returned an observation"
                    }
                ),
            );
            ledger.record(planning_observation(&proposal, &result, inspections + 1));
            tool_results.push(result);
            inspections += 1;
            force_submission = inspections >= MAX_PLANNING_INSPECTIONS;
            format_attempt = 0;
            packet =
                match self.assemble_planning_packet(machine, ledger, inspections, force_submission)
                {
                    Ok(packet) => packet,
                    Err(error) => return self.terminate_error(machine, events, error),
                };
            self.activity(
                events,
                &machine.state,
                "activity.completed",
                "context.refresh",
                &format!(
                    "Refreshed planning context after {} read-only inspection(s)",
                    inspections
                ),
            );
        }
    }

    fn request_proposal(
        &mut self,
        machine: &mut StateMachine,
        events: &mut Vec<EngineEvent>,
        prompt: &str,
        phase: &str,
        operation: &str,
        message: &str,
    ) -> Result<secondego_core::ActionProposal, RuntimeError> {
        for attempt in 0..MAX_TRANSIENT_PROVIDER_ATTEMPTS {
            self.check_cancel(machine, events)?;
            let estimated_tokens = self.provider.estimate_tokens(prompt) as u64;
            if let Err(resource_error) = self.resources.record_model_call(estimated_tokens) {
                let error = RuntimeError::Resource(resource_error.to_string());
                self.activity(
                    events,
                    &machine.state,
                    "activity.failed",
                    "resource.model",
                    &error.to_string(),
                );
                return Err(error);
            }
            self.activity(
                events,
                &machine.state,
                "activity.started",
                operation,
                message,
            );
            match self.provider.generate(
                prompt,
                &ProviderContext {
                    run_id: machine.state.run_id.to_string(),
                    phase: phase.into(),
                },
            ) {
                Ok(proposal) => return Ok(proposal),
                Err(provider_error) => {
                    let error = RuntimeError::Provider(provider_error);
                    self.activity(
                        events,
                        &machine.state,
                        "activity.failed",
                        operation,
                        &error.to_string(),
                    );
                    let transient =
                        matches!(&error, RuntimeError::Provider(error) if error.is_transient());
                    if transient && attempt + 1 < MAX_TRANSIENT_PROVIDER_ATTEMPTS {
                        if let Err(resource_error) = self.resources.record_retry() {
                            let error = RuntimeError::Resource(resource_error.to_string());
                            self.activity(
                                events,
                                &machine.state,
                                "activity.failed",
                                "resource.retry",
                                &error.to_string(),
                            );
                            return Err(error);
                        }
                        let delay = provider_retry_delay(attempt);
                        self.activity(
                            events,
                            &machine.state,
                            "activity.retrying",
                            "model.provider_retry",
                            &format!(
                                "Transient provider failure; retrying in {}ms ({}/{})",
                                delay.as_millis(),
                                attempt + 2,
                                MAX_TRANSIENT_PROVIDER_ATTEMPTS
                            ),
                        );
                        self.wait_for_provider_retry(machine, events, delay)?;
                        continue;
                    }
                    return Err(error);
                }
            }
        }
        Err(RuntimeError::Provider(ProviderError::ScriptExhausted))
    }

    fn wait_for_provider_retry(
        &self,
        machine: &mut StateMachine,
        events: &mut Vec<EngineEvent>,
        delay: Duration,
    ) -> Result<(), RuntimeError> {
        let deadline = Instant::now() + delay;
        while Instant::now() < deadline {
            self.check_cancel(machine, events)?;
            let remaining = deadline.saturating_duration_since(Instant::now());
            std::thread::sleep(remaining.min(Duration::from_millis(50)));
        }
        self.check_cancel(machine, events)
    }

    fn plan_adaptive_recovery(
        &mut self,
        machine: &mut StateMachine,
        events: &mut Vec<EngineEvent>,
        plan: &ActionPlan,
        verification: &VerificationResult,
        router: &ToolRouter,
        tool_results: &mut Vec<ToolResult>,
    ) -> Result<RecoveryPlan, RuntimeError> {
        self.resources
            .record_tool_call()
            .map_err(|error| RuntimeError::Resource(error.to_string()))?;
        let diff_proposal = secondego_core::ActionProposal {
            action: "git_diff".into(),
            arguments: serde_json::json!({}),
            rationale: "collect failed-attempt evidence".into(),
        };
        let diff = router
            .dispatch(&diff_proposal)
            .map_err(RuntimeError::Tool)?;
        let diff_text = if diff.success {
            compact_for_prompt(&diff.stdout, MAX_RECOVERY_DIFF_CHARS)
        } else {
            format!(
                "Diff unavailable: {}",
                compact_for_prompt(&diff.stderr, 1_000)
            )
        };
        tool_results.push(diff);
        let failure = serde_json::to_string(verification)
            .map(|value| compact_for_prompt(&value, MAX_RECOVERY_FAILURE_CHARS))
            .unwrap_or_else(|_| "Verification evidence could not be serialized".into());
        let commands = plan
            .verification_commands
            .iter()
            .map(|command| command.join(" "))
            .collect::<Vec<_>>()
            .join("\n");
        let base_prompt = format!(
            "{RECOVERY_INSTRUCTION}\n\nTASK:\n{}\n\nORIGINAL VERIFICATION COMMANDS:\n{}\n\nFAILED VERIFICATION EVIDENCE:\n{}\n\nCURRENT ATTEMPT DIFF:\n{}",
            machine.state.task, commands, failure, diff_text
        );
        let mut format_attempt = 0;
        loop {
            let prompt = if format_attempt == 0 {
                base_prompt.clone()
            } else {
                format!("{base_prompt}\n\n{RECOVERY_FORMAT_REPAIR_INSTRUCTION}")
            };
            let operation = if format_attempt == 0 {
                "model.recovery"
            } else {
                "model.recovery.repair"
            };
            let proposal = match self.request_proposal(
                machine,
                events,
                &prompt,
                "DIAGNOSE",
                operation,
                "Requesting an evidence-based recovery plan",
            ) {
                Ok(proposal) => proposal,
                Err(error) => {
                    if self.schedule_protocol_repair(
                        machine,
                        events,
                        &error,
                        &mut format_attempt,
                        "model.recovery.repair",
                    )? {
                        continue;
                    }
                    return Err(error);
                }
            };
            match parse_recovery_plan(proposal) {
                Ok(plan) => {
                    self.activity(
                        events,
                        &machine.state,
                        "activity.completed",
                        operation,
                        "Adaptive recovery plan received and validated",
                    );
                    return Ok(plan);
                }
                Err(error) => {
                    if self.schedule_protocol_repair(
                        machine,
                        events,
                        &error,
                        &mut format_attempt,
                        "model.recovery.repair",
                    )? {
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }

    fn schedule_protocol_repair(
        &mut self,
        machine: &mut StateMachine,
        events: &mut Vec<EngineEvent>,
        error: &RuntimeError,
        format_attempt: &mut usize,
        operation: &str,
    ) -> Result<bool, RuntimeError> {
        let failure_operation = if matches!(error, RuntimeError::Plan(_)) {
            "plan.validate"
        } else {
            operation
        };
        self.activity(
            events,
            &machine.state,
            "activity.failed",
            failure_operation,
            &error.to_string(),
        );
        if *format_attempt + 1 >= MAX_PLAN_FORMAT_ATTEMPTS
            || !is_recoverable_model_protocol_error(error)
        {
            return Ok(false);
        }
        if let Err(resource_error) = self.resources.record_retry() {
            let error = RuntimeError::Resource(resource_error.to_string());
            self.activity(
                events,
                &machine.state,
                "activity.failed",
                "resource.retry",
                &error.to_string(),
            );
            return Err(error);
        }
        *format_attempt += 1;
        self.activity(
            events,
            &machine.state,
            "activity.retrying",
            operation,
            "Model response did not meet the machine-readable contract; retrying once before repository changes",
        );
        Ok(true)
    }
}

fn execute_actions(
    resources: &mut ResourceUsage,
    router: &ToolRouter,
    actions: &[secondego_core::ActionProposal],
    cancellation: Option<&CancellationToken>,
    tool_results: &mut Vec<ToolResult>,
) -> Result<(), RuntimeError> {
    for action in actions {
        if cancellation.is_some_and(CancellationToken::is_cancelled) {
            return Err(RuntimeError::Cancelled);
        }
        resources
            .record_tool_call()
            .map_err(|error| RuntimeError::Resource(error.to_string()))?;
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

fn validate_planning_proposal(
    proposal: &secondego_core::ActionProposal,
    force_submission: bool,
) -> Result<(), RuntimeError> {
    if force_submission {
        return Err(RuntimeError::Plan(
            "planning inspection limit reached; provider must submit_plan".into(),
        ));
    }
    let arguments = proposal
        .arguments
        .as_object()
        .ok_or_else(|| RuntimeError::Plan("planning arguments must be an object".into()))?;
    let field = match proposal.action.as_str() {
        "read_file" => "path",
        "search_code" => "query",
        unsupported => {
            return Err(RuntimeError::Plan(format!(
                "planning action '{unsupported}' is not read-only; use read_file, search_code, or submit_plan"
            )));
        }
    };
    let value = arguments
        .get(field)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            RuntimeError::Plan(format!("planning {field} must be a non-empty string"))
        })?;
    if value.len() > 1_024 {
        return Err(RuntimeError::Plan(format!(
            "planning {field} exceeds 1024 characters"
        )));
    }
    Ok(())
}

fn planning_observation(
    proposal: &secondego_core::ActionProposal,
    result: &ToolResult,
    sequence: usize,
) -> EvidenceRecord {
    let target = proposal
        .arguments
        .get(if proposal.action == "read_file" {
            "path"
        } else {
            "query"
        })
        .and_then(|value| value.as_str())
        .unwrap_or("<invalid>");
    let output = if result.success {
        compact_for_prompt(&result.stdout, MAX_PLANNING_OBSERVATION_CHARS)
    } else {
        compact_for_prompt(&result.stderr, MAX_PLANNING_OBSERVATION_CHARS)
    };
    EvidenceRecord::new(
        format!("planning:{sequence}:{}", proposal.action),
        format!(
            "tool={} target={} success={} truncated={}\n{}",
            result.tool, target, result.success, result.truncated, output
        ),
        format!("planning observation: {target}"),
        5,
    )
}

fn compact_for_prompt(value: &str, limit: usize) -> String {
    let mut compact = value.chars().take(limit).collect::<String>();
    if value.chars().count() > limit {
        compact.push_str("\n[truncated]");
    }
    compact
}

fn safe_initial_evidence_path(path: &str) -> bool {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    !path.split('/').any(|part| part == ".git")
        && file_name != ".env"
        && !file_name.starts_with(".env.")
        && !matches!(
            file_name,
            "credentials.json" | "secrets.json" | "id_rsa" | "id_ed25519"
        )
}

fn provider_retry_delay(attempt: usize) -> Duration {
    let multiplier = 1_u64 << attempt.min(3);
    Duration::from_millis(400_u64.saturating_mul(multiplier))
}

fn merge_verification_commands(
    original: &[Vec<String>],
    additional: &[Vec<String>],
) -> Vec<Vec<String>> {
    let mut merged = original.to_vec();
    for command in additional {
        if !merged.contains(command) {
            merged.push(command.clone());
        }
    }
    merged
}

fn is_recoverable_model_protocol_error(error: &RuntimeError) -> bool {
    matches!(
        error,
        RuntimeError::Provider(ProviderError::InvalidResponse | ProviderError::InvalidAction(_))
            | RuntimeError::Plan(_)
    )
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

fn parse_recovery_plan(
    proposal: secondego_core::ActionProposal,
) -> Result<RecoveryPlan, RuntimeError> {
    if proposal.action == "submit_plan" {
        let plan = parse_plan(proposal)?;
        if plan.actions.is_empty() {
            return Err(RuntimeError::Plan(
                "recovery actions cannot be empty".into(),
            ));
        }
        return Ok(RecoveryPlan {
            actions: plan.actions,
            additional_verification_commands: plan.verification_commands,
        });
    }
    if proposal.action != "submit_recovery" {
        return Err(RuntimeError::Plan(
            "provider must return submit_recovery or submit_plan during diagnosis".into(),
        ));
    }
    let arguments = proposal
        .arguments
        .as_object()
        .ok_or_else(|| RuntimeError::Plan("recovery arguments must be an object".into()))?;
    let actions = parse_actions(arguments.get("actions"), "recovery actions")?;
    if actions.is_empty() {
        return Err(RuntimeError::Plan(
            "recovery actions cannot be empty".into(),
        ));
    }
    let additional_verification_commands = match arguments.get("verification_commands") {
        Some(value) => value
            .as_array()
            .ok_or_else(|| {
                RuntimeError::Plan("recovery verification_commands must be a list".into())
            })?
            .iter()
            .map(parse_command)
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    Ok(RecoveryPlan {
        actions,
        additional_verification_commands,
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

const INITIAL_EVIDENCE_FILES: usize = 6;
const MAX_PLANNING_INSPECTIONS: usize = 3;
const MAX_PLAN_FORMAT_ATTEMPTS: usize = 2;
const MAX_TRANSIENT_PROVIDER_ATTEMPTS: usize = 3;
const MAX_PLANNING_OBSERVATION_CHARS: usize = 6_000;
const MAX_RECOVERY_FAILURE_CHARS: usize = 8_000;
const MAX_RECOVERY_DIFF_CHARS: usize = 8_000;
const PLAN_INSTRUCTION: &str = "Return only one JSON object and no Markdown. During bounded planning, choose exactly one top-level action:\n1. read_file with arguments {path:string} to inspect one safe repository file;\n2. search_code with arguments {query:string} to inspect bounded repository matches;\n3. submit_plan with arguments {actions:[{action,arguments,rationale}], verification_commands:[[argv...]], optional recovery_actions:[...]}.\nPlanning read_file/search_code actions are read-only. After the inspection limit, you must use submit_plan. For submit_plan, use apply_patch for existing-file changes; use write_file only for genuinely new files. Allowed executable actions are read_file, search_code, apply_patch, edit_file, write_file, run_command, git_diff, git_status. Never return shell strings or Markdown.\nSDETwin Directives:\n1. Multi-File: When an issue spans multiple files or requires updating dependencies/callers, include actions for all necessary files.\n2. Autonomous Test Verification: Include a focused regression test where appropriate.";
const PLAN_FORMAT_REPAIR_INSTRUCTION: &str = "FORMAT REPAIR: Return only one JSON object, with no Markdown or prose. It must be either a read-only planning request {\"action\":\"read_file\",\"arguments\":{\"path\":\"...\"},\"rationale\":\"...\"}, a read-only search request {\"action\":\"search_code\",\"arguments\":{\"query\":\"...\"},\"rationale\":\"...\"}, or a final plan {\"action\":\"submit_plan\",\"arguments\":{\"actions\":[{\"action\":\"...\",\"arguments\":{},\"rationale\":\"...\"}],\"verification_commands\":[[\"command\",\"arg\"]],\"recovery_actions\":[]},\"rationale\":\"plain string\"}.";
const RECOVERY_INSTRUCTION: &str = "You are in the DIAGNOSE phase of a bounded coding run. Inspect the failed verification evidence and current isolated-worktree diff. Return only one JSON object with top-level action submit_recovery and arguments {actions:[{action,arguments,rationale}], optional verification_commands:[[argv...]]}. Actions must be evidence-based, minimal, and safe. Use apply_patch for existing files. Do not repeat unchanged actions, do not narrate, and do not return Markdown.";
const RECOVERY_FORMAT_REPAIR_INSTRUCTION: &str = "FORMAT REPAIR: Return only {\"action\":\"submit_recovery\",\"arguments\":{\"actions\":[{\"action\":\"apply_patch\",\"arguments\":{\"patch\":\"...\"},\"rationale\":\"...\"}],\"verification_commands\":[[\"command\",\"arg\"]]},\"rationale\":\"plain string\"}.";

#[cfg(test)]
mod tests {
    use super::*;
    use secondego_core::ActionProposal;
    use secondego_model::ScriptedProvider;
    use std::collections::VecDeque;
    use std::process::Command;
    use std::sync::{Arc, Mutex, atomic::AtomicUsize};

    #[derive(Clone)]
    struct SequenceProvider {
        responses: Arc<Mutex<VecDeque<Result<ActionProposal, ProviderError>>>>,
        calls: Arc<AtomicUsize>,
        prompts: Arc<Mutex<Vec<String>>>,
    }

    impl SequenceProvider {
        fn new(responses: Vec<Result<ActionProposal, ProviderError>>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses.into())),
                calls: Arc::new(AtomicUsize::new(0)),
                prompts: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.load(Ordering::Acquire)
        }

        fn prompts(&self) -> Vec<String> {
            self.prompts.lock().unwrap().clone()
        }
    }

    impl ModelProvider for SequenceProvider {
        fn generate(
            &self,
            prompt: &str,
            _context: &ProviderContext,
        ) -> Result<ActionProposal, ProviderError> {
            self.calls.fetch_add(1, Ordering::AcqRel);
            self.prompts.lock().unwrap().push(prompt.into());
            self.responses
                .lock()
                .map_err(|_| ProviderError::ScriptExhausted)?
                .pop_front()
                .unwrap_or(Err(ProviderError::ScriptExhausted))
        }
    }

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
        assert!(report.events.iter().any(|event| {
            event.event_type == "activity.started"
                && event
                    .payload
                    .get("operation")
                    .and_then(|value| value.as_str())
                    == Some("model.plan")
        }));
    }

    #[test]
    fn recoverable_provider_format_error_is_repaired_before_any_worktree_action() {
        let root = tempfile::tempdir().unwrap();
        git(root.path(), &["init", "-q"]);
        git(root.path(), &["config", "user.email", "test@example.com"]);
        git(root.path(), &["config", "user.name", "SecondEgo Test"]);
        git(root.path(), &["config", "commit.gpgsign", "false"]);
        std::fs::write(root.path().join("value.py"), "VALUE = 1\n").unwrap();
        git(root.path(), &["add", "value.py"]);
        git(root.path(), &["commit", "-qm", "initial"]);
        let proposal = ActionProposal {
            action: "submit_plan".into(),
            arguments: serde_json::json!({
                "actions": [{
                    "action": "edit_file",
                    "arguments": {"path": "value.py", "content": "VALUE = 2\n"},
                    "rationale": "update value"
                }],
                "verification_commands": [["python3", "-c", "from pathlib import Path; assert Path('value.py').read_text() == 'VALUE = 2\\n'"]]
            }),
            rationale: "plan".into(),
        };
        let provider = SequenceProvider::new(vec![
            Err(ProviderError::InvalidAction(
                "rationale must be a string".into(),
            )),
            Ok(proposal),
        ]);
        let mut engine = RustEngine::new(provider.clone());
        let report = engine.run("update value", root.path()).unwrap();

        assert!(report.verification_passed);
        assert_eq!(provider.call_count(), 2);
        assert!(provider.prompts()[1].contains("FORMAT REPAIR"));
        assert_eq!(report.resource_usage.get("model_calls"), Some(&2));
        assert_eq!(report.resource_usage.get("retries"), Some(&1));
        assert!(report.events.iter().any(|event| {
            event.event_type == "activity.retrying"
                && event
                    .payload
                    .get("operation")
                    .and_then(|value| value.as_str())
                    == Some("model.plan.repair")
        }));
        let repair_event = report
            .events
            .iter()
            .position(|event| event.event_type == "activity.retrying")
            .unwrap();
        let execution_event = report
            .events
            .iter()
            .position(|event| {
                event.event_type == "activity.started"
                    && event
                        .payload
                        .get("operation")
                        .and_then(|value| value.as_str())
                        == Some("execute.actions")
            })
            .unwrap();
        assert!(repair_event < execution_event);
    }

    #[test]
    fn planning_can_request_read_only_context_before_submitting_a_plan() {
        let root = initialized_repository("VALUE = 1\n");
        std::fs::write(
            root.path().join("helper.py"),
            "def current_value(): return 1\n",
        )
        .unwrap();
        git(root.path(), &["add", "helper.py"]);
        git(root.path(), &["commit", "-qm", "helper"]);
        let read = ActionProposal {
            action: "read_file".into(),
            arguments: serde_json::json!({"path": "helper.py"}),
            rationale: "inspect the helper before editing".into(),
        };
        let plan = plan_with_edit("VALUE = 2\n", "VALUE = 2\\n");
        let provider = SequenceProvider::new(vec![Ok(read), Ok(plan)]);
        let mut engine = RustEngine::new(provider.clone());
        let report = engine
            .run("update the value using the helper", root.path())
            .unwrap();

        assert!(report.verification_passed);
        assert_eq!(provider.call_count(), 2);
        assert!(provider.prompts()[1].contains("planning:1:read_file"));
        assert!(
            report
                .tool_results
                .iter()
                .any(|result| result.tool == "read_file")
        );
        assert!(report.events.iter().any(|event| {
            event
                .payload
                .get("operation")
                .and_then(|value| value.as_str())
                == Some("planning.inspect")
        }));
    }

    #[test]
    fn transient_provider_failure_retries_before_repository_execution() {
        let root = initialized_repository("VALUE = 1\n");
        let plan = plan_with_edit("VALUE = 2\n", "VALUE = 2\\n");
        let provider = SequenceProvider::new(vec![
            Err(ProviderError::HttpStatus {
                provider: "Gemini".into(),
                status: 503,
            }),
            Ok(plan),
        ]);
        let mut engine = RustEngine::new(provider.clone());
        let report = engine.run("update value", root.path()).unwrap();

        assert!(report.verification_passed);
        assert_eq!(provider.call_count(), 2);
        assert_eq!(report.resource_usage.get("retries"), Some(&1));
        assert!(report.events.iter().any(|event| {
            event.event_type == "activity.retrying"
                && event
                    .payload
                    .get("operation")
                    .and_then(|value| value.as_str())
                    == Some("model.provider_retry")
        }));
    }

    #[test]
    fn adaptive_recovery_uses_actual_failure_evidence_and_diff() {
        let root = initialized_repository("VALUE = 1\n");
        let initial = ActionProposal {
            action: "submit_plan".into(),
            arguments: serde_json::json!({
                "actions": [{
                    "action": "edit_file",
                    "arguments": {"path": "value.py", "content": "VALUE = 3\n"},
                    "rationale": "first attempt"
                }],
                "verification_commands": [["python3", "-c", "from pathlib import Path; assert Path('value.py').read_text() == 'VALUE = 2\\n'"]]
            }),
            rationale: "initial plan".into(),
        };
        let recovery = ActionProposal {
            action: "submit_recovery".into(),
            arguments: serde_json::json!({
                "actions": [{
                    "action": "edit_file",
                    "arguments": {"path": "value.py", "content": "VALUE = 2\n"},
                    "rationale": "use the assertion evidence"
                }]
            }),
            rationale: "repair value".into(),
        };
        let provider = SequenceProvider::new(vec![Ok(initial), Ok(recovery)]);
        let mut engine = RustEngine::new(provider.clone());
        let report = engine.run("repair value", root.path()).unwrap();

        assert!(report.verification_passed);
        assert_eq!(provider.call_count(), 2);
        assert!(provider.prompts()[1].contains("FAILED VERIFICATION EVIDENCE"));
        assert!(provider.prompts()[1].contains("CURRENT ATTEMPT DIFF"));
        assert_eq!(
            std::fs::read_to_string(root.path().join("value.py")).unwrap(),
            "VALUE = 2\n"
        );
    }

    #[test]
    fn invalid_plan_shape_is_repaired_once_before_execution() {
        let root = tempfile::tempdir().unwrap();
        git(root.path(), &["init", "-q"]);
        git(root.path(), &["config", "user.email", "test@example.com"]);
        git(root.path(), &["config", "user.name", "SecondEgo Test"]);
        git(root.path(), &["config", "commit.gpgsign", "false"]);
        std::fs::write(root.path().join("value.py"), "VALUE = 1\n").unwrap();
        git(root.path(), &["add", "value.py"]);
        git(root.path(), &["commit", "-qm", "initial"]);
        let invalid = ActionProposal {
            action: "edit_file".into(),
            arguments: serde_json::json!({"path": "value.py", "content": "VALUE = 999\n"}),
            rationale: "not a plan".into(),
        };
        let valid = ActionProposal {
            action: "submit_plan".into(),
            arguments: serde_json::json!({
                "actions": [{
                    "action": "edit_file",
                    "arguments": {"path": "value.py", "content": "VALUE = 2\n"},
                    "rationale": "update value"
                }],
                "verification_commands": [["python3", "-c", "from pathlib import Path; assert Path('value.py').read_text() == 'VALUE = 2\\n'"]]
            }),
            rationale: "plan".into(),
        };
        let provider = SequenceProvider::new(vec![Ok(invalid), Ok(valid)]);
        let mut engine = RustEngine::new(provider.clone());
        let report = engine.run("update value", root.path()).unwrap();

        assert!(report.verification_passed);
        assert_eq!(provider.call_count(), 2);
        assert_eq!(report.resource_usage.get("retries"), Some(&1));
        assert_eq!(
            std::fs::read_to_string(root.path().join("value.py")).unwrap(),
            "VALUE = 2\n"
        );
        assert!(report.events.iter().any(|event| {
            event.event_type == "activity.failed"
                && event
                    .payload
                    .get("operation")
                    .and_then(|value| value.as_str())
                    == Some("plan.validate")
        }));
    }

    #[test]
    fn plan_format_repair_is_bounded_and_never_starts_execution_when_it_fails() {
        let root = tempfile::tempdir().unwrap();
        git(root.path(), &["init", "-q"]);
        git(root.path(), &["config", "user.email", "test@example.com"]);
        git(root.path(), &["config", "user.name", "SecondEgo Test"]);
        git(root.path(), &["config", "commit.gpgsign", "false"]);
        std::fs::write(root.path().join("value.py"), "VALUE = 1\n").unwrap();
        git(root.path(), &["add", "value.py"]);
        git(root.path(), &["commit", "-qm", "initial"]);
        let provider = SequenceProvider::new(vec![
            Err(ProviderError::InvalidResponse),
            Err(ProviderError::InvalidAction("missing action".into())),
            Ok(ActionProposal {
                action: "submit_plan".into(),
                arguments: serde_json::json!({}),
                rationale: "must not be requested".into(),
            }),
        ]);
        let mut engine = RustEngine::new(provider.clone());
        let error = engine.run("update value", root.path()).unwrap_err();

        assert!(matches!(
            error,
            RuntimeError::Provider(ProviderError::InvalidAction(_))
        ));
        assert_eq!(provider.call_count(), MAX_PLAN_FORMAT_ATTEMPTS);
        assert_eq!(
            std::fs::read_to_string(root.path().join("value.py")).unwrap(),
            "VALUE = 1\n"
        );
    }

    #[test]
    fn resource_exhaustion_emits_a_terminal_event_and_discards_the_worktree() {
        let root = tempfile::tempdir().unwrap();
        git(root.path(), &["init", "-q"]);
        git(root.path(), &["config", "user.email", "test@example.com"]);
        git(root.path(), &["config", "user.name", "SecondEgo Test"]);
        git(root.path(), &["config", "commit.gpgsign", "false"]);
        std::fs::write(root.path().join("value.py"), "VALUE = 1\n").unwrap();
        git(root.path(), &["add", "value.py"]);
        git(root.path(), &["commit", "-qm", "initial"]);
        let proposal = ActionProposal {
            action: "submit_plan".into(),
            arguments: serde_json::json!({
                "actions": [],
                "verification_commands": [["python3", "-c", "assert True"]]
            }),
            rationale: "plan".into(),
        };
        let observed = Arc::new(Mutex::new(Vec::new()));
        let observed_for_sink = observed.clone();
        let mut engine =
            RustEngine::new(ScriptedProvider::new(vec![proposal])).with_event_sink(move |event| {
                observed_for_sink
                    .lock()
                    .unwrap()
                    .push(event.event_type.clone())
            });
        engine.resources.budget.max_tool_calls = 0;
        let error = engine.run("verify safely", root.path()).unwrap_err();

        assert!(matches!(error, RuntimeError::Resource(_)));
        assert!(
            observed
                .lock()
                .unwrap()
                .iter()
                .any(|event_type| event_type == "run.terminated")
        );
        assert_eq!(
            std::fs::read_to_string(root.path().join("value.py")).unwrap(),
            "VALUE = 1\n"
        );
        let worktrees = Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(root.path())
            .output()
            .unwrap();
        assert!(worktrees.status.success());
        assert!(!String::from_utf8_lossy(&worktrees.stdout).contains("secondego-attempt-"));
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

    #[test]
    fn cancellation_is_terminal_and_does_not_enter_execution() {
        let root = tempfile::tempdir().unwrap();
        git(root.path(), &["init", "-q"]);
        git(root.path(), &["config", "user.email", "test@example.com"]);
        git(root.path(), &["config", "user.name", "SecondEgo Test"]);
        git(root.path(), &["config", "commit.gpgsign", "false"]);
        std::fs::write(root.path().join("value.py"), "VALUE = 1\n").unwrap();
        git(root.path(), &["add", "value.py"]);
        git(root.path(), &["commit", "-qm", "initial"]);
        let cancellation = CancellationToken::default();
        cancellation.cancel();
        let mut engine =
            RustEngine::new(ScriptedProvider::new(Vec::new())).with_cancellation(cancellation);
        let error = engine.run("cancel me", root.path()).unwrap_err();
        assert!(matches!(error, RuntimeError::Cancelled));
    }

    fn initialized_repository(value: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        git(root.path(), &["init", "-q"]);
        git(root.path(), &["config", "user.email", "test@example.com"]);
        git(root.path(), &["config", "user.name", "SecondEgo Test"]);
        git(root.path(), &["config", "commit.gpgsign", "false"]);
        std::fs::write(root.path().join("value.py"), value).unwrap();
        git(root.path(), &["add", "value.py"]);
        git(root.path(), &["commit", "-qm", "initial"]);
        root
    }

    fn plan_with_edit(content: &str, expected_escaped: &str) -> ActionProposal {
        ActionProposal {
            action: "submit_plan".into(),
            arguments: serde_json::json!({
                "actions": [{
                    "action": "edit_file",
                    "arguments": {"path": "value.py", "content": content},
                    "rationale": "update value"
                }],
                "verification_commands": [[
                    "python3",
                    "-c",
                    format!("from pathlib import Path; assert Path('value.py').read_text() == '{expected_escaped}'")
                ]]
            }),
            rationale: "plan".into(),
        }
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
