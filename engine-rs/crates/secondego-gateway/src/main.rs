use std::collections::HashMap;
use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use secondego_core::EngineEvent;
use secondego_model::GeminiProvider;
use secondego_runtime::{RunReport, RustEngine};
use uuid::Uuid;

const MAX_REQUEST_BYTES: usize = 64 * 1024;

#[derive(Clone)]
struct RunRecord {
    report: Option<RunReport>,
    error: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = env::var("SECONDEGO_UI_TOKEN").unwrap_or_else(|_| "local-development-token".into());
    let listener = TcpListener::bind(("127.0.0.1", 8787))?;
    let runs: Arc<Mutex<HashMap<Uuid, RunRecord>>> = Arc::new(Mutex::new(HashMap::new()));
    eprintln!("SecondEgo Rust gateway listening on http://127.0.0.1:8787");
    for stream in listener.incoming().flatten() {
        let token = token.clone();
        let runs = runs.clone();
        thread::spawn(move || {
            let _ = handle(stream, &token, &runs);
        });
    }
    Ok(())
}

fn handle(
    mut stream: TcpStream,
    token: &str,
    runs: &Arc<Mutex<HashMap<Uuid, RunRecord>>>,
) -> std::io::Result<()> {
    let request = read_request(&mut stream)?;
    if request.headers.get("x-secondego-token").map(String::as_str) != Some(token) {
        return respond(
            &mut stream,
            401,
            serde_json::json!({"error":"unauthorized"}),
        );
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/health") => respond(
            &mut stream,
            200,
            serde_json::json!({"ok":true,"engine":"rust"}),
        ),
        ("POST", "/runs") => create_run(&mut stream, request.body, runs),
        ("GET", path) if path.starts_with("/runs/") => get_run(&mut stream, path, runs),
        _ => respond(&mut stream, 404, serde_json::json!({"error":"not found"})),
    }
}

fn create_run(
    stream: &mut TcpStream,
    body: Vec<u8>,
    runs: &Arc<Mutex<HashMap<Uuid, RunRecord>>>,
) -> std::io::Result<()> {
    let request: StartRequest = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return respond(stream, 400, serde_json::json!({"error":"invalid request"})),
    };
    if request.task.trim().is_empty() || request.workspace.trim().is_empty() {
        return respond(
            stream,
            400,
            serde_json::json!({"error":"task and workspace are required"}),
        );
    }
    let id = Uuid::new_v4();
    runs.lock().unwrap().insert(
        id,
        RunRecord {
            report: None,
            error: None,
        },
    );
    let runs_for_thread = runs.clone();
    thread::spawn(move || {
        let mut engine = RustEngine::new(GeminiProvider::default());
        let result = engine.run(request.task, request.workspace);
        let record = match result {
            Ok(report) => RunRecord {
                report: Some(report),
                error: None,
            },
            Err(error) => RunRecord {
                report: None,
                error: Some(error.to_string()),
            },
        };
        if let Ok(mut all_runs) = runs_for_thread.lock() {
            all_runs.insert(id, record);
        }
    });
    respond(
        stream,
        202,
        serde_json::json!({"run_id":id,"status":"running"}),
    )
}

fn get_run(
    stream: &mut TcpStream,
    path: &str,
    runs: &Arc<Mutex<HashMap<Uuid, RunRecord>>>,
) -> std::io::Result<()> {
    let parts: Vec<_> = path.trim_start_matches('/').split('/').collect();
    let Ok(id) = parts.get(1).unwrap_or(&"").parse::<Uuid>() else {
        return respond(stream, 400, serde_json::json!({"error":"invalid run id"}));
    };
    let Some(record) = runs.lock().unwrap().get(&id).cloned() else {
        return respond(stream, 404, serde_json::json!({"error":"run not found"}));
    };
    if parts.get(2).copied() == Some("events") {
        let events: Vec<EngineEvent> = record
            .report
            .as_ref()
            .map(|report| report.events.clone())
            .unwrap_or_default();
        return respond(
            stream,
            200,
            serde_json::json!({"run_id":id,"events":events}),
        );
    }
    if let Some(error) = record.error {
        return respond(
            stream,
            200,
            serde_json::json!({"run_id":id,"status":"failed","error":error}),
        );
    }
    if let Some(report) = record.report {
        return respond(
            stream,
            200,
            serde_json::json!({"run_id":id,"status":"complete","report":report}),
        );
    }
    respond(
        stream,
        200,
        serde_json::json!({"run_id":id,"status":"running"}),
    )
}

#[derive(serde::Deserialize)]
struct StartRequest {
    task: String,
    workspace: String,
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
    let data = serde_json::to_vec(&body)
        .unwrap_or_else(|_| b"{\"error\":\"serialization failed\"}".to_vec());
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        data.len()
    )?;
    stream.write_all(&data)
}
