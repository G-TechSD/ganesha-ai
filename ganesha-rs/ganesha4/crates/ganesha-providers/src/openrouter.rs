//! # OpenRouter Provider
//!
//! Implementation for OpenRouter aggregator API.
//! OpenRouter provides access to many models through a single API.

use crate::{
    GenerateOptions, LlmProvider, Message, MessageRole, ModelInfo, ModelTier,
    ProviderError, Response, Result, Usage, get_model_tier,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1";

/// OpenRouter provider - aggregates many LLM providers
pub struct OpenRouterProvider {
    client: Client,
    api_key: String,
    default_model: String,
    site_url: Option<String>,
    site_name: Option<String>,
}

impl OpenRouterProvider {
    /// Create a new OpenRouter provider
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            default_model: "anthropic/claude-3.5-sonnet".to_string(),
            site_url: None,
            site_name: Some("Ganesha".to_string()),
        }
    }

    /// Set the default model
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Set site URL for OpenRouter attribution
    pub fn with_site_url(mut self, url: impl Into<String>) -> Self {
        self.site_url = Some(url.into());
        self
    }

    /// Set site name for OpenRouter attribution
    pub fn with_site_name(mut self, name: impl Into<String>) -> Self {
        self.site_name = Some(name.into());
        self
    }

    /// Convert messages to OpenRouter format (OpenAI compatible)
    fn convert_messages(&self, messages: &[Message]) -> Vec<OpenRouterMessage> {
        messages
            .iter()
            .map(|m| OpenRouterMessage {
                role: match m.role {
                    MessageRole::System => "system".to_string(),
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::Tool => "tool".to_string(),
                },
                content: m.content.clone(),
                name: m.name.clone(),
            })
            .collect()
    }

    /// Get tier for an OpenRouter model
    fn get_openrouter_tier(&self, model_id: &str) -> ModelTier {
        // OpenRouter model IDs are like "anthropic/claude-3.5-sonnet"
        // Extract the model name part for tier lookup
        let model_name = model_id.split('/').last().unwrap_or(model_id);
        get_model_tier(model_name)
    }
}

#[async_trait]
impl LlmProvider for OpenRouterProvider {
    fn name(&self) -> &str {
        "openrouter"
    }

    async fn is_available(&self) -> bool {
        if self.api_key.is_empty() {
            return false;
        }

        // Try to list models
        let url = format!("{}/models", OPENROUTER_API_URL);
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
        self.get_openrouter_tier(model)
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/models", OPENROUTER_API_URL);
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

        let models_response: OpenRouterModelsResponse = response.json().await?;

        Ok(models_response
            .data
            .into_iter()
            .map(|m| {
                let tier = self.get_openrouter_tier(&m.id);
                ModelInfo {
                    id: m.id.clone(),
                    name: m.name.unwrap_or(m.id.clone()),
                    provider: "openrouter".to_string(),
                    tier,
                    context_length: Some(m.context_length.unwrap_or(4096)),
                    supports_vision: m.architecture.as_ref()
                        .and_then(|a| a.modality.as_ref())
                        .map(|m| m.contains("image"))
                        .unwrap_or(false),
                    supports_tools: true, // Most OpenRouter models support tools
                }
            })
            .collect())
    }

    async fn chat(&self, messages: &[Message], options: &GenerateOptions) -> Result<Response> {
        let model = options
            .model
            .as_ref()
            .unwrap_or(&self.default_model)
            .clone();

        debug!("OpenRouter chat with model: {}", model);

        let mut request = OpenRouterChatRequest {
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
                OpenRouterMessage {
                    role: "system".to_string(),
                    content: system.clone(),
                    name: None,
                },
            );
        }

        let url = format!("{}/chat/completions", OPENROUTER_API_URL);

        let mut req_builder = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json");

        // Add optional headers for OpenRouter attribution
        if let Some(ref site_url) = self.site_url {
            req_builder = req_builder.header("HTTP-Referer", site_url);
        }
        if let Some(ref site_name) = self.site_name {
            req_builder = req_builder.header("X-Title", site_name);
        }

        let response = req_builder.json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();

            if status == 429 {
                return Err(ProviderError::RateLimited { retry_after: None });
            }

            return Err(ProviderError::ApiError {
                status,
                message: body,
            });
        }

        let chat_response: OpenRouterChatResponse = response.json().await?;

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

// OpenRouter API types (OpenAI compatible)

#[derive(Debug, Serialize)]
struct OpenRouterChatRequest {
    model: String,
    messages: Vec<OpenRouterMessage>,
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
struct OpenRouterMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterChatResponse {
    choices: Vec<OpenRouterChoice>,
    usage: Option<OpenRouterUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponseMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModelsResponse {
    data: Vec<OpenRouterModel>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModel {
    id: String,
    name: Option<String>,
    context_length: Option<u32>,
    architecture: Option<ModelArchitecture>,
}

#[derive(Debug, Deserialize)]
struct ModelArchitecture {
    modality: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openrouter_provider_new() {
        let provider = OpenRouterProvider::new("sk-or-test");
        assert_eq!(provider.name(), "openrouter");
    }

    #[test]
    fn test_openrouter_provider_with_default_model() {
        let provider = OpenRouterProvider::new("sk-or-test")
            .with_default_model("anthropic/claude-3-opus");
        assert_eq!(provider.default_model, "anthropic/claude-3-opus");
    }

    #[test]
    fn test_openrouter_provider_with_site_url() {
        let provider = OpenRouterProvider::new("sk-or-test")
            .with_site_url("https://myapp.com");
        assert_eq!(provider.site_url.as_deref(), Some("https://myapp.com"));
    }

    #[test]
    fn test_openrouter_provider_with_site_name() {
        let provider = OpenRouterProvider::new("sk-or-test")
            .with_site_name("My App");
        assert_eq!(provider.site_name.as_deref(), Some("My App"));
    }

    #[test]
    fn test_openrouter_provider_chained() {
        let provider = OpenRouterProvider::new("sk-or-test")
            .with_default_model("meta/llama-3")
            .with_site_url("https://ganesha.ai")
            .with_site_name("Ganesha");
        assert_eq!(provider.default_model, "meta/llama-3");
        assert_eq!(provider.site_url.as_deref(), Some("https://ganesha.ai"));
        assert_eq!(provider.site_name.as_deref(), Some("Ganesha"));
    }
}
