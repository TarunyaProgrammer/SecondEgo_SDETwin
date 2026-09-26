use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use secondego_core::ActionProposal;
use serde::{Deserialize, Serialize};

pub mod gc;

const DEFAULT_MAX_FILE_BYTES: u64 = 512 * 1024;
const DEFAULT_MAX_OUTPUT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    WorkspaceNotDirectory,
    PathEscapesWorkspace,
    EmptyCommand,
    ExecutableNotAllowlisted(String),
    InvalidTimeout,
    ArgumentContainsNul,
    FileTooLarge,
    UnsupportedAction(String),
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PolicyError {}

#[derive(Debug, Clone)]
pub struct WorkspacePolicy {
    pub root: PathBuf,
}

impl WorkspacePolicy {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, PolicyError> {
        let root = root
            .into()
            .canonicalize()
            .map_err(|_| PolicyError::WorkspaceNotDirectory)?;
        if !root.is_dir() {
            return Err(PolicyError::WorkspaceNotDirectory);
        }
        Ok(Self { root })
    }

    pub fn resolve(&self, requested: impl AsRef<Path>) -> Result<PathBuf, PolicyError> {
        let requested = requested.as_ref();
        let candidate = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.root.join(requested)
        };
        let resolved = canonicalize_for_write(&candidate);
        if resolved == self.root || resolved.starts_with(&self.root) {
            Ok(resolved)
        } else {
            Err(PolicyError::PathEscapesWorkspace)
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandPolicy {
    pub allowed_executables: Vec<String>,
    pub max_timeout: Duration,
    pub max_output_bytes: usize,
}

impl Default for CommandPolicy {
    fn default() -> Self {
        Self {
            allowed_executables: [
                "cargo", "git", "npm", "pnpm", "pytest", "python", "python3", "ruff", "uv",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            max_timeout: Duration::from_secs(120),
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        }
    }
}

impl CommandPolicy {
    pub fn validate(&self, argv: &[String], timeout: Duration) -> Result<(), PolicyError> {
        let Some(executable) = argv.first() else {
            return Err(PolicyError::EmptyCommand);
        };
        if executable.is_empty() {
            return Err(PolicyError::EmptyCommand);
        }
        let name = Path::new(executable)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(executable);
        if !self
            .allowed_executables
            .iter()
            .any(|allowed| allowed == name)
        {
            return Err(PolicyError::ExecutableNotAllowlisted(name.to_owned()));
        }
        if timeout.is_zero() || timeout > self.max_timeout {
            return Err(PolicyError::InvalidTimeout);
        }
        if argv.iter().any(|argument| argument.contains('\0')) {
            return Err(PolicyError::ArgumentContainsNul);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub changed_paths: Vec<String>,
    pub duration_ms: u128,
    pub truncated: bool,
}

impl ToolResult {
    fn failure(tool: impl Into<String>, error: impl Into<String>, duration_ms: u128) -> Self {
        Self {
            tool: tool.into(),
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: error.into(),
            changed_paths: Vec::new(),
            duration_ms,
            truncated: false,
        }
    }
}

pub struct CommandRunner {
    pub workspace: WorkspacePolicy,
    pub policy: CommandPolicy,
}

impl CommandRunner {
    pub fn run(&self, argv: &[String], cwd: impl AsRef<Path>, timeout: Duration) -> ToolResult {
        let started = Instant::now();
        if let Err(error) = self.policy.validate(argv, timeout) {
            return ToolResult::failure(
                "run_command",
                error.to_string(),
                started.elapsed().as_millis(),
            );
        }
        let cwd = match self.workspace.resolve(cwd) {
            Ok(path) => path,
            Err(error) => {
                return ToolResult::failure(
                    "run_command",
                    error.to_string(),
                    started.elapsed().as_millis(),
                );
            }
        };
        let mut child = match Command::new(&argv[0])
            .args(&argv[1..])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                return ToolResult::failure(
                    "run_command",
                    format!("command could not start: {error}"),
                    started.elapsed().as_millis(),
                );
            }
        };
        let stdout_pipe = child.stdout.take();
        let stderr_pipe = child.stderr.take();
        let stdout_thread = spawn_reader(stdout_pipe, self.policy.max_output_bytes);
        let stderr_thread = spawn_reader(stderr_pipe, self.policy.max_output_bytes);
        let mut timed_out = false;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if started.elapsed() >= timeout => {
                    timed_out = true;
                    let _ = child.kill();
                    break child.wait().ok();
                }
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    let _ = child.kill();
                    return ToolResult::failure(
                        "run_command",
                        format!("command status failed: {error}"),
                        started.elapsed().as_millis(),
                    );
                }
            }
        };
        let (stdout_bytes, stdout_reader_truncated) = stdout_thread.join().unwrap_or_default();
        let (stderr_bytes, stderr_reader_truncated) = stderr_thread.join().unwrap_or_default();
        let elapsed = started.elapsed().as_millis();
        let (stdout, stdout_truncated) = cap_output(
            String::from_utf8_lossy(&stdout_bytes).into_owned(),
            self.policy.max_output_bytes,
        );
        let (mut stderr, stderr_truncated) = cap_output(
            String::from_utf8_lossy(&stderr_bytes).into_owned(),
            self.policy.max_output_bytes,
        );
        if timed_out {
            if !stderr.is_empty() {
                stderr.push('\n');
            }
            stderr.push_str(&format!(
                "command timed out after {}ms",
                timeout.as_millis()
            ));
        }
        ToolResult {
            tool: "run_command".into(),
            success: !timed_out
                && status
                    .as_ref()
                    .is_some_and(std::process::ExitStatus::success),
            exit_code: status.and_then(|value| value.code()),
            stdout,
            stderr,
            changed_paths: Vec::new(),
            duration_ms: elapsed,
            truncated: stdout_truncated
                || stderr_truncated
                || stdout_reader_truncated
                || stderr_reader_truncated,
        }
    }
}

