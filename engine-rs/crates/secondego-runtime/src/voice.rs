//! Optional, non-critical voice presentation for structured engine events.
//!
//! This module deliberately has no access to repository state, tools, model
//! prompts, or agent decisions. `VoicePolicy` turns a small allowlist of
//! engine events into local narration strings; `GeminiTtsProvider` only
//! converts one of those strings into audio bytes.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use reqwest::blocking::Client;
use secondego_core::EngineEvent;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const DEFAULT_TTS_MODEL: &str = "gemini-3.8-flash-lite-tts";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(8);
const DEFAULT_QUEUE_SIZE: usize = 8;
const MAX_NARRATION_CHARS: usize = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum VoiceState {
    Disabled,
    Idle,
    Generating,
    Speaking,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VoiceSnapshot {
    pub enabled: bool,
    pub provider: String,
    pub state: VoiceState,
    pub queue_length: usize,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VoiceConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout: Duration,
    pub max_queue_size: usize,
}

impl VoiceConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: env_flag("VOICE_ENABLED"),
            api_key: non_empty_env("GEMINI_API_KEY"),
            model: non_empty_env("GEMINI_TTS_MODEL")
                .unwrap_or_else(|| DEFAULT_TTS_MODEL.to_owned()),
            timeout: duration_env("VOICE_TIMEOUT", DEFAULT_TIMEOUT),
            max_queue_size: usize_env("VOICE_MAX_QUEUE_SIZE", DEFAULT_QUEUE_SIZE),
        }
    }
}

pub trait VoiceProvider: Send + Sync {
    /// Convert the supplied narration text into audio. The provider never
    /// receives an EngineEvent or any other agent context.
    fn synthesize(&self, text: &str) -> Result<AudioClip, VoiceError>;
}

pub trait AudioPlayer: Send + Sync {
    fn play(&self, audio: &AudioClip, timeout: Duration) -> Result<(), VoiceError>;
}

#[derive(Debug, Clone)]
pub struct AudioClip {
    pub bytes: Vec<u8>,
    pub mime_type: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoicePriority {
    High,
    Normal,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Narration {
    pub key: &'static str,
    pub text: &'static str,
    pub priority: VoicePriority,
}

/// Deterministic local event-to-speech mapping. Dynamic event payloads are
/// used only to select a fixed template; no payload text is sent to Gemini.
pub struct VoicePolicy;

impl VoicePolicy {
    pub fn narration(event: &EngineEvent) -> Option<Narration> {
        let operation = event
            .payload
            .get("operation")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let message = event
            .payload
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default();

        match event.event_type.as_str() {
            "state.changed" => match event.phase {
                secondego_core::Phase::Understand => Some(Narration {
                    key: "task_started",
                    text: "I'm starting the task and mapping the repository.",
                    priority: VoicePriority::Normal,
                }),
                secondego_core::Phase::Plan => Some(Narration {
                    key: "plan_ready",
                    text: "I found the relevant subsystem and I'm preparing the smallest change.",
                    priority: VoicePriority::Normal,
                }),
                secondego_core::Phase::Execute => Some(Narration {
                    key: "implementation_started",
                    text: "The implementation is underway.",
                    priority: VoicePriority::Normal,
                }),
                secondego_core::Phase::Verify => Some(Narration {
                    key: "verification_started",
                    text: "The implementation is ready for verification.",
                    priority: VoicePriority::Normal,
                }),
                secondego_core::Phase::Recover | secondego_core::Phase::Diagnose => {
                    Some(Narration {
                        key: "recovery_started",
                        text: "The previous approach failed. I'm tracing the cause before changing the code.",
                        priority: VoicePriority::High,
                    })
                }
                _ => None,
            },
            "activity.completed" if operation == "repository.index" => Some(Narration {
                key: "exploration_complete",
                text: "I've mapped the relevant parts of the repository.",
                priority: VoicePriority::Low,
            }),
            "activity.completed"
                if operation == "verification.run" || operation == "verification.recheck" =>
            {
                if message.contains("passed") {
                    Some(Narration {
                        key: "tests_passed",
                        text: "The targeted tests are passing.",
                        priority: VoicePriority::Normal,
                    })
                } else {
                    Some(Narration {
                        key: "test_failure",
                        text: "The tests exposed a regression. I'm tracing the failure.",
                        priority: VoicePriority::High,
                    })
                }
            }
            "activity.completed" if operation == "recovery.apply" => Some(Narration {
                key: "implementation_revised",
                text: "I've revised the implementation and I'm testing it again.",
                priority: VoicePriority::Normal,
            }),
            "activity.failed" if operation.starts_with("verification") => Some(Narration {
                key: "test_failure",
                text: "The tests exposed a regression. I'm tracing the failure.",
                priority: VoicePriority::High,
            }),
            "activity.retrying" => Some(Narration {
                key: "recovery_started",
                text: "The previous approach failed. I'm tracing the cause before changing the code.",
                priority: VoicePriority::High,
            }),
            "run.terminated" => match event.status {
                secondego_core::TerminalStatus::Complete => Some(Narration {
                    key: "task_completed",
                    text: "The implementation is complete and verified.",
                    priority: VoicePriority::High,
                }),
                secondego_core::TerminalStatus::Blocked
                | secondego_core::TerminalStatus::Failed => Some(Narration {
                    key: "task_blocked",
                    text: "The task is blocked and requires further investigation.",
                    priority: VoicePriority::High,
                }),
                _ => None,
            },
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceError {
    InvalidInput,
    MissingApiKey,
    InvalidModel,
    Http,
    Timeout,
    InvalidResponse,
    Audio,
    Playback,
}

impl std::fmt::Display for VoiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "invalid narration",
            Self::MissingApiKey => "missing Gemini API key",
            Self::InvalidModel => "invalid Gemini TTS model",
            Self::Http => "Gemini TTS request failed",
            Self::Timeout => "Gemini TTS request timed out",
            Self::InvalidResponse => "Gemini TTS returned malformed audio",
            Self::Audio => "audio output failed",
            Self::Playback => "audio playback failed",
        })
    }
}

impl std::error::Error for VoiceError {}

pub struct GeminiTtsProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl GeminiTtsProvider {
    pub fn new(api_key: String, model: String, timeout: Duration) -> Result<Self, VoiceError> {
        if api_key.trim().is_empty() {
            return Err(VoiceError::MissingApiKey);
        }
        if !valid_model_name(&model) {
            return Err(VoiceError::InvalidModel);
        }
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|_| VoiceError::Http)?;
        Ok(Self {
            client,
            api_key,
            model,
        })
    }

