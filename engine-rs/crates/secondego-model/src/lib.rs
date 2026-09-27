//! Provider-neutral structured-plan adapters.
//!
//! The runtime depends only on `ModelProvider`. Provider selection is explicit
//! through `SECONDEGO_PROVIDER`; the evaluator credential remains `AI_API_KEY`.

use std::collections::VecDeque;
use std::time::Duration;

use reqwest::blocking::Client;
use secondego_core::ActionProposal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com";
const DEEPSEEK_DEFAULT_MODEL: &str = "deepseek-flash";
const GEMINI_DEFAULT_MODEL: &str = "gemini-3.8-flash";
const MAX_RATIONALE_CHARS: usize = 2_000;
const FALLBACK_RATIONALE: &str = "No rationale supplied by model.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    MissingApiKey,
    UnsupportedProvider(String),
    Transport(String),
    InvalidResponse,
    InvalidAction(String),
    ModelProviderMismatch { provider: String, model: String },
    ScriptExhausted,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingApiKey => write!(
                formatter,
                "AI_API_KEY is required for the selected model provider"
            ),
            Self::UnsupportedProvider(provider) => write!(
                formatter,
                "unsupported model provider: {provider}; use deepseek or gemini"
            ),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    DeepSeek,
    Gemini,
}

impl ProviderKind {
    pub fn from_environment() -> Result<Self, ProviderError> {
        let value = std::env::var("SECONDEGO_PROVIDER")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                let model = std::env::var("SECONDEGO_MODEL").unwrap_or_default();
                if model.trim().to_ascii_lowercase().starts_with("gemini-") {
                    "gemini".into()
                } else {
                    "deepseek".into()
                }
            });
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "deepseek" => Ok(Self::DeepSeek),
            "gemini" => Ok(Self::Gemini),
            unsupported => Err(ProviderError::UnsupportedProvider(unsupported.into())),
        }
    }

    pub const fn default_model(self) -> &'static str {
        match self {
            Self::DeepSeek => DEEPSEEK_DEFAULT_MODEL,
            Self::Gemini => GEMINI_DEFAULT_MODEL,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::DeepSeek => "deepseek",
            Self::Gemini => "gemini",
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
        ProviderKind::DeepSeek => normalized.starts_with("gemini-"),
        ProviderKind::Gemini => normalized.starts_with("deepseek-"),
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

pub trait ModelProvider: Send + Sync {
    fn generate(
        &self,
        prompt: &str,
        context: &ProviderContext,
    ) -> Result<ActionProposal, ProviderError>;

    fn estimate_tokens(&self, text: &str) -> u32 {
        ((text.len() + 3) / 4).max(1) as u32
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
    ) -> Result<ActionProposal, ProviderError> {
        self.proposals
            .lock()
            .map_err(|_| ProviderError::ScriptExhausted)?
            .pop_front()
            .ok_or(ProviderError::ScriptExhausted)
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
    ) -> Result<ActionProposal, ProviderError> {
        let key = resolve_api_key(&self.api_key)?;
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let response = post_json(
            "DeepSeek",
            &url,
            &key,
            deepseek_request(&self.model, prompt),
            self.timeout,
        )?;
        parse_openai_content(&response).and_then(parse_action)
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
    ) -> Result<ActionProposal, ProviderError> {
        let api_key = resolve_api_key(&self.api_key)?;
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
        if !response.status().is_success() {
            return Err(http_status_error("Gemini", response.status()));
        }
        let payload: Value = response
            .json()
            .map_err(|_| ProviderError::InvalidResponse)?;
        parse_action(&parse_gemini_content(&payload)?)
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
                    "action": {"type": "STRING", "enum": ["submit_plan"]},
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
        })
    }

    pub fn with_model(mut self, model: String) -> Result<Self, ProviderError> {
        let kind = match &self {
            Self::DeepSeek(_) => ProviderKind::DeepSeek,
            Self::Gemini(_) => ProviderKind::Gemini,
        };
        validate_model_for_provider(kind, &model)?;
        match &mut self {
            Self::DeepSeek(provider) => provider.model = model,
            Self::Gemini(provider) => provider.model = model,
        }
        Ok(self)
    }
}

impl ModelProvider for ConfiguredProvider {
    fn generate(
        &self,
        prompt: &str,
        context: &ProviderContext,
    ) -> Result<ActionProposal, ProviderError> {
        match self {
            Self::DeepSeek(provider) => provider.generate(prompt, context),
            Self::Gemini(provider) => provider.generate(prompt, context),
        }
    }
}

fn resolve_api_key(explicit: &Option<String>) -> Result<String, ProviderError> {
    explicit
        .clone()
        .or_else(|| std::env::var("AI_API_KEY").ok())
        .filter(|key| !key.is_empty())
        .ok_or(ProviderError::MissingApiKey)
}

fn post_json(
    provider: &str,
    url: &str,
    api_key: &str,
    request: Value,
    timeout: Duration,
) -> Result<Value, ProviderError> {
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
    if !response.status().is_success() {
        return Err(http_status_error(provider, response.status()));
    }
    response.json().map_err(|_| ProviderError::InvalidResponse)
}

fn http_status_error(provider: &str, status: reqwest::StatusCode) -> ProviderError {
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return ProviderError::Transport(format!(
            "{provider} authentication failed (HTTP {status}). Verify that AI_API_KEY belongs to {provider}; the key is not printed."
        ));
    }
    ProviderError::Transport(format!("{provider} returned HTTP {status}"))
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
            serde_json::json!(["submit_plan"])
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
            provider.generate("prompt", &context).unwrap().action,
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
    }
}
