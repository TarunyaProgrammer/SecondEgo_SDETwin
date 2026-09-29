//! Provider-neutral structured-plan adapters.
//!
//! The runtime depends only on `ModelProvider`. Provider selection is explicit
//! through `SECONDEGO_PROVIDER`; provider-specific credentials stay outside the
//! runtime and are read only when a request is sent.

use std::collections::VecDeque;
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, RETRY_AFTER};
use secondego_core::ActionProposal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com";
const DEEPSEEK_DEFAULT_MODEL: &str = "deepseek-flash";
const GEMINI_DEFAULT_MODEL: &str = "gemini-3.8-flash";
const GROQ_BASE_URL: &str = "https://api.groq.com/openai/v1";
const GROQ_DEFAULT_MODEL: &str = "qwen/qwen3.8-27b";
const GROQ_MAX_COMPLETION_TOKENS: u32 = 4_096;
const GROQ_PLANNING_PROMPT_TOKEN_LIMIT: u32 = 3_500;
const GROQ_JSON_PLAN_RESPONSE_CONTRACT: &str = "Return exactly one JSON object with action=submit_plan, arguments, and rationale. The supplied repository evidence is the complete planning context. Submit the implementation and verification plan directly; do not request read_file or search_code. Do not use Markdown or provider tools.";
const GROQ_JSON_RECOVERY_RESPONSE_CONTRACT: &str = "Return exactly one JSON object with action=submit_recovery, arguments, and rationale. Do not use Markdown or provider tools.";
const GROQ_PLAN_ACTIONS: &[&str] = &["submit_plan"];
const GROQ_DIAGNOSE_ACTIONS: &[&str] = &["submit_recovery"];
const MAX_RATIONALE_CHARS: usize = 2_000;
const FALLBACK_RATIONALE: &str = "No rationale supplied by model.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    MissingApiKey,
    UnsupportedProvider(String),
    Authentication {
        provider: String,
        status: u16,
    },
    ModelUnavailable {
        provider: String,
        model: String,
    },
    RateLimited {
        provider: String,
        retry_after: Option<Duration>,
    },
    HttpStatus {
        provider: String,
        status: u16,
        code: Option<String>,
    },
    Transport(String),
    InvalidResponse,
    InvalidAction(String),
    ModelProviderMismatch {
        provider: String,
        model: String,
    },
    ScriptExhausted,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingApiKey => write!(
                formatter,
                "an API key is required for the selected model provider"
            ),
            Self::UnsupportedProvider(provider) => write!(
                formatter,
                "unsupported model provider: {provider}; use deepseek, gemini, or groq"
            ),
            Self::Authentication { provider, status } => write!(
                formatter,
                "{provider} authentication failed (HTTP {status}). Verify that the configured key belongs to {provider}; the key is not printed."
            ),
            Self::ModelUnavailable { provider, model } => write!(
                formatter,
                "{provider} model '{model}' was not found or is not enabled for this key; set SECONDEGO_MODEL to an available {provider} model"
            ),
            Self::RateLimited {
                provider,
                retry_after: Some(delay),
            } => write!(
                formatter,
                "{provider} rate limit reached; the provider allows one retry after {}s",
                format_duration(*delay)
            ),
            Self::RateLimited {
                provider,
                retry_after: None,
            } => write!(
                formatter,
                "{provider} rate limit reached; no safe retry delay was supplied"
            ),
            Self::HttpStatus {
                provider,
                status,
                code: Some(code),
            } => write!(formatter, "{provider} returned HTTP {status} ({code})"),
            Self::HttpStatus {
                provider,
                status,
                code: None,
            } => write!(formatter, "{provider} returned HTTP {status}"),
            Self::Transport(detail) => write!(formatter, "model request failed: {detail}"),
            Self::ModelProviderMismatch { provider, model } => write!(
                formatter,
                "model '{model}' does not belong to provider '{provider}'; set SECONDEGO_PROVIDER explicitly or choose a matching SECONDEGO_MODEL"
            ),
            Self::InvalidResponse => write!(formatter, "model returned an invalid response"),
            Self::InvalidAction(message) => write!(formatter, "invalid model action: {message}"),
            Self::ScriptExhausted => {
                write!(formatter, "scripted provider has no remaining proposal")
            }
        }
    }
}

impl std::error::Error for ProviderError {}

impl ProviderError {
    /// Errors that can plausibly succeed without changing the request. The
    /// runtime owns the bounded retry budget; providers only classify errors.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::Transport(_)
                | Self::HttpStatus {
                    status: 500 | 502 | 503 | 504,
                    ..
                }
        )
    }

    pub fn rate_limit_delay(&self) -> Option<Duration> {
        match self {
            Self::RateLimited {
                retry_after: Some(delay),
                ..
            } => Some(*delay),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    DeepSeek,
    Gemini,
    Groq,
}

impl ProviderKind {
    pub fn from_environment() -> Result<Self, ProviderError> {
        let value = std::env::var("SECONDEGO_PROVIDER")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                let model = std::env::var("SECONDEGO_MODEL").unwrap_or_default();
                let model = model.trim().to_ascii_lowercase();
                if model.starts_with("gemini-") {
                    "gemini".into()
                } else if is_known_groq_model(&model) {
                    "groq".into()
                } else {
                    "deepseek".into()
                }
            });
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "deepseek" => Ok(Self::DeepSeek),
            "gemini" => Ok(Self::Gemini),
            "groq" => Ok(Self::Groq),
            unsupported => Err(ProviderError::UnsupportedProvider(unsupported.into())),
        }
    }

    pub const fn default_model(self) -> &'static str {
        match self {
            Self::DeepSeek => DEEPSEEK_DEFAULT_MODEL,
            Self::Gemini => GEMINI_DEFAULT_MODEL,
            Self::Groq => GROQ_DEFAULT_MODEL,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::DeepSeek => "deepseek",
            Self::Gemini => "gemini",
            Self::Groq => "groq",
        }
    }
}

