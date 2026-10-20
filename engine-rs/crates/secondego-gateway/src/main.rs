use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use secondego_core::{EngineEvent, ExecutionState};
use secondego_model::{
    ConfiguredProvider, ModelProvider, ProviderContext, ProviderKind,
    available_models_for_provider, available_models_for_provider_with_key, configured_model,
};
use secondego_runtime::discovery::{
    DiscoveryLens, DiscoveryRequest, discover_with_observer_and_id,
};
use secondego_runtime::{
    CancellationToken, RunReport, RuntimeError, RustEngine, collect_garbage,
    resolve_repository_with_cancellation, validate_repository_source,
    voice::{VoiceService, VoiceStatusHandle},
};
use secondego_storage::{RunMetadata, SQLiteRunStore};
use uuid::Uuid;

const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_ACTIVE_CONNECTIONS: usize = 64;
const MAX_ACTIVE_RUNS: usize = 32;
const MAX_IN_MEMORY_RUNS: usize = 256;
const MAX_EVENTS_PER_RESPONSE: usize = 128;
const API_VERSION: u32 = 1;

type RunStore = Arc<Mutex<SQLiteRunStore>>;
type CredentialSession = Arc<Mutex<Option<ProviderSession>>>;

#[derive(Clone)]
struct ProviderSession {
    profile_id: String,
    provider: ProviderKind,
    model: String,
    api_key: String,
}

#[derive(Clone)]
struct RunRecord {
    repository: String,
    issue: String,
    profile_id: Option<String>,
    provider: String,
    model: String,
    status: String,
    events: Vec<EngineEvent>,
    report: Option<serde_json::Value>,
    error: Option<String>,
    cancellation: CancellationToken,
    voice: VoiceStatusHandle,
    created_at: Instant,
}

struct ConnectionPermit(Arc<AtomicUsize>);

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gc = collect_garbage();
    if gc.deleted > 0 {
        eprintln!(
            "SecondEgo garbage collector: reclaimed {} stale temp directories",
            gc.deleted
        );
    }
    let token = match env::var("SECONDEGO_UI_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        Some(token) => token,
        None if cfg!(debug_assertions) => "local-development-token".into(),
        None => {
            return Err(
                "SECONDEGO_UI_TOKEN is required when running the release gateway".into(),
            );
        }
    };
    let port = env::var("SECONDEGO_GATEWAY_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8787);
    let ui_root = env::var_os("SECONDEGO_UI_DIST")
        .map(PathBuf::from)
        .filter(|path| path.is_dir());
    let store = open_state_store()?;
    let reconciled = store
        .lock()
        .map_err(|_| "state store lock poisoned")?
        .reconcile_incomplete_runs("gateway restarted while the run was active")?;
    if reconciled > 0 {
        eprintln!(
            "SecondEgo Rust gateway: marked {reconciled} interrupted run(s) as failed"
        );
    }
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let runs: Arc<Mutex<HashMap<Uuid, RunRecord>>> = Arc::new(Mutex::new(HashMap::new()));
    let credentials: CredentialSession = Arc::new(Mutex::new(None));
    let active_connections = Arc::new(AtomicUsize::new(0));
    // Voice is presentation-only, but its background worker must be shared by
    // the long-lived gateway rather than leaked once for every submitted run.
    let voice = Arc::new(VoiceService::from_env());
    eprintln!("SecondEgo Rust gateway listening on http://127.0.0.1:{port}");
    for stream in listener.incoming().flatten() {
        let permit =
            match active_connections.fetch_update(Ordering::AcqRel, Ordering::Relaxed, |count| {
                (count < MAX_ACTIVE_CONNECTIONS).then_some(count + 1)
            }) {
                Ok(_) => ConnectionPermit(active_connections.clone()),
                Err(_) => {
                    let mut stream = stream;
                    let _ = respond(
                        &mut stream,
                        503,
                        serde_json::json!({"error":"gateway connection limit reached"}),
                    );
                    continue;
                }
            };
        let token = token.clone();
        let runs = runs.clone();
        let ui_root = ui_root.clone();
        let voice = voice.clone();
        let store = store.clone();
        let credentials = credentials.clone();
        thread::spawn(move || {
            let _permit = permit;
            let _ = handle(
                stream,
                &token,
                &runs,
                ui_root.as_deref(),
                &voice,
                &store,
                &credentials,
            );
        });
    }
    Ok(())
}

fn open_state_store() -> Result<RunStore, Box<dyn std::error::Error>> {
    let store = match env::var_os("SECONDEGO_STATE_DB") {
        Some(path) => {
            let path = PathBuf::from(path);
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                fs::create_dir_all(parent)?;
            }
            SQLiteRunStore::open(path)?
        }
        None => SQLiteRunStore::open_memory()?,
    };
    Ok(Arc::new(Mutex::new(store)))
}

fn handle(
    mut stream: TcpStream,
    token: &str,
    runs: &Arc<Mutex<HashMap<Uuid, RunRecord>>>,
    ui_root: Option<&Path>,
    voice: &Arc<VoiceService>,
    store: &RunStore,
    credentials: &CredentialSession,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let request = read_request(&mut stream)?;
    if request.method == "GET" {
        let route = request.path.split('?').next().unwrap_or_default();
        if route == "/" || route.starts_with("/assets/") {
            if let Some(ui_root) = ui_root {
                return serve_ui(&mut stream, ui_root, route);
            }
        }
    }
    // CORS preflight requests intentionally carry no application token. They
    // only ask whether the renderer may send the authenticated request.
    if request.method == "OPTIONS" {
        return respond(&mut stream, 204, serde_json::json!({}));
    }
    if request.headers.get("x-secondego-token").map(String::as_str) != Some(token) {
        return respond(
            &mut stream,
            401,
            serde_json::json!({"error":"unauthorized"}),
        );
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/health") => respond(
            &mut stream,
            200,
            serde_json::json!({"ok":true,"api_version":API_VERSION,"engine":"rust"}),
        ),
        ("POST", "/api/providers/session") => {
            set_provider_session(&mut stream, request.body, credentials)
        }
        ("DELETE", "/api/providers/session") => clear_provider_session(&mut stream, credentials),
        ("POST", "/api/providers/test") => test_provider(&mut stream, request.body, credentials),
        ("POST", "/api/providers/models") => {
            list_provider_models(&mut stream, request.body, credentials)
        }
        ("POST", "/api/runs") => create_run(&mut stream, request.body, runs, voice, store, credentials),
        ("GET", path) if path.split('?').next() == Some("/api/runs") => {
            list_runs(&mut stream, path, store)
        }
        ("POST", path) if path.starts_with("/api/runs/") && path.ends_with("/cancel") => {
            cancel_run(&mut stream, path, runs)
        }
        ("GET", path) if path.starts_with("/api/runs/") => get_run(&mut stream, path, runs, store),
        _ => respond(&mut stream, 404, serde_json::json!({"error":"not found"})),
    }
}

