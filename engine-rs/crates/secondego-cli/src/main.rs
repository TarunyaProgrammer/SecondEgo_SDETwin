use std::env;
use std::fs;
use std::path::PathBuf;

use secondego_core::ActionProposal;
use secondego_model::{GeminiProvider, ScriptedProvider};
use secondego_runtime::RustEngine;

fn main() {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let interactive = arguments.iter().any(|argument| argument == "--interactive");
    let workspace = value(&arguments, "--workspace")
        .or_else(|| env::var("SECONDEGO_REPOSITORY").ok())
        .or_else(|| interactive.then(|| prompt("Repository path")))
        .unwrap_or_else(|| ".".into());
    let task = value(&arguments, "--task")
        .or_else(|| value(&arguments, "--issue"))
        .or_else(|| env::var("SECONDEGO_TASK").ok())
        .or_else(|| interactive.then(|| prompt("Issue")))
        .unwrap_or_else(|| "complete the requested repository change".into());
    let events = value(&arguments, "--ui").as_deref() == Some("events");
    let state_db = value(&arguments, "--state-db");
    let result = match value(&arguments, "--script") {
        Some(script_path) => run_scripted(
            PathBuf::from(workspace),
            task,
            PathBuf::from(script_path),
            events,
            state_db,
        ),
        None => run_gemini(PathBuf::from(workspace), task, events, state_db),
    };
    if let Err(error) = result {
        eprintln!("SecondEgo failed: {error}");
        std::process::exit(1);
    }
}

fn run_scripted(
    workspace: PathBuf,
    task: String,
    script: PathBuf,
    events: bool,
    state_db: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(script)?)?;
    let proposal = if value.get("action").is_some() {
        serde_json::from_value(value)?
    } else {
        ActionProposal {
            action: "submit_plan".into(),
            arguments: value,
            rationale: "replayable fixture plan".into(),
        }
    };
    let mut engine = RustEngine::new(ScriptedProvider::new(vec![proposal]));
    emit_report(engine.run(task, workspace)?, events, state_db)
}

fn run_gemini(
    workspace: PathBuf,
    task: String,
    events: bool,
    state_db: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = RustEngine::new(GeminiProvider::default());
    emit_report(engine.run(task, workspace)?, events, state_db)
}

fn emit_report(
    report: secondego_runtime::RunReport,
    events: bool,
    state_db: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if events {
        for event in &report.events {
            println!("{}", serde_json::to_string(event)?);
        }
    }
    if let Some(path) = state_db {
        secondego_storage::SQLiteRunStore::open(path)?.save(&report.state, &report.events)?;
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report.verification_passed {
        Ok(())
    } else {
        Err("verification failed".into())
    }
}

fn value(arguments: &[String], name: &str) -> Option<String> {
    arguments
        .windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn prompt(label: &str) -> String {
    use std::io::{self, Write};
    print!("{label}: ");
    let _ = io::stdout().flush();
    let mut value = String::new();
    if io::stdin().read_line(&mut value).is_err() {
        return String::new();
    }
    value.trim().to_owned()
}
