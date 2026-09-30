use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::thread;
use std::time::Duration;

use secondego_core::ActionProposal;
use secondego_model::{ConfiguredProvider, ProviderKind, ScriptedProvider, configured_model};
use secondego_runtime::discovery::{DiscoveryLens, DiscoveryRequest, discover};
use secondego_runtime::{
    CancellationToken, RuntimeError, RustEngine, collect_garbage, resolve_repository,
    resolve_repository_with_cancellation, voice::VoiceService,
};

const CORAL: &str = "38;2;255;205;0";
const PARCHEMENT: &str = "38;2;255;241;216";
const PEACH: &str = "38;2;223;143;45";
const MUTED: &str = "38;2;170;143;92";
static INTERRUPT_REQUESTED: AtomicBool = AtomicBool::new(false);
static RUN_ACTIVE: AtomicBool = AtomicBool::new(false);
static PARENT_TERMINAL_PGRP: AtomicI32 = AtomicI32::new(0);

fn augment_env_path() {
    if let Some(path) = env::var_os("PATH") {
        let mut paths = env::split_paths(&path).collect::<Vec<_>>();
        let home = env::var("HOME").unwrap_or_default();
        let common = vec![
            format!("{}/.cargo/bin", home),
            format!("{}/.local/bin", home),
            "/opt/homebrew/bin".to_string(),
            "/usr/local/bin".to_string(),
        ];
        for dir in common {
            let p = PathBuf::from(dir);
            if !paths.contains(&p) {
                paths.push(p);
            }
        }
        if let Ok(new_path) = env::join_paths(paths) {
            unsafe {
                env::set_var("PATH", new_path);
            }
        }
    }
}