fn set_provider_session(
    stream: &mut TcpStream,
    body: Vec<u8>,
    credentials: &CredentialSession,
) -> std::io::Result<()> {
    let request: ProviderSessionRequest = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return respond(stream, 400, serde_json::json!({"error":"invalid request"})),
    };
    if !valid_profile_id(&request.profile_id) {
        return respond(
            stream,
            400,
            serde_json::json!({"error":"profile_id contains unsupported characters"}),
        );
    }
    if request.api_key.trim().is_empty() || request.api_key.len() > 512 {
        return respond(
            stream,
            400,
            serde_json::json!({"error":"api_key must be between 1 and 512 bytes"}),
        );
    }
    let provider = match ProviderKind::from_name(&request.provider) {
        Ok(provider) => provider,
        Err(error) => return respond(stream, 400, serde_json::json!({"error":error.to_string()})),
    };
    let model = request
        .model
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| provider.default_model().into());
    if model.chars().count() > 256 {
        return respond(stream, 400, serde_json::json!({"error":"model identifier exceeds field limits"}));
    }
    if let Err(error) = ConfiguredProvider::from_provider_key(
        provider,
        model.clone(),
        request.api_key.clone(),
    ) {
        return respond(stream, 400, serde_json::json!({"error":error.to_string()}));
    }
    let session = ProviderSession {
        profile_id: request.profile_id,
        provider,
        model,
        api_key: request.api_key,
    };
    if let Ok(mut active) = credentials.lock() {
        *active = Some(session.clone());
    } else {
        return respond(stream, 503, serde_json::json!({"error":"credential session unavailable"}));
    }
    respond(
        stream,
        200,
        serde_json::json!({
            "api_version": API_VERSION,
            "ok": true,
            "profile_id": session.profile_id,
            "provider": session.provider.name(),
            "model": session.model,
        }),
    )
}

fn clear_provider_session(
    stream: &mut TcpStream,
    credentials: &CredentialSession,
) -> std::io::Result<()> {
    if let Ok(mut active) = credentials.lock() {
        *active = None;
    } else {
        return respond(stream, 503, serde_json::json!({"error":"credential session unavailable"}));
    }
    respond(
        stream,
        200,
        serde_json::json!({"api_version": API_VERSION, "ok": true, "cleared": true}),
    )
}

fn test_provider(
    stream: &mut TcpStream,
    body: Vec<u8>,
    credentials: &CredentialSession,
) -> std::io::Result<()> {
    let request: ProviderTestRequest = if body.is_empty() {
        ProviderTestRequest::default()
    } else {
        match serde_json::from_slice(&body) {
            Ok(value) => value,
            Err(_) => return respond(stream, 400, serde_json::json!({"error":"invalid request"})),
        }
    };
    let session = session_snapshot(credentials);
    if let Some(profile_id) = request.profile_id.as_deref() {
        if !valid_profile_id(profile_id) {
            return respond(
                stream,
                400,
                serde_json::json!({"error":"invalid profile id"}),
            );
        }
        if session.as_ref().map(|session| session.profile_id.as_str()) != Some(profile_id)
            && env::var("SECONDEGO_PROFILE_ID").ok().as_deref() != Some(profile_id)
        {
            return respond(
                stream,
                409,
                serde_json::json!({"error":"profile is not the active local profile"}),
            );
        }
    }
    let started = std::time::Instant::now();
    let active_provider = match provider_kind_from_session(session.as_ref()) {
        Ok(provider) => provider,
        Err(error) => {
            return respond(
                stream,
                500,
                serde_json::json!({"api_version":API_VERSION,"ok":false,"error":error.to_string()}),
            );
        }
    };
    let requested_model = request.model.clone();
    let result = configured_provider_from_session(session.as_ref())
        .and_then(|provider| {
            if let Some(requested_provider) = request.provider.as_deref()
                && !requested_provider.eq_ignore_ascii_case(active_provider.name())
            {
                return Err(secondego_model::ProviderError::UnsupportedProvider(
                    requested_provider.to_owned(),
                ));
            }
            let model = requested_model
                .clone()
                .or_else(|| session.as_ref().map(|session| session.model.clone()))
                .unwrap_or(configured_model()?);
            provider.with_model(model)
        })
        .and_then(|provider| {
            provider.generate(
                "Return a submit_plan JSON object with empty arguments and rationale 'connection test'.",
                &ProviderContext {
                    run_id: "provider-test".into(),
                    phase: "PLAN".into(),
                },
            )
        });
    match result {
        Ok(_) => respond(
            stream,
            200,
            serde_json::json!({
                "api_version": API_VERSION,
                "ok": true,
                "latency_ms": started.elapsed().as_millis(),
                "profile_id": request.profile_id.or_else(|| session.as_ref().map(|session| session.profile_id.clone()).or_else(|| env::var("SECONDEGO_PROFILE_ID").ok())),
                "provider": active_provider.name(),
                "model": requested_model
                    .or_else(|| session.as_ref().map(|session| session.model.clone()))
                    .or_else(|| configured_model().ok())
                    .unwrap_or_else(|| "unknown".into()),
            }),
        ),
        Err(error) => respond(
            stream,
            502,
            serde_json::json!({"api_version":API_VERSION,"ok":false,"error":error.to_string()}),
        ),
    }
}