pub fn configured_model() -> Result<String, ProviderError> {
    let provider = ProviderKind::from_environment()?;
    let model = std::env::var("SECONDEGO_MODEL")
        .ok()
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| provider.default_model().into());
    validate_model_for_provider(provider, &model)?;
    Ok(model)
}

fn validate_model_for_provider(provider: ProviderKind, model: &str) -> Result<(), ProviderError> {
    let normalized = model.trim().to_ascii_lowercase();
    let mismatch = match provider {
        ProviderKind::DeepSeek => {
            normalized.starts_with("gemini-") || is_known_groq_model(&normalized)
        }
        ProviderKind::Gemini => {
            normalized.starts_with("deepseek-") || is_known_groq_model(&normalized)
        }
        ProviderKind::Groq => {
            normalized.starts_with("deepseek-") || normalized.starts_with("gemini-")
        }
    };
    if mismatch {
        return Err(ProviderError::ModelProviderMismatch {
            provider: provider.name().into(),
            model: model.into(),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderContext {
    pub run_id: String,
    pub phase: String,
}

/// Safe, provider-reported token accounting. These values never contain a
/// prompt, response, credential, or provider error body.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModelUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

/// Safe rate-limit metadata extracted from response headers. Groq documents
/// these headers as organization-level quota state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RateLimitSnapshot {
    pub remaining_tokens: Option<u64>,
    pub reset_after: Option<Duration>,
    pub retry_after: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelResponse {
    pub proposal: ActionProposal,
    pub usage: ModelUsage,
    pub rate_limit: RateLimitSnapshot,
}

impl ModelResponse {
    fn new(proposal: ActionProposal, usage: ModelUsage, rate_limit: RateLimitSnapshot) -> Self {
        Self {
            proposal,
            usage,
            rate_limit,
        }
    }

    fn scripted(proposal: ActionProposal) -> Self {
        Self::new(
            proposal,
            ModelUsage::default(),
            RateLimitSnapshot::default(),
        )
    }
}

impl From<ActionProposal> for ModelResponse {
    fn from(proposal: ActionProposal) -> Self {
        Self::scripted(proposal)
    }
}

pub trait ModelProvider: Send + Sync {
    fn generate(
        &self,
        prompt: &str,
        context: &ProviderContext,
    ) -> Result<ModelResponse, ProviderError>;

    fn estimate_tokens(&self, text: &str) -> u32 {
        ((text.len() + 3) / 4).max(1) as u32
    }

    /// A provider can ask the runtime to compact its planning packet before a
    /// request is made. `None` retains the evaluator-default context profile.
    fn planning_prompt_token_limit(&self) -> Option<u32> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct ScriptedProvider {
    proposals: std::sync::Arc<std::sync::Mutex<VecDeque<ActionProposal>>>,
}

impl ScriptedProvider {
    pub fn new(proposals: Vec<ActionProposal>) -> Self {
        Self {
            proposals: std::sync::Arc::new(std::sync::Mutex::new(proposals.into())),
        }
    }
}

impl ModelProvider for ScriptedProvider {
    fn generate(
        &self,
        _prompt: &str,
        _context: &ProviderContext,
    ) -> Result<ModelResponse, ProviderError> {
        self.proposals
            .lock()
            .map_err(|_| ProviderError::ScriptExhausted)?
            .pop_front()
            .ok_or(ProviderError::ScriptExhausted)
            .map(ModelResponse::scripted)
    }
}

#[derive(Debug, Clone)]
pub struct DeepSeekProvider {
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: String,
    pub timeout: Duration,
}

impl Default for DeepSeekProvider {
    fn default() -> Self {
        Self {
            model: std::env::var("SECONDEGO_MODEL")
                .ok()
                .filter(|model| !model.trim().is_empty())
                .unwrap_or_else(|| DEEPSEEK_DEFAULT_MODEL.into()),
            api_key: None,
            base_url: std::env::var("SECONDEGO_DEEPSEEK_BASE_URL")
                .unwrap_or_else(|_| DEEPSEEK_BASE_URL.into()),
            timeout: Duration::from_secs(60),
        }
    }
}

impl ModelProvider for DeepSeekProvider {
    fn generate(
        &self,
        prompt: &str,
        _context: &ProviderContext,
    ) -> Result<ModelResponse, ProviderError> {
        let key = resolve_api_key(&self.api_key, None, true)?;
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let response = post_json(
            "DeepSeek",
            &url,
            &key,
            deepseek_request(&self.model, prompt),
            self.timeout,
            Some(&self.model),
        )?;
        let proposal = parse_openai_content(&response.payload).and_then(parse_action)?;
        Ok(ModelResponse::new(
            proposal,
            openai_usage(&response.payload),
            response.rate_limit,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct GeminiProvider {
    pub model: String,
    pub api_key: Option<String>,
    pub timeout: Duration,
}

impl Default for GeminiProvider {
    fn default() -> Self {
        Self {
            model: std::env::var("SECONDEGO_MODEL")
                .ok()
                .filter(|model| !model.trim().is_empty())
                .unwrap_or_else(|| GEMINI_DEFAULT_MODEL.into()),
            api_key: None,
            timeout: Duration::from_secs(60),
        }
    }
}

impl ModelProvider for GeminiProvider {
    fn generate(
        &self,
        prompt: &str,
        _context: &ProviderContext,
    ) -> Result<ModelResponse, ProviderError> {
        let api_key = resolve_api_key(&self.api_key, None, true)?;
        let url = gemini_endpoint(&self.model);
        let request = gemini_request(prompt);
        let client = Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|error| ProviderError::Transport(error.to_string()))?;
        let response = client
            .post(url)
            .header("x-goog-api-key", api_key)
            .json(&request)
            .send()
            .map_err(|error| ProviderError::Transport(error.to_string()))?;
        let status = response.status();
        let rate_limit = rate_limit_snapshot(response.headers());
        if !status.is_success() {
            return Err(http_status_error(
                "Gemini",
                status,
                None,
                response_error_code(response),
                rate_limit,
            ));
        }
        let payload: Value = response
            .json()
            .map_err(|_| ProviderError::InvalidResponse)?;
        let proposal = parse_action(&parse_gemini_content(&payload)?)?;
        Ok(ModelResponse::new(
            proposal,
            gemini_usage(&payload),
            rate_limit,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct GroqProvider {
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: String,
    pub timeout: Duration,
}

impl Default for GroqProvider {
    fn default() -> Self {
        Self {
            model: std::env::var("SECONDEGO_MODEL")
                .ok()
                .filter(|model| !model.trim().is_empty())
                .unwrap_or_else(|| GROQ_DEFAULT_MODEL.into()),
            api_key: None,
            base_url: std::env::var("SECONDEGO_GROQ_BASE_URL")
                .unwrap_or_else(|_| GROQ_BASE_URL.into()),
            timeout: Duration::from_secs(60),
        }
    }
}

impl ModelProvider for GroqProvider {
    fn generate(
        &self,
        prompt: &str,
        context: &ProviderContext,
    ) -> Result<ModelResponse, ProviderError> {
        let key = resolve_api_key(&self.api_key, Some("GROQ_API_KEY"), false)?;
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let response = post_json(
            "Groq",
            &url,
            &key,
            groq_request(&self.model, prompt, &context.phase),
            self.timeout,
            Some(&self.model),
        )?;
        let proposal = parse_openai_content(&response.payload)
            .and_then(parse_action)
            .and_then(|proposal| validate_groq_response_action(proposal, &context.phase))?;
        Ok(ModelResponse::new(
            proposal,
            openai_usage(&response.payload),
            response.rate_limit,
        ))
    }

    fn estimate_tokens(&self, text: &str) -> u32 {
        // Source code and JSON are denser than prose; use a deliberately
        // conservative estimate when enforcing a rate-limited provider budget.
        ((text.len() + 2) / 3).max(1) as u32
    }

    fn planning_prompt_token_limit(&self) -> Option<u32> {
        Some(GROQ_PLANNING_PROMPT_TOKEN_LIMIT)
    }
}

fn gemini_endpoint(model: &str) -> String {
    format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent")
}

fn gemini_request(prompt: &str) -> Value {
    // Keep the schema shallow: `arguments` intentionally remains a dynamic
    // object because its exact shape depends on the bounded plan action. The
    // runtime still validates every nested field before executing anything.
    serde_json::json!({
        "contents": [{"parts": [{"text": prompt}]}],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseSchema": {
                "type": "OBJECT",
                "properties": {
                    "action": {"type": "STRING", "enum": ["read_file", "search_code", "submit_plan", "submit_recovery"]},
                    "arguments": {"type": "OBJECT"},
                    "rationale": {"type": "STRING"}
                },
                "required": ["action", "arguments", "rationale"]
            }
        }
    })
}

fn parse_gemini_content(payload: &Value) -> Result<String, ProviderError> {
    let parts = payload
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
        .ok_or(ProviderError::InvalidResponse)?;
    let text = parts
        .iter()
        .filter(|part| {
            !part
                .get("thought")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<String>();
    if text.trim().is_empty() {
        Err(ProviderError::InvalidResponse)
    } else {
        Ok(text)
    }
}

#[derive(Debug, Clone)]
pub enum ConfiguredProvider {
    DeepSeek(DeepSeekProvider),
    Gemini(GeminiProvider),
    Groq(GroqProvider),
}

impl ConfiguredProvider {
    pub fn from_environment() -> Result<Self, ProviderError> {
        let kind = ProviderKind::from_environment()?;
        let model = std::env::var("SECONDEGO_MODEL")
            .ok()
            .filter(|model| !model.trim().is_empty())
            .unwrap_or_else(|| kind.default_model().into());
        validate_model_for_provider(kind, &model)?;
        Ok(match kind {
            ProviderKind::DeepSeek => Self::DeepSeek(DeepSeekProvider {
                model,
                ..DeepSeekProvider::default()
            }),
            ProviderKind::Gemini => Self::Gemini(GeminiProvider {
                model,
                ..GeminiProvider::default()
            }),
            ProviderKind::Groq => Self::Groq(GroqProvider {
                model,
                ..GroqProvider::default()
            }),
        })
    }

    pub fn with_model(mut self, model: String) -> Result<Self, ProviderError> {
        let kind = match &self {
            Self::DeepSeek(_) => ProviderKind::DeepSeek,
            Self::Gemini(_) => ProviderKind::Gemini,
            Self::Groq(_) => ProviderKind::Groq,
        };
        validate_model_for_provider(kind, &model)?;
        match &mut self {
            Self::DeepSeek(provider) => provider.model = model,
            Self::Gemini(provider) => provider.model = model,
            Self::Groq(provider) => provider.model = model,
        }
        Ok(self)
    }
}

impl ModelProvider for ConfiguredProvider {
    fn generate(
        &self,
        prompt: &str,
        context: &ProviderContext,
    ) -> Result<ModelResponse, ProviderError> {
        match self {
            Self::DeepSeek(provider) => provider.generate(prompt, context),
            Self::Gemini(provider) => provider.generate(prompt, context),
            Self::Groq(provider) => provider.generate(prompt, context),
        }
    }

    fn estimate_tokens(&self, text: &str) -> u32 {
        match self {
            Self::DeepSeek(provider) => provider.estimate_tokens(text),
            Self::Gemini(provider) => provider.estimate_tokens(text),
            Self::Groq(provider) => provider.estimate_tokens(text),
        }
    }

    fn planning_prompt_token_limit(&self) -> Option<u32> {
        match self {
            Self::DeepSeek(provider) => provider.planning_prompt_token_limit(),
            Self::Gemini(provider) => provider.planning_prompt_token_limit(),
            Self::Groq(provider) => provider.planning_prompt_token_limit(),
        }
    }
}

fn resolve_api_key(
    explicit: &Option<String>,
    provider_key_name: Option<&str>,
    allow_generic_fallback: bool,
) -> Result<String, ProviderError> {
    let provider_key = explicit
        .clone()
        .or_else(|| provider_key_name.and_then(non_empty_env));
    if let Some(key) = provider_key.filter(|key| !key.trim().is_empty()) {
        return Ok(key);
    }
    if allow_generic_fallback {
        return non_empty_env("AI_API_KEY").ok_or(ProviderError::MissingApiKey);
    }
    Err(ProviderError::MissingApiKey)
}

#[derive(Debug)]
struct HttpJsonResponse {
    payload: Value,
    rate_limit: RateLimitSnapshot,
}

fn post_json(
    provider: &str,
    url: &str,
    api_key: &str,
    request: Value,
    timeout: Duration,
    model: Option<&str>,
) -> Result<HttpJsonResponse, ProviderError> {
    let client = Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|error| ProviderError::Transport(error.to_string()))?;
    let response = client
        .post(url)
        .bearer_auth(api_key)
        .json(&request)
        .send()
        .map_err(|error| ProviderError::Transport(error.to_string()))?;
    let status = response.status();
    let rate_limit = rate_limit_snapshot(response.headers());
    if !status.is_success() {
        return Err(http_status_error(
            provider,
            status,
            model,
            response_error_code(response),
            rate_limit,
        ));
    }
    let payload = response
        .json()
        .map_err(|_| ProviderError::InvalidResponse)?;
    Ok(HttpJsonResponse {
        payload,
        rate_limit,
    })
}

fn http_status_error(
    provider: &str,
    status: reqwest::StatusCode,
    model: Option<&str>,
    code: Option<String>,
    rate_limit: RateLimitSnapshot,
) -> ProviderError {
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return ProviderError::Authentication {
            provider: provider.into(),
            status: status.as_u16(),
        };
    }
    if status == reqwest::StatusCode::NOT_FOUND {
        if let Some(model) = model {
            return ProviderError::ModelUnavailable {
                provider: provider.into(),
                model: model.into(),
            };
        }
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return ProviderError::RateLimited {
            provider: provider.into(),
            retry_after: rate_limit.retry_after.or(rate_limit.reset_after),
        };
    }
    ProviderError::HttpStatus {
        provider: provider.into(),
        status: status.as_u16(),
        code,
    }
}

fn openai_usage(payload: &Value) -> ModelUsage {
    usage_from_fields(
        payload.pointer("/usage/prompt_tokens"),
        payload.pointer("/usage/completion_tokens"),
        payload.pointer("/usage/total_tokens"),
    )
}

fn gemini_usage(payload: &Value) -> ModelUsage {
    usage_from_fields(
        payload.pointer("/usageMetadata/promptTokenCount"),
        payload.pointer("/usageMetadata/candidatesTokenCount"),
        payload.pointer("/usageMetadata/totalTokenCount"),
    )
}

fn usage_from_fields(
    input: Option<&Value>,
    output: Option<&Value>,
    total: Option<&Value>,
) -> ModelUsage {
    let input_tokens = input.and_then(Value::as_u64);
    let output_tokens = output.and_then(Value::as_u64);
    let total_tokens = total.and_then(Value::as_u64).or_else(|| {
        input_tokens
            .zip(output_tokens)
            .map(|(left, right)| left + right)
    });
    ModelUsage {
        input_tokens,
        output_tokens,
        total_tokens,
    }
}

fn rate_limit_snapshot(headers: &HeaderMap) -> RateLimitSnapshot {
    RateLimitSnapshot {
        remaining_tokens: header_u64(headers, "x-ratelimit-remaining-tokens"),
        reset_after: header_duration(headers, "x-ratelimit-reset-tokens"),
        retry_after: headers
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_header_duration),
    }
}

fn header_u64(headers: &HeaderMap, name: &str) -> Option<u64> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse().ok())
}

fn header_duration(headers: &HeaderMap, name: &str) -> Option<Duration> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_header_duration)
}

fn parse_header_duration(value: &str) -> Option<Duration> {
    let value = value.trim();
    if let Some(milliseconds) = value.strip_suffix("ms") {
        let milliseconds = milliseconds.trim().parse::<f64>().ok()?;
        return (milliseconds.is_finite() && milliseconds >= 0.0)
            .then(|| Duration::from_secs_f64(milliseconds / 1_000.0));
    }
    let seconds = value.parse::<f64>().ok().or_else(|| {
        let mut remaining = value;
        let mut total = 0.0;
        for (unit, multiplier) in [("h", 3_600.0), ("m", 60.0), ("s", 1.0)] {
            let Some(index) = remaining.find(unit) else {
                continue;
            };
            let (number, tail) = remaining.split_at(index);
            total += number.trim().parse::<f64>().ok()? * multiplier;
            remaining = &tail[unit.len()..];
        }
        remaining.trim().is_empty().then_some(total)
    })?;
    (seconds.is_finite() && seconds >= 0.0).then(|| Duration::from_secs_f64(seconds))
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs_f64();
    if seconds.fract() == 0.0 {
        format!("{seconds:.0}")
    } else {
        format!("{seconds:.1}")
    }
}

fn response_error_code(response: reqwest::blocking::Response) -> Option<String> {
    let payload: Value = response.json().ok()?;
    safe_provider_error_code(&payload)
}

fn safe_provider_error_code(payload: &Value) -> Option<String> {
    safe_provider_code(
        payload
            .pointer("/error/code")
            .or_else(|| payload.get("code")),
    )
    .or_else(|| {
        payload
            .pointer("/error/failed_generation/reason")
            .and_then(Value::as_str)
            .and_then(classify_provider_failure_reason)
            .map(str::to_owned)
    })
    .or_else(|| safe_provider_code(payload.pointer("/error/type")))
}

fn safe_provider_code(value: Option<&Value>) -> Option<String> {
    let code = value?.as_str()?.trim();
    (!code.is_empty()
        && code.len() <= 96
        && code.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        }))
    .then(|| code.to_owned())
}

fn classify_provider_failure_reason(reason: &str) -> Option<&'static str> {
    let normalized = reason.to_ascii_lowercase();
    if normalized.contains("tool") && normalized.contains("json") {
        Some("tool_arguments_invalid_json")
    } else if normalized.contains("json")
        && (normalized.contains("schema") || normalized.contains("validat"))
    {
        Some("json_schema_validation_failed")
    } else {
        None
    }
}

fn deepseek_request(model: &str, prompt: &str) -> Value {
    serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "Return only one JSON object with action, arguments, and rationale. Do not use markdown."},
            {"role": "user", "content": prompt}
        ],
        "response_format": {"type": "json_object"},
        "thinking": {"type": "disabled"},
        "max_tokens": 4096
    })
}

