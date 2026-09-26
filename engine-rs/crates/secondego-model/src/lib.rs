use std::collections::VecDeque;
use std::time::Duration;

use reqwest::blocking::Client;
use secondego_core::ActionProposal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    MissingApiKey,
    Transport,
    InvalidResponse,
    InvalidAction(String),
    ScriptExhausted,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingApiKey => write!(formatter, "AI_API_KEY is required for GeminiProvider"),
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
pub struct GeminiProvider {
    pub model: String,
    pub api_key: Option<String>,
    pub timeout: Duration,
}

impl Default for GeminiProvider {
    fn default() -> Self {
        Self {
            model: "gemini-3.8-flash".into(),
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
        let api_key = self
            .api_key
            .clone()
            .or_else(|| std::env::var("AI_API_KEY").ok())
            .filter(|key| !key.is_empty())
            .ok_or(ProviderError::MissingApiKey)?;
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
            .and_then(|items| items.as_array())
            .and_then(|items| items.first())
            .and_then(|candidate| candidate.get("content"))
            .and_then(|content| content.get("parts"))
            .and_then(|parts| parts.as_array())
            .and_then(|parts| parts.first())
            .and_then(|part| part.get("text"))
            .and_then(Value::as_str)
            .ok_or(ProviderError::InvalidResponse)?;
        parse_action(text)
    }
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