    pub fn request_body(text: &str) -> Result<Value, VoiceError> {
        validate_narration(text)?;
        Ok(json!({
            "contents": [{
                "role": "user",
                "parts": [{ "text": text }]
            }],
            "generationConfig": {
                "responseModalities": ["AUDIO"],
                "speechConfig": {
                    "voiceConfig": {
                        "prebuiltVoiceConfig": { "voiceName": "Kore" }
                    }
                }
            }
        }))
    }
}

impl VoiceProvider for GeminiTtsProvider {
    fn synthesize(&self, text: &str) -> Result<AudioClip, VoiceError> {
        let body = Self::request_body(text)?;
        let endpoint = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model
        );
        let response = self
            .client
            .post(endpoint)
            .header("x-goog-api-key", &self.api_key)
            .json(&body)
            .send()
            .map_err(|error| {
                if error.is_timeout() {
                    VoiceError::Timeout
                } else {
                    VoiceError::Http
                }
            })?;
        if !response.status().is_success() {
            return Err(
                if response.status() == reqwest::StatusCode::REQUEST_TIMEOUT {
                    VoiceError::Timeout
                } else {
                    VoiceError::Http
                },
            );
        }
        let response: TtsResponse = response.json().map_err(|_| VoiceError::InvalidResponse)?;
        let encoded = response
            .candidates
            .and_then(|candidates| candidates.into_iter().next())
            .and_then(|candidate| candidate.content)
            .and_then(|content| content.parts.into_iter().next())
            .and_then(|part| part.inline_data)
            .map(|data| data.data)
            .ok_or(VoiceError::InvalidResponse)?;
        let bytes = BASE64.decode(encoded).map_err(|_| VoiceError::Audio)?;
        if bytes.is_empty() {
            return Err(VoiceError::InvalidResponse);
        }
        Ok(AudioClip {
            bytes,
            mime_type: "audio/wav".into(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct TtsResponse {
    candidates: Option<Vec<TtsCandidate>>,
}

#[derive(Debug, Deserialize)]
struct TtsCandidate {
    content: Option<TtsContent>,
}

#[derive(Debug, Deserialize)]
struct TtsContent {
    parts: Vec<TtsPart>,
}

#[derive(Debug, Deserialize)]
struct TtsPart {
    #[serde(rename = "inlineData")]
    inline_data: Option<InlineData>,
}

#[derive(Debug, Deserialize)]
struct InlineData {
    data: String,
}

#[derive(Clone)]
pub struct NullVoiceProvider;

impl VoiceProvider for NullVoiceProvider {
    fn synthesize(&self, _text: &str) -> Result<AudioClip, VoiceError> {
        Err(VoiceError::InvalidInput)
    }
}

pub struct CommandAudioPlayer;

impl AudioPlayer for CommandAudioPlayer {
    fn play(&self, audio: &AudioClip, timeout: Duration) -> Result<(), VoiceError> {
        if audio.bytes.is_empty() {
            return Err(VoiceError::Audio);
        }
        let directory = env::temp_dir().join("secondego-voice");
        fs::create_dir_all(&directory).map_err(|_| VoiceError::Audio)?;
        let extension = if audio.mime_type == "audio/wav" {
            "wav"
        } else {
            "audio"
        };
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| VoiceError::Audio)?
            .as_nanos();
        let path = directory.join(format!(
            "voice-{}-{timestamp}.{extension}",
            std::process::id()
        ));
        fs::write(&path, &audio.bytes).map_err(|_| VoiceError::Audio)?;

        let result = self.play_file(&path, timeout);
        let _ = fs::remove_file(path);
        result
    }
}

impl CommandAudioPlayer {
    fn play_file(&self, path: &PathBuf, timeout: Duration) -> Result<(), VoiceError> {
        #[cfg(target_os = "macos")]
        let mut child = Command::new("afplay")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| VoiceError::Playback)?;

        #[cfg(target_os = "windows")]
        let mut child = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command"])
            .arg(format!(
                "(New-Object Media.SoundPlayer '{}').PlaySync()",
                path.display()
            ))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| VoiceError::Playback)?;

        #[cfg(all(unix, not(target_os = "macos")))]
        let mut child = spawn_unix_player(path)?;

        wait_for_child(&mut child, timeout)
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_unix_player(path: &PathBuf) -> Result<Child, VoiceError> {
    for program in ["ffplay", "aplay", "paplay"] {
        match Command::new(program)
            .args(if program == "ffplay" {
                vec!["-nodisp", "-autoexit", "-loglevel", "quiet"]
            } else {
                Vec::new()
            })
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => return Ok(child),
            Err(_) => continue,
        }
    }
    Err(VoiceError::Playback)
}

fn wait_for_child(child: &mut Child, timeout: Duration) -> Result<(), VoiceError> {
    let started = SystemTime::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => return Err(VoiceError::Playback),
            Ok(None) => {}
            Err(_) => return Err(VoiceError::Playback),
        }
        if started.elapsed().unwrap_or(timeout) >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(VoiceError::Timeout);
        }
        thread::sleep(Duration::from_millis(25));
    }
}