fn groq_request(model: &str, prompt: &str, phase: &str) -> Value {
    let runtime_input = format!("BEGIN RUNTIME INPUT\n{prompt}\nEND RUNTIME INPUT");
    if groq_supports_strict_outputs(model) {
        serde_json::json!({
            "model": model,
            "messages": [
                {"role": "system", "content": groq_json_response_contract(phase)},
                {"role": "user", "content": runtime_input}
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": if phase.eq_ignore_ascii_case("DIAGNOSE") { "second_ego_recovery" } else { "second_ego_plan" },
                    "strict": true,
                    "schema": groq_response_schema(phase)
                }
            },
            "max_completion_tokens": GROQ_MAX_COMPLETION_TOKENS
        })
    } else {
        // Other Groq models retain the portable JSON-object transport. Do not
        // send GPT-OSS-only reasoning controls to a different model family.
        serde_json::json!({
            "model": model,
            "messages": [
                {"role": "system", "content": groq_json_response_contract(phase)},
                {"role": "user", "content": runtime_input}
            ],
            "response_format": {"type": "json_object"},
            "max_completion_tokens": GROQ_MAX_COMPLETION_TOKENS
        })
    }
}

fn groq_json_response_contract(phase: &str) -> &'static str {
    if phase.eq_ignore_ascii_case("DIAGNOSE") {
        GROQ_JSON_RECOVERY_RESPONSE_CONTRACT
    } else {
        GROQ_JSON_PLAN_RESPONSE_CONTRACT
    }
}