fn list_provider_models(
    stream: &mut TcpStream,
    body: Vec<u8>,
    credentials: &CredentialSession,
) -> std::io::Result<()> {
    let request: ProviderModelsRequest = if body.is_empty() {
        ProviderModelsRequest::default()
    } else {
        match serde_json::from_slice(&body) {
            Ok(value) => value,
            Err(_) => return respond(stream, 400, serde_json::json!({"error":"invalid request"})),
        }
    };
    let session = session_snapshot(credentials);
    if let Some(profile_id) = request.profile_id.as_deref() {
        if !valid_profile_id(profile_id) {
            return respond(
                stream,
                400,
                serde_json::json!({"error":"invalid profile id"}),
            );
        }
        if session.as_ref().map(|session| session.profile_id.as_str()) != Some(profile_id)
            && env::var("SECONDEGO_PROFILE_ID").ok().as_deref() != Some(profile_id)
        {
            return respond(
                stream,
                409,
                serde_json::json!({"error":"profile is not the active local profile"}),
            );
        }
    }
    let provider = match provider_kind_from_session(session.as_ref()) {
        Ok(provider) => provider,
        Err(error) => {
            return respond(
                stream,
                500,
                serde_json::json!({"api_version":API_VERSION,"ok":false,"error":error.to_string()}),
            );
        }
    };
    if let Some(requested_provider) = request.provider.as_deref()
        && !requested_provider.eq_ignore_ascii_case(provider.name())
    {
        return respond(
            stream,
            409,
            serde_json::json!({"error":"provider is not the active local provider"}),
        );
    }
    let started = std::time::Instant::now();
    let models = session
        .as_ref()
        .map(|session| available_models_for_provider_with_key(provider, &session.api_key))
        .unwrap_or_else(|| available_models_for_provider(provider));
    match models {
        Ok(models) => respond(
            stream,
            200,
            serde_json::json!({
                "api_version": API_VERSION,
                "ok": true,
                "provider": provider.name(),
                "default_model": provider.default_model(),
                "latency_ms": started.elapsed().as_millis(),
                "models": models,
            }),
        ),
        Err(error) => respond(
            stream,
            502,
            serde_json::json!({"api_version":API_VERSION,"ok":false,"provider":provider.name(),"error":error.to_string()}),
        ),
    }
}

fn serve_ui(stream: &mut TcpStream, root: &Path, route: &str) -> std::io::Result<()> {
    let root = root.canonicalize()?;
    let relative = if route == "/" {
        PathBuf::from("index.html")
    } else {
        PathBuf::from(route.trim_start_matches('/'))
    };
    let candidate = root.join(relative).canonicalize();
    let Ok(candidate) = candidate else {
        return respond(stream, 404, serde_json::json!({"error":"asset not found"}));
    };
    if !candidate.starts_with(&root) || !candidate.is_file() {
        return respond(stream, 404, serde_json::json!({"error":"asset not found"}));
    }
    let body = fs::read(&candidate)?;
    let content_type = match candidate
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    };
    respond_bytes(stream, 200, content_type, &body)
}