struct VoiceJob {
    run_id: Uuid,
    narration: Narration,
}

struct QueueState {
    jobs: VecDeque<VoiceJob>,
    closed: bool,
}

struct VoiceQueue {
    max_size: usize,
    state: Mutex<QueueState>,
    wake: Condvar,
}

impl VoiceQueue {
    fn new(max_size: usize) -> Self {
        Self {
            max_size: max_size.max(1),
            state: Mutex::new(QueueState {
                jobs: VecDeque::new(),
                closed: false,
            }),
            wake: Condvar::new(),
        }
    }

    fn push(&self, job: VoiceJob) -> bool {
        let mut state = self.state.lock().expect("voice queue mutex poisoned");
        if state.closed {
            return false;
        }
        if job.narration.priority == VoicePriority::High {
            state
                .jobs
                .retain(|queued| queued.narration.priority != VoicePriority::Low);
        }
        if state.jobs.len() >= self.max_size {
            if job.narration.priority == VoicePriority::High {
                if let Some(index) = state
                    .jobs
                    .iter()
                    .position(|queued| queued.narration.priority != VoicePriority::High)
                {
                    state.jobs.remove(index);
                } else {
                    state.jobs.pop_front();
                }
            } else {
                return false;
            }
        }
        state.jobs.push_back(job);
        self.wake.notify_one();
        true
    }