fn main() {
    augment_env_path();
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
    if arguments
        .first()
        .is_some_and(|argument| argument == "preflight")
    {
        if let Err(error) = run_preflight(&arguments[1..]) {
            eprintln!("SecondEgo preflight failed: {error}");
            std::process::exit(2);
        }
        return;
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "launch")
    {
        let _terminal_foreground = take_terminal_foreground();
        if let Err(error) = run_launcher(&arguments[1..]) {
            eprintln!("SecondEgo launcher failed: {error}");
            std::process::exit(2);
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
            resolved.root.clone(),
            task,
            PathBuf::from(script_path),
            events,
            state_db,
            interactive,
            cancellation,
        ),
        None => run_configured(
            resolved.root.clone(),
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
        if interactive {
            print_error_card(error.as_ref(), &resolved.root);
        } else {
            eprintln!("SecondEgo failed: {error}");
        }
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
    println!("  secondego launch                       Resolve features, then start a run");
    println!("  secondego preflight                    Show safe capability status");
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Surface {
    Terminal,
    Desktop,
}

#[derive(Clone, Debug)]
struct LaunchConfig {
    profile: String,
    surface: Surface,
    terminal_mode: String,
    voice_enabled: bool,
    gestures_enabled: bool,
}

#[derive(Default)]
struct RuntimeCredentials {
    ai_api_key: Option<String>,
    groq_api_key: Option<String>,
    gemini_api_key: Option<String>,
}

fn run_preflight(arguments: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let configure = arguments.iter().any(|argument| argument == "--configure");
    let noninteractive = arguments
        .iter()
        .any(|argument| argument == "--noninteractive")
        || env_flag("SECONDEGO_NONINTERACTIVE")
        || !io::stdin().is_terminal()
        || !io::stdout().is_terminal();
    let mut config = launch_config_from_environment();
    if configure && !noninteractive {
        configure_launch(&mut config)?;
        save_local_profile(&config)?;
        println!(
            "  {} saved non-secret defaults to .secondego/profile.conf",
            paint(MUTED, "·")
        );
    }
    print_preflight(&config, true);
    Ok(())
}

fn run_launcher(_arguments: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let interactive = !env_flag("SECONDEGO_NONINTERACTIVE")
        && io::stdin().is_terminal()
        && io::stdout().is_terminal();
    let mut config = launch_config_from_environment();
    if interactive && env::var("SECONDEGO_PROFILE").is_err() {
        select_profile(&mut config)?;
    } else {
        apply_profile_defaults(&mut config);
    }

    let mut credentials = RuntimeCredentials {
        ai_api_key: non_empty_env("AI_API_KEY"),
        groq_api_key: non_empty_env("GROQ_API_KEY"),
        gemini_api_key: non_empty_env("GEMINI_API_KEY"),
    };
    let credentials_missing = !model_api_key_present(&credentials)
        || (config.voice_enabled && credentials.gemini_api_key.is_none());
    print_preflight_with_credentials(&config, &credentials);
    if interactive {
        collect_missing_credentials(&config, &mut credentials)?;
    }
    validate_launch(&config, &credentials, interactive)?;
    if credentials_missing {
        print_preflight_with_credentials(&config, &credentials);
    }

    match config.surface {
        Surface::Terminal => launch_terminal(&config, &credentials),
        Surface::Desktop => launch_desktop(&config, &credentials),
    }
}

fn launch_config_from_environment() -> LaunchConfig {
    let mut config = read_local_profile().unwrap_or_else(|| LaunchConfig {
        profile: "judge".into(),
        surface: Surface::Terminal,
        terminal_mode: "headless".into(),
        voice_enabled: false,
        gestures_enabled: false,
    });
    if let Ok(value) = env::var("SECONDEGO_PROFILE") {
        config.profile = value;
    }
    if let Ok(value) = env::var("SECONDEGO_SURFACE") {
        config.surface = match value.to_ascii_lowercase().as_str() {
            "desktop" | "ui" | "notch" => Surface::Desktop,
            _ => Surface::Terminal,
        };
    }
    if let Ok(value) = env::var("SECONDEGO_TERMINAL_MODE").or_else(|_| env::var("UI_MODE")) {
        config.terminal_mode = value;
    }
    if env::var("VOICE_ENABLED").is_ok() {
        config.voice_enabled = env_flag("VOICE_ENABLED");
    }
    if env::var("SECONDEGO_GESTURES_ENABLED").is_ok() {
        config.gestures_enabled = env_flag("SECONDEGO_GESTURES_ENABLED");
    }
    config
}

fn read_local_profile() -> Option<LaunchConfig> {
    let text = fs::read_to_string(".secondego/profile.conf").ok()?;
    let mut config = LaunchConfig {
        profile: "judge".into(),
        surface: Surface::Terminal,
        terminal_mode: "headless".into(),
        voice_enabled: false,
        gestures_enabled: false,
    };
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "profile" => config.profile = value.trim().to_owned(),
            "surface" => {
                config.surface = if value.trim() == "desktop" {
                    Surface::Desktop
                } else {
                    Surface::Terminal
                }
            }
            "terminal_mode" => config.terminal_mode = value.trim().to_owned(),
            "voice_enabled" => config.voice_enabled = parse_bool(value),
            "gestures_enabled" => config.gestures_enabled = parse_bool(value),
            _ => {}
        }
    }
    Some(config)
}

fn save_local_profile(config: &LaunchConfig) -> io::Result<()> {
    fs::create_dir_all(".secondego")?;
    let surface = match config.surface {
        Surface::Terminal => "terminal",
        Surface::Desktop => "desktop",
    };
    let contents = format!(
        "profile={}\nsurface={}\nterminal_mode={}\nvoice_enabled={}\ngestures_enabled={}\n",
        config.profile,
        surface,
        config.terminal_mode,
        config.voice_enabled,
        config.gestures_enabled,
    );
    fs::write(".secondego/profile.conf", contents)
}

fn select_profile(config: &mut LaunchConfig) -> Result<(), Box<dyn std::error::Error>> {
    println!();
    println!(
        "{}",
        paint(
            CORAL,
            "╭─ PREFLIGHT PROFILE ─────────────────────────────────────╮"
        )
    );
    println!(
        "{}",
        paint(
            PEACH,
            "│  1  Judge-safe       terminal · voice off · gestures off │"
        )
    );
    println!(
        "{}",
        paint(
            PEACH,
            "│  2  Fast terminal    compact output · no optional work  │"
        )
    );
    println!(
        "{}",
        paint(
            PEACH,
            "│  3  Desktop demo     notch UI · optional capabilities   │"
        )
    );
    println!(
        "{}",
        paint(
            PEACH,
            "│  4  Custom           choose each capability             │"
        )
    );
    println!(
        "{}",
        paint(
            CORAL,
            "╰────────────────────────────────────────────────────────╯"
        )
    );
    let choice = prompt("Profile [1]");
    match choice.trim() {
        "2" => config.profile = "fast".into(),
        "3" => config.profile = "desktop".into(),
        "4" => {
            config.profile = "custom".into();
            configure_launch(config)?;
            return Ok(());
        }
        _ => config.profile = "judge".into(),
    }
    apply_profile_defaults(config);
    Ok(())
}

fn apply_profile_defaults(config: &mut LaunchConfig) {
    let surface_overridden = env::var("SECONDEGO_SURFACE").is_ok();
    match config.profile.as_str() {
        "fast" => {
            if !surface_overridden {
                config.surface = Surface::Terminal;
            }
            config.terminal_mode = "headless".into();
            config.voice_enabled = false;
            config.gestures_enabled = false;
        }
        "desktop" => {
            if !surface_overridden {
                config.surface = Surface::Desktop;
            }
            if env::var("VOICE_ENABLED").is_err() {
                config.voice_enabled = false;
            }
            if env::var("SECONDEGO_GESTURES_ENABLED").is_err() {
                config.gestures_enabled = false;
            }
        }
        "custom" => {}
        _ => {
            if !surface_overridden {
                config.surface = Surface::Terminal;
            }
            if env::var("SECONDEGO_TERMINAL_MODE").is_err() && env::var("UI_MODE").is_err() {
                config.terminal_mode = "headless".into();
            }
            if env::var("VOICE_ENABLED").is_err() {
                config.voice_enabled = false;
            }
            if env::var("SECONDEGO_GESTURES_ENABLED").is_err() {
                config.gestures_enabled = false;
            }
        }
    }
}

fn configure_launch(config: &mut LaunchConfig) -> Result<(), Box<dyn std::error::Error>> {
    config.surface = if prompt("Surface [1=terminal, 2=desktop]").trim() == "2" {
        Surface::Desktop
    } else {
        Surface::Terminal
    };
    config.terminal_mode = if prompt("Terminal output [1=compact, 2=events]").trim() == "2" {
        "events".into()
    } else {
        "headless".into()
    };
    config.voice_enabled = prompt("Enable Gemini voice? [y/N]")
        .trim()
        .eq_ignore_ascii_case("y");
    config.gestures_enabled = config.surface == Surface::Desktop
        && prompt("Enable camera gestures? [y/N]")
            .trim()
            .eq_ignore_ascii_case("y");
    Ok(())
}

fn collect_missing_credentials(
    config: &LaunchConfig,
    credentials: &mut RuntimeCredentials,
) -> Result<(), Box<dyn std::error::Error>> {
    match ProviderKind::from_environment() {
        Ok(ProviderKind::Groq) if credentials.groq_api_key.is_none() => {
            credentials.groq_api_key = Some(prompt_secret(
                "GROQ_API_KEY is missing. Enter your Groq API key",
            )?);
        }
        Ok(ProviderKind::DeepSeek | ProviderKind::Gemini) | Err(_)
            if credentials.ai_api_key.is_none() =>
        {
            credentials.ai_api_key =
                Some(prompt_secret("AI_API_KEY is missing. Enter model API key")?);
        }
        _ => {}
    }
    if config.voice_enabled && credentials.gemini_api_key.is_none() {
        credentials.gemini_api_key = Some(prompt_secret(
            "Voice is enabled and GEMINI_API_KEY is missing. Enter Google AI Studio key",
        )?);
    }
    Ok(())
}

fn validate_launch(
    config: &LaunchConfig,
    credentials: &RuntimeCredentials,
    interactive: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !model_api_key_present(credentials) {
        return Err(format!(
            "{} is required for the selected model provider",
            model_api_key_name()
        )
        .into());
    }
    if config.voice_enabled
        && credentials
            .gemini_api_key
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        return Err(
            "voice is enabled but GEMINI_API_KEY is missing; disable voice or provide the key"
                .into(),
        );
    }
    if config.gestures_enabled && config.surface != Surface::Desktop {
        return Err("camera gestures require the desktop/notch surface".into());
    }
    if config.surface == Surface::Desktop && !command_available("node") {
        return Err("desktop surface requires Node.js/npm; run make setup-desktop first".into());
    }
    if !interactive
        && config.surface == Surface::Desktop
        && !PathBuf::from("apps/desktop/node_modules").is_dir()
    {
        return Err("desktop dependencies are not installed; run make setup-desktop first".into());
    }
    Ok(())
}

