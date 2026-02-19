//! # OpenAI Provider
//!
//! Implementation for OpenAI and OpenAI-compatible APIs.

use crate::{
    GenerateOptions, LlmProvider, Message, MessageRole, ModelInfo, ModelTier,
    ProviderError, Response, Result, Usage, get_model_tier,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

/// OpenAI API provider
pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    base_url: String,
    default_model: String,
}

impl OpenAiProvider {
    /// Create a new OpenAI provider
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: "https://api.openai.com/v1".to_string(),
            default_model: "gpt-4o".to_string(),
        }
    }

    /// Create with a custom base URL (for compatible APIs)
    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: base_url.into(),
            default_model: "gpt-4o".to_string(),
        }
    }

    /// Set the default model
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Convert our message format to OpenAI format
    fn convert_messages(&self, messages: &[Message]) -> Vec<OpenAiMessage> {
        messages
            .iter()
            .map(|m| OpenAiMessage {
                role: match m.role {
                    MessageRole::System => "system".to_string(),
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::Tool => "tool".to_string(),
                },
                content: m.content.clone(),
                tool_call_id: m.tool_call_id.clone(),
                name: m.name.clone(),
            })
            .collect()
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    async fn is_available(&self) -> bool {
        // Check if API key is set and valid
        if self.api_key.is_empty() {
            return false;
        }

        // Try a simple models list request
        let url = format!("{}/models", self.base_url);
        match self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    fn default_model(&self) -> &str {
        &self.default_model
    }

    fn model_tier(&self, model: &str) -> ModelTier {
        get_model_tier(model)
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/models", self.base_url);
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::ApiError {
                status,
                message: body,
            });
        }

        let models_response: OpenAiModelsResponse = response.json().await?;

        Ok(models_response
            .data
            .into_iter()
            .filter(|m| m.id.starts_with("gpt"))
            .map(|m| ModelInfo {
                id: m.id.clone(),
                name: m.id.clone(),
                provider: "openai".to_string(),
                tier: get_model_tier(&m.id),
                context_length: Some(128000), // Default for GPT-4
                supports_vision: m.id.contains("vision") || m.id.contains("gpt-4o"),
                supports_tools: true,
            })
            .collect())
    }

    async fn chat(&self, messages: &[Message], options: &GenerateOptions) -> Result<Response> {
        let model = options
            .model
            .as_ref()
            .unwrap_or(&self.default_model)
            .clone();

        debug!("OpenAI chat with model: {}", model);

        let mut request = OpenAiChatRequest {
            model: model.clone(),
            messages: self.convert_messages(messages),
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            stop: options.stop.clone(),
            response_format: if options.json_mode {
                Some(ResponseFormat {
                    r#type: "json_object".to_string(),
                })
            } else {
                None
            },
        };

        // Add system prompt if provided
        if let Some(system) = &options.system {
            request.messages.insert(
                0,
                OpenAiMessage {
                    role: "system".to_string(),
                    content: system.clone(),
                    tool_call_id: None,
                    name: None,
                },
            );
        }

        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();

            // Check for rate limiting
            if status == 429 {
                return Err(ProviderError::RateLimited { retry_after: None });
            }

            return Err(ProviderError::ApiError {
                status,
                message: body,
            });
        }

        let chat_response: OpenAiChatResponse = response.json().await?;

        let choice = chat_response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::InvalidResponse("No choices in response".to_string()))?;

        Ok(Response {
            content: choice.message.content.unwrap_or_default(),
            model,
            finish_reason: choice.finish_reason,
            usage: chat_response.usage.map(|u| Usage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
        })
    }
}

// OpenAI API types

#[derive(Debug, Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Debug, Serialize)]
struct ResponseFormat {
    r#type: String,
}

#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModel {
    id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_provider_new() {
        let provider = OpenAiProvider::new("sk-test-key");
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_openai_provider_with_base_url() {
        let provider = OpenAiProvider::with_base_url("sk-test", "http://custom:8080/v1");
        assert_eq!(provider.base_url, "http://custom:8080/v1");
    }

    #[test]
    fn test_openai_provider_with_default_model() {
        let provider = OpenAiProvider::new("sk-test")
            .with_default_model("gpt-4o");
        assert_eq!(provider.default_model, "gpt-4o");
    }

    #[test]
    fn test_openai_provider_default_model_none() {
        let provider = OpenAiProvider::new("sk-test");
        assert_eq!(provider.default_model, "gpt-4o"); // default
    }
}