    fn pop(&self) -> Option<VoiceJob> {
        let mut state = self.state.lock().expect("voice queue mutex poisoned");
        loop {
            if let Some(job) = state.jobs.pop_front() {
                return Some(job);
            }
            if state.closed {
                return None;
            }
            state = self.wake.wait(state).expect("voice queue mutex poisoned");
        }
    }

    fn close(&self) {
        let mut state = self.state.lock().expect("voice queue mutex poisoned");
        state.closed = true;
        self.wake.notify_one();
    }

    fn len(&self) -> usize {
        self.state
            .lock()
            .expect("voice queue mutex poisoned")
            .jobs
            .len()
    }
}

#[derive(Default)]
struct PolicyState {
    seen: HashSet<(Uuid, &'static str)>,
}

#[derive(Clone)]
pub struct VoiceService {
    enabled: bool,
    available: bool,
    provider_name: String,
    provider: Arc<dyn VoiceProvider>,
    player: Arc<dyn AudioPlayer>,
    queue: Arc<VoiceQueue>,
    policy: Arc<Mutex<PolicyState>>,
    statuses: Arc<Mutex<HashMap<Uuid, (VoiceState, Option<String>)>>>,
    timeout: Duration,
}

#[derive(Clone)]
pub struct VoiceStatusHandle {
    enabled: bool,
    available: bool,
    provider_name: String,
    queue: Arc<VoiceQueue>,
    statuses: Arc<Mutex<HashMap<Uuid, (VoiceState, Option<String>)>>>,
}

impl VoiceStatusHandle {
    pub fn snapshot(&self, run_id: Uuid) -> VoiceSnapshot {
        let (state, last_error) = self
            .statuses
            .lock()
            .expect("voice status mutex poisoned")
            .get(&run_id)
            .cloned()
            .unwrap_or((
                if !self.enabled {
                    VoiceState::Disabled
                } else if !self.available {
                    VoiceState::Unavailable
                } else {
                    VoiceState::Idle
                },
                None,
            ));
        VoiceSnapshot {
            enabled: self.enabled,
            provider: self.provider_name.clone(),
            state,
            queue_length: self.queue.len(),
            last_error,
        }
    }
}

impl VoiceService {
    pub fn from_env() -> Self {
        let config = VoiceConfig::from_env();
        Self::from_config(config)
    }

    pub fn from_config(config: VoiceConfig) -> Self {
        let queue = Arc::new(VoiceQueue::new(config.max_queue_size));
        let statuses = Arc::new(Mutex::new(HashMap::new()));
        if !config.enabled {
            return Self {
                enabled: false,
                available: false,
                provider_name: "none".into(),
                provider: Arc::new(NullVoiceProvider),
                player: Arc::new(CommandAudioPlayer),
                queue,
                policy: Arc::new(Mutex::new(PolicyState::default())),
                statuses,
                timeout: config.timeout,
            };
        }
        let Some(api_key) = config.api_key else {
            return Self {
                enabled: true,
                available: false,
                provider_name: "gemini-tts".into(),
                provider: Arc::new(NullVoiceProvider),
                player: Arc::new(CommandAudioPlayer),
                queue,
                policy: Arc::new(Mutex::new(PolicyState::default())),
                statuses,
                timeout: config.timeout,
            };
        };
        let provider = match GeminiTtsProvider::new(api_key, config.model, config.timeout) {
            Ok(provider) => Arc::new(provider) as Arc<dyn VoiceProvider>,
            Err(_) => {
                return Self {
                    enabled: true,
                    available: false,
                    provider_name: "gemini-tts".into(),
                    provider: Arc::new(NullVoiceProvider),
                    player: Arc::new(CommandAudioPlayer),
                    queue,
                    policy: Arc::new(Mutex::new(PolicyState::default())),
                    statuses,
                    timeout: config.timeout,
                };
            }
        };
        let service = Self {
            enabled: true,
            available: true,
            provider_name: "gemini-tts".into(),
            provider,
            player: Arc::new(CommandAudioPlayer),
            queue,
            policy: Arc::new(Mutex::new(PolicyState::default())),
            statuses,
            timeout: config.timeout,
        };
        service.start_worker();
        service
    }

    #[cfg(test)]
    fn with_components(
        provider: Arc<dyn VoiceProvider>,
        player: Arc<dyn AudioPlayer>,
        max_queue_size: usize,
    ) -> Self {
        let service = Self {
            enabled: true,
            available: true,
            provider_name: "test".into(),
            provider,
            player,
            queue: Arc::new(VoiceQueue::new(max_queue_size)),
            policy: Arc::new(Mutex::new(PolicyState::default())),
            statuses: Arc::new(Mutex::new(HashMap::new())),
            timeout: Duration::from_secs(1),
        };
        service.start_worker();
        service
    }