fn launch_terminal(
    config: &LaunchConfig,
    credentials: &RuntimeCredentials,
) -> Result<(), Box<dyn std::error::Error>> {
    let executable = env::current_exe()?;
    let mut command = Command::new(executable);
    command.arg("--interactive");
    if config.terminal_mode == "events" {
        command.args(["--ui", "events"]);
    }
    apply_child_environment(&mut command, config, credentials);
    let status = command.status()?;
    if status.success() || status.code() == Some(130) {
        Ok(())
    } else {
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn launch_desktop(
    config: &LaunchConfig,
    credentials: &RuntimeCredentials,
) -> Result<(), Box<dyn std::error::Error>> {
    let desktop_dir = PathBuf::from("apps/desktop");
    if !desktop_dir.join("node_modules").is_dir() {
        println!(
            "  {} installing optional desktop dependencies...",
            paint(MUTED, "→")
        );
        let status = Command::new("npm")
            .args(["--prefix", "apps/desktop", "ci"])
            .status()?;
        if !status.success() {
            return Err("npm dependency installation failed".into());
        }
    }
    let mut command = Command::new("npm");
    command.args(["--prefix", "apps/desktop", "run", "desktop"]);
    apply_child_environment(&mut command, config, credentials);
    let status = command.status()?;
    if status.success() || status.code() == Some(130) {
        Ok(())
    } else {
        Err(format!("desktop run exited with {status}").into())
    }
}

fn apply_child_environment(
    command: &mut Command,
    config: &LaunchConfig,
    credentials: &RuntimeCredentials,
) {
    if let Some(value) = &credentials.ai_api_key {
        command.env("AI_API_KEY", value);
    }
    if let Some(value) = &credentials.groq_api_key {
        command.env("GROQ_API_KEY", value);
    }
    if let Some(value) = &credentials.gemini_api_key {
        command.env("GEMINI_API_KEY", value);
    }
    command
        .env(
            "VOICE_ENABLED",
            if config.voice_enabled {
                "true"
            } else {
                "false"
            },
        )
        .env(
            "SECONDEGO_GESTURES_ENABLED",
            if config.gestures_enabled {
                "true"
            } else {
                "false"
            },
        );
}

fn print_preflight(config: &LaunchConfig, setup_only: bool) {
    let credentials = RuntimeCredentials {
        ai_api_key: non_empty_env("AI_API_KEY"),
        groq_api_key: non_empty_env("GROQ_API_KEY"),
        gemini_api_key: non_empty_env("GEMINI_API_KEY"),
    };
    print_preflight_with_credentials(config, &credentials);
    if setup_only {
        println!(
            "  {} setup is complete; no coding run was started.",
            paint(MUTED, "·")
        );
    }
}

fn print_preflight_with_credentials(config: &LaunchConfig, credentials: &RuntimeCredentials) {
    let provider = ProviderKind::from_environment()
        .map(|value| value.name().to_owned())
        .unwrap_or_else(|_| "invalid".into());
    let model = configured_model().unwrap_or_else(|_| "invalid model".into());
    let surface = match config.surface {
        Surface::Terminal => "terminal",
        Surface::Desktop => "desktop/notch",
    };
    let ui_status = if config.surface == Surface::Desktop {
        if command_available("node") {
            "READY"
        } else {
            "MISSING NODE"
        }
    } else {
        "OFF"
    };
    let voice_status = if !config.voice_enabled {
        "OFF".to_owned()
    } else if credentials.gemini_api_key.is_none() {
        "MISSING KEY".to_owned()
    } else if cfg!(target_os = "macos") && !command_available("afplay") {
        "MISSING PLAYER".to_owned()
    } else {
        "READY".to_owned()
    };
    let gesture_status = if !config.gestures_enabled {
        "OFF".to_owned()
    } else if config.surface != Surface::Desktop {
        "REQUIRES UI".to_owned()
    } else {
        "READY / CAMERA PROMPT".to_owned()
    };
    println!();
    println!(
        "{}",
        paint(
            CORAL,
            "╭─ SECONDEGO PREFLIGHT ────────────────────────────────────╮"
        )
    );
    println!(
        "{}",
        paint(PEACH, &format!("│  PROFILE    {:<40}│", config.profile))
    );
    println!(
        "{}",
        paint(PEACH, &format!("│  PROVIDER   {provider:<40}│"))
    );
    println!(
        "{}",
        paint(
            PEACH,
            &format!("│  MODEL      {:<40}│", panel_value(&model, 40))
        )
    );
    println!(
        "{}",
        paint(
            PEACH,
            &format!(
                "│  MODEL KEY  {:<40}│",
                secret_status(model_api_key_present(credentials))
            )
        )
    );
    println!("{}", paint(PEACH, &format!("│  SURFACE    {surface:<40}│")));
    println!(
        "{}",
        paint(
            PEACH,
            &format!("│  TERMINAL   {:<40}│", config.terminal_mode)
        )
    );
    println!(
        "{}",
        paint(PEACH, &format!("│  NOTCH UI   {ui_status:<40}│"))
    );
    println!(
        "{}",
        paint(PEACH, &format!("│  VOICE      {:<40}│", voice_status))
    );
    println!(
        "{}",
        paint(PEACH, &format!("│  GESTURES   {:<40}│", gesture_status))
    );
    println!(
        "{}",
        paint(
            CORAL,
            "╰────────────────────────────────────────────────────────╯"
        )
    );
}

fn secret_status(present: bool) -> &'static str {
    if present { "PRESENT" } else { "MISSING" }
}

fn model_api_key_name() -> &'static str {
    match ProviderKind::from_environment() {
        Ok(ProviderKind::Groq) => "GROQ_API_KEY",
        _ => "AI_API_KEY",
    }
}