fn create_run(
    stream: &mut TcpStream,
    body: Vec<u8>,
    runs: &Arc<Mutex<HashMap<Uuid, RunRecord>>>,
    voice: &VoiceService,
    store: &RunStore,
    credentials: &CredentialSession,
) -> std::io::Result<()> {
    let request: StartRequest = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return respond(stream, 400, serde_json::json!({"error":"invalid request"})),
    };
    if let Err(error) = validate_start_request(&request) {
        return respond(stream, 400, serde_json::json!({"error":error}));
    }
    if let Err(error) = validate_repository_source(&request.repository) {
        return respond(stream, 400, serde_json::json!({"error":error.to_string()}));
    }
    let is_discovery = request.mode.as_deref() == Some("discover");
    // Capture the session before deriving any run metadata. Provider, model,
    // profile id, and the worker credential must all come from one coherent
    // point in time, even if the user switches profiles concurrently.
    let run_session = session_snapshot(credentials);
    if let Some(profile_id) = request.profile_id.as_deref()
        && !valid_profile_id(profile_id)
    {
        return respond(
            stream,
            400,
            serde_json::json!({"error":"profile_id contains unsupported characters"}),
        );
    }
    if let Some(profile_id) = request.profile_id.as_deref()
        && run_session.as_ref().map(|session| session.profile_id.as_str()) != Some(profile_id)
        && env::var("SECONDEGO_PROFILE_ID").ok().as_deref() != Some(profile_id)
    {
        return respond(
            stream,
            409,
            serde_json::json!({"error":"profile is not the active local profile"}),
        );
    }
    let model = match request.model.filter(|model| !model.trim().is_empty()) {
        Some(model) => model,
        None if is_discovery => "deterministic-discovery".into(),
        None => match run_session
            .as_ref()
            .map(|session| session.model.clone())
            .or_else(|| configured_model().ok())
        {
            Some(model) => model,
            None => {
                return respond(
                    stream,
                    500,
                    serde_json::json!({"error":"no configured provider model"}),
                );
            }
        },
    };
    let profile_id = request
        .profile_id
        .clone()
        .or_else(|| run_session.as_ref().map(|session| session.profile_id.clone()))
        .or_else(|| env::var("SECONDEGO_PROFILE_ID").ok());
    let provider = if is_discovery {
        "discovery".to_owned()
    } else {
        run_session
            .as_ref()
            .map(|session| session.provider.name().to_owned())
            .or_else(|| secondego_model::ProviderKind::from_environment().ok().map(|provider| provider.name().to_owned()))
            .unwrap_or_else(|| "unknown".into())
    };
    let id = Uuid::new_v4();
    let cancellation = CancellationToken::default();
    let voice_status = voice.status_handle();
    let created_at = Instant::now();
    {
        let mut all_runs = runs.lock().unwrap();
        let active_count = all_runs
            .values()
            .filter(|record| !is_terminal_status(&record.status))
            .count();
        if active_count >= MAX_ACTIVE_RUNS {
            return respond(
                stream,
                429,
                serde_json::json!({"error":"active run limit reached","limit":MAX_ACTIVE_RUNS}),
            );
        }
        prune_terminal_runs(&mut all_runs);
        all_runs.insert(
            id,
            RunRecord {
                repository: request.repository.clone(),
                issue: request.issue.clone(),
                profile_id: profile_id.clone(),
                provider: provider.clone(),
                model: model.clone(),
                status: "QUEUED".into(),
                events: Vec::new(),
                report: None,
                error: None,
                cancellation: cancellation.clone(),
                voice: voice_status.clone(),
                created_at,
            },
        );
    }
    let mut initial_state = ExecutionState::new(request.issue.clone(), request.repository.clone());
    initial_state.run_id = id;
    persist_snapshot(
        &store,
        &initial_state,
        &[],
        &RunMetadata {
            profile_id: profile_id.clone(),
            workspace_id: Some(request.repository.clone()),
            provider: Some(provider.clone()),
            model: Some(model.clone()),
            engine_version: Some(env!("CARGO_PKG_VERSION").into()),
        },
        None,
        None,
    );
    let runs_for_thread = runs.clone();
    let record_repository = request.repository.clone();
    let record_issue = request.issue.clone();
    let record_model = model.clone();
    let record_profile_id = profile_id.clone();
    let record_provider = provider.clone();
    let record_mode = request.mode.clone().unwrap_or_else(|| "task".into());
    let discovery_lenses = request.lenses.clone();
    let discovery_max_findings = request.max_findings.unwrap_or(20);
    let event_runs = runs.clone();
    let run_cancellation = cancellation.clone();
    let run_voice = voice.clone();
    let run_store = store.clone();
    thread::spawn(move || {
        let resolved =
            match resolve_repository_with_cancellation(&record_repository, Some(&run_cancellation))
            {
                Ok(value) => value,
                Err(error) => {
                    let message = error.to_string();
                    let cancelled = matches!(error, RuntimeError::Cancelled);
                    let status = if cancelled {
                        secondego_core::TerminalStatus::Cancelled
                    } else {
                        secondego_core::TerminalStatus::Failed
                    };
                    let state = fallback_state(
                        id,
                        record_issue.clone(),
                        record_repository.clone(),
                        status,
                        message.clone(),
                    );
                    persist_snapshot(
                        &run_store,
                        &state,
                        &[],
                        &RunMetadata {
                            profile_id: record_profile_id.clone(),
                            workspace_id: Some(record_repository.clone()),
                            provider: Some(record_provider.clone()),
                            model: Some(record_model.clone()),
                            engine_version: Some(env!("CARGO_PKG_VERSION").into()),
                        },
                        None,
                        (!cancelled).then_some(message.as_str()),
                    );
                    if let Ok(mut all_runs) = runs_for_thread.lock() {
                        if let Some(record) = all_runs.get_mut(&id) {
                            record.status = if cancelled { "CANCELLED" } else { "FAILED" }.into();
                            record.error = (!cancelled).then_some(message);
                        }
                    }
                    return;
                }
            };
        if record_mode == "discover" {
            if let Ok(mut all_runs) = runs_for_thread.lock() {
                if let Some(record) = all_runs.get_mut(&id) {
                    record.status = "RUNNING".into();
                }
            }
            let event_sink_runs = runs_for_thread.clone();
            let event_sink = move |event: &EngineEvent| {
                if let Ok(mut all_runs) = event_sink_runs.lock() {
                    if let Some(record) = all_runs.get_mut(&id) {
                        record.events.push(event.clone());
                        record.status = match event.status {
                            secondego_core::TerminalStatus::Complete => "COMPLETE",
                            secondego_core::TerminalStatus::Failed => "FAILED",
                            secondego_core::TerminalStatus::Cancelled => "CANCELLED",
                            secondego_core::TerminalStatus::Blocked => "BLOCKED",
                            secondego_core::TerminalStatus::Running => "RUNNING",
                        }
                        .into();
                    }
                }
            };
            let lenses = discovery_lenses
                .unwrap_or_default()
                .into_iter()
                .filter_map(|value| DiscoveryLens::parse(&value))
                .collect();
            let result = discover_with_observer_and_id(
                &resolved.root,
                DiscoveryRequest {
                    lenses,
                    max_findings: discovery_max_findings,
                },
                Some(id),
                event_sink,
            );
            let events = runs_for_thread
                .lock()
                .ok()
                .and_then(|all_runs| all_runs.get(&id).map(|record| record.events.clone()))
                .unwrap_or_default();
            let record = match result {
                Ok(report) => {
                    let result = serde_json::to_value(&report).ok();
                    let report_events = if events.is_empty() {
                        report.events.clone()
                    } else {
                        events
                    };
                    persist_snapshot(
                        &run_store,
                        &report.state,
                        &report_events,
                        &RunMetadata {
                            profile_id: record_profile_id.clone(),
                            workspace_id: Some(resolved.root.to_string_lossy().into_owned()),
                            provider: Some(record_provider.clone()),
                            model: Some(record_model.clone()),
                            engine_version: Some(env!("CARGO_PKG_VERSION").into()),
                        },
                        result.as_ref(),
                        None,
                    );
                    RunRecord {
                        repository: resolved.root.to_string_lossy().into_owned(),
                        issue: record_issue,
                        profile_id: record_profile_id,
                        provider: record_provider,
                        model: record_model,
                        status: "COMPLETE".into(),
                        events: report_events,
                        report: result,
                        error: None,
                        cancellation: run_cancellation.clone(),
                        voice: voice_status.clone(),
                        created_at,
                    }
                }
                Err(error) => {
                    let state = fallback_state(
                        id,
                        record_issue.clone(),
                        resolved.root.to_string_lossy().into_owned(),
                        secondego_core::TerminalStatus::Failed,
                        error.clone(),
                    );
                    persist_snapshot(
                        &run_store,
                        &state,
                        &events,
                        &RunMetadata {
                            profile_id: record_profile_id.clone(),
                            workspace_id: Some(resolved.root.to_string_lossy().into_owned()),
                            provider: Some(record_provider.clone()),
                            model: Some(record_model.clone()),
                            engine_version: Some(env!("CARGO_PKG_VERSION").into()),
                        },
                        None,
                        Some(error.as_str()),
                    );
                    RunRecord {
                        repository: record_repository,
                        issue: record_issue,
                        profile_id: record_profile_id,
                        provider: record_provider,
                        model: record_model,
                        status: "FAILED".into(),
                        events,
                        report: None,
                        error: Some(error),
                        cancellation: run_cancellation.clone(),
                        voice: voice_status.clone(),
                        created_at,
                    }
                }
            };
            if let Ok(mut all_runs) = runs_for_thread.lock() {
                all_runs.insert(id, record);
            }
            return;
        }
        let provider = match configured_provider_from_session(run_session.as_ref())
            .and_then(|provider| provider.with_model(model))
        {
            Ok(provider) => provider,
            Err(error) => {
                let message = error.to_string();
                let events = runs_for_thread
                    .lock()
                    .ok()
                    .and_then(|all_runs| all_runs.get(&id).map(|record| record.events.clone()))
                    .unwrap_or_default();
                let state = fallback_state(
                    id,
                    record_issue.clone(),
                    resolved.root.to_string_lossy().into_owned(),
                    secondego_core::TerminalStatus::Failed,
                    message.clone(),
                );
                persist_snapshot(
                    &run_store,
                    &state,
                    &events,
                    &RunMetadata {
                        profile_id: record_profile_id.clone(),
                        workspace_id: Some(resolved.root.to_string_lossy().into_owned()),
                        provider: Some(record_provider.clone()),
                        model: Some(record_model.clone()),
                        engine_version: Some(env!("CARGO_PKG_VERSION").into()),
                    },
                    None,
                    Some(message.as_str()),
                );
                if let Ok(mut all_runs) = runs_for_thread.lock() {
                    if let Some(record) = all_runs.get_mut(&id) {
                        record.status = "FAILED".into();
                        record.error = Some(message);
                    }
                }
                return;
            }
        };
        let event_sink = move |event: &EngineEvent| {
            if let Ok(mut all_runs) = event_runs.lock() {
                if let Some(record) = all_runs.get_mut(&id) {
                    record.events.push(event.clone());
                    record.status = match event.status {
                        secondego_core::TerminalStatus::Complete => "COMPLETE",
                        secondego_core::TerminalStatus::Failed => "FAILED",
                        secondego_core::TerminalStatus::Cancelled => "CANCELLED",
                        secondego_core::TerminalStatus::Blocked => "BLOCKED",
                        secondego_core::TerminalStatus::Running => "RUNNING",
                    }
                    .into();
                }
            }
        };
        let mut engine = RustEngine::new(provider)
            .with_event_sink(event_sink)
            .with_voice(run_voice.clone());
        engine = engine.with_cancellation(run_cancellation.clone());
        let result = engine.run_with_id(request.issue, resolved.root.clone(), id);
        let events = runs_for_thread
            .lock()
            .ok()
            .and_then(|all_runs| all_runs.get(&id).map(|record| record.events.clone()))
            .unwrap_or_default();
        let record = match result {
            Ok(report) => {
                let verified = report.verification_passed;
                let error = (!verified).then(|| {
                    report
                        .verification
                        .failure_summary
                        .clone()
                        .unwrap_or_else(|| "verification failed; attempt diff was discarded".into())
                });
                let payload = result_payload(&report);
                persist_snapshot(
                    &run_store,
                    &report.state,
                    &report.events,
                    &RunMetadata {
                        profile_id: record_profile_id.clone(),
                        workspace_id: Some(resolved.root.to_string_lossy().into_owned()),
                        provider: Some(record_provider.clone()),
                        model: Some(record_model.clone()),
                        engine_version: Some(env!("CARGO_PKG_VERSION").into()),
                    },
                    Some(&payload),
                    error.as_deref(),
                );
                RunRecord {
                    repository: resolved.root.to_string_lossy().into_owned(),
                    issue: record_issue.clone(),
                    profile_id: record_profile_id.clone(),
                    provider: record_provider.clone(),
                    model: record_model.clone(),
                    status: if verified { "COMPLETE" } else { "FAILED" }.into(),
                    events,
                    report: Some(payload),
                    error,
                    cancellation: run_cancellation.clone(),
                    voice: voice_status.clone(),
                    created_at,
                }
            }
            Err(error) => {
                let cancelled = matches!(error, RuntimeError::Cancelled);
                let message = error.to_string();
                let status = if cancelled {
                    secondego_core::TerminalStatus::Cancelled
                } else {
                    secondego_core::TerminalStatus::Failed
                };
                let state = fallback_state(
                    id,
                    record_issue.clone(),
                    record_repository.clone(),
                    status,
                    message.clone(),
                );
                persist_snapshot(
                    &run_store,
                    &state,
                    &events,
                    &RunMetadata {
                        profile_id: record_profile_id.clone(),
                        workspace_id: Some(record_repository.clone()),
                        provider: Some(record_provider.clone()),
                        model: Some(record_model.clone()),
                        engine_version: Some(env!("CARGO_PKG_VERSION").into()),
                    },
                    None,
                    (!cancelled).then_some(message.as_str()),
                );
                RunRecord {
                    repository: record_repository,
                    issue: record_issue,
                    profile_id: record_profile_id,
                    provider: record_provider,
                    model: record_model,
                    status: if cancelled { "CANCELLED" } else { "FAILED" }.into(),
                    events,
                    report: None,
                    error: (!cancelled).then_some(message),
                    cancellation: run_cancellation.clone(),
                    voice: voice_status.clone(),
                    created_at,
                }
            }
        };
        if let Ok(mut all_runs) = runs_for_thread.lock() {
            all_runs.insert(id, record);
        }
    });
    respond(
        stream,
        202,
        serde_json::json!({"api_version":API_VERSION,"request_id":id,"status":"QUEUED"}),
    )
}

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "COMPLETE" | "FAILED" | "CANCELLED" | "BLOCKED")
}

