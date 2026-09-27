use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use secondego_core::EngineEvent;
use secondego_model::{ConfiguredProvider, configured_model};
use secondego_runtime::discovery::{DiscoveryLens, DiscoveryRequest, discover};
use secondego_runtime::{
    CancellationToken, RunReport, RuntimeError, RustEngine, collect_garbage,
    resolve_repository_with_cancellation, validate_repository_source,
    voice::{VoiceService, VoiceStatusHandle},
};
use uuid::Uuid;

const MAX_REQUEST_BYTES: usize = 64 * 1024;

#[derive(Clone)]
struct RunRecord {
    repository: String,
    issue: String,
    model: String,
    status: String,
    events: Vec<EngineEvent>,
    report: Option<serde_json::Value>,
    error: Option<String>,
    cancellation: CancellationToken,
    voice: VoiceStatusHandle,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gc = collect_garbage();
    if gc.deleted > 0 {
        eprintln!(
            "SecondEgo garbage collector: reclaimed {} stale temp directories",
            gc.deleted
        );
    }
    let token = env::var("SECONDEGO_UI_TOKEN").unwrap_or_else(|_| "local-development-token".into());
    let port = env::var("SECONDEGO_GATEWAY_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8787);
    let ui_root = env::var_os("SECONDEGO_UI_DIST")
        .map(PathBuf::from)
        .filter(|path| path.is_dir());
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let runs: Arc<Mutex<HashMap<Uuid, RunRecord>>> = Arc::new(Mutex::new(HashMap::new()));
    // Voice is presentation-only, but its background worker must be shared by
    // the long-lived gateway rather than leaked once for every submitted run.
    let voice = Arc::new(VoiceService::from_env());
    eprintln!("SecondEgo Rust gateway listening on http://127.0.0.1:{port}");
    for stream in listener.incoming().flatten() {
        let token = token.clone();
        let runs = runs.clone();
        let ui_root = ui_root.clone();
        let voice = voice.clone();
        thread::spawn(move || {
            let _ = handle(stream, &token, &runs, ui_root.as_deref(), &voice);
        });
    }
    Ok(())
}

fn handle(
    mut stream: TcpStream,
    token: &str,
    runs: &Arc<Mutex<HashMap<Uuid, RunRecord>>>,
    ui_root: Option<&Path>,
    voice: &Arc<VoiceService>,
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
            serde_json::json!({"ok":true,"api_version":1,"engine":"rust"}),
        ),
        ("POST", "/api/runs") => create_run(&mut stream, request.body, runs, voice),
        ("POST", path) if path.starts_with("/api/runs/") && path.ends_with("/cancel") => {
            cancel_run(&mut stream, path, runs)
        }
        ("GET", path) if path.starts_with("/api/runs/") => get_run(&mut stream, path, runs),
        _ => respond(&mut stream, 404, serde_json::json!({"error":"not found"})),
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
    let model = match request.model.filter(|model| !model.trim().is_empty()) {
        Some(model) => model,
        None if is_discovery => "deterministic-discovery".into(),
        None => match configured_model() {
            Ok(model) => model,
            Err(error) => {
                return respond(stream, 500, serde_json::json!({"error": error.to_string()}));
            }
        },
    };
    let id = Uuid::new_v4();
    let cancellation = CancellationToken::default();
    let voice_status = voice.status_handle();
    runs.lock().unwrap().insert(
        id,
        RunRecord {
            repository: request.repository.clone(),
            issue: request.issue.clone(),
            model: model.clone(),
            status: "QUEUED".into(),
            events: Vec::new(),
            report: None,
            error: None,
            cancellation: cancellation.clone(),
            voice: voice_status.clone(),
        },
    );
    let runs_for_thread = runs.clone();
    let record_repository = request.repository.clone();
    let record_issue = request.issue.clone();
    let record_model = model.clone();
    let record_mode = request.mode.clone().unwrap_or_else(|| "task".into());
    let discovery_lenses = request.lenses.clone();
    let discovery_max_findings = request.max_findings.unwrap_or(20);
    let event_runs = runs.clone();
    let run_cancellation = cancellation.clone();
    let run_voice = voice.clone();
    thread::spawn(move || {
        let resolved =
            match resolve_repository_with_cancellation(&record_repository, Some(&run_cancellation))
            {
                Ok(value) => value,
                Err(error) => {
                    if let Ok(mut all_runs) = runs_for_thread.lock() {
                        if let Some(record) = all_runs.get_mut(&id) {
                            let cancelled = matches!(error, RuntimeError::Cancelled);
                            record.status = if cancelled { "CANCELLED" } else { "FAILED" }.into();
                            record.error = (!cancelled).then(|| error.to_string());
                        }
                    }
                    return;
                }
            };
        if record_mode == "discover" {
            let lenses = discovery_lenses
                .unwrap_or_default()
                .into_iter()
                .filter_map(|value| DiscoveryLens::parse(&value))
                .collect();
            let result = discover(
                &resolved.root,
                DiscoveryRequest {
                    lenses,
                    max_findings: discovery_max_findings,
                },
            );
            let record = match result {
                Ok(report) => RunRecord {
                    repository: resolved.root.to_string_lossy().into_owned(),
                    issue: record_issue,
                    model: record_model,
                    status: "COMPLETE".into(),
                    events: report.events.clone(),
                    report: serde_json::to_value(report).ok(),
                    error: None,
                    cancellation: run_cancellation.clone(),
                    voice: voice_status.clone(),
                },
                Err(error) => RunRecord {
                    repository: record_repository,
                    issue: record_issue,
                    model: record_model,
                    status: "FAILED".into(),
                    events: Vec::new(),
                    report: None,
                    error: Some(error),
                    cancellation: run_cancellation.clone(),
                    voice: voice_status.clone(),
                },
            };
            if let Ok(mut all_runs) = runs_for_thread.lock() {
                all_runs.insert(id, record);
            }
            return;
        }
        let provider = match ConfiguredProvider::from_environment()
            .and_then(|provider| provider.with_model(model))
        {
            Ok(provider) => provider,
            Err(error) => {
                if let Ok(mut all_runs) = runs_for_thread.lock() {
                    if let Some(record) = all_runs.get_mut(&id) {
                        record.status = "FAILED".into();
                        record.error = Some(error.to_string());
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
        let result = engine.run(request.issue, resolved.root.clone());
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
                RunRecord {
                    repository: resolved.root.to_string_lossy().into_owned(),
                    issue: record_issue.clone(),
                    model: record_model.clone(),
                    status: if verified { "COMPLETE" } else { "FAILED" }.into(),
                    events,
                    report: Some(result_payload(&report)),
                    error,
                    cancellation: run_cancellation.clone(),
                    voice: voice_status.clone(),
                }
            }
            Err(error) => {
                let cancelled = matches!(error, RuntimeError::Cancelled);
                RunRecord {
                    repository: record_repository,
                    issue: record_issue,
                    model: record_model,
                    status: if cancelled { "CANCELLED" } else { "FAILED" }.into(),
                    events,
                    report: None,
                    error: (!cancelled).then(|| error.to_string()),
                    cancellation: run_cancellation.clone(),
                    voice: voice_status.clone(),
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
        serde_json::json!({"request_id":id,"status":"QUEUED"}),
    )
}

fn get_run(
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
    let offset = path
        .split_once('?')
        .and_then(|(_, query)| {
            query
                .split('&')
                .find_map(|item| item.strip_prefix("offset="))
                .and_then(|value| value.parse::<usize>().ok())
        })
        .unwrap_or(0);
    let events: Vec<EngineEvent> = record.events.iter().skip(offset).cloned().collect();
    let result = record.report.clone();
    respond(
        stream,
        200,
        serde_json::json!({"request_id":id,"repository":record.repository,"issue":record.issue,"model":record.model,"status":record.status,"events":events,"result":result,"error":record.error,"voice":record.voice.snapshot(id)}),
    )
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
    model: Option<String>,
    mode: Option<String>,
    lenses: Option<Vec<String>>,
    max_findings: Option<usize>,
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
    serde_json::json!({
        "run_id": report.state.run_id,
        "status": serde_json::to_value(report.state.status).unwrap_or_else(|_| serde_json::json!("FAILED")),
        "phase": serde_json::to_value(report.state.phase).unwrap_or_else(|_| serde_json::json!("VERIFY")),
        "termination_reason": report.state.termination_reason,
        "changed_paths": report.changed_paths,
        "diff_transferred": report.diff_transferred,
        "resource_usage": report.resource_usage,
        "verification": report.verification,
        "evidence": report.evidence,
    })
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
    let body_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
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
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, X-SecondEgo-Token\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
    }
}