fn spawn_reader(
    pipe: Option<impl Read + Send + 'static>,
    limit: usize,
) -> thread::JoinHandle<(Vec<u8>, bool)> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 8 * 1024];
        let mut truncated = false;
        if let Some(mut pipe) = pipe {
            loop {
                match pipe.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        let remaining = limit.saturating_add(1).saturating_sub(bytes.len());
                        if remaining > 0 {
                            bytes.extend_from_slice(&buffer[..count.min(remaining)]);
                        }
                        if count > remaining {
                            truncated = true;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        (bytes, truncated)
    })
}

pub struct FileTool {
    pub workspace: WorkspacePolicy,
    pub max_file_bytes: u64,
}

impl FileTool {
    pub fn read(&self, path: impl AsRef<Path>) -> ToolResult {
        let started = Instant::now();
        let path = match self.workspace.resolve(path) {
            Ok(path) => path,
            Err(error) => {
                return ToolResult::failure(
                    "read_file",
                    error.to_string(),
                    started.elapsed().as_millis(),
                );
            }
        };
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                return ToolResult::failure(
                    "read_file",
                    error.to_string(),
                    started.elapsed().as_millis(),
                );
            }
        };
        if metadata.len() > self.max_file_bytes {
            return ToolResult::failure(
                "read_file",
                format!("file exceeds {} bytes", self.max_file_bytes),
                started.elapsed().as_millis(),
            );
        }
        match fs::read_to_string(&path) {
            Ok(stdout) => ToolResult {
                tool: "read_file".into(),
                success: true,
                exit_code: Some(0),
                stdout,
                stderr: String::new(),
                changed_paths: Vec::new(),
                duration_ms: started.elapsed().as_millis(),
                truncated: false,
            },
            Err(error) => ToolResult::failure(
                "read_file",
                error.to_string(),
                started.elapsed().as_millis(),
            ),
        }
    }

    pub fn write(&self, path: impl AsRef<Path>, content: &str) -> ToolResult {
        let started = Instant::now();
        if content.len() as u64 > self.max_file_bytes {
            return ToolResult::failure(
                "edit_file",
                "content exceeds file limit",
                started.elapsed().as_millis(),
            );
        }
        let requested = path.as_ref();
        let path = match self.workspace.resolve(requested) {
            Ok(path) => path,
            Err(error) => {
                return ToolResult::failure(
                    "edit_file",
                    error.to_string(),
                    started.elapsed().as_millis(),
                );
            }
        };
        if let Some(parent) = path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                return ToolResult::failure(
                    "edit_file",
                    error.to_string(),
                    started.elapsed().as_millis(),
                );
            }
        }
        match fs::write(&path, content) {
            Ok(()) => ToolResult {
                tool: "edit_file".into(),
                success: true,
                exit_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
                changed_paths: vec![requested.to_string_lossy().replace('\\', "/")],
                duration_ms: started.elapsed().as_millis(),
                truncated: false,
            },
            Err(error) => ToolResult::failure(
                "edit_file",
                error.to_string(),
                started.elapsed().as_millis(),
            ),
        }
    }
}

