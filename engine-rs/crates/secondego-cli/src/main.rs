use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::thread;
use std::time::Duration;

use secondego_core::ActionProposal;
use secondego_model::{ConfiguredProvider, ProviderKind, ScriptedProvider, configured_model};
use secondego_runtime::discovery::{DiscoveryLens, DiscoveryRequest, discover};
use secondego_runtime::{
    CancellationToken, RuntimeError, RustEngine, collect_garbage, resolve_repository,
    resolve_repository_with_cancellation,
};

const CORAL: &str = "38;2;255;205;0";
const PARCHEMENT: &str = "38;2;255;241;216";
const PEACH: &str = "38;2;223;143;45";
const MUTED: &str = "38;2;170;143;92";
static INTERRUPT_REQUESTED: AtomicBool = AtomicBool::new(false);
static RUN_ACTIVE: AtomicBool = AtomicBool::new(false);
static PARENT_TERMINAL_PGRP: AtomicI32 = AtomicI32::new(0);

fn main() {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
        || arguments.first().is_some_and(|argument| argument == "help")
    {
        print_help();
        return;
    }
    let gc = collect_garbage();
    if gc.deleted > 0 {
        eprintln!(
            "SecondEgo garbage collector: reclaimed {} stale temp directories",
            gc.deleted
        );
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "discover")
    {
        if let Err(error) = run_discovery(&arguments[1..]) {
            eprintln!("SecondEgo discovery failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    let interactive = arguments.iter().any(|argument| argument == "--interactive");
    let cancellation = CancellationToken::default();
    if interactive {
        install_cancel_handler();
    }
    let _terminal_foreground = interactive.then(take_terminal_foreground);
    if interactive {
        let bridge = cancellation.clone();
        thread::spawn(move || {
            while !INTERRUPT_REQUESTED.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(50));
            }
            bridge.cancel();
        });
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
    let script = value(&arguments, "--script");
    let model_identity = if script.is_some() {
        ("scripted".to_owned(), "local fixture plan".to_owned())
    } else {
        let provider = match ProviderKind::from_environment() {
            Ok(provider) => provider,
            Err(error) => {
                restore_terminal_foreground();
                eprintln!("SecondEgo configuration error: {error}");
                std::process::exit(2);
            }
        };
        let model = match configured_model() {
            Ok(model) => model,
            Err(error) => {
                restore_terminal_foreground();
                eprintln!("SecondEgo configuration error: {error}");
                std::process::exit(2);
            }
        };
        (provider.name().to_owned(), model)
    };
    if interactive {
        RUN_ACTIVE.store(true, Ordering::Release);
    }
    let events = value(&arguments, "--ui").as_deref() == Some("events");
    let state_db = value(&arguments, "--state-db");
    let resolved = match resolve_repository_with_cancellation(&workspace, Some(&cancellation)) {
        Ok(repository) => repository,
        Err(RuntimeError::Cancelled) => {
            RUN_ACTIVE.store(false, Ordering::Release);
            print_goodbye("The run was stopped before repository setup began.");
            return;
        }
        Err(error) => {
            RUN_ACTIVE.store(false, Ordering::Release);
            restore_terminal_foreground();
            eprintln!("SecondEgo repository error: {error}");
            std::process::exit(2);
        }
    };
    if interactive {
        print_run_card(
            &resolved.root,
            resolved.cloned,
            &model_identity.0,
            &model_identity.1,
        );
    } else if resolved.cloned {
        eprintln!(
            "SecondEgo cloned repository to: {}",
            resolved.root.display()
        );
    }
    let result = match script {
        Some(script_path) => run_scripted(
            resolved.root,
            task,
            PathBuf::from(script_path),
            events,
            state_db,
            interactive,
            cancellation,
        ),
        None => run_configured(
            resolved.root,
            task,
            events,
            state_db,
            interactive,
            cancellation,
        ),
    };
    RUN_ACTIVE.store(false, Ordering::Release);
    if let Err(error) = result {
        if error.to_string() == "run cancelled by user" {
            print_goodbye("The run was stopped safely and its isolated changes were discarded.");
            return;
        }
        restore_terminal_foreground();
        eprintln!("SecondEgo failed: {error}");
        std::process::exit(1);
    }
}

fn run_discovery(arguments: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = value(arguments, "--workspace")
        .or_else(|| value(arguments, "--repo"))
        .or_else(|| env::var("SECONDEGO_REPOSITORY").ok())
        .unwrap_or_else(|| ".".into());
    let resolved = resolve_repository(&workspace)?;
    let lenses = value(arguments, "--lens")
        .map(|value| {
            value
                .split(',')
                .filter_map(DiscoveryLens::parse)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let max_findings = value(arguments, "--max-findings")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20);
    let report = discover(
        &resolved.root,
        DiscoveryRequest {
            lenses,
            max_findings,
        },
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn print_help() {
    println!("SecondEgo — bounded local coding harness");
    println!();
    println!("COMMANDS");
    println!("  secondego --interactive                 Run the verified task-fixing harness");
    println!("  secondego --workspace PATH --task TEXT   Run a non-interactive task");
    println!("  secondego discover --repo PATH           Scan a repository without modifying it");
    println!("  secondego help                           Show this command guide");
    println!();
    println!("DISCOVERY SHOWCASE");
    println!("  cargo run --manifest-path engine-rs/Cargo.toml -p secondego-cli -- \\");
    println!("    discover --repo evaluation/discovery_fixture_repo --lens error,test,structural");
    println!();
    println!("DISCOVERY OPTIONS");
    println!("  --repo PATH | --workspace PATH           Local repository to inspect");
    println!("  --lens LIST                              error,test,structural (comma-separated)");
    println!(
        "  --max-findings N                         Cap the report at N findings (default: 20)"
    );
    println!();
    println!("SAFETY");
    println!(
        "  Discovery is read-only: it indexes, ranks, and reports evidence-backed candidates."
    );
    println!("  It does not edit files, run arbitrary commands, or transfer diffs.");
    println!();
    println!("SHOWCASE PLAN");
    println!("  1. Run the fixture command above and show the two deliberate findings.");
    println!("  2. Point it at a small target repository with one known swallowed exception.");
    println!("  3. Compare --lens error with --lens test,structural.");
    println!(
        "  4. Explain each finding through its path, line range, confidence, and verification plan."
    );
    println!("  5. Re-run and show the target Git diff is unchanged.");
}

fn run_scripted(
    workspace: PathBuf,
    task: String,
    script: PathBuf,
    events: bool,
    state_db: Option<String>,
    interactive: bool,
    cancellation: CancellationToken,
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
    let mut engine =
        RustEngine::new(ScriptedProvider::new(vec![proposal])).with_cancellation(cancellation);
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
    cancellation: CancellationToken,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine =
        RustEngine::new(ConfiguredProvider::from_environment()?).with_cancellation(cancellation);
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
        print!("  {} ", paint(CORAL, ">"));
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

/// Keeps Ctrl-C scoped to the interactive harness instead of the Make process
/// that launched it. This is a no-op for redirected/non-terminal input.
struct TerminalForegroundGuard;

impl Drop for TerminalForegroundGuard {
    fn drop(&mut self) {
        restore_terminal_foreground();
    }
}

fn take_terminal_foreground() -> TerminalForegroundGuard {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } != 1 {
        return TerminalForegroundGuard;
    }
    let parent_group = unsafe { libc::tcgetpgrp(libc::STDIN_FILENO) };
    if parent_group <= 0 {
        return TerminalForegroundGuard;
    }
    unsafe {
        // Moving to a new process group makes this harness, not Make, the
        // terminal's foreground recipient of SIGINT. Ignore SIGTTOU while
        // transferring that foreground ownership.
        libc::signal(libc::SIGTTOU, libc::SIG_IGN);
        if libc::setpgid(0, 0) == 0 {
            let harness_group = libc::getpgrp();
            if harness_group > 0 && libc::tcsetpgrp(libc::STDIN_FILENO, harness_group) == 0 {
                PARENT_TERMINAL_PGRP.store(parent_group, Ordering::Release);
            }
        }
    }
    TerminalForegroundGuard
}

fn restore_terminal_foreground() {
    let parent_group = PARENT_TERMINAL_PGRP.swap(0, Ordering::AcqRel);
    if parent_group > 0 {
        unsafe {
            libc::tcsetpgrp(libc::STDIN_FILENO, parent_group);
        }
    }
}

fn install_cancel_handler() {
    INTERRUPT_REQUESTED.store(false, Ordering::Release);
    unsafe {
        libc::signal(
            libc::SIGINT,
            handle_sigint as *const () as libc::sighandler_t,
        );
    }
}

extern "C" fn handle_sigint(_: libc::c_int) {
    const STOP_REQUESTED: &[u8] =
        b"\n  \x1b[38;2;255;185;156mStop requested. Finishing the current bounded operation; press Ctrl-C again to force quit.\x1b[0m\n";
    const FORCE_QUIT: &[u8] = b"\n  \x1b[38;2;255;185;156mForce quit.\x1b[0m\n";
    const PROMPT_EXIT: &[u8] =
        b"\n  \x1b[38;2;255;185;156mGoodbye for now - the village is resting.\x1b[0m\n";
    if !RUN_ACTIVE.load(Ordering::Acquire) {
        // Input reads can be restarted after SIGINT. At a prompt there is no
        // transaction to protect, so exit instead of leaving a stuck read.
        unsafe {
            libc::write(
                libc::STDERR_FILENO,
                PROMPT_EXIT.as_ptr().cast(),
                PROMPT_EXIT.len(),
            );
            let parent_group = PARENT_TERMINAL_PGRP.swap(0, Ordering::AcqRel);
            if parent_group > 0 {
                libc::tcsetpgrp(libc::STDIN_FILENO, parent_group);
            }
            libc::_exit(130);
        }
    }
    if INTERRUPT_REQUESTED.swap(true, Ordering::AcqRel) {
        unsafe {
            libc::write(
                libc::STDERR_FILENO,
                FORCE_QUIT.as_ptr().cast(),
                FORCE_QUIT.len(),
            );
            let parent_group = PARENT_TERMINAL_PGRP.swap(0, Ordering::AcqRel);
            if parent_group > 0 {
                libc::tcsetpgrp(libc::STDIN_FILENO, parent_group);
            }
            libc::_exit(130);
        }
    }
    unsafe {
        libc::write(
            libc::STDERR_FILENO,
            STOP_REQUESTED.as_ptr().cast(),
            STOP_REQUESTED.len(),
        );
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
    println!(
        "{}",
        paint(
            CORAL,
            " ███████╗███████╗ ██████╗ ██████╗ ███╗   ██╗██████╗      ███████╗ ██████╗  ██████╗ "
        )
    );
    println!(
        "{}",
        paint(
            CORAL,
            " ██╔════╝██╔════╝██╔════╝██╔═══██╗████╗  ██║██╔══██╗     ██╔════╝██╔════╝ ██╔═══██╗"
        )
    );
    println!(
        "{}",
        paint(
            PEACH,
            " ███████╗█████╗  ██║     ██║   ██║██╔██╗ ██║██║  ██║     █████╗  ██║  ███╗██║   ██║"
        )
    );
    println!(
        "{}",
        paint(
            MUTED,
            " ╚════██║██╔══╝  ██║     ██║   ██║██║╚██╗██║██║  ██║     ██╔══╝  ██║   ██║██║   ██║"
        )
    );
    println!(
        "{}",
        paint(
            CORAL,
            " ███████║███████╗╚██████╗╚██████╔╝██║ ╚████║██████╔╝     ███████╗╚██████╔╝╚██████╔╝"
        )
    );
    println!(
        "{}",
        paint(
            CORAL,
            " ╚══════╝╚══════╝ ╚═════╝ ╚═════╝ ╚═╝  ╚═══╝╚═════╝      ╚══════╝ ╚═════╝  ╚═════╝ "
        )
    );
    println!(
        "{}",
        paint(
            PEACH,
            "╭────────────────── SECOND EGO / VERIFIED CODING HARNESS ──────────────────╮"
        )
    );
    println!(
        "{}",
        paint(
            PARCHEMENT,
            "│  ENGINE     Rust state machine + provider boundary                       │"
        )
    );
    println!(
        "{}",
        paint(
            PEACH,
            "│  PIPELINE   understand > explore > plan > execute > verify               │"
        )
    );
    println!(
        "{}",
        paint(
            MUTED,
            "│  EVIDENCE   repository index · bounded tools · verification               │"
        )
    );
    println!(
        "{}",
        paint(
            PEACH,
            "╰─────────────────────────────────────────────────────────────────────────╯"
        )
    );
    println!(
        "  {}  Ctrl-C exits immediately at input; during a run, press once to stop safely.\n",
        paint(MUTED, "RUST ENGINE / INTERACTIVE")
    );
}

fn print_run_card(root: &std::path::Path, cloned: bool, provider: &str, model: &str) {
    let source = if cloned {
        "remote repository · shallow clone"
    } else {
        "local repository"
    };
    let root = panel_value(&root.display().to_string(), 61);
    let model = panel_value(model, 45);
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
    println!(
        "{}",
        paint(PEACH, &format!("│  MODEL    {provider:<13} {model}│"))
    );
    println!("{}", paint(PEACH, &format!("│  SOURCE   {source:<61}│")));
    println!("{}", paint(PEACH, &format!("│  ROOT     {root}│")));
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

fn panel_value(value: &str, width: usize) -> String {
    let count = value.chars().count();
    if count <= width {
        return format!("{value:<width$}");
    }
    let shortened = value
        .chars()
        .take(width.saturating_sub(1))
        .collect::<String>();
    format!("{shortened}…")
}

fn print_live_event(event: &secondego_core::EngineEvent) {
    let icon = match event.event_type.as_str() {
        "tool.completed" => "→",
        "verification.completed" => "✓",
        "run.terminated" => "■",
        _ => "·",
    };
    let message = event
        .payload
        .get("message")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            event
                .payload
                .get("reason")
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or(&event.event_type);
    println!(
        "  {} {:<10} {:<22} {}",
        paint(CORAL, icon),
        paint(MUTED, &format!("{:?}", event.phase)),
        event.event_type,
        message
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