fn prune_terminal_runs(runs: &mut HashMap<Uuid, RunRecord>) {
    while runs.len() >= MAX_IN_MEMORY_RUNS {
        let Some((id, _created_at)) = runs
            .iter()
            .filter(|(_, record)| is_terminal_status(&record.status))
            .min_by_key(|(_, record)| record.created_at)
            .map(|(id, record)| (*id, record.created_at))
        else {
            break;
        };
        runs.remove(&id);
    }
}

fn fallback_state(
    run_id: Uuid,
    task: String,
    workspace: String,
    status: secondego_core::TerminalStatus,
    reason: String,
) -> ExecutionState {
    let mut state = ExecutionState::new(task, workspace);
    state.run_id = run_id;
    state.status = status;
    state.termination_reason = Some(reason);
    state
}

fn persist_snapshot(
    store: &RunStore,
    state: &ExecutionState,
    events: &[EngineEvent],
    metadata: &RunMetadata,
    result: Option<&serde_json::Value>,
    error: Option<&str>,
) {
    if let Ok(store) = store.lock() {
        let _ = store.save_snapshot(state, events, metadata, result, error);
    }
}

fn list_runs(stream: &mut TcpStream, path: &str, store: &RunStore) -> std::io::Result<()> {
    let limit = path
        .split_once('?')
        .and_then(|(_, query)| {
            query
                .split('&')
                .find_map(|item| item.strip_prefix("limit="))
                .and_then(|value| value.parse::<u32>().ok())
        })
        .unwrap_or(20)
        .clamp(1, 50);
    let summaries = match store.lock().ok().and_then(|store| store.list(limit).ok()) {
        Some(summaries) => summaries,
        None => return respond(stream, 500, serde_json::json!({"error":"run history unavailable"})),
    };
    let runs = summaries
        .into_iter()
        .map(|summary| {
            serde_json::json!({
                "request_id": summary.run_id,
                "status": summary.status,
                "profile_id": summary.metadata.profile_id,
                "workspace": summary.metadata.workspace_id,
                "provider": summary.metadata.provider,
                "model": summary.metadata.model,
                "engine_version": summary.metadata.engine_version,
                "created_at": summary.created_at,
                "updated_at": summary.updated_at,
            })
        })
        .collect::<Vec<_>>();
    respond(
        stream,
        200,
        serde_json::json!({"api_version":API_VERSION,"runs":runs,"limit":limit}),
    )
}