pub struct SearchTool {
    pub workspace: WorkspacePolicy,
    pub max_results: usize,
}

impl SearchTool {
    pub fn text(&self, needle: &str) -> Vec<serde_json::Value> {
        if needle.is_empty() {
            return Vec::new();
        }
        let needle = needle.to_lowercase();
        let mut results = Vec::new();
        for entry in walkdir::WalkDir::new(&self.workspace.root)
            .into_iter()
            .filter_map(Result::ok)
        {
            if results.len() >= self.max_results || !entry.file_type().is_file() {
                continue;
            }
            let relative = entry
                .path()
                .strip_prefix(&self.workspace.root)
                .unwrap_or(entry.path());
            if relative.components().any(|part| {
                [".git", ".venv", "node_modules", "__pycache__", "target"]
                    .contains(&part.as_os_str().to_str().unwrap_or_default())
            }) {
                continue;
            }
            let Ok(content) = fs::read_to_string(entry.path()) else {
                continue;
            };
            for (line, text) in content.lines().enumerate() {
                if text.to_lowercase().contains(&needle) {
                    results.push(serde_json::json!({ "path": relative.to_string_lossy().replace('\\', "/"), "line": line + 1, "text": text }));
                    if results.len() >= self.max_results {
                        break;
                    }
                }
            }
        }
        results
    }
}

pub struct ToolRouter {
    pub files: FileTool,
    pub search: SearchTool,
    pub runner: CommandRunner,
}

pub struct GitTool<'a> {
    pub runner: &'a CommandRunner,
}

