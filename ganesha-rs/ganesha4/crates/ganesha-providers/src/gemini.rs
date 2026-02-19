//! # Gemini Provider
//!
//! Implementation for Google Gemini API using their OpenAI-compatible endpoint.

use crate::{
    GenerateOptions, LlmProvider, Message, MessageRole, ModelInfo, ModelTier,
    ProviderError, Response, Result, Usage, get_model_tier,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

/// Google Gemini API provider
pub struct GeminiProvider {
    client: Client,
    api_key: String,
    base_url: String,
    default_model: String,
}

impl GeminiProvider {
    /// Create a new Gemini provider
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
            default_model: "gemini-2.0-flash".to_string(),
        }
    }

    /// Set the default model
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Convert our message format to OpenAI-compatible format
    fn convert_messages(&self, messages: &[Message]) -> Vec<GeminiMessage> {
        messages
            .iter()
            .map(|m| GeminiMessage {
                role: match m.role {
                    MessageRole::System => "system".to_string(),
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::Tool => "tool".to_string(),
                },
                content: m.content.clone(),
            })
            .collect()
    }
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
    }

    async fn is_available(&self) -> bool {
        // Check if API key is set
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

        let models_response: GeminiModelsResponse = response.json().await?;

        Ok(models_response
            .data
            .into_iter()
            .filter(|m| m.id.contains("gemini"))
            .map(|m| ModelInfo {
                id: m.id.clone(),
                name: m.id.clone(),
                provider: "gemini".to_string(),
                tier: get_model_tier(&m.id),
                context_length: Some(1000000), // Gemini has large context
                supports_vision: m.id.contains("vision") || m.id.contains("flash") || m.id.contains("pro"),
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

        debug!("Gemini chat with model: {}", model);

        let mut request = GeminiChatRequest {
            model: model.clone(),
            messages: self.convert_messages(messages),
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            stop: options.stop.clone(),
        };

        // Add system prompt if provided
        if let Some(system) = &options.system {
            request.messages.insert(
                0,
                GeminiMessage {
                    role: "system".to_string(),
                    content: system.clone(),
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

        let chat_response: GeminiChatResponse = response.json().await?;

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

// Gemini API types (OpenAI-compatible)

#[derive(Debug, Serialize)]
struct GeminiChatRequest {
    model: String,
    messages: Vec<GeminiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct GeminiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct GeminiChatResponse {
    choices: Vec<GeminiChoice>,
    usage: Option<GeminiUsage>,
}

#[derive(Debug, Deserialize)]
struct GeminiChoice {
    message: GeminiResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponseMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct GeminiModelsResponse {
    data: Vec<GeminiModel>,
}

#[derive(Debug, Deserialize)]
struct GeminiModel {
    id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_provider_new() {
        let provider = GeminiProvider::new("test-api-key");
        assert_eq!(provider.name(), "gemini");
    }

    #[test]
    fn test_gemini_provider_with_default_model() {
        let provider = GeminiProvider::new("test-key")
            .with_default_model("gemini-pro");
        assert_eq!(provider.default_model, "gemini-pro");
    }

    #[test]
    fn test_gemini_provider_default_model_none() {
        let provider = GeminiProvider::new("test-key");
        assert!(!provider.default_model.is_empty()); // has a default
    }
}