fn get_run(
    stream: &mut TcpStream,
    path: &str,
    runs: &Arc<Mutex<HashMap<Uuid, RunRecord>>>,
    store: &RunStore,
) -> std::io::Result<()> {
    let route_path = path.split('?').next().unwrap_or(path);
    let parts: Vec<_> = route_path.trim_start_matches('/').split('/').collect();
    let Ok(id) = parts.get(2).unwrap_or(&"").parse::<Uuid>() else {
        return respond(stream, 400, serde_json::json!({"error":"invalid run id"}));
    };
    let offset = path
        .split_once('?')
        .and_then(|(_, query)| {
            query
                .split('&')
                .find_map(|item| item.strip_prefix("offset="))
                .and_then(|value| value.parse::<usize>().ok())
        })
        .unwrap_or(0);
    if let Some(record) = runs.lock().unwrap().get(&id).cloned() {
        let (events, next_offset, has_more) = bounded_event_page(&record.events, offset);
        return respond(
            stream,
            200,
            serde_json::json!({"api_version":API_VERSION,"request_id":id,"repository":record.repository,"issue":record.issue,"profile_id":record.profile_id,"provider":record.provider,"model":record.model,"status":record.status,"events":events,"event_offset":offset.min(record.events.len()),"next_offset":next_offset,"event_count":record.events.len(),"has_more":has_more,"result":record.report,"error":record.error,"voice":record.voice.snapshot(id)}),
        );
    }
    let stored = store
        .lock()
        .ok()
        .and_then(|store| store.load(&id.to_string()).ok())
        .flatten();
    let Some(stored) = stored else {
        return respond(stream, 404, serde_json::json!({"error":"run not found"}));
    };
    let status =
        serde_json::to_value(stored.state.status).unwrap_or_else(|_| serde_json::json!("FAILED"));
    let repository = stored
        .metadata
        .workspace_id
        .clone()
        .unwrap_or_else(|| stored.state.workspace.clone());
    let (events, next_offset, has_more) = bounded_event_page(&stored.events, offset);
    let event_count = stored.events.len();
    respond(
        stream,
        200,
        serde_json::json!({
            "api_version": API_VERSION,
            "request_id": id,
            "repository": repository,
            "issue": stored.state.task,
            "profile_id": stored.metadata.profile_id,
            "provider": stored.metadata.provider,
            "model": stored.metadata.model,
            "status": status,
            "events": events,
            "event_offset": offset.min(event_count),
            "next_offset": next_offset,
            "event_count": event_count,
            "has_more": has_more,
            "result": stored.result,
            "error": stored.error,
            "voice": serde_json::Value::Null,
        }),
    )
}

fn bounded_event_page(
    events: &[EngineEvent],
    offset: usize,
) -> (Vec<EngineEvent>, usize, bool) {
    let start = offset.min(events.len());
    let end = start
        .saturating_add(MAX_EVENTS_PER_RESPONSE)
        .min(events.len());
    (events[start..end].to_vec(), end, end < events.len())
}

fn cancel_run(
    stream: &mut TcpStream,
    path: &str,
    runs: &Arc<Mutex<HashMap<Uuid, RunRecord>>>,
) -> std::io::Result<()> {
    let route_path = path.split('?').next().unwrap_or(path);
    let parts: Vec<_> = route_path.trim_start_matches('/').split('/').collect();
    let Ok(id) = parts.get(2).unwrap_or(&"").parse::<Uuid>() else {
        return respond(stream, 400, serde_json::json!({"error":"invalid run id"}));
    };
    let Some(record) = runs.lock().unwrap().get(&id).cloned() else {
        return respond(stream, 404, serde_json::json!({"error":"run not found"}));
    };
    if matches!(
        record.status.as_str(),
        "COMPLETE" | "FAILED" | "CANCELLED" | "BLOCKED"
    ) {
        return respond(
            stream,
            409,
            serde_json::json!({"error":"run is already terminal", "status":record.status}),
        );
    }
    record.cancellation.cancel();
    if let Ok(mut all_runs) = runs.lock() {
        if let Some(current) = all_runs.get_mut(&id) {
            current.status = "CANCEL_REQUESTED".into();
        }
    }
    respond(
        stream,
        202,
        serde_json::json!({"request_id":id,"status":"CANCEL_REQUESTED"}),
    )
}

#[derive(Default, serde::Deserialize)]
struct StartRequest {
    repository: String,
    issue: String,
    profile_id: Option<String>,
    model: Option<String>,
    mode: Option<String>,
    lenses: Option<Vec<String>>,
    max_findings: Option<usize>,
}

#[derive(Default, serde::Deserialize)]
struct ProviderTestRequest {
    profile_id: Option<String>,
    provider: Option<String>,
    model: Option<String>,
}

#[derive(Default, serde::Deserialize)]
struct ProviderModelsRequest {
    profile_id: Option<String>,
    provider: Option<String>,
}

#[derive(Default, serde::Deserialize)]
struct ProviderSessionRequest {
    profile_id: String,
    provider: String,
    model: Option<String>,
    api_key: String,
}

fn session_snapshot(credentials: &CredentialSession) -> Option<ProviderSession> {
    credentials.lock().ok().and_then(|session| session.clone())
}

fn configured_provider_from_session(
    session: Option<&ProviderSession>,
) -> Result<ConfiguredProvider, secondego_model::ProviderError> {
    match session {
        Some(session) => ConfiguredProvider::from_provider_key(
            session.provider,
            session.model.clone(),
            session.api_key.clone(),
        ),
        None => ConfiguredProvider::from_environment(),
    }
}

fn provider_kind_from_session(
    session: Option<&ProviderSession>,
) -> Result<ProviderKind, secondego_model::ProviderError> {
    session
        .map(|session| session.provider)
        .map(Ok)
        .unwrap_or_else(ProviderKind::from_environment)
}