impl<'a> GitTool<'a> {
    pub fn diff(&self) -> ToolResult {
        rename_tool(
            self.runner.run(
                &["git", "diff", "--no-ext-diff", "--unified=3"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
                ".",
                Duration::from_secs(30),
            ),
            "git_diff",
        )
    }

    pub fn status(&self) -> ToolResult {
        rename_tool(
            self.runner.run(
                &["git", "status", "--short"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
                ".",
                Duration::from_secs(30),
            ),
            "git_status",
        )
    }
}

fn rename_tool(mut result: ToolResult, tool: &str) -> ToolResult {
    result.tool = tool.to_owned();
    result
}

impl ToolRouter {
    pub fn dispatch(&self, proposal: &ActionProposal) -> Result<ToolResult, PolicyError> {
        let arguments = proposal
            .arguments
            .as_object()
            .ok_or_else(|| PolicyError::UnsupportedAction(proposal.action.clone()))?;
        match proposal.action.as_str() {
            "read_file" => Ok(self.files.read(
                arguments
                    .get("path")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| PolicyError::UnsupportedAction("read_file path".into()))?,
            )),
            "edit_file" | "write_file" => Ok(self.files.write(
                arguments
                    .get("path")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| PolicyError::UnsupportedAction("file path required".into()))?,
                arguments
                    .get("content")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| PolicyError::UnsupportedAction("file content required".into()))?,
            )),
            "search_code" => Ok(ToolResult {
                tool: "search_code".into(),
                success: true,
                exit_code: Some(0),
                stdout: serde_json::to_string(
                    &self.search.text(
                        arguments
                            .get("query")
                            .and_then(|value| value.as_str())
                            .unwrap_or_default(),
                    ),
                )
                .unwrap_or_default(),
                stderr: String::new(),
                changed_paths: Vec::new(),
                duration_ms: 0,
                truncated: false,
            }),
            "git_diff" => Ok(GitTool {
                runner: &self.runner,
            }
            .diff()),
            "git_status" => Ok(GitTool {
                runner: &self.runner,
            }
            .status()),
            "run_command" => {
                let argv = arguments
                    .get("argv")
                    .and_then(|value| value.as_array())
                    .ok_or_else(|| PolicyError::UnsupportedAction("run_command argv".into()))?
                    .iter()
                    .map(|value| {
                        value.as_str().map(str::to_owned).ok_or_else(|| {
                            PolicyError::UnsupportedAction("run_command argument".into())
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(self.runner.run(
                    &argv,
                    arguments
                        .get("cwd")
                        .and_then(|value| value.as_str())
                        .unwrap_or("."),
                    Duration::from_secs(
                        arguments
                            .get("timeout_seconds")
                            .and_then(|value| value.as_u64())
                            .unwrap_or(30),
                    ),
                ))
            }
            _ => Err(PolicyError::UnsupportedAction(proposal.action.clone())),
        }
    }
}

fn canonicalize_for_write(path: &Path) -> PathBuf {
    if path.exists() {
        return path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    }
    let Some(parent) = path.parent() else {
        return path.to_path_buf();
    };
    let canonical_parent = parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_path_buf());
    canonical_parent.join(path.file_name().unwrap_or_default())
}

fn cap_output(output: String, limit: usize) -> (String, bool) {
    if output.len() <= limit {
        return (output, false);
    }
    (output.chars().take(limit).collect(), true)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionResult {
    pub passed: bool,
    pub transferred: bool,
    pub changed_paths: Vec<String>,
    pub reason: String,
}

#[derive(Debug)]
pub struct GitAttemptTransaction {
    pub workspace: WorkspacePolicy,
    pub max_patch_bytes: usize,
    pub max_file_bytes: u64,
    state_dir: Option<tempfile::TempDir>,
    state_lease: Option<gc::OwnedTempLease>,
    attempt_root: Option<PathBuf>,
}

impl GitAttemptTransaction {
    pub fn new(workspace: WorkspacePolicy) -> Self {
        Self {
            workspace,
            max_patch_bytes: 2_000_000,
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            state_dir: None,
            state_lease: None,
            attempt_root: None,
        }
    }

    pub fn active(&self) -> bool {
        self.attempt_root.is_some()
    }

    pub fn begin(&mut self) -> Result<WorkspacePolicy, PolicyError> {
        if self.active() {
            return Err(PolicyError::UnsupportedAction(
                "attempt transaction already active".into(),
            ));
        }
        git_checked(
            &self.workspace.root,
            &["rev-parse", "--show-toplevel"],
            Duration::from_secs(10),
        )?;
        let root = git_checked(
            &self.workspace.root,
            &["rev-parse", "--show-toplevel"],
            Duration::from_secs(10),
        )?;
        if PathBuf::from(root.stdout.trim()).canonicalize().ok()
            != Some(self.workspace.root.clone())
        {
            return Err(PolicyError::UnsupportedAction(
                "workspace must be the Git repository root".into(),
            ));
        }
        let status = git_checked(
            &self.workspace.root,
            &["status", "--porcelain", "--untracked-files=all"],
            Duration::from_secs(30),
        )?;
        if !status.stdout.trim().is_empty() {
            return Err(PolicyError::UnsupportedAction(
                "transaction requires a clean target repository".into(),
            ));
        }
        let head = git_checked(
            &self.workspace.root,
            &["rev-parse", "--verify", "HEAD"],
            Duration::from_secs(10),
        )?;
        if head.stdout.trim().is_empty() {
            return Err(PolicyError::UnsupportedAction(
                "transaction requires an initial commit".into(),
            ));
        }
        let state_dir = tempfile::Builder::new()
            .prefix("secondego-attempt-")
            .tempdir()
            .map_err(|error| PolicyError::UnsupportedAction(error.to_string()))?;
        let state_lease = gc::OwnedTempLease::create(state_dir.path(), "attempt")
            .map_err(|error| PolicyError::UnsupportedAction(error.to_string()))?;
        let attempt_root = state_dir.path().join("workspace");
        let attempt_string = attempt_root.to_string_lossy().to_string();
        if let Err(error) = git_checked(
            &self.workspace.root,
            &["worktree", "add", "--detach", &attempt_string, "HEAD"],
            Duration::from_secs(30),
        ) {
            return Err(error);
        }
        let attempt_policy = WorkspacePolicy::new(&attempt_root)?;
        self.state_dir = Some(state_dir);
        self.state_lease = Some(state_lease);
        self.attempt_root = Some(attempt_root);
        Ok(attempt_policy)
    }

    pub fn finish(&mut self, passed: bool) -> Result<TransactionResult, PolicyError> {
        let Some(attempt_root) = self.attempt_root.clone() else {
            return Err(PolicyError::UnsupportedAction(
                "no active attempt transaction".into(),
            ));
        };
        let result = (|| {
            let changed_paths = status_paths(&attempt_root)?;
            if !passed {
                return Ok(TransactionResult {
                    passed: false,
                    transferred: false,
                    changed_paths,
                    reason: "failed attempt discarded".into(),
                });
            }
            let patch_result = git_checked_with_limit(
                &attempt_root,
                &["diff", "--binary", "--no-ext-diff", "HEAD"],
                Duration::from_secs(30),
                self.max_patch_bytes,
            );
            let patch_result = patch_result?;
            if patch_result.truncated || patch_result.stdout.len() > self.max_patch_bytes {
                return Err(PolicyError::FileTooLarge);
            }
            if !patch_result.stdout.is_empty() {
                git_apply(&self.workspace.root, &patch_result.stdout)?;
            }
            for relative in untracked_paths(&attempt_root)? {
                self.transfer_new_file(&attempt_root, &relative)?;
            }
            Ok(TransactionResult {
                passed: true,
                transferred: true,
                changed_paths,
                reason: "verified attempt transferred to target workspace".into(),
            })
        })();
        self.cleanup(result)
    }

    pub fn abort(&mut self) {
        let _ = self.cleanup::<()>(Ok(()));
    }

    fn transfer_new_file(&self, attempt_root: &Path, relative: &str) -> Result<(), PolicyError> {
        let source = safe_child(attempt_root, relative)?;
        let destination = safe_child(&self.workspace.root, relative)?;
        let metadata = fs::symlink_metadata(&source)
            .map_err(|error| PolicyError::UnsupportedAction(error.to_string()))?;
        if !metadata.file_type().is_file() || metadata.len() > self.max_file_bytes {
            return Err(PolicyError::FileTooLarge);
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| PolicyError::UnsupportedAction(error.to_string()))?;
        }
        fs::copy(source, destination)
            .map_err(|error| PolicyError::UnsupportedAction(error.to_string()))?;
        Ok(())
    }

    fn cleanup<T>(&mut self, result: Result<T, PolicyError>) -> Result<T, PolicyError> {
        if let Some(attempt_root) = self.attempt_root.take() {
            let path = attempt_root.to_string_lossy().to_string();
            let cleanup_result = git_checked(
                &self.workspace.root,
                &["worktree", "remove", "--force", &path],
                Duration::from_secs(30),
            );
            self.state_lease.take();
            self.state_dir.take();
            if let Err(error) = cleanup_result {
                return Err(error);
            }
        }
        result
    }
}

fn git_checked(cwd: &Path, args: &[&str], timeout: Duration) -> Result<ToolResult, PolicyError> {
    git_checked_with_limit(cwd, args, timeout, DEFAULT_MAX_OUTPUT_BYTES)
}

fn git_checked_with_limit(
    cwd: &Path,
    args: &[&str],
    timeout: Duration,
    max_output_bytes: usize,
) -> Result<ToolResult, PolicyError> {
    let mut policy = CommandPolicy::default();
    policy.max_output_bytes = max_output_bytes;
    let runner = CommandRunner {
        workspace: WorkspacePolicy::new(cwd)?,
        policy,
    };
    let argv = std::iter::once("git".to_owned())
        .chain(args.iter().map(|arg| (*arg).to_owned()))
        .collect::<Vec<_>>();
    let result = runner.run(&argv, ".", timeout);
    if result.success {
        Ok(result)
    } else {
        Err(PolicyError::UnsupportedAction(result.stderr))
    }
}

fn git_apply(workspace: &Path, patch: &str) -> Result<(), PolicyError> {
    let mut child = Command::new("git")
        .args(["apply", "--binary", "--whitespace=nowarn", "-"])
        .current_dir(workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| PolicyError::UnsupportedAction(error.to_string()))?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(patch.as_bytes())
            .map_err(|error| PolicyError::UnsupportedAction(error.to_string()))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| PolicyError::UnsupportedAction(error.to_string()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(PolicyError::UnsupportedAction(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ))
    }
}

fn status_paths(workspace: &Path) -> Result<Vec<String>, PolicyError> {
    let output = git_checked(
        workspace,
        &["status", "--porcelain", "--untracked-files=all"],
        Duration::from_secs(30),
    )?
    .stdout;
    Ok(output
        .lines()
        .filter_map(|line| line.get(3..))
        .map(|value| {
            value
                .rsplit_once(" -> ")
                .map(|(_, path)| path)
                .unwrap_or(value)
                .to_owned()
        })
        .filter(|path| transfer_candidate(path))
        .collect())
}

fn untracked_paths(workspace: &Path) -> Result<Vec<String>, PolicyError> {
    let output = git_checked(
        workspace,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        Duration::from_secs(30),
    )?
    .stdout;
    Ok(output
        .split('\0')
        .filter(|value| !value.is_empty() && transfer_candidate(value))
        .map(str::to_owned)
        .collect())
}

fn transfer_candidate(relative: &str) -> bool {
    let sensitive = [".env", ".env.local", ".env.production", "credentials.json"];
    let generated = [
        "__pycache__",
        ".pytest_cache",
        ".mypy_cache",
        ".ruff_cache",
        "node_modules",
        ".venv",
        "build",
        "dist",
    ];
    let path = Path::new(relative);
    !sensitive.contains(
        &path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default(),
    ) && !path
        .components()
        .any(|part| generated.contains(&part.as_os_str().to_str().unwrap_or_default()))
}

fn safe_child(root: &Path, relative: &str) -> Result<PathBuf, PolicyError> {
    let root = root
        .canonicalize()
        .map_err(|error| PolicyError::UnsupportedAction(error.to_string()))?;
    let candidate = root.join(relative);
    let resolved = canonicalize_for_write(&candidate);
    if resolved == root || resolved.starts_with(&root) {
        Ok(resolved)
    } else {
        Err(PolicyError::PathEscapesWorkspace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn workspace_rejects_escape() {
        let root = tempfile::tempdir().unwrap();
        let workspace = WorkspacePolicy::new(root.path()).unwrap();
        assert_eq!(
            workspace.resolve("../outside"),
            Err(PolicyError::PathEscapesWorkspace)
        );
    }

    #[test]
    fn command_policy_requires_allowlisted_argv() {
        let policy = CommandPolicy::default();
        assert!(
            policy
                .validate(
                    &["cargo".into(), "test".into(), "--offline".into()],
                    Duration::from_secs(1)
                )
                .is_ok()
        );
        assert_eq!(
            policy.validate(&["curl".into()], Duration::from_secs(1)),
            Err(PolicyError::ExecutableNotAllowlisted("curl".into()))
        );
    }

    #[test]
    fn file_tool_writes_and_reads_only_bounded_workspace_files() {
        let root = tempfile::tempdir().unwrap();
        let workspace = WorkspacePolicy::new(root.path()).unwrap();
        let files = FileTool {
            workspace,
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
        };
        assert!(files.write("src/value.py", "VALUE = 1\n").success);
        assert_eq!(files.read("src/value.py").stdout, "VALUE = 1\n");
    }

    #[test]
    fn command_runner_enforces_timeout_and_reports_evidence() {
        let root = tempfile::tempdir().unwrap();
        let workspace = WorkspacePolicy::new(root.path()).unwrap();
        let runner = CommandRunner {
            workspace,
            policy: CommandPolicy::default(),
        };
        let result = runner.run(
            &[
                "python3".into(),
                "-c".into(),
                "import time; time.sleep(1)".into(),
            ],
            ".",
            Duration::from_millis(50),
        );
        assert!(!result.success);
        assert!(result.stderr.contains("timed out"));
    }

    #[test]
    fn failed_transaction_does_not_touch_target_workspace() {
        let root = tempfile::tempdir().unwrap();
        run_git(root.path(), &["init", "-q"]);
        run_git(root.path(), &["config", "user.email", "test@example.com"]);
        run_git(root.path(), &["config", "user.name", "SecondEgo Test"]);
        run_git(root.path(), &["config", "commit.gpgsign", "false"]);
        fs::write(root.path().join("README.md"), "before\n").unwrap();
        run_git(root.path(), &["add", "README.md"]);
        run_git(root.path(), &["commit", "-qm", "initial"]);
        let policy = WorkspacePolicy::new(root.path()).unwrap();
        let mut transaction = GitAttemptTransaction::new(policy);
        let attempt = transaction.begin().unwrap();
        fs::write(attempt.root.join("README.md"), "unverified\n").unwrap();
        let result = transaction.finish(false).unwrap();
        assert!(!result.transferred);
        assert_eq!(
            fs::read_to_string(root.path().join("README.md")).unwrap(),
            "before\n"
        );
    }

    fn run_git(cwd: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