fn groq_supports_strict_outputs(model: &str) -> bool {
    let model = model.trim().to_ascii_lowercase();
    model.starts_with("openai/gpt-oss-") || model == "qwen/qwen3.8-27b"
}

fn groq_allowed_actions(phase: &str) -> &'static [&'static str] {
    if phase.eq_ignore_ascii_case("DIAGNOSE") {
        GROQ_DIAGNOSE_ACTIONS
    } else {
        GROQ_PLAN_ACTIONS
    }
}

fn validate_groq_response_action(
    proposal: ActionProposal,
    phase: &str,
) -> Result<ActionProposal, ProviderError> {
    if groq_allowed_actions(phase)
        .iter()
        .any(|allowed| *allowed == proposal.action)
    {
        Ok(proposal)
    } else {
        Err(ProviderError::InvalidAction(format!(
            "Groq action '{}' is not allowed during {phase}",
            proposal.action
        )))
    }
}

fn groq_response_schema(phase: &str) -> Value {
    // Keep the per-action arguments schema flat (all strings, no arrays).
    // Groq's strict-mode JSON Schema validator rejects nested "items" for
    // array-typed fields (e.g. the old "argv" field caused HTTP 400), so we
    // only expose the four string fields the allowed actions actually use.
    let action_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": [
                    "replace_text", "write_file", "git_diff", "git_status"
                ]
            },
            "arguments": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path for replace_text or write_file; empty string for git_diff or git_status."
                    },
                    "content": {
                        "type": "string",
                        "description": "Complete new file content for write_file; empty string for all other actions."
                    },
                    "old_text": {
                        "type": "string",
                        "description": "Exact unique source fragment for replace_text, copied verbatim from retrieved evidence; empty string for all other actions."
                    },
                    "new_text": {
                        "type": "string",
                        "description": "Replacement fragment for replace_text, must differ from old_text; empty string for all other actions."
                    }
                },
                "required": ["path", "content", "old_text", "new_text"],
                "additionalProperties": false
            },
            "rationale": {"type": "string"}
        },
        "required": ["action", "arguments", "rationale"],
        "additionalProperties": false
    });
    let top_level_action = if phase.eq_ignore_ascii_case("DIAGNOSE") {
        "submit_recovery"
    } else {
        "submit_plan"
    };
    serde_json::json!({
        "type": "object",
        "properties": {
            "action": {"type": "string", "enum": [top_level_action]},
            "arguments": {
                "type": "object",
                "properties": {
                    "actions": {
                        "type": "array",
                        "items": action_schema.clone()
                    },
                    "verification_commands": {
                        "type": "array",
                        "items": {
                            "type": "array",
                            "items": {"type": "string"}
                        }
                    },
                    "recovery_actions": {
                        "type": "array",
                        "items": action_schema
                    }
                },
                "required": ["actions", "verification_commands", "recovery_actions"],
                "additionalProperties": false
            },
            "rationale": {"type": "string"}
        },
        "required": ["action", "arguments", "rationale"],
        "additionalProperties": false
    })
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn is_known_groq_model(model: &str) -> bool {
    model.starts_with("llama-")
        || model.starts_with("mixtral-")
        || model.starts_with("openai/")
        || model.starts_with("qwen/")
        || model.starts_with("groq/")
        || model.starts_with("canopylabs/")
        || model.starts_with("allam-")
}