fn valid_profile_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 192
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-' | ':')
        })
}

fn validate_start_request(request: &StartRequest) -> Result<(), &'static str> {
    let is_discovery = request.mode.as_deref() == Some("discover");
    if request.repository.trim().is_empty() || (!is_discovery && request.issue.trim().is_empty()) {
        return Err("repository is required and task issue must be non-empty");
    }
    if request.issue.chars().count() > 12_000 || request.repository.chars().count() > 4_096 {
        return Err("request exceeds field limits");
    }
    if request.model.as_deref().unwrap_or_default().chars().count() > 256 {
        return Err("model identifier exceeds field limits");
    }
    if request.mode.as_deref().unwrap_or("task") != "task"
        && request.mode.as_deref() != Some("discover")
    {
        return Err("mode must be task or discover");
    }
    if request
        .lenses
        .as_ref()
        .is_some_and(|lenses| lenses.len() > 8)
    {
        return Err("too many discovery lenses");
    }
    if request.max_findings.unwrap_or(20) > 40 {
        return Err("max_findings must be 40 or less");
    }
    Ok(())
}

fn result_payload(report: &RunReport) -> serde_json::Value {
    const MAX_DIFF_BYTES: usize = 512 * 1024;
    let patch = report.patch.as_deref().unwrap_or_default();
    let diff_truncated = patch.len() > MAX_DIFF_BYTES;
    let bounded_patch = if diff_truncated {
        let mut end = MAX_DIFF_BYTES;
        while end > 0 && !patch.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}\n\n[diff truncated by gateway]", &patch[..end])
    } else {
        patch.to_owned()
    };
    let changed_paths = report.changed_paths.clone();
    let mut normalized_files = normalized_diff(&bounded_patch);
    if normalized_files.is_empty() {
        normalized_files = changed_paths
            .iter()
            .map(|path| {
                serde_json::json!({
                    "path": path,
                    "status": "modified",
                    "additions": 0,
                    "deletions": 0,
                    "hunks": [],
                })
            })
            .collect();
    }
    serde_json::json!({
        "run_id": report.state.run_id,
        "status": serde_json::to_value(report.state.status).unwrap_or_else(|_| serde_json::json!("FAILED")),
        "phase": serde_json::to_value(report.state.phase).unwrap_or_else(|_| serde_json::json!("VERIFY")),
        "termination_reason": report.state.termination_reason,
        "changed_paths": changed_paths,
        "diff_transferred": report.diff_transferred,
        "diff": {
            "format": "git-unified-v1",
            "patch": bounded_patch,
            "truncated": diff_truncated,
            "files": normalized_files,
        },
        "resource_usage": report.resource_usage,
        "verification": report.verification,
        "evidence": report.evidence,
    })
}

#[derive(Default)]
struct NormalizedDiffFile {
    path: String,
    status: &'static str,
    additions: usize,
    deletions: usize,
    hunks: Vec<serde_json::Value>,
}

fn normalized_diff(patch: &str) -> Vec<serde_json::Value> {
    let mut files: Vec<NormalizedDiffFile> = Vec::new();
    let mut current: Option<usize> = None;
    let mut hunk: Option<usize> = None;
    for line in patch.lines() {
        if let Some(header) = line.strip_prefix("diff --git a/") {
            if files.len() >= 512 {
                break;
            }
            let path = header
                .split_once(" b/")
                .map(|(_, path)| path)
                .unwrap_or(header)
                .to_owned();
            files.push(NormalizedDiffFile {
                path,
                status: "modified",
                ..NormalizedDiffFile::default()
            });
            current = Some(files.len() - 1);
            hunk = None;
            continue;
        }
        let Some(file_index) = current else { continue };
        if line.starts_with("new file mode") {
            files[file_index].status = "added";
            continue;
        }
        if line.starts_with("deleted file mode") {
            files[file_index].status = "deleted";
            continue;
        }
        if line.starts_with("rename from ") || line.starts_with("rename to ") {
            files[file_index].status = "renamed";
            continue;
        }
        if line.starts_with("Binary files ") {
            files[file_index].status = "binary";
            continue;
        }
        if line.starts_with("@@ ") {
            if files[file_index].hunks.len() >= 512 {
                hunk = None;
                continue;
            }
            files[file_index]
                .hunks
                .push(serde_json::json!({"header": line, "lines": []}));
            hunk = Some(files[file_index].hunks.len() - 1);
            continue;
        }
        let Some(hunk_index) = hunk else { continue };
        let Some(kind) = line.as_bytes().first().copied() else {
            continue;
        };
        let (kind, content) = match kind {
            b'+' if !line.starts_with("+++") => ("add", &line[1..]),
            b'-' if !line.starts_with("---") => ("delete", &line[1..]),
            b' ' => ("context", &line[1..]),
            _ => continue,
        };
        if let Some(lines) = files[file_index].hunks[hunk_index]
            .get_mut("lines")
            .and_then(serde_json::Value::as_array_mut)
        {
            if lines.len() < 20_000 {
                lines.push(serde_json::json!({"kind": kind, "content": content}));
            }
        }
        match kind {
            "add" => files[file_index].additions += 1,
            "delete" => files[file_index].deletions += 1,
            _ => {}
        }
    }
    files
        .into_iter()
        .map(|file| {
            serde_json::json!({
                "path": file.path,
                "status": file.status,
                "additions": file.additions,
                "deletions": file.deletions,
                "hunks": file.hunks,
            })
        })
        .collect()
}

struct Request {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Request> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_end;
    loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "request ended",
            ));
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request too large",
            ));
        }
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            header_end = index + 4;
            break;
        }
    }
    let header_text = String::from_utf8_lossy(&bytes[..header_end]);
    let mut lines = header_text.split("\r\n");
    let mut request_line = lines.next().unwrap_or_default().split_whitespace();
    let method = request_line.next().unwrap_or_default().to_owned();
    let path = request_line.next().unwrap_or_default().to_owned();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.to_lowercase(), value.trim().to_owned()))
        .collect::<HashMap<_, _>>();
    let body_length = match headers.get("content-length") {
        Some(value) => value.parse::<usize>().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid content length")
        })?,
        None => 0,
    };
    if header_end + body_length > MAX_REQUEST_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "request too large",
        ));
    }
    while bytes.len() < header_end + body_length {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    if bytes.len() < header_end + body_length {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "request body ended before content length",
        ));
    }
    Ok(Request {
        method,
        path,
        headers,
        body: bytes
            [header_end..header_end + body_length.min(bytes.len().saturating_sub(header_end))]
            .to_vec(),
    })
}

