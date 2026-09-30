use secondego_core::{EngineEvent, ExecutionState, Phase, StateMachine, TerminalStatus};
use secondego_repository::{RepositoryIndex, RepositoryIndexer, Symbol};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const MAX_FINDINGS: usize = 40;
const MAX_EXCERPT_BYTES: usize = 900;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryLens {
    ErrorHandling,
    TestGap,
    Structural,
}

impl DiscoveryLens {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "error" | "errors" | "error_handling" => Some(Self::ErrorHandling),
            "test" | "tests" | "test_gap" => Some(Self::TestGap),
            "structure" | "structural" | "complexity" => Some(Self::Structural),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::ErrorHandling => "error_handling",
            Self::TestGap => "test_gap",
            Self::Structural => "structural",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    Bug,
    RegressionRisk,
    TestGap,
    MaintenanceRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingStatus {
    Candidate,
    Unverified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingEvidence {
    pub reference: String,
    pub path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub summary: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueCandidate {
    pub id: String,
    pub kind: FindingKind,
    pub status: FindingStatus,
    pub title: String,
    pub summary: String,
    pub severity: String,
    pub confidence: f32,
    pub affected_paths: Vec<String>,
    pub evidence: Vec<FindingEvidence>,
    pub verification_plan: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryRequest {
    pub lenses: Vec<DiscoveryLens>,
    pub max_findings: usize,
}

impl Default for DiscoveryRequest {
    fn default() -> Self {
        Self {
            lenses: vec![
                DiscoveryLens::ErrorHandling,
                DiscoveryLens::TestGap,
                DiscoveryLens::Structural,
            ],
            max_findings: 20,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryReport {
    pub state: ExecutionState,
    pub events: Vec<EngineEvent>,
    pub repository: RepositorySummary,
    pub lenses: Vec<String>,
    pub findings: Vec<IssueCandidate>,
    pub rejected_signals: usize,
    pub target_mutated: bool,
    pub termination_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositorySummary {
    pub files: usize,
    pub symbols: usize,
    pub tests: usize,
    pub parser_failures: usize,
}

pub fn discover(
    workspace: impl AsRef<Path>,
    request: DiscoveryRequest,
) -> Result<DiscoveryReport, String> {
    discover_with_observer(workspace, request, |_| {})
}

pub fn discover_with_observer<F>(
    workspace: impl AsRef<Path>,
    request: DiscoveryRequest,
    mut observer: F,
) -> Result<DiscoveryReport, String>
where
    F: FnMut(&EngineEvent),
{
    let workspace = workspace
        .as_ref()
        .canonicalize()
        .map_err(|error| format!("repository cannot be opened: {error}"))?;
    if !workspace.is_dir() {
        return Err("repository is not a directory".into());
    }
    let max_findings = request.max_findings.clamp(1, MAX_FINDINGS);
    let lenses = if request.lenses.is_empty() {
        DiscoveryRequest::default().lenses
    } else {
        request.lenses
    };
    let lens_names = lenses.iter().map(|lens| lens.name().to_owned()).collect();
    let mut machine = StateMachine::new(ExecutionState::new(
        "discover repository issues",
        workspace.to_string_lossy(),
    ));
    let mut events = Vec::new();
    let mut push_event = |event: EngineEvent| {
        observer(&event);
        events.push(event);
    };

    let act_event = |state: &ExecutionState, event_type: &str, operation: &str, message: &str| {
        let mut event = EngineEvent::new(state, event_type);
        event.payload = serde_json::json!({
            "operation": operation,
            "message": message,
        });
        event
    };

    push_event(
        machine
            .move_to(Phase::Understand, "initialize read-only discovery")
            .map_err(|error| error.to_string())?,
    );
    push_event(act_event(
        &machine.state,
        "activity.started",
        "repository.index",
        "Scanning workspace files and manifest topology",
    ));

    let index = RepositoryIndexer::new(&workspace)
        .build()
        .map_err(|error| format!("repository indexing failed: {error}"))?;

    push_event(act_event(
        &machine.state,
        "activity.completed",
        "repository.index",
        &format!(
            "Indexed {} files, {} symbols, {} tests",
            index.snapshot.files.len(),
            index.symbols.len(),
            index.tests.len()
        ),
    ));

    push_event(
        machine
            .move_to(Phase::Explore, "build repository intelligence")
            .map_err(|error| error.to_string())?,
    );
    let mut candidates = Vec::new();
    for lens in &lenses {
        push_event(act_event(
            &machine.state,
            "activity.started",
            &format!("lens.{}", lens.name()),
            &format!("Analyzing repository with lens '{}'", lens.name()),
        ));
        let before_count = candidates.len();
        match lens {
            DiscoveryLens::ErrorHandling => {
                collect_error_handling(&workspace, &index, &mut candidates)
            }
            DiscoveryLens::TestGap => collect_test_gaps(&workspace, &index, &mut candidates),
            DiscoveryLens::Structural => collect_structural(&index, &mut candidates),
        }
        let found = candidates.len().saturating_sub(before_count);
        push_event(act_event(
            &machine.state,
            "activity.completed",
            &format!("lens.{}", lens.name()),
            &format!("Found {} issue signal(s) for lens '{}'", found, lens.name()),
        ));
    }
    push_event(
        machine
            .move_to(Phase::Plan, "rank and deduplicate evidence-backed signals")
            .map_err(|error| error.to_string())?,
    );
    let before_dedup = candidates.len();
    push_event(act_event(
        &machine.state,
        "activity.started",
        "signals.rank",
        &format!("Ranking and deduplicating {} candidate signals", before_dedup),
    ));
    deduplicate(&mut candidates);
    let after_dedup = candidates.len();
    candidates.sort_by(|left, right| {
        right
            .confidence
            .total_cmp(&left.confidence)
            .then_with(|| left.id.cmp(&right.id))
    });
    candidates.truncate(max_findings);
    let retained_findings = candidates.len();
    push_event(act_event(
        &machine.state,
        "activity.completed",
        "signals.rank",
        &format!("Selected top {} prioritized issue candidates", retained_findings),
    ));
    push_event(
        machine
            .move_to(
                Phase::Execute,
                "evaluate deterministic discovery signals without mutation",
            )
            .map_err(|error| error.to_string())?,
    );
    push_event(
        machine
            .move_to(
                Phase::Verify,
                "complete deterministic discovery without mutation",
            )
            .map_err(|error| error.to_string())?,
    );
    let reason = format!(
        "discovery completed: {} findings from {} signals; target unchanged",
        candidates.len(),
        before_dedup
    );
    push_event(act_event(
        &machine.state,
        "activity.completed",
        "discovery.complete",
        &reason,
    ));
    push_event(
        machine
            .terminate(TerminalStatus::Complete, &reason)
            .map_err(|error| error.to_string())?,
    );
    Ok(DiscoveryReport {
        state: machine.state,
        events,
        repository: RepositorySummary {
            files: index.snapshot.files.len(),
            symbols: index.symbols.len(),
            tests: index.tests.len(),
            parser_failures: index.parser_failures.len(),
        },
        lenses: lens_names,
        findings: candidates,
        rejected_signals: before_dedup.saturating_sub(after_dedup)
            + after_dedup.saturating_sub(retained_findings),
        target_mutated: false,
        termination_reason: reason,
    })
}

fn collect_error_handling(
    root: &Path,
    index: &RepositoryIndex,
    findings: &mut Vec<IssueCandidate>,
) {
    for path in &index.snapshot.files {
        let is_py = path.ends_with(".py");
        let is_js = path.ends_with(".js") || path.ends_with(".ts") || path.ends_with(".tsx") || path.ends_with(".jsx");
        if (!is_py && !is_js) || index.snapshot.test_files.contains(path) {
            continue;
        }
        let absolute = root.join(path);
        let Ok(source) = fs::read_to_string(&absolute) else {
            continue;
        };
        let lines: Vec<_> = source.lines().collect();
        for (line_index, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if is_py {
                if !trimmed.starts_with("except") || !trimmed.ends_with(':') {
                    continue;
                }
                let next = lines
                    .get(line_index + 1)
                    .map(|value| value.trim())
                    .unwrap_or_default();
                if next == "pass" || next.starts_with("return") {
                    let line_number = line_index + 1;
                    findings.push(candidate(
                        format!("error-handling:{path}:{line_number}"),
                        FindingKind::Bug,
                        "Exception may be silently swallowed".into(),
                        format!("An exception handler at {path}:{line_number} immediately {next}, so failures may be hidden from callers."),
                        "medium".into(),
                        0.72,
                        path,
                        line_number,
                        line_number + 1,
                        format!("{}\n{}", line, next),
                        vec!["Trace callers and determine whether the fallback preserves the function contract.".into()],
                    ));
                }
            } else if is_js && (trimmed.contains("catch (") || trimmed.contains("catch(") || trimmed.starts_with("catch {") || trimmed.starts_with("catch{")) {
                let next = lines
                    .get(line_index + 1)
                    .map(|value| value.trim())
                    .unwrap_or_default();
                if trimmed.contains("{}") || next == "{}" || next == "return;" || next == "return" || next == "/* ignore */" || next == "// ignore" {
                    let line_number = line_index + 1;
                    findings.push(candidate(
                        format!("error-handling:{path}:{line_number}"),
                        FindingKind::Bug,
                        "Exception may be silently swallowed".into(),
                        format!("A catch block at {path}:{line_number} appears empty or returns immediately, which may swallow unexpected errors."),
                        "medium".into(),
                        0.70,
                        path,
                        line_number,
                        line_number + 1,
                        format!("{}\n{}", line, next),
                        vec!["Log or handle the exception appropriately instead of ignoring it.".into()],
                    ));
                }
            }
        }
    }
}

fn collect_test_gaps(root: &Path, index: &RepositoryIndex, findings: &mut Vec<IssueCandidate>) {
    let mut test_sources: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for link in &index.test_links {
        if !test_sources.contains_key(&link.test_path) {
            if let Ok(source) = fs::read_to_string(root.join(&link.test_path)) {
                test_sources.insert(link.test_path.clone(), source);
            }
        }
    }
    for symbol in &index.symbols {
        if symbol.kind != "function"
            || symbol.path.starts_with("tests/")
            || symbol.path.contains("/tests/")
        {
            continue;
        }
        let tested = index.test_links.iter().any(|link| {
            link.target_path == symbol.path
                && test_sources
                    .get(&link.test_path)
                    .map(|source| source.contains(&symbol.name))
                    .unwrap_or(false)
        });
        if !tested && symbol.name.len() > 4 && !symbol.name.starts_with('_') {
            findings.push(candidate(
                format!("test-gap:{}:{}", symbol.path, symbol.line_start),
                FindingKind::TestGap,
                "Public function has no detected test link".into(),
                format!("{}::{} is indexed as a public function, but no test file import was detected for {}.", symbol.path, symbol.name, symbol.path),
                "low".into(),
                0.55,
                &symbol.path,
                symbol.line_start,
                symbol.line_end,
                symbol_excerpt(root, symbol),
                vec!["Add or locate a focused test for normal and failure behavior before changing this code.".into()],
            ));
        }
    }
}

fn collect_structural(index: &RepositoryIndex, findings: &mut Vec<IssueCandidate>) {
    for symbol in &index.symbols {
        let span = symbol.line_end.saturating_sub(symbol.line_start) + 1;
        if symbol.kind == "function" && span >= 45 {
            findings.push(candidate(
                format!("structural:{}:{}", symbol.path, symbol.line_start),
                FindingKind::MaintenanceRisk,
                "Large function deserves focused review".into(),
                format!("{}::{} spans {span} lines, increasing the chance that branching and error paths are difficult to verify.", symbol.path, symbol.name),
                "low".into(),
                0.58,
                &symbol.path,
                symbol.line_start,
                symbol.line_end,
                format!("symbol {} lines {}-{}", symbol.name, symbol.line_start, symbol.line_end),
                vec!["Inspect branches and side effects; split only if behavior can be covered by focused tests.".into()],
            ));
        }
    }
}

fn candidate(
    id: String,
    kind: FindingKind,
    title: String,
    summary: String,
    severity: String,
    confidence: f32,
    path: &str,
    line_start: usize,
    line_end: usize,
    excerpt: String,
    verification_plan: Vec<String>,
) -> IssueCandidate {
    IssueCandidate {
        id: id.clone(),
        kind,
        status: FindingStatus::Candidate,
        title,
        summary,
        severity,
        confidence,
        affected_paths: vec![path.to_owned()],
        evidence: vec![FindingEvidence {
            reference: format!("finding:{id}"),
            path: path.to_owned(),
            line_start,
            line_end,
            summary: excerpt.chars().take(MAX_EXCERPT_BYTES).collect(),
            confidence,
        }],
        verification_plan,
    }
}

fn symbol_excerpt(root: &Path, symbol: &Symbol) -> String {
    let Ok(source) = fs::read_to_string(root.join(&symbol.path)) else {
        return format!(
            "symbol {} lines {}-{}",
            symbol.name, symbol.line_start, symbol.line_end
        );
    };
    source
        .lines()
        .skip(symbol.line_start.saturating_sub(1))
        .take((symbol.line_end - symbol.line_start + 1).min(12))
        .collect::<Vec<_>>()
        .join("\n")
}

fn deduplicate(findings: &mut Vec<IssueCandidate>) {
    let mut seen = BTreeSet::new();
    findings.retain(|finding| {
        let evidence = finding.evidence.first();
        let key = evidence
            .map(|item| format!("{:?}:{}:{}", finding.kind, item.path, item.line_start))
            .unwrap_or_else(|| finding.id.clone());
        seen.insert(key)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture() -> (tempfile::TempDir, String) {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(directory.path().join("src")).unwrap();
        fs::create_dir_all(directory.path().join("tests")).unwrap();
        let source = "def load_value(value):\n    try:\n        return int(value)\n    except Exception:\n        pass\n\ndef public_api(value):\n    return value\n";
        fs::write(directory.path().join("src/app.py"), source).unwrap();
        fs::write(
            directory.path().join("tests/test_app.py"),
            "from src.app import load_value\n\ndef test_load_value():\n    assert load_value('1') == 1\n",
        )
        .unwrap();
        let snapshot = fs::read(directory.path().join("src/app.py")).unwrap();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        (directory, format!("{stamp}:{snapshot:?}"))
    }

    #[test]
    fn discovery_finds_read_only_evidence_and_preserves_target() {
        let (directory, before) = fixture();
        let report = discover(directory.path(), DiscoveryRequest::default()).unwrap();
        assert!(!report.findings.is_empty());
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.kind == FindingKind::Bug)
        );
        assert!(!report.target_mutated);
        let after = format!(
            "{}:{:?}",
            before.split(':').next().unwrap(),
            fs::read(directory.path().join("src/app.py")).unwrap()
        );
        assert_eq!(before, after);
    }

    #[test]
    fn discovery_limits_findings_and_accepts_lens_aliases() {
        let (directory, _) = fixture();
        assert_eq!(
            DiscoveryLens::parse("error"),
            Some(DiscoveryLens::ErrorHandling)
        );
        let report = discover(
            directory.path(),
            DiscoveryRequest {
                lenses: vec![DiscoveryLens::ErrorHandling],
                max_findings: 1,
            },
        )
        .unwrap();
        assert_eq!(report.lenses, vec!["error_handling"]);
        assert!(report.findings.len() <= 1);
    }
}