fn parse_openai_content(payload: &Value) -> Result<&str, ProviderError> {
    payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .ok_or(ProviderError::InvalidResponse)
}

pub fn parse_action(text: &str) -> Result<ActionProposal, ProviderError> {
    let value = parse_structured_json(text)?;
    let action = value
        .get("action")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|action| !action.is_empty())
        .ok_or_else(|| ProviderError::InvalidAction("action must be a string".into()))?;
    let arguments = value
        .get("arguments")
        .and_then(Value::as_object)
        .ok_or_else(|| ProviderError::InvalidAction("arguments must be an object".into()))?;
    Ok(ActionProposal {
        action: action.into(),
        arguments: Value::Object(arguments.clone()),
        // Rationale never authorizes a tool or a diff. Some providers still
        // emit it as a list/object despite a schema request, so normalize it
        // rather than terminating a safe, otherwise-valid plan.
        rationale: normalize_rationale(value.get("rationale")),
    })
}

fn parse_structured_json(text: &str) -> Result<Value, ProviderError> {
    let trimmed = text.trim();
    let json = if let Some(fenced) = trimmed.strip_prefix("```") {
        let body_start = fenced.find('\n').ok_or(ProviderError::InvalidResponse)? + 1;
        fenced[body_start..]
            .strip_suffix("```")
            .map(str::trim)
            .ok_or(ProviderError::InvalidResponse)?
    } else {
        trimmed
    };
    serde_json::from_str(json).map_err(|_| ProviderError::InvalidResponse)
}