fn model_api_key_present(credentials: &RuntimeCredentials) -> bool {
    match ProviderKind::from_environment() {
        Ok(ProviderKind::Groq) => credentials
            .groq_api_key
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty()),
        _ => credentials
            .ai_api_key
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty()),
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn env_flag(name: &str) -> bool {
    env::var(name).ok().is_some_and(|value| parse_bool(&value))
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn command_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn prompt_secret(label: &str) -> io::Result<String> {
    print!("{}: ", paint(PEACH, label));
    io::stdout().flush()?;
    #[cfg(unix)]
    {
        let fd = libc::STDIN_FILENO;
        let mut original = unsafe { std::mem::zeroed::<libc::termios>() };
        if unsafe { libc::tcgetattr(fd, &mut original) } == 0 {
            let mut hidden = original;
            hidden.c_lflag &= !libc::ECHO;
            unsafe { libc::tcsetattr(fd, libc::TCSANOW, &hidden) };
            let mut value = String::new();
            let result = io::stdin().read_line(&mut value);
            unsafe { libc::tcsetattr(fd, libc::TCSANOW, &original) };
            println!();
            return result.map(|_| value.trim().to_owned());
        }
    }
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(value.trim().to_owned())
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
    let voice = VoiceService::from_env();
    let mut engine = RustEngine::new(ScriptedProvider::new(vec![proposal]))
        .with_cancellation(cancellation)
        .with_voice(voice.clone());
    if interactive && !events {
        engine = engine.with_event_sink(print_live_event);
    }
    let report = engine.run(task, &workspace)?;
    emit_report(report, events, state_db, interactive, Some(&voice), &workspace)
}

fn run_configured(
    workspace: PathBuf,
    task: String,
    events: bool,
    state_db: Option<String>,
    interactive: bool,
    cancellation: CancellationToken,
) -> Result<(), Box<dyn std::error::Error>> {
    let voice = VoiceService::from_env();
    let mut engine = RustEngine::new(ConfiguredProvider::from_environment()?)
        .with_cancellation(cancellation)
        .with_voice(voice.clone());
    if interactive && !events {
        engine = engine.with_event_sink(print_live_event);
    }
    let report = engine.run(task, &workspace)?;
    emit_report(report, events, state_db, interactive, Some(&voice), &workspace)
}

fn emit_report(
    report: secondego_runtime::RunReport,
    events: bool,
    state_db: Option<String>,
    interactive: bool,
    voice: Option<&VoiceService>,
    workspace: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if events {
        for event in &report.events {
            println!("{}", serde_json::to_string(event)?);
        }
    }
    if let Some(path) = state_db {
        secondego_storage::SQLiteRunStore::open(path)?.save(&report.state, &report.events)?;
    }
    let patch_path = if let Some(ref patch) = report.patch {
        if !patch.is_empty() {
            let dot_secondego = PathBuf::from(".secondego");
            let _ = fs::create_dir_all(&dot_secondego);
            let path = dot_secondego.join("latest.patch");
            if fs::write(&path, patch).is_ok() {
                fs::canonicalize(&path).ok().or(Some(path))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    if interactive {
        print_result_card(&report, workspace, patch_path.as_deref());
        if let Some(voice) = voice {
            print_voice_summary(voice, report.state.run_id);
        }
    } else {
        println!("{}", serde_json::to_string_pretty(&report)?);
    }
    if report.verification_passed {
        Ok(())
    } else {
        Err("verification failed".into())
    }
}

fn print_voice_summary(voice: &VoiceService, run_id: uuid::Uuid) {
    let snapshot = voice.snapshot(run_id);
    let state = format!("{:?}", snapshot.state).to_ascii_lowercase();
    let detail = snapshot
        .last_error
        .as_deref()
        .map(|error| format!(" · {error}"))
        .unwrap_or_default();
    println!(
        "  {} voice {} · provider={} · queue={}{}",
        paint(MUTED, "♪"),
        state,
        snapshot.provider,
        snapshot.queue_length,
        detail
    );
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

fn truncate_str(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        value.to_string()
    } else {
        let truncated: String = value.chars().take(max_len.saturating_sub(3)).collect();
        format!("{truncated}...")
    }
}

fn compute_diffstat(patch: &str) -> Vec<(String, usize, usize)> {
    let mut stats = Vec::new();
    let mut current_file: Option<String> = None;
    let mut ins = 0;
    let mut del = 0;
    for line in patch.lines() {
        if line.starts_with("diff --git a/") {
            if let Some(file) = current_file.take() {
                stats.push((file, ins, del));
                ins = 0;
                del = 0;
            }
            if let Some(path) = line.split(" b/").nth(1) {
                current_file = Some(path.to_string());
            }
        } else if line.starts_with('+') && !line.starts_with("+++") {
            ins += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            del += 1;
        }
    }
    if let Some(file) = current_file {
        stats.push((file, ins, del));
    }
    stats
}

fn print_result_card(
    report: &secondego_runtime::RunReport,
    workspace: &Path,
    patch_file: Option<&Path>,
) {
    let status = if report.verification_passed {
        "VERIFIED - CHANGES TRANSFERRED TO REPOSITORY"
    } else {
        "VERIFICATION FAILED - CHANGES DISCARDED (REPO PROTECTED)"
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
            &format!(
                "│  files      {:<58}│",
                if report.changed_paths.is_empty() {
                    "none".to_string()
                } else {
                    format!("{} file(s) modified", report.changed_paths.len())
                }
            ),
        )
    );
    println!(
        "{}",
        paint(
            color,
            &format!(
                "│  transfer   {:<58}│",
                if report.diff_transferred {
                    "applied to target repository"
                } else {
                    "discarded (isolated worktree tests did not pass)"
                }
            )
        )
    );
    if let Some(reason) = &report.state.termination_reason {
        let first_line = reason.lines().next().unwrap_or(reason);
        println!(
            "{}",
            paint(
                color,
                &format!("│  detail     {:<58}│", truncate_str(first_line, 58))
            )
        );
    }
    println!(
        "{}",
        paint(
            color,
            "╰───────────────────────────────────────────────────────────────────────╯"
        )
    );
    if !report.verification_passed {
        println!(
            "  {} {}",
            paint(CORAL, "!"),
            paint(
                MUTED,
                "Verification commands failed. Target repository was left untouched for safety."
            )
        );
    } else {
        println!(
            "  {} {}",
            paint(PARCHEMENT, "✓"),
            paint(
                MUTED,
                "All verification checks passed. Verified changes were transferred to your repository."
            )
        );
    }

    if !report.changed_paths.is_empty() {
        println!();
        println!(
            "  {}",
            paint(PEACH, "Changed Files (Cmd+Click to open in editor):")
        );
        let stats = report
            .patch
            .as_deref()
            .map(compute_diffstat)
            .unwrap_or_default();
        for relative in &report.changed_paths {
            let abs_path = workspace.join(relative);
            let stat_str = stats
                .iter()
                .find(|(path, _, _)| path == relative)
                .map(|(_, ins, del)| format!(" (+{ins} -{del})"))
                .unwrap_or_default();
            println!(
                "  {} file://{}{}",
                paint(MUTED, "•"),
                abs_path.display(),
                paint(PEACH, &stat_str)
            );
        }
    }

    if let Some(patch_path) = patch_file {
        println!();
        println!("  {}", paint(PEACH, "Full Unified Patch:"));
        println!("  {} file://{}", paint(MUTED, "•"), patch_path.display());
    }

    let current_dir = env::current_dir().unwrap_or_default();
    if workspace != current_dir {
        println!();
        println!("  {}", paint(PEACH, "Target Repository Location:"));
        println!("  {} file://{}", paint(MUTED, "•"), workspace.display());
        if report.diff_transferred {
            println!("  {} Inspect diff in target repo directly:", paint(MUTED, "→"));
            println!("    git -C \"{}\" diff", workspace.display());
        }
    }
}

fn print_error_card(error: &dyn std::error::Error, workspace: &Path) {
    let color = CORAL;
    println!();
    println!(
        "{}",
        paint(
            color,
            "╭─ RUN TERMINATED ─────────────────────────────────────────────────────╮"
        )
    );
    println!(
        "{}",
        paint(
            color,
            "│  EXECUTION STOPPED - TARGET REPOSITORY UNTOUCHED (PROTECTED)          │"
        )
    );
    println!(
        "{}",
        paint(
            color,
            &format!(
                "│  workspace  {:<58}│",
                truncate_str(&workspace.display().to_string(), 58)
            )
        )
    );
    println!(
        "{}",
        paint(
            color,
            &format!("│  status     {:<58}│", "isolated worktree discarded cleanly")
        )
    );
    let error_str = error.to_string();
    let first = error_str.lines().next().unwrap_or(&error_str);
    println!(
        "{}",
        paint(
            color,
            &format!("│  cause      {:<58}│", truncate_str(first, 58))
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
        "  {} {}",
        paint(CORAL, "!"),
        paint(
            MUTED,
            "No unverified changes were committed to your repository. The working tree remains safe."
        )
    );
    println!();
    println!("  {}", paint(PEACH, "Failure Details:"));
    println!("  {} {}", paint(MUTED, "•"), error);
    if error_str.contains("clean target repository") {
        println!();
        println!("  {}", paint(PEACH, "How to resolve:"));
        println!(
            "  {} Target repository has uncommitted changes from a previous run or edit.",
            paint(MUTED, "•")
        );
        println!(
            "  {} To discard uncommitted changes: git -C \"{}\" restore .",
            paint(MUTED, "•"),
            workspace.display()
        );
        println!(
            "  {} To keep uncommitted changes:    git -C \"{}\" commit -am \"save current work\"",
            paint(MUTED, "•"),
            workspace.display()
        );
    }
    println!();
    println!("  {}", paint(PEACH, "Target Repository Location:"));
    println!("  {} file://{}", paint(MUTED, "•"), workspace.display());
    println!();
}

fn paint(code: &str, value: &str) -> String {
    if io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none() {
        format!("\x1b[{code}m{value}\x1b[0m")
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{LaunchConfig, Surface, parse_bool, secret_status};

    #[test]
    fn boolean_configuration_accepts_safe_user_forms() {
        assert!(parse_bool("true"));
        assert!(parse_bool("YES"));
        assert!(parse_bool("1"));
        assert!(!parse_bool("false"));
        assert!(!parse_bool("off"));
    }

    #[test]
    fn safe_defaults_disable_optional_surfaces() {
        let config = LaunchConfig {
            profile: "judge".into(),
            surface: Surface::Terminal,
            terminal_mode: "headless".into(),
            voice_enabled: false,
            gestures_enabled: false,
        };
        assert_eq!(config.surface, Surface::Terminal);
        assert!(!config.voice_enabled);
        assert!(!config.gestures_enabled);
        assert_eq!(secret_status(false), "MISSING");
    }
}
