use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use secondego_core::ActionProposal;
use secondego_model::{ConfiguredProvider, ScriptedProvider};
use secondego_runtime::{RustEngine, collect_garbage, resolve_repository};

const CORAL: &str = "38;2;238;101;71";
const PARCHEMENT: &str = "38;2;255;241;216";
const PEACH: &str = "38;2;255;185;156";
const MUTED: &str = "38;2;166;131;119";

fn main() {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let gc = collect_garbage();
    if gc.deleted > 0 {
        eprintln!(
            "SecondEgo garbage collector: reclaimed {} stale temp directories",
            gc.deleted
        );
    }
    let interactive = arguments.iter().any(|argument| argument == "--interactive");
    if interactive {
        install_cancel_handler();
        print_banner();
    }
    let workspace = value(&arguments, "--workspace")
        .or_else(|| env::var("SECONDEGO_REPOSITORY").ok())
        .or_else(|| interactive.then(|| prompt("Repository path")))
        .unwrap_or_else(|| ".".into());
    let task = value(&arguments, "--task")
        .or_else(|| value(&arguments, "--issue"))
        .or_else(|| env::var("SECONDEGO_TASK").ok())
        .or_else(|| interactive.then(prompt_multiline))
        .unwrap_or_else(|| "complete the requested repository change".into());
    if interactive && task.trim().is_empty() {
        print_goodbye("No mission entered. The village is going to sleep.");
        return;
    }
    let events = value(&arguments, "--ui").as_deref() == Some("events");
    let state_db = value(&arguments, "--state-db");
    let resolved = match resolve_repository(&workspace) {
        Ok(repository) => repository,
        Err(error) => {
            eprintln!("SecondEgo repository error: {error}");
            std::process::exit(2);
        }
    };
    if interactive {
        print_run_card(&resolved.root, resolved.cloned);
    } else if resolved.cloned {
        eprintln!(
            "SecondEgo cloned repository to: {}",
            resolved.root.display()
        );
    }
    let result = match value(&arguments, "--script") {
        Some(script_path) => run_scripted(
            resolved.root,
            task,
            PathBuf::from(script_path),
            events,
            state_db,
            interactive,
        ),
        None => run_configured(resolved.root, task, events, state_db, interactive),
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
    interactive: bool,
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
    if interactive && !events {
        engine = engine.with_event_sink(print_live_event);
    }
    emit_report(engine.run(task, workspace)?, events, state_db, interactive)
}

fn run_configured(
    workspace: PathBuf,
    task: String,
    events: bool,
    state_db: Option<String>,
    interactive: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = RustEngine::new(ConfiguredProvider::from_environment()?);
    if interactive && !events {
        engine = engine.with_event_sink(print_live_event);
    }
    emit_report(engine.run(task, workspace)?, events, state_db, interactive)
}

fn emit_report(
    report: secondego_runtime::RunReport,
    events: bool,
    state_db: Option<String>,
    interactive: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if events {
        for event in &report.events {
            println!("{}", serde_json::to_string(event)?);
        }
    }
    if let Some(path) = state_db {
        secondego_storage::SQLiteRunStore::open(path)?.save(&report.state, &report.events)?;
    }
    if interactive {
        print_result_card(&report);
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
    print!("{} ", paint(PEACH, &format!("{label} ›")));
    let _ = io::stdout().flush();
    let mut value = String::new();
    if io::stdin().read_line(&mut value).is_err() {
        return String::new();
    }
    value.trim().to_owned()
}

fn prompt_multiline() -> String {
    println!();
    println!("{}", paint(CORAL, "Mission / issue"));
    println!(
        "  {}",
        paint(
            MUTED,
            "Paste or type the complete task. Finish with a line containing .done (Ctrl-D also works)."
        )
    );
    let mut lines = Vec::new();
    loop {
        print!("  {} ", paint(CORAL, "│"));
        let _ = io::stdout().flush();
        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let line = line.trim_end_matches(['\r', '\n']);
                if line.trim() == ".done" {
                    break;
                }
                lines.push(line.to_owned());
            }
        }
    }
    lines.join("\n").trim().to_owned()
}

fn install_cancel_handler() {
    unsafe {
        libc::signal(
            libc::SIGINT,
            handle_sigint as *const () as libc::sighandler_t,
        );
    }
}

extern "C" fn handle_sigint(_: libc::c_int) {
    const MESSAGE: &[u8] =
        b"\n  \x1b[38;2;255;185;156mGoodbye for now - the village is resting.\x1b[0m\n";
    unsafe {
        libc::write(libc::STDERR_FILENO, MESSAGE.as_ptr().cast(), MESSAGE.len());
        libc::_exit(130);
    }
}

