//! # Anthropic Provider
//!
//! Implementation for Anthropic Claude API.

use crate::{
    GenerateOptions, LlmProvider, Message, MessageRole, ModelInfo, ModelTier,
    ProviderError, Response, Result, Usage, get_model_tier,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

const ANTHROPIC_API_VERSION: &str = "2023-06-01";

/// Anthropic Claude provider
pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    default_model: String,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            default_model: "claude-3-5-sonnet-20241022".to_string(),
        }
    }

    /// Set the default model
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Convert our messages to Anthropic format
    /// Anthropic requires system prompt separate from messages
    fn convert_messages(&self, messages: &[Message]) -> (Option<String>, Vec<AnthropicMessage>) {
        let mut system = None;
        let mut anthropic_messages = Vec::new();

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    system = Some(msg.content.clone());
                }
                MessageRole::User => {
                    anthropic_messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: vec![ContentBlock::Text {
                            text: msg.content.clone(),
                        }],
                    });
                }
                MessageRole::Assistant => {
                    anthropic_messages.push(AnthropicMessage {
                        role: "assistant".to_string(),
                        content: vec![ContentBlock::Text {
                            text: msg.content.clone(),
                        }],
                    });
                }
                MessageRole::Tool => {
                    // Tool results in Anthropic format
                    anthropic_messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: vec![ContentBlock::ToolResult {
                            tool_use_id: msg.tool_call_id.clone().unwrap_or_default(),
                            content: msg.content.clone(),
                        }],
                    });
                }
            }
        }

        (system, anthropic_messages)
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    fn default_model(&self) -> &str {
        &self.default_model
    }

    fn model_tier(&self, model: &str) -> ModelTier {
        get_model_tier(model)
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        // Anthropic doesn't have a models endpoint, return known models
        Ok(vec![
            ModelInfo {
                id: "claude-3-5-sonnet-20241022".to_string(),
                name: "Claude 3.5 Sonnet".to_string(),
                provider: "anthropic".to_string(),
                tier: ModelTier::Exceptional,
                context_length: Some(200000),
                supports_vision: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "claude-3-opus-20240229".to_string(),
                name: "Claude 3 Opus".to_string(),
                provider: "anthropic".to_string(),
                tier: ModelTier::Exceptional,
                context_length: Some(200000),
                supports_vision: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "claude-3-5-haiku-20241022".to_string(),
                name: "Claude 3.5 Haiku".to_string(),
                provider: "anthropic".to_string(),
                tier: ModelTier::Capable,
                context_length: Some(200000),
                supports_vision: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "claude-3-haiku-20240307".to_string(),
                name: "Claude 3 Haiku".to_string(),
                provider: "anthropic".to_string(),
                tier: ModelTier::Capable,
                context_length: Some(200000),
                supports_vision: true,
                supports_tools: true,
            },
        ])
    }

    async fn chat(&self, messages: &[Message], options: &GenerateOptions) -> Result<Response> {
        let model = options
            .model
            .as_ref()
            .unwrap_or(&self.default_model)
            .clone();

        debug!("Anthropic chat with model: {}", model);

        let (mut system, anthropic_messages) = self.convert_messages(messages);

        // Override system if provided in options
        if let Some(sys) = &options.system {
            system = Some(sys.clone());
        }

        let request = AnthropicChatRequest {
            model: model.clone(),
            messages: anthropic_messages,
            system,
            max_tokens: options.max_tokens.unwrap_or(4096),
            temperature: options.temperature,
            stop_sequences: options.stop.clone(),
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_API_VERSION)
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

        let chat_response: AnthropicChatResponse = response.json().await?;

        // Extract text content from response
        let content = chat_response
            .content
            .into_iter()
            .filter_map(|block| match block {
                ResponseContentBlock::Text { text } => Some(text),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        Ok(Response {
            content,
            model,
            finish_reason: Some(chat_response.stop_reason.unwrap_or_default()),
            usage: Some(Usage {
                prompt_tokens: chat_response.usage.input_tokens,
                completion_tokens: chat_response.usage.output_tokens,
                total_tokens: chat_response.usage.input_tokens
                    + chat_response.usage.output_tokens,
            }),
        })
    }
}

// Anthropic API types

#[derive(Debug, Serialize)]
struct AnthropicChatRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<ContentBlock>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_result")]
    ToolResult { tool_use_id: String, content: String },
}

#[derive(Debug, Deserialize)]
struct AnthropicChatResponse {
    content: Vec<ResponseContentBlock>,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum ResponseContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_provider_new() {
        let provider = AnthropicProvider::new("sk-ant-test");
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn test_anthropic_provider_with_default_model() {
        let provider = AnthropicProvider::new("sk-ant-test")
            .with_default_model("claude-3-opus");
        assert_eq!(provider.default_model, "claude-3-opus");
    }

    #[test]
    fn test_anthropic_provider_default_model_none() {
        let provider = AnthropicProvider::new("sk-ant-test");
        assert!(!provider.default_model.is_empty()); // has a default
    }
}
