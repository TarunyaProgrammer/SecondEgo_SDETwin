use std::path::Path;
use std::process::Command;

use secondego_core::ActionProposal;
use secondego_model::ScriptedProvider;
use secondego_runtime::RustEngine;

#[test]
fn pagination_fixture_proves_indexed_plan_edit_verify_and_transfer() {
    let repository = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repository.path().join("src")).unwrap();
    std::fs::write(
        repository.path().join("src/pagination.py"),
        "def page_slice(items: list[str], page: int, page_size: int) -> list[str]:\n    if page < 1 or page_size < 1:\n        raise ValueError(\"page and page_size must be positive\")\n    start = page * page_size\n    return items[start : start + page_size]\n",
    ).unwrap();
    initialize_git(repository.path());

    let corrected = "def page_slice(items: list[str], page: int, page_size: int) -> list[str]:\n    if page < 1 or page_size < 1:\n        raise ValueError(\"page and page_size must be positive\")\n    start = (page - 1) * page_size\n    return items[start : start + page_size]\n";
    let proposal = ActionProposal {
        action: "submit_plan".into(),
        arguments: serde_json::json!({
            "actions": [{"action":"edit_file", "arguments":{"path":"src/pagination.py", "content":corrected}, "rationale":"convert the public page number to a zero-based slice"}],
            "verification_commands": [["python3", "-c", "from src.pagination import page_slice; assert page_slice(['a','b','c','d'], 1, 2) == ['a','b']; assert page_slice(['a','b','c','d'], 2, 2) == ['c','d']"]]
        }),
        rationale: "fixture plan".into(),
    };
    let mut engine = RustEngine::new(ScriptedProvider::new(vec![proposal]));
    let report = engine
        .run("Fix one-based pagination", repository.path())
        .unwrap();

    assert!(report.verification_passed);
    assert_eq!(
        report.state.status,
        secondego_core::TerminalStatus::Complete
    );
    assert_eq!(report.changed_paths, vec!["src/pagination.py"]);
    assert_eq!(
        std::fs::read_to_string(repository.path().join("src/pagination.py")).unwrap(),
        corrected
    );
    assert!(report.index_symbols >= 1);
    assert!(
        report
            .evidence
            .iter()
            .any(|item| item.reference == "repository:index")
    );
}

fn initialize_git(repository: &Path) {
    git(repository, &["init", "-q"]);
    git(repository, &["config", "user.email", "test@example.com"]);
    git(repository, &["config", "user.name", "SecondEgo Test"]);
    git(repository, &["config", "commit.gpgsign", "false"]);
    git(repository, &["add", "."]);
    git(repository, &["commit", "-qm", "baseline"]);
}

fn git(repository: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repository)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
