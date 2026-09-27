use secondego_context::{ContextAssembler, ContextBudget, EvidenceLedger, EvidenceRecord};
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
            cancellation: None,
        }
    }

    pub fn with_event_sink(mut self, sink: impl Fn(&EngineEvent) + Send + Sync + 'static) -> Self {
        self.event_sink = Some(Arc::new(sink));
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
            for item in ranked.iter().take(4) {
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
        self.check_cancel(&mut machine, &mut events)?;
        self.activity(
            &mut events,
            &machine.state,
            "activity.started",
            "context.assemble",
            "Assembling bounded planning context",
        );
        let state_json = match serde_json::to_string(&machine.state) {
            Ok(value) => value,
            Err(error) => {
                let error = RuntimeError::Context(error.to_string());
                self.activity(
                    &mut events,
                    &machine.state,
                    "activity.failed",
                    "context.assemble",
                    &error.to_string(),
                );
                return self.terminate_error(&mut machine, &mut events, error);
            }
        };
        let packet = match self.context.assemble(
            &machine.state.task,
            PLAN_INSTRUCTION,
            state_json,
            &ledger.active(),
        ) {
            Ok(packet) => packet,
            Err(error) => {
                let error = RuntimeError::Context(format!("{error:?}"));
                self.activity(
                    &mut events,
                    &machine.state,
                    "activity.failed",
                    "context.assemble",
                    &error.to_string(),
                );
                return self.terminate_error(&mut machine, &mut events, error);
            }
        };
        self.activity(
            &mut events,
            &machine.state,
            "activity.completed",
            "context.assemble",
            &format!(
                "Prepared {} evidence records ({} estimated tokens)",
                packet.evidence.len(),
                packet.estimated_tokens
            ),
        );
        // Planning is the last boundary before a worktree is created. A
        // provider-format issue may be repaired once, but authentication,
        // transport, cancellation, and unsafe plans always fail explicitly.
        let plan_prompt = packet.as_text();
        let mut validated_plan = None;
        for attempt in 0..MAX_PLAN_FORMAT_ATTEMPTS {
            self.check_cancel(&mut machine, &mut events)?;
            let (operation, message, prompt) = if attempt == 0 {
                (
                    "model.plan",
                    "Requesting a structured implementation plan",
                    plan_prompt.clone(),
                )
            } else {
                (
                    "model.plan.repair",
                    "Requesting one corrected structured implementation plan",
                    format!("{plan_prompt}\n\n{PLAN_FORMAT_REPAIR_INSTRUCTION}"),
                )
            };
            let estimated_tokens = self.provider.estimate_tokens(&prompt) as u64;
            if let Err(resource_error) = self.resources.record_model_call(estimated_tokens) {
                let error = RuntimeError::Resource(resource_error.to_string());
                self.activity(
                    &mut events,
                    &machine.state,
                    "activity.failed",
                    "resource.model",
                    &error.to_string(),
                );
                return self.terminate_error(&mut machine, &mut events, error);
            }
            self.activity(
                &mut events,
                &machine.state,
                "activity.started",
                operation,
                message,
            );
            let result = self
                .provider
                .generate(
                    &prompt,
                    &ProviderContext {
                        run_id: machine.state.run_id.to_string(),
                        phase: "PLAN".into(),
                    },
                )
                .map_err(RuntimeError::Provider)
                .and_then(parse_plan);
            match result {
                Ok(plan) => {
                    self.activity(
                        &mut events,
                        &machine.state,
                        "activity.completed",
                        operation,
                        "Structured plan received and validated",
                    );
                    validated_plan = Some(plan);
                    break;
                }
                Err(error) => {
                    let failure_operation = if matches!(error, RuntimeError::Plan(_)) {
                        "plan.validate"
                    } else {
                        operation
                    };
                    self.activity(
                        &mut events,
                        &machine.state,
                        "activity.failed",
                        failure_operation,
                        &error.to_string(),
                    );
                    if attempt + 1 < MAX_PLAN_FORMAT_ATTEMPTS
                        && is_recoverable_model_protocol_error(&error)
                    {
                        if let Err(resource_error) = self.resources.record_retry() {
                            let error = RuntimeError::Resource(resource_error.to_string());
                            self.activity(
                                &mut events,
                                &machine.state,
                                "activity.failed",
                                "resource.retry",
                                &error.to_string(),
                            );
                            return self.terminate_error(&mut machine, &mut events, error);
                        }
                        self.activity(
                            &mut events,
                            &machine.state,
                            "activity.retrying",
                            "model.plan.repair",
                            "Model response did not meet the plan contract; retrying once before repository changes",
                        );
                        continue;
                    }
                    return self.terminate_error(&mut machine, &mut events, error);
                }
            }
        }
        let plan = validated_plan.expect("plan loop returns an error or validates a plan");
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
        if !verification.passed && !plan.recovery_actions.is_empty() {
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
                    .move_to(Phase::Recover, "apply one bounded, model-proposed repair")
                    .map_err(|error| RuntimeError::State(error.to_string()))?,
            );
            self.activity(
                &mut events,
                &machine.state,
                "activity.started",
                "recovery.apply",
                "Applying one bounded recovery plan",
            );
            if let Err(error) = execute_actions(
                &mut self.resources,
                &router,
                &plan.recovery_actions,
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
            verification = verifier.run(&plan.verification_commands);
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

const MAX_PLAN_FORMAT_ATTEMPTS: usize = 2;
const PLAN_INSTRUCTION: &str = "Return only one JSON object. Its top-level action must be the string `submit_plan`; arguments must be an object containing actions (a list of {action,arguments,rationale}), verification_commands (a non-empty list of argv arrays), and optional recovery_actions; top-level rationale must be a plain string. Use only read_file, search_code, edit_file, write_file, run_command, git_diff, git_status. Never return shell strings or Markdown.\nSDETwin Directives:\n1. Multi-File: When an issue spans multiple files or requires updating dependencies/callers, include actions for all necessary files.\n2. Autonomous Test Verification: If appropriate for the issue, include a focused reproduction test in tests/ to verify the fix and prevent regressions.";
const PLAN_FORMAT_REPAIR_INSTRUCTION: &str = "FORMAT REPAIR: The previous response did not meet the machine-readable plan contract. Return only one JSON object, with no Markdown or prose. Required exact top-level shape: {\"action\":\"submit_plan\",\"arguments\":{\"actions\":[{\"action\":\"...\",\"arguments\":{},\"rationale\":\"...\"}],\"verification_commands\":[[\"command\",\"arg\"]],\"recovery_actions\":[]},\"rationale\":\"plain string\"}. All action and rationale fields must be strings; all arguments fields must be objects; verification commands must be argv arrays.";

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