fn respond(stream: &mut TcpStream, status: u16, body: serde_json::Value) -> std::io::Result<()> {
    let data = if status == 204 {
        Vec::new()
    } else {
        serde_json::to_vec(&body)
            .unwrap_or_else(|_| b"{\"error\":\"serialization failed\"}".to_vec())
    };
    respond_bytes(stream, status, "application/json", &data)
}

fn respond_bytes(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    data: &[u8],
) -> std::io::Result<()> {
    let reason = match status {
        204 => "No Content",
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, X-SecondEgo-Token\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        data.len(),
    )?;
    stream.write_all(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use secondego_core::ExecutionState;
    use secondego_verification::{FailureClass, VerificationResult};

    #[test]
    fn gateway_rejects_empty_and_oversized_start_requests() {
        assert!(
            validate_start_request(&StartRequest {
                repository: "".into(),
                issue: "task".into(),
                model: None,
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate_start_request(&StartRequest {
                repository: "/tmp/repo".into(),
                issue: "x".repeat(12_001),
                model: None,
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate_start_request(&StartRequest {
                repository: "/tmp/repo".into(),
                issue: "task".into(),
                model: Some("x".repeat(257)),
                ..Default::default()
            })
            .is_err()
        );
    }

    #[test]
    fn gateway_accepts_the_renderer_request_shape() {
        assert!(
            validate_start_request(&StartRequest {
                repository: "/tmp/repo".into(),
                issue: "fix pagination".into(),
                model: Some("deepseek-flash".into()),
                ..Default::default()
            })
            .is_ok()
        );
        assert!(
            validate_start_request(&StartRequest {
                repository: "/tmp/repo".into(),
                issue: String::new(),
                mode: Some("discover".into()),
                lenses: Some(vec!["error".into(), "test".into()]),
                max_findings: Some(10),
                ..Default::default()
            })
            .is_ok()
        );
    }

    #[test]
    fn result_payload_preserves_the_renderer_contract() {
        let mut state = ExecutionState::new("task", "/tmp/repo");
        state.status = secondego_core::TerminalStatus::Complete;
        state.termination_reason = Some("verified".into());
        let report = RunReport {
            state,
            events: Vec::new(),
            verification: VerificationResult {
                passed: true,
                commands: vec!["pytest -q".into()],
                passed_tests: 1,
                failed_tests: 0,
                failure_class: FailureClass::None,
                failure_summary: None,
                failure_record: None,
                evidence: Vec::new(),
            },
            evidence: Vec::new(),
            resource_usage: std::collections::BTreeMap::new(),
            verification_passed: true,
            changed_paths: vec!["src/example.py".into()],
            diff_transferred: true,
            patch: None,
            tool_results: Vec::new(),
            index_files: 1,
            index_symbols: 1,
            index_parser_failures: 0,
        };
        let payload = result_payload(&report);
        assert!(payload.get("run_id").is_some());
        assert!(
            payload
                .get("verification")
                .and_then(|value| value.get("passed"))
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
        );
        assert!(payload.get("changed_paths").is_some());
        assert_eq!(payload["diff_transferred"], true);
        assert!(payload.get("evidence").is_some());
        assert_eq!(payload["diff"]["format"], "git-unified-v1");
        assert_eq!(payload["diff"]["files"][0]["path"], "src/example.py");
    }

    #[test]
    fn normalized_diff_preserves_file_status_counts_and_hunks() {
        let files = normalized_diff(
            "diff --git a/src/example.py b/src/example.py\n--- a/src/example.py\n+++ b/src/example.py\n@@ -1 +1,2 @@\n-old()\n+new()\n+verified()\n",
        );
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["status"], "modified");
        assert_eq!(files[0]["additions"], 2);
        assert_eq!(files[0]["deletions"], 1);
        assert_eq!(files[0]["hunks"][0]["lines"][0]["kind"], "delete");
        assert_eq!(files[0]["hunks"][0]["lines"][1]["content"], "new()");
    }

    #[test]
    fn event_pages_are_bounded_and_resume_from_the_next_cursor() {
        let state = ExecutionState::new("task", "/tmp/repo");
        let events = (0..(MAX_EVENTS_PER_RESPONSE + 2))
            .map(|index| {
                let mut event = secondego_core::EngineEvent::new(&state, "activity.started");
                event.payload = serde_json::json!({"index": index});
                event
            })
            .collect::<Vec<_>>();

        let (first, next, more) = bounded_event_page(&events, 0);
        assert_eq!(first.len(), MAX_EVENTS_PER_RESPONSE);
        assert_eq!(next, MAX_EVENTS_PER_RESPONSE);
        assert!(more);
        assert_eq!(first[0].payload["index"], 0);

        let (last, next, more) = bounded_event_page(&events, next);
        assert_eq!(last.len(), 2);
        assert_eq!(next, events.len());
        assert!(!more);
        assert_eq!(last[1].payload["index"], MAX_EVENTS_PER_RESPONSE + 1);

        let (empty, next, more) = bounded_event_page(&events, usize::MAX);
        assert!(empty.is_empty());
        assert_eq!(next, events.len());
        assert!(!more);
    }

    #[test]
    fn configured_provider_can_be_frozen_for_an_enqueued_run() {
        let first = ProviderSession {
            profile_id: "openai:work".into(),
            provider: ProviderKind::OpenAI,
            model: "gpt-5-mini".into(),
            api_key: "first-secret".into(),
        };
        let provider = configured_provider_from_session(Some(&first)).unwrap();
        match provider {
            ConfiguredProvider::OpenAI(provider) => {
                assert_eq!(provider.model, "gpt-5-mini");
                assert_eq!(provider.api_key.as_deref(), Some("first-secret"));
            }
            _ => panic!("expected the frozen OpenAI session"),
        }

        let second = ProviderSession {
            profile_id: "anthropic:work".into(),
            provider: ProviderKind::Anthropic,
            model: "claude-sonnet-4-6".into(),
            api_key: "second-secret".into(),
        };
        let later_provider = configured_provider_from_session(Some(&second)).unwrap();
        assert!(matches!(later_provider, ConfiguredProvider::Anthropic(_)));
    }
}