fn print_goodbye(message: &str) {
    println!();
    println!(
        "  {}",
        paint(PEACH, &format!("Goodbye for now — {message}"))
    );
}

fn print_banner() {
    println!();
    for line in [
        " ███████╗███████╗ ██████╗ ██████╗ ███╗   ██╗██████╗     ███████╗ ██████╗  ██████╗ ",
        " ██╔════╝██╔════╝██╔════╝██╔═══██╗████╗  ██║██╔══██╗    ██╔════╝██╔════╝ ██╔═══██╗",
        " ███████╗█████╗  ██║     ██║   ██║██╔██╗ ██║██║  ██║    █████╗  ██║  ███╗██║   ██║",
        " ╚════██║██╔══╝  ██║     ██║   ██║██║╚██╗██║██║  ██║    ██╔══╝  ██║   ██║██║   ██║",
        " ███████║███████╗╚██████╗╚██████╔╝██║ ╚████║██████╔╝    ███████╗╚██████╔╝╚██████╔╝",
        " ╚══════╝╚══════╝ ╚═════╝ ╚═════╝ ╚═╝  ╚═══╝╚═════╝     ╚══════╝ ╚═════╝  ╚═════╝ ",
    ] {
        println!("{}", paint(CORAL, line));
    }
    println!();
    println!(
        "{}",
        paint(
            CORAL,
            "╭─ SECOND EGO · VERIFIED CODING HARNESS ───────────────────────────────╮"
        )
    );
    println!(
        "{}",
        paint(
            PARCHEMENT,
            "│  understand  ›  explore  ›  plan  ›  execute  ›  verify              │"
        )
    );
    println!(
        "{}",
        paint(
            PEACH,
            "│  local-first repository intelligence with bounded, inspectable runs   │"
        )
    );
    println!(
        "{}",
        paint(
            CORAL,
            "╰───────────────────────────────────────────────────────────────────────╯"
        )
    );
    println!(
        "  {}  Type Ctrl-C to cancel.\n",
        paint(MUTED, "RUST ENGINE / INTERACTIVE")
    );
}

fn print_run_card(root: &std::path::Path, cloned: bool) {
    let source = if cloned {
        "remote repository · shallow clone"
    } else {
        "local repository"
    };
    println!(
        "{}",
        paint(
            CORAL,
            "╭─ RUN CONFIG ─────────────────────────────────────────────────────────╮"
        )
    );
    println!(
        "{}",
        paint(
            PEACH,
            &format!("│  ENGINE   Rust state machine + provider boundary                       │")
        )
    );
    println!("{}", paint(PEACH, &format!("│  SOURCE   {source:<61}│")));
    println!(
        "{}",
        paint(PEACH, &format!("│  ROOT     {:<61}│", root.display()))
    );
    println!(
        "{}",
        paint(
            CORAL,
            "│  OUTPUT   live evidence + final JSON report                           │"
        )
    );
    println!(
        "{}",
        paint(
            CORAL,
            "╰───────────────────────────────────────────────────────────────────────╯"
        )
    );
}

fn print_live_event(event: &secondego_core::EngineEvent) {
    let icon = match event.event_type.as_str() {
        "tool.completed" => "→",
        "verification.completed" => "✓",
        "run.terminated" => "■",
        _ => "·",
    };
    println!(
        "  {} {:<10} {}",
        paint(CORAL, icon),
        paint(MUTED, &format!("{:?}", event.phase)),
        event.event_type
    );
}

fn print_result_card(report: &secondego_runtime::RunReport) {
    let status = if report.verification_passed {
        "VERIFIED"
    } else {
        "FAILED"
    };
    let color = if report.verification_passed {
        PARCHEMENT
    } else {
        CORAL
    };
    println!();
    println!(
        "{}",
        paint(
            color,
            "╭─ RUN RESULT ─────────────────────────────────────────────────────────╮"
        )
    );
    println!("{}", paint(color, &format!("│  {:<68}│", status)));
    println!(
        "{}",
        paint(
            color,
            &format!("│  phase      {:<58}│", format!("{:?}", report.state.phase)),
        )
    );
    println!(
        "{}",
        paint(
            color,
            &format!("│  changed    {:<58}│", report.state.changed_paths.len())
        )
    );
    println!(
        "{}",
        paint(
            color,
            "╰───────────────────────────────────────────────────────────────────────╯"
        )
    );
    println!(
        "  {}",
        paint(MUTED, "Machine-readable report follows below.")
    );
}

fn paint(code: &str, value: &str) -> String {
    if io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none() {
        format!("\x1b[{code}m{value}\x1b[0m")
    } else {
        value.to_owned()
    }
}
