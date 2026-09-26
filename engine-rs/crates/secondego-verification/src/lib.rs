use std::time::Duration;

use regex::Regex;
use secondego_tools::{CommandRunner, ToolResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FailureClass {
    None,
    TestFailure,
    BuildFailure,
    LintFailure,
    TypeError,
    RuntimeError,
    ToolFailure,
    EnvironmentFailure,
    ModelPlanningFailure,
    Regression,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureRecord {
    pub failure_class: FailureClass,
    pub summary: String,
    pub failing_tests: Vec<String>,
    pub error_locations: Vec<String>,
    pub fingerprint: Option<String>,
    pub changed_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationEvidence {
    pub command: Vec<String>,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationResult {
    pub passed: bool,
    pub commands: Vec<String>,
    pub passed_tests: u32,
    pub failed_tests: u32,
    pub failure_class: FailureClass,
    pub failure_summary: Option<String>,
    pub failure_record: Option<FailureRecord>,
    pub evidence: Vec<VerificationEvidence>,
}

pub struct VerificationEngine<'a> {
    pub runner: &'a CommandRunner,
}

impl<'a> VerificationEngine<'a> {
    pub fn run(&self, commands: &[Vec<String>]) -> VerificationResult {
        if commands.is_empty() {
            return VerificationResult {
                passed: false,
                commands: Vec::new(),
                passed_tests: 0,
                failed_tests: 0,
                failure_class: FailureClass::EnvironmentFailure,
                failure_summary: Some("no verification commands configured".into()),
                failure_record: None,
                evidence: Vec::new(),
            };
        }
        let mut evidence = Vec::new();
        for command in commands {
            let result = self.runner.run(command, ".", Duration::from_secs(30));
            let output = [result.stdout.as_str(), result.stderr.as_str()]
                .join("\n")
                .trim()
                .to_owned();
            evidence.push(VerificationEvidence {
                command: command.clone(),
                success: result.success,
                exit_code: result.exit_code,
                duration_ms: result.duration_ms,
                output: output.chars().take(4_000).collect(),
            });
            if !result.success {
                let failure_class = classify_failure(&output, &result);
                let failure_record = build_failure_record(failure_class, &output);
                return VerificationResult {
                    passed: false,
                    commands: evidence.iter().map(|item| item.command.join(" ")).collect(),
                    passed_tests: 0,
                    failed_tests: failure_record.failing_tests.len() as u32,
                    failure_class,
                    failure_summary: Some(output.chars().take(1_000).collect()),
                    failure_record: Some(failure_record),
                    evidence,
                };
            }
        }
        VerificationResult {
            passed: true,
            commands: evidence.iter().map(|item| item.command.join(" ")).collect(),
            passed_tests: 0,
            failed_tests: 0,
            failure_class: FailureClass::None,
            failure_summary: None,
            failure_record: None,
            evidence,
        }
    }
}

fn classify_failure(output: &str, result: &ToolResult) -> FailureClass {
    let normalized = output.to_lowercase();
    if result.exit_code.is_none()
        || ["timed out", "not found", "could not start", "no such file"]
            .iter()
            .any(|term| normalized.contains(term))
    {
        return FailureClass::EnvironmentFailure;
    }
    if normalized.contains("syntaxerror") || normalized.contains("syntax error") {
        return FailureClass::BuildFailure;
    }
    if normalized.contains("mypy") || normalized.contains("type error") {
        return FailureClass::TypeError;
    }
    if normalized.contains("lint") || normalized.contains("ruff") {
        return FailureClass::LintFailure;
    }
    if normalized.contains("failed") || normalized.contains("assert") {
        return FailureClass::TestFailure;
    }
    FailureClass::Unknown
}

fn build_failure_record(failure_class: FailureClass, output: &str) -> FailureRecord {
    let test_regex = Regex::new(r"(?:FAILED|ERROR)\s+([^\s]+)").expect("static regex");
    let location_regex =
        Regex::new(r"([A-Za-z0-9_./-]+\.(?:py|js|ts|tsx|jsx)):(\d+)").expect("static regex");
    let failing_tests = test_regex
        .captures_iter(output)
        .filter_map(|capture| capture.get(1).map(|value| value.as_str().to_owned()))
        .take(20)
        .collect();
    let error_locations = location_regex
        .captures_iter(output)
        .filter_map(|capture| {
            Some(format!(
                "{}:{}",
                capture.get(1)?.as_str(),
                capture.get(2)?.as_str()
            ))
        })
        .take(20)
        .collect();
    let fingerprint = output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(240).collect());
    FailureRecord {
        failure_class,
        summary: output.chars().take(1_000).collect(),
        failing_tests,
        error_locations,
        fingerprint,
        changed_paths: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secondego_tools::{CommandPolicy, WorkspacePolicy};

    #[test]
    fn missing_commands_are_environment_failures() {
        let workspace = tempfile::tempdir().unwrap();
        let policy = WorkspacePolicy::new(workspace.path()).unwrap();
        let runner = CommandRunner {
            workspace: policy,
            policy: CommandPolicy::default(),
        };
        let result = VerificationEngine { runner: &runner }.run(&[]);
        assert_eq!(result.failure_class, FailureClass::EnvironmentFailure);
    }

    #[test]
    fn failed_assertions_create_structured_failure_evidence() {
        let workspace = tempfile::tempdir().unwrap();
        let policy = WorkspacePolicy::new(workspace.path()).unwrap();
        let runner = CommandRunner {
            workspace: policy,
            policy: CommandPolicy::default(),
        };
        let result = VerificationEngine { runner: &runner }.run(&[vec![
            "python3".into(),
            "-c".into(),
            "assert False".into(),
        ]]);
        assert_eq!(result.failure_class, FailureClass::TestFailure);
        assert!(result.failure_record.is_some());
        assert_eq!(result.evidence.len(), 1);
    }
}