    pub fn observe(&self, event: &EngineEvent) {
        if !self.enabled || !self.available {
            return;
        }
        let Some(narration) = VoicePolicy::narration(event) else {
            return;
        };
        let mut policy = self.policy.lock().expect("voice policy mutex poisoned");
        let key = (event.run_id, narration.key);
        if !policy.seen.insert(key) {
            return;
        }
        if policy.seen.len() > 256 {
            policy.seen.clear();
            policy.seen.insert(key);
        }
        drop(policy);
        self.queue.push(VoiceJob {
            run_id: event.run_id,
            narration,
        });
    }

    pub fn snapshot(&self, run_id: Uuid) -> VoiceSnapshot {
        self.status_handle().snapshot(run_id)
    }

    pub fn status_handle(&self) -> VoiceStatusHandle {
        VoiceStatusHandle {
            enabled: self.enabled,
            available: self.available,
            provider_name: self.provider_name.clone(),
            queue: self.queue.clone(),
            statuses: self.statuses.clone(),
        }
    }

    pub fn shutdown(&self) {
        self.queue.close();
    }

    fn start_worker(&self) {
        let queue = self.queue.clone();
        let provider = self.provider.clone();
        let player = self.player.clone();
        let statuses = self.statuses.clone();
        let timeout = self.timeout;
        thread::Builder::new()
            .name("secondego-voice".into())
            .spawn(move || {
                loop {
                    let Some(job) = queue.pop() else {
                        break;
                    };
                    set_status(&statuses, job.run_id, VoiceState::Generating, None);
                    match provider.synthesize(job.narration.text) {
                        Ok(audio) => {
                            set_status(&statuses, job.run_id, VoiceState::Speaking, None);
                            match player.play(&audio, timeout) {
                                Ok(()) => set_status(&statuses, job.run_id, VoiceState::Idle, None),
                                Err(error) => set_status(
                                    &statuses,
                                    job.run_id,
                                    VoiceState::Unavailable,
                                    Some(error.to_string()),
                                ),
                            }
                        }
                        Err(error) => set_status(
                            &statuses,
                            job.run_id,
                            VoiceState::Unavailable,
                            Some(error.to_string()),
                        ),
                    }
                }
            })
            .ok();
    }
}

fn set_status(
    statuses: &Mutex<HashMap<Uuid, (VoiceState, Option<String>)>>,
    run_id: Uuid,
    state: VoiceState,
    last_error: Option<String>,
) {
    statuses
        .lock()
        .expect("voice status mutex poisoned")
        .insert(run_id, (state, last_error));
}

fn validate_narration(text: &str) -> Result<(), VoiceError> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_NARRATION_CHARS {
        return Err(VoiceError::InvalidInput);
    }
    Ok(())
}

fn valid_model_name(model: &str) -> bool {
    !model.is_empty()
        && model.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
}

