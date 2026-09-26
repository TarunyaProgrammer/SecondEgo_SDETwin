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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    MissingApiKey,
    UnsupportedProvider(String),
    Transport,
    InvalidResponse,
    InvalidAction(String),
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
            Self::Transport => write!(formatter, "model request failed"),
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
        let value = std::env::var("SECONDEGO_PROVIDER").unwrap_or_else(|_| "deepseek".into());
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
    Ok(std::env::var("SECONDEGO_MODEL")
        .ok()
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| provider.default_model().into()))
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
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, api_key
        );
        let request = serde_json::json!({
            "contents": [{"parts": [{"text": prompt}]}],
            "generationConfig": {"responseMimeType": "application/json"}
        });
        let client = Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|_| ProviderError::Transport)?;
        let response = client
            .post(url)
            .json(&request)
            .send()
            .map_err(|_| ProviderError::Transport)?;
        if !response.status().is_success() {
            return Err(ProviderError::Transport);
        }
        let payload: Value = response
            .json()
            .map_err(|_| ProviderError::InvalidResponse)?;
        let text = payload
            .get("candidates")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|candidate| candidate.get("content"))
            .and_then(|content| content.get("parts"))
            .and_then(Value::as_array)
            .and_then(|parts| parts.first())
            .and_then(|part| part.get("text"))
            .and_then(Value::as_str)
            .ok_or(ProviderError::InvalidResponse)?;
        parse_action(text)
    }
}

#[derive(Debug, Clone)]
pub enum ConfiguredProvider {
    DeepSeek(DeepSeekProvider),
    Gemini(GeminiProvider),
}

impl ConfiguredProvider {
    pub fn from_environment() -> Result<Self, ProviderError> {
        match ProviderKind::from_environment()? {
            ProviderKind::DeepSeek => Ok(Self::DeepSeek(DeepSeekProvider::default())),
            ProviderKind::Gemini => Ok(Self::Gemini(GeminiProvider::default())),
        }
    }

    pub fn with_model(mut self, model: String) -> Self {
        match &mut self {
            Self::DeepSeek(provider) => provider.model = model,
            Self::Gemini(provider) => provider.model = model,
        }
        self
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
    url: &str,
    api_key: &str,
    request: Value,
    timeout: Duration,
) -> Result<Value, ProviderError> {
    let client = Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|_| ProviderError::Transport)?;
    let response = client
        .post(url)
        .bearer_auth(api_key)
        .json(&request)
        .send()
        .map_err(|_| ProviderError::Transport)?;
    if !response.status().is_success() {
        return Err(ProviderError::Transport);
    }
    response.json().map_err(|_| ProviderError::InvalidResponse)
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
    let value: Value = serde_json::from_str(text).map_err(|_| ProviderError::InvalidResponse)?;
    let action = value
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| ProviderError::InvalidAction("action must be a string".into()))?;
    let arguments = value
        .get("arguments")
        .and_then(Value::as_object)
        .ok_or_else(|| ProviderError::InvalidAction("arguments must be an object".into()))?;
    let rationale = value
        .get("rationale")
        .and_then(Value::as_str)
        .ok_or_else(|| ProviderError::InvalidAction("rationale must be a string".into()))?;
    Ok(ActionProposal {
        action: action.into(),
        arguments: Value::Object(arguments.clone()),
        rationale: rationale.into(),
    })
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
}