fn normalize_rationale(value: Option<&Value>) -> String {
    let rationale = match value {
        Some(Value::String(text)) => text.trim().to_owned(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        Some(Value::Object(object)) => ["summary", "reason", "text", "rationale"]
            .into_iter()
            .find_map(|field| object.get(field).and_then(Value::as_str))
            .map(str::trim)
            .unwrap_or_default()
            .to_owned(),
        _ => String::new(),
    };
    let rationale = if rationale.is_empty() {
        FALLBACK_RATIONALE.into()
    } else {
        rationale
    };
    rationale.chars().take(MAX_RATIONALE_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deepseek_is_the_evaluator_default() {
        assert_eq!(ProviderKind::DeepSeek.default_model(), "deepseek-flash");
        assert_eq!(ProviderKind::DeepSeek.name(), "deepseek");
    }

    #[test]
    fn groq_has_a_json_object_default_model() {
        assert_eq!(ProviderKind::Groq.default_model(), "qwen/qwen3.8-27b");
        assert_eq!(ProviderKind::Groq.name(), "groq");
    }

    #[test]
    fn deepseek_payload_requests_json_without_thinking() {
        let request = deepseek_request("deepseek-flash", "make a plan");
        assert_eq!(request["response_format"]["type"], "json_object");
        assert_eq!(request["thinking"]["type"], "disabled");
        assert_eq!(request["messages"][1]["content"], "make a plan");
    }

    #[test]
    fn gemini_endpoint_never_embeds_the_api_key() {
        let endpoint = gemini_endpoint("gemini-3.8-flash");
        assert_eq!(
            endpoint,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.8-flash:generateContent"
        );
        assert!(!endpoint.contains("key="));
    }

    #[test]
    fn gemini_request_requires_the_safe_top_level_contract() {
        let request = gemini_request("make a plan");
        assert_eq!(
            request["generationConfig"]["responseMimeType"],
            "application/json"
        );
        assert_eq!(
            request["generationConfig"]["responseSchema"]["properties"]["action"]["enum"],
            serde_json::json!(["read_file", "search_code", "submit_plan", "submit_recovery"])
        );
        assert_eq!(
            request["generationConfig"]["responseSchema"]["properties"]["rationale"]["type"],
            "STRING"
        );
    }

    #[test]
    fn gemini_content_ignores_thought_parts_and_concatenates_response_parts() {
        let payload = serde_json::json!({
            "candidates": [{"content": {"parts": [
                {"text": "internal", "thought": true},
                {"text": "{\"action\":"},
                {"text": "\"submit_plan\",\"arguments\":{},\"rationale\":\"plan\"}"}
            ]}}]
        });
        assert_eq!(
            parse_gemini_content(&payload).unwrap(),
            r#"{"action":"submit_plan","arguments":{},"rationale":"plan"}"#
        );
    }

    #[test]
    fn openai_compatible_response_is_parsed() {
        let payload = serde_json::json!({"choices":[{"message":{"content":"{\"action\":\"read_file\",\"arguments\":{\"path\":\"README.md\"},\"rationale\":\"inspect\"}"}}]});
        assert_eq!(
            parse_openai_content(&payload).unwrap(),
            r#"{"action":"read_file","arguments":{"path":"README.md"},"rationale":"inspect"}"#
        );
    }

    #[test]
    fn groq_gpt_oss_payload_uses_strict_direct_plan_schema() {
        let request = groq_request("openai/gpt-oss-20b", "make a plan", "PLAN");
        assert_eq!(request["model"], "openai/gpt-oss-20b");
        assert_eq!(request["max_completion_tokens"], 4096);
        assert!(request.get("tools").is_none());
        assert_eq!(request["response_format"]["type"], "json_schema");
        assert_eq!(request["response_format"]["json_schema"]["strict"], true);
        assert_eq!(
            request["response_format"]["json_schema"]["schema"]["properties"]["action"]["enum"],
            serde_json::json!(["submit_plan"])
        );
        assert_eq!(
            request["response_format"]["json_schema"]["schema"]["additionalProperties"],
            false
        );
    }

    #[test]
    fn groq_strict_schema_requires_recovery_during_diagnosis() {
        let request = groq_request("openai/gpt-oss-20b", "recover", "DIAGNOSE");
        assert_eq!(
            request["response_format"]["json_schema"]["schema"]["properties"]["action"]["enum"],
            serde_json::json!(["submit_recovery"])
        );
    }

    #[test]
    fn groq_non_gpt_oss_payload_uses_portable_json_without_reasoning_controls() {
        let request = groq_request("llama-3.3-70b-versatile", "make a plan", "PLAN");
        assert_eq!(request["response_format"]["type"], "json_object");
        assert!(request.get("tools").is_none());
        assert!(request.get("tool_choice").is_none());
        assert!(request.get("reasoning_effort").is_none());
        assert!(request.get("include_reasoning").is_none());
        assert_eq!(request["max_completion_tokens"], 4096);
        assert!(
            request["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("action=submit_plan")
        );
    }

    #[test]
    fn groq_portable_json_rejects_a_planning_inspection() {
        let payload = serde_json::json!({
            "choices": [{"message": {"content": "{\"action\":\"search_code\",\"arguments\":{\"query\":\"cache\"},\"rationale\":\"inspect\"}"}}]
        });
        let error = parse_openai_content(&payload)
            .and_then(parse_action)
            .and_then(|proposal| validate_groq_response_action(proposal, "PLAN"))
            .unwrap_err();
        assert!(matches!(error, ProviderError::InvalidAction(_)));
    }

    #[test]
    fn groq_portable_json_rejects_top_level_patch() {
        let payload = serde_json::json!({
            "choices": [{"message": {"content": "{\"action\":\"apply_patch\",\"arguments\":{\"patch\":\"diff\"},\"rationale\":\"edit\"}"}}]
        });
        let error = parse_openai_content(&payload)
            .and_then(parse_action)
            .and_then(|proposal| validate_groq_response_action(proposal, "PLAN"))
            .unwrap_err();
        assert!(matches!(error, ProviderError::InvalidAction(_)));
    }

    #[test]
    fn qwen_uses_the_same_strict_plan_contract() {
        let request = groq_request("qwen/qwen3.8-27b", "make a plan", "PLAN");
        assert_eq!(request["response_format"]["type"], "json_schema");
        assert_eq!(request["response_format"]["json_schema"]["strict"], true);
        assert!(request.get("tools").is_none());
        let actions = request["response_format"]["json_schema"]["schema"]["properties"]
            ["arguments"]["properties"]["actions"]["items"]["properties"]["action"]["enum"]
            .as_array()
            .unwrap();
        assert!(actions.iter().any(|action| action == "replace_text"));
        assert!(!actions.iter().any(|action| action == "apply_patch"));
    }

    #[test]
    fn scripted_provider_is_deterministic_and_bounded() {
        let provider = ScriptedProvider::new(vec![ActionProposal {
            action: "read_file".into(),
            arguments: serde_json::json!({"path": "README.md"}),
            rationale: "inspect".into(),
        }]);
        let context = ProviderContext {
            run_id: "run".into(),
            phase: "PLAN".into(),
        };
        assert_eq!(
            provider
                .generate("prompt", &context)
                .unwrap()
                .proposal
                .action,
            "read_file"
        );
        assert_eq!(
            provider.generate("prompt", &context),
            Err(ProviderError::ScriptExhausted)
        );
    }

    #[test]
    fn action_parser_rejects_unstructured_provider_output() {
        assert!(matches!(
            parse_action("not json"),
            Err(ProviderError::InvalidResponse)
        ));
        assert!(
            parse_action(
                r#"{"action":"read_file","arguments":{"path":"a"},"rationale":"inspect"}"#
            )
            .is_ok()
        );
        assert!(matches!(
            parse_action(r#"{"action":"submit_plan","arguments_json":"{}","rationale":"plan"}"#),
            Err(ProviderError::InvalidAction(_))
        ));
    }

    #[test]
    fn action_parser_accepts_fenced_json_and_normalizes_non_authoritative_rationale() {
        let proposal = parse_action(
            "```json\n{\"action\":\"submit_plan\",\"arguments\":{},\"rationale\":[\"inspect cache\",\"then verify\"]}\n```",
        )
        .unwrap();
        assert_eq!(proposal.action, "submit_plan");
        assert_eq!(proposal.rationale, "inspect cache then verify");

        let fallback =
            parse_action(r#"{"action":"submit_plan","arguments":{},"rationale":{"detail":true}}"#)
                .unwrap();
        assert_eq!(fallback.rationale, FALLBACK_RATIONALE);
    }

    #[test]
    fn action_parser_keeps_executable_fields_strict() {
        assert!(matches!(
            parse_action(r#"{"action":"","arguments":{},"rationale":"x"}"#),
            Err(ProviderError::InvalidAction(_))
        ));
        assert!(matches!(
            parse_action(r#"{"action":"submit_plan","arguments":[],"rationale":"x"}"#),
            Err(ProviderError::InvalidAction(_))
        ));
    }

    #[test]
    fn configured_provider_rejects_a_cross_provider_model_override() {
        let provider = ConfiguredProvider::DeepSeek(DeepSeekProvider::default());
        assert!(matches!(
            provider.with_model("gemini-3.8-flash".into()),
            Err(ProviderError::ModelProviderMismatch { .. })
        ));

        let provider = ConfiguredProvider::Groq(GroqProvider::default());
        assert!(matches!(
            provider.with_model("gemini-3.8-flash".into()),
            Err(ProviderError::ModelProviderMismatch { .. })
        ));
    }

    #[test]
    fn provider_classifies_only_safe_http_errors_as_transient() {
        assert!(
            ProviderError::HttpStatus {
                provider: "Gemini".into(),
                status: 503,
                code: None,
            }
            .is_transient()
        );
        assert!(ProviderError::Transport("connection reset".into()).is_transient());
        assert!(
            !ProviderError::Authentication {
                provider: "DeepSeek".into(),
                status: 401,
            }
            .is_transient()
        );
        assert!(
            !ProviderError::HttpStatus {
                provider: "Gemini".into(),
                status: 400,
                code: None,
            }
            .is_transient()
        );
        assert!(
            !ProviderError::RateLimited {
                provider: "Groq".into(),
                retry_after: Some(Duration::from_secs(2)),
            }
            .is_transient()
        );
        assert!(matches!(
            http_status_error(
                "Groq",
                reqwest::StatusCode::NOT_FOUND,
                Some("missing-model"),
                None,
                RateLimitSnapshot::default(),
            ),
            ProviderError::ModelUnavailable { .. }
        ));
    }

    #[test]
    fn rate_limit_headers_are_parsed_without_response_content() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-ratelimit-remaining-tokens",
            reqwest::header::HeaderValue::from_static("2719"),
        );
        headers.insert(
            "x-ratelimit-reset-tokens",
            reqwest::header::HeaderValue::from_static("7.66s"),
        );
        headers.insert(RETRY_AFTER, reqwest::header::HeaderValue::from_static("8"));

        let snapshot = rate_limit_snapshot(&headers);

        assert_eq!(snapshot.remaining_tokens, Some(2_719));
        assert_eq!(snapshot.reset_after, Some(Duration::from_millis(7_660)));
        assert_eq!(snapshot.retry_after, Some(Duration::from_secs(8)));
        assert_eq!(
            parse_header_duration("2m59.56s"),
            Some(Duration::from_millis(179_560))
        );
        assert!(matches!(
            http_status_error(
                "Groq",
                reqwest::StatusCode::TOO_MANY_REQUESTS,
                Some("openai/gpt-oss-20b"),
                Some("rate_limit_exceeded".into()),
                snapshot,
            ),
            ProviderError::RateLimited {
                retry_after: Some(delay),
                ..
            } if delay == Duration::from_secs(8)
        ));
    }

    #[test]
    fn provider_usage_is_aggregated_from_safe_numeric_fields() {
        let usage = openai_usage(&serde_json::json!({
            "usage": {"prompt_tokens": 321, "completion_tokens": 144, "total_tokens": 465}
        }));
        assert_eq!(usage.input_tokens, Some(321));
        assert_eq!(usage.output_tokens, Some(144));
        assert_eq!(usage.total_tokens, Some(465));
    }

    #[test]
    fn provider_error_details_are_classified_without_retaining_attempted_arguments() {
        let code = safe_provider_error_code(&serde_json::json!({
            "error": {
                "type": "invalid_request_error",
                "failed_generation": {
                    "reason": "Tool call arguments are not valid JSON",
                    "attempted_arguments": "untrusted repository content"
                }
            }
        }));
        assert_eq!(code.as_deref(), Some("tool_arguments_invalid_json"));
    }
}