fn env_flag(name: &str) -> bool {
    matches!(
        env::var(name).ok().as_deref().map(str::trim),
        Some("1" | "true" | "TRUE" | "yes" | "on")
    )
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn duration_env(name: &str, default: Duration) -> Duration {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(|seconds| Duration::from_secs(seconds.clamp(1, 60)))
        .unwrap_or(default)
}

fn usize_env(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|size| size.clamp(1, 32))
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use secondego_core::{ExecutionState, Phase, TerminalStatus};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn event(event_type: &str, phase: Phase) -> EngineEvent {
        let state = ExecutionState::new("task", "/repo");
        let mut event = EngineEvent::new(&state, event_type);
        event.phase = phase;
        event
    }

    #[test]
    fn policy_only_emits_fixed_narration_for_meaningful_events() {
        let task = event("state.changed", Phase::Understand);
        let narration = VoicePolicy::narration(&task).unwrap();
        assert_eq!(narration.key, "task_started");
        assert!(narration.text.contains("mapping the repository"));
        assert!(VoicePolicy::narration(&event("activity.started", Phase::Explore)).is_none());
    }

    #[test]
    fn verification_failure_and_completion_are_distinct_fixed_templates() {
        let mut failed = event("activity.completed", Phase::Verify);
        failed.payload = json!({"operation":"verification.run","message":"Verification produced a failure record"});
        assert_eq!(VoicePolicy::narration(&failed).unwrap().key, "test_failure");

        let mut passed = event("activity.completed", Phase::Verify);
        passed.payload = json!({"operation":"verification.run","message":"Verification passed"});
        assert_eq!(VoicePolicy::narration(&passed).unwrap().key, "tests_passed");
    }

    #[test]
    fn tts_request_contains_only_the_supplied_short_text() {
        let body = GeminiTtsProvider::request_body("The targeted tests are passing.").unwrap();
        assert_eq!(
            body["contents"][0]["parts"][0]["text"],
            "The targeted tests are passing."
        );
        assert!(body.get("repository").is_none());
        assert_eq!(body["generationConfig"]["responseModalities"][0], "AUDIO");
    }

    #[test]
    fn invalid_or_long_narration_never_reaches_provider() {
        assert_eq!(validate_narration(""), Err(VoiceError::InvalidInput));
        assert_eq!(
            validate_narration(&"x".repeat(MAX_NARRATION_CHARS + 1)),
            Err(VoiceError::InvalidInput)
        );
    }

    #[test]
    fn high_priority_jobs_supersede_queued_low_priority_jobs() {
        let queue = VoiceQueue::new(3);
        let run_id = Uuid::new_v4();
        for key in ["a", "b", "c"] {
            assert!(queue.push(VoiceJob {
                run_id,
                narration: Narration {
                    key,
                    text: "low",
                    priority: VoicePriority::Low
                },
            }));
        }
        assert!(queue.push(VoiceJob {
            run_id,
            narration: Narration {
                key: "high",
                text: "high",
                priority: VoicePriority::High
            },
        }));
        assert_eq!(queue.len(), 1);
    }

    struct MockProvider {
        calls: Arc<Mutex<Vec<String>>>,
        failures: bool,
    }

    impl VoiceProvider for MockProvider {
        fn synthesize(&self, text: &str) -> Result<AudioClip, VoiceError> {
            self.calls.lock().unwrap().push(text.to_owned());
            if self.failures {
                Err(VoiceError::Http)
            } else {
                Ok(AudioClip {
                    bytes: vec![1],
                    mime_type: "audio/wav".into(),
                })
            }
        }
    }

    struct MockPlayer {
        calls: Arc<AtomicUsize>,
    }

    impl AudioPlayer for MockPlayer {
        fn play(&self, _audio: &AudioClip, _timeout: Duration) -> Result<(), VoiceError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    #[test]
    fn provider_failure_is_isolated_from_event_observation_and_repeated_events_are_suppressed() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let service = VoiceService::with_components(
            Arc::new(MockProvider {
                calls: calls.clone(),
                failures: true,
            }),
            Arc::new(MockPlayer {
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            2,
        );
        let event = event("state.changed", Phase::Understand);
        service.observe(&event);
        service.observe(&event);
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(
            calls.lock().unwrap().as_slice(),
            ["I'm starting the task and mapping the repository."]
        );
        assert_eq!(
            service.snapshot(event.run_id).state,
            VoiceState::Unavailable
        );
    }

    #[test]
    fn terminal_completion_uses_high_priority_narration() {
        let state = ExecutionState::new("task", "/repo");
        let mut event = EngineEvent::new(&state, "run.terminated");
        event.status = TerminalStatus::Complete;
        assert_eq!(
            VoicePolicy::narration(&event).unwrap().priority,
            VoicePriority::High
        );
    }

    #[test]
    fn disabled_voice_never_requires_credentials_or_starts_a_worker() {
        let service = VoiceService::from_config(VoiceConfig {
            enabled: false,
            api_key: None,
            model: DEFAULT_TTS_MODEL.into(),
            timeout: Duration::from_secs(1),
            max_queue_size: 1,
        });
        let snapshot = service.snapshot(Uuid::new_v4());
        assert!(!snapshot.enabled);
        assert_eq!(snapshot.provider, "none");
        assert_eq!(snapshot.state, VoiceState::Disabled);
    }

    #[test]
    fn enabled_voice_without_a_key_degrades_to_unavailable() {
        let service = VoiceService::from_config(VoiceConfig {
            enabled: true,
            api_key: None,
            model: DEFAULT_TTS_MODEL.into(),
            timeout: Duration::from_secs(1),
            max_queue_size: 1,
        });
        assert_eq!(
            service.snapshot(Uuid::new_v4()).state,
            VoiceState::Unavailable
        );
    }
}
